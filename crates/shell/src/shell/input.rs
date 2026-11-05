use super::*;

impl Shell {
    pub(super) fn key(&mut self, key: Key, text: Option<&str>, captured: bool) -> Task<Message> {
        // Escape drops the answer only while it's being typed; one kept as a draft while an
        // overlay is open is the overlay's to close.
        if self.replying.is_some() && self.overlay.is_none() && key == Key::Named(Named::Escape) {
            self.replying = None;
            return Task::none();
        }
        let launcher = self.overlay == Some(Overlay::Launcher);
        if self.overlay == Some(Overlay::Clipboard) {
            return self.clipboard_key(key, text, captured);
        }
        if self.overlay == Some(Overlay::Polkit) {
            if key == Key::Named(Named::Escape) {
                return self.set_overlay(None);
            }
            return Task::none();
        }
        if self.overlay == Some(Overlay::Keys) {
            match key.as_ref() {
                Key::Named(Named::Escape) => return self.set_overlay(None),
                _ if captured => {}
                Key::Named(Named::Backspace) => {
                    self.keys_query.pop();
                }
                _ => {
                    if let Some(typed) = text.filter(|t| t.chars().all(|c| !c.is_control())) {
                        self.keys_query.push_str(typed);
                    }
                }
            }
            return Task::none();
        }
        match key.as_ref() {
            Key::Named(Named::Escape) if self.overlay.is_some() => return self.set_overlay(None),
            Key::Named(Named::Escape) if self.demo => return iced::exit(),
            Key::Named(Named::ArrowDown) if launcher => {
                let by = if self.launcher.grid() {
                    crate::launcher::COLUMNS
                } else {
                    1
                };
                return self.launcher.step(by as isize);
            }
            Key::Named(Named::ArrowUp) if launcher => {
                let by = if self.launcher.grid() {
                    crate::launcher::COLUMNS
                } else {
                    1
                };
                return self.launcher.step(-(by as isize));
            }
            Key::Named(Named::ArrowRight) if launcher && self.launcher.grid() => {
                return self.launcher.step(1);
            }
            Key::Named(Named::ArrowLeft) if launcher && self.launcher.grid() => {
                return self.launcher.step(-1);
            }
            _ if captured => {}
            Key::Named(Named::Backspace) if launcher => {
                let mut query = self.launcher.query.clone();
                query.pop();
                return self.search(query);
            }
            Key::Named(Named::Enter) if launcher => return self.update(Message::Submit),
            _ if launcher => {
                if let Some(typed) = text.filter(|t| t.chars().all(|c| !c.is_control())) {
                    let query = format!("{}{typed}", self.launcher.query);
                    return self.search(query);
                }
            }
            Key::Character("0") => return self.switch(None),
            Key::Character(digit @ ("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")) => {
                let index = digit.parse::<usize>().unwrap_or(1) - 1;
                if index < self.config.modes.len() {
                    return self.switch(Some(index));
                }
            }
            Key::Character("m") if self.demo => return self.toggle(Overlay::Switcher),
            Key::Character("=" | "+") if self.demo => return self.nudge(Nudge::VolumeUp),
            Key::Character("-") if self.demo => return self.nudge(Nudge::VolumeDown),
            Key::Character("q") if self.demo => return self.toggle(Overlay::Panel),
            Key::Named(Named::Space) if self.demo => return self.toggle(Overlay::Launcher),
            Key::Character("/") if self.demo => return self.toggle(Overlay::Keys),
            Key::Named(Named::ArrowLeft) => self.workspace = self.workspace.saturating_sub(1),
            Key::Named(Named::ArrowRight) => {
                self.workspace = (self.workspace + 1).min(self.status.workspaces.saturating_sub(1));
            }
            _ => {}
        }
        Task::none()
    }

    pub(super) fn clipboard_key(
        &mut self,
        key: Key,
        text: Option<&str>,
        captured: bool,
    ) -> Task<Message> {
        match key.as_ref() {
            Key::Named(Named::Escape) => self.set_overlay(None),
            Key::Named(Named::ArrowDown) => self.picker.step(1, &self.clips),
            Key::Named(Named::ArrowUp) => self.picker.step(-1, &self.clips),
            Key::Named(Named::Delete) => match self.picker.chosen(&self.clips) {
                Some(index) => self.update(Message::ClipRemove(index)),
                None => Task::none(),
            },
            _ if captured => Task::none(),
            Key::Named(Named::Enter) => self.update(Message::ClipCopy(None)),
            Key::Named(Named::Backspace) => {
                let mut query = self.picker.query.clone();
                query.pop();
                self.picker.search(query)
            }
            _ => match text.filter(|t| t.chars().all(|c| !c.is_control())) {
                Some(typed) => {
                    let query = format!("{}{typed}", self.picker.query);
                    self.picker.search(query)
                }
                None => Task::none(),
            },
        }
    }

    pub(super) fn toggle(&mut self, overlay: Overlay) -> Task<Message> {
        if self.overlay == Some(overlay) {
            self.set_overlay(None)
        } else {
            self.set_overlay(Some(overlay))
        }
    }

