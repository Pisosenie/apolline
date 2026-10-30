use std::path::PathBuf;

use apolline_theme::{Mode, font, metrics};
use iced::widget::{container, stack};
use iced::{Alignment, Element, Fill, Font, Padding, Subscription, Task, Theme, window};

use crate::shell::{Message, Overlay, Shell};
use crate::{fonts, shot};

pub fn run(mode: Option<Mode>, screenshot: Option<PathBuf>, open: Option<Overlay>) -> iced::Result {
    let app = iced::application(
        move || {
            let (mut shell, load) = Shell::new(mode, true);
            let load = Task::batch([load, shell.update(Message::Overlay(open))]);
            let preview = Preview {
                shell,
                screenshot: screenshot.clone(),
                ticks: 0,
                shooting: false,
            };
            (preview, load)
        },
        Preview::update,
        Preview::view,
    )
    .title("Apolline")
    .subscription(Preview::subscription)
    .theme(Theme::Dark)
    .default_font(Font::with_name(font::UI))
    .window_size((1440.0, 900.0));
    fonts::ALL
        .into_iter()
        .fold(app, |app, font| app.font(font))
        .run()
}

struct Preview {
    shell: Shell,
    screenshot: Option<PathBuf>,
    ticks: u32,
    shooting: bool,
}

impl Preview {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick if self.screenshot.is_some() => {
                self.ticks += 1;
                if !self.shooting && self.ticks >= 2 && self.shell.is_ready() {
                    self.shooting = true;
                    return window::latest()
                        .and_then(window::screenshot)
                        .map(Message::Captured);
                }
                self.shell.update(Message::Tick)
            }
            Message::Captured(shot) => {
                if let Some(path) = self.screenshot.take()
                    && let Err(err) = shot::save(&path, &shot)
                {
                    eprintln!("apolline: could not save {}: {err}", path.display());
                }
                iced::exit()
            }
            other => self.shell.update(other),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let popups = container(self.shell.popups())
            .width(Fill)
            .align_x(Alignment::End)
            .padding(Padding::new(12.0).top(metrics::BAR_INSET + metrics::BAR_HEIGHT + 8.0));
        let mut layers = stack![self.shell.wallpaper(), self.shell.bar(None), popups];
        if self.shell.overlay().is_some() {
            layers = layers.push(self.shell.overlay_view());
        }
        layers.into()
    }

    fn subscription(&self) -> Subscription<Message> {
        self.shell.subscription()
    }
}
