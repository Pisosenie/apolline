use super::*;

impl Shell {
    pub(super) fn rule(&self) -> Notifications {
        if self.config.alerts.do_not_disturb {
            Notifications::Hold
        } else {
            self.config.notifications(self.mode)
        }
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
        )
    }
}
