use super::*;
use iced::widget::column;

impl Shell {
    // The wallpapers the switcher shows, one for each mode and the plain desktop.
    pub(super) fn wallpapers(&self) -> Vec<String> {
        let mut names: Vec<String> = (0..self.config.modes.len())
            .map(|i| self.config.wallpaper(Some(i)).to_owned())
            .collect();
        names.push(self.config.desktop.wallpaper.clone());
        names.sort();
        names.dedup();
        names
    }

    // Thumbnails of wallpapers no longer in use are let go.
    pub(super) fn load_thumbs(&mut self) -> Task<Message> {
        let names = self.wallpapers();
        self.thumbs.retain(|name, _| names.contains(name));
        Task::batch(
            names
                .into_iter()
                .filter(|name| !self.thumbs.contains_key(name))
                .map(|name| {
                    Task::perform(wallpaper::thumbnail(name.clone()), move |thumb| {
                        Message::Thumb(name.clone(), thumb)
                    })
                }),
        )
    }

    pub(super) fn wallpaper_name(&self, output: Option<&str>) -> &str {
        if let Some(own) = self.current().and_then(|m| m.wallpaper.as_deref()) {
            return own;
        }
        output
            .and_then(|output| {
                self.config
                    .desktop
                    .outputs
                    .iter()
                    .find(|(name, _)| name == output)
            })
            .map_or(self.config.desktop.wallpaper.as_str(), |(_, picture)| {
                picture.as_str()
            })
    }

    pub(super) fn wanted_wallpapers(&self) -> Vec<String> {
        let mut names: Vec<String> = std::iter::once(None)
            .chain(self.outputs.iter().map(|o| Some(o.as_str())))
            .map(|output| self.wallpaper_name(output).to_owned())
            .filter(|name| name != apolline_config::NO_WALLPAPER)
            // A live one's picture is its poster or the preview its project came with.
            .filter(|name| {
                crate::live::source(name).is_none() || crate::live::picture(name).exists()
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }

    pub(super) fn rotate(&mut self) -> Task<Message> {
        let every = self.config.desktop.rotate;
        if self.demo || every == 0 || self.rotation_held() {
            return Task::none();
        }
        let since = self.rotated.as_ref().map_or(self.started, |(_, at)| *at);
        if since.elapsed() < Duration::from_secs(u64::from(every) * 60) {
            return Task::none();
        }
        let library = wallpaper::library(&self.config.desktop.folders);
        let current = &self.plain.desktop.wallpaper;
        let next = if self.config.desktop.shuffle && library.len() > 1 {
            let others: Vec<&String> = library.iter().filter(|name| *name != current).collect();
            let pick = Instant::now().elapsed().as_nanos() as usize
                ^ (Local::now().timestamp_subsec_nanos() as usize);
            others
                .get(pick % others.len().max(1))
                .map(|name| (*name).clone())
        } else {
            let at = library.iter().position(|name| name == current);
            library
                .get(at.map_or(0, |i| (i + 1) % library.len().max(1)))
                .cloned()
        };
        let Some(next) = next.filter(|next| next != current) else {
            self.rotated = Some((current.clone(), Instant::now()));
            return Task::none();
        };
        self.plain.desktop.wallpaper = next.clone();
        self.dress();
        self.rotated = Some((next, Instant::now()));
        self.sync();
        self.load_wallpaper()
    }

    // Each screen keeps its old picture until the new one has loaded, then fades over.
    pub(super) fn show_wallpapers(&mut self) {
        let screens: Vec<Option<String>> = std::iter::once(None)
            .chain(self.outputs.iter().cloned().map(Some))
            .collect();
        let mut shown = Vec::with_capacity(screens.len());
        // Without animations a new picture is just there.
        let fades = !self.demo && !self.speed().time(FADE).is_zero();
        for output in screens {
            let wanted = self.wallpaper_name(output.as_deref()).to_owned();
            let before = self
                .shown
                .iter()
                .position(|s| s.output == output)
                .map(|i| self.shown.swap_remove(i));
            let loaded = self.wallpapers.iter().any(|(n, _)| *n == wanted);
            match before {
                Some(before) if !loaded || before.name == wanted => shown.push(before),
                before => {
                    let from = before
                        .filter(|_| fades)
                        .and_then(|b| self.picture(&b.name).cloned())
                        .map(|old| (old, Instant::now()));
                    shown.push(Shown {
                        output,
                        name: wanted,
                        from,
                    });
                }
            }
        }
        self.shown = shown;
        let wanted = self.wanted_wallpapers();
        let shown = &self.shown;
        self.wallpapers
            .retain(|(n, _)| wanted.contains(n) || shown.iter().any(|s| s.name == *n));
        self.blurred.retain(|(n, _)| wanted.contains(n));
    }

    pub(super) fn picture(&self, name: &str) -> Option<&image::Handle> {
        self.wallpapers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, handle)| handle)
    }

