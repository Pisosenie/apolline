use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use crate::style::text;
use apolline_theme::{Palette, Rgba, neutral};
use chrono::{DateTime, Local};
use iced::gradient::{Gradient, Linear};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::operation::focus;
use iced::widget::{
    Row, Space, button, column, container, image, pick_list, row, space, stack, text_input,
};
use iced::{
    Alignment, Background, Border, ContentFit, Element, Fill, Font, Radians, Subscription, Task,
    window,
};

use crate::greet_look::{self, Look};
use crate::icon::{Icon, icon};
use crate::{fonts, greetd, shot, style, wallpaper};

const FIELD: &str = "greet-password";
const LOGIN: &str = "greet-login";

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
    Login(String),
    ToPassword,
    Session(Session),
    Password(String),
    Submit,
    Done(Result<(), String>),
    Power(&'static str),
    Wallpaper(Result<image::Handle, String>),
    Tick,
    Shot(window::Screenshot),
    Leave,
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
    login: String,
    password: String,
    state: State,
    wallpaper: Option<image::Handle>,
    now: DateTime<Local>,
    socket: Option<String>,
    look: Look,
    preview: bool,
    screenshot: Option<PathBuf>,
}

pub fn main(args: &[String]) -> ExitCode {
    if args.iter().any(|a| a == "--kiosk") {
        return kiosk();
    }
    let preview = args.iter().any(|a| a == "--preview");
    let screenshot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let look = Look::read(
        &args
            .iter()
            .position(|a| a == "--look")
            .and_then(|i| args.get(i + 1))
            .map_or_else(|| PathBuf::from(greet_look::SYSTEM), PathBuf::from),
    );
    if look.language.is_some() {
        crate::i18n::choose(look.language.as_deref());
    }
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
        (system_accounts(), sessions())
    };
    let (last_login, last_session) = if preview { (None, None) } else { last() };
    let session = sessions
        .iter()
        .find(|s| Some(&s.name) == last_session.as_ref())
        .or_else(|| {
            sessions
                .iter()
                .find(|s| s.command.first().is_some_and(|c| c.contains("niri")))
        })
        .or(sessions.first())
        .cloned();
    let user = users
        .iter()
        .position(|u| Some(&u.login) == last_login.as_ref())
        .unwrap_or(0);
    let login = if users.is_empty() {
        last_login.unwrap_or_default()
    } else {
        String::new()
    };
    let greet = std::sync::Mutex::new(Some(Greet {
        users,
        user,
        sessions,
        session,
        login,
        password: String::new(),
        state: State::Typing,
        wallpaper: None,
        now: Local::now(),
        socket,
        look: look.clone(),
        preview,
        screenshot,
    }));
    let app = iced::application(
        move || {
            let greet = greet
                .lock()
                .ok()
                .and_then(|mut g| g.take())
                .expect("booted once");
            let name = greet.look.wallpaper.clone();
            let load = if greet.look.blur {
                Task::perform(wallpaper::load_blurred(name), Message::Wallpaper)
            } else {
                Task::perform(wallpaper::load(name), Message::Wallpaper)
            };
            let first = if greet.users.is_empty() && greet.login.is_empty() {
                LOGIN
            } else {
                FIELD
            };
            (greet, Task::batch([load, focus(first)]))
        },
        Greet::update,
        Greet::view,
    )
    .title("Apolline")
    .subscription(Greet::subscription)
    .theme(|_: &Greet| style::theme())
    .default_font(Font::with_name(
        look.font
            .map_or_else(crate::style::family, |font| font.name()),
    ))
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

// More than this and the screen asks for a name instead of listing everyone.
const LISTED: usize = 12;

fn system_accounts() -> Vec<Account> {
    // getent also sees people from LDAP, sssd or systemd-homed, not just /etc/passwd;
    // a directory that doesn't answer in two seconds isn't waited for.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let listed = std::process::Command::new("getent")
            .arg("passwd")
            .stdin(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned());
        let _ = tx.send(listed);
    });
    let passwd = rx
        .recv_timeout(Duration::from_secs(2))
        .ok()
        .flatten()
        .unwrap_or_else(|| std::fs::read_to_string("/etc/passwd").unwrap_or_default());
    let defs = std::fs::read_to_string("/etc/login.defs").unwrap_or_default();
    let people = accounts(&passwd, people(&defs));
    if people.len() > LISTED {
        Vec::new()
    } else {
        people
    }
}

