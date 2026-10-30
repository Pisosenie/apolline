use apolline_theme::{Mode, Palette, neutral};
use iced::widget::operation::{focus, scroll_to};
use iced::widget::scrollable::AbsoluteOffset;
use iced::widget::{
    Column, button, column, container, image, mouse_area, opaque, row, scrollable, space, svg,
    text, text_input,
};
use iced::{Alignment, Element, Fill, Length, Task};

use crate::apps::{self, App, Picture};
use crate::icon::{Icon, icon};
use crate::panel::Power;
use crate::shell::{Message, Overlay};
use crate::style;

const WIDTH: f32 = 680.0;
const ROW: f32 = 54.0;
const VISIBLE: usize = 8;
const SEARCH: &str = "launcher-search";
const LIST: &str = "launcher-list";

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    App(String),
    Mode(Mode),
    Settings,
    Power(Power),
}

#[derive(Default)]
pub struct Launcher {
    pub query: String,
    pub selected: usize,
    apps: Vec<App>,
}

struct Entry<'a> {
    title: String,
    about: String,
    picture: Pic<'a>,
    target: Target,
    game: bool,
}

enum Pic<'a> {
    App(Option<&'a Picture>),
    Icon(Icon),
}

const ACTIONS: [(&str, &str, Icon, Target); 6] = [
    (
        "Work",
        "Switch to Work mode",
        Icon::Briefcase,
        Target::Mode(Mode::Work),
    ),
    (
        "Play",
        "Switch to Play mode · games, focus, boost",
        Icon::Gamepad,
        Target::Mode(Mode::Play),
    ),
    (
        "Evening",
        "Switch to Evening mode",
        Icon::Moon,
        Target::Mode(Mode::Evening),
    ),
    (
        "Settings",
        "Apolline settings",
        Icon::Settings,
        Target::Settings,
    ),
    (
        "Lock screen",
        "Lock",
        Icon::Lock,
        Target::Power(Power::Lock),
    ),
    (
        "Log out",
        "Leave niri",
        Icon::LogOut,
        Target::Power(Power::LogOut),
    ),
];

impl Launcher {
    pub fn open(&mut self) -> Task<Message> {
        self.query.clear();
        self.selected = 0;
        Task::batch([
            focus(SEARCH),
            scroll_to(LIST, AbsoluteOffset { x: 0.0, y: 0.0 }),
        ])
    }

    pub fn focus(&self) -> Task<Message> {
        focus(SEARCH)
    }

    pub fn set_apps(&mut self, apps: Vec<App>) {
        self.apps = apps;
    }

    pub fn search(&mut self, query: String) -> Task<Message> {
        self.query = query;
        self.selected = 0;
        scroll_to(LIST, AbsoluteOffset { x: 0.0, y: 0.0 })
    }

    pub fn step(&mut self, by: isize) -> Task<Message> {
        let count = self.entries().len();
        if count == 0 {
            return Task::none();
        }
        self.selected = (self.selected as isize + by).rem_euclid(count as isize) as usize;
        let top = self.selected.saturating_sub(VISIBLE / 2) as f32 * ROW;
        scroll_to(LIST, AbsoluteOffset { x: 0.0, y: top })
    }

    pub fn chosen(&self) -> Option<Target> {
        self.entries()
            .into_iter()
            .nth(self.selected)
            .map(|entry| entry.target)
    }

