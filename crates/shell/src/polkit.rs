use std::collections::HashMap;
use std::sync::Mutex;

use apolline_theme::Palette;
use iced::futures::channel::{mpsc, oneshot};
use iced::futures::{SinkExt, Stream};
use iced::widget::{Row, button, column, container, mouse_area, opaque, row, text, text_input};
use iced::{Alignment, Element, Fill, Padding, Subscription};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use zbus::{Connection, fdo};
use zbus::{OwnedVariant, Variant};

use crate::icon::{Icon, icon};
use crate::shell::Message;
use crate::style;

pub const FIELD: &str = "polkit-password";
const PATH: &str = "/org/apolline/PolicyKit1/AuthenticationAgent";
const SOCKET: &str = "/run/polkit/agent-helper.socket";
const HELPER: &str = "/usr/lib/polkit-1/polkit-agent-helper-1";

#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub cookie: String,
    pub message: String,
    pub action: String,
    pub users: Vec<String>,
}

pub struct Dialog {
    pub request: Request,
    pub user: usize,
    pub password: String,
    pub error: Option<String>,
    pub busy: bool,
}

impl Dialog {
    pub fn new(request: Request) -> Self {
        Self {
            request,
            user: 0,
            password: String::new(),
            error: None,
            busy: false,
        }
    }
}

static WAITING: Mutex<Vec<(String, oneshot::Sender<bool>)>> = Mutex::new(Vec::new());

pub fn finish(cookie: &str, granted: bool) {
    let Ok(mut waiting) = WAITING.lock() else {
        return;
    };
    if let Some(index) = waiting.iter().position(|(c, _)| c == cookie) {
        let (_, done) = waiting.remove(index);
        let _ = done.send(granted);
    }
}

struct Agent {
    output: mpsc::Sender<Message>,
}

#[zbus::interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl Agent {
    async fn begin_authentication(
        &self,
        action_id: String,
        message: String,
        _icon_name: String,
        _details: HashMap<String, String>,
        cookie: String,
        identities: Vec<(String, HashMap<String, OwnedVariant>)>,
    ) -> fdo::Result<()> {
        let (done, wait) = oneshot::channel();
        if let Ok(mut waiting) = WAITING.lock() {
            waiting.push((cookie.clone(), done));
        }
        let request = Request {
            cookie,
            message,
            action: action_id,
            users: users(&identities),
        };
        let _ = self.output.clone().send(Message::Polkit(request)).await;
        match wait.await {
            Ok(true) => Ok(()),
            _ => Err(fdo::Error::Failed("the dialog was dismissed".into())),
        }
    }

    async fn cancel_authentication(&self, cookie: String) -> fdo::Result<()> {
        finish(&cookie, false);
        let _ = self.output.clone().send(Message::PolkitGone(cookie)).await;
        Ok(())
    }
}

pub fn subscription() -> Subscription<Message> {
    Subscription::run(serve)
}

fn serve() -> impl Stream<Item = Message> {
    iced::stream::channel(4, async |output| {
        let builder = zbus::connection::Builder::system().serve_at(PATH, Agent { output });
        let Ok(connection) = builder.build().await else {
            return;
        };
        if let Err(err) = register(&connection).await {
            eprintln!("apolline: no polkit agent: {err}");
            return;
        }
        iced::futures::future::pending::<()>().await;
        drop(connection);
    })
}

async fn register(connection: &Connection) -> Result<(), String> {
    let session = crate::logind::session_id(connection)
        .await
        .ok_or("not in a login session")?;
    let details = HashMap::from([("session-id", Variant::from(session.as_str()))]);
    let locale = std::env::var("LANG").unwrap_or_else(|_| "en_US.UTF-8".into());
    connection
        .call_method(
            Some("org.freedesktop.PolicyKit1"),
            "/org/freedesktop/PolicyKit1/Authority",
            Some("org.freedesktop.PolicyKit1.Authority"),
            "RegisterAuthenticationAgent",
            &(("unix-session", details), locale, PATH),
        )
        .await
        .map(drop)
        .map_err(|err| err.to_string())
}

