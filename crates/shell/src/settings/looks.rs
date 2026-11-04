use super::*;
use iced::widget::column;

impl Settings {
    pub(super) fn templates(&self) -> Element<'_, Message> {
        let card = |name: &'static str, template: ModeSettings, about: &'static str| {
            let accent = match template.accent {
                Some(Accent::Fixed(color)) => color,
                _ => auto_accent(
                    template
                        .wallpaper
                        .as_deref()
                        .unwrap_or(&self.config.desktop.wallpaper),
                ),
            };
            let palette = Palette::new(accent);
            button(
                column![
                    container(icon(Icon::named(&template.icon), 20.0, palette.accent))
                        .center(40)
                        .style(style::pill(palette.accent_soft)),
                    text(if name.is_empty() { "Empty" } else { name })
                        .size(15)
                        .font(style::semibold()),
                    text(about).size(12).color(style::color(style::faint())),
                ]
                .spacing(8),
            )
            .width(Fill)
            .padding(16)
            .style(|theme, status| {
                let mut style = style::secondary(theme, status);
                style.border = Border {
                    color: style::color(neutral::hairline()),
                    width: 1.0,
                    radius: style::RADIUS_TILE.into(),
                };
                style
            })
            .on_press(Message::AddMode(name))
        };
        let about = |name: &str| match name {
            "Work" => "Calm wallpaper, low-priority popups wait",
            "Play" => "Performance, no popups, the screen stays awake",
            "Evening" => "Night wallpaper, warm accent, everything gets through",
            "Focus" => "Only urgent popups, a violet accent",
            "Presentation" => "No popups and the screen never sleeps",
            _ => "",
        };
        let mut cards: Vec<Element<'_, Message>> = TEMPLATES
            .map(|name| card(name, ModeSettings::template(name), about(name)).into())
            .into();
        cards.push(card("", ModeSettings::blank(""), "Start from nothing").into());
        let mut grid = Column::new().spacing(12);
        let mut cards = cards.into_iter();
        loop {
            let chunk: Vec<Element<'_, Message>> = cards.by_ref().take(3).collect();
            if chunk.is_empty() {
                break;
            }
            grid = grid.push(Row::with_children(chunk).spacing(12));
        }
        let intro = if self.config.modes.is_empty() {
            "A mode changes the wallpaper, accent, notifications, apps, performance and keys in one go. You don't need any; when you want one, start here."
        } else {
            "Pick a starting point. Everything in it can be changed afterwards."
        };
        let mut page = column![
            text(intro).size(14).color(style::color(style::soft_text())),
            grid
        ]
        .spacing(18);
        if !self.config.modes.is_empty() {
            page = page.push(
                button(text("Cancel").size(13))
                    .padding([7, 14])
                    .style(style::secondary)
                    .on_press(Message::Adding(false)),
            );
        }
        page.into()
    }

    pub(super) fn theme_card<'a>(
        &'a self,
        index: usize,
        theme: &'a Look,
        mine: bool,
    ) -> Element<'a, Message> {
        let applied = self.config.desktop.theme.as_deref();
        let accent = match theme.accent {
            Accent::Fixed(color) => color,
            Accent::Auto => auto_accent(&theme.wallpaper),
            Accent::Palette => apolline_theme::palettes::named(&theme.palette)
                .map_or_else(|| auto_accent(&theme.wallpaper), |(_, accent)| accent),
        };
        let look = Palette::new(accent);
        let picture: Element<'a, Message> = match self.thumb(&theme.wallpaper) {
            Some(handle) => container(
                image(handle.clone())
                    .width(Fill)
                    .height(132)
                    .content_fit(ContentFit::Cover)
                    .border_radius(10.0),
            )
            .clip(true)
            .into(),
            None => container(space())
                .width(Fill)
                .height(132)
                .style(|_| container::background(style::color(neutral::ink())))
                .into(),
        };
        let bar = container(
            row![
                container(Space::new().width(14).height(4)).style(style::pill(look.accent)),
                container(Space::new().width(4).height(4))
                    .style(style::pill(neutral::text().alpha(0.6))),
                space::horizontal(),
                container(Space::new().width(22).height(4))
                    .style(style::pill(neutral::text().alpha(0.7))),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .padding([5, 8])
        .width(Fill)
        .style(move |_| {
            let mut style = style::glass(
                theme.bar.opacity,
                theme.bar.floating,
                apolline_theme::metrics::RADIUS_BAR,
            )(&Theme::Dark);
            if theme.bar.floating {
                style.border.radius = 7.0.into();
            }
            style
        });
        let inset = if theme.bar.floating { 6.0 } else { 0.0 };
        let window = container(space())
            .width(Length::FillPortion(3))
            .height(Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(style::color(
                    neutral::window().alpha(theme.opacity.unwrap_or(1.0) * 0.95),
                ))),
                border: Border {
                    color: style::color(if theme.layout.borders {
                        look.accent
                    } else {
                        Rgba::hex(0).alpha(0.0)
                    }),
                    width: 1.0,
                    radius: (f32::from(theme.layout.radius) * 0.5).into(),
                },
                ..container::Style::default()
            });
        let other = container(space())
            .width(Length::FillPortion(2))
            .height(Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(style::color(
                    neutral::window().alpha(theme.inactive_opacity.unwrap_or(1.0) * 0.9),
                ))),
                border: Border {
                    radius: (f32::from(theme.layout.radius) * 0.5).into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });
        let gap = f32::from(theme.layout.gaps) * 0.5;
        let windows = row![window, other].spacing(gap).padding(gap.max(2.0));
        let screen: Element<'_, Message> = match theme.bar.position {
            Edge::Top => column![container(bar).padding(inset), windows].into(),
            Edge::Bottom => column![windows, container(bar).padding(inset)].into(),
            // A side bar, as a thin strip of its glass beside the windows.
            Edge::Left | Edge::Right => {
                let strip = container(space().width(8).height(Fill))
                    .padding(inset)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(style::color(
                            neutral::ink().alpha(theme.bar.opacity),
                        ))),
                        border: Border::default().rounded(4),
                        ..container::Style::default()
                    });
                if theme.bar.position == Edge::Left {
                    row![strip, windows].into()
                } else {
                    row![windows, strip].into()
                }
            }
        };
        let on = applied == Some(theme.name.as_str());
        let mut caption = row![
            container(Space::new().width(10).height(10)).style(style::pill(look.accent)),
            text(theme.name.as_str()).size(14).font(style::semibold()),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        if let Some(author) = theme.author.as_ref().filter(|_| !on) {
            caption = caption.push(
                text(if mine {
                    "yours".to_owned()
                } else {
                    author.clone()
                })
                .size(12)
                .color(style::color(style::faint())),
            );
        }
        caption = caption.push(space::horizontal());
        if on {
            caption = caption.push(
                container(
                    text("Applied")
                        .size(11)
                        .font(style::semibold())
                        .color(style::color(look.on_accent)),
                )
                .padding([3, 9])
                .style(style::pill(look.accent)),
            );
        }
        button(
            column![
                stack![picture, container(screen).height(132)].height(132),
                caption
            ]
            .spacing(10),
        )
        .width(Fill)
        .padding(8)
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(Background::Color(style::color(match status {
                button::Status::Hovered | button::Status::Pressed => style::fill_color(),
                _ => style::sunken_color(),
            }))),
            text_color: style::color(neutral::text()),
            border: Border {
                color: style::color(if on { look.accent } else { neutral::hairline() }),
                width: if on { 2.0 } else { 1.0 },
                radius: style::RADIUS_TILE.into(),
            },
            ..button::Style::default()
        })
        .on_press(Message::ApplyTheme(index))
        .into()
    }

    pub(super) fn layers(&self, palette: Palette) -> Option<Element<'_, Message>> {
        let theme = self.config.desktop.theme.as_deref().and_then(|name| {
            self.themes
                .iter()
                .enumerate()
                .find(|(_, (t, _))| apolline_config::slug(&t.name) == apolline_config::slug(name))
        });
        let mode = self
            .live
            .mode
            .as_ref()
            .and_then(|key| self.config.modes.iter().find(|m| m.key() == *key));
        let over = mode.map(mode_overrides).filter(|o| !o.is_empty());
        if theme.is_none() && over.is_none() {
            return None;
        }
        let and = |words: &[&str]| match words.split_last() {
            Some((last, [])) => (*last).to_owned(),
            Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
            None => String::new(),
        };
        let tweaks = theme.map_or_else(Vec::new, |(_, (t, _))| {
            t.tweaks(&Look::of(&self.config, "now", None))
        });
        let mut words = Column::new().spacing(2).width(Fill);
        if let Some((_, (theme, _))) = theme {
            words = words.push(
                text(if tweaks.is_empty() {
                    format!("{} as it comes", theme.name)
                } else {
                    format!("{}, with your own {}", theme.name, and(&tweaks))
                })
                .size(13),
            );
        }
        if let (Some(mode), Some(over)) = (mode, &over) {
            let refs: Vec<&str> = over.iter().map(String::as_str).collect();
            words = words.push(
                text(format!("{} is on: {} for now", mode.name, and(&refs)))
                    .size(12)
                    .color(style::color(style::faint())),
            );
        }
        let mut line = row![icon(Icon::Layers, 15.0, palette.accent), words]
            .spacing(10)
            .align_y(Alignment::Center);
        if let Some((index, (theme, _))) = theme.filter(|_| !tweaks.is_empty()) {
            line = line.push(
                button(text(format!("Back to {}", theme.name)).size(12))
                    .padding([6, 12])
                    .style(style::secondary)
                    .on_press(Message::ApplyTheme(index)),
            );
        }
        Some(
            container(line)
                .padding([10, 14])
                .width(Fill)
                .style(style::pill(palette.accent_soft))
                .into(),
        )
    }

    pub(super) fn themes(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let mut gallery = Column::new().spacing(14);
        if let Some(layers) = self.layers(palette) {
            gallery = gallery.push(layers);
        }
        let cards: Vec<Element<'_, Message>> = self
            .themes
            .iter()
            .enumerate()
            .map(|(index, (theme, file))| self.theme_card(index, theme, file.is_some()))
            .collect();
        let mut cards = cards.into_iter();
        loop {
            let chunk: Vec<Element<'_, Message>> = cards.by_ref().take(3).collect();
            if chunk.is_empty() {
                break;
            }
            let missing = 3 - chunk.len();
            let mut line = Row::with_children(chunk).spacing(14);
            for _ in 0..missing {
                line = line.push(Space::new().width(Fill));
            }
            gallery = gallery.push(line);
        }

        let note: Element<'_, Message> = match &self.theme_note {
            Some(Ok(line)) => text(line.as_str())
                .size(12)
                .color(style::color(style::GOOD))
                .into(),
            Some(Err(line)) => text(line.as_str())
                .size(12)
                .color(style::color(style::WARN))
                .into(),
            None => space().into(),
        };
        column![
            gallery,
            group(
                "Make your own",
                vec![
                    setting(
                        "Save my look",
                        "Wallpaper, accent, corners, gaps, borders, shadows, opacity, animations and the bar",
                        row![
                            text_input("Name it", &self.theme_name)
                                .on_input(Message::ThemeName)
                                .on_submit(Message::SaveTheme)
                                .padding([8, 12])
                                .size(13)
                                .width(180)
                                .style(style::field(palette)),
                            button(text("Save").size(13))
                                .padding([8, 14])
                                .style(style::primary(palette))
                                .on_press_maybe(
                                    (!self.theme_name.trim().is_empty()).then_some(Message::SaveTheme)
                                ),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    ),
                    setting(
                        "Add someone's theme",
                        "A theme.toml, or a folder with one and its pictures",
                        row![
                            text_input("~/Downloads/look.toml", &self.theme_path)
                                .on_input(Message::ThemePath)
                                .on_submit(Message::ImportTheme)
                                .padding([8, 12])
                                .size(13)
                                .width(220)
                                .style(style::field(palette)),
                            button(
                                row![icon(Icon::Download, 14.0, neutral::text()), text("Add").size(13)]
                                    .spacing(6)
                                    .align_y(Alignment::Center)
                            )
                            .padding([8, 14])
                            .style(style::secondary)
                            .on_press_maybe(
                                (!self.theme_path.trim().is_empty()).then_some(Message::ImportTheme)
                            ),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    ),
                    setting(
                        "Themes folder",
                        "Share a look by sending its file",
                        button(
                            row![icon(Icon::Folder, 14.0, neutral::text()), text("Open").size(13)]
                                .spacing(6)
                                .align_y(Alignment::Center)
                        )
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press(Message::OpenThemes),
                    ),
                ]
                .into_iter()
                .chain(self.theme_note.is_some().then(|| padded(note)))
                .collect()
            ),
        ]
        .spacing(22)
        .into()
    }
}
