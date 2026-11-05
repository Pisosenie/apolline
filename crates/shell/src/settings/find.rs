use std::cell::RefCell;

use super::*;
use iced::widget::column;

pub(super) const PAGE: &str = "settings-page";
pub(super) const LIT: &str = "settings-spot";

// Where the lit row sits in the page's scrolled content, a little room above it.
#[derive(Default)]
struct Locate {
    content: Option<iced::Rectangle>,
    row: Option<iced::Rectangle>,
}

impl iced::advanced::widget::Operation<f32> for Locate {
    fn traverse(
        &mut self,
        operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation<f32>),
    ) {
        operate(self);
    }

    fn container(&mut self, id: Option<&iced::advanced::widget::Id>, bounds: iced::Rectangle) {
        if id == Some(&iced::advanced::widget::Id::new(LIT)) {
            self.row = Some(bounds);
        }
    }

    fn scrollable(
        &mut self,
        id: Option<&iced::advanced::widget::Id>,
        _bounds: iced::Rectangle,
        content: iced::Rectangle,
        _translation: iced::Vector,
        _state: &mut dyn iced::advanced::widget::operation::Scrollable,
    ) {
        if id == Some(&iced::advanced::widget::Id::new(PAGE)) {
            self.content = Some(content);
        }
    }

    fn finish(&self) -> iced::advanced::widget::operation::Outcome<f32> {
        match (self.content, self.row) {
            (Some(content), Some(row)) => iced::advanced::widget::operation::Outcome::Some(
                (row.y - content.y - 24.0).max(0.0),
            ),
            _ => iced::advanced::widget::operation::Outcome::None,
        }
    }
}

// One row a page draws, as `setting` and `group` saw it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Row {
    pub group: String,
    pub title: String,
    pub about: String,
}

thread_local! {
    // While a page is built for the index, the rows it draws.
    static NOTED: RefCell<Option<Vec<Row>>> = const { RefCell::new(None) };
    // The row the last search led to, drawn lit.
    static SPOT: RefCell<Option<(String, Rgba)>> = const { RefCell::new(None) };
}

// A group is drawn after its rows, so it names the ones noted just before it.
pub(super) fn note_group(title: &str) {
    NOTED.with(|noted| {
        if let Some(rows) = noted.borrow_mut().as_mut() {
            for row in rows.iter_mut().rev().take_while(|row| row.group.is_empty()) {
                row.group = title.to_owned();
            }
        }
    });
}

pub(super) fn note(title: &str, about: &str) {
    NOTED.with(|noted| {
        if let Some(rows) = noted.borrow_mut().as_mut() {
            rows.push(Row {
                group: String::new(),
                title: title.to_owned(),
                about: about.to_owned(),
            });
        }
    });
}

// The accent to light the row with, when it's the one a search led to.
pub(super) fn lit(title: &str) -> Option<Rgba> {
    SPOT.with(|spot| {
        spot.borrow()
            .as_ref()
            .filter(|(spotted, _)| spotted == title)
            .map(|(_, accent)| *accent)
    })
}

fn rows_of(build: impl FnOnce()) -> Vec<Row> {
    NOTED.with(|noted| *noted.borrow_mut() = Some(Vec::new()));
    build();
    NOTED.with(|noted| noted.borrow_mut().take().unwrap_or_default())
}

// Every word of the query somewhere in the row, in English or in the language shown.
pub(super) fn matches(words: &[String], row: &Row, page: Page) -> bool {
    let said = |english: &str| {
        let shown = crate::i18n::find(english).unwrap_or(english);
        format!("{english} {shown}").to_lowercase()
    };
    let haystack = [
        said(&row.title),
        said(&row.about),
        said(&row.group),
        said(page.title()),
    ]
    .join(" ");
    words.iter().all(|word| haystack.contains(word.as_str()))
}

const MOST: usize = 40;

impl Settings {
    // What every page draws, found by building each one once.
    pub(super) fn index(&self) -> Vec<(Page, Vec<Row>)> {
        Page::ALL
            .into_iter()
            .map(|page| {
                (
                    page,
                    rows_of(|| {
                        let _ = self.form(page);
                    }),
                )
            })
            .collect()
    }

    pub(super) fn spotlight<'a>(
        &'a self,
        build: impl FnOnce() -> Element<'a, Message>,
    ) -> Element<'a, Message> {
        let accent = self.palette().accent;
        SPOT.with(|spot| {
            *spot.borrow_mut() = self.spot.as_ref().map(|(_, title)| (title.clone(), accent))
        });
        let built = build();
        SPOT.with(|spot| *spot.borrow_mut() = None);
        built
    }

    // To the page, the row lit and scrolled to once the page is drawn.
    pub(super) fn go_to(&mut self, page: Page, title: String) -> Task<Message> {
        self.page = page;
        self.query.clear();
        self.spot = Some((page, title));
        iced::advanced::widget::operate(Locate::default()).then(|top| {
            iced::widget::operation::scroll_to(
                PAGE,
                iced::widget::operation::AbsoluteOffset {
                    x: None,
                    y: Some(top),
                },
            )
        })
    }

    pub(super) fn results(&self) -> Element<'_, Message> {
        let words: Vec<String> = self
            .query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let found: Vec<(Page, Row)> = self
            .index()
            .into_iter()
            .flat_map(|(page, rows)| rows.into_iter().map(move |row| (page, row)))
            .filter(|(page, row)| matches(&words, row, *page))
            .take(MOST)
            .collect();
        let header = text(crate::i18n::fill(
            "Settings with “{query}”",
            &[("query", &self.query.trim())],
        ))
        .size(22)
        .font(style::semibold());
        if found.is_empty() {
            return column![
                header,
                text("Nothing here has those words; try fewer")
                    .size(14)
                    .color(style::color(style::faint()))
            ]
            .spacing(10)
            .into();
        }
        let list = Column::with_children(found.into_iter().map(|(page, row)| {
            let mut words =
                column![text(row.title.clone()).size(14).font(style::medium())].spacing(3);
            if !row.about.is_empty() {
                words = words.push(
                    text(row.about.clone())
                        .size(12)
                        .color(style::color(style::faint())),
                );
            }
            let place = if row.group.is_empty() {
                crate::i18n::t(page.title()).to_owned()
            } else {
                format!(
                    "{} · {}",
                    crate::i18n::t(page.title()),
                    crate::i18n::find(&row.group).unwrap_or(&row.group)
                )
            };
            button(
                row![
                    container(icon(page.icon(), 15.0, neutral::text())).center(30),
                    words.width(Fill),
                    text(place).size(12).color(style::color(style::faint())),
                ]
                .spacing(14)
                .align_y(Alignment::Center),
            )
            .width(Fill)
            .padding([10, 14])
            .style(style::ghost(10.0))
            .on_press(Message::FoundSetting(page, row.title))
            .into()
        }))
        .spacing(2);
        column![
            header,
            scrollable(list)
                .direction(style::thin())
                .style(style::scroller)
                .height(Fill)
        ]
        .spacing(18)
        .into()
    }
}
