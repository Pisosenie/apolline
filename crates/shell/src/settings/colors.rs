use super::*;

// Each role a color of your own can take, said the way people see it.
const ROLES: [(&str, &str, &str); 5] = [
    (
        "ink",
        "Bar and dimming",
        "The bar's glass and what darkens behind overlays",
    ),
    ("panel", "Notification cards", ""),
    (
        "surface",
        "Quick settings and the launcher",
        "And the other overlays",
    ),
    (
        "window",
        "Apolline's windows",
        "This one, the lock screen's fields, dialogs",
    ),
    ("text", "Text", "Dark text makes the whole palette light"),
];

impl Settings {
    // One line until it's opened, or until a color of your own is set.
    pub(super) fn own_colors(&self) -> Element<'_, Message> {
        let set = ROLES
            .iter()
            .any(|(role, ..)| self.config.colors.get(role).is_some());
        if !self.colors_open && !set {
            return group(
                "Your colors",
                vec![setting(
                    "Colors of your own",
                    "Over any palette, the bar, cards, overlays and text a role at a time",
                    button(text("Choose").size(13))
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press(Message::OwnColors),
                )],
            );
        }
        let palette = self.palette();
        let scheme = self.config.scheme(None);
        let shown = |role: &str| match role {
            "ink" => scheme.ink,
            "panel" => scheme.panel,
            "surface" => scheme.surface,
            "window" => scheme.window,
            _ => scheme.text,
        };
        let rows = ROLES
            .iter()
            .map(|&(role, title, about)| {
                let own = self.config.colors.get(role);
                let color = shown(role).alpha(1.0);
                let swatch =
                    container(Space::new().width(22).height(22)).style(move |_| container::Style {
                        background: Some(Background::Color(style::color(color))),
                        border: Border {
                            color: style::color(neutral::hairline()),
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..container::Style::default()
                    });
                let draft = self
                    .color_drafts
                    .get(role)
                    .cloned()
                    .unwrap_or_else(|| own.map(Rgba::to_hex).unwrap_or_default());
                let wrong = self
                    .color_drafts
                    .get(role)
                    .is_some_and(|d| !d.trim().is_empty() && Rgba::parse(d.trim()).is_none());
                let field = text_input(&color.to_hex(), &draft)
                    .on_input(move |text| Message::ColorDraft(role, text))
                    .on_submit(Message::ColorSet(role))
                    .padding([6, 10])
                    .size(13)
                    .width(100)
                    .font(style::mono())
                    .style(move |theme, status| {
                        if wrong {
                            style::wrong_field(palette)(theme, status)
                        } else {
                            style::field(palette)(theme, status)
                        }
                    });
                let mut control = row![swatch, field].spacing(10).align_y(Alignment::Center);
                if own.is_some() {
                    control = control.push(
                        button(icon(Icon::X, 12.0, style::faint()))
                            .padding(5)
                            .style(style::ghost(6.0))
                            .on_press(Message::ColorReset(role)),
                    );
                }
                setting(title, about, control)
            })
            .chain([note(
                "Over any palette, a role at a time; Enter keeps it, × gives the palette's back",
            )])
            .collect();
        group("Your colors", rows)
    }

    pub(super) fn set_color(&mut self, role: &'static str) {
        let Some(draft) = self.color_drafts.get(role) else {
            return;
        };
        let draft = draft.trim().to_owned();
        if draft.is_empty() {
            self.color_drafts.remove(role);
            self.edit(|e| e.unset("colors", role));
            return;
        }
        let text = if draft.starts_with('#') {
            draft
        } else {
            format!("#{draft}")
        };
        if let Some(color) = Rgba::parse(&text) {
            self.color_drafts.remove(role);
            self.edit(|e| e.set("colors", role, color.to_hex()));
        }
    }
}
