use super::*;

impl Settings {
    pub(super) fn place_windows(&mut self, message: Message) {
        let index = self.selected;
        match message {
            Message::RememberWindows => match self.windows_now() {
                Ok(windows) if !windows.is_empty() => {
                    self.edit(|e| e.set_mode_windows(index, &windows));
                    self.windows_note = None;
                }
                Ok(_) => self.windows_note = Some("No windows to remember".into()),
                Err(err) => self.windows_note = Some(err),
            },
            Message::ForgetWindow(at) => {
                if let Some(mode) = self.config.modes.get(index) {
                    let mut windows = mode.windows.clone();
                    if at < windows.len() {
                        windows.remove(at);
                        self.edit(|e| e.set_mode_windows(index, &windows));
                    }
                }
            }
            _ => {}
        }
    }

    // What niri shows right now, as a mode would put it back.
    fn windows_now(&self) -> Result<Vec<apolline_config::Placed>, String> {
        if !crate::niri::connected() {
            return Err(crate::i18n::t("This works inside a niri session").to_owned());
        }
        let ask = |what: &str| {
            std::process::Command::new("niri")
                .args(["msg", "-j", what])
                .output()
                .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
                .map_err(|err| format!("niri msg {what}: {err}"))
        };
        let windows = apolline_niri::windows(&ask("windows")?);
        let workspaces = apolline_niri::workspaces(&ask("workspaces")?);
        Ok(crate::places::remember(
            &windows,
            &workspaces,
            &crate::niri::outputs(),
            self.config.layout.gaps,
            |window| crate::places::run_of(window, &self.apps),
        ))
    }

    pub(super) fn mode_windows<'a>(&'a self, mode: &'a ModeSettings) -> Element<'a, Message> {
        let mut rows: Vec<Element<'_, Message>> = mode
            .windows
            .iter()
            .enumerate()
            .map(|(at, window)| {
                let mut spot = vec![
                    window
                        .screen
                        .clone()
                        .unwrap_or_else(|| crate::i18n::t("this screen").to_owned()),
                    crate::i18n::fill("workspace {n}", &[("n", &window.workspace)]),
                    crate::i18n::fill("column {n}", &[("n", &window.column)]),
                ];
                spot.extend(window.width.map(|w| format!("{w}%")));
                if window.run.is_none() {
                    spot.push(crate::i18n::t("only if open").to_owned());
                }
                setting_owned(
                    window.app.as_str(),
                    Some(spot.join(" · ")),
                    button(icon(Icon::X, 12.0, style::faint()))
                        .padding(5)
                        .style(style::ghost(6.0))
                        .on_press(Message::ForgetWindow(at)),
                )
            })
            .collect();
        let about = if mode.windows.is_empty() {
            "Lay the windows out as you like, then remember: the mode puts them back each time it comes on and starts what isn't open"
        } else {
            "What's open moves to its place, what isn't starts; anything else stays as it is"
        };
        rows.push(setting(
            "Remember the windows as they are",
            about,
            button(
                text(if mode.windows.is_empty() {
                    "Remember"
                } else {
                    "Remember again"
                })
                .size(13),
            )
            .padding([8, 14])
            .style(style::secondary)
            .on_press_maybe(crate::niri::connected().then_some(Message::RememberWindows)),
        ));
        if let Some(line) = &self.windows_note {
            rows.push(note(line));
        }
        group("Windows in their places", rows)
    }
}
