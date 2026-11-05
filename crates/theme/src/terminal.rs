use crate::{Rgba, Scheme};

// Where red, green, yellow, blue, magenta and cyan sit on the wheel, nudged a little toward the
// accent so a terminal's colors feel like the rest of the desktop.
const HUES: [f32; 6] = [4.0, 140.0, 42.0, 212.0, 300.0, 182.0];
const PULL: f32 = 0.06;

fn toward(hue: f32, accent: f32) -> f32 {
    let gap = (accent - hue + 540.0).rem_euclid(360.0) - 180.0;
    hue + gap * PULL
}

// The 16 colors a terminal draws text with, made from the scheme's ground and text: soft and light
// over dark, deeper over light, each one readable on the background.
pub fn colors(scheme: &Scheme, accent: Rgba) -> [Rgba; 16] {
    let ground = scheme.window.alpha(1.0);
    let ink = scheme.text.alpha(1.0);
    let ((s, l), (bright_s, bright_l)) = if scheme.light {
        ((0.70, 0.36), (0.78, 0.29))
    } else {
        ((0.62, 0.66), (0.72, 0.76))
    };
    let hue = |i: usize| toward(HUES[i], accent.hue());
    let tone = |i: usize, s: f32, l: f32| readable(Rgba::hsl(hue(i), s, l), ground, scheme.light);
    let (black, white, gray, brightest) = if scheme.light {
        (
            ink,
            ground.mix(ink, 0.25),
            ground.mix(ink, 0.6),
            ground.mix(ink, 0.12),
        )
    } else {
        (
            ground.mix(ink, 0.12),
            ground.mix(ink, 0.78),
            ground.mix(ink, 0.4),
            ink,
        )
    };
    let mut out = [black; 16];
    for i in 0..6 {
        out[1 + i] = tone(i, s, l);
        out[9 + i] = tone(i, bright_s, bright_l);
    }
    out[7] = white;
    out[8] = gray;
    out[15] = brightest;
    out
}

// Darker over a light ground, lighter over a dark one, until it reads.
fn readable(color: Rgba, ground: Rgba, light: bool) -> Rgba {
    let goal = if light {
        Rgba::hex(0x000000)
    } else {
        Rgba::hex(0xffffff)
    };
    let mut color = color;
    for _ in 0..12 {
        if color.contrast(ground) >= 3.0 {
            break;
        }
        color = color.mix(goal, 0.15);
    }
    color
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_palette_gets_colors_that_read_on_its_ground() {
        let palettes = crate::palettes::NAMED
            .iter()
            .map(|(name, scheme, accent)| (*name, *scheme, *accent));
        for (name, scheme, accent) in palettes {
            let ground = scheme.window.alpha(1.0);
            let colors = colors(&scheme, accent);
            for (i, color) in colors
                .iter()
                .enumerate()
                .filter(|(i, _)| !matches!(i, 0 | 7 | 8 | 15))
            {
                assert!(
                    color.contrast(ground) >= 3.0,
                    "{name}: color {i} {} on {}",
                    color.to_hex(),
                    ground.to_hex()
                );
            }
            assert!(colors[15].contrast(ground) > colors[8].contrast(ground) || scheme.light);
        }
    }
}
