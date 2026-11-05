use super::*;
use iced::widget::column;

impl Settings {
    pub(super) fn notifications(&self) -> Element<'_, Message> {
        let alerts = &self.config.alerts;
        let palette = self.palette();
        let test_state: Element<'_, Message> = match &self.tested {
            Some(Ok(())) => text(crate::i18n::fill(
                "Sent — look {where}",
                &[("where", &crate::i18n::t(where_cards(alerts.position)))],
            ))
            .size(12)
            .color(style::color(style::GOOD))
            .into(),
            Some(Err(err)) => text(crate::i18n::fill(
                "Nothing shows them yet: {err}",
                &[("err", &err)],
            ))
            .size(12)
            .color(style::color(style::WARN))
            .into(),
            None => space().into(),
        };
        let mut per_mode: Vec<Element<'_, Message>> = self
            .config
            .modes
            .iter()
            .enumerate()
            .map(|(index, mode)| {
                setting_with_icon(
                    Icon::named(&mode.icon),
                    mode.name.as_str(),
                    "",
                    rules(index, mode.notifications),
                )
            })
            .collect();
        if per_mode.is_empty() {
            per_mode.push(note(
                "Without a mode every notification shows. Each mode can hold some back.",
            ));
        }
        column![
            group(
                "Everywhere",
                vec![
                    setting(
                        "Do not disturb",
                        "Holds everything but urgent ones, in every mode",
                        switch(alerts.do_not_disturb, Message::DoNotDisturb, palette),
                    ),
                    setting(
                        "Where they show up",
                        "Newest nearest the edge",
                        corner_picker(alerts.position, palette),
                    ),
                    setting(
                        "Popups stay for",
                        "Urgent ones stay until you close them",
                        value_slider(2..=30, alerts.seconds, Message::PopupSeconds, "s", palette),
                    ),
                    setting(
                        "Cards at once",
                        "Newer ones push the oldest out; the one you're answering stays",
                        value_slider(1..=8, alerts.cards, Message::Cards, "", palette),
                    ),
                    setting(
                        "Card width",
                        "Wider cards show more of what was said",
                        value_slider(280..=560, alerts.width, Message::CardWidth, "pt", palette),
                    ),
                    self.sound_row(palette),
                    self.quiet_row(palette),
                    setting(
                        "Try it",
                        "Sends a notification to whatever shows them",
                        row![
                            test_state,
                            button(
                                row![
                                    icon(Icon::Send, 14.0, neutral::text()),
                                    text("Send a test").size(13)
                                ]
                                .spacing(8)
                                .align_y(Alignment::Center)
                            )
                            .padding([8, 14])
                            .style(style::secondary)
                            .on_press(Message::Test)
                        ]
                        .spacing(12)
                        .align_y(Alignment::Center),
                    ),
                ]
            ),
            group("In each mode", per_mode),
            group(
                "Apps",
                vec![
                    self.app_list("Never pop up", "Kept in the history only", "mute", &alerts.mute, "App name", palette),
                    self.app_list("Always show", "Even with do not disturb or a mode that holds them", "always", &alerts.always, "App name", palette),
                    note("Use the name an app signs its notifications with, as the history in quick settings shows it."),
                ]
            ),
        ]
        .spacing(22)
        .into()
    }

    // Each calendar with what the bar last heard from it, then ways to add one.
    pub(super) fn calendar_rows(&self, palette: Palette) -> Vec<Element<'_, Message>> {
        let mut rows: Vec<Element<'_, Message>> = self
            .config
            .calendars
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let heard = heard_line(self.heard.get(&events::key(source)));
                row![
                    column![
                        text(events::name(source)).size(13),
                        text(heard).size(11).color(style::color(style::faint())),
                    ]
                    .spacing(3)
                    .width(Fill),
                    button(icon(Icon::X, 14.0, style::faint()))
                        .padding(6)
                        .style(style::ghost(8.0))
                        .on_press(Message::RemoveCalendar(index)),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding([10, 16])
                .into()
            })
            .collect();
        let link = self.calendar_draft.trim();
        rows.push(
            row![
                text_input(
                    crate::i18n::t("~/calendar.ics, or an https or webcal link"),
                    &self.calendar_draft
                )
                .on_input(Message::CalendarDraft)
                .on_submit(Message::AddCalendar)
                .padding([8, 12])
                .size(13)
                .style(style::field(palette)),
                button(text("Add").size(13))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press_maybe((!link.is_empty()).then_some(Message::AddCalendar)),
            ]
            .spacing(8)
            .padding([12, 16])
            .into(),
        );
        rows.push(self.caldav_form(palette));
        match (&self.keyring, &self.caldav.error) {
            (Some(Err(error)), _) | (_, Some(error)) => rows.push(warning(error)),
            _ => rows.push(note(
                "Files and links are read every ten minutes, with no sign-in. A CalDAV password goes to your keyring; this file keeps the address and user name.",
            )),
        }
        rows
    }

    pub(super) fn caldav_form(&self, palette: Palette) -> Element<'_, Message> {
        let form = &self.caldav;
        let field = |hint, value| {
            text_input(hint, value)
                .padding([8, 12])
                .size(13)
                .style(style::field(palette))
        };
        let ready = !form.checking
            && !matches!(self.keyring, Some(Err(_)))
            && !form.url.trim().is_empty()
            && !form.user.trim().is_empty()
            && !form.password.is_empty();
        column![
            text("CalDAV").size(14).font(style::medium()),
            text("Nextcloud, Fastmail, Radicale and the like, by the calendar's own address")
                .size(12)
                .color(style::color(style::faint())),
            field(
                "https://cloud.example.org/remote.php/dav/calendars/me/personal/",
                &form.url
            )
            .on_input(Message::CalDavUrl),
            row![
                field("User name", &form.user).on_input(Message::CalDavUser),
                field("Password", &form.password)
                    .secure(true)
                    .on_input(Message::CalDavPassword)
                    .on_submit(Message::AddCalDav),
                button(
                    text(if form.checking {
                        "Signing in…"
                    } else {
                        "Add"
                    })
                    .size(13)
                )
                .padding([8, 14])
                .style(style::secondary)
                .on_press_maybe(ready.then_some(Message::AddCalDav)),
            ]
            .spacing(8),
        ]
        .spacing(8)
        .padding([12, 16])
        .into()
    }

    pub(super) fn app_list<'a>(
        &'a self,
        title: &'a str,
        about: &'a str,
        key: &'static str,
        apps: &'a [String],
        hint: &'static str,
        palette: Palette,
    ) -> Element<'a, Message> {
        let mut chips = Row::new().spacing(6).align_y(Alignment::Center);
        for (index, app) in apps.iter().enumerate() {
            chips = chips.push(
                container(
                    row![
                        text(app.as_str()).size(12),
                        button(icon(Icon::X, 12.0, style::faint()))
                            .padding(2)
                            .style(style::ghost(6.0))
                            .on_press(Message::AppListRemove(key, index)),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([4, 10])
                .style(style::pill(Rgba::hex(0xffffff).alpha(0.06))),
            );
        }
        let draft = if self.app_draft.0 == key {
            self.app_draft.1.as_str()
        } else {
            ""
        };
        chips = chips.push(
            text_input(hint, draft)
                .on_input(move |name| Message::AppDraft(key, name))
                .on_submit(Message::AppListAdd(key))
                .padding([6, 10])
                .size(12)
                .width(150)
                .style(style::field(palette)),
        );
        setting(title, about, chips.wrap())
    }
}

