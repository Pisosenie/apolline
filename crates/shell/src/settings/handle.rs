use super::*;

impl Settings {
    pub(super) fn handle(&mut self, message: Message) -> Task<Message> {
        let index = self.selected;
        match message {
            Message::Find(query) => self.query = query,
            Message::ColorDraft(role, text) => {
                self.color_drafts.insert(role, text);
            }
            Message::ColorSet(role) => self.set_color(role),
            Message::ClockDraft(key, text) => {
                self.clock_drafts.insert(key, text);
            }
            Message::ClockSet(key) => self.set_clock(key),
            Message::ColorReset(role) => {
                self.color_drafts.remove(role);
                self.edit(|e| e.unset("colors", role));
            }
            Message::FoundSetting(page, title) => return self.go_to(page, title),
            Message::Page(page) => {
                self.page = page;
                self.query.clear();
                self.spot = None;
                self.recording = None;
                self.recording_mode_key = false;
                self.armed = false;
                if page == Page::About && self.machine.is_none() {
                    return Task::perform(crate::doctor::gather(), Message::Machine);
                }
            }
            Message::CheckMachine => {
                return Task::perform(crate::doctor::gather(), Message::Machine);
            }
            Message::Machine(lines) => self.machine = Some(lines),
            Message::Select(index) => {
                self.selected = index;
                self.adding = false;
                self.armed = false;
                self.name_draft = None;
                self.key_combo = None;
                return self.custom_thumb();
            }
            Message::Adding(adding) => self.adding = adding,
            Message::AddMode(template) => {
                let base = if template.is_empty() {
                    "New mode"
                } else {
                    template
                };
                let mut name = base.to_owned();
                let mut n = 2;
                while self.config.find(&name).is_some() {
                    name = format!("{base} {n}");
                    n += 1;
                }
                let mode = if template.is_empty() {
                    ModeSettings::blank(&name)
                } else {
                    ModeSettings {
                        name,
                        ..ModeSettings::template(template)
                    }
                };
                let mut added = None;
                self.edit(|e| added = Some(e.add_mode(&mode)));
                self.selected = added.unwrap_or(0);
                self.adding = false;
                return self.custom_thumb();
            }
            Message::RemoveMode => {
                if !self.armed {
                    self.armed = true;
                    return Task::none();
                }
                let starts_here = self.config.start() == Some(index);
                self.edit(|e| {
                    if starts_here {
                        e.set("general", "mode", "");
                    }
                    e.remove_mode(index);
                });
                self.selected = index.saturating_sub(1);
                self.armed = false;
            }
            Message::MoveMode(by) => {
                let to = index as isize + by;
                if to >= 0 && (to as usize) < self.config.modes.len() {
                    self.edit(|e| e.move_mode(index, to as usize));
                    self.selected = to as usize;
                }
            }
            Message::ModeName(name) => self.name_draft = Some(name),
            Message::ModeNameDone => {
                let Some(name) = self.name_draft.take().map(|n| n.trim().to_owned()) else {
                    return Task::none();
                };
                let taken = self.config.find(&name).is_some_and(|other| other != index);
                if name.is_empty() || taken {
                    self.error = Some(if taken {
                        format!("there is already a mode called “{name}”")
                    } else {
                        "a mode needs a name".to_owned()
                    });
                    return Task::none();
                }
                let starts_here = self.config.start() == Some(index);
                self.edit(|e| {
                    e.set_mode(index, "name", name.as_str());
                    if starts_here {
                        e.set("general", "mode", name.as_str());
                    }
                });
            }
            Message::ModeIcon(name) => self.edit(|e| e.set_mode(index, "icon", name)),
            Message::ModeWallpaper(wallpaper) => {
                self.edit(|e| match &wallpaper {
                    Some(name) => e.set_mode(index, "wallpaper", name.as_str()),
                    None => e.unset_mode(index, "wallpaper"),
                });
                return self.custom_thumb();
            }
            Message::ModeAccent(accent) => self.edit(|e| e.set_mode_accent(index, accent)),
            Message::ForMonitor(monitor) => self.monitor = monitor,
            Message::DisplayMode(name, mode) => {
                self.change_display(|e| e.set_display(&name, "mode", mode.as_str()))
            }
            Message::DisplayScale(name, percent) => {
                let scale = f64::from(percent) / 100.0;
                self.change_display(|e| e.set_display(&name, "scale", scale))
            }
            Message::DisplayTurn(name, turn) => self.change_display(|e| {
                if turn == "normal" {
                    e.unset_display(&name, "transform");
                } else {
                    e.set_display(&name, "transform", turn);
                }
            }),
            Message::DisplayVrr(name, on) => self.edit(|e| {
                if on {
                    e.set_display(&name, "vrr", true);
                } else {
                    e.unset_display(&name, "vrr");
                }
            }),
            Message::DisplayX(name, x) => {
                if self.place.0 != name {
                    self.place = (name, String::new(), String::new());
                }
                self.place.1 = x;
            }
            Message::DisplayY(name, y) => {
                if self.place.0 != name {
                    self.place = (name, String::new(), String::new());
                }
                self.place.2 = y;
            }
            Message::DisplayMove(name) if self.place.0 == name => {
                if let (Ok(x), Ok(y)) = (
                    self.place.1.trim().parse::<i64>(),
                    self.place.2.trim().parse::<i64>(),
                ) {
                    self.change_display(|e| e.set_display_position(&name, x, y));
                    self.place = (String::new(), String::new(), String::new());
                }
            }
            Message::DisplayMove(_) => {}
            Message::DisplayPlace(name, x, y) => {
                // Without niri the sample screens move here; with it, niri's answer moves them.
                if !crate::niri::connected()
                    && let Some(logical) = self
                        .outputs
                        .iter_mut()
                        .find(|o| o.name == name)
                        .and_then(|o| o.logical.as_mut())
                {
                    (logical.x, logical.y) = (x as i32, y as i32);
                }
                self.change_display(|e| e.set_display_position(&name, x, y))
            }
            Message::KeepDisplays => self.revert = None,
            Message::RevertDisplays => self.put_displays_back(),
            Message::DesktopWallpaper(name) if self.monitor.is_some() => {
                let monitor = self.monitor.clone().unwrap_or_default();
                let mut outputs = self.config.desktop.outputs.clone();
                outputs.retain(|(output, _)| *output != monitor);
                if name != self.config.desktop.wallpaper {
                    outputs.push((monitor, name));
                }
                self.edit(|e| e.set_pairs("desktop", "outputs", &outputs));
            }
            Message::DesktopWallpaper(name) => {
                self.edit(|e| e.set("desktop", "wallpaper", name.as_str()));
                return self.custom_thumb();
            }
            Message::DesktopAccent(accent) => self.edit(|e| e.set_accent(accent)),
            Message::DesktopClock(label) => {
                let spot = CLOCK_SPOTS
                    .iter()
                    .find(|(_, name)| *name == label)
                    .map(|(key, _)| *key)
                    .unwrap_or("off");
                self.edit(|e| {
                    if spot == "off" {
                        e.unset("desktop", "clock");
                    } else {
                        e.set("desktop", "clock", spot);
                    }
                });
            }
            // A palette brings its own accent; one made from the wallpaper takes the wallpaper's.
            Message::Palette(name) => self.edit(|e| {
                if name == "dark" {
                    e.unset("desktop", "palette");
                } else {
                    e.set("desktop", "palette", name.as_str());
                }
                let made = apolline_config::WALLPAPER_PALETTES.contains(&name.as_str());
                e.set_accent(if made { Accent::Auto } else { Accent::Palette });
            }),
            Message::WallpaperDraft(draft) => self.wallpaper_draft = draft,
            Message::AllWallpapers => self.all_walls = !self.all_walls,
            Message::Rotate(minutes) => self.edit(|e| {
                if minutes == 0 {
                    e.unset("desktop", "rotate");
                } else {
                    e.set("desktop", "rotate", i64::from(minutes));
                }
            }),
            Message::Shuffle(on) => self.edit(|e| {
                if on {
                    e.set("desktop", "shuffle", true);
                } else {
                    e.unset("desktop", "shuffle");
                }
            }),
            Message::Cursor(theme) => self.edit(|e| {
                if theme == SYSTEM_CURSOR {
                    e.unset("desktop", "cursor");
                } else {
                    e.set("desktop", "cursor", theme.as_str());
                }
            }),
            Message::Icons(theme) => {
                self.edit(|e| {
                    if theme == SYSTEM_CURSOR {
                        e.unset("desktop", "icons");
                    } else {
                        e.set("desktop", "icons", theme.as_str());
                    }
                });
                return Task::perform(apps::scan(self.config.desktop.icons.clone()), Message::Apps);
            }
            Message::GtkTheme(theme) => self.edit(|e| {
                if theme == SYSTEM_CURSOR {
                    e.unset("desktop", "gtk");
                } else {
                    e.set("desktop", "gtk", theme.as_str());
                }
            }),
            Message::FontFamily(family) => self.edit(|e| {
                if family == BUILT_IN_FONT {
                    e.unset("desktop", "font");
                } else {
                    e.set("desktop", "font", family.as_str());
                }
            }),
            Message::CursorSize(size) => {
                self.edit(|e| e.set("desktop", "cursor_size", i64::from(size)))
            }
            Message::ColorApps(on) => self.edit(|e| {
                if on {
                    e.set("apps", "colors", true);
                } else {
                    e.unset("apps", "colors");
                }
            }),
            Message::Apps(apps) => self.apps = apps,
            Message::PinQuery(query) => self.pin_query = query,
            Message::AppDraft(key, name) => self.app_draft = (key, name),
            Message::AppListAdd(key) if self.app_draft.0 == key => {
                let name = self.app_draft.1.trim().to_owned();
                let mut list = self.app_list_of(key).to_vec();
                if !name.is_empty() && !list.iter().any(|a| a.eq_ignore_ascii_case(&name)) {
                    list.push(name);
                    self.edit(|e| e.set_list("notifications", key, &list));
                }
                self.app_draft = ("", String::new());
            }
            Message::AppListAdd(_) => {}
            Message::AppListRemove(key, index) => {
                let mut list = self.app_list_of(key).to_vec();
                if index < list.len() {
                    list.remove(index);
                    self.edit(|e| {
                        if list.is_empty() {
                            e.unset("notifications", key);
                        } else {
                            e.set_list("notifications", key, &list);
                        }
                    });
                }
            }
            Message::ModuleDraft(draft) => self.module_draft = draft,
            Message::AddModule => {
                let command = self.module_draft.trim().to_owned();
                if !command.is_empty() {
                    self.edit(|e| e.add_module(&command));
                    self.module_draft.clear();
                }
            }
            Message::RemoveModule(index) => self.edit(|e| e.remove_module(index)),
            Message::CalendarDraft(draft) => self.calendar_draft = draft,
            Message::AddCalendar => {
                let link = Calendar::Ics(self.calendar_draft.trim().to_owned());
                if !self.calendar_draft.trim().is_empty() && !self.config.calendars.contains(&link)
                {
                    let mut calendars = self.config.calendars.clone();
                    calendars.push(link);
                    self.edit(|e| e.set_calendars(&calendars));
                }
                self.calendar_draft.clear();
            }
            Message::RemoveCalendar(index) if index < self.config.calendars.len() => {
                let mut calendars = self.config.calendars.clone();
                let gone = calendars.remove(index);
                self.edit(|e| e.set_calendars(&calendars));
                // Listed twice, it still needs its password.
                if let Calendar::CalDav { url, user } = &gone
                    && !calendars.contains(&gone)
                {
                    let (url, user) = (url.clone(), user.clone());
                    return Task::perform(
                        async move { secrets::forget(&url, &user).await },
                        Message::Forgotten,
                    );
                }
            }
            Message::RemoveCalendar(_) => {}
            Message::CalDavUrl(url) => self.caldav.url = url,
            Message::CalDavUser(user) => self.caldav.user = user,
            Message::CalDavPassword(password) => self.caldav.password = password,
            Message::AddCalDav => return self.add_caldav(),
            Message::CalDavAdded(Ok(calendar)) => {
                let mut calendars = self.config.calendars.clone();
                calendars.push(calendar);
                self.edit(|e| e.set_calendars(&calendars));
                self.caldav = CalDavForm::default();
            }
            Message::CalDavAdded(Err(error)) => {
                self.caldav.checking = false;
                self.caldav.error = Some(error);
            }
            Message::Forgotten(result) => self.caldav.error = result.err(),
            Message::Keyring(result) => self.keyring = Some(result),
            Message::WeatherDraft(draft) => self.weather_draft = Some(draft),
            Message::SetWeather => {
                if let Some(place) = self.weather_draft.take() {
                    let place = place.trim().to_owned();
                    self.edit(|e| {
                        if place.is_empty() {
                            e.unset("bar", "weather");
                        } else {
                            e.set("bar", "weather", place.as_str());
                        }
                    });
                }
            }
            Message::PinApp(id) => {
                let mut pinned = self.config.pinned.clone();
                if !pinned.contains(&id) {
                    pinned.push(id);
                    self.edit(|e| e.set_list("bar", "pinned", &pinned));
                }
                self.pin_query.clear();
            }
            Message::UnpinApp(index) => {
                let mut pinned = self.config.pinned.clone();
                if index < pinned.len() {
                    pinned.remove(index);
                    self.edit(|e| e.set_list("bar", "pinned", &pinned));
                }
            }
            Message::RemoveFolder(index) => {
                let mut folders = self.config.desktop.folders.clone();
                if index < folders.len() {
                    folders.remove(index);
                    self.edit(|e| e.set_list("desktop", "folders", &folders));
                    return self.scan_library();
                }
            }
            Message::UseWallpaper => {
                let path = self.wallpaper_draft.trim().to_owned();
                let folder = wallpaper::expand(&path).filter(|p| p.is_dir());
                match wallpaper::resolve(&path) {
                    _ if path.is_empty() => {}
                    _ if folder.is_some() => {
                        let mut folders = self.config.desktop.folders.clone();
                        let name = wallpaper::shorten(&folder.unwrap_or_default());
                        if !folders.contains(&name) {
                            folders.push(name);
                            self.edit(|e| e.set_list("desktop", "folders", &folders));
                        }
                        self.wallpaper_draft.clear();
                        return self.scan_library();
                    }
                    wallpaper::Source::File(file) if !file.exists() => {
                        self.error = Some(format!("{} does not exist", file.display()));
                    }
                    _ => {
                        self.wallpaper_draft.clear();
                        return self.update(if self.page == Page::Modes {
                            Message::ModeWallpaper(Some(path))
                        } else {
                            Message::DesktopWallpaper(path)
                        });
                    }
                }
            }
            Message::Notifications(index, n) => self.edit(|e| e.set_mode_notifications(index, n)),
            Message::Performance(on) => self.edit(|e| e.set_mode(index, "performance", on)),
            Message::KeepAwake(on) => self.edit(|e| e.set_mode(index, "keep_awake", on)),
            Message::Replay(0) => self.edit(|e| e.unset_mode(index, "replay")),
            Message::Replay(seconds) => {
                self.edit(|e| e.set_mode(index, "replay", i64::from(seconds)))
            }
            Message::ModeTheme(theme) => self.edit(|e| match &theme {
                Some(name) => e.set_mode(index, "theme", name.as_str()),
                None => e.unset_mode(index, "theme"),
            }),
            Message::WhenGame(on) => self.set_trigger(index, Trigger::Game, on),
            Message::WhenBattery(on) => self.set_trigger(index, Trigger::Battery, on),
            Message::HoursDraft(draft) => self.hours_draft = draft,
            Message::MonitorDraft(draft) => self.monitor_draft = draft,
            Message::SetHours => match Trigger::parse(self.hours_draft.trim()) {
                Some(hours @ Trigger::Hours { .. }) => {
                    self.set_trigger(index, hours, true);
                    self.hours_draft.clear();
                }
                _ => self.error = Some("Hours look like 20:00-07:00".into()),
            },
            Message::SetMonitor => {
                let name = self.monitor_draft.trim().to_owned();
                if !name.is_empty() {
                    self.set_trigger(index, Trigger::Monitor(name), true);
                    self.monitor_draft.clear();
                }
            }
            Message::DropTrigger(at) => {
                if let Some(mode) = self.config.modes.get(index) {
                    let mut when = mode.when.clone();
                    if at < when.len() {
                        when.remove(at);
                        self.write_triggers(index, &when);
                    }
                }
            }
            Message::StartHere(on) => {
                let name = self.config.modes.get(index).map(|m| m.name.clone());
                self.edit(|e| {
                    e.set(
                        "general",
                        "mode",
                        if on {
                            name.unwrap_or_default()
                        } else {
                            String::new()
                        },
                    )
                });
            }
            Message::KeyCommand(command) => self.key_command = command,
            Message::RecordModeKey => {
                self.recording_mode_key = true;
                self.recording = None;
            }
            Message::AddModeKey => {
                let command = self.key_command.trim().to_owned();
                if let (Some(combo), false) = (self.key_combo.clone(), command.is_empty())
                    && let Some(mode) = self.config.modes.get(index)
                {
                    let mut keys: Vec<(String, String)> = mode
                        .keys
                        .iter()
                        .filter(|(c, _)| !keys::same(c, &combo))
                        .cloned()
                        .collect();
                    keys.push((combo, command));
                    self.edit(|e| e.set_mode_keys(index, &keys));
                    self.key_combo = None;
                    self.key_command.clear();
                }
            }
            Message::RemoveModeKey(k) => {
                if let Some(mode) = self.config.modes.get(index) {
                    let mut keys = mode.keys.clone();
                    if k < keys.len() {
                        keys.remove(k);
                        self.edit(|e| e.set_mode_keys(index, &keys));
                    }
                }
            }
            Message::Language(code) => self.edit(|e| match code {
                "auto" => e.unset("general", "language"),
                code => e.set("general", "language", code),
            }),
            message @ (Message::Browse
            | Message::Catalog(_)
            | Message::CatalogPicture(..)
            | Message::CatalogInstall(_)
            | Message::CatalogInstalled(_)) => return self.browse(message),
            message => return self.tweak(message),
        }
        Task::none()
    }
}
