use std::path::PathBuf;

use apolline_theme::{font, metrics};
use iced::widget::{container, stack};
use iced::{Alignment, Element, Fill, Font, Padding, Subscription, Task, Theme, window};

use crate::shell::{Message, Shell};
use crate::{fonts, shot};

pub fn run(
    mode: Option<String>,
    screenshot: Option<PathBuf>,
    open: Option<Message>,
) -> iced::Result {
    let app = iced::application(
        move || {
            let (mut shell, load) = Shell::new(mode.clone(), true);
            let mut start = open.clone();
            let load = match start.take_if(|_| screenshot.is_none()) {
                Some(message) => Task::batch([load, shell.update(message)]),
                None => load,
            };
            let preview = Preview {
                shell,
                screenshot: screenshot.clone(),
                start,
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
    start: Option<Message>,
    ticks: u32,
    shooting: bool,
}

impl Preview {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick if self.screenshot.is_some() => {
                self.ticks += 1;
                let ready = self.ticks >= 2 && self.shell.is_ready();
                if ready && let Some(message) = self.start.take() {
                    return self.shell.update(message);
                }
                if !self.shooting && ready {
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
        let bar = container(container(self.shell.bar(None)).padding(self.shell.bar_margin()))
            .height(Fill)
            .align_y(if self.shell.bar_at_bottom() {
                Alignment::End
            } else {
                Alignment::Start
            });
        let mut layers = stack![self.shell.wallpaper(), bar, popups];
        if let Some(osd) = self.shell.osd_view() {
            layers = layers.push(
                container(osd)
                    .width(Fill)
                    .height(Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::End)
                    .padding(Padding::new(0.0).bottom(56.0)),
            );
        }
        if self.shell.overlay().is_some() {
            layers = layers.push(self.shell.overlay_view());
        }
        layers.into()
    }

    fn subscription(&self) -> Subscription<Message> {
        self.shell.subscription()
    }
}
