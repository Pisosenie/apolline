// How things on the shell's surfaces come, go and move: eased over a short time, at the speed
// [windows] animations gives niri, and not at all when that is 0.
use std::time::Duration;

use apolline_config::Edge;
use apolline_theme::Rgba;
use iced::time::Instant;
use iced::{Alignment, Vector};

pub const OPEN: Duration = Duration::from_millis(180);
pub const CLOSE: Duration = Duration::from_millis(120);
// Cards closing a gap, the workspace pill, a level bar.
pub const MOVE: Duration = Duration::from_millis(220);
// How far an overlay slides in from its edge, and how much smaller it starts.
const SLIDE: f32 = 10.0;
const SHRINK: f32 = 0.04;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Speed(f32);

impl Speed {
    pub const NONE: Speed = Speed(0.0);

    // As niri reads [windows] animations: unset is 1, 0 or less is none, 2 is twice as fast. A
    // performance mode turns them off there, so here too.
    pub fn new(animations: Option<f32>, performance: bool) -> Self {
        match animations {
            _ if performance => Self(0.0),
            None => Self(1.0),
            Some(speed) if speed > 0.0 && speed.is_finite() => Self(speed.max(0.05)),
            Some(_) => Self(0.0),
        }
    }

    pub fn time(self, length: Duration) -> Duration {
        if self.0 <= 0.0 {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(length.as_secs_f64() / f64::from(self.0))
        }
    }
}

impl Default for Speed {
    fn default() -> Self {
        Self(1.0)
    }
}

// Fast at first and settling softly.
pub fn ease_out(t: f32) -> f32 {
    let left = 1.0 - t.clamp(0.0, 1.0);
    1.0 - left * left * left
}

// A number on its way to another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    length: Duration,
}

impl Tween {
    pub fn still(value: f32, now: Instant) -> Self {
        Self {
            from: value,
            to: value,
            start: now,
            length: Duration::ZERO,
        }
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    pub fn value(&self, now: Instant) -> f32 {
        self.from + (self.to - self.from) * ease_out(self.done(now))
    }

    pub fn moving(&self, now: Instant) -> bool {
        self.from != self.to && self.done(now) < 1.0
    }

    // Off toward `to` from wherever it is now, so a turn midway carries on without a jump.
    pub fn go(self, to: f32, now: Instant, length: Duration) -> Self {
        if to == self.to {
            return self;
        }
        Self {
            from: self.value(now),
            to,
            start: now,
            length,
        }
    }

    // The lowest and highest it passes through on the way, to leave room for both.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn span(&self, now: Instant) -> (f32, f32) {
        if self.moving(now) {
            (self.from.min(self.to), self.from.max(self.to))
        } else {
            (self.to, self.to)
        }
    }

    fn done(&self, now: Instant) -> f32 {
        if self.length.is_zero() {
            return 1.0;
        }
        let gone = now.saturating_duration_since(self.start);
        (gone.as_secs_f32() / self.length.as_secs_f32()).min(1.0)
    }
}

// Something that opens and closes: 0 hidden, 1 shown. Opening takes OPEN and closing the quicker
// CLOSE, each only for the part of the way that is left, so turning back midway is smooth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reveal(Tween);

impl Reveal {
    pub fn hidden(now: Instant) -> Self {
        Self(Tween::still(0.0, now))
    }

    pub fn show(self, now: Instant, speed: Speed) -> Self {
        self.turn(1.0, OPEN, now, speed)
    }

    pub fn hide(self, now: Instant, speed: Speed) -> Self {
        self.turn(0.0, CLOSE, now, speed)
    }

    fn turn(self, to: f32, length: Duration, now: Instant, speed: Speed) -> Self {
        let left = (to - self.0.value(now)).abs();
        Self(self.0.go(to, now, speed.time(length).mul_f32(left)))
    }

    pub fn progress(&self, now: Instant) -> f32 {
        self.0.value(now)
    }

    // Shown or on its way there.
    pub fn is_open(&self) -> bool {
        self.0.target() == 1.0
    }

    // Hidden and done with it.
    pub fn is_gone(&self, now: Instant) -> bool {
        !self.is_open() && !self.0.moving(now)
    }

