use std::collections::HashMap;
use std::sync::Mutex;

use crate::lit::button;
use crate::style::text;
use apolline_theme::Palette;
use iced::futures::channel::{mpsc, oneshot};
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{Row, column, container, mouse_area, opaque, row, text_input};
use iced::{Alignment, Element, Fill, Padding, Subscription};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use zbus::fdo::DBusProxy;
use zbus::message::Header;
use zbus::names::WellKnownName;
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
    pub info: Option<String>,
    // What PAM asked for last, when it isn't just the password.
    pub asks: Option<String>,
    pub busy: bool,
    pub answers: Option<mpsc::Sender<String>>,
    pub check: Option<iced::task::Handle>,
}

impl Dialog {
    pub fn new(request: Request) -> Self {
        Self {
            request,
            user: 0,
            password: String::new(),
            error: None,
            info: None,
            asks: None,
            busy: false,
            answers: None,
            check: None,
        }
    }
}

static WAITING: Mutex<Vec<(String, oneshot::Sender<bool>)>> = Mutex::new(Vec::new());
// polkitd asks one thing at a time, give or take; more than this is someone else.
const MOST_WAITING: usize = 8;
const POLKIT: &str = "org.freedesktop.PolicyKit1";

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

// Only polkitd itself may ask; anyone on the bus could otherwise put up a password dialog.
async fn from_polkit(connection: &Connection, header: &Header<'_>) -> bool {
    let (Some(sender), Ok(bus)) = (header.sender(), DBusProxy::new(connection).await) else {
        return false;
    };
    let polkit = WellKnownName::from_static_str_unchecked(POLKIT).into();
    bus.get_name_owner(polkit)
        .await
        .is_ok_and(|owner| owner.as_str() == sender.as_str())
}

fn refused() -> fdo::Error {
    fdo::Error::AccessDenied("only polkit asks".into())
}

