mod color;

pub use color::Rgba;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    #[default]
    Work,
    Play,
    Evening,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Work, Mode::Play, Mode::Evening];

    pub fn key(self) -> &'static str {
        match self {
            Mode::Work => "work",
            Mode::Play => "play",
            Mode::Evening => "evening",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Work => "Work",
            Mode::Play => "Play",
            Mode::Evening => "Evening",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.key() == key)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub accent: Rgba,
    pub on_accent: Rgba,
    pub accent_soft: Rgba,
    pub wallpaper: &'static str,
}

impl Palette {
    pub const fn of(mode: Mode) -> Self {
        match mode {
            Mode::Work => Self::new(0x86bdf0, 0x0b1724, "fog.jpg"),
            Mode::Play => Self::new(0xff6a3d, 0x1a0703, "ember.jpg"),
            Mode::Evening => Self::new(0xf2b35c, 0x1d1204, "night.jpg"),
        }
    }

    pub fn with_accent(self, accent: Rgba) -> Self {
        let dark = neutral::INK;
        let on_accent = if accent.contrast(dark) >= accent.contrast(neutral::TEXT) {
            dark
        } else {
            neutral::TEXT
        };
        Self {
            accent,
            on_accent,
            accent_soft: accent.alpha(SOFT),
            ..self
        }
    }

    const fn new(accent: u32, on_accent: u32, wallpaper: &'static str) -> Self {
        Self {
            accent: Rgba::hex(accent),
            on_accent: Rgba::hex(on_accent),
            accent_soft: Rgba::hex(accent).alpha(SOFT),
            wallpaper,
        }
    }
}

const SOFT: f32 = 0.18;

pub mod neutral {
    use crate::Rgba;

    pub const TEXT: Rgba = Rgba::hex(0xf4f3f0);
    pub const MUTED: Rgba = TEXT.alpha(0.72);
    pub const INK: Rgba = Rgba::hex(0x0c0d10);
    pub const GLASS: Rgba = INK.alpha(0.55);
    pub const PANEL: Rgba = Rgba::hex(0x101114).alpha(0.78);
    pub const WINDOW: Rgba = Rgba::hex(0x141619);
    pub const HAIRLINE: Rgba = Rgba::hex(0xffffff).alpha(0.08);
    pub const RAISED: Rgba = Rgba::hex(0xffffff).alpha(0.16);
}

pub mod metrics {
    pub const BAR_HEIGHT: f32 = 36.0;
    pub const BAR_INSET: f32 = 8.0;
    pub const GAP: f32 = 10.0;
    pub const RADIUS_BAR: f32 = 12.0;
    pub const RADIUS_PANEL: f32 = 16.0;
    pub const RADIUS_CONTROL: f32 = 8.0;
}

pub mod font {
    pub const UI: &str = "Geist";
    pub const MONO: &str = "Geist Mono";
    pub const BODY: f32 = 13.0;
    pub const SMALL: f32 = 12.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    const BRIGHT_WALLPAPER: Rgba = Rgba::hex(0x808080);

    #[test]
    fn modes_round_trip_through_keys() {
        for mode in Mode::ALL {
            assert_eq!(Mode::from_key(mode.key()), Some(mode));
        }
        assert_eq!(Mode::Evening.next(), Mode::Work);
    }

    #[test]
    fn every_accent_reads_on_its_own_surfaces() {
        let panel = neutral::PANEL.over(BRIGHT_WALLPAPER);
        for mode in Mode::ALL {
            let p = Palette::of(mode);
            assert!(
                p.on_accent.contrast(p.accent) >= 4.5,
                "{mode:?} label on accent"
            );
            assert!(p.accent.contrast(panel) >= 4.5, "{mode:?} accent on panel");
        }
    }

    #[test]
    fn muted_text_reads_on_glass_over_bright_wallpaper() {
        let glass = neutral::GLASS.over(BRIGHT_WALLPAPER);
        assert!(neutral::MUTED.over(glass).contrast(glass) >= 4.5);
    }

    #[test]
    fn custom_accent_picks_readable_label() {
        let light = Palette::of(Mode::Work).with_accent(Rgba::hex(0xffe08a));
        let dark = Palette::of(Mode::Work).with_accent(Rgba::hex(0x3b2a7a));
        assert_eq!(light.on_accent, neutral::INK);
        assert_eq!(dark.on_accent, neutral::TEXT);
    }
}