    fn entries(&self) -> Vec<Entry<'_>> {
        let query = self.query.trim();
        let mut ranked: Vec<(u32, Entry)> = Vec::new();
        let installed = self.apps.iter().any(|app| app.id == "apolline-settings");
        if !query.is_empty() {
            for (title, about, glyph, target) in ACTIONS {
                if installed && target == Target::Settings {
                    continue;
                }
                if let Some(score) = apps::rank(query, title, about) {
                    ranked.push((
                        score + 50,
                        Entry {
                            title: title.to_owned(),
                            about: about.to_owned(),
                            picture: Pic::Icon(glyph),
                            target,
                            game: false,
                        },
                    ));
                }
            }
        }
        for app in &self.apps {
            if let Some(score) = apps::score(query, app) {
                let bonus = if query.is_empty() && app.game { 1 } else { 0 };
                ranked.push((
                    score + bonus,
                    Entry {
                        title: app.name.clone(),
                        about: app.about.clone(),
                        picture: Pic::App(app.icon.as_ref()),
                        target: Target::App(app.exec.clone()),
                        game: app.game,
                    },
                ));
            }
        }
        ranked.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        ranked.into_iter().map(|(_, entry)| entry).collect()
    }

    pub fn view(&self, palette: Palette, mode: Mode) -> Element<'_, Message> {
        let entries = self.entries();
        let games = entries.iter().filter(|e| e.game).count();

        let search = row![
            icon(Icon::Search, 20.0, style::FAINT),
            text_input("Search apps, games and actions", &self.query)
                .id(SEARCH)
                .on_input(Message::Query)
                .on_submit(Message::Submit)
                .size(18)
                .padding(0)
                .style(style::bare_field(palette)),
            container(
                row![
                    icon(Icon::of(mode), 13.0, palette.accent),
                    text(mode.name())
                        .size(12)
                        .color(style::color(palette.accent)),
                ]
                .spacing(6)
                .align_y(Alignment::Center)
            )
            .padding([4, 10])
            .style(style::pill(palette.accent_soft)),
        ]
        .spacing(14)
        .align_y(Alignment::Center);

        let list: Element<'_, Message> = if entries.is_empty() {
            container(
                column![
                    icon(Icon::Search, 28.0, style::FAINT),
                    text(format!("Nothing called “{}”", self.query.trim()))
                        .size(14)
                        .color(style::color(style::SOFT_TEXT)),
                ]
                .spacing(10)
                .align_x(Alignment::Center),
            )
            .center_x(Fill)
            .padding([48, 0])
            .into()
        } else {
            let rows = Column::with_children(
                entries
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| self.row(index, entry, palette)),
            );
            scrollable(rows)
                .id(LIST)
                .height(Length::Fixed(ROW * entries.len().min(VISIBLE) as f32))
                .direction(style::thin())
                .style(style::scroller)
                .into()
        };

        let caption = if self.query.trim().is_empty() {
            match games {
                0 => "Apps".to_owned(),
                1 => "1 game · apps".to_owned(),
                n => format!("{n} games · apps"),
            }
        } else {
            match entries.len() {
                1 => "1 result".to_owned(),
                n => format!("{n} results"),
            }
        };

        let footer = row![
            text(caption).size(12).color(style::color(style::FAINT)),
            space::horizontal(),
            hint("↑↓", "move"),
            hint("↵", "open"),
            hint("esc", "close"),
        ]
        .spacing(14)
        .align_y(Alignment::Center);

        let panel = container(
            column![
                container(search).padding([18, 20]),
                container(space().height(1))
                    .width(Fill)
                    .style(|_| container::background(style::color(neutral::HAIRLINE))),
                container(list).padding([8, 8]),
                container(footer).padding([10, 18]),
            ]
            .width(WIDTH),
        )
        .style(style::surface);

        mouse_area(
            container(opaque(panel))
                .width(Fill)
                .height(Fill)
                .align_x(Alignment::Center)
                .padding(iced::Padding::new(0.0).top(160.0))
                .style(style::scrim),
        )
        .on_press(Message::Overlay(None::<Overlay>))
        .into()
    }

    fn row<'a>(&self, index: usize, entry: &Entry<'a>, palette: Palette) -> Element<'a, Message> {
        let selected = index == self.selected;
        let picture: Element<'a, Message> = match entry.picture {
            Pic::App(Some(Picture::Raster(path))) => image(image::Handle::from_path(path))
                .width(32)
                .height(32)
                .into(),
            Pic::App(Some(Picture::Vector(path))) => svg(svg::Handle::from_path(path))
                .width(32)
                .height(32)
                .into(),
            Pic::App(None) => letter(&entry.title, palette),
            Pic::Icon(glyph) => container(icon(glyph, 17.0, neutral::TEXT))
                .center(32)
                .style(move |_| {
                    let mut style = style::pill(style::FILL)(&iced::Theme::Dark);
                    style.border.radius = 9.0.into();
                    style
                })
                .into(),
        };
        let mut words =
            column![text(entry.title.clone()).size(15).font(style::medium())].spacing(1);
        if !entry.about.is_empty() {
            words = words.push(
                text(entry.about.clone())
                    .size(12)
                    .color(style::color(style::FAINT)),
            );
        }
        let mut line = row![picture, words, space::horizontal()]
            .spacing(14)
            .align_y(Alignment::Center);
        if entry.game {
            line = line.push(
                container(text("Game").size(11).color(style::color(palette.accent)))
                    .padding([3, 8])
                    .style(style::pill(palette.accent_soft)),
            );
        }
        if selected {
            line = line.push(icon(Icon::ChevronRight, 16.0, style::FAINT));
        }
        button(container(line).height(ROW - 4.0).align_y(Alignment::Center))
            .width(Fill)
            .padding([2, 12])
            .style(move |theme, status| {
                let mut style = style::ghost(12.0)(theme, status);
                if selected {
                    style.background = Some(iced::Background::Color(style::color(style::HOVER)));
                }
                style
            })
            .on_press(Message::Run(entry.target.clone()))
            .into()
    }
}

