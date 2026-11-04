use super::*;
use iced::widget::column;

impl Settings {
    pub(super) fn modes(&self) -> Element<'_, Message> {
        if self.config.modes.is_empty() || self.adding {
            return self.templates();
        }
        let index = self.selected.min(self.config.modes.len() - 1);
        let mode = &self.config.modes[index];
        let palette = self.config.palette(Some(index));

        let mut tabs = Row::new().spacing(6).align_y(Alignment::Center);
        for (i, other) in self.config.modes.iter().enumerate() {
            let on = i == index;
            let tint = if on {
                self.config.palette(Some(i)).accent
            } else {
                style::faint()
            };
            tabs = tabs.push(
                button(
                    row![
                        icon(Icon::named(&other.icon), 14.0, tint),
                        text(other.name.as_str()).size(13)
                    ]
                    .spacing(7)
                    .align_y(Alignment::Center),
                )
                .padding([7, 14])
                .style(style::choice(on))
                .on_press(Message::Select(i)),
            );
        }
        tabs = tabs.push(
            button(
                row![
                    icon(Icon::Plus, 14.0, style::soft_text()),
                    text("Add").size(13)
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([7, 12])
            .style(style::ghost(8.0))
            .on_press(Message::Adding(true)),
        );

        let preview = self.preview(self.config.wallpaper(Some(index)), 170.0, {
            let chip = container(
                row![
                    icon(Icon::named(&mode.icon), 12.0, palette.accent),
                    text(mode.name.as_str())
                        .size(11)
                        .color(style::color(palette.accent))
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            )
            .padding([3, 9])
            .style(style::pill(palette.accent_soft));
            let mini_bar = container(
                row![chip, space::horizontal(), text("14:32").size(11)].align_y(Alignment::Center),
            )
            .padding([5, 6])
            .width(Fill)
            .style(style::bar);
            let window =
                container(space())
                    .width(Fill)
                    .height(Fill)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(style::color(
                            neutral::window().alpha(0.92),
                        ))),
                        border: Border {
                            color: style::color(palette.accent),
                            width: 1.0,
                            radius: 8.0.into(),
                        },
                        ..container::Style::default()
                    });
            column![mini_bar, row![window, space().width(Fill)].spacing(8)]
                .spacing(8)
                .padding(10)
                .into()
        });

        let name = text_input(
            crate::i18n::t("Name"),
            self.name_draft.as_deref().unwrap_or(&mode.name),
        )
        .on_input(Message::ModeName)
        .on_submit(Message::ModeNameDone)
        .padding([8, 12])
        .size(14)
        .width(260)
        .style(style::field(palette));
        let rename = button(text("Rename").size(13))
            .padding([8, 14])
            .style(style::secondary)
            .on_press_maybe(self.name_draft.is_some().then_some(Message::ModeNameDone));
        let icons = Row::with_children(MODE_ICONS.map(|(key, glyph)| {
            let on = mode.icon == key;
            button(
                container(icon(
                    glyph,
                    17.0,
                    if on {
                        palette.on_accent
                    } else {
                        style::soft_text()
                    },
                ))
                .center(34),
            )
            .padding(0)
            .style(move |theme, status| {
                let mut style = style::tile(on, palette)(theme, status);
                style.border.radius = 9.0.into();
                style
            })
            .on_press(Message::ModeIcon(key))
            .into()
        }))
        .spacing(6)
        .wrap()
        .vertical_spacing(6);

