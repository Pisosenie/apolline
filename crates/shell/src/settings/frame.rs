use super::*;
use iced::widget::column;

impl Settings {
    pub(super) fn view(&self) -> Element<'_, Message> {
        responsive(move |size| {
            let wide = size.width >= WIDE;
            let compact = size.width < NARROW;
            let content = match (&self.editor, self.page) {
                (Err(err), page) if page != Page::About => broken(err),
                _ if !self.query.trim().is_empty() => self.results(),
                (_, page) => self.spotlight(|| self.page_view(page, wide)),
            };
            let padding = if wide { [28, 36] } else { [22, 24] };
            container(row![
                self.sidebar(compact),
                container(content).padding(padding).width(Fill)
            ])
            .width(Fill)
            .height(Fill)
            .style(|_| container::background(style::color(neutral::window())))
            .into()
        })
        .into()
    }

    pub(super) fn form(&self, page: Page) -> Element<'_, Message> {
        match page {
            Page::Modes => self.modes(),
            Page::Themes => self.themes(),
            Page::Appearance => self.appearance(),
            Page::Windows => self.windows(),
            Page::Displays => self.displays(),
            Page::Bar => self.bar(),
            Page::Lock => self.lock(),
            Page::Notifications => self.notifications(),
            Page::Keyboard => self.keyboard(),
            Page::Pointer => self.pointer(),
            Page::Shortcuts => self.shortcuts(),
            Page::About => self.about(),
            Page::Welcome => self.welcome(),
        }
    }

    pub(super) fn page_view(&self, page: Page, wide: bool) -> Element<'_, Message> {
        let form = self.form(page);
        let writes = match (&self.editor, page) {
            (_, Page::About | Page::Welcome) => None,
            (Ok(editor), Page::Modes) if self.selected < self.config.modes.len() => {
                Some(editor.mode_section(self.selected))
            }
            (Ok(_), Page::Modes) => Some(String::new()),
            (Ok(editor), Page::Displays) => Some(format!(
                "{}\n{}",
                editor.lists("display"),
                editor.sections(page.sections())
            )),
            (Ok(editor), Page::Bar) => Some(format!(
                "{}\n{}",
                editor.sections(page.sections()),
                editor.lists("module")
            )),
            (Ok(editor), Page::Windows) => Some(format!(
                "{}\n{}",
                editor.sections(page.sections()),
                editor.lists("rule")
            )),
            (Ok(editor), page) => Some(editor.sections(page.sections())),
            (Err(_), _) => None,
        };
        let mut header = row![
            column![
                text(page.title()).size(26).font(style::semibold()),
                text(page.subtitle())
                    .size(14)
                    .color(style::color(style::faint())),
            ]
            .spacing(4)
            .width(Fill)
        ]
        .align_y(Alignment::Start);
        if self.editor.is_ok() && !matches!(page, Page::About | Page::Welcome) {
            let history = |glyph, label, enabled: bool, message| {
                let tint = if enabled {
                    neutral::text()
                } else {
                    style::faint()
                };
                button(
                    row![
                        icon(glyph, 14.0, tint),
                        text(label).size(13).color(style::color(tint))
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([7, 12])
                .style(style::ghost(8.0))
                .on_press_maybe(enabled.then_some(message))
            };
            header = header.push(
                row![
                    history(Icon::Undo, "Undo", !self.undo.is_empty(), Message::Undo),
                    history(Icon::Redo, "Redo", !self.redo.is_empty(), Message::Redo),
                ]
                .spacing(4),
            );
        }
        let body: Element<'_, Message> = match writes {
            Some(code) if wide => row![
                scrollable(form)
                    .id(find::PAGE)
                    .direction(style::thin())
                    .style(style::scroller)
                    .height(Fill)
                    .width(Fill),
                self.writes(code),
            ]
            .spacing(16)
            .into(),
            Some(code) => {
                scrollable(container(column![form, self.writes(code)].spacing(22)).padding([0, 16]))
                    .id(find::PAGE)
                    .height(Fill)
                    .into()
            }
            None => scrollable(container(form).padding([0, 16]))
                .id(find::PAGE)
                .height(Fill)
                .into(),
        };
        column![header, body].spacing(22).into()
    }

    pub(super) fn sidebar(&self, compact: bool) -> Element<'_, Message> {
        let palette = self.palette();
        let mut brand = row![
            container(icon(Icon::Sparkles, 16.0, palette.on_accent))
                .center(32)
                .style(style::pill(palette.accent)),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        if !compact {
            brand = brand.push(column![
                text("Apolline").size(15).font(style::semibold()),
                text("Settings")
                    .size(12)
                    .color(style::color(style::faint())),
            ]);
        }

        let nav = Column::with_children(Page::ALL.map(|page| {
            let on = self.page == page;
            let tint = if on {
                palette.accent
            } else {
                style::soft_text()
            };
            let mut label = row![icon(page.icon(), 16.0, tint)]
                .spacing(12)
                .align_y(Alignment::Center);
            if !compact {
                label = label.push(text(page.title()).size(14).font(if on {
                    style::medium()
                } else {
                    Font::with_name(crate::style::family())
                }));
            }
            button(label)
                .width(Fill)
                .padding([9, 12])
                .style(move |theme, status| {
                    let mut style = style::ghost(10.0)(theme, status);
                    if on {
                        style.background =
                            Some(Background::Color(style::color(style::fill_color())));
                    }
                    style.text_color = style::color(if on {
                        neutral::text()
                    } else {
                        style::soft_text()
                    });
                    style
                })
                .on_press(Message::Page(page))
                .into()
        }))
        .spacing(2);

        let problems = self.diagnostics.len();
        let health = if compact {
            row![if problems == 0 {
                icon(Icon::Done, 14.0, style::GOOD)
            } else {
                icon(Icon::Warning, 14.0, style::WARN)
            }]
        } else if problems == 0 {
            row![
                icon(Icon::Done, 14.0, style::GOOD),
                text("Config is clean")
                    .size(12)
                    .color(style::color(style::faint())),
            ]
        } else {
            row![
                icon(Icon::Warning, 14.0, style::WARN),
                text(crate::i18n::count(
                    problems,
                    "1 problem in the config",
                    "{n} problems in the config",
                ))
                .size(12)
                .color(style::color(style::WARN)),
            ]
        };
        let footer = button(health.spacing(8).align_y(Alignment::Center))
            .padding([8, 12])
            .width(Fill)
            .style(style::ghost(10.0))
            .on_press(Message::Page(Page::About));

        let mut top = column![brand, space().height(18)].spacing(2);
        if !compact {
            top = top.push(
                text_input(crate::i18n::t("Search settings"), &self.query)
                    .on_input(Message::Find)
                    .padding([8, 12])
                    .size(13)
                    .style(style::field(palette)),
            );
            top = top.push(space().height(10));
        }
        container(column![top, nav, space::vertical(), footer].spacing(2))
            .width(if compact { 68 } else { 240 })
            .height(Fill)
            .padding([20, 14])
            .style(|_| container::Style {
                background: Some(Background::Color(style::color(
                    style::sunken_color().over(neutral::window()),
                ))),
                border: Border {
                    color: style::color(neutral::hairline()),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            })
            .into()
    }

    pub(super) fn preview<'a>(
        &'a self,
        name: &str,
        height: f32,
        overlay: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let picture: Element<'a, Message> = match self.thumb(name) {
            Some(handle) => container(
                image(handle.clone())
                    .width(Fill)
                    .height(height)
                    .content_fit(ContentFit::Cover)
                    .border_radius(14.0),
            )
            .clip(true)
            .into(),
            None => container(space())
                .width(Fill)
                .height(height)
                .style(|_| container::Style {
                    background: Some(Background::Color(style::color(neutral::ink()))),
                    border: Border {
                        radius: 14.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .into(),
        };
        container(stack![picture, container(overlay).height(height)])
            .padding(1)
            .style(|_| container::Style {
                border: Border {
                    color: style::color(neutral::hairline()),
                    width: 1.0,
                    radius: 15.0.into(),
                },
                ..container::Style::default()
            })
            .into()
    }

    pub(super) fn writes(&self, code: String) -> Element<'_, Message> {
        let palette = self.palette();
        let code = code.trim().to_owned();
        let code = if code.is_empty() {
            "# nothing set yet, defaults apply".to_owned()
        } else {
            code
        };
        let mut panel = column![
            row![
                icon(Icon::Code, 15.0, style::faint()),
                text("config.toml").size(13).font(style::medium()),
                space::horizontal(),
                text("live").size(11).color(style::color(style::GOOD)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            highlight(&code, palette),
            text("Saved as you change it. Your own comments in the file stay.")
                .size(12)
                .color(style::color(style::faint())),
        ]
        .spacing(14);
        if let Some(error) = &self.error {
            panel = panel.push(
                text(error.as_str())
                    .size(12)
                    .color(style::color(style::WARN)),
            );
        }
        container(scrollable(panel))
            .width(Length::Fixed(340.0))
            .padding(18)
            .style(|_| container::Style {
                background: Some(Background::Color(style::color(style::sunken_color()))),
                border: Border {
                    color: style::color(neutral::hairline()),
                    width: 1.0,
                    radius: style::RADIUS_TILE.into(),
                },
                ..container::Style::default()
            })
            .into()
    }
}
