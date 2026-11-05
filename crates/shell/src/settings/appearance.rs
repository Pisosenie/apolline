use super::*;
use iced::widget::column;

impl Settings {
    pub(super) fn wallpaper_picker<'a>(
        &'a self,
        current: Option<&str>,
        inherit: bool,
        on: fn(Option<String>) -> Message,
        palette: Palette,
    ) -> Element<'a, Message> {
        let ring = move |on: bool| {
            move |_: &Theme, _| button::Style {
                background: None,
                border: Border {
                    color: style::color(if on {
                        palette.accent
                    } else {
                        neutral::hairline()
                    }),
                    width: if on { 2.0 } else { 1.0 },
                    radius: 12.0.into(),
                },
                ..button::Style::default()
            }
        };
        let mut tiles: Vec<Element<'a, Message>> = Vec::new();
        if inherit {
            let picture: Element<'a, Message> = match self.thumb(&self.config.desktop.wallpaper) {
                Some(handle) => stack![
                    image(handle.clone())
                        .width(Fill)
                        .height(88)
                        .content_fit(ContentFit::Cover)
                        .border_radius(9.0)
                        .opacity(0.45_f32),
                    container(text("Desktop's").size(12).font(style::medium())).center(Fill)
                ]
                .height(88)
                .into(),
                None => container(text("Desktop's").size(12))
                    .center_x(Fill)
                    .center_y(88)
                    .into(),
            };
            tiles.push(
                button(picture)
                    .width(Fill)
                    .padding(3)
                    .style(ring(current.is_none()))
                    .on_press(on(None))
                    .into(),
            );
        }
        let names: Vec<&str> = wallpaper::bundled()
            .iter()
            .map(|(name, _)| *name)
            .chain(self.library.iter().map(String::as_str))
            .collect();
        let limit = if self.all_walls { names.len() } else { WALLS };
        for name in names.iter().take(limit) {
            let picture: Element<'a, Message> = match self.thumb(name) {
                Some(handle) => container(
                    image(handle.clone())
                        .width(Fill)
                        .height(88)
                        .content_fit(ContentFit::Cover)
                        .border_radius(9.0),
                )
                .clip(true)
                .into(),
                None => container(space())
                    .width(Fill)
                    .height(88)
                    .style(|_| container::background(style::color(style::sunken_color())))
                    .into(),
            };
            tiles.push(
                button(picture)
                    .width(Fill)
                    .padding(3)
                    .style(ring(current == Some(*name)))
                    .on_press(on(Some((*name).to_owned())))
                    .into(),
            );
        }
        if names.len() > WALLS {
            let label = if self.all_walls {
                "Fewer".to_owned()
            } else {
                format!("+{} more", names.len() - WALLS)
            };
            tiles.push(
                button(container(text(label).size(13)).center_x(Fill).center_y(88))
                    .width(Fill)
                    .padding(3)
                    .style(ring(false))
                    .on_press(Message::AllWallpapers)
                    .into(),
            );
        }
        let mut grid = Column::new().spacing(12);
        let mut tiles = tiles.into_iter().peekable();
        while tiles.peek().is_some() {
            let mut line = Row::new().spacing(12);
            let mut count = 0;
            for tile in tiles.by_ref().take(PER_ROW) {
                line = line.push(tile);
                count += 1;
            }
            for _ in count..PER_ROW {
                line = line.push(Space::new().width(Fill));
            }
            grid = grid.push(line);
        }
        let walls = grid;
        let own = row![
            text_input(
                crate::i18n::t("A picture or a folder of them"),
                &self.wallpaper_draft
            )
            .on_input(Message::WallpaperDraft)
            .on_submit(Message::UseWallpaper)
            .padding([8, 12])
            .size(13)
            .style(style::field(palette)),
            button(text("Add").size(13))
                .padding([8, 16])
                .style(style::secondary)
                .on_press(Message::UseWallpaper),
        ]
        .spacing(8);
        let mut column = column![walls, own].spacing(14);
        if !self.config.desktop.folders.is_empty() {
            let folders = Row::with_children(self.config.desktop.folders.iter().enumerate().map(
                |(index, folder)| {
                    container(
                        row![
                            icon(Icon::Folder, 13.0, style::faint()),
                            text(folder.as_str()).size(12),
                            button(icon(Icon::X, 12.0, style::faint()))
                                .padding(2)
                                .style(style::ghost(6.0))
                                .on_press(Message::RemoveFolder(index)),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    )
                    .padding([4, 10])
                    .style(style::pill(Rgba::hex(0xffffff).alpha(0.06)))
                    .into()
                },
            ))
            .spacing(6)
            .wrap();
            column = column.push(folders);
        }
        if let Some(name) =
            current.filter(|name| !is_bundled(name) && !self.library.iter().any(|n| n == name))
        {
            column = column.push(
                row![
                    icon(Icon::Image, 14.0, style::faint()),
                    text(crate::i18n::fill(
                        "Using your file {name}",
                        &[("name", &name)]
                    ))
                    .size(12)
                    .color(style::color(style::faint())),
                ]
                .spacing(8),
            );
        }
        column.into()
    }

    pub(super) fn appearance(&self) -> Element<'_, Message> {
        let layout = self.config.layout;
        let palette = self.palette();
        let shown = self
            .monitor
            .as_ref()
            .and_then(|m| {
                self.config
                    .desktop
                    .outputs
                    .iter()
                    .find(|(output, _)| output == m)
            })
            .map_or(self.config.desktop.wallpaper.as_str(), |(_, picture)| {
                picture.as_str()
            });
        let mut screens: Vec<Element<'_, Message>> = Vec::new();
        if self.monitors.len() > 1 {
            let chips = Row::with_children(
                std::iter::once(None)
                    .chain(self.monitors.iter().map(Some))
                    .map(|monitor| {
                        let label = monitor.map_or("Every monitor", String::as_str);
                        button(text(label).size(12))
                            .padding([6, 12])
                            .style(style::choice(self.monitor.as_ref() == monitor))
                            .on_press(Message::ForMonitor(monitor.cloned()))
                            .into()
                    }),
            )
            .spacing(4)
            .wrap();
            screens.push(padded(chips));
        }
        screens.push(padded(self.wallpaper_picker(
            Some(shown),
            false,
            desktop_wallpaper,
            palette,
        )));
        let desktop = group(
            "Desktop",
            vec![
                column(screens).spacing(0).into(),
                setting(
                    "Change the wallpaper",
                    "Goes through the gallery above; the file keeps the one you picked",
                    segmented(
                        [
                            (0u32, "Never", None),
                            (15, "15 min", None),
                            (60, "Hourly", None),
                            (1440, "Daily", None),
                        ],
                        self.config.desktop.rotate,
                        Message::Rotate,
                    ),
                ),
                setting(
                    "Shuffle",
                    "Pick at random instead of in order",
                    switch(self.config.desktop.shuffle, Message::Shuffle, palette),
                ),
                padded(palette_picker(&self.config.desktop.palette, palette)),
                setting(
                    "Clock on the desktop",
                    "The time and the date, big, on the wallpaper",
                    iced::widget::pick_list(
                        CLOCK_SPOTS
                            .iter()
                            .map(|(_, name)| (*name).to_owned())
                            .collect::<Vec<_>>(),
                        Some(
                            CLOCK_SPOTS
                                .iter()
                                .find(|(key, _)| {
                                    Some(*key)
                                        == self.config.desktop.clock.map(apolline_config::Spot::key)
                                        || (*key == "off" && self.config.desktop.clock.is_none())
                                })
                                .map_or("Off", |(_, name)| name)
                                .to_owned(),
                        ),
                        Message::DesktopClock,
                    )
                    .text_size(13)
                    .padding([7, 12]),
                ),
                padded(accent_picker(
                    Some(self.config.desktop.accent),
                    auto_accent(&self.config.desktop.wallpaper),
                    false,
                    desktop_accent,
                    palette,
                )),
                setting(
                    "Pointer",
                    "Cursor theme and size, for niri and GTK apps",
                    row![
                        iced::widget::pick_list(
                            std::iter::once(SYSTEM_CURSOR.to_owned())
                                .chain(self.cursors.iter().cloned())
                                .collect::<Vec<_>>(),
                            Some(
                                self.config
                                    .desktop
                                    .cursor
                                    .clone()
                                    .unwrap_or_else(|| SYSTEM_CURSOR.to_owned()),
                            ),
                            Message::Cursor,
                        )
                        .text_size(13)
                        .padding([7, 12]),
                        segmented(
                            [(24u16, "24", None), (32, "32", None), (48, "48", None)],
                            self.config.desktop.cursor_size.unwrap_or(24),
                            Message::CursorSize,
                        ),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                ),
                setting(
                    "Font",
                    "For Apolline from its next start, and for GTK apps right away",
                    iced::widget::pick_list(
                        std::iter::once(BUILT_IN_FONT.to_owned())
                            .chain(self.families.iter().cloned())
                            .collect::<Vec<_>>(),
                        Some(
                            self.config
                                .desktop
                                .font
                                .clone()
                                .unwrap_or_else(|| BUILT_IN_FONT.to_owned()),
                        ),
                        Message::FontFamily,
                    )
                    .text_size(13)
                    .padding([7, 12]),
                ),
                setting(
                    "GTK theme",
                    "How GTK 3 apps draw their controls; GTK 4 apps follow Color apps",
                    iced::widget::pick_list(
                        std::iter::once(SYSTEM_CURSOR.to_owned())
                            .chain(self.gtk_themes.iter().cloned())
                            .collect::<Vec<_>>(),
                        Some(
                            self.config
                                .desktop
                                .gtk
                                .clone()
                                .unwrap_or_else(|| SYSTEM_CURSOR.to_owned()),
                        ),
                        Message::GtkTheme,
                    )
                    .text_size(13)
                    .padding([7, 12]),
                ),
                setting(
                    "Icons",
                    "The icon theme for app icons here and in GTK apps",
                    iced::widget::pick_list(
                        std::iter::once(SYSTEM_CURSOR.to_owned())
                            .chain(self.icon_themes.iter().cloned())
                            .collect::<Vec<_>>(),
                        Some(
                            self.config
                                .desktop
                                .icons
                                .clone()
                                .unwrap_or_else(|| SYSTEM_CURSOR.to_owned()),
                        ),
                        Message::Icons,
                    )
                    .text_size(13)
                    .padding([7, 12]),
                ),
                self.color_apps(palette),
            ],
        );
        let preview = self.preview(&self.config.desktop.wallpaper, 200.0, {
            let pane = |wide: bool| {
                let lines = column![
                    container(space())
                        .width(if wide { 140 } else { 80 })
                        .height(8)
                        .style(style::pill(Rgba::hex(0xffffff).alpha(0.16))),
                    container(space())
                        .width(if wide { 220 } else { 110 })
                        .height(6)
                        .style(style::pill(Rgba::hex(0xffffff).alpha(0.08))),
                    container(space())
                        .width(if wide { 180 } else { 90 })
                        .height(6)
                        .style(style::pill(Rgba::hex(0xffffff).alpha(0.08))),
                ]
                .spacing(8);
                let focused = wide;
                container(lines)
                    .padding(14)
                    .width(if wide {
                        Length::FillPortion(3)
                    } else {
                        Length::FillPortion(2)
                    })
                    .height(Fill)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(style::color(neutral::window()))),
                        border: Border {
                            color: style::color(match (layout.borders, focused) {
                                (true, true) => palette.accent,
                                (true, false) => Rgba::hex(0xffffff).alpha(0.08),
                                (false, _) => Rgba::hex(0).alpha(0.0),
                            }),
                            width: if layout.borders { 1.0 } else { 0.0 },
                            radius: f32::from(layout.radius).into(),
                        },
                        shadow: if layout.shadows {
                            Shadow {
                                color: iced::Color::from_rgba8(0, 0, 0, 0.45),
                                offset: Vector::new(0.0, 8.0),
                                blur_radius: 24.0,
                            }
                        } else {
                            Shadow::default()
                        },
                        ..container::Style::default()
                    })
            };
            row![pane(true), pane(false)]
                .spacing(f32::from(layout.gaps))
                .padding(f32::from(layout.gaps).max(4.0))
                .into()
        });

        column![
            self.layers(palette),
            preview,
            desktop,
            self.own_colors(),
            group(
                "Windows",
                vec![
                    setting(
                        "Gaps",
                        "Space between windows and around them",
                        value_slider(0..=32, layout.gaps, Message::Gaps, "px", palette),
                    ),
                    setting(
                        "Corner radius",
                        "How round window corners are",
                        value_slider(0..=24, layout.radius, Message::Radius, "px", palette),
                    ),
                    setting(
                        "Borders",
                        "A thin line around the focused window; a gradient flows across the workspace",
                        segmented(
                            [(0u8, "Off", None), (1, "Accent", None), (2, "Gradient", None)],
                            match (layout.borders, layout.gradient) {
                                (false, _) => 0,
                                (true, false) => 1,
                                (true, true) => 2,
                            },
                            Message::BorderStyle,
                        ),
                    ),
                    setting(
                        "Border width",
                        "One pixel is a hairline; four and up is a frame",
                        value_slider(1..=8, layout.border_width, Message::BorderWidth, "px", palette),
                    ),
                    setting(
                        "Shadows",
                        "Soft shadows under windows. Play mode always turns them off",
                        switch(layout.shadows, Message::Shadows, palette),
                    ),
                ]
            ),
            self.surfaces(palette),
        ]
        .spacing(22)
        .into()
    }

    pub(super) fn surfaces(&self, palette: Palette) -> Element<'_, Message> {
        let style = &self.config.style;
        let mut rows = vec![setting(
            "Launcher layout",
            "A list with details, or a grid of big icons moved through with the arrows",
            segmented(
                [(false, "List", None), (true, "Grid", None)],
                style.grid,
                Message::LauncherGrid,
            ),
        )];
        for look in Part::ALL {
            let surface = look.of(style);
            let (corners, glass) = look.defaults();
            rows.push(setting(
                look.name(),
                "Size, corners and how solid it is over what's behind",
                column![
                    value_slider(
                        80..=150,
                        (surface.size.unwrap_or(1.0) * 100.0).round() as u16,
                        move |v| Message::Style(look, Knob::Size, v),
                        "%",
                        palette,
                    ),
                    value_slider(
                        0..=40,
                        surface.radius.unwrap_or(corners),
                        move |v| Message::Style(look, Knob::Corners, v),
                        "px",
                        palette,
                    ),
                    value_slider(
                        30..=100,
                        (surface.opacity.unwrap_or(glass) * 100.0).round() as u16,
                        move |v| Message::Style(look, Knob::Glass, v),
                        "%",
                        palette,
                    ),
                    self.font_picker(look, surface.font),
                ]
                .spacing(6),
            ));
        }
        group("Launcher, quick settings and popups", rows)
    }

    // The fonts on this computer, or the one everything else uses.
    pub(super) fn font_picker(
        &self,
        look: Part,
        font: Option<apolline_config::FontName>,
    ) -> Element<'_, Message> {
        iced::widget::pick_list(
            std::iter::once(SAME_FONT.to_owned())
                .chain(self.families.iter().cloned())
                .collect::<Vec<_>>(),
            Some(
                font.map_or(SAME_FONT, apolline_config::FontName::name)
                    .to_owned(),
            ),
            move |family| Message::SurfaceFont(look, family),
        )
        .text_size(13)
        .padding([7, 12])
        .width(240)
        .into()
    }

    pub(super) fn color_apps(&self, palette: Palette) -> Element<'_, Message> {
        let on = self.config.apps.colors;
        let mut about =
            column![
                row![
                    column![
                    text("Color apps too").size(14),
                    text("GTK and Qt apps get the accent and the dark style, and follow your modes")
                        .size(12)
                        .color(style::color(style::faint())),
                ]
                    .spacing(3)
                    .width(Fill),
                    switch(on, Message::ColorApps, palette),
                ]
                .spacing(16)
                .align_y(Alignment::Center),
            ]
            .spacing(8);
        let heading = if on {
            "Writes, and removes when you turn it off:"
        } else {
            "Would write, and remove when you turn it off:"
        };
        about = about.push(text(heading).size(11).color(style::color(style::faint())));
        for file in crate::paint::files() {
            about = about.push(
                text(file)
                    .size(11)
                    .font(style::mono())
                    .color(style::color(style::faint())),
            );
        }
        padded(about)
    }
}

