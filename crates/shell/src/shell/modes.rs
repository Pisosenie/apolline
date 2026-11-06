use super::*;

impl Shell {
    pub(super) fn current(&self) -> Option<&ModeSettings> {
        self.mode.and_then(|i| self.config.modes.get(i))
    }

    pub(super) fn mode_names(&self) -> Vec<(String, String)> {
        self.config
            .modes
            .iter()
            .map(|m| (m.name.clone(), m.icon.clone()))
            .collect()
    }

    pub(super) fn night_scheduled(&self) -> bool {
        use chrono::Timelike;
        let now = (self.now.hour() * 60 + self.now.minute()) as u16;
        self.config
            .night
            .hours
            .is_some_and(|(from, until)| Trigger::covers(from, until, now))
    }

    pub(super) fn night_light(&self) -> Option<u32> {
        let on = self.night.1.unwrap_or(self.night.0);
        (on && !self.demo).then_some(self.config.night.temperature)
    }

    pub(super) fn triggers(&mut self) -> Task<Message> {
        if self.demo {
            return Task::none();
        }
        let game = self.niri.as_ref().is_some_and(|state| {
            state.recent().iter().any(|w| {
                w.app_id.as_deref().is_some_and(|app| {
                    app.starts_with("steam_app_") || app == "gamescope" || app.ends_with(".exe")
                })
            })
        });
        let battery = self
            .system
            .as_ref()
            .and_then(|s| s.battery)
            .is_some_and(|b| !b.charging);
        let now = {
            use chrono::Timelike;
            (self.now.hour() * 60 + self.now.minute()) as u16
        };
        let why: Vec<Option<Trigger>> = self
            .config
            .modes
            .iter()
            .map(|mode| {
                mode.when
                    .iter()
                    .find(|trigger| match trigger {
                        Trigger::Game => game,
                        Trigger::Battery => battery,
                        Trigger::Hours { from, until } => Trigger::covers(*from, *until, now),
                        Trigger::Monitor(name) => self.outputs.contains(name),
                    })
                    .cloned()
            })
            .collect();
        let keys: Vec<String> = self.config.modes.iter().map(ModeSettings::key).collect();
        let active: Vec<String> = keys
            .iter()
            .zip(&why)
            .filter(|(_, why)| why.is_some())
            .map(|(key, _)| key.clone())
            .collect();
        let current = self.current().map(ModeSettings::key);
        let plan = plan(&active, &self.fired, current.as_deref(), self.auto.as_ref());
        let mut task = Task::none();
        if let Some(back) = plan.back {
            task = self.enter(back.and_then(|key| self.config.find(&key)));
        }
        if let Some(index) = plan.start.as_deref().and_then(|key| self.config.find(key)) {
            task = Task::batch([task, self.enter(Some(index))]);
            self.reason = why[index].clone();
        }
        self.auto = plan.auto;
        self.fired = active;
        task
    }

    pub(super) fn switch(&mut self, mode: Option<usize>) -> Task<Message> {
        let close = self.set_overlay(None);
        Task::batch([close, self.enter(mode)])
    }

    pub(super) fn enter(&mut self, mode: Option<usize>) -> Task<Message> {
        let fast = |mode: Option<usize>| {
            mode.and_then(|i| self.config.modes.get(i))
                .is_some_and(|m| m.performance)
        };
        let profile = match (fast(self.mode), fast(mode)) {
            (false, true) => Task::perform(profile::boost(), Message::Boosted),
            (true, false) => match self.boosted.take() {
                Some(previous) => Task::future(profile::restore(previous)).discard(),
                None => Task::none(),
            },
            _ => Task::none(),
        };
        self.mode = mode;
        self.dress();
        self.sync();
        self.release_held();
        let note = self.keep_replay();
        Task::batch([self.load_wallpaper(), self.open_apps(), profile, note])
    }

    // Keeps the last moments while the mode wants them, for as long as it says.
    pub(super) fn keep_replay(&mut self) -> Task<Message> {
        let wanted = self.current().map_or(0, |m| m.replay);
        if self.replay.as_ref().map_or(0, |r| r.seconds) == wanted {
            return Task::none();
        }
        self.replay = None;
        if wanted == 0 {
            return Task::none();
        }
        match crate::capture::Replay::start(wanted) {
            Some(Ok(replay)) => {
                self.replay = Some(replay);
                Task::none()
            }
            Some(Err(err)) => self.show(Osd::Note(err)),
            None => Task::none(),
        }
    }

