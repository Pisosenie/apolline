use super::*;
use iced::widget::column;

impl Settings {
    pub(super) fn module_rows(&self, palette: Palette) -> Vec<Element<'_, Message>> {
        let mut rows: Vec<Element<'_, Message>> = self
            .config
            .modules
            .iter()
            .enumerate()
            .map(|(index, module)| {
                let field = |which: ModuleField, written: String| match &self.module_field {
                    Some((at, now, draft)) if *at == index && *now == which => draft.clone(),
                    _ => written,
                };
                let input = |which: ModuleField, placeholder: &'static str, value: String| {
                    text_input(crate::i18n::t(placeholder), &value)
                        .on_input(move |draft| Message::ModuleField(index, which, draft))
                        .on_submit(Message::ModuleFieldDone)
                        .padding([5, 8])
                        .size(12)
                        .style(style::field(palette))
                };
                let name = field(ModuleField::Name, module.name.clone().unwrap_or_default());
                let every = field(
                    ModuleField::Every,
                    module.every.map(|s| s.to_string()).unwrap_or_default(),
                );
                row![
                    text(module.command.as_str())
                        .size(12)
                        .font(style::mono())
                        .width(Fill),
                    text(if module.click.is_some() {
                        "clickable"
                    } else {
                        ""
                    })
                    .size(11)
                    .color(style::color(style::faint())),
                    input(ModuleField::Name, "name", name).width(110),
                    input(ModuleField::Every, "every … s", every).width(80),
                    button(icon(Icon::X, 14.0, style::faint()))
                        .padding(6)
                        .style(style::ghost(8.0))
                        .on_press(Message::RemoveModule(index)),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding([8, 16])
                .into()
            })
            .collect();
        rows.push(
            row![
                text_input(
                    crate::i18n::t("A command, like  date +%A"),
                    &self.module_draft
                )
                .on_input(Message::ModuleDraft)
                .on_submit(Message::AddModule)
                .padding([8, 12])
                .size(13)
                .font(style::mono())
                .style(style::field(palette)),
                button(text("Add").size(13))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press_maybe(
                        (!self.module_draft.trim().is_empty()).then_some(Message::AddModule)
                    ),
            ]
            .spacing(8)
            .padding([12, 16])
            .into(),
        );
        rows.push(note(
            "Each line the command prints shows on the bar. One that prints once and exits reruns every so many seconds. A name places it on the bar on its own; click = a command for a click goes in the file.",
        ));
        rows
    }

    // A name or a rerun typed for a module; a renamed module keeps its place on the bar.
    pub(super) fn module_written(&mut self, index: usize, field: ModuleField, value: &str) {
        let Some(module) = self.config.modules.get(index) else {
            return;
        };
        match field {
            ModuleField::Every if value.is_empty() => self.edit(|e| e.unset_module(index, "every")),
            ModuleField::Every => {
                if let Ok(seconds) = value.parse::<u16>()
                    && seconds > 0
                {
                    self.edit(|e| e.set_module(index, "every", i64::from(seconds)));
                }
            }
            ModuleField::Name => {
                let old = module.name.clone();
                let new = (!value.is_empty()).then(|| value.to_owned());
                if old == new {
                    return;
                }
                // It keeps its spot under the new name; one leaving `modules` stands where they are.
                let bar = &self.config.bar;
                let now = [&bar.left, &bar.center, &bar.right]
                    .map(|side| side.iter().map(|p| p.name().to_owned()).collect::<Vec<_>>());
                let sides = now.clone().map(|side| {
                    let mut names = Vec::new();
                    for name in side {
                        if old.as_ref() == Some(&name) {
                            names.extend(new.clone());
                            continue;
                        }
                        if old.is_none() && name == "modules" {
                            names.extend(new.clone());
                        }
                        names.push(name);
                    }
                    names
                });
                let placed = sides != now;
                self.edit(|e| {
                    match &new {
                        Some(name) => e.set_module(index, "name", name.as_str()),
                        None => e.unset_module(index, "name"),
                    }
                    if placed {
                        let [left, center, right] = &sides;
                        e.set_list("bar", "left", left);
                        e.set_list("bar", "center", center);
                        e.set_list("bar", "right", right);
                    }
                });
            }
        }
    }

    pub(super) fn pinned_apps(&self, palette: Palette) -> Vec<Element<'_, Message>> {
        let mut rows: Vec<Element<'_, Message>> = self
            .config
            .pinned
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let name = apps::find(&self.apps, id).map_or(id.as_str(), |app| app.name.as_str());
                row![
                    crate::windows::picture(&self.apps, id, 20.0, palette).map(|_| Message::Noop),
                    text(name).size(13).width(Fill),
                    button(icon(Icon::X, 14.0, style::faint()))
                        .padding(6)
                        .style(style::ghost(8.0))
                        .on_press(Message::UnpinApp(index)),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding([8, 16])
                .into()
            })
            .collect();
        let query = self.pin_query.trim();
        let mut found = Row::new().spacing(6);
        if !query.is_empty() {
            let mut matches: Vec<(u32, &App)> = self
                .apps
                .iter()
                .filter(|app| !self.config.pinned.contains(&app.id))
                .filter_map(|app| apps::score(query, app).map(|score| (score, app)))
                .collect();
            matches.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
            for (_, app) in matches.into_iter().take(4) {
                found = found.push(
                    button(
                        row![
                            crate::windows::picture(&self.apps, &app.id, 16.0, palette)
                                .map(|_| Message::Noop),
                            text(app.name.as_str()).size(12),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    )
                    .padding([6, 10])
                    .style(style::secondary)
                    .on_press(Message::PinApp(app.id.clone())),
                );
            }
        }
        rows.push(
            column![
                text_input(crate::i18n::t("Find an app to pin"), &self.pin_query)
                    .on_input(Message::PinQuery)
                    .padding([8, 12])
                    .size(13)
                    .style(style::field(palette)),
                found,
            ]
            .spacing(10)
            .padding([12, 16])
            .into(),
        );
        rows.push(note(
            "Pinned apps sit before the open windows. A click jumps to the app or starts it; right-click any icon on the bar to pin or unpin it.",
        ));
        rows
    }

    // The bar's pieces and the quick settings button's icons, arranged by dragging.
    // What the bar shows where and what's off it, and the quick settings button's icons.
    pub(super) fn boards(&self) -> (board::Board, board::Board) {
        let bar = &self.config.bar;
        let placed: Vec<String> = [&bar.left, &bar.center, &bar.right]
            .iter()
            .flat_map(|zone| zone.iter().map(|p| p.name().to_owned()))
            .collect();
        let named = self.config.modules.iter().filter_map(|m| m.name.clone());
        let spare = Piece::NAMED
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .chain(named)
            .filter(|name| !placed.contains(name))
            .collect();
        let zone = |pieces: &[Piece]| pieces.iter().map(|p| p.name().to_owned()).collect();
        let pieces = board::Board {
            zones: vec![zone(&bar.left), zone(&bar.center), zone(&bar.right)],
            spare,
        };
        let tray = board::Board {
            zones: vec![bar.tray.iter().map(|i| i.name().to_owned()).collect()],
            spare: TrayItem::ALL
                .iter()
                .filter(|(_, item)| !bar.tray.contains(item))
                .map(|(name, _)| (*name).to_owned())
                .collect(),
        };
        (pieces, tray)
    }

    pub(super) fn layout_rows(&self, palette: Palette) -> Vec<Element<'_, Message>> {
        let (pieces, tray) = self.boards();
        vec![
            labelled(
                "What the bar shows, and where",
                board::view(
                    pieces,
                    &["Left", "Center", "Right"],
                    piece_label,
                    palette,
                    self.bar_held.clone(),
                    Message::BarHold,
                    Message::BoardMove,
                    Message::BarDrop,
                ),
            ),
            labelled(
                "The quick settings button",
                board::view(
                    tray,
                    &["Shown"],
                    tray_label,
                    palette,
                    self.tray_held.clone(),
                    Message::TrayHold,
                    Message::BoardMove,
                    Message::TrayDrop,
                ),
            ),
            note(
                "Drag a piece to move it along the bar or between its sides, and below the bar to take it off. Mode, media, notifications and recording show only when there's something to show.",
            ),
        ]
    }

    pub(super) fn bar(&self) -> Element<'_, Message> {
        let look = &self.config.bar;
        let palette = self.palette();
        let status = &self.sample;
        let tall = if look.position.vertical() {
            300.0
        } else {
            96.0
        };
        let preview = self.preview(&self.config.desktop.wallpaper, tall, {
            let bar = bar::view(bar::Bar {
                mode: None,
                has_modes: !self.config.modes.is_empty(),
                palette,
                active: status.active,
                status: status.clone(),
                battery: status.battery,
                track: None,
                controls: &self.controls,
                held: 2,
                do_not_disturb: self.config.alerts.do_not_disturb,
                awake: false,
                apps: &self.apps,
                now: chrono::Local::now(),
                look,
                clock: &self.config.clock,
                radius: style::bar_size(&self.config.style).1,
                panel_open: false,
                pinned: self
                    .config
                    .pinned
                    .iter()
                    .map(|id| bar::Pinned {
                        id: id.clone(),
                        running: false,
                        focused: false,
                    })
                    .collect(),
                dragging: None,
                tray: &[],
                icons: None,
                modules: Vec::new(),
                recording: None,
                timers: Vec::new(),
                listening: &[],
                updates: None,
                casting: false,
                next: None,
                stats: crate::stats::Stats::default(),
            })
            .map(|_| Message::Noop);
            let inset = if look.floating { 10.0 } else { 0.0 };
            container(bar)
                .padding(inset)
                .width(Fill)
                .height(Fill)
                .align_x(if look.position == Edge::Right {
                    Alignment::End
                } else {
                    Alignment::Start
                })
                .align_y(if look.position == Edge::Bottom {
                    Alignment::End
                } else {
                    Alignment::Start
                })
                .into()
        });
        column![
            preview,
            group("Layout", self.layout_rows(palette)),
            self.quick_settings(),
            group(
                "Place and look",
                vec![
                    setting(
                        "Edge",
                        "",
                        segmented(
                            [
                                (Edge::Top, "Top", None),
                                (Edge::Bottom, "Bottom", None),
                                (Edge::Left, "Left", None),
                                (Edge::Right, "Right", None),
                            ],
                            look.position,
                            Message::BarEdge,
                        ),
                    ),
                    setting(
                        "Shape",
                        "One strip, a pill for each side and the middle, or all of it in one pill",
                        segmented(
                            [
                                (apolline_config::Shape::Bar, "Bar", None),
                                (apolline_config::Shape::Islands, "Islands", None),
                                (apolline_config::Shape::Pill, "Pill", None),
                            ],
                            look.shape,
                            Message::BarShape,
                        ),
                    ),
                    setting(
                        "Style",
                        "An island with a gap around it, or flush with the screen edge",
                        segmented(
                            [(true, "Floating", None), (false, "Edge to edge", None)],
                            look.floating,
                            Message::BarFloating,
                        ),
                    ),
                    setting(
                        "Hide it",
                        "It comes out when the pointer reaches the screen edge",
                        switch(self.config.bar.autohide, Message::BarAutohide, palette),
                    ),
                    setting(
                        "Frosted",
                        "Blur what's behind the bar, like frosted glass",
                        switch(look.blur, Message::BarBlur, palette),
                    ),
                    setting(
                        "Size",
                        "Its height, with the text and icons in it",
                        value_slider(
                            80..=150,
                            (self.config.style.bar.size.unwrap_or(1.0) * 100.0).round() as u16,
                            |v| Message::Style(Part::Bar, Knob::Size, v),
                            "%",
                            palette,
                        ),
                    ),
                    setting(
                        "Font",
                        "For what the bar says",
                        self.font_picker(Part::Bar, self.config.style.bar.font),
                    ),
                    setting(
                        "Corners",
                        "Of the island, when it floats",
                        value_slider(
                            0..=24,
                            self.config
                                .style
                                .bar
                                .radius
                                .unwrap_or(apolline_theme::metrics::RADIUS_BAR as u16),
                            |v| Message::Style(Part::Bar, Knob::Corners, v),
                            "px",
                            palette,
                        ),
                    ),
                    setting(
                        "Glass",
                        "How solid the bar is over the wallpaper",
                        value_slider(
                            0..=100,
                            (look.opacity * 100.0).round() as u16,
                            Message::BarOpacity,
                            "%",
                            palette,
                        ),
                    ),
                ]
            ),
            group("Pinned apps", self.pinned_apps(palette)),
            group("Your modules", self.module_rows(palette)),
            group(
                "Calendar",
                vec![
                    setting(
                        "Weather",
                        "A city, or latitude,longitude. Open-Meteo is asked every 30 minutes; nothing is sent until you set one",
                        row![
                            text_input(crate::i18n::t("Lisbon"),
                                self.weather_draft
                                    .as_deref()
                                    .or(self.config.weather.as_deref())
                                    .unwrap_or(""),
                            )
                            .on_input(Message::WeatherDraft)
                            .on_submit(Message::SetWeather)
                            .padding([8, 12])
                            .size(13)
                            .width(180)
                            .style(style::field(palette)),
                            button(text("Save").size(13))
                                .padding([8, 14])
                                .style(style::secondary)
                                .on_press_maybe(self.weather_draft.is_some().then_some(Message::SetWeather)),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    ),
                ]
                .into_iter()
                .chain(self.calendar_rows(palette))
                .collect()
            ),
            group(
                "Workspaces",
                vec![setting(
                    "Show workspaces as",
                    "Dots stay calm, numbers match Mod+1…9",
                    segmented(
                        [
                            (Workspaces::Dots, "Dots", None),
                            (Workspaces::Numbers, "Numbers", None)
                        ],
                        look.workspaces,
                        Message::Workspaces,
                    ),
                )]
            ),
            group(
                "The machine",
                vec![setting(
                    "Show on the bar",
                    "Read every few seconds while shown; the bar repaints only when a number changes",
                    {
                        let stats = self.config.bar.stats;
                        let chip = |name: &'static str, label: &'static str, on: bool| {
                            button(text(label).size(12))
                                .padding([6, 12])
                                .style(style::choice(on))
                                .on_press(Message::Stat(name, !on))
                        };
                        row![
                            chip("cpu", "CPU", stats.cpu),
                            chip("memory", "Memory", stats.memory),
                            chip("temperature", "Temperature", stats.temperature),
                        ]
                        .spacing(6)
                    },
                )],
            ),
            group(
                "Clock",
                vec![
                    setting(
                        "Date",
                        "Weekday, day and month next to the time",
                        switch(look.date, Message::Date, palette),
                    ),
                    setting(
                        "Seconds",
                        "",
                        switch(look.seconds, Message::Seconds, palette)
                    ),
                    setting_owned(
                        "Time",
                        Some(self.clock_sample(
                            "Now it reads {sample}; %-I:%M %p for twelve hours",
                            self.config.clock.time(look.seconds),
                        )),
                        self.clock_field("time", self.config.clock.time.as_deref(), palette),
                    ),
                    setting_owned(
                        "Date format",
                        Some(self.clock_sample(
                            "Now it reads {sample}; %A, %-d %B for a long one",
                            self.config.clock.date(),
                        )),
                        self.clock_field("date", self.config.clock.date.as_deref(), palette),
                    ),
                ]
            ),
            group(
                "Battery",
                vec![setting(
                    "Percentage",
                    "Next to the battery icon",
                    switch(look.battery_percent, Message::BatteryPercent, palette),
                )]
            ),
        ]
        .spacing(22)
        .into()
    }
}