fn letter<'a>(name: &str, palette: Palette) -> Element<'a, Message> {
    let first = name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    container(
        text(first)
            .size(15)
            .font(style::semibold())
            .color(style::color(palette.accent)),
    )
    .center(32)
    .style(move |_| container::Style {
        background: Some(iced::Background::Color(style::color(palette.accent_soft))),
        border: iced::Border {
            radius: 9.0.into(),
            ..iced::Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

fn hint<'a>(key: &'a str, label: &'a str) -> Element<'a, Message> {
    row![
        container(text(key).size(11).font(style::mono()))
            .padding([1, 6])
            .style(|_| container::Style {
                border: iced::Border {
                    color: style::color(neutral::RAISED),
                    width: 1.0,
                    radius: 5.0.into(),
                },
                ..container::Style::default()
            }),
        text(label).size(12).color(style::color(style::FAINT)),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launcher() -> Launcher {
        let mut launcher = Launcher::default();
        launcher.set_apps(apps::sample());
        launcher
    }

    #[test]
    fn games_come_first_when_nothing_is_typed() {
        let launcher = launcher();
        let entries = launcher.entries();
        assert!(entries[..3].iter().all(|e| e.game));
        assert!(entries[3..].iter().all(|e| !e.game));
        assert!(entries.iter().all(|e| !matches!(e.target, Target::Mode(_))));
    }

    #[test]
    fn typing_finds_apps_and_actions() {
        let mut launcher = launcher();
        let _ = launcher.search("play".into());
        assert_eq!(launcher.chosen(), Some(Target::Mode(Mode::Play)));
        let _ = launcher.search("fire".into());
        assert_eq!(launcher.chosen(), Some(Target::App("firefox".into())));
        let _ = launcher.search("lock".into());
        assert_eq!(launcher.chosen(), Some(Target::Power(Power::Lock)));
    }

    #[test]
    fn installed_settings_replace_the_built_in_action() {
        let mut launcher = launcher();
        let mut apps = apps::sample();
        apps.push(App {
            id: "apolline-settings".into(),
            name: "Apolline Settings".into(),
            about: "Desktop settings".into(),
            exec: "apolline settings".into(),
            icon: None,
            game: false,
            keywords: String::new(),
        });
        launcher.set_apps(apps);
        let _ = launcher.search("sett".into());
        assert_eq!(launcher.entries().len(), 1);
        assert_eq!(
            launcher.chosen(),
            Some(Target::App("apolline settings".into()))
        );
    }

    #[test]
    fn arrows_wrap_around() {
        let mut launcher = launcher();
        let _ = launcher.search("code".into());
        let count = launcher.entries().len();
        assert!(count >= 2);
        let _ = launcher.step(-1);
        assert_eq!(launcher.selected, count - 1);
        let _ = launcher.step(1);
        assert_eq!(launcher.selected, 0);
    }
}