// "Read at 14:05", or what went wrong, in a sentence.
fn heard_line(heard: Option<&Heard>) -> String {
    let at = |seconds: i64| {
        let at = chrono::DateTime::from_timestamp(seconds, 0)
            .unwrap_or_default()
            .with_timezone(&chrono::Local);
        let today = at.date_naive() == chrono::Local::now().date_naive();
        at.format(if today { "%H:%M" } else { "%-d %b, %H:%M" })
    };
    match heard.map(|heard| (heard.read, heard.error.as_deref())) {
        Some((Some(read), None)) => crate::i18n::fill("Read at {time}", &[("time", &at(read))]),
        Some((Some(read), Some(error))) => {
            format!("{error}. Still showing what it said at {}", at(read))
        }
        Some((None, Some(error))) => error.to_owned(),
        _ => "Not read yet".into(),
    }
}

impl Settings {
    fn sound_row(&self, palette: Palette) -> Element<'_, Message> {
        use apolline_config::Sound;
        let kind = match &self.config.alerts.sound {
            Sound::Off => "off",
            Sound::Theme => "theme",
            Sound::File(_) => "file",
        };
        let mut control = column![segmented(
            [
                ("off", "Off", None),
                ("theme", "The theme's", None),
                ("file", "Your file", None),
            ],
            kind,
            Message::SoundKind,
        )]
        .spacing(8)
        .align_x(Alignment::End);
        if kind == "file" || self.sound_draft.is_some() {
            let set = match &self.config.alerts.sound {
                Sound::File(path) => path.as_str(),
                _ => "",
            };
            control = control.push(
                text_input(
                    "~/Music/ding.oga",
                    self.sound_draft.as_deref().unwrap_or(set),
                )
                .on_input(Message::SoundDraft)
                .on_submit(Message::SoundSet)
                .padding([6, 10])
                .size(13)
                .width(260)
                .style(style::field(palette)),
            );
        }
        setting(
            "Sound",
            "When a card shows; apps that play their own stay quiet here",
            control,
        )
    }

    pub(super) fn set_sound_kind(&mut self, kind: &'static str) {
        match kind {
            "off" => {
                self.sound_draft = None;
                self.edit(|e| e.unset("notifications", "sound"));
            }
            "theme" => {
                self.sound_draft = None;
                self.edit(|e| e.set("notifications", "sound", true));
            }
            _ => self.sound_draft = Some(String::new()),
        }
    }

    pub(super) fn set_sound_file(&mut self) {
        let Some(draft) = self.sound_draft.as_ref().map(|d| d.trim().to_owned()) else {
            return;
        };
        if draft.is_empty() {
            return;
        }
        self.sound_draft = None;
        self.edit(|e| e.set("notifications", "sound", draft.as_str()));
    }
}