pub fn people(login_defs: &str) -> (u32, u32) {
    let value = |key: &str| {
        login_defs.lines().find_map(|line| {
            let mut words = line.split_whitespace();
            (words.next() == Some(key)).then(|| words.next()?.parse().ok())?
        })
    };
    (
        value("UID_MIN").unwrap_or(1000),
        value("UID_MAX").unwrap_or(60000),
    )
}

pub fn accounts(passwd: &str, (first, last): (u32, u32)) -> Vec<Account> {
    const HOMED: std::ops::RangeInclusive<u32> = 60001..=60513;
    let mut seen = Vec::new();
    passwd
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split(':').collect();
            let uid: u32 = f.get(2)?.parse().ok()?;
            let shell = f.get(6)?;
            if !((first..=last).contains(&uid) || HOMED.contains(&uid))
                || seen.contains(&f[0])
                || shell.ends_with("nologin")
                || shell.ends_with("false")
            {
                return None;
            }
            seen.push(f[0]);
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

// The greeter's own home, e.g. /var/lib/greetd, keeps who logged in last and how.
fn last_file() -> Option<PathBuf> {
    let state = apolline_config::xdg_dir("XDG_STATE_HOME", ".local/state")?;
    Some(state.join("apolline").join("last-login"))
}

fn last() -> (Option<String>, Option<String>) {
    let text = last_file()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    let mut lines = text
        .lines()
        .map(str::trim)
        .map(|l| (!l.is_empty()).then(|| l.to_owned()));
    (lines.next().flatten(), lines.next().flatten())
}

fn remember(login: &str, session: &str) {
    let Some(path) = last_file() else { return };
    if let Some(dir) = path.parent()
        && std::fs::create_dir_all(dir).is_ok()
    {
        let _ = std::fs::write(path, format!("{login}\n{session}\n"));
    }
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
            Message::User(_) | Message::Login(_) | Message::Session(_)
                if self.state == State::Checking => {}
            Message::User(index) => {
                self.user = index;
                self.state = State::Typing;
                return focus(FIELD);
            }
            Message::Login(login) => {
                self.login = login;
                if matches!(self.state, State::Wrong(_)) {
                    self.state = State::Typing;
                }
            }
            Message::ToPassword => return focus(FIELD),
            Message::Session(session) => self.session = Some(session),
            Message::Password(password) => {
                self.password = password;
                if matches!(self.state, State::Wrong(_)) {
                    self.state = State::Typing;
                }
            }
            Message::Submit if self.state != State::Checking && !self.password.is_empty() => {
                let Some(session) = &self.session else {
                    self.state = State::Wrong("No sessions in /usr/share/wayland-sessions".into());
                    return Task::none();
                };
                let login = match self.users.get(self.user) {
                    Some(user) => user.login.clone(),
                    None if !self.login.trim().is_empty() => self.login.trim().to_owned(),
                    None => {
                        self.state = State::Wrong("Type your username".into());
                        return focus(LOGIN);
                    }
                };
                let Some(socket) = self.socket.clone() else {
                    self.state = State::Wrong("This is a preview: greetd isn't here".into());
                    return Task::none();
                };
                self.state = State::Checking;
                let (password, command) =
                    (std::mem::take(&mut self.password), session.command.clone());
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
            Message::Done(Ok(())) => {
                let login = match self.users.get(self.user) {
                    Some(user) => user.login.as_str(),
                    None => self.login.trim(),
                };
                let session = self.session.as_ref().map_or("", |s| s.name.as_str());
                remember(login, session);
                return iced::exit();
            }
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
            Message::Leave => return iced::exit(),
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
        let tick = iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick);
        if !self.preview {
            return tick;
        }
        let leave = keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed {
                key: Key::Named(Named::Escape),
                ..
            } => Some(Message::Leave),
            _ => None,
        });
        Subscription::batch([tick, leave])
    }

    fn view(&self) -> Element<'_, Message> {
        style::within(self.look.font, || self.built())
    }

    fn built(&self) -> Element<'_, Message> {
        let palette = self.look.palette;
        let picture: Element<'_, Message> = match &self.wallpaper {
            Some(handle) => image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Cover)
                .into(),
            None => Space::new().width(Fill).height(Fill).into(),
        };
        let shade = container(Space::new().width(Fill).height(Fill)).style(|_| {
            let dark = |alpha: f32| style::color(Rgba::hex(0x0c0d10).alpha(alpha));
            container::Style {
                background: Some(Background::Gradient(Gradient::Linear(
                    Linear::new(Radians(std::f32::consts::PI))
                        .add_stop(0.0, dark(0.4))
                        .add_stop(0.25, dark(0.1))
                        .add_stop(0.5, dark(0.1))
                        .add_stop(1.0, dark(0.7)),
                ))),
                ..container::Style::default()
            }
        });

        let power = |glyph: Icon, action: &'static str| {
            button(container(icon(glyph, 16.0, neutral::text())).center(36))
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
            text(
                self.now
                    .format(&crate::i18n::names("%A, %-d %B", &self.now))
                    .to_string()
                    .to_uppercase()
            )
            .size(14)
            .font(style::medium())
            .color(style::color(neutral::text().alpha(0.86))),
            text(self.now.format("%H:%M").to_string())
                .size(200)
                .font(style::light()),
        ]
        .spacing(0);
        let centered = self.look.centered;
        let clock = if centered {
            clock.align_x(Alignment::Center)
        } else {
            clock
        };

        let mut left = column![clock].spacing(22);
        if self.users.len() > 1 {
            left = left.push(
                Row::with_children(self.users.iter().enumerate().map(|(index, user)| {
                    button(text(user.name.as_str()).size(13))
                        .padding([6, 14])
                        .style(style::choice(index == self.user))
                        .on_press_maybe(
                            (self.state != State::Checking).then_some(Message::User(index)),
                        )
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

        let body = if centered {
            column![
                container(top).padding([28, 36]),
                space::vertical(),
                container(left.align_x(Alignment::Center)).center_x(Fill),
                space::vertical(),
                space().height(56),
            ]
        } else {
            column![
                container(top).padding([28, 36]),
                space::vertical(),
                container(left).padding(iced::Padding::new(88.0).top(0.0)),
            ]
        };
        container(stack![picture, shade, body.width(Fill).height(Fill)])
            .style(|_| container::background(style::color(neutral::ink())))
            .into()
    }

    fn field(&self, palette: Palette) -> Element<'_, Message> {
        let name = self
            .users
            .get(self.user)
            .map_or(self.login.as_str(), |u| u.name.as_str());
        let initial: String = name
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        let checking = self.state == State::Checking;
        let who: Element<'_, Message> = if self.users.is_empty() {
            let mut login = text_input(crate::i18n::t("Username"), &self.login)
                .id(LOGIN)
                .width(120)
                .size(14)
                .padding(0)
                .style(style::bare_field(palette));
            if !checking {
                login = login
                    .on_input(Message::Login)
                    .on_submit(Message::ToPassword);
            }
            login.into()
        } else {
            text(name).size(14).font(style::medium()).into()
        };
        let mut input = text_input(
            crate::i18n::t(if checking { "Checking…" } else { "Password" }),
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
        let go = button(container(icon(Icon::Arrow, 16.0, neutral::text())).center(36))
            .padding(0)
            .style(style::ghost(18.0))
            .on_press_maybe((!checking).then_some(Message::Submit));
        let edge = match self.state {
            State::Wrong(_) => style::DANGER.alpha(0.8),
            _ => Rgba::hex(0xffffff).alpha(0.12),
        };
        container(
            row![avatar, who, input, go]
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

// greetd runs `apolline greet --kiosk`: cage, keyed the way the system is (what `localectl
// set-x11-keymap` wrote), with the greeter in it. Without this cage types in US English.
fn kiosk() -> ExitCode {
    let read = |path: &str| std::fs::read_to_string(path).unwrap_or_default();
    let keyboard = [
        xkb(&read("/etc/X11/xorg.conf.d/00-keyboard.conf")),
        xkb_vars(&read("/etc/vconsole.conf")),
        xkb_vars(&read("/etc/default/keyboard")),
    ]
    .into_iter()
    .find(|found| !found.is_empty())
    .unwrap_or_default();
    let me = std::env::current_exe().unwrap_or_else(|_| "apolline".into());
    let mut cage = std::process::Command::new("cage");
    cage.args(["-s", "--"]).arg(me).arg("greet");
    // Set in greetd's own config, they stay.
    for (key, value) in keyboard {
        if std::env::var_os(key).is_none() {
            cage.env(key, value);
        }
    }
    #[cfg(unix)]
    let err = std::os::unix::process::CommandExt::exec(&mut cage);
    #[cfg(not(unix))]
    let err = cage
        .status()
        .err()
        .unwrap_or_else(|| std::io::Error::other("cage quit"));
    eprintln!("apolline greet: cage: {err}");
    ExitCode::FAILURE
}

// The X keyboard options in an xorg.conf section, as the names xkbcommon reads from the environment.
fn xkb(conf: &str) -> Vec<(&'static str, String)> {
    conf.lines()
        .filter_map(|line| {
            let line = line.split('#').next()?.trim();
            let mut quoted = line.strip_prefix("Option")?.split('"').skip(1).step_by(2);
            let key = match quoted.next()? {
                "XkbLayout" => "XKB_DEFAULT_LAYOUT",
                "XkbVariant" => "XKB_DEFAULT_VARIANT",
                "XkbOptions" => "XKB_DEFAULT_OPTIONS",
                "XkbModel" => "XKB_DEFAULT_MODEL",
                _ => return None,
            };
            Some((key, quoted.next()?.to_owned()))
        })
        .collect()
}

// The same as shell variables, the way Debian's /etc/default/keyboard and newer systemd's
// /etc/vconsole.conf keep them: XKBLAYOUT="us,ru".
fn xkb_vars(text: &str) -> Vec<(&'static str, String)> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.split('#').next()?.trim().split_once('=')?;
            let key = match key.trim() {
                "XKBLAYOUT" => "XKB_DEFAULT_LAYOUT",
                "XKBVARIANT" => "XKB_DEFAULT_VARIANT",
                "XKBOPTIONS" => "XKB_DEFAULT_OPTIONS",
                "XKBMODEL" => "XKB_DEFAULT_MODEL",
                _ => return None,
            };
            let value = value.trim().trim_matches('"').trim_matches('\'');
            (!value.is_empty()).then(|| (key, value.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debian_and_vconsole_keyboards_count_too() {
        let debian = "XKBMODEL=\"pc105\"\nXKBLAYOUT=\"us,ru\"\nXKBVARIANT=\"\"\nXKBOPTIONS=\"grp:alt_shift_toggle\"\nBACKSPACE=\"guess\"\n";
        assert_eq!(
            xkb_vars(debian),
            [
                ("XKB_DEFAULT_MODEL", "pc105".to_owned()),
                ("XKB_DEFAULT_LAYOUT", "us,ru".to_owned()),
                ("XKB_DEFAULT_OPTIONS", "grp:alt_shift_toggle".to_owned()),
            ]
        );
        assert_eq!(
            xkb_vars("KEYMAP=ru\nXKBLAYOUT=us,ru\n"),
            [("XKB_DEFAULT_LAYOUT", "us,ru".to_owned())]
        );
    }

    #[test]
    fn the_system_keyboard_reaches_cage() {
        let conf = r#"# Written by systemd-localed(8), read by systemd-localed and Xorg.
Section "InputClass"
        Identifier "system-keyboard"
        MatchIsKeyboard "on"
        Option "XkbLayout" "us,ru"
        Option "XkbModel" "pc105"
        Option "XkbVariant" ","
        Option "XkbOptions" "grp:alt_shift_toggle" # switch with Alt+Shift
EndSection
"#;
        assert_eq!(
            xkb(conf),
            [
                ("XKB_DEFAULT_LAYOUT", "us,ru".to_owned()),
                ("XKB_DEFAULT_MODEL", "pc105".to_owned()),
                ("XKB_DEFAULT_VARIANT", ",".to_owned()),
                ("XKB_DEFAULT_OPTIONS", "grp:alt_shift_toggle".to_owned()),
            ]
        );
        assert!(xkb("Section \"InputClass\"\nEndSection\n").is_empty());
    }

    #[test]
    fn people_and_sessions_come_from_the_system_files() {
        let passwd = "root:x:0:0::/root:/bin/bash\nnobody:x:65534:65534::/:/usr/bin/nologin\npolina:x:1000:1000:Polina,,,:/home/polina:/bin/zsh\ngreeter:x:964:964::/var/lib/greetd:/usr/bin/nologin\ndanil:x:1001:1001::/home/danil:/bin/bash\n";
        let found = accounts(passwd, people(""));
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "Polina");
        assert_eq!(found[1].name, "danil");
        let defs = "# comment\nUID_MIN\t\t\t 500\nUID_MAX 60000\n";
        assert_eq!(people(defs), (500, 60000));
        let homed = "lena:x:60100:60100:Lena:/home/lena:/bin/bash\nlena:x:60100:60100:Lena:/home/lena:/bin/bash\n";
        assert_eq!(accounts(homed, people(defs)).len(), 1, "homed, once");

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
