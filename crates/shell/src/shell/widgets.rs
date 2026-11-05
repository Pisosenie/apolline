use super::*;
use crate::icon::Icon;
use iced::widget::{Column, row};

const EVENTS: usize = 3;

impl Shell {
    // [desktop] widgets: lines under the desktop clock, in the order they're named.
    pub(super) fn desktop_widgets(&self, align: Alignment) -> Option<Element<'_, Message>> {
        let lines: Vec<Element<'_, Message>> = self
            .config
            .desktop
            .widgets
            .iter()
            .flat_map(|name| self.widget(name))
            .collect();
        // A soft shade behind them, so they read on a light picture too.
        (!lines.is_empty()).then(|| {
            container(Column::with_children(lines).spacing(6).align_x(align))
                .padding([10, 14])
                .style(|_| container::Style {
                    background: Some(iced::Background::Color(style::color(
                        apolline_theme::Scheme::DARK.window.alpha(0.32),
                    ))),
                    border: iced::Border::default().rounded(14),
                    ..container::Style::default()
                })
                .into()
        })
    }

    fn widget(&self, name: &str) -> Vec<Element<'_, Message>> {
        match name {
            "weather" => self
                .weather
                .iter()
                .map(|w| {
                    let (glyph, words) = crate::weather::describe(w.code);
                    let words = crate::i18n::find(words).unwrap_or(words);
                    line(Some(glyph), format!("{:.0}° {words} · {}", w.now, w.place))
                })
                .collect(),
            "agenda" => {
                let now = self.now.naive_local();
                let format = self.config.clock.time(false);
                self.agenda
                    .on(now.date())
                    .iter()
                    .filter(|o| o.end.or(o.start).is_none_or(|t| t > now.time()))
                    .take(EVENTS)
                    .map(|o| {
                        let words = match o.start {
                            Some(start) => format!("{}  {}", start.format(format), o.summary),
                            None => o.summary.to_string(),
                        };
                        line(Some(Icon::Calendar), words)
                    })
                    .collect()
            }
            "media" => self
                .track()
                .into_iter()
                .filter(|track| track.playing)
                .map(|track| {
                    let words = match &track.artist {
                        Some(artist) => format!("{} — {artist}", track.title),
                        None => track.title.clone(),
                    };
                    line(Some(Icon::Music), crate::media::shorten(&words, 60))
                })
                .collect(),
            module => self
                .config
                .modules
                .iter()
                .position(|m| m.name.as_deref() == Some(module))
                .and_then(|index| self.module_text.iter().find(|(i, _)| *i == index))
                .map(|(_, said)| line(None, said.text.clone()))
                .into_iter()
                .collect(),
        }
    }
}

fn line<'a>(glyph: Option<Icon>, words: String) -> Element<'a, Message> {
    let ink = apolline_theme::Scheme::DARK.text.alpha(0.85);
    let words = style::verbatim(words)
        .size(15)
        .font(style::medium())
        .color(style::color(ink));
    match glyph {
        Some(glyph) => row![crate::icon::icon(glyph, 16.0, ink), words]
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
        None => words.into(),
    }
}
