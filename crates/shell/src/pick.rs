use std::io::{BufRead, IsTerminal};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use apolline_theme::{Palette, font, neutral};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::operation::{focus, scroll_to};
use iced::widget::scrollable::AbsoluteOffset;
use iced::widget::{
    Column, button, column, container, mouse_area, opaque, row, scrollable, space, text, text_input,
};
use iced::{Alignment, Element, Fill, Font, Padding, Subscription, Task, Theme};

use crate::icon::{Icon, icon};
use crate::{apps, fonts, style};

const FIELD: &str = "pick-search";
const LIST: &str = "pick-list";
const ROW: f32 = 40.0;

#[cfg_attr(target_os = "linux", iced_layershell::to_layer_message)]
#[derive(Debug, Clone)]
pub enum Message {
    Query(String),
    Key(Key),
    Choose(Option<usize>),
    Opened,
}

struct Pick {
    items: Vec<String>,
    prompt: String,
    query: String,
    selected: usize,
    chosen: Arc<Mutex<Option<String>>>,
    palette: Palette,
}

pub fn main(args: &[String]) -> ExitCode {
    let prompt = match args {
        [flag, prompt, ..] if flag == "--prompt" => prompt.clone(),
        _ => "Pick one".to_owned(),
    };
    if std::io::stdin().is_terminal() {
        eprintln!("apolline pick: send it lines on stdin, like  ls | apolline pick");
        return ExitCode::from(2);
    }
    let items: Vec<String> = std::io::stdin()
        .lock()
        .lines()
        .map_while(Result::ok)
        .filter(|line| !line.trim().is_empty())
        .collect();
    let chosen = Arc::new(Mutex::new(None));
    let palette = apolline_config::Config::load(&apolline_config::default_path())
        .0
        .palette(None);
    let result = run(Pick {
        items,
        prompt,
        query: String::new(),
        selected: 0,
        chosen: chosen.clone(),
        palette,
    });
    if let Err(err) = result {
        eprintln!("apolline pick: {err}");
        return ExitCode::FAILURE;
    }
    match chosen.lock().ok().and_then(|mut c| c.take()) {
        Some(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        None => ExitCode::from(1),
    }
}

#[cfg(target_os = "linux")]
fn run(pick: Pick) -> Result<(), String> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};

    let pick = Mutex::new(Some(pick));
    let app = iced_layershell::application(
        move || {
            let pick = pick
                .lock()
                .ok()
                .and_then(|mut p| p.take())
                .expect("booted once");
            (pick, Task::none())
        },
        "apolline-pick",
        Pick::update,
        Pick::view,
    )
    .subscription(Pick::subscription)
    .theme(|_: &Pick| Theme::Dark)
    .style(|_, theme| iced::theme::Style {
        background_color: iced::Color::TRANSPARENT,
        text_color: theme.palette().text,
    })
    .settings(Settings {
        default_font: Font::with_name(font::UI),
        layer_settings: LayerShellSettings {
            anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
            layer: Layer::Overlay,
            exclusive_zone: -1,
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            start_mode: StartMode::Active,
            ..Default::default()
        },
        ..Default::default()
    });
    fonts::ALL
        .into_iter()
        .fold(app, |app, font| app.font(font))
        .run()
        .map_err(|err| err.to_string())
}

#[cfg(not(target_os = "linux"))]
fn run(pick: Pick) -> Result<(), String> {
    let pick = Mutex::new(Some(pick));
    let app = iced::application(
        move || {
            let pick = pick
                .lock()
                .ok()
                .and_then(|mut p| p.take())
                .expect("booted once");
            (pick, focus(FIELD))
        },
        Pick::update,
        Pick::view,
    )
    .title("Apolline")
    .subscription(Pick::subscription)
    .theme(Theme::Dark)
    .default_font(Font::with_name(font::UI))
    .window_size((900.0, 640.0));
    fonts::ALL
        .into_iter()
        .fold(app, |app, font| app.font(font))
        .run()
        .map_err(|err| err.to_string())
}