pub(super) fn desktop_wallpaper(name: Option<String>) -> Message {
    Message::DesktopWallpaper(name.unwrap_or_else(|| "fog.jpg".into()))
}

pub(super) fn desktop_accent(accent: Option<Accent>) -> Message {
    Message::DesktopAccent(accent.unwrap_or(Accent::Auto))
}

pub(super) fn is_bundled(name: &str) -> bool {
    wallpaper::bundled().iter().any(|(n, _)| *n == name)
}

pub(super) async fn thumbnails() -> Vec<(String, image::Handle)> {
    let make = || {
        wallpaper::bundled()
            .iter()
            .filter_map(|(name, bytes)| {
                let thumb = ::image::load_from_memory(bytes)
                    .ok()?
                    .thumbnail(760, 475)
                    .to_rgba8();
                let (width, height) = thumb.dimensions();
                Some((
                    (*name).to_owned(),
                    image::Handle::from_rgba(width, height, thumb.into_raw()),
                ))
            })
            .collect()
    };
    tokio::task::spawn_blocking(make).await.unwrap_or_default()
}

// The surfaces with a style of their own, by their [style.…] name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Part {
    Launcher,
    Panel,
    Cards,
    Osd,
    Bar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Knob {
    Size,
    Corners,
    Glass,
}