    pub(super) fn load_wallpaper(&mut self) -> Task<Message> {
        self.fit_live();
        self.show_wallpapers();
        let wanted = self.wanted_wallpapers();
        let missing: Vec<String> = wanted
            .into_iter()
            .filter(|name| !self.wallpapers.iter().any(|(n, _)| n == name))
            .collect();
        Task::batch(missing.into_iter().map(|name| {
            Task::perform(wallpaper::load(name.clone()), move |result| {
                Message::Wallpaper(name.clone(), result)
            })
        }))
    }

    pub fn wallpaper(&self, output: Option<&str>) -> Element<'_, Message> {
        let picture = self.softened(self.wallpaper_name(output), self.wallpaper_picture(output));
        let shown = self.desktop_clock(self.config.desktop.clock);
        match shown {
            Some(shown) => stack![picture, shown].into(),
            None => picture,
        }
    }

    // The time and the date, big and light, on the wallpaper, with the widgets under them. With
    // no clock the widgets stand on their own in the bottom left.
    pub(super) fn desktop_clock(
        &self,
        spot: Option<apolline_config::Spot>,
    ) -> Option<Element<'_, Message>> {
        use apolline_config::{Corner, Spot};
        let clocked = spot.is_some();
        let spot = spot.unwrap_or(Spot::Corner(Corner::BottomLeft));
        let ink = apolline_theme::Scheme::DARK.text;
        let (x, y) = match spot {
            Spot::Center => (Alignment::Center, Alignment::Center),
            Spot::Corner(corner) => (
                match corner {
                    Corner::TopLeft | Corner::BottomLeft => Alignment::Start,
                    Corner::Top | Corner::Bottom => Alignment::Center,
                    Corner::TopRight | Corner::BottomRight => Alignment::End,
                },
                if corner.bottom() {
                    Alignment::End
                } else {
                    Alignment::Start
                },
            ),
        };
        let clock = column![
            text(
                self.now
                    .format(&crate::i18n::names(
                        self.config.clock.time(false),
                        &self.now
                    ))
                    .to_string(),
            )
            .size(120)
            .font(style::light())
            .line_height(0.95)
            .color(style::color(ink)),
            text(
                self.now
                    .format(&crate::i18n::names("%A, %-d %B", &self.now))
                    .to_string()
            )
            .size(18)
            .font(style::medium())
            .color(style::color(ink.alpha(0.8))),
        ]
        .spacing(2)
        .align_x(x);
        let widgets = self.desktop_widgets(x);
        let clock: Element<'_, Message> = match (clocked, widgets) {
            (true, Some(widgets)) => column![clock, widgets].spacing(18).align_x(x).into(),
            (true, None) => clock.into(),
            (false, Some(widgets)) => widgets,
            (false, None) => return None,
        };
        // Clear of the bar on its side, and of the screen's edges.
        let bar = style::under_bar(&self.config.bar, &self.config.style, 1.0) + 32.0;
        let padding = match self.config.bar.position {
            Edge::Top => Padding::new(56.0).top(bar),
            Edge::Bottom => Padding::new(56.0).bottom(bar),
            Edge::Left => Padding::new(56.0).left(bar),
            Edge::Right => Padding::new(56.0).right(bar),
        };
        Some(
            container(clock)
                .width(Fill)
                .height(Fill)
                .padding(padding)
                .align_x(x)
                .align_y(y)
                .into(),
        )
    }

    // Another program draws this screen's wallpaper under the shell's clock and widgets: one the
    // user runs, or apolline-wall for a live one.
    pub fn wallpaper_elsewhere_on(&self, output: Option<&str>) -> bool {
        self.wallpaper_name(output) == apolline_config::NO_WALLPAPER
            || self.live_source(output).is_some()
    }

    // Rotation changes the wallpaper of every screen without one of its own, and leaves it alone
    // while that one is drawn by another program.
    fn rotation_held(&self) -> bool {
        let shared = self.wallpaper_name(None);
        shared == apolline_config::NO_WALLPAPER || crate::live::source(shared).is_some()
    }

    pub(super) fn wallpaper_picture(&self, output: Option<&str>) -> Element<'_, Message> {
        if self.wallpaper_name(output) == apolline_config::NO_WALLPAPER
            || self.live_source(output).is_some()
        {
            return space().width(Fill).height(Fill).into();
        }
        let shown = self
            .shown
            .iter()
            .find(|s| s.output.as_deref() == output)
            .filter(|s| self.picture(&s.name).is_some())
            .or_else(|| self.shown.iter().find(|s| s.output.is_none()));
        let picture = |handle: &image::Handle, opacity: f32| {
            image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Cover)
                .opacity(opacity)
        };
        match shown.and_then(|s| Some((s, self.picture(&s.name)?))) {
            Some((shown, handle)) => match &shown.from {
                Some((old, at)) => {
                    let length = self.speed().time(FADE).max(Duration::from_millis(1));
                    let t = (at.elapsed().as_secs_f32() / length.as_secs_f32()).min(1.0);
                    let eased = 1.0 - (1.0 - t).powi(3);
                    stack![picture(old, 1.0), picture(handle, eased)].into()
                }
                None => picture(handle, 1.0).into(),
            },
            None => container(space())
                .width(Fill)
                .height(Fill)
                .style(|_| container::background(style::color(neutral::ink())))
                .into(),
        }
    }

    fn screens(&self) -> Vec<Option<String>> {
        if self.outputs.is_empty() {
            vec![None]
        } else {
            self.outputs.iter().cloned().map(Some).collect()
        }
    }

    // The live wallpaper a screen shows, unless apolline-wall can't draw it; then its poster
    // stands in as a picture. Worked out in fit_live, since it looks at the disk.
    pub(super) fn live_source(&self, output: Option<&str>) -> Option<&PathBuf> {
        self.live_here
            .iter()
            .find(|(screen, _)| screen.as_deref() == output)
            .map(|(_, source)| source)
    }

    pub(super) fn live_wanted(&self) -> bool {
        !self.live_here.is_empty()
    }

    // Tells apolline-wall what changed: a live wallpaper for each screen that has one, a pause
    // for a screen that went back to a picture, and the accent for its shaders.
    pub(super) fn fit_live(&mut self) {
        // Picking live wallpapers again is a fresh start: one that failed may be fixed now, and
        // apolline-wall may have been installed since it was missing.
        let names: Vec<String> = self
            .screens()
            .iter()
            .map(|output| self.wallpaper_name(output.as_deref()).to_owned())
            .filter(|name| crate::live::source(name).is_some())
            .collect();
        if names != self.live_names {
            self.live_names = names;
            self.live_failed.clear();
            self.live_gone = false;
        }
        self.live_here = if self.live_gone {
            Vec::new()
        } else {
            self.screens()
                .into_iter()
                .filter_map(|output| {
                    let source = crate::live::source(self.wallpaper_name(output.as_deref()))?;
                    (!self.live_failed.contains(&source)).then_some((output, source))
                })
                .collect()
        };
        if !self.live_wanted() {
            self.live = None;
            self.live_told.clear();
            self.live_accent = None;
            return;
        }
        let Some(commands) = self.live.clone() else {
            return;
        };
        let screens = self.screens();
        // Pauses go first, so one doesn't land after a show it would undo. What was told to
        // every screen before niri named them went to each of them, so once they have names
        // each one without a live wallpaper of its own is paused by name.
        let everywhere = self.live_told.contains_key(&None) && !screens.contains(&None);
        let mut lines: Vec<String> = self
            .live_told
            .keys()
            .filter(|output| output.is_some())
            .chain(screens.iter().filter(|_| everywhere))
            .filter(|output| {
                !screens.contains(output) || self.live_source(output.as_deref()).is_none()
            })
            .map(|output| crate::live::pause(output.as_deref(), true))
            .collect();
        lines.dedup();
        // A mode for games holds live wallpapers still, leaving the GPU to the game.
        let held = self.current().is_some_and(|mode| mode.performance);
        let hold_changed = held != self.live_held;
        self.live_held = held;
        let mut told = std::collections::BTreeMap::new();
        for (output, source) in self.live_here.clone() {
            let new = !self.live_told.contains_key(&output);
            if self.live_told.get(&output) != Some(&source) {
                lines.push(crate::live::show(output.as_deref(), &source));
            }
            if new || hold_changed {
                lines.push(crate::live::pause(output.as_deref(), held));
            }
            told.insert(output, source);
        }
        self.live_told = told;
        let accent = self.config.palette(self.mode).accent.to_hex();
        if self.live_accent.as_ref() != Some(&accent) {
            lines.push(crate::live::accent(&accent));
            self.live_accent = Some(accent);
        }
        for line in lines {
            let _ = commands.unbounded_send(line);
        }
    }

    pub(super) fn on_live(&mut self, event: crate::live::Event) -> Task<Message> {
        use crate::live::Event;
        match event {
            Event::Started(commands) => {
                self.live = Some(commands);
                self.live_told.clear();
                self.live_held = false;
                self.live_accent = None;
                self.fit_live();
                Task::none()
            }
            // A video whose frame couldn't be saved gives itself as the poster: nothing to keep.
            Event::Ready { source, poster } if poster == source => Task::none(),
            // Its frame becomes the picture behind the name: the accent, the switcher and the
            // lock screen use it, and it shows if the live one stops.
            Event::Ready { source, poster } => {
                let names: Vec<String> = self
                    .screens()
                    .iter()
                    .map(|output| self.wallpaper_name(output.as_deref()).to_owned())
                    .filter(|name| crate::live::source(name).as_ref() == Some(&source))
                    .collect();
                for name in names {
                    if crate::live::keep_poster(&name, &poster) {
                        self.wallpapers.retain(|(n, _)| *n != name);
                        self.thumbs.remove(&name);
                    }
                }
                Task::batch([self.load_wallpaper(), self.load_thumbs()])
            }
            Event::Failed { source, message } => {
                eprintln!("apolline: live wallpaper: {message}");
                match source {
                    Some(source) => {
                        self.live_failed.insert(source);
                        self.sync();
                        self.load_wallpaper()
                    }
                    None => Task::none(),
                }
            }
            Event::Gone(message) => {
                eprintln!("apolline: live wallpapers: {message}");
                self.live_gone = true;
                self.sync();
                self.load_wallpaper()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::futures::channel::mpsc;

    fn said(heard: &mut mpsc::UnboundedReceiver<String>) -> Vec<String> {
        std::iter::from_fn(|| heard.try_recv().ok()).collect()
    }

    #[test]
    fn apolline_wall_is_told_each_screen_once_and_pauses_for_pictures_and_games() {
        let dir = std::env::temp_dir().join(format!("apolline-desk-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("project.json"), "{}").unwrap();
        let scene = dir.to_str().unwrap().to_owned();
        let (mut shell, _) = Shell::new(None, true);
        shell.outputs = vec!["DP-1".into(), "HDMI-A-1".into()];
        shell.config.desktop.wallpaper = scene.clone();
        shell.config.desktop.outputs = vec![("HDMI-A-1".into(), "fog.jpg".into())];
        shell.mode = None;
        let (commands, mut heard) = mpsc::unbounded();
        let _ = shell.on_live(crate::live::Event::Started(commands));
        let first = said(&mut heard);
        assert_eq!(
            first[..2],
            [
                crate::live::show(Some("DP-1"), &dir),
                crate::live::pause(Some("DP-1"), false)
            ]
        );
        assert!(first[2].contains("uniforms"), "{first:?}");
        assert!(shell.wallpaper_elsewhere_on(Some("DP-1")));
        assert!(!shell.wallpaper_elsewhere_on(Some("HDMI-A-1")));
        assert!(shell.rotation_held());

        shell.fit_live();
        assert!(said(&mut heard).is_empty(), "nothing changed, nothing said");

        shell.config.modes = vec![ModeSettings {
            performance: true,
            ..ModeSettings::blank("Play")
        }];
        shell.mode = Some(0);
        shell.fit_live();
        assert_eq!(said(&mut heard), [crate::live::pause(Some("DP-1"), true)]);

        shell.mode = None;
        shell.config.desktop.wallpaper = "night.jpg".into();
        shell.fit_live();
        assert!(
            shell.live.is_none(),
            "no live wallpaper left, so the child goes"
        );
        assert!(!shell.wallpaper_elsewhere_on(Some("DP-1")));
        assert!(!shell.rotation_held());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn what_went_to_every_screen_is_paused_where_a_picture_shows_once_they_have_names() {
        let dir = std::env::temp_dir().join(format!("apolline-desk-early-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("project.json"), "{}").unwrap();
        let (mut shell, _) = Shell::new(None, true);
        shell.mode = None;
        shell.outputs = Vec::new();
        shell.config.desktop.wallpaper = dir.to_str().unwrap().to_owned();
        let (commands, mut heard) = mpsc::unbounded();
        let _ = shell.on_live(crate::live::Event::Started(commands));
        assert_eq!(said(&mut heard)[0], crate::live::show(None, &dir));
        shell.outputs = vec!["DP-1".into(), "HDMI-A-1".into()];
        shell.config.desktop.outputs = vec![("HDMI-A-1".into(), "fog.jpg".into())];
        shell.fit_live();
        let lines = said(&mut heard);
        assert_eq!(
            lines[0],
            crate::live::pause(Some("HDMI-A-1"), true),
            "{lines:?}"
        );
        assert!(lines.contains(&crate::live::show(Some("DP-1"), &dir)));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