    pub fn moving(&self, now: Instant) -> bool {
        self.0.moving(now)
    }
}

// How something is drawn while it moves: faded, shifted, and scaled from a point given as a
// fraction of its bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub opacity: f32,
    pub offset: Vector,
    pub scale: f32,
    pub origin: (f32, f32),
}

impl Pose {
    pub const STILL: Pose = Pose {
        opacity: 1.0,
        offset: Vector::ZERO,
        scale: 1.0,
        origin: (0.5, 0.5),
    };

    pub fn faded(opacity: f32) -> Self {
        Self {
            opacity,
            ..Self::STILL
        }
    }

    pub fn is_still(&self) -> bool {
        self.opacity >= 1.0 && self.offset == Vector::ZERO && self.scale == 1.0
    }
}

// A card coming in by the `from` edge, `progress` of the way: slid toward that edge and a little
// smaller, growing out of the side it sits against, at the end of it `along` says.
pub fn entrance(progress: f32, from: Edge, along: Alignment) -> Pose {
    let left = 1.0 - progress.clamp(0.0, 1.0);
    let across = |along: Alignment| match along {
        Alignment::Start => 0.0,
        Alignment::Center => 0.5,
        Alignment::End => 1.0,
    };
    let slide = SLIDE * left;
    let (offset, origin) = match from {
        Edge::Top => (Vector::new(0.0, -slide), (across(along), 0.0)),
        Edge::Bottom => (Vector::new(0.0, slide), (across(along), 1.0)),
        // Down a side the end of the edge is its top, as style::beside_bar has it.
        Edge::Left => (Vector::new(-slide, 0.0), (0.0, across(down(along)))),
        Edge::Right => (Vector::new(slide, 0.0), (1.0, across(down(along)))),
    };
    Pose {
        opacity: 1.0,
        offset,
        scale: 1.0 - SHRINK * left,
        origin,
    }
}

fn down(along: Alignment) -> Alignment {
    match along {
        Alignment::End => Alignment::Start,
        other => other,
    }
}

// How much of a pill that sits `at` (between two places while it slides) is on place `index`.
pub fn share(at: f32, index: usize) -> f32 {
    (1.0 - (at - index as f32).abs()).clamp(0.0, 1.0)
}

