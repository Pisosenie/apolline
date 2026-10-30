use std::path::PathBuf;
use std::time::Duration;

use apolline_config::Config;
use apolline_theme::{Palette, Rgba, font, neutral};
use chrono::{DateTime, Local};
use iced::gradient::Linear;
use iced::keyboard::{self, Key, key::Named};
use iced::widget::operation::focus;
use iced::widget::{Space, button, column, container, image, row, space, stack, text, text_input};
use iced::{
    Alignment, Background, Border, ContentFit, Element, Fill, Font, Gradient, Radians,
    Subscription, Task, Theme, window,
};

use crate::control::{self, Shared};
use crate::icon::{Icon, icon};
use crate::idle::{self, Stage};
use crate::media::{self, Action, Track};
use crate::power::Battery;
use crate::shell;
use crate::status::short_layout;
use crate::{controls, fonts, niri, power, shot, style, wallpaper};

const FIELD: &str = "lock-password";
const DARK: Rgba = Rgba::hex(0x040508);

pub fn run(preview: bool, mode: Option<String>, screenshot: Option<PathBuf>) -> Result<(), String> {
    if preview || screenshot.is_some() {
        let app = iced::application(
            move || Lock::new(true, mode.clone(), screenshot.clone()),
            Lock::update,
            Lock::view_window,
        )
        .title("Apolline Lock")
        .subscription(Lock::subscription)
        .theme(Theme::Dark)
        .default_font(Font::with_name(font::UI))
        .window_size((1440.0, 900.0));
        return fonts::ALL
            .into_iter()
            .fold(app, |app, font| app.font(font))
            .run()
            .map_err(|err| err.to_string());
    }
    session(mode)
}

#[cfg(target_os = "linux")]
fn session(mode: Option<String>) -> Result<(), String> {
    let guard = std::fs::File::create(control::runtime_dir().join("apolline-lock.lock"))
        .map_err(|err| err.to_string())?;
    if guard.try_lock().is_err() {
        return Ok(());
    }
    let app = iced_sessionlock::application(
        move || Lock::new(false, mode.clone(), None),
        Lock::update,
        Lock::view,
    )
    .subscription(Lock::subscription)
    .theme(|_: &Lock| Theme::Dark)
    .default_font(Font::with_name(font::UI));
    fonts::ALL
        .into_iter()
        .fold(app, |app, font| app.font(font))
        .run()
        .map_err(|err| err.to_string())
}

#[cfg(not(target_os = "linux"))]
fn session(_: Option<String>) -> Result<(), String> {
    Err("locking needs a Wayland session on Linux; try --preview".into())
}

#[cfg_attr(target_os = "linux", iced_sessionlock::to_session_message)]
#[derive(Clone, Debug)]
pub enum Message {
    Tick,
    Opened,
    Key(keyboard::Event, bool),
    Password(String),
    Submit,
    Checked(Result<(), String>),
    Wallpaper(Result<image::Handle, String>),
    Battery(Option<Battery>),
    Media(Option<Track>),
    Player(Action),
    Niri(apolline_niri::Event),
    Network(Option<String>),
    Idle(Stage, bool),
    Shot(window::Screenshot),
    Noop,
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Typing,
    Checking,
    Wrong(String),
}

struct User {
    login: String,
    name: String,
    face: Option<image::Handle>,
}

struct Lock {
    config: Config,
    mode: Option<usize>,
    user: User,
    password: String,
    state: State,
    wallpaper: Option<image::Handle>,
    battery: Option<Battery>,
    track: Option<Track>,
    niri: apolline_niri::State,
    network: Option<String>,
    waiting: usize,
    now: DateTime<Local>,
    ticks: u64,
    preview: bool,
    screenshot: Option<PathBuf>,
    shooting: bool,
}

