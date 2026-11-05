use super::*;
use iced::widget::column;

impl Settings {
    pub(super) fn lock(&self) -> Element<'_, Message> {
        let lock = self.config.lock;
        let palette = self.palette();
        let preview = self.preview(&self.config.desktop.wallpaper, 220.0, {
            let pill = container(
                row![
                    container(text("D").size(10).color(style::color(palette.on_accent)))
                        .center(18)
                        .style(style::pill(palette.accent)),
                    text("Password")
                        .size(10)
                        .color(style::color(style::faint())),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .width(150)
            .padding([4, 6])
            .style(|_| container::Style {
                background: Some(Background::Color(style::color(
                    Rgba::hex(0x0c0d10).alpha(0.5),
                ))),
                border: Border {
                    color: style::color(Rgba::hex(0xffffff).alpha(0.12)),
                    width: 1.0,
                    radius: style::PILL.into(),
                },
                ..container::Style::default()
            });
            let clock = column![
                text(
                    chrono::Local::now()
                        .format(self.config.clock.time(false))
                        .to_string()
                )
                .size(68)
                .font(style::light()),
                pill
            ]
            .spacing(8);
            let mut right = column![].align_x(Alignment::End);
            if lock.show_media {
                right = right.push(
                    container(
                        row![
                            icon(Icon::Music, 12.0, palette.accent),
                            text("Night Drive").size(10)
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    )
                    .padding([6, 10])
                    .style(style::pill(Rgba::hex(0x0c0d10).alpha(0.5))),
                );
            }
            column![
                space::vertical(),
                row![clock, space::horizontal(), right].align_y(Alignment::End)
            ]
            .padding(22)
            .into()
        });
        let lock_now = button(
            row![
                icon(Icon::Lock, 14.0, palette.on_accent),
                text("Lock now").size(13)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([8, 14])
        .style(style::primary(palette))
        .on_press(Message::LockNow);
        column![
            preview,
            group(
                "When",
                vec![
                    setting(
                        "Before sleep",
                        "Locks the moment the laptop goes to sleep",
                        switch(lock.before_sleep, Message::LockBeforeSleep, palette),
                    ),
                    setting(
                        "Lock key",
                        "Change it in Shortcuts",
                        keycaps(&self.config.shortcuts.lock),
                    ),
                    setting("Try it", "Your password unlocks it", lock_now),
                ]
            ),
            group(
                "When you step away",
                vec![
                    setting(
                        "Lock after",
                        "Video players keep the screen on while they play",
                        minutes(self.config.idle.lock_after, Message::IdleLock, palette),
                    ),
                    setting(
                        "Screen off after",
                        "Any key or touch wakes it",
                        minutes(self.config.idle.screen_off_after, Message::IdleScreen, palette),
                    ),
                    setting(
                        "Sleep after",
                        "",
                        minutes(self.config.idle.suspend_after, Message::IdleSuspend, palette),
                    ),
                    note("Modes with “keep the screen awake” on skip all of this."),
                ]
            ),
            {
                let battery = self.config.idle.on_battery();
                group(
                    "On battery",
                    vec![
                        setting(
                            "Lock after",
                            "",
                            minutes(battery.lock_after, Message::BatteryLock, palette),
                        ),
                        setting(
                            "Screen off after",
                            "",
                            minutes(battery.screen_off_after, Message::BatteryScreen, palette),
                        ),
                        setting(
                            "Sleep after",
                            "",
                            minutes(battery.suspend_after, Message::BatterySuspend, palette),
                        ),
                        note("Sooner here saves the battery. Set to the same as above, a time follows it."),
                    ],
                )
            },
            group(
                "Look",
                vec![
                    setting(
                        "Clock and password",
                        "In the bottom left, or both in the middle",
                        segmented(
                            [(false, "Corner", None), (true, "Center", None)],
                            lock.centered,
                            Message::LockCentered,
                        ),
                    ),
                    setting(
                        "Blur the wallpaper",
                        "Soft behind the clock, like frosted glass",
                        switch(lock.blur, Message::LockBlur, palette),
                    ),
                ]
            ),
            self.login_screen(),
            group(
                "Shows",
                vec![
                    setting(
                        "What's playing",
                        "With a pause button",
                        switch(lock.show_media, Message::LockMedia, palette),
                    ),
                    setting(
                        "Weather",
                        "From Settings → Bar → Weather",
                        switch(lock.show_weather, Message::LockWeather, palette),
                    ),
                    setting(
                        "What's next",
                        "The next thing in your calendar",
                        switch(lock.show_next, Message::LockNext, palette),
                    ),
                    setting(
                        "Waiting notifications",
                        "Only how many; what they say stays hidden",
                        switch(lock.show_count, Message::LockCount, palette),
                    ),
                    fact(Icon::Keyboard, "The keyboard layout sits next to the password"),
                ]
            ),
            group(
                "Password",
                vec![
                    setting(
                        "Fingerprint",
                        "A finger on the reader unlocks it instead of the password. Leave it off if your login asks for a second factor.",
                        switch(lock.fingerprint, Message::LockFingerprint, palette),
                    ),
                    note(
                        "Checked by PAM, the same way you log in: /etc/pam.d/apolline when it's installed, login otherwise.",
                    ),
                ]
            ),
        ]
        .spacing(22)
        .into()
    }

    pub(super) fn login_screen(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let buttons = row![
            button(text("Preview").size(13))
                .padding([8, 14])
                .style(style::secondary)
                .on_press(Message::GreetPreview),
            button(text("Use it").size(13))
                .padding([8, 14])
                .style(style::primary(palette))
                .on_press(Message::GreetInstall),
        ]
        .spacing(8);
        let mut rows = vec![setting(
            "This look at login",
            "Your wallpaper, accent and font, and the clock where it is above, for everyone who signs in here. Asks for your password.",
            buttons,
        )];
        match &self.greet_note {
            Some(Ok(())) => rows.push(fact(
                Icon::Check,
                "The login screen has it from the next time it shows",
            )),
            Some(Err(err)) => rows.push(note(err)),
            None => rows.push(note(&format!(
                "Kept in {}; with greetd running `apolline greet --kiosk`. A font has to be installed for everyone to show there.",
                crate::greet_look::SYSTEM
            ))),
        }
        group("Login screen", rows)
    }
}
