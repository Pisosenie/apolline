use super::*;

impl Shell {
    pub(super) fn run(&mut self, target: Target) -> Task<Message> {
        let close = self.set_overlay(None);
        let task = match target {
            Target::App(command) => {
                self.usage.note(&command, crate::usage::now());
                self.usage.save();
                self.launcher.set_usage(self.usage.clone());
                self.spawn(&command)
            }
            Target::Run(command) => self.spawn(&command),
            Target::Web(query) => {
                let opener = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                self.spawn(&format!(
                    "{opener} '{}'",
                    crate::launcher::web_address(&query)
                ))
            }
            Target::Window(id) => {
                return Task::batch([close, self.update(Message::FocusWindow(id))]);
            }
            Target::Open(path) => {
                let opener = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                let quoted = format!("'{}'", path.display().to_string().replace('\'', r"'\''"));
                self.spawn(&format!("{opener} {quoted}"))
            }
            Target::Capture(capture) => match capture.command() {
                Some(command) => self.spawn(command),
                None if capture == crate::capture::Capture::Color => {
                    Task::perform(crate::capture::pick_color(), Message::Picked)
                }
                None if capture == crate::capture::Capture::Mark => {
                    return Task::batch([close, self.open_marks()]);
                }
                None => return Task::batch([close, self.toggle_recording()]),
            },
            Target::Copy(text) => {
                let content = clipboard::Content::Text(text);
                clipboard::copy(&content);
                clipboard::remember(&mut self.clips, Clip::new(content));
                Task::none()
            }
            Target::Mode(mode) => self.switch(mode),
            Target::Settings => self.spawn(&format!("{} settings", me())),
            Target::Keys => {
                return Task::batch([close, self.set_overlay(Some(Overlay::Keys))]);
            }
            Target::Clipboard => {
                return Task::batch([close, self.set_overlay(Some(Overlay::Clipboard))]);
            }
            Target::Power(power) => self.spawn(&power.command()),
        };
        Task::batch([close, task])
    }

    pub(super) fn pinned(&self) -> Vec<bar::Pinned> {
        let windows: Vec<(String, bool)> = match &self.niri {
            Some(state) => state
                .recent()
                .into_iter()
                .map(|w| (w.app_id.clone().unwrap_or_default(), w.is_focused))
                .collect(),
            None => self
                .status
                .windows
                .iter()
                .map(|(_, app, focused)| (app.clone(), *focused))
                .collect(),
        };
        self.config
            .pinned
            .iter()
            .map(|id| {
                let mine: Vec<bool> = windows
                    .iter()
                    .filter(|(app, _)| bar::app_key(&self.apps, app) == id)
                    .map(|(_, focused)| *focused)
                    .collect();
                bar::Pinned {
                    id: id.clone(),
                    running: !mine.is_empty(),
                    focused: mine.contains(&true),
                }
            })
            .collect()
    }

    pub(super) fn open_pinned(&mut self, id: &str) -> Task<Message> {
        let windows: Vec<(u64, bool)> = match &self.niri {
            Some(state) => state
                .recent()
                .into_iter()
                .filter(|w| bar::app_key(&self.apps, w.app_id.as_deref().unwrap_or_default()) == id)
                .map(|w| (w.id, w.is_focused))
                .collect(),
            None => self
                .status
                .windows
                .iter()
                .filter(|(_, app, _)| bar::app_key(&self.apps, app) == id)
                .map(|(window, _, focused)| (*window, *focused))
                .collect(),
        };
        let next = match windows.iter().position(|(_, focused)| *focused) {
            Some(at) => windows.get(at + 1).or(windows.first()),
            None => windows.first(),
        };
        if let Some((window, _)) = next {
            return self.update(Message::FocusWindow(*window));
        }
        match apps::find(&self.apps, id) {
            Some(app) => {
                let exec = app.exec.clone();
                self.spawn(&exec)
            }
            None => Task::none(),
        }
    }

    pub(super) fn open_windows(&self) -> Vec<OpenWindow> {
        match &self.niri {
            Some(state) => state
                .recent()
                .into_iter()
                .filter(|window| !window.is_focused)
                .map(|window| OpenWindow {
                    id: window.id,
                    title: window.title.clone().unwrap_or_default(),
                    app: window.app_id.clone().unwrap_or_default(),
                })
                .collect(),
            None => self
                .status
                .windows
                .iter()
                .filter(|(_, _, focused)| !focused)
                .map(|(id, app, _)| OpenWindow {
                    id: *id,
                    title: app.clone(),
                    app: app.clone(),
                })
                .collect(),
        }
    }