    pub(super) fn set_overlay(&mut self, overlay: Option<Overlay>) -> Task<Message> {
        if self.overlay == overlay {
            return Task::none();
        }
        if self.overlay == Some(Overlay::Polkit)
            && let Some(dialog) = self.polkit.take()
        {
            polkit::finish(&dialog.request.cookie, false);
        }
        if self.overlay == Some(Overlay::Marks) {
            self.marks = None;
        }
        if self.overlay == Some(Overlay::Panel) {
            self.leave_detail();
        }
        self.overlay = overlay;
        self.power_menu = false;
        self.armed = None;
        match overlay {
            Some(Overlay::Launcher) => Task::batch([
                {
                    self.launcher.set_windows(self.open_windows());
                    self.launcher
                        .set_look(self.config.style.launcher, self.config.style.grid);
                    self.launcher
                        .set_finds(self.config.launcher.clone(), &self.config.terminal);
                    self.launcher.open()
                },
                Task::perform(apps::scan(self.config.desktop.icons.clone()), Message::Apps),
                Task::perform(crate::files::index(), Message::Files),
            ]),
            Some(Overlay::Panel) => {
                self.detail = None;
                self.leave_detail();
                self.held.clear();
                self.read_controls()
            }
            Some(Overlay::Clipboard) => self.picker.open(),
            Some(Overlay::Calendar) => {
                self.month = 0;
                self.day = None;
                self.now = Local::now();
                Task::none()
            }
            Some(Overlay::Keys) => {
                let niri = std::fs::read_to_string(fragment::niri_config()).unwrap_or_default();
                self.keymap = keymap::entries(&self.config, self.mode, &niri);
                self.keys_query.clear();
                iced::widget::operation::focus(keymap::SEARCH)
            }
            _ => Task::none(),
        }
    }

    pub(super) fn open_marks(&mut self) -> Task<Message> {
        let last = self.shot.as_ref().map(std::path::PathBuf::from);
        match last
            .filter(|file| file.is_file())
            .or_else(crate::marks::latest)
        {
            Some(file) => Task::perform(crate::marks::open(file), Message::MarksOpened),
            None => self.show(Osd::Note("No screenshot to mark yet".into())),
        }
    }

    pub(super) fn search(&mut self, query: String) -> Task<Message> {
        let found = self.launcher.search(query);
        if self.launcher.wants_rates() {
            let rates = Task::perform(crate::rates::load(), Message::Rates);
            return Task::batch([found, rates]);
        }
        found
    }

    pub(super) fn marks_key(
        &mut self,
        key: Key,
        modifiers: keyboard::Modifiers,
        text: Option<&str>,
    ) -> Task<Message> {
        use crate::marks::{Event, Tool};
        let typing = self.marks.as_ref().is_some_and(|m| m.typing());
        if typing && !modifiers.command() {
            let event = match key.as_ref() {
                Key::Named(Named::Escape) => Event::Drop,
                Key::Named(Named::Enter) => Event::Place,
                Key::Named(Named::Backspace) => Event::Erase,
                _ => match text.filter(|t| t.chars().all(|c| !c.is_control())) {
                    Some(typed) => Event::Typed(typed.to_owned()),
                    None => return Task::none(),
                },
            };
            return self.mark(event);
        }
        let event = match key.as_ref() {
            Key::Named(Named::Escape) => return self.set_overlay(None),
            Key::Named(Named::Enter) => Event::Both,
            Key::Character("z") if modifiers.command() => Event::Undo,
            Key::Character("c") if modifiers.command() => Event::Copy,
            Key::Character("s") if modifiers.command() => Event::Save,
            Key::Character("p") => Event::Tool(Tool::Pen),
            Key::Character("a") => Event::Tool(Tool::Arrow),
            Key::Character("b") => Event::Tool(Tool::Box),
            Key::Character("m") => Event::Tool(Tool::Marker),
            Key::Character("h") => Event::Tool(Tool::Hide),
            Key::Character("t") => Event::Tool(Tool::Text),
            Key::Character("n") => Event::Tool(Tool::Step),
            _ => return Task::none(),
        };
        self.mark(event)
    }

    pub(super) fn mark(&mut self, event: crate::marks::Event) -> Task<Message> {
        use crate::marks::Event;
        let Some(marks) = self.marks.as_mut() else {
            return Task::none();
        };
        if !matches!(event, Event::Copy | Event::Save | Event::Both) {
            marks.update(event);
            return Task::none();
        }
        let marks = marks.clone();
        // niri copied the shot as it was; with parts hidden, that copy must not stay around.
        let hides = marks.hides();
        if hides && let Ok(original) = std::fs::read(&marks.file) {
            self.clips.retain(
                |clip| !matches!(&clip.content, clipboard::Content::Png(png) if **png == original),
            );
        }
        let close = if matches!(event, Event::Both) {
            self.set_overlay(None)
        } else {
            Task::none()
        };
        let finish = Task::perform(
            async move {
                let done = tokio::task::spawn_blocking(move || {
                    let png = marks.render()?;
                    let file = marks.marked();
                    let mut said = Vec::new();
                    if hides || matches!(event, Event::Copy | Event::Both) {
                        let content = clipboard::Content::Png(std::sync::Arc::new(png.clone()));
                        clipboard::copy(&content);
                        said.push("Copied".to_owned());
                    }
                    if matches!(event, Event::Save | Event::Both) {
                        crate::marks::save(&file, &png)?;
                        let name = file.file_name().map(|n| n.to_string_lossy().into_owned());
                        said.push(crate::i18n::fill(
                            "Saved {name}",
                            &[("name", &name.unwrap_or_default())],
                        ));
                    }
                    Ok::<_, String>(said.join(" · "))
                })
                .await;
                done.unwrap_or_else(|err| Err(err.to_string()))
            },
            Message::MarksDone,
        );
        Task::batch([close, finish])
    }
}
