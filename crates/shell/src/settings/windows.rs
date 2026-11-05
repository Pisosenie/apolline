use super::*;
use iced::widget::column;

impl Settings {
    pub(super) fn windows(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let windows = &self.config.windows;
        let width = windows.width.unwrap_or(Width::Half);
        let opacity = windows.opacity.unwrap_or(1.0);
        let inactive = windows.inactive_opacity.unwrap_or(opacity);
        let centered = windows.center == Some(Center::Always)
            || (windows.center == Some(Center::Overflow) && width == Width::Full);

        let pane = move |focused: bool, portion: u16| {
            let alpha = if focused { opacity } else { inactive };
            container(
                column![
                    container(space())
                        .width(Length::FillPortion(3))
                        .height(7)
                        .style(style::pill(Rgba::hex(0xffffff).alpha(0.16 * alpha))),
                    container(space())
                        .width(Length::FillPortion(2))
                        .height(5)
                        .style(style::pill(Rgba::hex(0xffffff).alpha(0.08 * alpha))),
                ]
                .spacing(7),
            )
            .padding(12)
            .width(Length::FillPortion(portion.max(1)))
            .height(Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(style::color(
                    neutral::window().alpha(alpha * 0.96),
                ))),
                border: Border {
                    color: style::color(if focused {
                        palette.accent
                    } else {
                        Rgba::hex(0xffffff).alpha(0.08)
                    }),
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..container::Style::default()
            })
        };
        let focused = (width.proportion() * 1000.0) as u16;
        let rest = 1000 - focused.min(1000);
        let strip: Element<'_, Message> = if width == Width::Full {
            row![pane(true, 1000)].into()
        } else if centered {
            row![
                pane(false, rest / 2),
                pane(true, focused),
                pane(false, rest / 2)
            ]
            .spacing(10)
            .into()
        } else {
            row![
                pane(false, rest / 3),
                pane(true, focused),
                pane(false, rest - rest / 3)
            ]
            .spacing(10)
            .into()
        };
        let preview = self.preview(
            &self.config.desktop.wallpaper,
            170.0,
            container(strip).padding(12).into(),
        );

        let presets = windows
            .presets
            .clone()
            .unwrap_or_else(|| vec![Width::Third, Width::Half, Width::TwoThirds]);
        let preset_chips = Row::with_children(Width::ALL.map(|w| {
            button(text(w.key()).size(13))
                .padding([6, 12])
                .style(style::choice(presets.contains(&w)))
                .on_press(Message::WinPreset(w))
                .into()
        }))
        .spacing(4);

        let speed = match windows.animations {
            Some(s) if s <= 0.0 => 0,
            Some(s) if s > 1.25 => 1,
            Some(s) if s < 0.8 => 3,
            _ => 2,
        };
        let percent = |value: f32| (value * 100.0).round() as u16;

        let mut rules: Vec<Element<'_, Message>> = self
            .config
            .rules
            .iter()
            .enumerate()
            .map(|(index, rule)| {
                let mut facts = Vec::new();
                match rule.floating {
                    Some(true) => facts.push("floating".to_owned()),
                    Some(false) => facts.push("tiled".to_owned()),
                    None => {}
                }
                if let Some(width) = rule.width {
                    facts.push(format!("{} wide", width.key()));
                }
                if let Some(opacity) = rule.opacity {
                    facts.push(format!("{}% opaque", percent(opacity)));
                }
                row![
                    icon(Icon::AppWindow, 15.0, style::faint()),
                    text(rule.app.as_str()).size(13).font(style::mono()),
                    text(facts.join(" · "))
                        .size(12)
                        .color(style::color(style::faint()))
                        .width(Fill),
                    button(icon(Icon::X, 14.0, style::faint()))
                        .padding(6)
                        .style(style::ghost(8.0))
                        .on_press(Message::RemoveRule(index)),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding([10, 16])
                .into()
            })
            .collect();
        let width_choices = Row::with_children(Width::ALL.map(|w| {
            button(text(w.key()).size(12))
                .padding([5, 9])
                .style(style::choice(self.rule.width == Some(w)))
                .on_press(Message::RuleWidth(w))
                .into()
        }))
        .spacing(2);
        rules.push(
            row![
                text_input(crate::i18n::t("app id, like pavucontrol"), &self.rule.app)
                    .on_input(Message::RuleApp)
                    .on_submit(Message::AddRule)
                    .padding([8, 12])
                    .size(13)
                    .font(style::mono())
                    .width(230)
                    .style(style::field(palette)),
                row![
                    text("Floating")
                        .size(12)
                        .color(style::color(style::soft_text())),
                    switch(
                        self.rule.floating == Some(true),
                        Message::RuleFloating,
                        palette
                    ),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                width_choices,
                space::horizontal(),
                button(text("Add").size(13))
                    .padding([8, 14])
                    .style(style::primary(palette))
                    .on_press_maybe((!self.rule.app.trim().is_empty()).then_some(Message::AddRule)),
            ]
            .spacing(14)
            .align_y(Alignment::Center)
            .padding([12, 16])
            .into(),
        );
        rules.push(note(
            "The app id is what niri calls the window; `niri msg windows` lists them.",
        ));

        column![
            preview,
            group(
                "New windows",
                vec![
                    setting(
                        "Open at",
                        if windows.width.is_none() {
                            "Not set here, so niri's own choice applies"
                        } else {
                            "Width of a new column"
                        },
                        segmented(
                            Width::ALL.map(|w| (w, w.key(), None)),
                            windows.width,
                            Message::WinWidth,
                        ),
                    ),
                    setting("Widths Mod+R cycles through", "", preset_chips),
                ]
            ),
            group(
                "Focus",
                vec![
                    setting(
                        "Alt+Tab with previews",
                        "Recent windows first; Alt+` stays in one app; arrows, Home and End move",
                        switch(windows.switcher, Message::RecentWindows, palette),
                    ),
                    setting(
                        "Center the focused window",
                        "",
                        segmented(
                            [
                                (Center::Never, "Never", None),
                                (Center::Overflow, "When it doesn't fit", None),
                                (Center::Always, "Always", None),
                            ],
                            windows.center,
                            Message::WinCenter,
                        ),
                    ),
                    setting(
                        "Center a lone window",
                        "One window on a workspace sits in the middle",
                        switch(windows.center_lone, Message::CenterLone, palette),
                    ),
                    setting(
                        "Focus follows the mouse",
                        "Pointing at a window focuses it",
                        switch(windows.focus_follows_mouse, Message::FocusFollows, palette),
                    ),
                    setting(
                        "Pointer follows focus",
                        "Switching windows with the keyboard moves the pointer along",
                        switch(windows.warp_mouse, Message::WarpMouse, palette),
                    ),
                ]
            ),
            group(
                "Look",
                vec![
                    setting(
                        "Focused window",
                        "How opaque the window you're in is",
                        value_slider(30..=100, percent(opacity), Message::Opacity, "%", palette),
                    ),
                    setting(
                        "Other windows",
                        "Fade the rest to see what's in focus",
                        value_slider(
                            30..=100,
                            percent(inactive),
                            Message::InactiveOpacity,
                            "%",
                            palette
                        ),
                    ),
                    setting(
                        "Blur behind",
                        "Shows through see-through windows: just the wallpaper, or everything under them",
                        segmented(
                            [
                                (None, "Off", None),
                                (Some(Blur::Wallpaper), "Wallpaper", None),
                                (Some(Blur::All), "Everything", None),
                            ],
                            Some(windows.blur),
                            Message::WinBlur,
                        ),
                    ),
                ]
            ),
            group(
                "Motion",
                vec![
                    setting(
                        "Speed",
                        "Performance modes always turn animations off",
                        segmented(
                            [
                                (0u8, "Off", None),
                                (1, "Fast", None),
                                (2, "Normal", None),
                                (3, "Slow", None),
                            ],
                            speed,
                            Message::Animations,
                        ),
                    ),
                    setting(
                        "Feel",
                        "How windows and workspaces move; bouncy overshoots a little",
                        segmented(
                            [
                                (None, "niri's", None),
                                (Some(Motion::Snappy), "Snappy", None),
                                (Some(Motion::Smooth), "Smooth", None),
                                (Some(Motion::Bouncy), "Bouncy", None),
                            ],
                            Some(windows.motion),
                            Message::WinMotion,
                        ),
                    ),
                    setting(
                        "New windows",
                        "How a window shows up when it opens",
                        segmented(
                            [
                                (None, "niri's", None),
                                (Some(Appear::Pop), "Pop", None),
                                (Some(Appear::Slide), "Slide", None),
                                (Some(Appear::Fade), "Fade", None),
                            ],
                            Some(windows.appear),
                            Message::WinAppear,
                        ),
                    ),
                ]
            ),
            group("Apps", rules),
        ]
        .spacing(22)
        .into()
    }

    pub(super) fn app_list_of(&self, key: &str) -> &[String] {
        match key {
            "mute" => &self.config.alerts.mute,
            "silent" => &self.config.alerts.silent,
            _ => &self.config.alerts.always,
        }
    }
}
