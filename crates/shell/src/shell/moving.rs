use super::*;
use crate::motion::{Pose, Reveal, Speed};
use crate::posed;

// What on the shell's surfaces is coming, going or moving, and the time it's all drawn at.
pub(super) struct Moving {
    pub(super) now: Instant,
    // The overlay on screen, and the one still fading out after it was closed.
    overlay: Reveal,
    leaving: Option<Overlay>,
}

impl Moving {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            now,
            overlay: Reveal::hidden(now),
            leaving: None,
        }
    }

    // Frames come only while this is so; at rest there are none.
    pub(super) fn busy(&self) -> bool {
        self.overlay.moving(self.now)
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

    // What finished going after this update is let go.
    pub(super) fn settle(&mut self) {
        if self.moving.overlay.is_gone(self.moving.now)
            && let Some(gone) = self.moving.leaving.take()
        {
            self.let_go(gone);
        }
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
}