    pub(super) fn spawn(&self, command: &str) -> Task<Message> {
        if self.niri.is_some() {
            return niri::send(apolline_niri::Request::Spawn(command.to_owned()));
        }
        if self.demo && !command.ends_with(" settings") {
            println!("apolline: would run: {command}");
            return Task::none();
        }
        let line = format!("{command} &");
        Task::future(async move {
            if let Err(err) = tokio::process::Command::new("sh")
                .args(["-c", &line])
                .status()
                .await
            {
                eprintln!("apolline: {err}");
            }
        })
        .discard()
    }

    pub(super) fn nudge(&mut self, nudge: Nudge) -> Task<Message> {
        if self.sample() {
            let osd = nudge.sample(&mut self.controls);
            return self.show(osd);
        }
        Task::perform(controls::nudge(nudge), Message::Osd)
    }

    pub(super) fn idle(&mut self, stage: Stage) -> Task<Message> {
        match stage {
            Stage::Lock => self.lock(),
            Stage::ScreenOff => self.spawn("niri msg action power-off-monitors"),
            Stage::Suspend => self.spawn("systemctl suspend"),
        }
    }

    pub(super) fn lock(&mut self) -> Task<Message> {
        self.publish();
        let close = self.set_overlay(None);
        Task::batch([close, self.spawn(&Power::Lock.command())])
    }

    pub(super) fn player(&mut self, action: Action) -> Task<Message> {
        if self.system.is_none() {
            if let (Some(track), Action::Toggle) = (&mut self.status.media, action) {
                track.playing = !track.playing;
            }
            return Task::none();
        }
        match self.track() {
            Some(track) if !track.player.is_empty() => {
                Task::future(media::control(track.player.clone(), action)).discard()
            }
            _ => Task::none(),
        }
    }

    // Nothing typed or asked outlives the list it was typed in.
    pub(super) fn leave_detail(&mut self) {
        self.joining = None;
        self.asking = None;
        self.pin.clear();
        crate::bluetooth::refuse();
    }

    // A new conversation with polkit's helper, with a password already in when it was typed first.
    pub(super) fn ask_polkit(&mut self, password: Option<String>) -> Task<Message> {
        if self.sample() {
            return Task::none();
        }
        let Some(dialog) = &mut self.polkit else {
            return Task::none();
        };
        let (mut answer, answers) = iced::futures::channel::mpsc::channel(1);
        dialog.busy = password.is_some_and(|password| answer.try_send(password).is_ok());
        dialog.asks = None;
        dialog.answers = Some(answer);
        dialog.info = None;
        let user = dialog
            .request
            .users
            .get(dialog.user)
            .cloned()
            .unwrap_or_default();
        let cookie = dialog.request.cookie.clone();
        let (talk, handle) = Task::run(
            polkit::conversation(user, cookie.clone(), answers),
            move |said| Message::PolkitSaid(cookie.clone(), said),
        )
        .abortable();
        dialog.check = Some(handle.abort_on_drop());
        talk
    }

    pub(super) fn listing(&self, detail: panel::Detail) -> bool {
        self.overlay == Some(Overlay::Panel) && self.detail == Some(detail)
    }

    pub(super) fn read_controls(&self) -> Task<Message> {
        if self.sample() {
            return Task::none();
        }
        Task::perform(controls::read(), Message::Controls)
    }

    pub(super) fn toggle_recording(&mut self) -> Task<Message> {
        if self.recording.is_some() {
            return self.stop_recording();
        }
        let output = self.niri.as_ref().and_then(|n| n.focused_output());
        match crate::capture::Recording::start(output) {
            Ok(recording) => {
                self.recording = Some(recording);
                Task::none()
            }
            Err(err) => self.show(Osd::Note(err)),
        }
    }

    pub(super) fn stop_recording(&mut self) -> Task<Message> {
        let Some(recording) = self.recording.take() else {
            return Task::none();
        };
        let file = recording.stop();
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.show(Osd::Note(crate::i18n::fill(
            "Saved {name}",
            &[("name", &name)],
        )))
    }
}
