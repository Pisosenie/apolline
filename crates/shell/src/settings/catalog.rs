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
            Message::CatalogInstalled(Err(err)) => {
                self.catalog_busy = None;
                self.theme_note = Some(Err(err));
            }
            _ => {}
        }
        Task::none()
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
        let installed = self
            .themes
            .iter()
            .any(|(theme, _)| theme.name == entry.name);
        let busy = self.catalog_busy.as_deref() == Some(entry.folder.as_str());
        let action: Element<'a, Message> = if installed {
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
            button(text(if busy { "Installing…" } else { "Install" }).size(12))
                .padding([6, 12])
                .style(style::primary(palette))
                .on_press_maybe(
                    (self.catalog_busy.is_none()).then_some(Message::CatalogInstall(index)),
                )
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
