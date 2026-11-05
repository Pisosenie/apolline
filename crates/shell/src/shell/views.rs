use super::*;

impl Shell {
    pub fn bar_edge(&self) -> Edge {
        self.config.bar.position
    }

    pub fn osd_view(&self) -> Option<Element<'_, Message>> {
        style::within(self.config.style.osd.font, || self.built_osd())
    }

    pub(super) fn built_osd(&self) -> Option<Element<'_, Message>> {
        self.osd
            .as_ref()
            .map(|(osd, _)| osd::view(osd, self.config.palette(self.mode), self.config.style.osd))
    }

    #[cfg(target_os = "linux")]
    pub fn size_of(&self, overlay: Overlay) -> f32 {
        let style = &self.config.style;
        match overlay {
            Overlay::Launcher => style.launcher.size,
            Overlay::Panel => style.panel.size,
            _ => None,
        }
        .unwrap_or(1.0)
    }

    pub fn overlay(&self) -> Option<Overlay> {
        self.overlay
    }

    pub fn overlay_view(&self) -> Element<'_, Message> {
        style::within(self.overlay_font(), || self.built_overlay())
    }

    // Quick settings and what opens from the bar take their font; the rest keep the shell's.
    pub(super) fn overlay_font(&self) -> Option<apolline_config::FontName> {
        match self.overlay {
            Some(Overlay::Launcher) => self.config.style.launcher.font,
            Some(Overlay::Panel | Overlay::Calendar | Overlay::TrayMenu) => {
                self.config.style.panel.font
            }
            _ => None,
        }
    }

    pub(super) fn built_overlay(&self) -> Element<'_, Message> {
        let palette = self.config.palette(self.mode);
        match self.overlay {
            Some(Overlay::Switcher) => switcher::view(self.mode, &self.config, &self.thumbs),
            Some(Overlay::Launcher) => self.launcher.view(
                palette,
                self.current()
                    .map(|m| (m.name.as_str(), crate::icon::Icon::named(&m.icon))),
            ),
            Some(Overlay::Clipboard) => self.picker.view(&self.clips, palette),
            Some(Overlay::Keys) => keymap::view(&self.keymap, &self.keys_query, palette),
            Some(Overlay::Marks) => match &self.marks {
                Some(marks) => crate::marks::view(marks, palette),
                None => space().into(),
            },
            Some(Overlay::TrayMenu) => match &self.tray_menu {
                Some((item, entries)) => crate::tray_menu::view(
                    item,
                    entries,
                    palette,
                    self.config.bar.position,
                    style::under_bar(&self.config.bar, &self.config.style, 1.0),
                ),
                None => space().into(),
            },
            Some(Overlay::Calendar) => crate::calendar::view(
                self.now,
                self.month,
                self.day,
                &self.agenda,
                self.weather.as_ref(),
                palette,
                self.config.bar.position,
                style::under_bar(&self.config.bar, &self.config.style, 1.0),
                self.config.calendars.len(),
            ),
            Some(Overlay::Polkit) => match &self.polkit {
                Some(dialog) => polkit::view(dialog, palette),
                None => space().into(),
            },
            Some(Overlay::Panel) => panel::view(Panel {
                edge: self.config.bar.position,
                gap: style::under_bar(
                    &self.config.bar,
                    &self.config.style,
                    self.config.style.panel.size.unwrap_or(1.0),
                ),
                modes: &self.config.modes,
                mode: self.mode,
                palette,
                controls: &self.controls,
                battery: self.battery(),
                track: self.track(),
                do_not_disturb: self.config.alerts.do_not_disturb,
                power_menu: self.power_menu,
                armed: self.armed,
                awake: self.awake,
                look: self.config.style.panel,
                night: self.night.1.unwrap_or(self.night.0),
                profiles: self.profiles.as_ref(),
                quiet_until: self.quiet_until,
                history: &self.history,
                opened: &self.opened,
                apps: &self.apps,
                detail: self.detail,
                networks: &self.networks,
                gadgets: &self.gadgets,
                joining: self.joining.as_ref(),
                asking: self.asking.as_ref(),
                pin: &self.pin,
                note: self.net_note.as_deref(),
                busy: self.net_busy,
                layout: &self.config.panel,
                tunnels: &self.tunnels,
            }),
            None => space().into(),
        }
    }

    // At the bottom of the screen the newest card is the one nearest the edge.
    pub fn popups(&self) -> Element<'_, Message> {
        style::within(self.config.style.cards.font, || self.built_popups())
    }

    pub(super) fn built_popups(&self) -> Element<'_, Message> {
        let mut cards = self.shown_cards();
        if self.config.alerts.position.bottom() {
            cards.reverse();
        }
        popup::stack(
            cards,
            &self.apps,
            self.config.palette(self.mode),
            self.replying.as_ref(),
            self.config.style.cards,
            self.config.alerts.width,
        )
    }

    #[cfg(target_os = "linux")]
    // Where the cards go, how big they're drawn, and how wide they are.
    pub fn cards_place(&self) -> (apolline_config::Corner, f32, u16) {
        let size = self.config.style.cards.size.unwrap_or(1.0);
        (self.config.alerts.position, size, self.config.alerts.width)
    }

    #[cfg(target_os = "linux")]
    pub fn osd_size(&self) -> f32 {
        self.config.style.osd.size.unwrap_or(1.0)
    }

    #[cfg(target_os = "linux")]
    // Where the bar is, whether it floats, and how much bigger it is drawn.
    pub fn bar_look(&self) -> (Edge, bool, u16) {
        let size = self.config.style.bar.size.unwrap_or(1.0);
        (
            self.config.bar.position,
            self.config.bar.floating,
            (size * 100.0).round() as u16,
        )
    }

    pub fn bar(&self, output: Option<&str>) -> Element<'_, Message> {
        style::within(self.config.style.bar.font, || self.built_bar(output))
    }

    pub(super) fn built_bar(&self, output: Option<&str>) -> Element<'_, Message> {
        let (status, active) = match &self.niri {
            Some(state) => {
                let status = Status::from_niri(state, output);
                let active = status.active;
                (status, active)
            }
            None => (self.status.clone(), self.workspace),
        };
        bar::view(Bar {
            mode: self.current(),
            has_modes: !self.config.modes.is_empty(),
            palette: self.config.palette(self.mode),
            active,
            status,
            battery: self.battery(),
            track: self.track(),
            controls: &self.controls,
            held: self.held.len(),
            do_not_disturb: self.config.alerts.do_not_disturb,
            awake: self.awake,
            now: self.now,
            clock: &self.config.clock,
            look: &self.config.bar,
            radius: style::bar_size(&self.config.style).1,
            panel_open: self.overlay == Some(Overlay::Panel),
            apps: &self.apps,
            pinned: self.pinned(),
            tray: &self.tray,
            icons: self.config.desktop.icons.as_deref(),
            dragging: self
                .dragging
                .as_ref()
                .map(|(app, was_pinned, x)| (app.as_str(), *was_pinned, *x)),
            recording: self.recording.as_ref().map(|r| r.started.elapsed()),
            stats: self.stats,
            next: self
                .agenda
                .soon(self.now.naive_local())
                .map(|(o, wait)| (&*o.summary, wait)),
            modules: self
                .module_text
                .iter()
                .filter(|(i, _)| *i < self.config.modules.len())
                .map(|(i, text)| bar::Module {
                    index: *i,
                    name: self.config.modules[*i].name.as_deref(),
                    text,
                    clickable: self.config.modules[*i].click.is_some(),
                })
                .collect(),
        })
    }

    pub fn bar_margin(&self) -> Padding {
        let look = &self.config.bar;
        let inset = if look.floating {
            metrics::BAR_INSET
        } else {
            0.0
        };
        match look.position {
            Edge::Top => Padding::new(inset).bottom(0),
            Edge::Bottom => Padding::new(inset).top(0),
            Edge::Left => Padding::new(inset).right(0),
            Edge::Right => Padding::new(inset).left(0),
        }
    }
}
