// What the bar is made of, placed by name in [bar] left, center and right.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Piece {
    Mode,
    Workspaces,
    Windows,
    Clock,
    Media,
    Notifications,
    Recording,
    // Packages waiting for an update.
    Updates,
    Stats,
    Tray,
    // The icons apps keep in the tray.
    Apps,
    // Every [[module]] not placed by its name.
    Modules,
    // A [[module]] by its name.
    Module(String),
}

impl Piece {
    pub const NAMED: [(&'static str, Piece); 12] = [
        ("mode", Piece::Mode),
        ("workspaces", Piece::Workspaces),
        ("windows", Piece::Windows),
        ("clock", Piece::Clock),
        ("media", Piece::Media),
        ("notifications", Piece::Notifications),
        ("recording", Piece::Recording),
        ("updates", Piece::Updates),
        ("stats", Piece::Stats),
        ("tray", Piece::Tray),
        ("apps", Piece::Apps),
        ("modules", Piece::Modules),
    ];

    pub fn named(name: &str) -> Piece {
        Self::NAMED
            .iter()
            .find(|(known, _)| *known == name)
            .map_or_else(
                || Piece::Module(name.to_owned()),
                |(_, piece)| piece.clone(),
            )
    }

    pub fn name(&self) -> &str {
        match self {
            Piece::Module(name) => name,
            piece => Self::NAMED
                .iter()
                .find(|(_, known)| known == piece)
                .map_or("", |(name, _)| name),
        }
    }

    pub fn left() -> Vec<Piece> {
        vec![Piece::Mode, Piece::Workspaces, Piece::Windows]
    }

    pub fn center() -> Vec<Piece> {
        vec![Piece::Clock]
    }

    pub fn right() -> Vec<Piece> {
        vec![
            Piece::Media,
            Piece::Notifications,
            Piece::Recording,
            Piece::Updates,
            Piece::Modules,
            Piece::Stats,
            Piece::Apps,
            Piece::Tray,
        ]
    }
}

// What the quick settings button on the bar shows, in [bar] tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrayItem {
    Keyboard,
    Awake,
    Wifi,
    Bluetooth,
    Volume,
    Battery,
}

impl TrayItem {
    pub const ALL: [(&'static str, TrayItem); 6] = [
        ("keyboard", TrayItem::Keyboard),
        ("awake", TrayItem::Awake),
        ("wifi", TrayItem::Wifi),
        ("bluetooth", TrayItem::Bluetooth),
        ("volume", TrayItem::Volume),
        ("battery", TrayItem::Battery),
    ];

    pub fn named(name: &str) -> Option<TrayItem> {
        Self::ALL
            .iter()
            .find(|(known, _)| *known == name)
            .map(|(_, item)| *item)
    }

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, item)| *item == self)
            .map_or("", |(name, _)| name)
    }

    pub fn all() -> Vec<TrayItem> {
        Self::ALL.iter().map(|(_, item)| *item).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;

    fn names(pieces: &[Piece]) -> Vec<&str> {
        pieces.iter().map(Piece::name).collect()
    }

    #[test]
    fn the_bar_is_laid_out_as_before_unless_told() {
        let (config, problems) = Config::parse("");
        assert!(problems.is_empty());
        assert_eq!(names(&config.bar.left), ["mode", "workspaces", "windows"]);
        assert_eq!(names(&config.bar.center), ["clock"]);
        assert_eq!(
            names(&config.bar.right),
            [
                "media",
                "notifications",
                "recording",
                "updates",
                "modules",
                "stats",
                "apps",
                "tray"
            ]
        );
        assert_eq!(config.bar.tray, TrayItem::all());
    }

    #[test]
    fn a_screen_can_have_its_own_pieces_or_no_bar() {
        let (config, problems) = Config::parse(
            r#"
[bar]
screens = ["eDP-1", "DP-1"]
left = ["workspaces"]

[bar."DP-1"]
right = ["stats"]
wrong = 1
"#,
        );
        assert_eq!(problems.len(), 1, "{problems:?}");
        let bar = &config.bar;
        assert!(bar.shown_on(Some("eDP-1")) && !bar.shown_on(Some("HDMI-A-1")));
        assert!(bar.shown_on(None), "without niri there's one bar");
        let (config, problems) = Config::parse(
            "[bar]\nskip = [\"DP-2\"]\nright = [\"stats\", \"wether\"]\n\n[bar.\"DP-2\"]\nleft = [\"workspaces\"]\n",
        );
        assert_eq!(problems.len(), 1, "said once: {problems:?}");
        assert_eq!(names(&config.bar.on(Some("DP-2")).right), ["stats"]);
        assert!(!config.bar.shown_on(Some("DP-2")) && config.bar.shown_on(Some("HDMI-A-1")));
        let screen = bar.on(Some("DP-1"));
        assert_eq!(names(&screen.right), ["stats"]);
        assert_eq!(
            names(&screen.left),
            ["workspaces"],
            "the rest as everywhere"
        );
        assert_eq!(names(&bar.on(Some("eDP-1")).right), names(&bar.right));
        assert!(bar.has(&Piece::Clock));
    }

    #[test]
    fn pieces_go_where_they_are_named_and_modules_by_their_name() {
        let (config, problems) = Config::parse(
            r#"
[bar]
left = ["clock"]
center = ["workspaces"]
right = ["weather", "tray"]
tray = ["battery", "volume"]

[[module]]
name = "weather"
command = "curl wttr.in"
"#,
        );
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(names(&config.bar.left), ["clock"]);
        assert_eq!(names(&config.bar.center), ["workspaces"]);
        assert_eq!(
            config.bar.right,
            [Piece::Module("weather".into()), Piece::Tray]
        );
        assert_eq!(config.bar.tray, [TrayItem::Battery, TrayItem::Volume]);
        assert_eq!(config.modules[0].name.as_deref(), Some("weather"));
    }

    #[test]
    fn a_misspelled_piece_is_reported_and_left_out() {
        let (config, problems) = Config::parse("[bar]\nleft = [\"clok\", \"workspaces\"]\n");
        assert_eq!(names(&config.bar.left), ["workspaces"]);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].line, 2);
        assert!(
            problems[0].message.contains("clok"),
            "{}",
            problems[0].message
        );
        assert_eq!(problems[0].hint.as_deref(), Some("did you mean “clock”?"));
    }

    #[test]
    fn a_piece_shows_once() {
        let (config, problems) = Config::parse(
            "[bar]\nleft = [\"clock\"]\ncenter = [\"clock\", \"media\"]\nright = [\"tray\"]\n",
        );
        assert_eq!(names(&config.bar.left), ["clock"]);
        assert_eq!(names(&config.bar.center), ["media"]);
        assert_eq!(problems.len(), 1);
        assert!(
            problems[0].message.contains("twice"),
            "{}",
            problems[0].message
        );
    }

    #[test]
    fn the_tray_takes_only_what_it_knows() {
        let (config, problems) = Config::parse("[bar]\ntray = [\"wifi\", \"radio\"]\n");
        assert_eq!(config.bar.tray, [TrayItem::Wifi]);
        assert_eq!(problems.len(), 1);
    }
}