impl Settings {
    fn quiet_row(&self, palette: Palette) -> Element<'_, Message> {
        let set = self
            .config
            .alerts
            .quiet_hours
            .map(|(from, until)| apolline_config::Trigger::Hours { from, until }.text())
            .unwrap_or_default();
        let draft = self.quiet_draft.as_deref().unwrap_or(&set);
        let wrong = !draft.trim().is_empty()
            && !matches!(
                apolline_config::Trigger::parse(draft),
                Some(apolline_config::Trigger::Hours { .. })
            );
        setting(
            "Quiet hours",
            "Do not disturb on its own, every day; turning it off in quick settings lasts until they end",
            text_input("23:00-07:00", draft)
                .on_input(Message::QuietDraft)
                .on_submit(Message::QuietSet)
                .padding([6, 10])
                .size(13)
                .width(140)
                .font(style::mono())
                .style(move |theme, status| {
                    if wrong {
                        style::wrong_field(palette)(theme, status)
                    } else {
                        style::field(palette)(theme, status)
                    }
                }),
        )
    }

    pub(super) fn set_quiet(&mut self) {
        let Some(draft) = self.quiet_draft.as_ref().map(|d| d.trim().to_owned()) else {
            return;
        };
        if draft.is_empty() {
            self.quiet_draft = None;
            self.edit(|e| e.unset("notifications", "quiet_hours"));
        } else if let Some(hours @ apolline_config::Trigger::Hours { .. }) =
            apolline_config::Trigger::parse(&draft)
        {
            self.quiet_draft = None;
            self.edit(|e| e.set("notifications", "quiet_hours", hours.text()));
        }
    }
}
