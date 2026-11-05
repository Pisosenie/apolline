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
    // When an update is already on its way for motion to end.
    woken: Option<Instant>,
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
            woken: None,
        }
    }

    // When everything in motion has got where it's going; none at rest.
    pub(super) fn ends(&self) -> Option<Instant> {
        let now = self.now;
        let pills = self.pills.iter().map(|(_, pill)| pill.ends(now));
        [
            self.overlay.ends(now),
            self.osd.ends(now),
            self.level.ends(now),
            self.cards.ends(now),
        ]
        .into_iter()
        .chain(pills)
        .flatten()
        .max()
    }

    // The time to draw at: the latest update's, or later when no update has come since, so a
    // surface drawn without one still shows what it should by now.
    pub(super) fn drawn_at(&self) -> Instant {
        self.now.max(Instant::now())
    }
}

impl Moving {
    // The overlay changed from `was` to `open`: one closed fades out first, one opened comes in,
    // and one fading out that's opened again turns back where it is. Gives back the ones gone
    // at once, to let go of what they hold.
    fn turn(&mut self, was: Option<Overlay>, open: Option<Overlay>, speed: Speed) -> Vec<Overlay> {
        let now = self.now;
        let mut gone = Vec::new();
        match (was, open) {
            (Some(closed), None) if animated(closed) => {
                self.leaving = Some(closed);
                self.overlay = self.overlay.hide(now, speed);
            }
            (Some(closed), _) => gone.push(closed),
            (None, _) => {}
        }
        let Some(open) = open else {
            return gone;
        };
        match self.leaving.take() {
            Some(leaving) if leaving == open => {}
            Some(leaving) => {
                gone.push(leaving);
                self.overlay = Reveal::hidden(now);
            }
            None => self.overlay = Reveal::hidden(now),
        }
        let speed = if animated(open) { speed } else { Speed::NONE };
        self.overlay = self.overlay.show(now, speed);
        gone
    }

    // The overlay that just finished fading out.
    fn faded_out(&mut self) -> Option<Overlay> {
        if self.overlay.is_gone(self.now) {
            self.leaving.take()
        } else {
            None
        }
    }

    fn overlay_shown(&self, at: Instant) -> f32 {
        self.overlay.progress(at)
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
        if let Some(gone) = self.moving.faded_out() {
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
            .map(|(_, pill)| pill.value(self.moving.drawn_at()))
    }

    // One more update once motion should be over, whatever else comes or doesn't by then.
    pub(super) fn wake(&mut self) -> Task<Message> {
        let Some(end) = self.moving.ends() else {
            return Task::none();
        };
        if self.moving.woken.is_some_and(|woken| woken >= end) {
            return Task::none();
        }
        self.moving.woken = Some(end);
        let wait = end.saturating_duration_since(Instant::now());
        // The timer is made when the task first runs, inside the runtime.
        Task::future(async move {
            tokio::time::sleep(wait).await;
            Message::Motion(Instant::now())
        })
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
        self.moving.cards.height(self.moving.drawn_at()).ceil() as u32
    }

    // At the bottom of the screen the newest card is the one nearest the edge.
    pub(super) fn built_popups(&self) -> Element<'_, Message> {
        let corner = self.config.alerts.position;
        let mut cards = self.moving.cards.placed(self.moving.drawn_at(), corner);
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

    pub(super) fn turn_overlay(&mut self, was: Option<Overlay>) {
        let speed = self.speed();
        for gone in self.moving.turn(was, self.overlay, speed) {
            self.let_go(gone);
        }
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
        let shown = self.moving.overlay_shown(self.moving.drawn_at());
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
        let now = self.moving.drawn_at();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{CLOSE, OPEN};

    #[test]
    fn an_overlay_left_without_frames_is_fully_open_once_its_time_is_up() {
        let start = Instant::now();
        let mut moving = Moving::new(start);
        let gone = moving.turn(None, Some(Overlay::Panel), Speed::default());
        assert!(gone.is_empty());
        assert!(moving.overlay_shown(start) < 0.1);
        assert!(moving.ends().is_some_and(|end| end <= start + OPEN));
        // No frame and no update since: drawn later, it's all there.
        assert_eq!(moving.overlay_shown(start + OPEN), 1.0);
        assert_eq!(moving.overlay_shown(start + 10 * OPEN), 1.0);
    }

    #[test]
    fn drawn_without_any_update_it_shows_what_it_should_by_now() {
        let long_ago = Instant::now() - Duration::from_secs(1);
        let mut moving = Moving::new(long_ago);
        moving.turn(None, Some(Overlay::Launcher), Speed::default());
        assert_eq!(moving.overlay_shown(moving.drawn_at()), 1.0);
    }

    #[test]
    fn a_closed_overlay_fades_then_is_let_go_and_reopens_where_it_is() {
        let start = Instant::now();
        let mut moving = Moving::new(start);
        moving.turn(None, Some(Overlay::Calendar), Speed::default());
        moving.now = start + OPEN;
        assert!(
            moving
                .turn(Some(Overlay::Calendar), None, Speed::default())
                .is_empty()
        );
        assert_eq!(moving.faded_out(), None, "still fading");
        moving.now += CLOSE / 2;
        let midway = moving.overlay_shown(moving.now);
        moving.turn(None, Some(Overlay::Calendar), Speed::default());
        assert_eq!(
            moving.overlay_shown(moving.now),
            midway,
            "turned back where it was"
        );
        moving.turn(Some(Overlay::Calendar), None, Speed::default());
        moving.now += CLOSE;
        assert_eq!(moving.faded_out(), Some(Overlay::Calendar));
        assert_eq!(moving.ends(), None, "at rest");
    }

    #[test]
    fn without_animations_an_overlay_is_open_and_gone_at_once() {
        let now = Instant::now();
        let mut moving = Moving::new(now);
        moving.turn(None, Some(Overlay::Keys), Speed::NONE);
        assert_eq!(moving.overlay_shown(now), 1.0);
        moving.turn(Some(Overlay::Keys), None, Speed::NONE);
        assert_eq!(moving.faded_out(), Some(Overlay::Keys));
        assert_eq!(moving.ends(), None);
    }

    #[test]
    fn switching_overlays_lets_the_old_one_go_at_once() {
        let now = Instant::now();
        let mut moving = Moving::new(now);
        moving.turn(None, Some(Overlay::Panel), Speed::default());
        let gone = moving.turn(
            Some(Overlay::Panel),
            Some(Overlay::Machine),
            Speed::default(),
        );
        assert_eq!(gone, [Overlay::Panel]);
    }
}