impl Lock {
    fn new(
        preview: bool,
        mode: Option<String>,
        screenshot: Option<PathBuf>,
    ) -> (Self, Task<Message>) {
        let (config, _) = Config::load(&apolline_config::default_path());
        let shared = control::shared();
        let mode = mode
            .or(shared.mode)
            .and_then(|key| config.find(&key))
            .or_else(|| config.start());
        let sample = preview && !niri::connected();
        let lock = Self {
            mode,
            user: user(),
            password: String::new(),
            state: State::Typing,
            wallpaper: None,
            battery: sample.then_some(Battery {
                percent: 82,
                charging: false,
            }),
            track: sample.then(|| Track::sample("Night Drive", "Low Tide")),
            niri: apolline_niri::State::default(),
            network: sample.then(|| "Polina".to_owned()),
            waiting: if sample { 2 } else { shared.waiting },
            now: Local::now(),
            ticks: 0,
            preview,
            screenshot,
            shooting: false,
            config,
        };
        let name = lock.config.wallpaper(mode).to_owned();
        let load = Task::perform(wallpaper::load(name), Message::Wallpaper);
        let network = if sample { Task::none() } else { read_network() };
        (lock, Task::batch([load, network, focus(FIELD)]))
    }

    fn palette(&self) -> Palette {
        self.config.palette(self.mode)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.now = Local::now();
                self.ticks += 1;
                if !self.preview {
                    let Shared { waiting, .. } = control::shared();
                    self.waiting = waiting;
                }
                if self.screenshot.is_some() && !self.shooting && self.wallpaper.is_some() {
                    self.shooting = true;
                    return window::latest()
                        .and_then(window::screenshot)
                        .map(Message::Shot);
                }
                if self.ticks.is_multiple_of(15) && !self.preview {
                    return read_network();
                }
            }
            Message::Opened => return focus(FIELD),
            Message::Key(keyboard::Event::KeyPressed { key, text, .. }, captured) => {
                if self.state == State::Checking {
                    return Task::none();
                }
                match key.as_ref() {
                    Key::Named(Named::Escape) => {
                        self.password.clear();
                        self.state = State::Typing;
                    }
                    _ if captured => {}
                    Key::Named(Named::Enter) => return self.update(Message::Submit),
                    Key::Named(Named::Backspace) => {
                        self.password.pop();
                    }
                    _ => {
                        if let Some(typed) = text.filter(|t| t.chars().all(|c| !c.is_control())) {
                            self.password.push_str(&typed);
                            self.state = State::Typing;
                        }
                    }
                }
            }
            Message::Password(password) => {
                self.password = password;
                self.state = State::Typing;
            }
            Message::Submit => {
                if self.password.is_empty() || self.state == State::Checking {
                    return Task::none();
                }
                if self.preview {
                    self.state = State::Wrong("This is a preview, nothing to unlock".into());
                    self.password.clear();
                    return Task::none();
                }
                self.state = State::Checking;
                let login = self.user.login.clone();
                let password = std::mem::take(&mut self.password);
                return Task::perform(check(login, password), Message::Checked);
            }
            Message::Checked(Ok(())) => return unlock(),
            Message::Checked(Err(err)) => {
                self.state = State::Wrong(err);
                return focus(FIELD);
            }
            Message::Wallpaper(Ok(handle)) => self.wallpaper = Some(handle),
            Message::Wallpaper(Err(err)) => eprintln!("apolline: wallpaper: {err}"),
            Message::Battery(battery) => self.battery = battery,
            Message::Media(track) => self.track = track,
            Message::Player(action) => {
                if let Some(track) = &mut self.track {
                    if track.player.is_empty() {
                        track.playing = !track.playing;
                    } else {
                        return Task::future(media::control(track.player.clone(), action))
                            .discard();
                    }
                }
            }
            Message::Niri(event) => self.niri.apply(event),
            Message::Network(network) => self.network = network,
            Message::Idle(Stage::ScreenOff, true) if !self.preview => {
                return Task::future(async {
                    let _ = tokio::process::Command::new("niri")
                        .args(["msg", "action", "power-off-monitors"])
                        .status()
                        .await;
                })
                .discard();
            }
            Message::Shot(capture) => {
                if let Some(path) = self.screenshot.take()
                    && let Err(err) = shot::save(&path, &capture)
                {
                    eprintln!("apolline: could not save {}: {err}", path.display());
                }
                return iced::exit();
            }
            _ => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
            window::open_events().map(|_| Message::Opened),
            iced::event::listen_with(|event, status, _| match event {
                iced::Event::Keyboard(key @ keyboard::Event::KeyPressed { .. }) => {
                    Some(Message::Key(key, status == iced::event::Status::Captured))
                }
                _ => None,
            }),
            power::subscription().map(|message| match message {
                shell::Message::Battery(battery) => Message::Battery(battery),
                _ => Message::Noop,
            }),
            media::subscription().map(|message| match message {
                shell::Message::Media(track) => Message::Media(track),
                _ => Message::Noop,
            }),
            niri::events().map(|message| match message {
                shell::Message::Niri(event) => Message::Niri(event),
                _ => Message::Noop,
            }),
            idle::subscription(if self.preview {
                Vec::new()
            } else {
                vec![(Stage::ScreenOff, 30)]
            })
            .map(|(stage, idle)| Message::Idle(stage, idle)),
        ])
    }

    fn view_window(&self) -> Element<'_, Message> {
        self.view(window::Id::unique())
    }

    fn view(&self, _: window::Id) -> Element<'_, Message> {
        let palette = self.palette();
        let picture: Element<'_, Message> = match &self.wallpaper {
            Some(handle) => image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Cover)
                .into(),
            None => Space::new().width(Fill).height(Fill).into(),
        };
        let shade = container(Space::new().width(Fill).height(Fill)).style(|_| {
            let dark = |alpha: f32| style::color(DARK.alpha(alpha));
            container::Style {
                background: Some(Background::Gradient(Gradient::Linear(
                    Linear::new(Radians(std::f32::consts::PI))
                        .add_stop(0.0, dark(0.35))
                        .add_stop(0.22, dark(0.0))
                        .add_stop(0.42, dark(0.0))
                        .add_stop(1.0, dark(0.7)),
                ))),
                ..container::Style::default()
            }
        });

        let top = row![self.mode_pill(palette), space::horizontal(), self.status()]
            .align_y(Alignment::Center)
            .padding([32, 40]);

        let clock = column![
            text(self.now.format("%A, %-d %B").to_string().to_uppercase())
                .size(14)
                .font(style::medium())
                .color(style::color(neutral::TEXT.alpha(0.86))),
            text(self.now.format("%H:%M").to_string())
                .size(232)
                .font(style::light())
                .line_height(0.92),
        ]
        .spacing(4);

        let left = column![clock, space().height(36), self.field(palette), self.hint()]
            .spacing(12)
            .max_width(560);

        let mut right = column![].spacing(14).align_x(Alignment::End);
        if self.config.lock.show_media
            && let Some(track) = &self.track
        {
            right = right.push(player(track, palette));
        }
        if self.waiting > 0 {
            let line = match self.waiting {
                1 => "1 notification · hidden until you unlock".to_owned(),
                n => format!("{n} notifications · hidden until you unlock"),
            };
            right = right.push(
                row![
                    icon(Icon::Lock, 12.0, neutral::TEXT.alpha(0.8)),
                    text(line)
                        .size(12)
                        .color(style::color(neutral::TEXT.alpha(0.8))),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );
        }

        let bottom = row![left, space::horizontal(), right]
            .align_y(Alignment::End)
            .padding(iced::Padding::new(88.0).top(0.0));

        container(stack![
            picture,
            shade,
            column![top, space::vertical(), bottom]
                .width(Fill)
                .height(Fill),
        ])
        .style(|_| container::background(style::color(neutral::INK)))
        .into()
    }

    fn mode_pill(&self, palette: Palette) -> Option<Element<'_, Message>> {
        let mode = self.mode.and_then(|i| self.config.modes.get(i))?;
        let pill = container(
            row![
                container(Space::new().width(6).height(6)).style(style::dot(palette.accent)),
                text(mode.name.as_str()).size(12).font(style::medium()),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([7, 12])
        .style(glass(0.45, 0.1, style::PILL));
        Some(pill.into())
    }

    fn status(&self) -> Element<'_, Message> {
        let tint = style::color(neutral::TEXT.alpha(0.86));
        let mut items = row![].spacing(18).align_y(Alignment::Center);
        if let Some(layout) = self.layout() {
            items = items.push(text(layout).size(12).font(style::mono()).color(tint));
        }
        if let Some(network) = &self.network {
            items = items.push(
                row![
                    icon(Icon::Wifi, 13.0, neutral::TEXT.alpha(0.86)),
                    text(network.as_str()).size(12).color(tint)
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            );
        }
        if let Some(battery) = self.battery {
            items = items.push(
                row![
                    icon(
                        Icon::battery(battery.percent, battery.charging),
                        15.0,
                        neutral::TEXT.alpha(0.86)
                    ),
                    text(format!("{}%", battery.percent))
                        .size(12)
                        .font(style::mono())
                        .color(tint),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            );
        }
        items.into()
    }

    fn layout(&self) -> Option<String> {
        match self.niri.layout() {
            Some(name) => Some(short_layout(name)),
            None if self.preview => Some("EN".to_owned()),
            None => None,
        }
    }

    fn field(&self, palette: Palette) -> Element<'_, Message> {
        let avatar: Element<'_, Message> = match &self.user.face {
            Some(face) => image(face.clone())
                .width(36)
                .height(36)
                .content_fit(ContentFit::Cover)
                .border_radius(18.0)
                .into(),
            None => container(
                text(initial(&self.user.name))
                    .size(14)
                    .font(style::semibold())
                    .color(style::color(palette.on_accent)),
            )
            .center(36)
            .style(style::pill(palette.accent))
            .into(),
        };
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
        let mut line = row![
            avatar,
            text(self.user.name.clone()).size(14).font(style::medium()),
            input,
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        if let Some(layout) = self.layout() {
            line = line.push(
                text(layout)
                    .size(11)
                    .font(style::mono())
                    .color(style::color(style::FAINT)),
            );
        }
        line = line.push(
            button(container(icon(Icon::Arrow, 16.0, neutral::TEXT)).center(36))
                .padding(0)
                .style(|_: &Theme, status| button::Style {
                    background: Some(Background::Color(style::color(Rgba::hex(0xffffff).alpha(
                        match status {
                            button::Status::Hovered | button::Status::Pressed => 0.22,
                            _ => 0.14,
                        },
                    )))),
                    border: Border {
                        radius: style::PILL.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                })
                .on_press_maybe((!checking).then_some(Message::Submit)),
        );
        let edge = match self.state {
            State::Wrong(_) => style::DANGER.alpha(0.8),
            _ => Rgba::hex(0xffffff).alpha(0.12),
        };
        container(line)
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

    fn hint(&self) -> Element<'_, Message> {
        match &self.state {
            State::Wrong(reason) => text(reason.as_str())
                .size(13)
                .color(style::color(style::DANGER))
                .into(),
            _ => Space::new().height(16).into(),
        }
    }
}

fn player(track: &Track, palette: Palette) -> Element<'_, Message> {
    let mut words = column![
        text(media::shorten(&track.title, 28))
            .size(15)
            .font(style::medium())
    ]
    .spacing(3)
    .width(Fill);
    if let Some(artist) = &track.artist {
        words = words.push(
            text(media::shorten(artist, 32))
                .size(13)
                .color(style::color(neutral::MUTED)),
        );
    }
    container(
        row![
            container(icon(Icon::Music, 24.0, palette.accent))
                .center(64)
                .style(move |_| container::Style {
                    background: Some(Background::Color(style::color(palette.accent_soft))),
                    border: Border {
                        radius: 10.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
            words,
            button(
                container(icon(
                    if track.playing {
                        Icon::Pause
                    } else {
                        Icon::Play
                    },
                    14.0,
                    neutral::TEXT
                ))
                .center(36)
            )
            .padding(0)
            .style(|_: &Theme, _| button::Style {
                background: Some(Background::Color(style::color(
                    Rgba::hex(0xffffff).alpha(0.12)
                ))),
                border: Border {
                    radius: style::PILL.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            })
            .on_press(Message::Player(Action::Toggle)),
        ]
        .spacing(14)
        .align_y(Alignment::Center),
    )
    .width(340)
    .padding(14)
    .style(glass(0.5, 0.1, 18.0))
    .into()
}

fn glass(alpha: f32, edge: f32, radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        text_color: Some(style::color(neutral::TEXT)),
        background: Some(Background::Color(style::color(
            Rgba::hex(0x0c0d10).alpha(alpha),
        ))),
        border: Border {
            color: style::color(Rgba::hex(0xffffff).alpha(edge)),
            width: 1.0,
            radius: radius.into(),
        },
        ..container::Style::default()
    }
}

fn initial(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default()
}

fn read_network() -> Task<Message> {
    Task::perform(controls::read(), |controls| {
        Message::Network(controls.wifi.filter(|w| w.on).and_then(|w| w.connected))
    })
}

#[cfg(target_os = "linux")]
fn unlock() -> Task<Message> {
    Task::done(Message::UnLock)
}

#[cfg(not(target_os = "linux"))]
fn unlock() -> Task<Message> {
    iced::exit()
}

fn user() -> User {
    let uid = std::fs::metadata("/proc/self")
        .ok()
        .map(|meta| std::os::unix::fs::MetadataExt::uid(&meta));
    let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
    let entry = uid.and_then(|uid| account(&passwd, uid));
    let login = entry
        .as_ref()
        .map(|(login, _)| login.clone())
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_default();
    let name = entry
        .and_then(|(_, name)| name)
        .unwrap_or_else(|| login.clone());
    let face = [
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".face")),
        Some(PathBuf::from("/var/lib/AccountsService/icons").join(&login)),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.is_file())
    .map(image::Handle::from_path);
    User { login, name, face }
}

fn account(passwd: &str, uid: u32) -> Option<(String, Option<String>)> {
    passwd.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() < 5 || fields[2].parse::<u32>().ok()? != uid {
            return None;
        }
        let name = fields[4]
            .split(',')
            .next()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned);
        Some((fields[0].to_owned(), name))
    })
}