    // Holds niri's keys that would leave the window in front, a game say, or gives them back.
    pub(super) fn stay(&mut self) -> Task<Message> {
        let key = self.config.shortcuts.stay.clone();
        let front = self
            .niri
            .as_ref()
            .and_then(|state| state.recent().into_iter().find(|w| w.is_focused))
            .map(|w| (w.id, w.title.clone().unwrap_or_default()));
        let note = match (self.staying.take(), front) {
            (Some(_), _) => crate::i18n::t("Workspaces and windows are yours again").to_owned(),
            (None, Some((id, title))) => {
                self.staying = Some(id);
                crate::i18n::fill(
                    "Staying in {title}; {key} lets go",
                    &[("title", &title), ("key", &key)],
                )
            }
            (None, None) => crate::i18n::t("No window to stay in").to_owned(),
        };
        self.sync();
        self.show(Osd::Note(note))
    }

    pub(super) fn save_replay(&mut self) -> Task<Message> {
        let saved = self
            .replay
            .as_mut()
            .is_some_and(|r| r.running() && r.save());
        let note = match &self.replay {
            Some(replay) if saved => crate::i18n::count(
                usize::from(replay.seconds),
                "Saved the last second to Videos/Replays",
                "Saved the last {n} seconds to Videos/Replays",
            ),
            Some(_) => {
                crate::i18n::t("gpu-screen-recorder stopped; switch the mode again").to_owned()
            }
            None => crate::i18n::t("Nothing is kept: turn on “Keep the last moments” in a mode")
                .to_owned(),
        };
        self.show(Osd::Note(note))
    }

    pub(super) fn dress(&mut self) {
        let mut config = self.plain.clone();
        let theme = self
            .mode
            .and_then(|i| self.plain.modes.get(i))
            .and_then(|m| m.theme.as_deref())
            .map(apolline_config::slug);
        if let Some(wanted) = theme
            && let Some((theme, _)) = crate::themes::all()
                .into_iter()
                .find(|(t, _)| apolline_config::slug(&t.name) == wanted)
        {
            theme.dress(&mut config);
        }
        let placed = crate::plugins::modules(&self.plugins, &config);
        config.modules.extend(placed);
        self.launcher
            .set_prefixes(crate::plugins::search::prefixes(&self.plugins));
        self.config = config;
    }

    pub(super) fn release_held(&mut self) {
        let rule = self.rule();
        let now = Instant::now();
        let (show, keep) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|n| notify::lets_through(rule, n.urgency));
        self.held = keep;
        self.popups
            .extend(show.into_iter().map(|n: Notification| (n, now)));
    }

    pub(super) fn open_apps(&self) -> Task<Message> {
        let (Some(state), Some(mode)) = (&self.niri, self.current()) else {
            return Task::none();
        };
        Task::batch(
            mode.open
                .iter()
                .filter(|command| !state.is_running(command))
                .map(|command| niri::send(apolline_niri::Request::Spawn(command.clone()))),
        )
    }
}

pub(super) struct Plan {
    back: Option<Option<String>>,
    start: Option<String>,
    auto: Option<(String, Option<String>)>,
}

pub(super) fn plan(
    active: &[String],
    fired: &[String],
    current: Option<&str>,
    auto: Option<&(String, Option<String>)>,
) -> Plan {
    let mut current = current.map(str::to_owned);
    let mut auto = auto.cloned();
    let mut back = None;
    if let Some((on, before)) = auto.clone()
        && !active.contains(&on)
    {
        auto = None;
        if current.as_ref() == Some(&on) {
            back = Some(before.clone());
            current = before;
        }
    }
    let start = active
        .iter()
        .find(|key| !fired.contains(key) && current.as_ref() != Some(*key))
        .cloned();
    if let Some(key) = &start {
        let before = match &auto {
            Some((on, before)) if current.as_ref() == Some(on) => before.clone(),
            _ => current,
        };
        auto = Some((key.clone(), before));
    }
    Plan { back, start, auto }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(list: &[&str]) -> Vec<String> {
        list.iter().map(|k| (*k).to_owned()).collect()
    }

    #[test]
    fn triggers_hand_back_what_you_chose_and_never_fire_twice() {
        let on_battery = plan(&keys(&["saver"]), &[], None, None);
        assert_eq!(on_battery.start.as_deref(), Some("saver"));
        let auto = on_battery.auto;
        assert_eq!(auto, Some(("saver".into(), None)));

        let picked_focus = plan(
            &keys(&["saver"]),
            &keys(&["saver"]),
            Some("focus"),
            auto.as_ref(),
        );
        assert!(picked_focus.start.is_none() && picked_focus.back.is_none());

        let night = plan(
            &keys(&["saver", "night"]),
            &keys(&["saver"]),
            Some("focus"),
            picked_focus.auto.as_ref(),
        );
        assert_eq!(night.start.as_deref(), Some("night"));
        assert_eq!(night.auto, Some(("night".into(), Some("focus".into()))));

        let morning = plan(
            &keys(&["saver"]),
            &keys(&["saver", "night"]),
            Some("night"),
            night.auto.as_ref(),
        );
        assert_eq!(
            morning.back,
            Some(Some("focus".into())),
            "back to the mode you picked"
        );
        assert!(morning.start.is_none(), "battery already had its turn");
        assert!(morning.auto.is_none());
    }
}
