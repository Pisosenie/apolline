use super::*;
use iced::widget::column;

impl Settings {
    pub(super) fn keyboard(&self) -> Element<'_, Message> {
        let keyboard = &self.config.keyboard;
        let palette = self.palette();

        let mut chips = Row::new().spacing(8).align_y(Alignment::Center);
        for (index, code) in keyboard.layouts.iter().enumerate() {
            let name = LAYOUTS
                .iter()
                .find(|(c, _)| c == code)
                .map_or(code.as_str(), |(_, name)| name);
            chips = chips.push(
                container(
                    row![
                        text(code.to_uppercase())
                            .size(11)
                            .font(style::semibold())
                            .color(style::color(palette.accent)),
                        text(name).size(13),
                        button(icon(Icon::X, 12.0, style::faint()))
                            .padding(3)
                            .style(style::ghost(6.0))
                            .on_press(Message::RemoveLayout(index)),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding(iced::Padding::new(4.0).left(12.0).right(6.0))
                .style(style::pill(style::fill_color())),
            );
        }
        let options: Vec<Layout> = LAYOUTS
            .iter()
            .filter(|(code, _)| !keyboard.layouts.iter().any(|l| l == code))
            .map(|&(code, name)| Layout(code, name))
            .collect();
        let picker = pick_list(options, None::<Layout>, Message::AddLayout)
            .placeholder("Add a layout…")
            .text_size(13)
            .padding([8, 12])
            .width(240)
            .style(style::picker(palette))
            .menu_style(style::menu(palette));
        let layouts: Element<'_, Message> = if keyboard.layouts.is_empty() {
            column![
                text("Using the layouts from your niri config")
                    .size(13)
                    .color(style::color(style::faint())),
                picker
            ]
            .spacing(12)
            .into()
        } else {
            column![chips.wrap(), picker].spacing(12).into()
        };

        let mut switch_rows = vec![setting(
            "Switch layouts with",
            if keyboard.switch.is_none() {
                "Not set here yet, so niri's choice applies"
            } else {
                ""
            },
            segmented(
                [
                    (Switch::AltShift, "Alt+Shift", None),
                    (Switch::CtrlShift, "Ctrl+Shift", None),
                    (Switch::SuperSpace, "Super+Space", None),
                    (Switch::Caps, "Caps Lock", None),
                ],
                keyboard.switch,
                Message::Switch,
            ),
        )];
        switch_rows.push(note("Any other keys: Shortcuts → Next keyboard layout."));
        if keyboard.switch == Some(Switch::SuperSpace)
            && keys::same(&self.config.shortcuts.launcher, "Mod+Space")
        {
            switch_rows.push(warning(
                "Super+Space also opens the launcher. Pick another key here or in Shortcuts.",
            ));
        }

        let delay = keyboard.repeat_delay.unwrap_or(600);
        let rate = keyboard.repeat_rate.unwrap_or(25);
        let language = segmented(
            [
                ("auto", "Same as the system", None),
                ("en", "English", None),
                ("ru", "Русский", None),
            ],
            match self.config.language.as_deref() {
                Some("en") => "en",
                Some("ru") => "ru",
                _ => "auto",
            },
            Message::Language,
        );
        column![
            group(
                "Language",
                vec![setting(
                    "Apolline speaks",
                    "The bar, quick settings, the launcher and this window",
                    language
                )]
            ),
            group("Layouts", vec![padded(layouts)]),
            group("Switching", switch_rows),
            group(
                "Typing",
                vec![
                    setting(
                        "Repeat after",
                        "How long a key is held before it repeats",
                        value_slider(150..=1000, delay, Message::RepeatDelay, "ms", palette),
                    ),
                    setting(
                        "Repeat speed",
                        "Characters per second while held",
                        value_slider(10..=60, rate, Message::RepeatRate, "/s", palette),
                    ),
                    padded(
                        text_input(crate::i18n::t("Hold a key here to try it…"), &self.typing)
                            .on_input(Message::Typing)
                            .padding([10, 12])
                            .size(14)
                            .style(style::field(palette)),
                    ),
                ]
            ),
        ]
        .spacing(22)
        .into()
    }

