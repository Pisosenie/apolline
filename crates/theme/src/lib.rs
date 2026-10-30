mod color;

pub use color::{Rgba, accent_of};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub accent: Rgba,
    pub on_accent: Rgba,
    pub accent_soft: Rgba,
}

impl Palette {
    pub fn new(accent: Rgba) -> Self {
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
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new(ACCENTS[0])
    }
}

pub const ACCENTS: [Rgba; 6] = [
    Rgba::hex(0x86bdf0),
    Rgba::hex(0xff6a3d),
    Rgba::hex(0xf2b35c),
    Rgba::hex(0xc084fc),
    Rgba::hex(0x5dd39e),
    Rgba::hex(0xf472b6),
];

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
    fn every_accent_reads_on_its_own_surfaces() {
        let panel = neutral::PANEL.over(BRIGHT_WALLPAPER);
        for accent in ACCENTS {
            let p = Palette::new(accent);
            assert!(p.on_accent.contrast(p.accent) >= 4.5, "label on {accent:?}");
            assert!(p.accent.contrast(panel) >= 4.5, "{accent:?} on panel");
        }
    }

    #[test]
    fn the_accent_comes_from_the_vivid_part_of_the_picture() {
        let mut pixels = Vec::new();
        for i in 0..10_000 {
            let px: [u8; 4] = match i % 10 {
                0..=5 => [40, 42, 46, 255],
                6..=8 => [20, 60, 170, 255],
                _ => [200, 200, 200, 255],
            };
            pixels.extend(px);
        }
        let accent = accent_of(&pixels).unwrap();
        assert!(accent.b > accent.r && accent.b > accent.g, "{accent:?}");
        let panel = neutral::PANEL.over(BRIGHT_WALLPAPER);
        assert!(
            Palette::new(accent).accent.contrast(panel) >= 4.5,
            "{accent:?}"
        );
        assert_eq!(
            accent_of(&[128, 128, 128, 255].repeat(500)),
            None,
            "grey has no accent"
        );
    }

    #[test]
    fn hue_turns_and_lightness_stays() {
        assert_eq!(Rgba::hex(0xff0000).shift_hue(120.0), Rgba::hex(0x00ff00));
        assert_eq!(Rgba::hex(0x808080).shift_hue(90.0), Rgba::hex(0x808080));
        let accent = ACCENTS[0];
        assert_eq!(accent.shift_hue(360.0), accent);
        let turned = accent.shift_hue(60.0);
        assert!((turned.luminance() - accent.luminance()).abs() < 0.25);
    }

    #[test]
    fn muted_text_reads_on_glass_over_bright_wallpaper() {
        let glass = neutral::GLASS.over(BRIGHT_WALLPAPER);
        assert!(neutral::MUTED.over(glass).contrast(glass) >= 4.5);
    }

    #[test]
    fn custom_accent_picks_readable_label() {
        let light = Palette::new(Rgba::hex(0xffe08a));
        let dark = Palette::new(Rgba::hex(0x3b2a7a));
        assert_eq!(light.on_accent, neutral::INK);
        assert_eq!(dark.on_accent, neutral::TEXT);
    }
}