        let mut apps = Column::new().spacing(8);
        for (app, command) in mode.open.iter().enumerate() {
            apps = apps.push(
                row![
                    icon(Icon::AppWindow, 15.0, style::faint()),
                    text(command.as_str()).font(style::mono()).size(13),
                    space::horizontal(),
                    button(icon(Icon::X, 14.0, style::faint()))
                        .padding(6)
                        .style(style::ghost(8.0))
                        .on_press(Message::Remove(app)),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            );
        }
        if mode.open.is_empty() {
            apps = apps.push(
                text("Nothing opens yet")
                    .size(13)
                    .color(style::color(style::faint())),
            );
        }
        apps = apps.push(
            row![
                text_input(crate::i18n::t("firefox --new-window"), &self.draft)
                    .on_input(Message::Draft)
                    .on_submit(Message::Add)
                    .padding([8, 12])
                    .size(13)
                    .font(style::mono())
                    .style(style::field(palette)),
                button(
                    row![
                        icon(Icon::Plus, 14.0, palette.on_accent),
                        text("Add").size(13)
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center)
                )
                .padding([8, 14])
                .style(style::primary(palette))
                .on_press(Message::Add),
            ]
            .spacing(8),
        );

        let mut keys = vec![];
        for (k, (combo, command)) in mode.keys.iter().enumerate() {
            let taken = Shortcut::ALL
                .into_iter()
                .find(|&s| keys::same(self.config.shortcuts.get(s), combo));
            let replaced = self
                .niri_binds
                .iter()
                .find(|(bind, _)| keys::same(bind, combo));
            let about = match (taken, replaced) {
                (Some(shortcut), _) => Some(format!(
                    "Taken by “{}”, so it won't work; pick other keys",
                    fragment::action(shortcut).0
                )),
                (None, Some((_, action))) => Some(crate::i18n::fill(
                    "Replaces {action} while this mode is on",
                    &[("action", &action)],
                )),
                _ => None,
            };
            let mut words = column![text(command.as_str()).size(13).font(style::mono())].spacing(3);
            if let Some(about) = about {
                let tint = if taken.is_some() {
                    style::WARN
                } else {
                    style::faint()
                };
                words = words.push(text(about).size(12).color(style::color(tint)));
            }
            keys.push(
                row![
                    container(keycaps(combo)).width(Length::Fixed(200.0)),
                    words.width(Fill),
                    button(icon(Icon::X, 14.0, style::faint()))
                        .padding(6)
                        .style(style::ghost(8.0))
                        .on_press(Message::RemoveModeKey(k)),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding([10, 16])
                .into(),
            );
        }
        let capture: Element<'_, Message> = match (&self.key_combo, self.recording_mode_key) {
            (_, true) => container(
                text("Press the keys…")
                    .size(13)
                    .color(style::color(palette.accent)),
            )
            .padding([6, 12])
            .style(style::pill(palette.accent_soft))
            .into(),
            (Some(combo), false) => button(keycaps(combo))
                .padding(2)
                .style(style::ghost(8.0))
                .on_press(Message::RecordModeKey)
                .into(),
            (None, false) => button(text("Choose keys").size(13))
                .padding([7, 14])
                .style(style::secondary)
                .on_press(Message::RecordModeKey)
                .into(),
        };
        keys.push(
            row![
                container(capture).width(Length::Fixed(200.0)),
                text_input(
                    crate::i18n::t("command, like  steam  or  niri msg action fullscreen-window"),
                    &self.key_command
                )
                .on_input(Message::KeyCommand)
                .on_submit(Message::AddModeKey)
                .padding([8, 12])
                .size(13)
                .font(style::mono())
                .style(style::field(palette)),
                button(text("Add").size(13))
                    .padding([8, 14])
                    .style(style::primary(palette))
                    .on_press_maybe(
                        (self.key_combo.is_some() && !self.key_command.trim().is_empty())
                            .then_some(Message::AddModeKey)
                    ),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .padding([12, 16])
            .into(),
        );

        let starts_here = self.config.start() == Some(index);
        let last = self.config.modes.len() - 1;
        let manage = row![
            button(
                row![
                    icon(Icon::ArrowUp, 14.0, style::soft_text()),
                    text("Earlier").size(13)
                ]
                .spacing(6)
                .align_y(Alignment::Center)
            )
            .padding([7, 12])
            .style(style::secondary)
            .on_press_maybe((index > 0).then_some(Message::MoveMode(-1))),
            button(
                row![
                    icon(Icon::ArrowDown, 14.0, style::soft_text()),
                    text("Later").size(13)
                ]
                .spacing(6)
                .align_y(Alignment::Center)
            )
            .padding([7, 12])
            .style(style::secondary)
            .on_press_maybe((index < last).then_some(Message::MoveMode(1))),
            space::horizontal(),
            button(
                text(if self.armed {
                    "Delete for good?"
                } else {
                    "Delete mode"
                })
                .size(13)
            )
            .padding([7, 14])
            .style(move |theme, status| {
                let mut style = style::danger(self.armed)(theme, status);
                style.border.radius = 10.0.into();
                style
            })
            .on_press(Message::RemoveMode),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        column![
            tabs.wrap().vertical_spacing(6),
            preview,
            self.why_on(mode, palette),
            group(
                "Name and icon",
                vec![
                    padded(row![name, rename].spacing(8).align_y(Alignment::Center)),
                    padded(icons),
                ]
            ),
            group(
                "Look",
                vec![
                    padded(self.wallpaper_picker(
                        mode.wallpaper.as_deref(),
                        true,
                        mode_wallpaper,
                        palette
                    )),
                    padded(accent_picker(
                        mode.accent,
                        auto_accent(self.config.wallpaper(Some(self.selected))),
                        true,
                        mode_accent,
                        palette
                    )),
                ]
            ),
            group(
                "Notifications",
                vec![setting(
                    "Popups",
                    "Held ones wait behind the bell on the bar",
                    rules(index, mode.notifications),
                )]
            ),
            group("Open when this mode starts", vec![padded(apps)]),
            group(
                "Theme",
                vec![padded(
                    Row::with_children(
                        std::iter::once(None)
                            .chain(
                                self.themes
                                    .iter()
                                    .map(|(theme, _)| Some(theme.name.clone()))
                            )
                            .map(|name| {
                                let on = mode.theme.as_deref().map(apolline_config::slug)
                                    == name.as_deref().map(apolline_config::slug);
                                let label = name.clone().unwrap_or_else(|| "The desktop's".into());
                                button(text(label).size(12))
                                    .padding([6, 12])
                                    .style(style::choice(on))
                                    .on_press(Message::ModeTheme(name))
                                    .into()
                            }),
                    )
                    .spacing(4)
                    .wrap(),
                )]
            ),
            group(
                "Behaviour",
                vec![
                    setting(
                        "Performance",
                        "Power profile to performance, games drive the refresh rate, effects off",
                        switch(mode.performance, Message::Performance, palette),
                    ),
                    setting(
                        "Keep the screen awake",
                        "No idle lock, no screen off, no sleep while this mode is on",
                        switch(mode.keep_awake, Message::KeepAwake, palette),
                    ),
                    setting(
                        "Start in this mode",
                        "Apolline switches to it when you log in",
                        switch(starts_here, Message::StartHere, palette),
                    ),
                ]
            ),
            group("Turns itself on", self.triggers(mode, palette)),
            group("Keys only in this mode", keys),
            padded(manage),
        ]
        .spacing(22)
        .into()
    }

    pub(super) fn why_on(
        &self,
        mode: &ModeSettings,
        palette: Palette,
    ) -> Option<Element<'_, Message>> {
        if self.live.mode.as_deref() != Some(mode.key().as_str()) {
            return None;
        }
        let reason = self.live.reason.as_deref().and_then(Trigger::parse)?;
        let because = match reason {
            Trigger::Game => "a game is running".to_owned(),
            Trigger::Battery => "the laptop runs on battery".to_owned(),
            Trigger::Hours { from, until } => format!(
                "it's between {} and {}",
                apolline_config::clock(from),
                apolline_config::clock(until)
            ),
            Trigger::Monitor(name) => format!("{name} is plugged in"),
        };
        let back = self
            .live
            .back
            .as_deref()
            .and_then(|key| self.config.find(key))
            .map_or("the plain desktop".to_owned(), |i| {
                self.config.modes[i].name.clone()
            });
        Some(
            container(
                row![
                    icon(Icon::Zap, 15.0, palette.accent),
                    text(format!(
                        "On now because {because}. When that ends, {back} comes back."
                    ))
                    .size(13),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding([10, 14])
            .width(Fill)
            .style(style::pill(palette.accent_soft))
            .into(),
        )
    }

    pub(super) fn set_trigger(&mut self, index: usize, trigger: Trigger, on: bool) {
        let Some(mode) = self.config.modes.get(index) else {
            return;
        };
        let mut when: Vec<Trigger> = mode
            .when
            .iter()
            .filter(|t| std::mem::discriminant(*t) != std::mem::discriminant(&trigger))
            .cloned()
            .collect();
        if on {
            when.push(trigger);
        }
        self.write_triggers(index, &when);
    }

    pub(super) fn write_triggers(&mut self, index: usize, when: &[Trigger]) {
        let list: Vec<String> = when.iter().map(Trigger::text).collect();
        self.edit(|e| {
            if list.is_empty() {
                e.unset_mode(index, "when");
            } else {
                e.set_mode_list(index, "when", &list);
            }
        });
    }

    pub(super) fn triggers(
        &self,
        mode: &ModeSettings,
        palette: Palette,
    ) -> Vec<Element<'_, Message>> {
        let has = |wanted: &Trigger| {
            mode.when
                .iter()
                .any(|t| std::mem::discriminant(t) == std::mem::discriminant(wanted))
        };
        let mut rows = vec![
            setting(
                "While a game runs",
                "Steam, Proton, gamescope or Wine; it ends when the last game closes",
                switch(has(&Trigger::Game), Message::WhenGame, palette),
            ),
            setting(
                "On battery",
                "Unplugging turns it on, plugging in brings back the mode you had",
                switch(has(&Trigger::Battery), Message::WhenBattery, palette),
            ),
        ];
        let field = |placeholder: &'static str,
                     value: &str,
                     on_input: fn(String) -> Message,
                     submit: Message| {
            row![
                text_input(placeholder, value)
                    .on_input(on_input)
                    .on_submit(submit.clone())
                    .padding([8, 12])
                    .size(13)
                    .width(200)
                    .style(style::field(palette)),
                button(text("Add").size(13))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press(submit),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        };
        rows.push(setting(
            "At these hours",
            "Like 20:00-07:00; it may run past midnight",
            field(
                "20:00-07:00",
                &self.hours_draft,
                Message::HoursDraft,
                Message::SetHours,
            ),
        ));
        rows.push(setting(
            "When a monitor is connected",
            "Its name from niri msg outputs, like HDMI-A-1",
            field(
                "HDMI-A-1",
                &self.monitor_draft,
                Message::MonitorDraft,
                Message::SetMonitor,
            ),
        ));
        let listed: Vec<(usize, &Trigger)> = mode
            .when
            .iter()
            .enumerate()
            .filter(|(_, t)| matches!(t, Trigger::Hours { .. } | Trigger::Monitor(_)))
            .collect();
        if !listed.is_empty() {
            let chips = Row::with_children(listed.into_iter().map(|(at, trigger)| {
                let label = match trigger {
                    Trigger::Hours { from, until } => {
                        format!(
                            "{} – {}",
                            apolline_config::clock(*from),
                            apolline_config::clock(*until)
                        )
                    }
                    Trigger::Monitor(name) => format!("{name} connected"),
                    _ => trigger.text(),
                };
                container(
                    row![
                        text(label).size(12),
                        button(icon(Icon::X, 12.0, style::faint()))
                            .padding(2)
                            .style(style::ghost(6.0))
                            .on_press(Message::DropTrigger(at)),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([4, 10])
                .style(style::pill(Rgba::hex(0xffffff).alpha(0.06)))
                .into()
            }))
            .spacing(6)
            .wrap();
            rows.push(padded(chips));
        }
        rows.push(note(
            "When the reason ends, the mode you had before comes back. Switching by hand always wins.",
        ));
        rows
    }
}

pub(super) fn mode_wallpaper(name: Option<String>) -> Message {
    Message::ModeWallpaper(name)
}

pub(super) fn mode_accent(accent: Option<Accent>) -> Message {
    Message::ModeAccent(accent)
}

pub(super) fn rules<'a>(index: usize, current: Notifications) -> Element<'a, Message> {
    let on = move |n| Message::Notifications(index, n);
    let buttons = Row::with_children(
        [
            (Notifications::All, "All"),
            (Notifications::Mentions, "Hold quiet ones"),
            (Notifications::Hold, "Only urgent"),
        ]
        .map(|(value, label)| {
            button(text(label).size(13))
                .padding([6, 12])
                .style(style::choice(value == current))
                .on_press(on(value))
                .into()
        }),
    )
    .spacing(2);
    container(buttons).padding(3).style(style::segments).into()
}

// What a mode puts over the everyday look while it's on.
pub(super) fn mode_overrides(mode: &ModeSettings) -> Vec<String> {
    let mut over = Vec::new();
    if let Some(theme) = &mode.theme {
        over.push(format!("{theme}'s look"));
    }
    if mode.wallpaper.is_some() && mode.theme.is_none() {
        over.push("its own wallpaper".to_owned());
    }
    if mode.accent.is_some() && mode.theme.is_none() {
        over.push("its own accent".to_owned());
    }
    if mode.performance {
        over.push("no shadows, blur or animations".to_owned());
    }
    over
}
