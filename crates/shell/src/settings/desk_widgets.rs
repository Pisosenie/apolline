use super::*;
use iced::widget::column;

use apolline_config::Desktop;

fn widget_label(name: &str) -> String {
    match name {
        "weather" => "Weather",
        "agenda" => "Today's events",
        "media" => "What's playing",
        name => return name.to_owned(),
    }
    .to_owned()
}

impl Settings {
    // The desktop clock's row, with what goes under it (or in its place, with no clock).
    pub(super) fn with_widgets<'a>(&'a self, clock: Element<'a, Message>) -> Element<'a, Message> {
        let shown = &self.config.desktop.widgets;
        let names = Desktop::WIDGETS
            .iter()
            .map(|name| (*name).to_owned())
            .chain(self.config.modules.iter().filter_map(|m| m.name.clone()));
        let chips = Row::with_children(names.map(|name| {
            let on = shown.contains(&name);
            button(text(widget_label(&name)).size(12))
                .padding([6, 12])
                .style(style::choice(on))
                .on_press(Message::DesktopWidget(name, !on))
                .into()
        }))
        .spacing(6)
        .wrap();
        column![clock, labelled("On the desktop, under the clock", chips)]
            .spacing(12)
            .into()
    }

    pub(super) fn set_desktop_widget(&mut self, name: String, on: bool) {
        let mut widgets: Vec<String> = self
            .config
            .desktop
            .widgets
            .iter()
            .filter(|w| **w != name)
            .cloned()
            .collect();
        if on {
            widgets.push(name);
        }
        if widgets.is_empty() {
            self.edit(|e| e.unset("desktop", "widgets"));
        } else {
            self.edit(|e| e.set_list("desktop", "widgets", &widgets));
        }
    }
}
