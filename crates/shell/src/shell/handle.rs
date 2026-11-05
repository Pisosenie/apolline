use super::*;

impl Shell {
    pub(super) fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.now = Local::now();
                if self.recording.as_mut().is_some_and(|r| r.quit()) {
                    self.recording = None;
                    return self.show(Osd::Note("The recording stopped: wf-recorder quit".into()));
                }
                self.ticks += 1;
                let rung = self.ring_timers();
                let scheduled = self.night_scheduled();
                if scheduled != self.night.0 {
                    self.night = (scheduled, None);
                }
                // Do not disturb set for a while ends on its own.
                if self.quiet_until.is_some_and(|until| until <= self.now) {
                    return Task::batch([
                        rung,
                        self.update(Message::DoNotDisturb(false)),
                        self.expire(),
                    ]);
                }
                // And what the quiet hours held comes out when they end.
                let quiet = self.quiet();
                if self.was_quiet && !quiet {
                    self.release_held();
                }
                self.was_quiet = quiet;
                let triggers = self.triggers();
                let rotate = self.rotate();
                let remind = self.remind();
                self.publish();
                return Task::batch([self.expire(), triggers, rotate, remind, rung]);
            }
            Message::CalendarMonth(month) => {
                self.month = month;
                self.day = None;
            }
            Message::CalendarDay(day) => self.day = Some(day),
            Message::Stats(stats) => self.stats = stats,
            Message::Machine(machine) => self.machine = Some(machine),
            Message::Listening(apps) => self.listening = apps,
            Message::Sound(heard) => return self.hear_sound(heard),
            Message::CheckUpdates | Message::Control(Command::Updates) => {
                return Task::perform(crate::updates::check(), Message::Updates);
            }
            // A look that failed, offline say, keeps the last count rather than hiding it.
            Message::Updates(updates) => {
                if updates.is_some() {
                    self.updates = updates;
                }
            }
            Message::Upgrade => {
                let line =
                    crate::terminal::running(&self.config.terminal, &crate::updates::command());
                return self.spawn(&line);
            }
            Message::MachineSort(by_memory) => {
                self.by_memory = by_memory;
                self.ending = None;
            }
            // The first click asks, the second ends it.
            Message::EndProcess(pid) if self.ending != Some(pid) => self.ending = Some(pid),
            Message::EndProcess(pid) => {
                self.ending = None;
                if let Err(err) = crate::machine::end(pid) {
                    return self.show(Osd::Note(err));
                }
            }
            Message::Rates(Ok(rates)) => self.launcher.set_rates(rates),
            Message::Rates(Err(err)) => eprintln!("apolline: exchange rates: {err}"),
            Message::Agenda(agenda) => self.agenda = agenda,
            Message::Files(files) => self.launcher.set_files(files),
            Message::Detail(detail) => {
                self.detail = detail;
                self.leave_detail();
                self.net_note = None;
                if self.sample() {
                    self.networks = crate::wifi::sample();
                    self.gadgets = crate::bluetooth::sample();
                }
            }
            Message::Wifi(seen) => {
                self.controls.wifi = seen.radio;
                self.networks = seen.networks;
            }
            Message::Bluetooth(seen) => {
                self.controls.bluetooth = seen.radio;
                self.gadgets = seen.devices;
            }
            Message::PickNetwork(network) => {
                self.net_note = None;
                if network.asks_password() {
                    self.joining = Some((network, String::new()));
                    return iced::widget::operation::focus(panel::PASSWORD);
                }
                self.joining = None;
                self.net_busy = true;
                return Task::perform(crate::wifi::join(network.clone(), None), move |result| {
                    Message::Joined(network, result)
                });
            }
            Message::NetPassword(password) => {
                if let Some((_, typed)) = &mut self.joining {
                    *typed = password;
                }
            }
            Message::JoinNetwork => {
                if let Some((network, password)) = self.joining.take()
                    && !password.is_empty()
                {
                    // The row stays open, empty, while it joins.
                    self.joining = Some((network.clone(), String::new()));
                    self.net_busy = true;
                    self.net_note = None;
                    return Task::perform(
                        crate::wifi::join(network.clone(), Some(password)),
                        move |result| Message::Joined(network, result),
                    );
                }
            }
            Message::Joined(network, result) => {
                self.net_busy = false;
                self.joining = None;
                if let Err(failed) = result {
                    self.net_note = Some(failed.said);
                    // A typo, or a saved password that stopped working: ask for it again.
                    if failed.password {
                        self.joining = Some((network, String::new()));
                        return iced::widget::operation::focus(panel::PASSWORD);
                    }
                }
            }
            Message::ForgetNetwork(network) => {
                self.net_busy = true;
                self.net_note = None;
                return Task::perform(crate::wifi::forget(network), Message::RadioDone);
            }
            Message::Gadget(device) => {
                self.net_busy = true;
                self.net_note = None;
                return Task::perform(crate::bluetooth::toggle(device), Message::RadioDone);
            }
            Message::ForgetGadget(device) => {
                self.net_busy = true;
                self.net_note = None;
                return Task::perform(crate::bluetooth::forget(device), Message::RadioDone);
            }
            Message::BluetoothAsk(ask) => {
                let typing = matches!(
                    ask.what,
                    crate::bluetooth::Asking::Pin | crate::bluetooth::Asking::Passkey
                );
                self.asking = Some(ask);
                self.pin.clear();
                if typing {
                    return iced::widget::operation::focus(panel::PIN);
                }
            }
            Message::BluetoothDone(id) => {
                if self.asking.as_ref().is_some_and(|ask| ask.is(id)) {
                    self.asking = None;
                    self.pin.clear();
                }
            }
            Message::PinTyped(pin) => self.pin = pin,
            Message::PairReply(yes) => {
                if let Some(ask) = self.asking.take() {
                    let pin = std::mem::take(&mut self.pin);
                    return Task::future(crate::bluetooth::reply(ask, yes.then_some(pin)))
                        .discard();
                }
            }
            Message::Switched(_, Ok(())) => {}
            // It didn't take: the tile goes back and says why.
            Message::Switched(change, Err(err)) => {
                let (back, name) = match change {
                    Change::Wifi(on) => (Change::Wifi(!on), "Wi-Fi"),
                    Change::Bluetooth(on) => (Change::Bluetooth(!on), "Bluetooth"),
                    _ => return Task::none(),
                };
                self.controls.apply(&back);
                return self.show(Osd::Note(format!("{name}: {err}")));
            }
            Message::RadioDone(result) => {
                self.net_busy = false;
                self.asking = None;
                self.net_note = result.err();
            }
            Message::Weather(weather) => {
                if weather.is_some() || self.config.weather.is_none() {
                    self.weather = weather;
                }
            }
            Message::NightLight(on) => self.night.1 = Some(on),
            Message::Picked(Some(hex)) => {
                let content = clipboard::Content::Text(hex.clone());
                clipboard::copy(&content);
                clipboard::remember(&mut self.clips, Clip::new(content));
                return self.show(Osd::Note(crate::i18n::fill(
                    "Copied {hex}",
                    &[("hex", &hex)],
                )));
            }
            Message::Picked(None) => {}
            Message::StopRecording => return self.stop_recording(),
            Message::Module(index, said) => {
                self.module_text.retain(|(i, _)| *i != index);
                if !said.text.is_empty() {
                    self.module_text.push((index, said));
                    self.module_text.sort_by_key(|(i, _)| *i);
                }
            }
            Message::ModuleAct(index, act) => return self.module_act(index, act),
            Message::StartTimer(length, label)
            | Message::Control(Command::Timer(length, label)) => {
                self.timers_started += 1;
                let id = self.timers_started;
                self.timers
                    .push(crate::timers::Timer::start(id, length, label));
                self.timers.sort_by_key(|t| t.ends);
            }
            Message::StopTimer(id) => self.timers.retain(|t| t.id != id),
            Message::Control(Command::StopTimers) => self.timers.clear(),
            Message::ModuleScroll(index, y) => {
                let act = match y {
                    y if y > 0.0 => crate::modules::Act::Up,
                    y if y < 0.0 => crate::modules::Act::Down,
                    _ => return Task::none(),
                };
                let now = std::time::Instant::now();
                let soon = self.module_scrolled.is_some_and(|(i, at)| {
                    i == index && now.duration_since(at) < crate::modules::SCROLL_GAP
                });
                if !soon {
                    self.module_scrolled = Some((index, now));
                    return self.module_act(index, act);
                }
            }
            Message::ModuleRerun(which) => {
                let count = self.config.modules.len();
                self.module_runs.resize(count, 0);
                for (i, run) in self.module_runs.iter_mut().enumerate() {
                    if which.is_none_or(|w| w == i) {
                        *run = run.wrapping_add(1);
                    }
                }
            }
            Message::PluginTile(key, Some(state)) if !self.switching.contains(&key) => {
                self.tile_is(key, state);
            }
            Message::PluginTile(..) => {}
            Message::PluginSwitch(key) => return self.switch_tile(key),
            Message::PluginSwitched(key, was, now) => self.tile_switched(key, was, now),
            Message::PluginFound(found) => self.launcher.set_plugin_found(found),
            Message::Control(Command::Module(name)) => {
                let named = self
                    .config
                    .modules
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| name.is_none() || m.name == name)
                    .map(|(i, _)| Task::done(Message::ModuleRerun(Some(i))));
                return Task::batch(named);
            }
            Message::Frame => {
                for shown in &mut self.shown {
                    if shown
                        .from
                        .as_ref()
                        .is_some_and(|(_, at)| at.elapsed() >= FADE)
                    {
                        shown.from = None;
                    }
                }
            }
            Message::ConfigChanged => {
                let now = stamp(&self.config_path);
                if now != self.config_stamp {
                    self.config_stamp = now;
                    let reload = self.reload_config();
                    return Task::batch([reload, self.triggers()]);
                }
            }
            Message::Key(
                keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    text,
                    ..
                },
                _,
            ) if self.overlay == Some(Overlay::Marks) => {
                return self.marks_key(key, modifiers, text.as_deref());
            }
            Message::Key(keyboard::Event::KeyPressed { key, text, .. }, captured) => {
                return self.key(key, text.as_deref(), captured);
            }
            Message::Control(Command::Switcher) => return self.toggle(Overlay::Switcher),
            Message::Control(Command::Launcher) => return self.toggle(Overlay::Launcher),
            Message::Control(Command::Panel) => return self.toggle(Overlay::Panel),
            Message::Control(Command::Clipboard) => return self.toggle(Overlay::Clipboard),
            Message::Control(Command::Keys) => return self.toggle(Overlay::Keys),
            Message::Control(Command::Machine) => return self.toggle(Overlay::Machine),
            Message::Control(Command::Replay) => return self.save_replay(),
            Message::Control(Command::Terminal) => {
                let terminal = self.config.terminal.clone();
                return self.spawn(&terminal);
            }
            Message::KeysQuery(query) => self.keys_query = query,
            Message::FocusWindow(id) => {
                if self.niri.is_some() {
                    return niri::send(apolline_niri::Request::FocusWindow(id));
                }
                for window in &mut self.status.windows {
                    window.2 = window.0 == id;
                }
            }
            Message::OpenPinned(id) => return self.open_pinned(&id),
            Message::Tray(crate::tray::Event::Items(items)) => {
                let theme = self.config.desktop.icons.as_deref();
                self.tray = crate::tray::draw(items, &self.tray, theme);
            }
            Message::Tunnels(tunnels) => self.tunnels = tunnels,
            Message::Vpn(tunnel, on) => {
                self.net_note = None;
                return Task::perform(crate::vpn::switch(tunnel, on), Message::VpnSwitched);
            }
            Message::VpnSwitched(Err(err)) => self.net_note = Some(err),
            Message::VpnSwitched(Ok(())) => {}
            Message::TrayPress(index, press) => {
                if let Some(item) = self.tray.get(index).cloned() {
                    return Task::future(crate::tray::press(item, press, (0, 0))).discard();
                }
            }
            Message::TrayMenu(index) => {
                let Some(item) = self.tray.get(index).cloned() else {
                    return Task::none();
                };
                // Without a menu of its own, the right click is the app's to handle.
                if item.menu.is_none() {
                    return self.update(Message::TrayPress(index, crate::tray::Press::Secondary));
                }
                return Task::perform(crate::tray::menu(item), move |entries| {
                    Message::TrayMenuRead(index, entries)
                });
            }
            Message::TrayMenuRead(index, entries) => {
                if let Some(item) = self.tray.get(index).cloned()
                    && !entries.is_empty()
                {
                    self.tray_menu = Some((item, entries));
                    return self.set_overlay(Some(Overlay::TrayMenu));
                }
            }
            Message::TrayChoose(id) => {
                let close = self.set_overlay(None);
                if let Some((item, _)) = self.tray_menu.take() {
                    let press = crate::tray::press(item, crate::tray::Press::Menu(id), (0, 0));
                    return Task::batch([close, Task::future(press).discard()]);
                }
                return close;
            }
            Message::Dock(event) => return self.dock_event(event),
            Message::StripStart(picked, x) => {
                self.dragging = picked.map(|(app, was_pinned)| (app, was_pinned, x));
            }
            Message::StripMove(x) => {
                if let Some((_, _, at)) = &mut self.dragging {
                    *at = x;
                }
            }
            Message::StripDrop(x) => {
                if let Some((app, was_pinned, _)) = self.dragging.take() {
                    let pinned = strip::landing(&self.config.pinned, &app, was_pinned, x);
                    if pinned != self.config.pinned {
                        self.save(|e| e.set_list("bar", "pinned", &pinned));
                        return self.reload_config();
                    }
                }
            }
            Message::Pin(id) => {
                let mut pinned = self.config.pinned.clone();
                match pinned.iter().position(|p| *p == id) {
                    Some(index) => {
                        pinned.remove(index);
                    }
                    None => pinned.push(id),
                }
                self.save(|e| e.set_list("bar", "pinned", &pinned));
                return self.reload_config();
            }
            Message::CloseWindow(id) => {
                if self.niri.is_some() {
                    return niri::send(apolline_niri::Request::CloseWindow(id));
                }
                self.status.windows.retain(|window| window.0 != id);
            }
            Message::Clip(clip) => clipboard::remember(&mut self.clips, clip),
            Message::ClipQuery(query) => return self.picker.search(query),
            Message::ClipCopy(index) => {
                if let Some(index) = index.or_else(|| self.picker.chosen(&self.clips))
                    && let Some(clip) = self.clips.get(index).map(Clip::again)
                {
                    clipboard::copy(&clip.content);
                    clipboard::remember(&mut self.clips, clip);
                    return self.set_overlay(None);
                }
            }
            Message::ClipPin(index) => {
                if let Some(clip) = self.clips.get_mut(index) {
                    clip.pinned = !clip.pinned;
                    if !self.sample() {
                        return clipboard::keep(&self.clips);
                    }
                }
            }
            Message::ClipRemove(index) => {
                if index < self.clips.len() {
                    let removed = self.clips.remove(index);
                    let count = self.picker.visible(&self.clips).len();
                    self.picker.selected = self.picker.selected.min(count.saturating_sub(1));
                    if removed.pinned && !self.sample() {
                        return clipboard::keep(&self.clips);
                    }
                }
            }
            Message::Control(Command::Nudge(nudge)) => return self.nudge(nudge),
            Message::Osd(Some(osd)) => return self.show(osd),
            Message::OsdDone(serial) => {
                if self.osd.as_ref().is_some_and(|(_, shown)| *shown == serial) {
                    self.osd = None;
                }
            }
            Message::Overlay(overlay) => return self.set_overlay(overlay),
            Message::Opened if self.overlay == Some(Overlay::Launcher) => {
                return self.launcher.focus();
            }
            Message::Opened if self.overlay == Some(Overlay::Clipboard) => {
                return self.picker.focus();
            }
            Message::Opened if self.overlay == Some(Overlay::Keys) => {
                return iced::widget::operation::focus(keymap::SEARCH);
            }
            Message::Opened if self.overlay == Some(Overlay::Polkit) => {
                return iced::widget::operation::focus(polkit::FIELD);
            }
            Message::Polkit(request) => {
                if let Some(old) = self.polkit.replace(polkit::Dialog::new(request)) {
                    polkit::finish(&old.request.cookie, false);
                }
                self.overlay = None;
                return Task::batch([
                    self.set_overlay(Some(Overlay::Polkit)),
                    iced::widget::operation::focus(polkit::FIELD),
                    self.ask_polkit(None),
                ]);
            }
            Message::PolkitGone(cookie) => {
                if self
                    .polkit
                    .as_ref()
                    .is_some_and(|d| d.request.cookie == cookie)
                {
                    self.polkit = None;
                    return self.set_overlay(None);
                }
            }
            Message::PolkitUser(index) => {
                if let Some(dialog) = &mut self.polkit {
                    dialog.user = index;
                    dialog.error = None;
                    return self.ask_polkit(None);
                }
            }
            Message::PolkitPassword(password) => {
                if let Some(dialog) = &mut self.polkit {
                    dialog.password = password;
                }
            }
            Message::PolkitSubmit => {
                if let Some(dialog) = &mut self.polkit
                    && !dialog.busy
                    && !dialog.password.is_empty()
                {
                    let password = std::mem::take(&mut dialog.password);
                    dialog.error = None;
                    // Into the running conversation, or a new one when that one is over.
                    let sent = dialog
                        .answers
                        .as_mut()
                        .is_some_and(|answers| answers.try_send(password.clone()).is_ok());
                    if !sent {
                        return self.ask_polkit(Some(password));
                    }
                    dialog.busy = true;
                }
            }
            Message::PolkitSaid(cookie, _)
                if self
                    .polkit
                    .as_ref()
                    .is_none_or(|d| d.request.cookie != cookie) => {}
            Message::PolkitSaid(_, polkit::Said::Info(info)) => {
                if let Some(dialog) = &mut self.polkit {
                    dialog.info = Some(info);
                }
            }
            // Asked for more after an answer, a code or a key's PIN: the field is free again.
            Message::PolkitSaid(_, polkit::Said::Asks(asks)) => {
                if let Some(dialog) = &mut self.polkit {
                    let asks = asks.trim().trim_end_matches(':').trim().to_owned();
                    let plain = asks.is_empty() || asks.eq_ignore_ascii_case("password");
                    dialog.asks = (!plain).then_some(asks);
                    dialog.busy = false;
                    return iced::widget::operation::focus(polkit::FIELD);
                }
            }
            Message::PolkitSaid(_, polkit::Said::Done(Ok(()))) => {
                if let Some(dialog) = self.polkit.take() {
                    polkit::finish(&dialog.request.cookie, true);
                }
                return self.set_overlay(None);
            }
            Message::PolkitSaid(_, polkit::Said::Done(Err(error))) => {
                if let Some(dialog) = &mut self.polkit {
                    let tried = std::mem::replace(&mut dialog.busy, false);
                    dialog.error = Some(error);
                    dialog.answers = None;
                    dialog.check = None;
                    // After a wrong password the helper starts over, so a finger works again;
                    // a helper that fails by itself waits for the next try instead.
                    let again = if tried {
                        self.ask_polkit(None)
                    } else {
                        Task::none()
                    };
                    return Task::batch([again, iced::widget::operation::focus(polkit::FIELD)]);
                }
            }
            Message::PolkitCancel => return self.set_overlay(None),
            // One that arrives after its wallpaper went out of use isn't kept.
            Message::Thumb(name, Some(thumb)) if self.wallpapers().contains(&name) => {
                self.thumbs.insert(name, thumb);
            }
            Message::SettingsPage(page) => {
                let close = self.set_overlay(None);
                return Task::batch([
                    close,
                    self.spawn(&format!("{} settings --page {page}", me())),
                ]);
            }
            Message::Control(Command::Mode(name)) => match self.config.find(&name) {
                Some(index) => return self.switch(Some(index)),
                None => eprintln!("apolline: there is no mode called “{name}”"),
            },
            Message::Control(Command::NoMode) => return self.switch(None),
            message => return self.handle_more(message),
        }
        Task::none()
    }

    // The sound server said something changed: what's shown of it is read again. Open quick
    // settings show it all; otherwise only the bar's volume is.
    fn hear_sound(&mut self, heard: controls::Heard) -> Task<Message> {
        if self.sample() {
            return Task::none();
        }
        let listening = if heard.recording {
            Task::perform(crate::privacy::read(), Message::Listening)
        } else {
            Task::none()
        };
        // Nothing heard is the timer below coming due.
        let timer = !heard.any();
        let held = self.sound_held.take();
        let heard = heard.and(held.unwrap_or_default());
        let quiet = self.touched.map_or(Duration::ZERO, |at| {
            QUIET_AFTER_TOUCH.saturating_sub(at.elapsed())
        });
        // Right after a slider moves, what's read would be thrown away, so it waits; one timer
        // at a time, however much is said meanwhile.
        if !quiet.is_zero() && (heard.output || heard.other) {
            self.sound_held = Some(heard);
            let later = if held.is_none() || timer {
                Task::perform(tokio::time::sleep(quiet), |()| {
                    Message::Sound(controls::Heard::default())
                })
            } else {
                Task::none()
            };
            return Task::batch([listening, later]);
        }
        let panel = self.overlay == Some(Overlay::Panel);
        let controls = if panel && (heard.output || heard.other) {
            self.read_controls()
        } else if heard.output {
            Task::perform(controls::volume(), Message::Volume)
        } else {
            Task::none()
        };
        Task::batch([controls, listening])
    }
}