pub(super) fn piece_label(name: &str) -> String {
    match name {
        "mode" => "Mode",
        "workspaces" => "Workspaces",
        "windows" => "Apps and windows",
        "clock" => "Clock",
        "media" => "Now playing",
        "notifications" => "Notifications",
        "recording" => "Recording",
        "updates" => "Updates",
        "stats" => "CPU and memory",
        "tray" => "Quick settings",
        "apps" => "App icons",
        "modules" => "Your modules",
        name => return name.to_owned(),
    }
    .to_owned()
}

pub(super) fn tray_label(name: &str) -> String {
    match name {
        "keyboard" => "Layout",
        "awake" => "Awake",
        "wifi" => "Wi-Fi",
        "bluetooth" => "Bluetooth",
        "volume" => "Volume",
        "battery" => "Battery",
        name => return name.to_owned(),
    }
    .to_owned()
}

impl Settings {
    fn clock_sample(&self, line: &'static str, format: &str) -> String {
        let now = chrono::Local::now();
        let sample = now.format(&crate::i18n::names(format, &now)).to_string();
        crate::i18n::fill(line, &[("sample", &sample)])
    }

    // A strftime field that keeps what's typed until Enter, and turns red while chrono can't read it.
    fn clock_field<'a>(
        &'a self,
        key: &'static str,
        set: Option<&'a str>,
        palette: Palette,
    ) -> Element<'a, Message> {
        let draft = self
            .clock_drafts
            .get(key)
            .map_or(set.unwrap_or_default(), String::as_str);
        let wrong = !apolline_config::Clock::readable(draft);
        text_input(if key == "time" { "%H:%M" } else { "%a %-d %b" }, draft)
            .on_input(move |text| Message::ClockDraft(key, text))
            .on_submit(Message::ClockSet(key))
            .padding([6, 10])
            .size(13)
            .width(150)
            .font(style::mono())
            .style(move |theme, status| {
                if wrong {
                    style::wrong_field(palette)(theme, status)
                } else {
                    style::field(palette)(theme, status)
                }
            })
            .into()
    }

    pub(super) fn set_clock(&mut self, key: &'static str) {
        let Some(draft) = self.clock_drafts.get(key).map(|d| d.trim().to_owned()) else {
            return;
        };
        if !apolline_config::Clock::readable(&draft) {
            return;
        }
        self.clock_drafts.remove(key);
        if draft.is_empty() {
            self.edit(|e| e.unset("clock", key));
        } else {
            self.edit(|e| e.set("clock", key, draft.as_str()));
        }
    }
}