async fn check(login: String, password: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || authenticate(&login, &password))
        .await
        .map_err(|err| err.to_string())?
}

#[cfg(target_os = "linux")]
struct Conversation {
    login: String,
    password: String,
    notes: Vec<String>,
}

#[cfg(target_os = "linux")]
impl pam::Conversation for Conversation {
    fn prompt_echo(&mut self, _: &std::ffi::CStr) -> Result<std::ffi::CString, ()> {
        std::ffi::CString::new(self.login.clone()).map_err(drop)
    }

    fn prompt_blind(&mut self, _: &std::ffi::CStr) -> Result<std::ffi::CString, ()> {
        std::ffi::CString::new(self.password.clone()).map_err(drop)
    }

    fn info(&mut self, message: &std::ffi::CStr) {
        self.notes.push(message.to_string_lossy().into_owned());
    }

    fn error(&mut self, message: &std::ffi::CStr) {
        self.notes.push(message.to_string_lossy().into_owned());
    }
}

#[cfg(target_os = "linux")]
fn authenticate(login: &str, password: &str) -> Result<(), String> {
    use pam::PamReturnCode as Code;

    let service = std::env::var("APOLLINE_PAM_SERVICE").unwrap_or_else(|_| {
        if std::path::Path::new("/etc/pam.d/apolline").exists() {
            "apolline".to_owned()
        } else {
            "login".to_owned()
        }
    });
    let conversation = Conversation {
        login: login.to_owned(),
        password: password.to_owned(),
        notes: Vec::new(),
    };
    let mut client =
        pam::Client::with_conversation(&service, conversation).map_err(|err| err.to_string())?;
    let Err(err) = client.authenticate() else {
        return Ok(());
    };
    if let Some(note) = client.conversation().notes.last() {
        return Err(note.clone());
    }
    Err(match err.0 {
        Code::Auth_Err | Code::Cred_Insufficient | Code::AuthTok_Err => "Wrong password".into(),
        Code::MaxTries => "Too many tries, wait a little".into(),
        Code::Acct_Expired | Code::AuthTok_Expired | Code::New_Authtok_Reqd => {
            "Your password has expired".into()
        }
        other => format!("Can't unlock: {other:?}"),
    })
}

