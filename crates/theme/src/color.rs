#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Rgba {
    pub const fn hex(rgb: u32) -> Self {
        Self {
            r: (rgb >> 16) as u8,
            g: (rgb >> 8) as u8,
            b: rgb as u8,
            a: 1.0,
        }
    }

    pub const fn alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let hex = s.strip_prefix('#')?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(hex, 16).ok().map(Self::hex)
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub fn over(self, below: Rgba) -> Rgba {
        let mix = |top: u8, bottom: u8| {
            (top as f32 * self.a + bottom as f32 * (1.0 - self.a)).round() as u8
        };
        Rgba {
            r: mix(self.r, below.r),
            g: mix(self.g, below.g),
            b: mix(self.b, below.b),
            a: 1.0,
        }
    }

    pub fn luminance(self) -> f32 {
        let linear = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }

    pub fn contrast(self, other: Rgba) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_full_hex() {
        assert_eq!(Rgba::parse("#ff6a3d"), Some(Rgba::hex(0xff6a3d)));
        assert_eq!(Rgba::parse("ff6a3d"), None);
        assert_eq!(Rgba::parse("#ff6a3"), None);
        assert_eq!(Rgba::parse("#+f6a3d"), None);
        assert_eq!(Rgba::hex(0x0b1724).to_hex(), "#0b1724");
    }

    #[test]
    fn contrast_matches_wcag_extremes() {
        let black = Rgba::hex(0x000000);
        let white = Rgba::hex(0xffffff);
        assert!((black.contrast(white) - 21.0).abs() < 0.01);
        assert!((white.contrast(white) - 1.0).abs() < 0.01);
    }
}
