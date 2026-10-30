use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use apolline_theme::{Mode, font};
use iced::widget::stack;
use iced::{Element, Font, Subscription, Task, Theme, window};

use crate::shell::{self, Message, Shell};

pub fn run(mode: Option<Mode>, screenshot: Option<PathBuf>) -> iced::Result {
    iced::application(
        move || Preview {
            shell: Shell::new(mode),
            screenshot: screenshot.clone(),
            ticks: 0,
        },
        Preview::update,
        Preview::view,
    )
    .title("Apolline")
    .subscription(Preview::subscription)
    .theme(Theme::Dark)
    .default_font(Font::with_name(font::UI))
    .window_size((1440.0, 900.0))
    .run()
}

struct Preview {
    shell: Shell,
    screenshot: Option<PathBuf>,
    ticks: u32,
}

impl Preview {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick if self.screenshot.is_some() => {
                self.ticks += 1;
                if self.ticks == 2 {
                    return window::latest()
                        .and_then(window::screenshot)
                        .map(Message::Captured);
                }
                self.shell.update(Message::Tick)
            }
            Message::Captured(shot) => {
                if let Some(path) = self.screenshot.take()
                    && let Err(err) = save_png(&path, &shot)
                {
                    eprintln!("apolline: could not save {}: {err}", path.display());
                }
                iced::exit()
            }
            other => self.shell.update(other),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        stack![self.shell.wallpaper(), self.shell.bar()].into()
    }

    fn subscription(&self) -> Subscription<Message> {
        shell::ticks()
    }
}

fn save_png(path: &Path, shot: &window::Screenshot) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(
        BufWriter::new(File::create(path)?),
        shot.size.width,
        shot.size.height,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&shot.rgba)?;
    Ok(())
}
