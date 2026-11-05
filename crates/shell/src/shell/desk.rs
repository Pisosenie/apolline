use super::*;
use iced::widget::column;

impl Shell {
    pub(super) fn load_thumbs(&self) -> Task<Message> {
        let mut names: Vec<String> = (0..self.config.modes.len())
            .map(|i| self.config.wallpaper(Some(i)).to_owned())
            .collect();
        names.push(self.config.desktop.wallpaper.clone());
        names.sort();
        names.dedup();
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
            .collect();
        names.sort();
        names.dedup();
        names
    }

    pub(super) fn rotate(&mut self) -> Task<Message> {
        let every = self.config.desktop.rotate;
        if self.demo || every == 0 {
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
                        .filter(|_| !self.demo)
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
    }

    pub(super) fn picture(&self, name: &str) -> Option<&image::Handle> {
        self.wallpapers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, handle)| handle)
    }

    pub(super) fn load_wallpaper(&mut self) -> Task<Message> {
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
        let picture = self.wallpaper_picture(output);
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

    pub(super) fn wallpaper_picture(&self, output: Option<&str>) -> Element<'_, Message> {
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
                    let t = (at.elapsed().as_secs_f32() / FADE.as_secs_f32()).min(1.0);
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
}