fn users(identities: &[(String, HashMap<String, OwnedVariant>)]) -> Vec<String> {
    let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
    let groups = std::fs::read_to_string("/etc/group").unwrap_or_default();
    let number = |details: &HashMap<String, OwnedVariant>, key: &str| {
        details.get(key).and_then(|v| u32::try_from(v).ok())
    };
    let mut users = Vec::new();
    for (kind, details) in identities {
        match kind.as_str() {
            "unix-user" => {
                if let Some(name) = number(details, "uid").and_then(|uid| login(&passwd, uid)) {
                    users.push(name);
                }
            }
            "unix-group" => {
                if let Some(gid) = number(details, "gid") {
                    users.extend(members(&groups, &passwd, gid));
                }
            }
            _ => {}
        }
    }
    let me = std::env::var("USER").unwrap_or_default();
    users.sort_by_key(|user| *user != me);
    users.dedup();
    users
}

fn login(passwd: &str, uid: u32) -> Option<String> {
    passwd.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        (fields.get(2)?.parse::<u32>().ok()? == uid).then(|| fields[0].to_owned())
    })
}

fn members(groups: &str, passwd: &str, gid: u32) -> Vec<String> {
    let mut found: Vec<String> = groups
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(':').collect();
            (fields.get(2)?.parse::<u32>().ok()? == gid).then(|| fields.get(3).copied())?
        })
        .flat_map(|list| list.split(',').filter(|m| !m.is_empty()).map(str::to_owned))
        .collect();
    found.extend(passwd.lines().filter_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        (fields.get(3)?.parse::<u32>().ok()? == gid).then(|| fields[0].to_owned())
    }));
    found
}

pub async fn authenticate(user: String, cookie: String, password: String) -> Result<(), String> {
    match tokio::net::UnixStream::connect(SOCKET).await {
        Ok(stream) => {
            let (read, mut write) = stream.into_split();
            write
                .write_all(format!("{user}\n{cookie}\n").as_bytes())
                .await
                .map_err(|err| err.to_string())?;
            converse(BufReader::new(read), write, &user, &password).await
        }
        Err(_) => {
            let mut helper = tokio::process::Command::new(HELPER)
                .arg(&user)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|err| format!("{HELPER}: {err}"))?;
            let (Some(mut stdin), Some(stdout)) = (helper.stdin.take(), helper.stdout.take())
            else {
                return Err("the polkit helper has no pipes".into());
            };
            stdin
                .write_all(format!("{cookie}\n").as_bytes())
                .await
                .map_err(|err| err.to_string())?;
            converse(BufReader::new(stdout), stdin, &user, &password).await
        }
    }
}

async fn converse(
    reader: impl AsyncBufRead + Unpin,
    mut writer: impl AsyncWrite + Unpin,
    user: &str,
    password: &str,
) -> Result<(), String> {
    let mut lines = reader.lines();
    let mut said = None;
    while let Some(line) = lines.next_line().await.map_err(|err| err.to_string())? {
        let (kind, rest) = line.split_once(' ').unwrap_or((line.as_str(), ""));
        let answer = match kind {
            "PAM_PROMPT_ECHO_OFF" => Some(password),
            "PAM_PROMPT_ECHO_ON" => Some(user),
            "PAM_ERROR_MSG" | "PAM_TEXT_INFO" => {
                said = Some(unescape(rest));
                None
            }
            "SUCCESS" => return Ok(()),
            "FAILURE" => return Err(said.unwrap_or_else(|| "Wrong password".into())),
            _ => None,
        };
        if let Some(answer) = answer {
            writer
                .write_all(format!("{answer}\n").as_bytes())
                .await
                .map_err(|err| err.to_string())?;
        }
    }
    Err("the polkit helper stopped answering".into())
}

fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 't') => out.push(' '),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out.trim().to_owned()
}