// Part of the way from one color to another, alpha too.
pub fn blend(from: Rgba, to: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    from.mix(to, t).alpha(from.a + (to.a - from.a) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn the_speed_follows_niri() {
        assert_eq!(Speed::new(None, false).time(OPEN), OPEN);
        assert_eq!(Speed::new(Some(2.0), false).time(ms(200)), ms(100));
        assert_eq!(Speed::new(Some(0.5), false).time(ms(100)), ms(200));
        assert_eq!(Speed::new(Some(0.0), false).time(OPEN), Duration::ZERO);
        assert_eq!(Speed::new(Some(-1.0), false).time(OPEN), Duration::ZERO);
        assert_eq!(Speed::new(Some(f32::NAN), false).time(OPEN), Duration::ZERO);
        assert_eq!(Speed::new(None, true).time(OPEN), Duration::ZERO, "play");
        assert!(Speed::new(Some(1e-9), false).time(OPEN) < Duration::from_secs(5));
    }

    #[test]
    fn easing_starts_fast_and_lands_exactly() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert_eq!(ease_out(7.0), 1.0);
        assert!(ease_out(0.5) > 0.8, "most of the way by half time");
        assert!(ease_out(0.3) < ease_out(0.31));
    }

    #[test]
    fn a_tween_moves_then_rests() {
        let start = Instant::now();
        let tween = Tween::still(0.0, start).go(10.0, start, ms(100));
        assert_eq!(tween.value(start), 0.0);
        assert!(tween.moving(start + ms(50)));
        let half = tween.value(start + ms(50));
        assert!(half > 5.0 && half < 10.0, "{half}");
        assert_eq!(tween.value(start + ms(100)), 10.0);
        assert!(!tween.moving(start + ms(100)));
        assert_eq!(tween.span(start + ms(10)), (0.0, 10.0));
        assert_eq!(tween.span(start + ms(200)), (10.0, 10.0));
    }

    #[test]
    fn a_tween_turned_midway_does_not_jump() {
        let start = Instant::now();
        let out = Tween::still(0.0, start).go(10.0, start, ms(100));
        let turn = start + ms(40);
        let back = out.go(0.0, turn, ms(100));
        assert_eq!(back.value(turn), out.value(turn));
        assert_eq!(back.value(turn + ms(100)), 0.0);
        assert_eq!(
            back.go(0.0, turn + ms(10), ms(100)),
            back,
            "same way, same tween"
        );
    }

    #[test]
    fn nothing_moves_without_animations() {
        let now = Instant::now();
        let off = Speed::new(Some(0.0), false);
        let tween = Tween::still(0.0, now).go(3.0, now, off.time(MOVE));
        assert!(!tween.moving(now));
        assert_eq!(tween.value(now), 3.0);
        let open = Reveal::hidden(now).show(now, off);
        assert!(!open.moving(now));
        assert_eq!(open.progress(now), 1.0);
        let shut = open.hide(now, off);
        assert!(shut.is_gone(now));
    }

    #[test]
    fn a_reveal_opens_closes_and_turns_back() {
        let now = Instant::now();
        let speed = Speed::default();
        let open = Reveal::hidden(now).show(now, speed);
        assert!(open.is_open() && open.moving(now) && !open.is_gone(now));
        assert_eq!(open.progress(now + OPEN), 1.0);
        assert!(!open.moving(now + OPEN));

        let shown = now + OPEN;
        let shut = open.hide(shown, speed);
        assert!(!shut.is_open() && !shut.is_gone(shown));
        assert!(shut.is_gone(shown + CLOSE), "closing is the quicker");

        // Opened again halfway out: on its way back up from where it was.
        let midway = shown + CLOSE / 2;
        let again = shut.show(midway, speed);
        assert_eq!(again.progress(midway), shut.progress(midway));
        assert!(again.is_open() && !again.is_gone(midway));
        let left = 1.0 - shut.progress(midway);
        assert!(again.moving(midway + OPEN.mul_f32(left * 0.9)));
        assert!(
            !again.moving(midway + OPEN.mul_f32(left)),
            "only the part left"
        );
    }

    #[test]
    fn an_entrance_comes_from_its_edge_and_ends_still() {
        let start = entrance(0.0, Edge::Top, Alignment::End);
        assert!(start.offset.y < 0.0 && start.offset.x == 0.0);
        assert_eq!(start.origin, (1.0, 0.0), "out of the top right");
        assert!(start.scale < 1.0);
        let right = entrance(0.0, Edge::Right, Alignment::End);
        assert!(right.offset.x > 0.0);
        assert_eq!(right.origin, (1.0, 0.0), "down a side the end is the top");
        let bottom = entrance(0.5, Edge::Bottom, Alignment::Center);
        assert!(bottom.offset.y > 0.0 && bottom.offset.y < SLIDE);
        assert!(entrance(1.0, Edge::Left, Alignment::Start).is_still());
        assert!(!Pose::faded(0.5).is_still());
    }

    #[test]
    fn a_sliding_pill_is_shared_by_its_neighbours() {
        assert_eq!(share(2.0, 2), 1.0);
        assert_eq!(share(2.0, 3), 0.0);
        assert_eq!(share(2.25, 2), 0.75);
        assert_eq!(share(2.25, 3), 0.25);
        assert_eq!(share(2.25, 0), 0.0);
        let total: f32 = (0..5).map(|i| share(1.6, i)).sum();
        assert!((total - 1.0).abs() < 1e-6, "the row keeps its length");
    }

    #[test]
    fn colors_blend_with_their_alpha() {
        let from = Rgba::hex(0x000000).alpha(0.2);
        let to = Rgba::hex(0xffffff);
        assert_eq!(blend(from, to, 0.0), from);
        assert_eq!(blend(from, to, 1.0), to);
        let half = blend(from, to, 0.5);
        assert_eq!(half.r, 128);
        assert!((half.a - 0.6).abs() < 1e-6);
    }
}
