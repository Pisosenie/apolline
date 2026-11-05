use super::*;

impl Shell {
    pub(super) fn rule(&self) -> Notifications {
        if self.quiet() {
            Notifications::Hold
        } else {
            self.config.notifications(self.mode)
        }
    }

    // Do not disturb, turned on by hand or by the quiet hours.
    pub(super) fn quiet(&self) -> bool {
        self.config.alerts.do_not_disturb || self.quiet_hours_end().is_some()
    }

    // When the quiet hours we're in end; none outside them, or once turned off for tonight.
    pub(super) fn quiet_hours_end(&self) -> Option<chrono::DateTime<chrono::Local>> {
        use chrono::Timelike;
        let (from, until) = self.config.alerts.quiet_hours?;
        let minute = (self.now.hour() * 60 + self.now.minute()) as u16;
        let end = self.now + chrono::TimeDelta::minutes(minutes_left(from, until, minute)?);
        let end = end.with_second(0).unwrap_or(end);
        (self.quiet_off_until.is_none_or(|off| off < end)).then_some(end)
    }

    // The window of the app a notification came from, to bring forward on a click.
    pub(super) fn app_window(&self, notification: &notify::Notification) -> Option<u64> {
        let state = self.niri.as_ref()?;
        let names = [
            notification.desktop.as_deref(),
            Some(notification.app.as_str()),
        ];
        state
            .recent()
            .into_iter()
            .find(|window| {
                let Some(app) = window.app_id.as_deref() else {
                    return false;
                };
                names.iter().flatten().any(|name| {
                    !name.is_empty()
                        && (app.eq_ignore_ascii_case(name)
                            || app
                                .rsplit('.')
                                .next()
                                .is_some_and(|last| last.eq_ignore_ascii_case(name)))
                })
            })
            .map(|window| window.id)
    }

    pub(super) fn remind(&mut self) -> Task<Message> {
        let now = self.now.naive_local();
        self.reminded
            .retain(|(begins, _, _)| *begins > now - chrono::TimeDelta::hours(1));
        let due: Vec<(crate::events::Occurrence, chrono::TimeDelta)> = self
            .agenda
            .due(now)
            .filter(|(o, lead)| {
                !self.reminded.iter().any(|(begins, what, at)| {
                    Some(*begins) == o.begins() && *what == o.summary && at == lead
                })
            })
            .map(|(o, lead)| (o.clone(), lead))
            .collect();
        let mut cards = Vec::new();
        for (occurrence, lead) in due {
            self.reminded.push((
                occurrence.begins().unwrap_or(now),
                occurrence.summary.clone(),
                lead,
            ));
            self.reminders = self.reminders.wrapping_add(1);
            let card = notify::reminder_card(&occurrence, now, self.reminders);
            cards.push(self.update(Message::Notify(card)));
        }
        Task::batch(cards)
    }

    pub(super) fn forget(&mut self, id: u32) {
        self.popups.retain(|(n, _)| n.id != id);
        self.held.retain(|n| n.id != id);
        self.replying.take_if(|(replying, _)| *replying == id);
    }

    // The card being answered needs the keyboard until the answer is sent or dropped.
    #[cfg(target_os = "linux")]
    pub fn replying(&self) -> bool {
        self.replying.is_some()
    }

    pub(super) fn expire(&mut self) -> Task<Message> {
        let now = Instant::now();
        let answering = self.replying.as_ref().map(|(id, _)| *id);
        let (gone, live): (Vec<_>, Vec<_>) = std::mem::take(&mut self.popups)
            .into_iter()
            .partition(|(n, shown)| {
                answering != Some(n.id)
                    && n.timeout.is_some_and(|t| now.duration_since(*shown) >= t)
            });
        self.popups = live;
        // A card that slips into the history isn't closed for the app: a click there
        // still has to reach it. Only what isn't kept is.
        Task::batch(
            gone.into_iter()
                .filter(|(n, _)| !self.history.iter().any(|kept| kept.id == n.id))
                .map(|(n, _)| notify::closed(n.id, Closed::Expired)),
        )
    }

    #[cfg(target_os = "linux")]
    pub fn popup_height(&self) -> u32 {
        popup::height(&self.shown_cards())
    }

    #[cfg(target_os = "linux")]
    pub fn answerable(&self) -> bool {
        self.shown_cards().iter().any(|n| n.reply.is_some())
    }

    pub(super) fn shown_cards(&self) -> Vec<&notify::Notification> {
        popup::shown(
            self.popups.iter().rev().map(|(n, _)| n),
            self.replying.as_ref().map(|(id, _)| *id),
            usize::from(self.config.alerts.cards),
        )
    }
}

// Minutes until quiet hours from..until end, when `minute` of the day falls in them.
fn minutes_left(from: u16, until: u16, minute: u16) -> Option<i64> {
    apolline_config::Trigger::covers(from, until, minute)
        .then(|| (i64::from(until) - i64::from(minute)).rem_euclid(24 * 60))
}

#[cfg(test)]
mod tests {
    use super::minutes_left;

    #[test]
    fn quiet_hours_run_past_midnight_and_end_on_time() {
        let (eleven, seven) = (23 * 60, 7 * 60);
        assert_eq!(minutes_left(eleven, seven, 23 * 60 + 30), Some(7 * 60 + 30));
        assert_eq!(minutes_left(eleven, seven, 6 * 60), Some(60));
        assert_eq!(
            minutes_left(eleven, seven, seven),
            None,
            "over at seven sharp"
        );
        assert_eq!(minutes_left(eleven, seven, 12 * 60), None);
        assert_eq!(minutes_left(13 * 60, 14 * 60, 13 * 60 + 15), Some(45));
    }
}
