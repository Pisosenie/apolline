use super::*;
use crate::deck::Deck;
use crate::motion::{self, MOVE, Pose, Reveal, Speed, Tween};
use crate::posed;

// What on the shell's surfaces is coming, going or moving, and the time it's all drawn at.
pub(super) struct Moving {
    pub(super) now: Instant,
    // The overlay on screen, and the one still fading out after it was closed.
    overlay: Reveal,
    leaving: Option<Overlay>,
    // The OSD, and where its level bar is.
    osd: Reveal,
    level: Tween,
    cards: Deck<Notification>,
    // Where each screen's active workspace pill is.
    pills: Vec<(Option<String>, Tween)>,
}

impl Moving {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            now,
            overlay: Reveal::hidden(now),
            leaving: None,
            osd: Reveal::hidden(now),
            level: Tween::still(0.0, now),
            cards: Deck::new(popup::GAP as f32),
            pills: Vec::new(),
        }
    }

    // Frames come only while this is so; at rest there are none.
    pub(super) fn busy(&self) -> bool {
        self.overlay.moving(self.now)
            || self.osd.moving(self.now)
            || self.level.moving(self.now)
            || self.cards.moving(self.now)
            || self.pills.iter().any(|(_, pill)| pill.moving(self.now))
    }
}

// What comes and goes with motion; the rest open and close at once.
fn animated(overlay: Overlay) -> bool {
    !matches!(
        overlay,
        Overlay::Switcher | Overlay::Polkit | Overlay::Marks
    )
}

impl Shell {
    pub(super) fn speed(&self) -> Speed {
        let performance = self.current().is_some_and(|m| m.performance);
        Speed::new(self.config.windows.animations, performance)
    }

    // The time of this update: a frame's own, or now; never back.
    pub(super) fn clock(&mut self, message: &Message) {
        let at = match message {
            Message::Motion(at) => *at,
            _ => Instant::now(),
        };
        self.moving.now = self.moving.now.max(at);
    }

    // After every update: what finished going is let go, and the cards, the pills and the dock
    // follow what's there now.
    pub(super) fn settle(&mut self) {
        if self.moving.overlay.is_gone(self.moving.now)
            && let Some(gone) = self.moving.leaving.take()
        {
            self.let_go(gone);
        }
        if self.moving.osd.is_gone(self.moving.now) {
            self.osd = None;
        }
        self.follow_cards();
        self.follow_pills();
        self.dock.pace(self.speed());
    }

    // Each screen's pill heads for its active workspace.
    fn follow_pills(&mut self) {
        let now = self.moving.now;
        let length = self.speed().time(MOVE);
        let actives: Vec<(Option<&str>, usize)> = match &self.niri {
            Some(state) => self
                .outputs
                .iter()
                .map(|output| {
                    let output = Some(output.as_str());
                    (output, crate::status::active_on(state, output))
                })
                .collect(),
            None => vec![(None, self.workspace)],
        };
        let pills = &mut self.moving.pills;
        pills.retain(|(output, _)| actives.iter().any(|(o, _)| *o == output.as_deref()));
        for (output, active) in actives {
            let at = active as f32;
            match pills.iter_mut().find(|(o, _)| o.as_deref() == output) {
                Some((_, pill)) => *pill = pill.go(at, now, length),
                None => pills.push((output.map(str::to_owned), Tween::still(at, now))),
            }
        }
    }

    pub(super) fn pill(&self, output: Option<&str>) -> Option<f32> {
        self.moving
            .pills
            .iter()
            .find(|(o, _)| o.as_deref() == output)
            .map(|(_, pill)| pill.value(self.moving.now))
    }

    // The cards drawn follow the ones that should show.
    fn follow_cards(&mut self) {
        let speed = self.speed();
        let answering = self.replying.as_ref().map(|(id, _)| *id);
        let shown = popup::shown(
            self.popups.iter().rev().map(|(n, _)| n),
            answering,
            usize::from(self.config.alerts.cards),
        );
        let wanted: Vec<(u32, f32, &Notification)> = shown
            .into_iter()
            .map(|n| (n.id, popup::card_height(n) as f32, n))
            .collect();
        self.moving.cards.follow(&wanted, self.moving.now, speed);
    }