#[cfg(not(target_os = "linux"))]
fn authenticate(_: &str, _: &str) -> Result<(), String> {
    Err("Unlocking needs Linux".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_account_comes_from_passwd() {
        let passwd = "root:x:0:0::/root:/bin/bash\ndanil:x:1000:1000:Danil Zorin,,,:/home/danil:/bin/zsh\nbot:x:1001:1001::/home/bot:/bin/sh\n";
        assert_eq!(
            account(passwd, 1000),
            Some(("danil".into(), Some("Danil Zorin".into())))
        );
        assert_eq!(account(passwd, 1001), Some(("bot".into(), None)));
        assert_eq!(account(passwd, 42), None);
        assert_eq!(initial("danil"), "D");
    }

    #[test]
    fn a_wrong_password_clears_the_field_and_says_so() {
        let (mut lock, _) = Lock::new(true, Some("evening".into()), None);
        let _ = lock.update(Message::Password("hunter2".into()));
        let _ = lock.update(Message::Checked(Err("Wrong password".into())));
        assert_eq!(lock.state, State::Wrong("Wrong password".into()));
        let _ = lock.update(Message::Password("h".into()));
        assert_eq!(lock.state, State::Typing);
        assert_eq!(lock.mode, lock.config.find("evening"));
    }
}
