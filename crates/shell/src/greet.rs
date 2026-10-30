use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use apolline_theme::{Palette, Rgba, neutral};
use chrono::{DateTime, Local};
use iced::widget::operation::focus;
use iced::widget::{
    Row, Space, button, column, container, image, pick_list, row, space, stack, text, text_input,
};
use iced::{
    Alignment, Background, Border, ContentFit, Element, Fill, Font, Subscription, Task, Theme,
    window,
};

use crate::icon::{Icon, icon};
use crate::{fonts, greetd, shot, style, wallpaper};

const FIELD: &str = "greet-password";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub login: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub name: String,
    pub command: Vec<String>,
}

impl std::fmt::Display for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Debug, Clone)]
enum Message {
    User(usize),
    Session(Session),
    Password(String),
    Submit,
    Done(Result<(), String>),
    Power(&'static str),
    Wallpaper(Result<image::Handle, String>),
    Tick,
    Shot(window::Screenshot),
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Typing,
    Checking,
    Wrong(String),
}

struct Greet {
    users: Vec<Account>,
    user: usize,
    sessions: Vec<Session>,
    session: Option<Session>,
    password: String,
    state: State,
    wallpaper: Option<image::Handle>,
    now: DateTime<Local>,
    socket: Option<String>,
    palette: Palette,
    screenshot: Option<PathBuf>,
}

pub fn main(args: &[String]) -> ExitCode {
    let preview = args.iter().any(|a| a == "--preview");
    let screenshot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let socket = std::env::var("GREETD_SOCK").ok();
    if socket.is_none() && !preview && screenshot.is_none() {
        eprintln!("apolline greet: run it from greetd, or add --preview");
        return ExitCode::from(2);
    }
    let (users, sessions) = if preview && socket.is_none() {
        (
            vec![
                Account {
                    login: "polina".into(),
                    name: "Polina".into(),
                },
                Account {
                    login: "danil".into(),
                    name: "Danil".into(),
                },
            ],
            vec![
                Session {
                    name: "Niri".into(),
                    command: vec!["niri-session".into()],
                },
                Session {
                    name: "Sway".into(),
                    command: vec!["sway".into()],
                },
            ],
        )
    } else {
        (
            accounts(&std::fs::read_to_string("/etc/passwd").unwrap_or_default()),
            sessions(),
        )
    };
    let session = sessions
        .iter()
        .find(|s| s.command.first().is_some_and(|c| c.contains("niri")))
        .or(sessions.first())
        .cloned();
    let greet = std::sync::Mutex::new(Some(Greet {
        users,
        user: 0,
        sessions,
        session,
        password: String::new(),
        state: State::Typing,
        wallpaper: None,
        now: Local::now(),
        socket,
        palette: Palette::default(),
        screenshot,
    }));
    let app = iced::application(
        move || {
            let greet = greet
                .lock()
                .ok()
                .and_then(|mut g| g.take())
                .expect("booted once");
            let load = Task::perform(wallpaper::load("fog.jpg".into()), Message::Wallpaper);
            (greet, Task::batch([load, focus(FIELD)]))
        },
        Greet::update,
        Greet::view,
    )
    .title("Apolline")
    .subscription(Greet::subscription)
    .theme(Theme::Dark)
    .default_font(Font::with_name(crate::style::family()))
    .window(window::Settings {
        size: iced::Size::new(1440.0, 900.0),
        decorations: false,
        ..window::Settings::default()
    });
    let result = fonts::ALL
        .into_iter()
        .fold(app, |app, font| app.font(font))
        .run();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("apolline greet: {err}");
            ExitCode::FAILURE
        }
    }
}

pub fn accounts(passwd: &str) -> Vec<Account> {
    passwd
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split(':').collect();
            let uid: u32 = f.get(2)?.parse().ok()?;
            let shell = f.get(6)?;
            if !(1000..60000).contains(&uid)
                || shell.ends_with("nologin")
                || shell.ends_with("false")
            {
                return None;
            }
            let gecos = f
                .get(4)
                .and_then(|g| g.split(',').next())
                .unwrap_or_default();
            Some(Account {
                login: f[0].to_owned(),
                name: if gecos.trim().is_empty() {
                    f[0].to_owned()
                } else {
                    gecos.trim().to_owned()
                },
            })
        })
        .collect()
}