    #[cfg(target_os = "linux")]
    pub fn popup_height(&self) -> u32 {
        self.moving.cards.height(self.moving.now).ceil() as u32
    }

    // At the bottom of the screen the newest card is the one nearest the edge.
    pub(super) fn built_popups(&self) -> Element<'_, Message> {
        let corner = self.config.alerts.position;
        let mut cards = self.moving.cards.placed(self.moving.now, corner);
        if corner.bottom() {
            cards.reverse();
        }
        popup::stack(
            cards,
            corner.bottom(),
            &self.apps,
            self.config.palette(self.mode),
            self.replying.as_ref(),
            self.config.style.cards,
            self.config.alerts.width,
        )
    }

    // The overlay changed from `was`: one closed fades out first, one opened comes in, and one
    // fading out that's opened again turns back where it is.
    pub(super) fn turn_overlay(&mut self, was: Option<Overlay>) {
        let now = self.moving.now;
        let speed = self.speed();
        match (was, self.overlay) {
            (Some(gone), None) if animated(gone) => {
                self.moving.leaving = Some(gone);
                self.moving.overlay = self.moving.overlay.hide(now, speed);
            }
            (Some(gone), _) => self.let_go(gone),
            (None, _) => {}
        }
        let Some(open) = self.overlay else {
            return;
        };
        match self.moving.leaving.take() {
            Some(leaving) if leaving == open => {}
            Some(leaving) => {
                self.let_go(leaving);
                self.moving.overlay = Reveal::hidden(now);
            }
            None => self.moving.overlay = Reveal::hidden(now),
        }
        let speed = if animated(open) { speed } else { Speed::NONE };
        self.moving.overlay = self.moving.overlay.show(now, speed);
    }

    // What an overlay holds only while it's on screen.
    fn let_go(&mut self, overlay: Overlay) {
        match overlay {
            Overlay::Launcher => self.launcher.close(),
            Overlay::Marks => self.marks = None,
            _ => {}
        }
    }

    // The overlay on screen: the open one, or one on its way out.
    pub fn overlay_drawn(&self) -> Option<Overlay> {
        self.overlay.or(self.moving.leaving)
    }

    pub fn overlay_view(&self) -> Element<'_, Message> {
        let shown = self.moving.overlay.progress(self.moving.now);
        let built = style::within(self.overlay_font(), || {
            posed::within(shown, || self.built_overlay())
        });
        posed::posed(built, Pose::faded(shown))
            .passive(self.overlay.is_none())
            .into()
    }

    // A new OSD fades in; the same kind again keeps it up, its bar gliding to the new level.
    pub(super) fn bring_osd(&mut self, osd: &Osd) {
        let now = self.moving.now;
        let speed = self.speed();
        let same = self
            .osd
            .as_ref()
            .is_some_and(|(was, _)| std::mem::discriminant(was) == std::mem::discriminant(osd));
        if let Some(level) = osd::level(osd) {
            self.moving.level = if same {
                self.moving.level.go(level, now, speed.time(MOVE))
            } else {
                Tween::still(level, now)
            };
        }
        self.moving.osd = self.moving.osd.show(now, speed);
    }

    pub(super) fn fade_osd(&mut self) {
        self.moving.osd = self.moving.osd.hide(self.moving.now, self.speed());
    }

    // Rising a little as it fades in.
    pub fn osd_view(&self) -> Option<Element<'_, Message>> {
        let (osd, _) = self.osd.as_ref()?;
        let now = self.moving.now;
        let shown = self.moving.osd.progress(now);
        let built = style::within(self.config.style.osd.font, || {
            style::faded(shown, || {
                osd::view(
                    osd,
                    self.moving.level.value(now),
                    self.config.palette(self.mode),
                    self.config.style.osd,
                )
            })
        });
        let pose = Pose {
            opacity: shown,
            ..motion::entrance(shown, Edge::Bottom, Alignment::Center)
        };
        Some(posed::posed(built, pose).into())
    }
}
