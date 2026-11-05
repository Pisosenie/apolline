// Notification cards in motion: a new one slides in from its corner, one that goes fades out where
// it stood, and then the ones beyond it glide into the room it left. Places are measured from the
// screen edge the cards sit against, the newest nearest.
use apolline_config::Corner;
use iced::Vector;
use iced::time::Instant;

use crate::motion::{MOVE, Pose, Reveal, Speed, Tween};

// How far a card slides in from.
const SLIDE: f32 = 32.0;

struct Slot<T> {
    id: u32,
    card: T,
    height: f32,
    shown: Reveal,
    // How far from the edge it is.
    place: Tween,
}

enum Entry<T> {
    Kept(Slot<T>),
    New(u32, f32, T),
}

pub struct Deck<T> {
    slots: Vec<Slot<T>>,
    gap: f32,
}

impl<T: Clone + PartialEq> Deck<T> {
    pub fn new(gap: f32) -> Self {
        Self {
            slots: Vec::new(),
            gap,
        }
    }

    // The cards that should show now, nearest the edge first, by id and height. One no longer
    // there keeps its place while it fades, and only once it's gone do the others move up.
    pub fn follow(&mut self, wanted: &[(u32, f32, &T)], now: Instant, speed: Speed) {
        let mut kept: Vec<Option<Slot<T>>> = std::mem::take(&mut self.slots)
            .into_iter()
            .filter(|slot| !slot.shown.is_gone(now))
            .map(Some)
            .collect();
        let mut entries: Vec<Entry<T>> = wanted
            .iter()
            .map(|&(id, height, card)| {
                let found = kept
                    .iter_mut()
                    .find(|slot| slot.as_ref().is_some_and(|slot| slot.id == id))
                    .and_then(Option::take);
                match found {
                    Some(slot) => Entry::Kept(Slot {
                        card: if slot.card == *card {
                            slot.card
                        } else {
                            card.clone()
                        },
                        height,
                        shown: slot.shown.show(now, speed),
                        ..slot
                    }),
                    None => Entry::New(id, height, card.clone()),
                }
            })
            .collect();
        for (index, slot) in kept.into_iter().enumerate() {
            let Some(slot) = slot else {
                continue;
            };
            let shown = slot.shown.hide(now, speed);
            // Without animations it's gone at once, and makes room at once.
            if !shown.is_gone(now) {
                let leaving = Slot { shown, ..slot };
                entries.insert(index.min(entries.len()), Entry::Kept(leaving));
            }
        }
        self.slots = self.place(entries, now, speed);
    }

    fn place(&self, entries: Vec<Entry<T>>, now: Instant, speed: Speed) -> Vec<Slot<T>> {
        let mut edge = 0.0;
        entries
            .into_iter()
            .map(|entry| {
                let slot = match entry {
                    Entry::Kept(slot) => Slot {
                        place: slot.place.go(edge, now, speed.time(MOVE)),
                        ..slot
                    },
                    Entry::New(id, height, card) => Slot {
                        id,
                        card,
                        height,
                        shown: Reveal::hidden(now).show(now, speed),
                        place: Tween::still(edge, now),
                    },
                };
                edge += slot.height + self.gap;
                slot
            })
            .collect()
    }