    pub(super) fn pointer(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let touchpad = self.config.touchpad.unwrap_or_default();
        let mouse = self.config.mouse.unwrap_or_default();
        let mut form = Column::new().spacing(22);
        if self.config.touchpad.is_none() && self.config.mouse.is_none() {
            form = form.push(note(
                "Until you change something here, niri's own settings apply.",
            ));
        }
        form.push(group(
            "Touchpad",
            vec![
                setting(
                    "Tap to click",
                    "A light tap counts as a click",
                    switch(touchpad.tap, Message::Tap, palette),
                ),
                setting(
                    "Natural scrolling",
                    "Content follows your fingers",
                    switch(touchpad.natural_scroll, Message::TouchNatural, palette),
                ),
                setting(
                    "Ignore while typing",
                    "No stray clicks from your palms",
                    switch(touchpad.disable_while_typing, Message::TouchTyping, palette),
                ),
                setting(
                    "Speed",
                    "",
                    speed_slider(touchpad.speed, Message::TouchSpeed, palette),
                ),
            ],
        ))
        .push(group(
            "Mouse",
            vec![
                setting(
                    "Natural scrolling",
                    "",
                    switch(mouse.natural_scroll, Message::MouseNatural, palette),
                ),
                setting(
                    "Acceleration",
                    "Off gives the flat response many players prefer",
                    switch(mouse.acceleration, Message::MouseAcceleration, palette),
                ),
                setting(
                    "Speed",
                    "",
                    speed_slider(mouse.speed, Message::MouseSpeed, palette),
                ),
            ],
        ))
        .into()
    }

    pub(super) fn shortcuts(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let defaults = apolline_config::Shortcuts::default();
        let mut rows = Vec::new();
        for shortcut in Shortcut::ALL {
            let combo = self.config.shortcuts.get(shortcut);
            let recording = self.recording == Some(shortcut);
            let keys_view: Element<'_, Message> = if recording {
                container(
                    text("Press the keys…")
                        .size(13)
                        .color(style::color(palette.accent)),
                )
                .padding([6, 12])
                .style(style::pill(palette.accent_soft))
                .into()
            } else if combo.is_empty() {
                text(crate::i18n::t("Not set"))
                    .size(13)
                    .color(style::color(style::faint()))
                    .into()
            } else {
                keycaps(combo)
            };
            let mut controls = row![keys_view].spacing(8).align_y(Alignment::Center);
            controls = controls.push(
                button(text(if recording { "Cancel" } else { "Change" }).size(13))
                    .padding([7, 14])
                    .style(style::secondary)
                    .on_press(Message::Record(if recording {
                        None
                    } else {
                        Some(shortcut)
                    })),
            );
            if combo != defaults.get(shortcut) && !recording {
                controls = controls.push(
                    button(icon(Icon::Restart, 14.0, style::faint()))
                        .padding(8)
                        .style(style::ghost(8.0))
                        .on_press(Message::ResetShortcut(shortcut)),
                );
            }
            let (title, _) = fragment::action(shortcut);
            let replaced = self
                .niri_binds
                .iter()
                .find(|(bind, _)| keys::same(bind, combo))
                .map(|(_, action)| {
                    crate::i18n::fill(
                        "Replaces {action} from your niri config",
                        &[("action", &action)],
                    )
                });
            rows.push(setting_owned(title, replaced, controls));
        }
        if let Some(error) = &self.shortcut_error {
            rows.push(warning(error));
        }
        rows.push(note(
            "Mod is the Super key, or Alt when niri runs inside another window.",
        ));

        let mut known = Vec::new();
        for (combo, action) in &self.niri_binds {
            known.push(
                row![
                    container(keycaps(combo)).width(Length::FillPortion(2)),
                    text(action.as_str())
                        .width(Length::FillPortion(3))
                        .size(12)
                        .font(style::mono())
                        .color(style::color(style::soft_text())),
                ]
                .spacing(16)
                .align_y(Alignment::Center)
                .padding([6, 16])
                .into(),
            );
        }
        let mut form = column![group("Apolline", rows)].spacing(22);
        if !known.is_empty() {
            form = form.push(group(
                "Already in your niri config",
                vec![Column::with_children(known).spacing(0).into()],
            ));
        }
        form.into()
    }
}

pub(super) fn keycaps<'a>(combo: &str) -> Element<'a, Message> {
    Row::with_children(keys::caps(combo).into_iter().map(|cap| {
        container(text(cap).size(12).font(style::mono()))
            .padding([3, 8])
            .style(|_| container::Style {
                background: Some(Background::Color(style::color(style::fill_color()))),
                border: Border {
                    color: style::color(neutral::hairline()),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                shadow: Shadow {
                    color: iced::Color::from_rgba8(0, 0, 0, 0.5),
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 0.0,
                },
                ..container::Style::default()
            })
            .into()
    }))
    .spacing(4)
    .align_y(Alignment::Center)
    .into()
}

pub(super) fn read_niri_binds() -> Vec<(String, String)> {
    std::fs::read_to_string(fragment::niri_config())
        .map(|text| keys::binds(&text))
        .unwrap_or_default()
}
