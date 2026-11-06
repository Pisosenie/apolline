use crate::{Rgba, Scheme};

// Palettes people already know, each with the accent it's known for.
const fn dark(ink: u32, base: u32, mantle: u32, text: u32) -> Scheme {
    Scheme {
        light: false,
        ink: Rgba::hex(ink),
        panel: Rgba::hex(mantle).alpha(0.86),
        surface: Rgba::hex(base).alpha(0.96),
        window: Rgba::hex(base),
        text: Rgba::hex(text),
        overlay: Rgba::hex(0xffffff),
    }
}

const fn light(ink: u32, base: u32, mantle: u32, text: u32) -> Scheme {
    Scheme {
        light: true,
        ink: Rgba::hex(ink),
        panel: Rgba::hex(mantle).alpha(0.92),
        surface: Rgba::hex(base).alpha(0.97),
        window: Rgba::hex(base),
        text: Rgba::hex(text),
        overlay: Rgba::hex(0x000000),
    }
}

pub const NAMED: [(&str, Scheme, Rgba); 15] = [
    ("dark", Scheme::DARK, Rgba::hex(0x86bdf0)),
    (
        "light",
        light(0xf3f1ec, 0xfbfaf7, 0xffffff, 0x1b1c20),
        Rgba::hex(0x2f6fd6),
    ),
    (
        "catppuccin-mocha",
        dark(0x11111b, 0x1e1e2e, 0x181825, 0xcdd6f4),
        Rgba::hex(0xcba6f7),
    ),
    (
        "catppuccin-macchiato",
        dark(0x181926, 0x24273a, 0x1e2030, 0xcad3f5),
        Rgba::hex(0xc6a0f6),
    ),
    (
        "catppuccin-latte",
        light(0xdce0e8, 0xeff1f5, 0xe6e9ef, 0x4c4f69),
        Rgba::hex(0x8839ef),
    ),
    (
        "gruvbox",
        dark(0x1d2021, 0x282828, 0x32302f, 0xebdbb2),
        Rgba::hex(0xfe8019),
    ),
    (
        "gruvbox-light",
        light(0xebdbb2, 0xfbf1c7, 0xf2e5bc, 0x3c3836),
        Rgba::hex(0xaf3a03),
    ),
    (
        "nord",
        dark(0x242933, 0x2e3440, 0x3b4252, 0xeceff4),
        Rgba::hex(0x88c0d0),
    ),
    (
        "tokyo-night",
        dark(0x16161e, 0x1a1b26, 0x1f2335, 0xc0caf5),
        Rgba::hex(0x7aa2f7),
    ),
    (
        "rose-pine",
        dark(0x191724, 0x1f1d2e, 0x26233a, 0xe0def4),
        Rgba::hex(0xc4a7e7),
    ),
    (
        "rose-pine-dawn",
        light(0xf2e9e1, 0xfaf4ed, 0xfffaf3, 0x575279),
        Rgba::hex(0x907aa9),
    ),
    (
        "everforest",
        dark(0x232a2e, 0x2d353b, 0x343f44, 0xd3c6aa),
        Rgba::hex(0xa7c080),
    ),
    (
        "kanagawa",
        dark(0x16161d, 0x1f1f28, 0x2a2a37, 0xdcd7ba),
        Rgba::hex(0x7e9cd8),
    ),
    (
        "dracula",
        dark(0x191a21, 0x282a36, 0x21222c, 0xf8f8f2),
        Rgba::hex(0xbd93f9),
    ),
    (
        "paper",
        light(0xe9e7e2, 0xf7f6f2, 0xffffff, 0x111111),
        Rgba::hex(0xd03a2f),
    ),
];

pub fn named(name: &str) -> Option<(Scheme, Rgba)> {
    NAMED
        .iter()
        .find(|(known, ..)| *known == name)
        .map(|(_, scheme, accent)| (*scheme, *accent))
}

// Neutrals tinted with the accent's hue, the way Material's tonal palettes are: dark ones barely
// coloured, light ones a touch warmer of the same hue.
pub fn tonal(accent: Rgba, is_light: bool) -> Scheme {
    let hue = accent.hue();
    let tone = |saturation: f32, light: f32| Rgba::hsl(hue, saturation, light);
    if is_light {
        Scheme {
            light: true,
            ink: tone(0.22, 0.91),
            panel: tone(0.30, 0.98).alpha(0.92),
            surface: tone(0.26, 0.97).alpha(0.97),
            window: tone(0.24, 0.96),
            text: tone(0.25, 0.10),
            overlay: Rgba::hex(0x000000),
        }
    } else {
        Scheme {
            light: false,
            ink: tone(0.20, 0.05),
            panel: tone(0.16, 0.08).alpha(0.86),
            surface: tone(0.16, 0.10).alpha(0.96),
            window: tone(0.14, 0.09),
            text: tone(0.20, 0.95),
            overlay: Rgba::hex(0xffffff),
        }
    }
}

// An accent that reads on the scheme's surfaces: darkened on light ones, lightened on dark ones,
// a step at a time, until it stands out enough.
pub fn readable(accent: Rgba, scheme: &Scheme) -> Rgba {
    let ground = scheme.surface.over(scheme.ink);
    let toward = if scheme.light {
        Rgba::hex(0x000000)
    } else {
        Rgba::hex(0xffffff)
    };
    let mut color = accent;
    for step in 1..=10 {
        if color.contrast(ground) >= 3.0 {
            break;
        }
        color = accent.mix(toward, step as f32 * 0.08);
    }
    color
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_palette_reads() {
        for (name, scheme, accent) in NAMED {
            let ground = scheme.surface.over(scheme.ink);
            assert!(scheme.text.contrast(ground) >= 4.5, "{name}: text");
            assert!(
                readable(accent, &scheme).contrast(ground) >= 3.0,
                "{name}: accent"
            );
        }
    }

    #[test]
    fn a_light_scheme_darkens_a_pale_accent() {
        let (paper, _) = named("light").unwrap();
        let pale = Rgba::hex(0x86bdf0);
        let fixed = readable(pale, &paper);
        assert!(fixed.luminance() < pale.luminance());
        assert!(fixed.contrast(paper.surface.over(paper.ink)) >= 3.0);
    }

    #[test]
    fn the_wallpapers_hue_tints_its_neutrals() {
        let orange = Rgba::hex(0xff6a3d);
        for is_light in [false, true] {
            let scheme = tonal(orange, is_light);
            let ground = scheme.surface.over(scheme.ink);
            assert!(scheme.text.contrast(ground) >= 7.0);
            let hue = scheme.window.hue();
            assert!((hue - orange.hue()).abs() < 12.0, "{hue}");
        }
    }
}
