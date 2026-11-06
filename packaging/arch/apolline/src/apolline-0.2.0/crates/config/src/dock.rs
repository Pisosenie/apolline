use crate::Edge;

// A row of apps along a screen edge. Off unless asked for: the bar already has windows and pins.
#[derive(Clone, Debug, PartialEq)]
pub struct Dock {
    pub show: bool,
    pub position: Edge,
    // Out of the way until the pointer reaches the screen edge, over windows rather than beside.
    pub autohide: bool,
    // The icons' size at rest, in pixels.
    pub size: u16,
    // Icons near the pointer grow.
    pub magnify: bool,
    // Screens without a dock, by name.
    pub skip: Vec<String>,
}

impl Dock {
    pub const SIZES: std::ops::RangeInclusive<u16> = 32..=96;
    // The edges a dock can sit on; the top is the bar's.
    pub const EDGES: [(&'static str, Edge); 3] = [
        ("bottom", Edge::Bottom),
        ("left", Edge::Left),
        ("right", Edge::Right),
    ];
    pub const OPTIONS: [&'static str; 6] =
        ["show", "position", "autohide", "size", "magnify", "skip"];

    pub fn shown_on(&self, output: Option<&str>) -> bool {
        self.show && output.is_none_or(|name| !self.skip.iter().any(|screen| screen == name))
    }
}

impl Default for Dock {
    fn default() -> Self {
        Self {
            show: false,
            position: Edge::Bottom,
            autohide: true,
            size: 48,
            magnify: true,
            skip: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Config, Edge};

    #[test]
    fn the_dock_is_off_until_asked_for() {
        let (config, problems) = Config::parse("");
        assert!(problems.is_empty());
        assert!(!config.dock.show);
        assert_eq!(config.dock.position, Edge::Bottom);
        assert!(config.dock.autohide && config.dock.magnify);
        assert_eq!(config.dock.size, 48);
    }

    #[test]
    fn the_dock_reads_every_option() {
        let (config, problems) = Config::parse(
            "[dock]\nshow = true\nposition = \"left\"\nautohide = false\nsize = 64\nmagnify = false\nskip = [\"HDMI-A-1\"]\n",
        );
        assert!(problems.is_empty(), "{problems:?}");
        let dock = &config.dock;
        assert!(dock.show && !dock.autohide && !dock.magnify);
        assert_eq!((dock.position, dock.size), (Edge::Left, 64));
        assert!(dock.shown_on(Some("DP-1")) && dock.shown_on(None));
        assert!(!dock.shown_on(Some("HDMI-A-1")));
    }

    #[test]
    fn a_wrong_dock_option_gets_a_hint() {
        let (config, problems) =
            Config::parse("[dock]\nshow = true\nmagnfy = true\nposition = \"top\"\nsize = 200\n");
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems.iter().any(|p| {
            p.message.contains("magnfy") && p.hint.as_deref().is_some_and(|h| h.contains("magnify"))
        }));
        assert!(problems.iter().any(|p| p.message.contains("top")));
        assert!(problems.iter().any(|p| p.message.contains("size")));
        assert_eq!((config.dock.position, config.dock.size), (Edge::Bottom, 48));
        let (_, problems) = Config::parse("[dok]\nshow = true\n");
        assert!(
            problems[0]
                .hint
                .as_deref()
                .is_some_and(|h| h.contains("dock"))
        );
    }

    #[test]
    fn the_dock_glass_has_its_corners_and_opacity() {
        let (config, problems) =
            Config::parse("[style.dock]\nradius = 12\nopacity = 0.2\nsize = 1.2\n");
        assert_eq!(problems.len(), 1, "size is [dock] size: {problems:?}");
        assert_eq!(config.style.dock.radius, Some(12));
        assert_eq!(config.style.dock.opacity, Some(0.2));
    }
}
