use super::*;
use iced::widget::column;

use crate::catalog::Entry;

impl Settings {
    pub(super) fn browse(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Browse => {
                self.catalog = Some(Err(String::new()));
                return Task::perform(crate::catalog::fetch(), Message::Catalog);
            }
            Message::Catalog(found) => {
                let pictures: Vec<Task<Message>> = found
                    .iter()
                    .flatten()
                    .map(|entry| {
                        let folder = entry.folder.clone();
                        Task::perform(crate::catalog::preview(entry.clone()), move |picture| {
                            Message::CatalogPicture(folder.clone(), picture)
                        })
                    })
                    .collect();
                self.catalog = Some(found);
                return Task::batch(pictures);
            }
            Message::CatalogPicture(folder, Some(picture)) => {
                self.catalog_pictures.insert(folder, picture);
            }
            Message::CatalogInstall(index) => {
                if let Some(Ok(entries)) = &self.catalog
                    && let Some(entry) = entries.get(index)
                {
                    self.catalog_busy = Some(entry.folder.clone());
                    return Task::perform(
                        crate::catalog::install(entry.clone()),
                        Message::CatalogInstalled,
                    );
                }
            }
            Message::CatalogInstalled(Ok(name)) => {
                self.catalog_busy = None;
                self.themes = themes::all();
                self.theme_note = Some(Ok(crate::i18n::fill(
                    "{theme} is with your themes now",
                    &[("theme", &name)],
                )));
                return self.scan_library();
            }
            Message::CatalogInstalled(Err(err)) | Message::CatalogTried(Err(err)) => {
                self.catalog_busy = None;
                self.theme_note = Some(Err(err));
            }
            Message::CatalogTry(index) => {
                let Some(Ok(entries)) = &self.catalog else {
                    return Task::none();
                };
                let Some(entry) = entries.get(index).cloned() else {
                    return Task::none();
                };
                if self.has_theme(&entry.name) {
                    return self.try_on(&entry.name, None);
                }
                self.catalog_busy = Some(entry.folder.clone());
                return Task::perform(crate::catalog::install(entry), Message::CatalogTried);
            }
            Message::CatalogTried(Ok(name)) => {
                self.catalog_busy = None;
                self.themes = themes::all();
                return Task::batch([
                    self.try_on(&name, Some(apolline_config::slug(&name))),
                    self.scan_library(),
                ]);
            }
            Message::KeepTheme => {
                crate::tryon::keep(&self.path);
                if let Some(trying) = self.trying.take() {
                    self.theme_note = Some(Ok(crate::i18n::fill(
                        "{theme} is yours now",
                        &[("theme", &trying.theme)],
                    )));
                }
            }
            Message::PutThemeBack => self.put_theme_back(),
            _ => {}
        }
        Task::none()
    }

    fn has_theme(&self, name: &str) -> bool {
        let wanted = apolline_config::slug(name);
        self.themes
            .iter()
            .any(|(theme, _)| apolline_config::slug(&theme.name) == wanted)
    }

    // Worn now, with the config as it was kept aside for a minute.
    fn try_on(&mut self, name: &str, fetched: Option<String>) -> Task<Message> {
        let wanted = apolline_config::slug(name);
        let Some((theme, _)) = self
            .themes
            .iter()
            .find(|(theme, _)| apolline_config::slug(&theme.name) == wanted)
            .cloned()
        else {
            return Task::none();
        };
        let before = std::fs::read_to_string(&self.path).unwrap_or_default();
        match crate::tryon::start(&self.path, &themes::dir(), &theme.name, &before, fetched) {
            Ok(trying) => {
                self.trying = Some(trying);
                self.themes = themes::all();
                self.theme_note = None;
                self.edit(|e| theme.apply(e));
                // Up to Keep and Go back, which sit at the top of the page, over the catalog.
                let top = iced::widget::operation::scroll_to(
                    super::find::PAGE,
                    iced::widget::operation::AbsoluteOffset {
                        x: None,
                        y: Some(0.0),
                    },
                );
                Task::batch([self.custom_thumb(), top])
            }
            Err(err) => {
                self.theme_note = Some(Err(err));
                Task::none()
            }
        }
    }

    pub(super) fn put_theme_back(&mut self) {
        self.trying = None;
        match crate::tryon::put_back(&self.path, &themes::dir()) {
            Ok(_) => self.theme_note = Some(Ok(crate::i18n::t("Back as it was").to_owned())),
            Err(err) => self.theme_note = Some(Err(err)),
        }
        self.themes = themes::all();
        self.reload();
    }

    // Each second: the minute is up, or the shell put it back after a crash.
    pub(super) fn watch_trial(&mut self) {
        let Some(trying) = &self.trying else {
            return;
        };
        if crate::tryon::pending(&self.path).is_none() {
            self.trying = None;
            self.themes = themes::all();
            self.reload();
        } else if trying.left().is_zero() {
            self.put_theme_back();
        }
    }

    pub(super) fn trial(&self) -> Option<Element<'_, Message>> {
        let trying = self.trying.as_ref()?;
        let palette = self.palette();
        Some(
            container(
                row![
                    icon(Icon::Sparkles, 16.0, palette.accent),
                    text(crate::i18n::fill(
                        "Wearing {theme}. Keep it? Going back in {seconds} s",
                        &[
                            ("theme", &trying.theme),
                            ("seconds", &trying.left().as_secs()),
                        ]
                    ))
                    .size(13)
                    .width(Fill),
                    button(text("Go back").size(13))
                        .padding([7, 14])
                        .style(style::secondary)
                        .on_press(Message::PutThemeBack),
                    button(text("Keep").size(13))
                        .padding([7, 16])
                        .style(style::primary(palette))
                        .on_press(Message::KeepTheme),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding([10, 14])
            .style(style::pill(palette.accent_soft))
            .into(),
        )
    }

    pub(super) fn catalog(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let browse = |label: &'static str| {
            button(
                row![
                    icon(Icon::Search, 14.0, neutral::text()),
                    text(label).size(13)
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([8, 14])
            .style(style::secondary)
            .on_press(Message::Browse)
        };
        let rows = match &self.catalog {
            None => vec![setting(
                "Themes people made",
                "With their wallpapers. Nothing is fetched until you ask",
                browse("Browse"),
            )],
            Some(Err(err)) if err.is_empty() => vec![note("Looking…")],
            Some(Err(err)) => vec![
                note(err),
                setting("Couldn't get the catalog", "", browse("Try again")),
            ],
            Some(Ok(entries)) if entries.is_empty() => vec![note("The catalog is empty for now")],
            Some(Ok(entries)) => {
                let cards: Vec<Element<'_, Message>> = entries
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| self.catalog_card(index, entry, palette))
                    .collect();
                let mut grid = Column::new().spacing(14);
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
                    grid = grid.push(line);
                }
                vec![padded(grid)]
            }
        };
        group("More themes", rows)
    }

    fn catalog_card<'a>(
        &'a self,
        index: usize,
        entry: &'a Entry,
        palette: Palette,
    ) -> Element<'a, Message> {
        let picture: Element<'a, Message> = match self.catalog_pictures.get(&entry.folder) {
            Some(handle) => container(
                image(handle.clone())
                    .width(Fill)
                    .height(120)
                    .content_fit(ContentFit::Cover)
                    .border_radius(10.0),
            )
            .clip(true)
            .into(),
            None => container(space())
                .width(Fill)
                .height(120)
                .style(|_| container::background(style::color(neutral::ink())))
                .into(),
        };
        let installed = self.has_theme(&entry.name);
        let busy = self.catalog_busy.as_deref() == Some(entry.folder.as_str());
        let on_trial = self
            .trying
            .as_ref()
            .is_some_and(|t| apolline_config::slug(&t.theme) == apolline_config::slug(&entry.name));
        let action: Element<'a, Message> = if on_trial {
            text("On trial")
                .size(12)
                .color(style::color(palette.accent))
                .into()
        } else if installed {
            row![
                icon(Icon::Check, 13.0, style::faint()),
                text("Installed")
                    .size(12)
                    .color(style::color(style::faint()))
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .into()
        } else {
            let free = self.catalog_busy.is_none();
            row![
                button(text("Try on").size(12))
                    .padding([6, 12])
                    .style(style::secondary)
                    .on_press_maybe(free.then_some(Message::CatalogTry(index))),
                button(text(if busy { "Fetching…" } else { "Install" }).size(12))
                    .padding([6, 12])
                    .style(style::primary(palette))
                    .on_press_maybe(free.then_some(Message::CatalogInstall(index))),
            ]
            .spacing(6)
            .into()
        };
        let mut card = column![
            picture,
            row![
                text(entry.name.as_str())
                    .size(14)
                    .font(style::medium())
                    .width(Fill),
                action
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        ]
        .spacing(8);
        let byline = [entry.author.as_str(), entry.about.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        if !byline.is_empty() {
            card = card.push(text(byline).size(12).color(style::color(style::faint())));
        }
        container(card)
            .padding(10)
            .width(Fill)
            .style(style::surface)
            .into()
    }
}