pub fn view(dialog: &Dialog, palette: Palette) -> Element<'_, Message> {
    let request = &dialog.request;
    let user = request.users.get(dialog.user).map_or("", String::as_str);
    let mut body = column![
        row![
            container(icon(Icon::Lock, 20.0, palette.accent))
                .center(40)
                .style(style::pill(palette.accent_soft)),
            column![
                text("An app asks for permission")
                    .size(17)
                    .font(style::semibold()),
                text(request.message.as_str())
                    .size(13)
                    .color(style::color(style::SOFT_TEXT)),
            ]
            .spacing(4)
            .width(Fill),
        ]
        .spacing(14),
    ]
    .spacing(18);
    if request.users.len() > 1 {
        let choices = Row::with_children(request.users.iter().enumerate().map(|(index, name)| {
            button(text(name.as_str()).size(12))
                .padding([5, 12])
                .style(style::choice(index == dialog.user))
                .on_press(Message::PolkitUser(index))
                .into()
        }))
        .spacing(4);
        body = body.push(choices);
    }
    let field = text_input(&format!("Password for {user}"), &dialog.password)
        .id(FIELD)
        .secure(true)
        .on_input_maybe((!dialog.busy).then_some(Message::PolkitPassword))
        .on_submit(Message::PolkitSubmit)
        .padding([10, 14])
        .size(14)
        .style(style::field(palette));
    body = body.push(field);
    let note: Element<'_, Message> = match &dialog.error {
        Some(error) => text(error.as_str())
            .size(12)
            .color(style::color(style::WARN))
            .into(),
        None => text(request.action.as_str())
            .size(11)
            .font(style::mono())
            .color(style::color(style::FAINT))
            .into(),
    };
    body = body.push(
        row![
            container(note).width(Fill),
            button(text("Cancel").size(13))
                .padding([8, 16])
                .style(style::secondary)
                .on_press(Message::PolkitCancel),
            button(text(if dialog.busy { "Checking…" } else { "Allow" }).size(13))
                .padding([8, 18])
                .style(style::primary(palette))
                .on_press_maybe(
                    (!dialog.busy && !dialog.password.is_empty()).then_some(Message::PolkitSubmit)
                ),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    );
    let card = container(body).padding(22).width(460).style(style::surface);
    mouse_area(
        container(opaque(card))
            .width(Fill)
            .height(Fill)
            .align_x(Alignment::Center)
            .padding(Padding::new(0.0).top(200.0))
            .style(style::scrim),
    )
    .on_press(Message::PolkitCancel)
    .into()
}

pub fn sample() -> Request {
    Request {
        cookie: "sample".into(),
        message: "Authentication is required to run /usr/bin/gparted as the super user".into(),
        action: "org.freedesktop.policykit.exec".into(),
        users: vec!["you".into(), "admin".into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helper_conversation_answers_prompts_and_reads_the_verdict() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(conversation());
    }

    async fn conversation() {
        let script = b"PAM_TEXT_INFO Touch\\nthe key\nPAM_PROMPT_ECHO_OFF Password: \nSUCCESS\n";
        let mut written = Vec::new();
        let result = converse(&script[..], &mut written, "dzorin", "polina").await;
        assert_eq!(result, Ok(()));
        assert_eq!(written, b"polina\n");

        let script = b"PAM_PROMPT_ECHO_OFF Password: \nPAM_ERROR_MSG Account locked for 10 minutes\nFAILURE\n";
        let result = converse(&script[..], Vec::new(), "dzorin", "nope").await;
        assert_eq!(result, Err("Account locked for 10 minutes".into()));
    }

    #[test]
    fn group_identities_expand_to_their_members() {
        let passwd = "root:x:0:0::/root:/bin/bash\ndzorin:x:1000:1000::/home/dzorin:/bin/zsh\nops:x:1001:998::/home/ops:/bin/sh\n";
        let groups = "wheel:x:998:dzorin\n";
        assert_eq!(login(passwd, 1000).as_deref(), Some("dzorin"));
        assert_eq!(members(groups, passwd, 998), ["dzorin", "ops"]);
    }
}