#[zbus::interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl Agent {
    #[allow(clippy::too_many_arguments)]
    async fn begin_authentication(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        action_id: String,
        message: String,
        _icon_name: String,
        _details: HashMap<String, String>,
        cookie: String,
        identities: Vec<(String, HashMap<String, OwnedVariant>)>,
    ) -> fdo::Result<()> {
        if !from_polkit(connection, &header).await {
            return Err(refused());
        }
        let (done, wait) = oneshot::channel();
        {
            let mut waiting = WAITING
                .lock()
                .map_err(|_| fdo::Error::Failed("the agent broke".into()))?;
            waiting.retain(|(_, done)| !done.is_canceled());
            if waiting.len() >= MOST_WAITING {
                return Err(fdo::Error::LimitsExceeded("too many asks at once".into()));
            }
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

    async fn cancel_authentication(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        cookie: String,
    ) -> fdo::Result<()> {
        if !from_polkit(connection, &header).await {
            return Err(refused());
        }
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
            Some(POLKIT),
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
    let lookup = |db: &str, id: u32| {
        std::process::Command::new("getent")
            .args([db, &id.to_string()])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let number = |details: &HashMap<String, OwnedVariant>, key: &str| {
        details.get(key).and_then(|v| u32::try_from(v).ok())
    };
    let mut users = Vec::new();
    for (kind, details) in identities {
        match kind.as_str() {
            "unix-user" => {
                if let Some(name) = number(details, "uid").and_then(|uid| {
                    lookup("passwd", uid)
                        .and_then(|line| login(&line, uid))
                        .or_else(|| login(&passwd, uid))
                }) {
                    users.push(name);
                }
            }
            "unix-group" => {
                if let Some(gid) = number(details, "gid") {
                    let line = lookup("group", gid).unwrap_or_else(|| groups.clone());
                    users.extend(members(&line, &passwd, gid));
                }
            }
            _ => {}
        }
    }
    let me = std::env::var("USER").unwrap_or_default();
    users.sort_by_key(|user| *user != me);
    let mut seen = std::collections::HashSet::new();
    users.retain(|user| seen.insert(user.clone()));
    if users.is_empty() && !me.is_empty() {
        users.push(me);
    }
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

// What the helper says while it checks.
#[derive(Clone, Debug, PartialEq)]
pub enum Said {
    // A line from PAM for the person: "Place your finger on the reader", "Account locked".
    Info(String),
    // PAM wants something typed that nothing typed yet answers: "Password:", "Verification code:".
    Asks(String),
    Done(Result<(), String>),
}

// One go at the helper, started when the dialog shows, so whatever PAM wants first, a finger or a
// key, is on screen before a password is typed; the password goes in when PAM asks for it.
pub fn conversation(
    user: String,
    cookie: String,
    mut answers: mpsc::Receiver<String>,
) -> impl Stream<Item = Said> {
    iced::stream::channel(4, async move |mut output| {
        let done = match helper(&user, &cookie).await {
            Ok((reader, writer, _child)) => {
                converse(reader, writer, &user, &mut answers, &mut output).await
            }
            Err(err) => Err(err),
        };
        let _ = output.send(Said::Done(done)).await;
    })
}

type Pipes = (
    Box<dyn AsyncBufRead + Unpin + Send>,
    Box<dyn AsyncWrite + Unpin + Send>,
    Option<tokio::process::Child>,
);

// polkit's socket-activated helper, or the setuid one where there is no socket.
async fn helper(user: &str, cookie: &str) -> Result<Pipes, String> {
    if let Ok(stream) = tokio::net::UnixStream::connect(SOCKET).await {
        let (read, mut write) = stream.into_split();
        write
            .write_all(format!("{user}\n{cookie}\n").as_bytes())
            .await
            .map_err(|err| err.to_string())?;
        return Ok((Box::new(BufReader::new(read)), Box::new(write), None));
    }
    let mut child = tokio::process::Command::new(HELPER)
        .arg(user)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|err| format!("{HELPER}: {err}"))?;
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return Err("the polkit helper has no pipes".into());
    };
    stdin
        .write_all(format!("{cookie}\n").as_bytes())
        .await
        .map_err(|err| err.to_string())?;
    Ok((
        Box::new(BufReader::new(stdout)),
        Box::new(stdin),
        Some(child),
    ))
}

async fn converse(
    reader: impl AsyncBufRead + Unpin,
    mut writer: impl AsyncWrite + Unpin,
    user: &str,
    answers: &mut mpsc::Receiver<String>,
    output: &mut mpsc::Sender<Said>,
) -> Result<(), String> {
    let mut lines = reader.lines();
    let mut said = None;
    // The password typed before anything was asked, kept for the hidden prompt it's meant for.
    let mut ahead = None;
    while let Some(line) = lines.next_line().await.map_err(|err| err.to_string())? {
        let (kind, rest) = line.split_once(' ').unwrap_or((line.as_str(), ""));
        let answer = match kind {
            "PAM_PROMPT_ECHO_OFF" => {
                // Typed ahead, it goes in at once; otherwise the dialog says what's asked and waits.
                let typed = match ahead.take().or_else(|| answers.try_recv().ok()) {
                    Some(typed) => typed,
                    None => {
                        let _ = output.send(Said::Asks(unescape(rest))).await;
                        answers.next().await.ok_or("the dialog went away")?
                    }
                };
                Some(typed)
            }
            "PAM_PROMPT_ECHO_ON" if asks_for_login(&unescape(rest)) => Some(user.to_owned()),
            // Something shown as it's typed, a code say: the person answers it, never the password.
            "PAM_PROMPT_ECHO_ON" => {
                ahead = ahead.or_else(|| answers.try_recv().ok());
                let _ = output.send(Said::Asks(unescape(rest))).await;
                Some(answers.next().await.ok_or("the dialog went away")?)
            }
            "PAM_ERROR_MSG" | "PAM_TEXT_INFO" => {
                let text = unescape(rest);
                let _ = output.send(Said::Info(text.clone())).await;
                said = Some(text);
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

fn asks_for_login(prompt: &str) -> bool {
    let prompt = prompt.to_lowercase();
    prompt.starts_with("login") || prompt.contains("user")
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
                    .color(style::color(style::soft_text())),
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
    let placeholder = dialog
        .asks
        .clone()
        .unwrap_or_else(|| crate::i18n::fill("Password for {user}", &[("user", &user)]));
    let field = text_input(&placeholder, &dialog.password)
        .id(FIELD)
        .secure(true)
        .on_input_maybe((!dialog.busy).then_some(Message::PolkitPassword))
        .on_submit(Message::PolkitSubmit)
        .padding([10, 14])
        .size(14)
        .style(style::field(palette));
    body = body.push(field);
    let note: Element<'_, Message> = match (&dialog.error, &dialog.info) {
        (Some(error), _) => text(error.as_str())
            .size(12)
            .color(style::color(style::WARN))
            .into(),
        (None, Some(info)) => text(info.as_str())
            .size(12)
            .color(style::color(style::soft_text()))
            .into(),
        (None, None) => text(request.action.as_str())
            .size(11)
            .font(style::mono())
            .color(style::color(style::faint()))
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
    let card = container(body)
        .padding(22)
        .width(460)
        .style(style::surface)
        .id(style::CARD);
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
        let (mut answer, mut answers) = mpsc::channel(1);
        let (mut output, said) = mpsc::channel(4);
        answer.try_send("polina".to_owned()).unwrap();
        let script = b"PAM_TEXT_INFO Touch\\nthe key\nPAM_PROMPT_ECHO_OFF Password: \nSUCCESS\n";
        let mut written = Vec::new();
        let result = converse(
            &script[..],
            &mut written,
            "dzorin",
            &mut answers,
            &mut output,
        )
        .await;
        assert_eq!(result, Ok(()));
        assert_eq!(written, b"polina\n");
        drop(output);
        let said: Vec<Said> = said.collect().await;
        assert_eq!(
            said,
            [Said::Info("Touch the key".into())],
            "shown before any password"
        );

        let (mut output, _said) = mpsc::channel(4);
        answer.try_send("nope".to_owned()).unwrap();
        let script = b"PAM_PROMPT_ECHO_OFF Password: \nPAM_ERROR_MSG Account locked for 10 minutes\nFAILURE\n";
        let result = converse(&script[..], Vec::new(), "dzorin", &mut answers, &mut output).await;
        assert_eq!(result, Err("Account locked for 10 minutes".into()));

        // A second question, a one-time code: the dialog hears it and the answer goes in after.
        let (mut output, said) = mpsc::channel(4);
        answer.try_send("polina".to_owned()).unwrap();
        let script =
            b"PAM_PROMPT_ECHO_OFF Password: \nPAM_PROMPT_ECHO_OFF Verification code: \nSUCCESS\n";
        let mut written = Vec::new();
        let talk = converse(
            &script[..],
            &mut written,
            "dzorin",
            &mut answers,
            &mut output,
        );
        let typed_later = async {
            answer.send("123456".to_owned()).await.unwrap();
        };
        let (result, ()) = iced::futures::join!(talk, typed_later);
        assert_eq!(result, Ok(()));
        assert_eq!(written, b"polina\n123456\n");
        drop(output);
        let said: Vec<Said> = said.collect().await;
        assert_eq!(said, [Said::Asks("Verification code:".into())]);

        // A visible question gets what's typed for it, not the password typed ahead.
        let (mut output, said) = mpsc::channel(4);
        answer.try_send("polina".to_owned()).unwrap();
        let script = b"PAM_PROMPT_ECHO_ON login: \nPAM_PROMPT_ECHO_ON One-time code: \nPAM_PROMPT_ECHO_OFF Password: \nSUCCESS\n";
        let mut written = Vec::new();
        let talk = converse(
            &script[..],
            &mut written,
            "dzorin",
            &mut answers,
            &mut output,
        );
        let typed_later = async {
            answer.send("424242".to_owned()).await.unwrap();
        };
        let (result, ()) = iced::futures::join!(talk, typed_later);
        assert_eq!(result, Ok(()));
        assert_eq!(written, b"dzorin\n424242\npolina\n");
        drop(output);
        let said: Vec<Said> = said.collect().await;
        assert_eq!(said, [Said::Asks("One-time code:".into())]);

        drop(answer);
        let (mut output, _said) = mpsc::channel(4);
        let script = b"PAM_PROMPT_ECHO_OFF Password: \n";
        let result = converse(&script[..], Vec::new(), "dzorin", &mut answers, &mut output).await;
        assert!(result.is_err(), "a closed dialog ends it");
    }

    #[test]
    fn group_identities_expand_to_their_members() {
        let passwd = "root:x:0:0::/root:/bin/bash\ndzorin:x:1000:1000::/home/dzorin:/bin/zsh\nops:x:1001:998::/home/ops:/bin/sh\n";
        let groups = "wheel:x:998:dzorin\n";
        assert_eq!(login(passwd, 1000).as_deref(), Some("dzorin"));
        assert_eq!(members(groups, passwd, 998), ["dzorin", "ops"]);
    }
}