fn sessions() -> Vec<Session> {
    let mut files: Vec<PathBuf> = std::fs::read_dir("/usr/share/wayland-sessions")
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    files
        .iter()
        .filter_map(|path| session(&std::fs::read_to_string(path).ok()?))
        .collect()
}

pub fn session(desktop: &str) -> Option<Session> {
    let mut name = None;
    let mut exec = None;
    let mut entry = false;
    for line in desktop.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            entry = line == "[Desktop Entry]";
            continue;
        }
        if !entry {
            continue;
        }
        if let Some(v) = line.strip_prefix("Name=") {
            name.get_or_insert(v.to_owned());
        } else if let Some(v) = line.strip_prefix("Exec=") {
            exec.get_or_insert(v.to_owned());
        }
    }
    let command: Vec<String> = exec?
        .split_whitespace()
        .filter(|word| !word.starts_with('%'))
        .map(str::to_owned)
        .collect();
    (!command.is_empty()).then(|| Session {
        name: name.unwrap_or_else(|| command[0].clone()),
        command,
    })
}

impl Greet {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::User(index) => {
                self.user = index;
                self.state = State::Typing;
                return focus(FIELD);
            }
            Message::Session(session) => self.session = Some(session),
            Message::Password(password) => {
                self.password = password;
                if matches!(self.state, State::Wrong(_)) {
                    self.state = State::Typing;
                }
            }
            Message::Submit if self.state != State::Checking && !self.password.is_empty() => {
                let (Some(user), Some(session)) = (self.users.get(self.user), &self.session) else {
                    return Task::none();
                };
                let Some(socket) = self.socket.clone() else {
                    self.state = State::Wrong("This is a preview: greetd isn't here".into());
                    return Task::none();
                };
                self.state = State::Checking;
                let (login, password, command) = (
                    user.login.clone(),
                    std::mem::take(&mut self.password),
                    session.command.clone(),
                );
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            greetd::login(&socket, &login, &password, &command)
                        })
                        .await
                        .unwrap_or_else(|err| Err(err.to_string()))
                    },
                    Message::Done,
                );
            }
            Message::Submit => {}
            Message::Done(Ok(())) => return iced::exit(),
            Message::Done(Err(err)) => {
                self.state = State::Wrong(err);
                return focus(FIELD);
            }
            Message::Power(action) => {
                return Task::future(async move {
                    let _ = tokio::process::Command::new("systemctl")
                        .arg(action)
                        .status()
                        .await;
                })
                .discard();
            }
            Message::Wallpaper(Ok(handle)) => self.wallpaper = Some(handle),
            Message::Wallpaper(Err(err)) => eprintln!("apolline greet: {err}"),
            Message::Tick => {
                self.now = Local::now();
                if self.screenshot.is_some() && self.wallpaper.is_some() {
                    return window::latest()
                        .and_then(window::screenshot)
                        .map(Message::Shot);
                }
            }
            Message::Shot(capture) => {
                if let Some(path) = self.screenshot.take()
                    && let Err(err) = shot::save(&path, &capture)
                {
                    eprintln!("apolline greet: could not save {}: {err}", path.display());
                }
                return iced::exit();
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick)
    }

    fn view(&self) -> Element<'_, Message> {
        let palette = self.palette;
        let picture: Element<'_, Message> = match &self.wallpaper {
            Some(handle) => image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Cover)
                .into(),
            None => Space::new().width(Fill).height(Fill).into(),
        };
        let shade = container(Space::new().width(Fill).height(Fill))
            .style(|_| container::background(style::color(Rgba::hex(0x0c0d10).alpha(0.35))));

        let power = |glyph: Icon, action: &'static str| {
            button(container(icon(glyph, 16.0, neutral::TEXT)).center(36))
                .padding(0)
                .style(style::ghost(18.0))
                .on_press(Message::Power(action))
        };
        let mut top = row![space::horizontal()]
            .spacing(8)
            .align_y(Alignment::Center);
        if self.sessions.len() > 1 {
            top = top.push(
                pick_list(
                    self.sessions.clone(),
                    self.session.clone(),
                    Message::Session,
                )
                .text_size(13)
                .padding([7, 12]),
            );
        }
        top = top
            .push(power(Icon::Restart, "reboot"))
            .push(power(Icon::Power, "poweroff"));

        let clock = column![
            text(self.now.format("%A, %-d %B").to_string().to_uppercase())
                .size(14)
                .font(style::medium())
                .color(style::color(neutral::TEXT.alpha(0.86))),
            text(self.now.format("%H:%M").to_string())
                .size(200)
                .font(style::light()),
        ]
        .spacing(0);

        let mut left = column![clock].spacing(22);
        if self.users.len() > 1 {
            left = left.push(
                Row::with_children(self.users.iter().enumerate().map(|(index, user)| {
                    button(text(user.name.as_str()).size(13))
                        .padding([6, 14])
                        .style(style::choice(index == self.user))
                        .on_press(Message::User(index))
                        .into()
                }))
                .spacing(6),
            );
        }
        left = left.push(self.field(palette));
        if let State::Wrong(said) = &self.state {
            left = left.push(
                text(said.as_str())
                    .size(13)
                    .color(style::color(style::WARN)),
            );
        }

        stack![
            picture,
            shade,
            column![
                container(top).padding([28, 36]),
                space::vertical(),
                container(left).padding(iced::Padding::new(88.0).top(0.0)),
            ]
            .width(Fill)
            .height(Fill),
        ]
        .into()
    }

    fn field(&self, palette: Palette) -> Element<'_, Message> {
        let name = self.users.get(self.user).map_or("", |u| u.name.as_str());
        let initial: String = name
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        let checking = self.state == State::Checking;
        let mut input = text_input(
            if checking { "Checking…" } else { "Password" },
            &self.password,
        )
        .id(FIELD)
        .secure(true)
        .size(15)
        .padding(0)
        .style(style::bare_field(palette));
        if !checking {
            input = input.on_input(Message::Password).on_submit(Message::Submit);
        }
        let avatar = container(
            text(initial)
                .size(14)
                .font(style::semibold())
                .color(style::color(palette.on_accent)),
        )
        .center(36)
        .style(style::pill(palette.accent));
        let go = button(container(icon(Icon::Arrow, 16.0, neutral::TEXT)).center(36))
            .padding(0)
            .style(style::ghost(18.0))
            .on_press_maybe((!checking).then_some(Message::Submit));
        let edge = match self.state {
            State::Wrong(_) => style::DANGER.alpha(0.8),
            _ => Rgba::hex(0xffffff).alpha(0.12),
        };
        container(
            row![avatar, text(name).size(14).font(style::medium()), input, go]
                .spacing(12)
                .align_y(Alignment::Center),
        )
        .width(420)
        .height(56)
        .padding([0, 10])
        .align_y(Alignment::Center)
        .style(move |_| container::Style {
            background: Some(Background::Color(style::color(
                Rgba::hex(0x0c0d10).alpha(0.5),
            ))),
            border: Border {
                color: style::color(edge),
                width: 1.0,
                radius: 28.0.into(),
            },
            ..container::Style::default()
        })
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn people_and_sessions_come_from_the_system_files() {
        let passwd = "root:x:0:0::/root:/bin/bash\nnobody:x:65534:65534::/:/usr/bin/nologin\npolina:x:1000:1000:Polina,,,:/home/polina:/bin/zsh\ngreeter:x:964:964::/var/lib/greetd:/usr/bin/nologin\ndanil:x:1001:1001::/home/danil:/bin/bash\n";
        let found = accounts(passwd);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "Polina");
        assert_eq!(found[1].name, "danil");

        let niri = session("[Desktop Entry]\nName=Niri\nComment=A scrollable-tiling compositor\nExec=niri-session\nType=Application\n[Desktop Action x]\nName=Other\n").unwrap();
        assert_eq!(niri.name, "Niri");
        assert_eq!(niri.command, ["niri-session"]);
        assert_eq!(
            session("[Desktop Entry]\nName=Odd\nExec=start %U\n")
                .unwrap()
                .command,
            ["start"]
        );
        assert!(session("[Desktop Entry]\nName=Broken\n").is_none());
    }
}
