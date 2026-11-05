use super::*;

impl Shell {
    // Modes, workspaces, the panel and power; handle() passes them here.
    pub(super) fn handle_more(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Mode(mode) => {
                return self.switch(mode);
            }
            Message::Control(Command::NextMode) => {
                let next = match self.mode {
                    None if !self.config.modes.is_empty() => Some(0),
                    Some(i) if i + 1 < self.config.modes.len() => Some(i + 1),
                    _ => None,
                };
                return self.switch(next);
            }
            Message::Workspace(_, Some(id)) => {
                return niri::send(apolline_niri::Request::FocusWorkspace(id));
            }
            Message::Workspace(index, None) => self.workspace = index,
            Message::Niri(apolline_niri::Event::ScreenshotCaptured { path: Some(path) }) => {
                self.popups.retain(|(n, _)| n.id != notify::SHOT_CARD);
                self.popups.push((notify::shot_card(&path), Instant::now()));
                self.shot = Some(path);
            }
            Message::Niri(event) => {
                let switched = matches!(event, apolline_niri::Event::KeyboardLayoutSwitched { .. });
                let state = self.niri.get_or_insert_default();
                state.apply(event);
                if switched && let Some(name) = state.layout().map(str::to_owned) {
                    return self.show(Osd::Layout(name));
                }
                let state = self.niri.get_or_insert_default();
                let outputs = state.outputs();
                let mut tasks = vec![];
                if outputs != self.outputs {
                    self.outputs = outputs;
                    self.sync();
                    tasks.push(self.load_wallpaper());
                }
                tasks.push(self.triggers());
                return Task::batch(tasks);
            }
            Message::NiriReply(Err(err)) => eprintln!("apolline: niri: {err}"),
            Message::Wallpaper(name, Ok(handle)) if self.wanted_wallpapers().contains(&name) => {
                self.wallpapers.retain(|(n, _)| *n != name);
                self.wallpapers.push((name, handle));
                self.show_wallpapers();
                self.sync();
            }
            Message::Wallpaper(_, Err(err)) => eprintln!("apolline: wallpaper: {err}"),
            Message::Battery(battery) => {
                self.system.get_or_insert_default().battery = battery;
                let low = power::low(battery);
                let warn = match (low, battery) {
                    (Some(mark), Some(battery)) if self.warned.is_none_or(|w| mark < w) => {
                        self.update(Message::Notify(notify::battery_card(mark, battery)))
                    }
                    _ => Task::none(),
                };
                // Plugged in: the warning goes, and comes back on the next way down. A reading
                // that wobbles over the mark on battery doesn't count.
                let plugged = battery.is_none_or(|b| b.charging);
                if plugged && self.warned.is_some() {
                    self.forget(notify::BATTERY_CARD);
                    self.history.retain(|n| n.id != notify::BATTERY_CARD);
                }
                self.warned = match (plugged, self.warned, low) {
                    (true, _, _) => None,
                    (false, Some(warned), Some(low)) => Some(warned.min(low)),
                    (false, warned, low) => warned.or(low),
                };
                return Task::batch([warn, self.triggers()]);
            }
            Message::Media(media) => self.system.get_or_insert_default().media = media,
            Message::Boosted(previous) if self.current().is_some_and(|m| m.performance) => {
                self.boosted = previous
            }
            Message::Boosted(Some(previous)) => {
                return Task::future(profile::restore(previous)).discard();
            }
            Message::Notify(mut notification) => {
                // A new version of a card being answered keeps what's typed, if it can still be
                // answered.
                self.popups.retain(|(n, _)| n.id != notification.id);
                self.held.retain(|n| n.id != notification.id);
                if notification.reply.is_none()
                    && self
                        .replying
                        .as_ref()
                        .is_some_and(|(id, _)| *id == notification.id)
                {
                    self.replying = None;
                }
                if notification.timeout == Some(notify::SHOWN_FOR) {
                    let seconds = u64::from(self.config.alerts.seconds);
                    notification.timeout = Some(Duration::from_secs(seconds));
                }
                self.history.retain(|n| n.id != notification.id);
                let mut dropped = Vec::new();
                if notification.id != notify::CONFIG_CARD && !notification.transient {
                    self.history.insert(0, notification.clone());
                    dropped = self.history.split_off(self.history.len().min(50));
                }
                let dropped = Task::batch(
                    dropped
                        .into_iter()
                        .filter(|n| {
                            !self.popups.iter().any(|(p, _)| p.id == n.id)
                                && !self.held.iter().any(|h| h.id == n.id)
                        })
                        .map(|n| notify::closed(n.id, Closed::Expired)),
                );
                // Into the history and nowhere else; the app isn't told it was closed,
                // since some take that as read.
                if self.config.alerts.muted(&notification.app) {
                    // Kept nowhere, so the app hears it's gone.
                    if notification.transient {
                        return Task::batch([
                            dropped,
                            notify::closed(notification.id, Closed::Expired),
                        ]);
                    }
                    return dropped;
                }
                if self.config.alerts.always(&notification.app)
                    || notify::lets_through(self.rule(), notification.urgency)
                {
                    let ring = crate::sound::pick(&self.config.alerts.sound, &notification.sound)
                        .filter(|_| !self.config.alerts.silent(&notification.app));
                    self.popups.push((notification, Instant::now()));
                    if let Some(file) = ring {
                        return Task::batch([
                            dropped,
                            Task::future(crate::sound::play(file)).discard(),
                        ]);
                    }
                } else {
                    self.held.push(notification);
                }
                return dropped;
            }
            Message::Unnotify(id) => {
                self.forget(id);
                self.history.retain(|n| n.id != id);
                return notify::closed(id, Closed::Requested);
            }
            Message::Forget(id) => {
                self.forget(id);
                self.history.retain(|n| n.id != id);
                return notify::closed(id, Closed::Dismissed);
            }
            Message::ClearAll => {
                self.held.clear();
                let gone: Vec<u32> = self.history.drain(..).map(|n| n.id).collect();
                return Task::batch(
                    gone.into_iter()
                        .map(|id| notify::closed(id, Closed::Dismissed)),
                );
            }
            Message::KeepAwake(on) => self.awake = on,
            Message::Idle(stage, true) => return self.idle(stage),
            Message::Dismiss(id) => {
                self.forget(id);
                // Still in the history, where a click has to reach the app.
                if self.history.iter().any(|n| n.id == id) {
                    return Task::none();
                }
                return notify::closed(id, Closed::Dismissed);
            }
            Message::Activate(notify::SHOT_CARD) => {
                self.forget(notify::SHOT_CARD);
                return self.open_marks();
            }
            Message::Activate(id) => {
                let Some(notification) = self
                    .popups
                    .iter()
                    .map(|(n, _)| n)
                    .chain(&self.history)
                    .find(|n| n.id == id)
                    .cloned()
                else {
                    return Task::none();
                };
                if !notification.resident {
                    self.forget(id);
                    self.history.retain(|n| n.id != id);
                }
                let close = if self.overlay == Some(Overlay::Panel) {
                    self.set_overlay(None)
                } else {
                    Task::none()
                };
                let focus = match self.app_window(&notification) {
                    Some(window) => niri::send(apolline_niri::Request::FocusWindow(window)),
                    // From before this start, so no app hears the click: it opens instead.
                    None if notify::is_past(id) => {
                        [notification.desktop.as_deref(), Some(&notification.app)]
                            .into_iter()
                            .flatten()
                            .find_map(|name| crate::apps::find(&self.apps, name))
                            .map(|app| app.id.clone())
                            .map_or_else(Task::none, |app| self.open_pinned(&app))
                    }
                    None => Task::none(),
                };
                // A resident card stays: it hears the click and nothing closes.
                let told = match (notification.default_action(), notification.resident) {
                    (true, true) => notify::act(id, "default".into()),
                    (true, false) => notify::invoke(id, "default".into()),
                    (false, true) => Task::none(),
                    (false, false) => notify::closed(id, Closed::Dismissed),
                };
                return Task::batch([close, focus, told]);
            }
            Message::Quiet(choice) => {
                let now = Local::now();
                self.detail = None;
                self.quiet_until = match choice {
                    panel::Quiet::For(minutes) => {
                        Some(now + chrono::TimeDelta::minutes(i64::from(minutes)))
                    }
                    panel::Quiet::Morning => {
                        let eight = chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap_or_default();
                        let day = if now.time() < eight {
                            now.date_naive()
                        } else {
                            now.date_naive() + chrono::TimeDelta::days(1)
                        };
                        chrono::TimeZone::from_local_datetime(&Local, &day.and_time(eight))
                            .earliest()
                    }
                    panel::Quiet::Always | panel::Quiet::Off => None,
                };
                return self.update(Message::DoNotDisturb(choice != panel::Quiet::Off));
            }
            Message::HistoryGroup(app) => {
                if self.opened.contains(&app) {
                    self.opened.retain(|o| *o != app);
                } else {
                    self.opened.push(app);
                }
            }
            // A link from a calendar, so given to the opener as it is, never to a shell.
            Message::OpenLink(link) => {
                if !link.starts_with("https://") {
                    return Task::none();
                }
                let opener = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                let open = Task::future(async move {
                    let mut command = tokio::process::Command::new(opener);
                    command
                        .arg(link)
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null());
                    if let Ok(mut child) = command.spawn() {
                        let _ = child.wait().await;
                    }
                })
                .discard();
                return Task::batch([self.set_overlay(None), open]);
            }
            Message::ReplyOpen(id) => {
                self.replying = Some((id, String::new()));
                return iced::widget::operation::focus(popup::REPLY);
            }
            Message::ReplyRefocus if self.replying.is_some() => {
                return iced::widget::operation::is_focused(popup::REPLY).then(|focused| {
                    if focused {
                        Task::none()
                    } else {
                        iced::widget::operation::focus(popup::REPLY)
                    }
                });
            }
            Message::ReplyUnfocused => {
                if self
                    .replying
                    .as_ref()
                    .is_some_and(|(_, typed)| typed.trim().is_empty())
                {
                    self.replying = None;
                }
            }
            Message::ReplyTyped(typed) => {
                if let Some((_, text)) = &mut self.replying {
                    *text = typed;
                }
            }
            Message::ReplySend => {
                if let Some((id, typed)) = self.replying.take()
                    && !typed.trim().is_empty()
                {
                    self.forget(id);
                    self.history.retain(|n| n.id != id);
                    return notify::replied(id, typed);
                }
            }
            // A calendar reminder's Join button carries its link as the key.
            Message::Invoke(id, key) if notify::is_reminder(id) => {
                self.forget(id);
                self.history.retain(|n| n.id != id);
                return self.update(Message::OpenLink(key));
            }
            Message::Invoke(id, key) => {
                let resident = self.popups.iter().any(|(n, _)| n.id == id && n.resident);
                if resident {
                    return notify::act(id, key);
                }
                self.forget(id);
                self.history.retain(|n| n.id != id);
                return notify::invoke(id, key);
            }
            Message::MarksOpened(Ok(marks)) => {
                // Never over a password prompt, nor over marks not saved yet.
                if self.overlay == Some(Overlay::Polkit) {
                    return Task::none();
                }
                if self.marks.as_ref().is_some_and(|m| !m.is_empty()) {
                    return self.show(Osd::Note("Save or close these marks first".into()));
                }
                self.marks = Some(std::sync::Arc::unwrap_or_clone(marks));
                return self.set_overlay(Some(Overlay::Marks));
            }
            Message::MarksOpened(Err(err)) => return self.show(Osd::Note(err)),
            Message::Mark(event) => return self.mark(event),
            Message::MarksDone(Ok(said) | Err(said)) => return self.show(Osd::Note(said)),
            Message::CloseMarks => return self.set_overlay(None),
            Message::Apps(apps) => {
                let apps = if apps.is_empty() && self.demo {
                    apps::sample()
                } else {
                    apps
                };
                self.apps = apps.clone();
                self.launcher.set_apps(apps);
            }
            Message::Query(query) => return self.search(query),
            Message::Submit => {
                if let Some(target) = self.launcher.chosen() {
                    return self.run(target);
                }
            }
            Message::Run(target) => return self.run(target),
            Message::Controls(controls) => {
                if self
                    .touched
                    .is_none_or(|at| at.elapsed() > QUIET_AFTER_TOUCH)
                {
                    self.controls = Controls {
                        wifi: self.controls.wifi.take(),
                        bluetooth: self.controls.bluetooth.take(),
                        ..controls
                    };
                }
            }
            Message::Profiles(profiles) => self.profiles = profiles,
            Message::PickProfile(profile) => {
                if let Some(profiles) = &mut self.profiles {
                    let previous = std::mem::replace(&mut profiles.active, profile.clone());
                    if !self.sample() {
                        return Task::perform(crate::profile::choose(profile), move |result| {
                            Message::ProfileSet(previous, result)
                        });
                    }
                }
            }
            Message::ProfileSet(_, Ok(())) => {}
            Message::ProfileSet(previous, Err(err)) => {
                if let Some(profiles) = &mut self.profiles {
                    profiles.active = previous;
                }
                return self.show(Osd::Note(crate::i18n::fill(
                    "Power: {err}",
                    &[("err", &err)],
                )));
            }
            Message::Volume(volume) => {
                if self
                    .touched
                    .is_none_or(|at| at.elapsed() > QUIET_AFTER_TOUCH)
                {
                    self.controls.volume = volume;
                }
            }
            Message::Change(change) => {
                self.controls.apply(&change);
                self.touched = Some(Instant::now());
                if !self.sample() {
                    return match change {
                        Change::Wifi(on) => Task::perform(crate::wifi::power(on), move |result| {
                            Message::Switched(change, result)
                        }),
                        Change::Bluetooth(on) => {
                            Task::perform(crate::bluetooth::power(on), move |result| {
                                Message::Switched(change, result)
                            })
                        }
                        change => Task::future(controls::change(change)).discard(),
                    };
                }
            }
            Message::Player(action) => return self.player(action),
            Message::DoNotDisturb(on) => {
                if !on {
                    self.quiet_until = None;
                    self.quiet_off_until = self.quiet_hours_end();
                }
                self.plain.alerts.do_not_disturb = on;
                self.config.alerts.do_not_disturb = on;
                if !self.demo {
                    self.save(|editor| editor.set("notifications", "do_not_disturb", on));
                }
                self.release_held();
            }
            Message::Power(power) => {
                if power.needs_second_click() && self.armed != Some(power) {
                    self.armed = Some(power);
                    return Task::none();
                }
                let close = self.set_overlay(None);
                return Task::batch([close, self.spawn(&power.command())]);
            }
            Message::PowerMenu(open) => {
                self.power_menu = open;
                self.armed = None;
            }
            Message::OpenSettings => {
                let close = self.set_overlay(None);
                return Task::batch([close, self.spawn(&format!("{} settings", me()))]);
            }
            Message::Screenshot => {
                let close = self.set_overlay(None);
                return Task::batch([close, self.spawn("niri msg action screenshot")]);
            }
            Message::LockNow => return self.lock(),
            Message::Sleeping if self.config.lock.before_sleep => return self.lock(),
            _ => {}
        }
        Task::none()
    }
}