    // Room for every card wherever it is on its way, so none is cut off mid-glide.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn height(&self, now: Instant) -> f32 {
        self.slots
            .iter()
            .map(|slot| slot.place.span(now).1 + slot.height)
            .fold(0.0, f32::max)
    }

    // When the last card in motion gets where it's going.
    pub fn ends(&self, now: Instant) -> Option<Instant> {
        self.slots
            .iter()
            .flat_map(|slot| [slot.shown.ends(now), slot.place.ends(now)])
            .flatten()
            .max()
    }

    // Each card nearest the edge first, as it's drawn now, and whether it's on its way out.
    pub fn placed(&self, now: Instant, corner: Corner) -> Vec<(&T, Pose, bool)> {
        self.slots
            .iter()
            .map(|slot| {
                let shown = slot.shown.progress(now);
                // Laid out at its place, drawn where it is on the way there.
                let away = slot.place.value(now) - slot.place.target();
                let slide = if slot.shown.is_open() {
                    SLIDE * (1.0 - shown)
                } else {
                    0.0
                };
                let offset = match corner {
                    Corner::TopRight => Vector::new(slide, away),
                    Corner::TopLeft => Vector::new(-slide, away),
                    Corner::Top => Vector::new(0.0, away - slide),
                    Corner::BottomRight => Vector::new(slide, -away),
                    Corner::BottomLeft => Vector::new(-slide, -away),
                    Corner::Bottom => Vector::new(0.0, slide - away),
                };
                let pose = Pose {
                    opacity: shown,
                    offset,
                    ..Pose::STILL
                };
                (&slot.card, pose, !slot.shown.is_open())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{CLOSE, OPEN};
    use std::time::Duration;

    fn follow(deck: &mut Deck<&'static str>, cards: &[(u32, &'static str)], now: Instant) {
        let wanted: Vec<(u32, f32, &&str)> = cards.iter().map(|(id, c)| (*id, 100.0, c)).collect();
        deck.follow(&wanted, now, Speed::default());
    }

    fn shown(deck: &Deck<&'static str>, now: Instant) -> Vec<(&'static str, Pose, bool)> {
        deck.placed(now, Corner::TopRight)
            .into_iter()
            .map(|(card, pose, leaving)| (*card, pose, leaving))
            .collect()
    }

    #[test]
    fn a_new_card_slides_in_from_its_corner() {
        let now = Instant::now();
        let mut deck = Deck::new(8.0);
        follow(&mut deck, &[(1, "a")], now);
        let (_, pose, leaving) = shown(&deck, now)[0];
        assert!(pose.opacity < 0.1);
        assert!(
            pose.offset.x > 0.0 && pose.offset.y == 0.0,
            "from the right"
        );
        assert!(!leaving && deck.ends(now).is_some());
        let later = now + OPEN;
        assert!(shown(&deck, later)[0].1.is_still());
        assert!(!deck.ends(later).is_some());
        assert_eq!(deck.height(later), 100.0);
        let below = deck.placed(now, Corner::BottomLeft)[0].1;
        assert!(below.offset.x < 0.0, "and from the left on that side");
    }

    #[test]
    fn a_card_that_goes_fades_where_it_stood_then_the_rest_move_up() {
        let start = Instant::now();
        let mut deck = Deck::new(8.0);
        follow(&mut deck, &[(1, "new"), (2, "old")], start);
        let gone = start + OPEN;
        follow(&mut deck, &[(2, "old")], gone);
        let cards = shown(&deck, gone);
        assert_eq!(cards.len(), 2, "still there while it fades");
        assert!(cards[0].2 && !cards[1].2, "the one going hears nothing");
        assert_eq!(cards[1].1.offset, Vector::ZERO, "the other waits");
        assert_eq!(deck.height(gone), 208.0);

        let faded = gone + CLOSE;
        follow(&mut deck, &[(2, "old")], faded);
        let cards = shown(&deck, faded);
        assert_eq!(cards.len(), 1);
        assert_eq!(
            cards[0].1.offset.y, 108.0,
            "drawn where it was, laid out at the top"
        );
        assert_eq!(deck.height(faded), 208.0, "room for it until it's there");
        let midway = faded + MOVE / 2;
        let y = shown(&deck, midway)[0].1.offset.y;
        assert!(y > 0.0 && y < 108.0, "{y}");
        let there = faded + MOVE;
        assert_eq!(shown(&deck, there)[0].1.offset, Vector::ZERO);
        assert_eq!(deck.height(there), 100.0);
        assert!(!deck.ends(there).is_some());
    }

    #[test]
    fn at_the_bottom_the_rest_move_down_toward_the_edge() {
        let start = Instant::now();
        let mut deck = Deck::new(8.0);
        follow(&mut deck, &[(1, "new"), (2, "old")], start);
        follow(&mut deck, &[(2, "old")], start + OPEN);
        follow(&mut deck, &[(2, "old")], start + OPEN + CLOSE);
        let pose = deck.placed(start + OPEN + CLOSE, Corner::Bottom)[0].1;
        assert_eq!(pose.offset.y, -108.0);
    }

    #[test]
    fn a_card_back_while_fading_turns_around_in_its_place() {
        let start = Instant::now();
        let mut deck = Deck::new(8.0);
        follow(&mut deck, &[(1, "a"), (2, "b"), (3, "c")], start);
        let shown_at = start + OPEN;
        follow(&mut deck, &[(1, "a"), (3, "c")], shown_at);
        let order: Vec<&str> = shown(&deck, shown_at).iter().map(|c| c.0).collect();
        assert_eq!(order, ["a", "b", "c"], "the going one keeps its place");
        let midway = shown_at + CLOSE / 2;
        let faded = shown(&deck, midway)[1].1.opacity;
        follow(&mut deck, &[(1, "a"), (2, "b2"), (3, "c")], midway);
        let (card, pose, leaving) = shown(&deck, midway)[1];
        assert_eq!(card, "b2", "what it says now");
        assert_eq!(pose.opacity, faded);
        assert!(!leaving);
    }

    #[test]
    fn a_new_card_pushes_the_others_away_smoothly() {
        let start = Instant::now();
        let mut deck = Deck::new(8.0);
        follow(&mut deck, &[(1, "old")], start);
        let later = start + Duration::from_secs(1);
        follow(&mut deck, &[(2, "new"), (1, "old")], later);
        let cards = shown(&deck, later);
        assert_eq!(cards[1].1.offset.y, -108.0, "starts where it was");
        assert_eq!(deck.height(later), 208.0, "room for where it's going");
    }

    #[test]
    fn without_animations_everything_is_where_it_ends_at_once() {
        let now = Instant::now();
        let mut deck = Deck::new(8.0);
        let off = Speed::new(Some(0.0), false);
        let (a, b) = ("a", "b");
        deck.follow(&[(1, 100.0, &a), (2, 100.0, &b)], now, off);
        assert!(
            deck.placed(now, Corner::TopRight)
                .iter()
                .all(|c| c.1.is_still())
        );
        deck.follow(&[(2, 100.0, &b)], now, off);
        assert_eq!(deck.placed(now, Corner::TopRight).len(), 1);
        assert!(!deck.ends(now).is_some());
        assert_eq!(deck.height(now), 100.0);
    }
}
