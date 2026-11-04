use super::*;

impl Settings {
    // The looks, the bar, input, the lock, windows and themes; handle() passes them here.
    pub(super) fn tweak(&mut self, message: Message) -> Task<Message> {
        let index = self.selected;
        match message {
            Message::Draft(draft) => self.draft = draft,
            Message::Add => {
                let command = self.draft.trim().to_owned();
                if !command.is_empty()
                    && let Some(mode) = self.config.modes.get(index)
                {
                    let mut open = mode.open.clone();
                    open.push(command);
                    self.edit(|e| e.set_mode_list(index, "open", &open));
                    self.draft.clear();
                }
            }
            Message::Remove(app) => {
                if let Some(mode) = self.config.modes.get(index) {
                    let mut open = mode.open.clone();
                    if app < open.len() {
                        open.remove(app);
                        self.edit(|e| e.set_mode_list(index, "open", &open));
                    }
                }
            }
            Message::Gaps(v) => self.stage(|e| e.set("layout", "gaps", i64::from(v))),
            Message::Radius(v) => self.stage(|e| e.set("layout", "radius", i64::from(v))),
            Message::Stat(name, on) => {
                let mut names = self.config.bar.stats.names();
                names.retain(|n| n != name);
                if on {
                    names.push(name.to_owned());
                }
                names.sort_by_key(|n| apolline_config::Stats::ALL.iter().position(|a| a == n));
                self.edit(|e| {
                    if names.is_empty() {
                        e.unset("bar", "stats");
                    } else {
                        e.set_list("bar", "stats", &names);
                    }
                });
            }
            Message::NightSchedule(on) => {
                self.night_draft = None;
                self.edit(|e| {
                    if on {
                        e.set("night", "hours", "21:00-07:00");
                    } else {
                        e.unset("night", "hours");
                    }
                });
            }
            Message::NightDraft(draft) => self.night_draft = Some(draft),
            Message::SetNight => {
                if let Some(Trigger::Hours { from, until }) =
                    self.night_draft.as_deref().and_then(Trigger::parse)
                {
                    let hours = Trigger::Hours { from, until }.text();
                    self.edit(|e| e.set("night", "hours", hours.as_str()));
                    self.night_draft = None;
                }
            }
            Message::NightWarmth(kelvin) => {
                self.stage(|e| e.set("night", "temperature", i64::from(kelvin)))
            }
            Message::SurfaceFont(look, family) => self.edit(|e| {
                if family == SAME_FONT {
                    e.unset_in(&["style", look.key()], "font");
                } else {
                    e.set_in(&["style", look.key()], "font", family.as_str());
                }
            }),
            Message::Style(look, knob, v) => self.stage(|e| match knob {
                Knob::Corners => e.set_in(&["style", look.key()], "radius", i64::from(v)),
                Knob::Glass => e.set_in(&["style", look.key()], "opacity", hundredths(v)),
                Knob::Size => e.set_in(&["style", look.key()], "size", hundredths(v)),
            }),
            Message::LauncherGrid(on) => {
                self.edit(|e| e.set_in(&["style", "launcher"], "grid", on))
            }
            Message::CardsCorner(corner) => {
                self.edit(|e| e.set("notifications", "position", corner.key()))
            }
            Message::BorderStyle(style) => self.edit(|e| {
                e.set("layout", "borders", style > 0);
                if style == 2 {
                    e.set("layout", "gradient", true);
                } else {
                    e.unset("layout", "gradient");
                }
            }),
            Message::Shadows(on) => self.edit(|e| e.set("layout", "shadows", on)),
            Message::BorderWidth(width) => self.stage(|e| {
                if width == 1 {
                    e.unset("layout", "border_width");
                } else {
                    e.set("layout", "border_width", i64::from(width));
                }
            }),
            Message::Workspaces(style) => {
                let value = match style {
                    Workspaces::Dots => "dots",
                    Workspaces::Numbers => "numbers",
                };
                self.edit(|e| e.set("bar", "workspaces", value));
            }
            Message::Seconds(on) => self.edit(|e| e.set("bar", "seconds", on)),
            Message::BarHold(held) => self.bar_held = held,
            Message::TrayHold(held) => self.tray_held = held,
            Message::BoardMove(at) => {
                for held in [&mut self.bar_held, &mut self.tray_held]
                    .into_iter()
                    .flatten()
                {
                    held.at = at;
                }
            }
            Message::BarDrop(at, width) => {
                let (board, _) = self.boards();
                let held = self.bar_held.take();
                if let Some(board) =
                    held.map(|h| board::dropped(&board, &h.name, at, piece_label, width))
                {
                    let names = |zone: Option<&Vec<String>>| zone.cloned().unwrap_or_default();
                    let [left, center, right] = [0, 1, 2].map(|z| names(board.zones.get(z)));
                    self.edit(|e| {
                        e.set_list("bar", "left", &left);
                        e.set_list("bar", "center", &center);
                        e.set_list("bar", "right", &right);
                    });
                }
            }
            Message::TrayDrop(at, width) => {
                let (_, board) = self.boards();
                let held = self.tray_held.take();
                if let Some(board) =
                    held.map(|h| board::dropped(&board, &h.name, at, tray_label, width))
                {
                    let tray = board.zones.first().cloned().unwrap_or_default();
                    self.edit(|e| e.set_list("bar", "tray", &tray));
                }
            }
            Message::BarEdge(edge) => self.edit(|e| e.set("bar", "position", edge.key())),
            Message::BarFloating(on) => self.edit(|e| e.set("bar", "floating", on)),
            Message::BarShape(shape) => self.edit(|e| match shape {
                apolline_config::Shape::Bar => e.unset("bar", "shape"),
                shape => e.set("bar", "shape", shape.key()),
            }),
            Message::BarOpacity(v) => self.stage(|e| e.set("bar", "opacity", f64::from(v) / 100.0)),
            Message::BarBlur(on) => self.edit(|e| {
                if on {
                    e.set("bar", "blur", true);
                } else {
                    e.unset("bar", "blur");
                }
            }),
            Message::Date(on) => self.edit(|e| e.set("bar", "date", on)),
            Message::BatteryPercent(on) => self.edit(|e| e.set("bar", "battery_percent", on)),
            Message::DoNotDisturb(on) => {
                self.edit(|e| e.set("notifications", "do_not_disturb", on))
            }
            Message::PopupSeconds(v) => {
                self.stage(|e| e.set("notifications", "seconds", i64::from(v)))
            }
            Message::Test => {
                self.tested = None;
                return Task::perform(
                    notify::send(
                        "This is how a popup looks".into(),
                        crate::i18n::count(
                            usize::from(self.config.alerts.seconds),
                            "It stays for 1 second",
                            "It stays for {n} seconds",
                        ),
                    ),
                    Message::Tested,
                );
            }
            Message::Tested(result) => self.tested = Some(result),
            Message::AddLayout(layout) => {
                let mut layouts = self.config.keyboard.layouts.clone();
                if !layouts.iter().any(|l| l == layout.0) {
                    layouts.push(layout.0.to_owned());
                    self.edit(|e| e.set_list("keyboard", "layouts", &layouts));
                }
            }
            Message::RemoveLayout(index) => {
                let mut layouts = self.config.keyboard.layouts.clone();
                if index < layouts.len() {
                    layouts.remove(index);
                    self.edit(|e| e.set_list("keyboard", "layouts", &layouts));
                }
            }
            Message::Switch(switch) => self.edit(|e| e.set("keyboard", "switch", switch.key())),
            Message::RepeatDelay(v) => {
                self.stage(|e| e.set("keyboard", "repeat_delay", i64::from(v)))
            }
            Message::RepeatRate(v) => {
                self.stage(|e| e.set("keyboard", "repeat_rate", i64::from(v)))
            }
            Message::Typing(text) => self.typing = text,
            Message::Tap(on) => self.edit(|e| e.set("touchpad", "tap", on)),
            Message::TouchNatural(on) => self.edit(|e| e.set("touchpad", "natural_scroll", on)),
            Message::TouchTyping(on) => {
                self.edit(|e| e.set("touchpad", "disable_while_typing", on))
            }
            Message::TouchSpeed(v) => self.stage(|e| e.set_speed("touchpad", v)),
            Message::MouseNatural(on) => self.edit(|e| e.set("mouse", "natural_scroll", on)),
            Message::MouseAcceleration(on) => self.edit(|e| e.set("mouse", "acceleration", on)),
            Message::MouseSpeed(v) => self.stage(|e| e.set_speed("mouse", v)),
            Message::Record(shortcut) => {
                self.recording = shortcut;
                self.shortcut_error = None;
            }
            Message::Key(keyboard::Event::KeyPressed { key, modifiers, .. })
                if self.recording_mode_key =>
            {
                if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    self.recording_mode_key = false;
                } else if let Some(combo) = keys::combo(&key, modifiers) {
                    self.key_combo = Some(combo);
                    self.recording_mode_key = false;
                }
            }
            Message::Key(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if let Some(shortcut) = self.recording {
                    if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                        self.recording = None;
                    } else if let Some(combo) = keys::combo(&key, modifiers) {
                        self.record(shortcut, combo);
                    }
                } else if modifiers.command() && key.as_ref() == keyboard::Key::Character("z") {
                    self.step(!modifiers.shift());
                }
            }
            Message::Undo => self.step(true),
            Message::Redo => self.step(false),
            Message::ResetShortcut(shortcut) => {
                let combo = apolline_config::Shortcuts::default()
                    .get(shortcut)
                    .to_owned();
                self.record(shortcut, combo);
            }
            Message::LockBeforeSleep(on) => self.edit(|e| e.set("lock", "before_sleep", on)),
            Message::LockMedia(on) => self.edit(|e| e.set("lock", "show_media", on)),
            Message::LockCentered(on) => self.edit(|e| {
                if on {
                    e.set("lock", "layout", "center");
                } else {
                    e.unset("lock", "layout");
                }
            }),
            Message::LockBlur(on) => self.edit(|e| e.set("lock", "blur", on)),
            Message::LockFingerprint(on) => self.edit(|e| e.set("lock", "fingerprint", on)),
            Message::IdleLock(v) => self.stage(|e| e.set("idle", "lock_after", i64::from(v))),
            Message::IdleScreen(v) => {
                self.stage(|e| e.set("idle", "screen_off_after", i64::from(v)))
            }
            Message::IdleSuspend(v) => self.stage(|e| e.set("idle", "suspend_after", i64::from(v))),
            Message::GreetPreview => {
                self.greet_note = None;
                let staged = crate::greet_look::stage(&self.config);
                return match staged {
                    Ok(files) => {
                        let line = format!(
                            "{} greet --preview --look '{}' &",
                            crate::control::me(),
                            files[0].display()
                        );
                        Task::future(async move {
                            let _ = tokio::process::Command::new("sh")
                                .args(["-c", &line])
                                .status()
                                .await;
                        })
                        .discard()
                    }
                    Err(err) => {
                        self.greet_note = Some(Err(err));
                        Task::none()
                    }
                };
            }
            Message::GreetInstall => {
                self.greet_note = None;
                return match crate::greet_look::stage(&self.config) {
                    Ok(files) => {
                        Task::perform(crate::greet_look::install(files), Message::GreetDone)
                    }
                    Err(err) => {
                        self.greet_note = Some(Err(err));
                        Task::none()
                    }
                };
            }
            Message::GreetDone(result) => self.greet_note = Some(result),
            Message::LockNow => {
                let line = format!("{} lock &", crate::control::me());
                return Task::future(async move {
                    let _ = tokio::process::Command::new("sh")
                        .args(["-c", &line])
                        .status()
                        .await;
                })
                .discard();
            }
            Message::WinWidth(width) => self.edit(|e| e.set("windows", "width", width.key())),
            Message::WinPreset(width) => {
                let mut presets = self
                    .config
                    .windows
                    .presets
                    .clone()
                    .unwrap_or_else(|| vec![Width::Third, Width::Half, Width::TwoThirds]);
                if presets.contains(&width) {
                    presets.retain(|w| *w != width);
                } else {
                    presets.push(width);
                    presets.sort_by(|a, b| a.proportion().total_cmp(&b.proportion()));
                }
                if !presets.is_empty() {
                    let keys: Vec<String> = presets.iter().map(|w| w.key().to_owned()).collect();
                    self.edit(|e| e.set_list("windows", "presets", &keys));
                }
            }
            Message::WinCenter(center) => self.edit(|e| e.set("windows", "center", center.key())),
            Message::CenterLone(on) => self.edit(|e| e.set("windows", "center_lone", on)),
            Message::RecentWindows(on) => self.edit(|e| e.set("windows", "switcher", on)),
            Message::FocusFollows(on) => self.edit(|e| e.set("windows", "focus_follows_mouse", on)),
            Message::WarpMouse(on) => self.edit(|e| e.set("windows", "warp_mouse", on)),
            Message::Opacity(v) => {
                self.stage(|e| e.set("windows", "opacity", f64::from(v) / 100.0))
            }
            Message::InactiveOpacity(v) => {
                self.stage(|e| e.set("windows", "inactive_opacity", f64::from(v) / 100.0))
            }
            Message::WinMotion(motion) => self.edit(|e| match motion {
                Some(motion) => e.set("windows", "motion", motion.key()),
                None => e.unset("windows", "motion"),
            }),
            Message::WinAppear(appear) => self.edit(|e| match appear {
                Some(appear) => e.set("windows", "appear", appear.key()),
                None => e.unset("windows", "appear"),
            }),
            Message::WinBlur(blur) => self.edit(|e| match blur {
                Some(blur) => e.set("windows", "blur", blur.key()),
                None => e.unset("windows", "blur"),
            }),
            Message::Animations(choice) => match choice {
                2 => self.edit(|e| e.unset("windows", "animations")),
                _ => {
                    let speed = [0.0, 2.0, 1.0, 0.5][usize::from(choice.min(3))];
                    self.edit(|e| e.set("windows", "animations", speed));
                }
            },
            Message::RuleApp(app) => self.rule.app = app,
            Message::RuleFloating(on) => self.rule.floating = Some(on),
            Message::RuleWidth(width) => {
                self.rule.width = (self.rule.width != Some(width)).then_some(width)
            }
            Message::AddRule => {
                let app = self.rule.app.trim().to_owned();
                if !app.is_empty() {
                    let rule = Rule {
                        app,
                        ..self.rule.clone()
                    };
                    self.edit(|e| e.add_rule(&rule));
                    self.rule.app.clear();
                }
            }
            Message::RemoveRule(index) => self.edit(|e| e.remove_rule(index)),
            Message::ApplyTheme(index) => {
                if let Some((theme, _)) = self.themes.get(index).cloned() {
                    self.edit(|e| theme.apply(e));
                    self.theme_note = None;
                    return self.custom_thumb();
                }
            }
            Message::ThemeName(name) => self.theme_name = name,
            Message::SaveTheme => {
                let name = self.theme_name.trim().to_owned();
                if !name.is_empty() {
                    let theme = Look::of(&self.config, &name, std::env::var("USER").ok());
                    self.theme_note = Some(themes::save(&theme).map(|path| {
                        format!(
                            "Saved as {}. Send that file to share the look.",
                            path.display()
                        )
                    }));
                    if self.theme_note.as_ref().is_some_and(Result::is_ok) {
                        self.edit(|e| e.set("desktop", "theme", name.as_str()));
                        self.theme_name.clear();
                    }
                    self.themes = themes::all();
                }
            }
            Message::ThemePath(path) => self.theme_path = path,
            Message::ImportTheme => {
                let path = self.theme_path.trim();
                if !path.is_empty() {
                    let path = match path.strip_prefix("~/") {
                        Some(rest) => std::env::var_os("HOME")
                            .map(PathBuf::from)
                            .unwrap_or_default()
                            .join(rest),
                        None => PathBuf::from(path),
                    };
                    self.theme_note = Some(
                        themes::install(&path)
                            .map(|theme| format!("“{}” is in the gallery now", theme.name)),
                    );
                    self.theme_path.clear();
                    self.themes = themes::all();
                    return self.custom_thumb();
                }
            }
            Message::OpenThemes => {
                let _ = std::fs::create_dir_all(themes::dir());
                let opener = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                let dir = themes::dir();
                return Task::future(async move {
                    let _ = tokio::process::Command::new(opener).arg(dir).status().await;
                })
                .discard();
            }
            Message::Setup => {
                let look = Fragment {
                    config: &self.config,
                    mode: self.config.start(),
                    outputs: &[],
                };
                self.setup_log =
                    match fragment::setup(&fragment::path(), &fragment::niri_config(), &look) {
                        Ok(done) => done,
                        Err(err) => vec![format!("could not set up: {err}")],
                    };
            }
            Message::Finish => return iced::exit(),
            Message::OpenFile => {
                let opener = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                let path = self.path.clone();
                return Task::future(async move {
                    let _ = tokio::process::Command::new(opener)
                        .arg(path)
                        .status()
                        .await;
                })
                .discard();
            }
            Message::Commit => self.save(),
            Message::Thumbs(thumbs) => {
                for (name, handle) in thumbs {
                    self.thumbs.retain(|(n, _)| *n != name);
                    self.thumbs.push((name, handle));
                }
            }
            Message::Thumb(name, Some(handle)) => {
                self.thumbs.retain(|(n, _)| *n != name);
                self.thumbs.push((name, handle));
                return self.next_thumb();
            }
            Message::Thumb(_, None) => return self.next_thumb(),
            Message::Tick => {
                self.live = crate::control::shared();
                if self.page == Page::Bar {
                    self.heard = events::heard();
                }
                if self
                    .revert
                    .as_ref()
                    .is_some_and(|(at, _)| std::time::Instant::now() >= *at)
                {
                    self.put_displays_back();
                }
                if self.page == Page::Displays
                    && self.outputs_may_change()
                    && crate::niri::connected()
                {
                    self.outputs = crate::niri::outputs();
                }
                if stamp(&self.path) != self.stamp {
                    self.reload();
                }
                if self.screenshot.is_some()
                    && !self.shooting
                    && self.queue.is_empty()
                    && wallpaper::bundled()
                        .iter()
                        .all(|(name, _)| self.thumb(name).is_some())
                {
                    self.shooting = true;
                    return window::latest()
                        .and_then(window::screenshot)
                        .map(Message::Shot);
                }
            }
            Message::Shot(capture) => {
                if let Some(path) = self.screenshot.take()
                    && let Err(err) = shot::save(&path, &capture)
                {
                    eprintln!("apolline: could not save {}: {err}", path.display());
                }
                return iced::exit();
            }
            Message::Key(_) | Message::Noop => {}
            _ => {}
        }
        Task::none()
    }
}
