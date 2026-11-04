use super::*;
use iced::widget::column;

impl Settings {
    pub(super) fn welcome(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let included = std::fs::read_to_string(fragment::niri_config())
            .is_ok_and(|text| text.contains(&fragment::path().display().to_string()));
        let mut connect = Column::new().spacing(6);
        if included {
            connect = connect.push(
                row![
                    icon(Icon::Done, 16.0, style::GOOD),
                    text("Apolline is in your niri config").size(13),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );
        } else {
            connect = connect.push(
                button(text("Add Apolline to niri").size(13))
                    .padding([8, 16])
                    .style(style::primary(palette))
                    .on_press(Message::Setup),
            );
        }
        for line in &self.setup_log {
            connect = connect.push(
                text(line.as_str())
                    .size(12)
                    .color(style::color(style::faint())),
            );
        }

        let looks = Row::with_children(
            self.themes
                .iter()
                .enumerate()
                .filter(|(_, (_, file))| file.is_none())
                .map(|(index, (theme, _))| self.theme_card(index, theme, false)),
        )
        .spacing(12);

        let modes = Row::with_children(TEMPLATES.map(|name| {
            let have = self.config.find(name).is_some();
            let label = row![
                icon(
                    if have { Icon::Check } else { Icon::Plus },
                    13.0,
                    neutral::text()
                ),
                text(name).size(13),
            ]
            .spacing(6)
            .align_y(Alignment::Center);
            button(label)
                .padding([7, 14])
                .style(style::choice(have))
                .on_press_maybe((!have).then_some(Message::AddMode(name)))
                .into()
        }))
        .spacing(8)
        .wrap();

        let shortcuts = &self.config.shortcuts;
        let keys = Column::with_children(
            [
                (
                    shortcuts.get(Shortcut::Launcher).to_owned(),
                    "Open apps, games, windows and sums",
                ),
                (
                    shortcuts.get(Shortcut::Panel).to_owned(),
                    "Quick settings and past notifications",
                ),
                ("Alt+Tab".to_owned(), "Jump between recent windows"),
                (
                    shortcuts.get(Shortcut::Keys).to_owned(),
                    "Every other shortcut, searchable",
                ),
                (shortcuts.get(Shortcut::Lock).to_owned(), "Lock the screen"),
            ]
            .into_iter()
            .map(|(combo, what)| {
                row![
                    container(text(combo).size(12).font(style::mono()))
                        .padding([4, 10])
                        .width(150)
                        .style(style::pill(Rgba::hex(0xffffff).alpha(0.08))),
                    text(what).size(13).color(style::color(style::soft_text())),
                ]
                .spacing(14)
                .align_y(Alignment::Center)
                .into()
            }),
        )
        .spacing(8);

        column![
            welcome_step(
                palette,
                1,
                "Connect to niri",
                "One include line at the end of your niri config; a copy of it is kept.",
                connect.into()
            ),
            welcome_step(
                palette,
                2,
                "Pick a look",
                "Wallpaper, accent, corners and motion in one click.",
                column![
                    looks,
                    row![
                        switch(self.config.apps.colors, Message::ColorApps, palette),
                        text("Color GTK and Qt apps with the same accent").size(13),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                ]
                .spacing(14)
                .into()
            ),
            welcome_step(
                palette,
                3,
                "Modes, if you want them",
                "A mode is one key that changes wallpaper, apps, notifications and performance.",
                modes.into()
            ),
            welcome_step(
                palette,
                4,
                "Five keys to remember",
                "Mod is the Super key.",
                keys.into()
            ),
            row![
                space::horizontal(),
                button(text("More settings").size(13))
                    .padding([9, 16])
                    .style(style::secondary)
                    .on_press(Message::Page(Page::Modes)),
                button(text("Done").size(13))
                    .padding([9, 22])
                    .style(style::primary(palette))
                    .on_press(Message::Finish),
            ]
            .spacing(8),
        ]
        .spacing(14)
        .into()
    }

    pub(super) fn about(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let niri_running = std::env::var_os("NIRI_SOCKET").is_some();
        let included = std::fs::read_to_string(fragment::niri_config())
            .is_ok_and(|text| text.contains(&fragment::path().display().to_string()));

        let hero = row![
            container(icon(Icon::Sparkles, 28.0, palette.on_accent))
                .center(64)
                .style(style::pill(palette.accent)),
            column![
                text("Apolline").size(34).font(style::light()),
                text(format!(
                    "Version {} · a calm shell for niri",
                    env!("CARGO_PKG_VERSION")
                ))
                .size(13)
                .color(style::color(style::faint())),
            ]
            .spacing(2),
        ]
        .spacing(18)
        .align_y(Alignment::Center);

        let status = |good: bool, yes: &str, no: &str| {
            let (glyph, tint, label) = if good {
                (Icon::Done, style::GOOD, yes.to_owned())
            } else {
                (Icon::Warning, style::WARN, no.to_owned())
            };
            row![
                icon(glyph, 15.0, tint),
                text(label).size(13).color(style::color(tint))
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        };

        let setup: Element<'_, Message> = if included {
            status(true, "Included in your niri config", "").into()
        } else {
            row![
                status(false, "", "Not set up"),
                button(text("Set up").size(13))
                    .padding([7, 14])
                    .style(style::primary(palette))
                    .on_press(Message::Setup),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
        };

        let mut rows = vec![
            setting(
                "niri",
                "",
                status(niri_running, "Running", "Not running in this session"),
            ),
            setting(
                "Apolline in niri",
                "Wallpaper, bar, keys and window style",
                setup,
            ),
            setting_owned(
                "Config file",
                Some(self.path.display().to_string()),
                button(
                    row![
                        icon(Icon::External, 14.0, neutral::text()),
                        text("Open").size(13)
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([7, 14])
                .style(style::secondary)
                .on_press(Message::OpenFile),
            ),
        ];
        for line in &self.setup_log {
            rows.push(note(line));
        }

        let mut check = vec![setting(
            "Config check",
            "The same as running  apolline check",
            status(
                self.diagnostics.is_empty() && self.editor.is_ok(),
                "No problems",
                "Needs a look",
            ),
        )];
        if let Err(err) = &self.editor {
            check.push(warning(err));
        }
        for diagnostic in &self.diagnostics {
            let mut line = format!("line {}: {}", diagnostic.line, diagnostic.message);
            if let Some(hint) = &diagnostic.hint {
                line.push_str(&format!(" — {hint}"));
            }
            check.push(warning(&line));
        }

        column![hero, group("Status", rows), group("Health", check)]
            .spacing(26)
            .into()
    }
}

fn welcome_step<'a>(
    palette: Palette,
    number: u8,
    title: &'static str,
    about: &'static str,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let badge = text(number.to_string())
        .size(13)
        .font(style::semibold())
        .color(style::color(palette.on_accent));
    container(
        row![
            container(badge)
                .center(28)
                .style(style::pill(palette.accent)),
            column![
                text(title).size(16).font(style::semibold()),
                text(about).size(13).color(style::color(style::faint())),
                body,
            ]
            .spacing(10)
            .width(Fill),
        ]
        .spacing(16),
    )
    .padding(20)
    .width(Fill)
    .style(style::card)
    .into()
}
