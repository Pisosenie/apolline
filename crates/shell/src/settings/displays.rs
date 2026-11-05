use super::*;

impl Settings {
    pub(super) fn change_display(&mut self, change: impl FnOnce(&mut Editor)) {
        let before = self.saved.clone();
        let kept = self.revert.take();
        self.edit(change);
        self.revert = if self.saved == before {
            kept
        } else {
            let first = kept.map_or(before, |(_, text)| text);
            Some((
                std::time::Instant::now() + std::time::Duration::from_secs(15),
                first,
            ))
        };
    }

    pub(super) fn put_displays_back(&mut self) {
        let Some((_, before)) = self.revert.take() else {
            return;
        };
        if let Ok(before) = Editor::parse(&before) {
            self.edit(|e| e.take_list("display", &before));
        }
    }

    pub(super) fn outputs_may_change(&self) -> bool {
        self.revert.is_some()
    }

    pub(super) fn displays(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let mut page = Column::new().spacing(22);
        if let Some((at, _)) = &self.revert {
            let left = at
                .saturating_duration_since(std::time::Instant::now())
                .as_secs();
            page = page.push(
                container(
                    row![
                        icon(Icon::Monitor, 16.0, palette.accent),
                        text(format!(
                            "Keep these display settings? Going back in {left} s"
                        ))
                        .size(13)
                        .width(Fill),
                        button(text("Go back").size(13))
                            .padding([7, 14])
                            .style(style::secondary)
                            .on_press(Message::RevertDisplays),
                        button(text("Keep").size(13))
                            .padding([7, 16])
                            .style(style::primary(palette))
                            .on_press(Message::KeepDisplays),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .padding([10, 14])
                .style(style::pill(palette.accent_soft)),
            );
        }
        if self.outputs.is_empty() {
            page = page.push(note(
                "No screens to show: this works inside a niri session, where niri msg outputs lists them.",
            ));
        }
        let screens: Vec<crate::arrange::Screen> = self
            .outputs
            .iter()
            .filter_map(|output| {
                let logical = output.logical.as_ref()?;
                Some(crate::arrange::Screen {
                    name: output.name.clone(),
                    label: if output.model.is_empty() {
                        format!("{} × {}", logical.width, logical.height)
                    } else {
                        output.model.clone()
                    },
                    place: crate::arrange::Place {
                        x: logical.x.into(),
                        y: logical.y.into(),
                        width: logical.width.into(),
                        height: logical.height.into(),
                    },
                })
            })
            .collect();
        if screens.len() > 1 {
            page = page.push(group(
                "Arrangement",
                vec![
                    crate::arrange::view(screens, palette, Message::DisplayPlace),
                    note("Drag a screen to where it stands on your desk; it settles against the nearest edge."),
                ],
            ));
        }
        let niri = std::fs::read_to_string(fragment::niri_config()).unwrap_or_default();
        for output in &self.outputs {
            let name = output.name.clone();
            let mine = self.config.displays.iter().find(|d| d.name == output.name);
            let logical = output.logical.as_ref();
            let mode_label = |m: &apolline_niri::OutputMode| {
                let hz = format!("{:.3}", f64::from(m.refresh_rate) / 1000.0);
                format!(
                    "{} × {} · {} Hz",
                    m.width,
                    m.height,
                    hz.trim_end_matches('0').trim_end_matches('.')
                )
            };
            let modes: Vec<String> = output.modes.iter().map(mode_label).collect();
            let current = output
                .current_mode
                .and_then(|i| output.modes.get(i))
                .map(mode_label);
            let raw: Vec<(String, String)> = output
                .modes
                .iter()
                .map(|m| {
                    (
                        mode_label(m),
                        format!(
                            "{}x{}@{:.3}",
                            m.width,
                            m.height,
                            f64::from(m.refresh_rate) / 1000.0
                        ),
                    )
                })
                .collect();
            let scale = (logical.map_or(1.0, |l| l.scale) * 100.0).round() as u16;
            let turn = mine
                .and_then(|d| d.transform.as_deref())
                .unwrap_or("normal");
            let turn: &'static str = apolline_config::TRANSFORMS
                .into_iter()
                .find(|t| *t == turn)
                .unwrap_or("normal");
            let mut rows = vec![
                setting(
                    "Resolution",
                    "",
                    iced::widget::pick_list(modes, current, {
                        let name = name.clone();
                        move |label: String| {
                            let mode = raw
                                .iter()
                                .find(|(l, _)| *l == label)
                                .map(|(_, m)| m.clone())
                                .unwrap_or_default();
                            Message::DisplayMode(name.clone(), mode)
                        }
                    })
                    .text_size(13)
                    .padding([7, 12]),
                ),
                setting(
                    "Scale",
                    "Bigger text and controls on sharp screens",
                    segmented(
                        [
                            (100u16, "100%", None),
                            (125, "125%", None),
                            (150, "150%", None),
                            (175, "175%", None),
                            (200, "200%", None),
                        ],
                        scale,
                        {
                            let name = name.clone();
                            move |s| Message::DisplayScale(name.clone(), s)
                        },
                    ),
                ),
                setting(
                    "Rotation",
                    "",
                    segmented(
                        [
                            ("normal", "None", None),
                            ("90", "90°", None),
                            ("180", "180°", None),
                            ("270", "270°", None),
                        ],
                        turn,
                        {
                            let name = name.clone();
                            move |t| Message::DisplayTurn(name.clone(), t)
                        },
                    ),
                ),
                setting_owned(
                    "Position",
                    Some(format!(
                        "Now at {}, {} in the shared space of all screens",
                        logical.map_or(0, |l| l.x),
                        logical.map_or(0, |l| l.y)
                    )),
                    row![
                        text_input(
                            crate::i18n::t("x"),
                            if self.place.0 == name {
                                &self.place.1
                            } else {
                                ""
                            }
                        )
                        .on_input({
                            let name = name.clone();
                            move |x| Message::DisplayX(name.clone(), x)
                        })
                        .padding([7, 10])
                        .size(13)
                        .width(70)
                        .style(style::field(palette)),
                        text_input(
                            crate::i18n::t("y"),
                            if self.place.0 == name {
                                &self.place.2
                            } else {
                                ""
                            }
                        )
                        .on_input({
                            let name = name.clone();
                            move |y| Message::DisplayY(name.clone(), y)
                        })
                        .on_submit(Message::DisplayMove(name.clone()))
                        .padding([7, 10])
                        .size(13)
                        .width(70)
                        .style(style::field(palette)),
                        button(text("Move").size(13))
                            .padding([7, 14])
                            .style(style::secondary)
                            .on_press(Message::DisplayMove(name.clone())),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                ),
            ];
            rows.push(setting(
                "The bar",
                "Off leaves this screen to its windows and wallpaper",
                switch(
                    self.config.bar.shown_on(Some(&name)),
                    {
                        let name = name.clone();
                        move |on| Message::DisplayBar(name.clone(), on)
                    },
                    palette,
                ),
            ));
            if output.vrr_supported {
                rows.push(setting(
                    "Variable refresh rate",
                    "Always on; off lets only games in a performance mode use it",
                    switch(
                        mine.and_then(|d| d.vrr).unwrap_or(false),
                        {
                            let name = name.clone();
                            move |on| Message::DisplayVrr(name.clone(), on)
                        },
                        palette,
                    ),
                ));
            }
            if niri.contains(&format!("output \"{}\"", output.name)) {
                rows.push(note(
                    "Your niri config already describes this screen, and niri keeps the first description it reads, so change it there.",
                ));
            }
            let title = format!("{} {} · {}", output.make, output.model, output.name);
            page = page.push(group(title, rows));
        }
        page.push(self.night(palette)).into()
    }

    pub(super) fn night(&self, palette: Palette) -> Element<'_, Message> {
        let night = &self.config.night;
        let mut rows = vec![setting(
            "On a schedule",
            "Warmer screens for these hours; the tile in quick settings overrides it until the next change",
            switch(night.hours.is_some(), Message::NightSchedule, palette),
        )];
        if let Some((from, until)) = night.hours {
            let saved = Trigger::Hours { from, until }.text();
            let draft = self.night_draft.clone().unwrap_or_else(|| saved.clone());
            let valid = matches!(Trigger::parse(&draft), Some(Trigger::Hours { .. }));
            rows.push(setting_owned(
                "Hours",
                Some("From and until, like 21:00-07:00".to_owned()),
                row![
                    text_input(crate::i18n::t("21:00-07:00"), &draft)
                        .on_input(Message::NightDraft)
                        .on_submit(Message::SetNight)
                        .padding([8, 12])
                        .size(13)
                        .width(140)
                        .style(style::field(palette)),
                    button(text("Save").size(13))
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press_maybe((valid && draft != saved).then_some(Message::SetNight)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ));
            rows.push(setting(
                "Warmth",
                "Lower is warmer; 6500 K is daylight",
                value_slider(
                    2500..=6000,
                    night.temperature.clamp(2500, 6000) as u16,
                    Message::NightWarmth,
                    "K",
                    palette,
                ),
            ));
        }
        group("Night light", rows)
    }
}

// Two screens to show when there is no niri to ask, as in a preview on another system.
pub(super) fn sample_outputs() -> Vec<apolline_niri::Output> {
    let screen =
        |name: &str, make: &str, model: &str, mode: (u32, u32), logical| apolline_niri::Output {
            name: name.into(),
            make: make.into(),
            model: model.into(),
            modes: vec![apolline_niri::OutputMode {
                width: mode.0,
                height: mode.1,
                refresh_rate: 60_000,
                is_preferred: true,
            }],
            current_mode: Some(0),
            vrr_supported: false,
            vrr_enabled: false,
            logical: Some(logical),
        };
    let logical = |x, y, width, height, scale| apolline_niri::Logical {
        x,
        y,
        width,
        height,
        scale,
        transform: "Normal".into(),
    };
    vec![
        screen(
            "eDP-1",
            "BOE",
            "Built-in",
            (2560, 1600),
            logical(0, 440, 1600, 1000, 1.6),
        ),
        screen(
            "DP-1",
            "Dell Inc.",
            "U2723QE",
            (2560, 1440),
            logical(1600, 0, 2560, 1440, 1.0),
        ),
    ]
}