impl Part {
    const ALL: [Part; 4] = [Part::Launcher, Part::Panel, Part::Cards, Part::Osd];

    pub(super) fn key(self) -> &'static str {
        match self {
            Part::Launcher => "launcher",
            Part::Panel => "panel",
            Part::Cards => "notifications",
            Part::Osd => "osd",
            Part::Bar => "bar",
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Part::Launcher => "Launcher",
            Part::Panel => "Quick settings",
            Part::Cards => "Notifications",
            Part::Osd => "Volume and brightness",
            Part::Bar => "Bar",
        }
    }

    pub(super) fn of(self, style: &apolline_config::Styles) -> apolline_config::Surface {
        match self {
            Part::Launcher => style.launcher,
            Part::Panel => style.panel,
            Part::Cards => style.cards,
            Part::Osd => style.osd,
            Part::Bar => style.bar,
        }
    }

    // Corners and glass as each one comes.
    pub(super) fn defaults(self) -> (u16, f32) {
        match self {
            Part::Launcher | Part::Panel => (20, 0.95),
            Part::Cards => (16, 1.0),
            Part::Osd => (16, 0.95),
            Part::Bar => (apolline_theme::metrics::RADIUS_BAR as u16, 0.55),
        }
    }
}

pub(super) fn where_cards(corner: Corner) -> &'static str {
    match corner {
        Corner::TopLeft => "at the top left",
        Corner::Top => "at the top",
        Corner::TopRight => "at the top right",
        Corner::BottomLeft => "at the bottom left",
        Corner::Bottom => "at the bottom",
        Corner::BottomRight => "at the bottom right",
    }
}