impl Pick {
    fn visible(&self) -> Vec<usize> {
        let query = self.query.trim();
        if query.is_empty() {
            return (0..self.items.len()).collect();
        }
        let mut ranked: Vec<(u32, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| apps::rank(query, item, "").map(|score| (score, index)))
            .collect();
        ranked.sort_by_key(|(score, index)| (std::cmp::Reverse(*score), *index));
        ranked.into_iter().map(|(_, index)| index).collect()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Query(query) => {
                self.query = query;
                self.selected = 0;
                return scroll_to(LIST, AbsoluteOffset { x: 0.0, y: 0.0 });
            }
            Message::Key(Key::Named(Named::Escape)) => return iced::exit(),
            Message::Key(Key::Named(step @ (Named::ArrowDown | Named::ArrowUp))) => {
                let count = self.visible().len();
                if count > 0 {
                    let by = if step == Named::ArrowDown { 1 } else { -1 };
                    self.selected =
                        (self.selected as isize + by).rem_euclid(count as isize) as usize;
                    let top = self.selected.saturating_sub(5) as f32 * ROW;
                    return scroll_to(LIST, AbsoluteOffset { x: 0.0, y: top });
                }
            }
            Message::Key(_) => {}
            Message::Choose(at) => {
                let visible = self.visible();
                let picked = match at {
                    Some(index) => Some(index),
                    None => visible.get(self.selected).copied(),
                };
                let line = match picked {
                    Some(index) => self.items.get(index).cloned(),
                    None => (!self.query.trim().is_empty()).then(|| self.query.clone()),
                };
                if let (Some(line), Ok(mut chosen)) = (line, self.chosen.lock()) {
                    *chosen = Some(line);
                }
                return iced::exit();
            }
            Message::Opened => return focus(FIELD),
            #[allow(unreachable_patterns)]
            _ => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed { key, .. } => Some(Message::Key(key)),
                _ => None,
            }),
            iced::window::open_events().map(|_| Message::Opened),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let palette = self.palette;
        let visible = self.visible();
        let header = row![
            icon(Icon::Search, 18.0, style::FAINT),
            text_input(&self.prompt, &self.query)
                .id(FIELD)
                .on_input(Message::Query)
                .on_submit(Message::Choose(None))
                .size(17)
                .padding(0)
                .style(style::bare_field(palette)),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        let rows = Column::with_children(visible.iter().enumerate().map(|(at, &index)| {
            let on = at == self.selected;
            button(text(self.items[index].as_str()).size(14))
                .width(Fill)
                .height(ROW)
                .padding([10, 14])
                .style(move |theme, status| {
                    let mut style = style::ghost(10.0)(theme, status);
                    if on {
                        style.background = Some(iced::Background::Color(style::color(style::FILL)));
                    }
                    style
                })
                .on_press(Message::Choose(Some(index)))
                .into()
        }));
        let list = scrollable(rows)
            .id(LIST)
            .direction(style::thin())
            .style(style::scroller)
            .height(iced::Length::Shrink);
        let count = text(format!("{} of {}", visible.len(), self.items.len()))
            .size(12)
            .color(style::color(style::FAINT));
        let panel = container(
            column![
                container(header).padding([16, 18]),
                container(space())
                    .width(Fill)
                    .height(1)
                    .style(|_| { container::background(style::color(neutral::HAIRLINE)) }),
                container(container(list).max_height(ROW * 9.0)).padding(8),
                container(count).padding(Padding::new(0.0).left(18).bottom(12)),
            ]
            .width(620),
        )
        .style(style::surface);
        mouse_area(
            container(opaque(panel))
                .width(Fill)
                .height(Fill)
                .align_x(Alignment::Center)
                .padding(Padding::new(0.0).top(150.0))
                .style(style::scrim),
        )
        .on_press(Message::Key(Key::Named(Named::Escape)))
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_narrows_and_enter_takes_the_top_line_or_what_was_typed() {
        let chosen = Arc::new(Mutex::new(None));
        let mut pick = Pick {
            items: vec!["shutdown".into(), "reboot".into(), "suspend".into()],
            prompt: String::new(),
            query: String::new(),
            selected: 0,
            chosen: chosen.clone(),
            palette: Palette::default(),
        };
        let _ = pick.update(Message::Query("re".into()));
        assert_eq!(pick.visible(), [1]);
        let _ = pick.update(Message::Choose(None));
        assert_eq!(chosen.lock().unwrap().as_deref(), Some("reboot"));

        let _ = pick.update(Message::Query("hibernate".into()));
        let _ = pick.update(Message::Choose(None));
        assert_eq!(chosen.lock().unwrap().as_deref(), Some("hibernate"));
    }
}
