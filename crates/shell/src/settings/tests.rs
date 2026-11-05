use super::*;
use apolline_config::{Mouse, Touchpad};

#[test]
fn the_bar_can_show_the_machine() {
    let dir = std::env::temp_dir().join(format!("apolline-stats-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "").unwrap();
    let (mut settings, _) = Settings::open(path.clone(), None);
    let _ = settings.update(Message::Stat("temperature", true));
    let _ = settings.update(Message::Stat("cpu", true));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(r#"stats = ["cpu", "temperature"]"#), "{text}");
    assert!(settings.config.bar.stats.cpu && !settings.config.bar.stats.memory);
    let _ = settings.update(Message::Stat("cpu", false));
    let _ = settings.update(Message::Stat("temperature", false));
    assert!(!std::fs::read_to_string(&path).unwrap().contains("stats"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn night_light_gets_its_hours_and_warmth() {
    let dir = std::env::temp_dir().join(format!("apolline-night-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "[layout]\ngaps = 8\n").unwrap();
    let (mut settings, _) = Settings::open(path.clone(), None);

    let _ = settings.update(Message::NightSchedule(true));
    assert_eq!(settings.config.night.hours, Some((21 * 60, 7 * 60)));
    let _ = settings.update(Message::NightDraft("22:30-6:00".into()));
    let _ = settings.update(Message::SetNight);
    assert_eq!(settings.config.night.hours, Some((22 * 60 + 30, 6 * 60)));
    let _ = settings.update(Message::NightDraft("late".into()));
    let _ = settings.update(Message::SetNight);
    assert_eq!(
        settings.config.night.hours,
        Some((22 * 60 + 30, 6 * 60)),
        "nonsense is kept out"
    );
    let _ = settings.update(Message::NightWarmth(3500));
    let _ = settings.update(Message::Commit);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("[night]\nhours = \"22:30-06:00\"\ntemperature = 3500"),
        "{text}"
    );
    let _ = settings.update(Message::NightSchedule(false));
    assert_eq!(settings.config.night.hours, None);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_monitor_can_have_its_own_wallpaper_and_give_it_back() {
    let dir = std::env::temp_dir().join(format!("apolline-monitors-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "[desktop]\nwallpaper = \"fog.jpg\"\n").unwrap();
    let (mut settings, _) = Settings::open(path.clone(), None);
    settings.monitors = vec!["eDP-1".into(), "HDMI-A-1".into()];

    let _ = settings.update(Message::ForMonitor(Some("HDMI-A-1".into())));
    let _ = settings.update(Message::DesktopWallpaper("night.jpg".into()));
    assert_eq!(
        settings.config.desktop.outputs,
        [("HDMI-A-1".to_owned(), "night.jpg".to_owned())]
    );
    assert_eq!(settings.config.desktop.wallpaper, "fog.jpg");

    let _ = settings.update(Message::DesktopWallpaper("fog.jpg".into()));
    assert!(
        settings.config.desktop.outputs.is_empty(),
        "same as everywhere drops the line"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn going_back_on_a_display_change_leaves_other_changes_alone() {
    let dir = std::env::temp_dir().join(format!("apolline-revert-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "[layout]\nshadows = true\n").unwrap();
    let (mut settings, _) = Settings::open(path.clone(), None);

    let _ = settings.update(Message::DisplayTurn("eDP-1".into(), "normal"));
    assert!(
        settings.revert.is_none(),
        "nothing changed, nothing to take back"
    );

    let _ = settings.update(Message::DisplayScale("eDP-1".into(), 125));
    let _ = settings.update(Message::Shadows(false));
    let _ = settings.update(Message::RevertDisplays);
    assert!(settings.config.displays.is_empty());
    assert!(!settings.config.layout.shadows, "the other change stays");
    assert!(settings.revert.is_none());

    let _ = settings.update(Message::DisplayVrr("eDP-1".into(), true));
    let _ = settings.update(Message::DisplayVrr("eDP-1".into(), false));
    assert!(
        settings.config.displays.is_empty(),
        "off goes back to games-only"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn undo_walks_the_file_back_and_redo_forward() {
    let dir = std::env::temp_dir().join(format!("apolline-undo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "# mine\n[layout]\ngaps = 10\n").unwrap();
    let (mut settings, _) = Settings::open(path.clone(), None);

    let _ = settings.update(Message::Gaps(20));
    let _ = settings.update(Message::Commit);
    let _ = settings.update(Message::ApplyTheme(1));
    assert_eq!(settings.config.desktop.theme.as_deref(), Some("Ember"));

    let _ = settings.update(Message::Undo);
    assert_eq!(settings.config.desktop.theme, None);
    assert_eq!(settings.config.layout.gaps, 20);
    let _ = settings.update(Message::Undo);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "# mine\n[layout]\ngaps = 10\n"
    );
    let _ = settings.update(Message::Undo);
    assert_eq!(settings.config.layout.gaps, 10, "nothing left to undo");

    let _ = settings.update(Message::Redo);
    assert_eq!(settings.config.layout.gaps, 20);
    let _ = settings.update(Message::Gaps(4));
    let _ = settings.update(Message::Commit);
    assert!(
        settings.redo.is_empty(),
        "a new change drops what was undone"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn clicks_end_up_in_the_file_and_comments_survive() {
    let dir = std::env::temp_dir().join(format!("apolline-settings-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, include_str!("../../../../assets/config.toml")).unwrap();

    let (mut settings, _) = Settings::open(path.clone(), None);
    let _ = settings.update(Message::AddMode("Work"));
    let _ = settings.update(Message::AddMode("Evening"));
    assert_eq!(settings.selected, 1);
    let _ = settings.update(Message::ModeAccent(Some(Accent::Fixed(Rgba::hex(
        0xf472b6,
    )))));
    let _ = settings.update(Message::Notifications(1, Notifications::Hold));
    let _ = settings.update(Message::ModeName("Night Owl".into()));
    let _ = settings.update(Message::ModeNameDone);
    let _ = settings.update(Message::StartHere(true));
    let _ = settings.update(Message::RecordModeKey);
    let _ = settings.update(Message::Key(keyboard::Event::KeyPressed {
        key: keyboard::Key::Character("n".into()),
        modified_key: keyboard::Key::Character("n".into()),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::LOGO,
        text: None,
        repeat: false,
    }));
    let _ = settings.update(Message::KeyCommand("foot -e btop".into()));
    let _ = settings.update(Message::AddModeKey);
    let _ = settings.update(Message::MoveMode(-1));
    assert_eq!(settings.selected, 0);
    let _ = settings.update(Message::Draft("mpv --shuffle ~/Music".into()));
    let _ = settings.update(Message::Add);
    let _ = settings.update(Message::Draft("foot".into()));
    let _ = settings.update(Message::Add);
    let _ = settings.update(Message::Remove(0));
    let _ = settings.update(Message::Gaps(14));
    let _ = settings.update(Message::Commit);
    let _ = settings.update(Message::AddLayout(Layout("ru", "Russian")));
    let _ = settings.update(Message::Switch(Switch::Caps));
    let _ = settings.update(Message::Tap(false));
    let _ = settings.update(Message::TouchSpeed(0.25));
    let _ = settings.update(Message::Commit);

    let text = std::fs::read_to_string(&path).unwrap();
    let (config, problems) = Config::parse(&text);
    assert_eq!(problems, vec![]);
    let evening = &config.modes[0];
    assert_eq!(evening.name, "Night Owl");
    assert_eq!(evening.accent, Some(Accent::Fixed(Rgba::hex(0xf472b6))));
    assert_eq!(evening.notifications, Notifications::Hold);
    assert_eq!(evening.open, ["foot"]);
    assert_eq!(
        evening.keys,
        [("Mod+N".to_owned(), "foot -e btop".to_owned())]
    );
    assert_eq!(config.start(), Some(0));
    assert_eq!(config.modes[1].name, "Work");

    let _ = settings.update(Message::RemoveMode);
    let _ = settings.update(Message::RemoveMode);
    let (config, problems) = Config::load(&path);
    assert_eq!(problems, vec![]);
    assert_eq!(config.modes.len(), 1);
    assert_eq!(config.start(), None, "deleting the start mode clears it");
    assert_eq!(config.layout.gaps, 14);
    assert_eq!(config.keyboard.layouts, ["ru"]);
    assert_eq!(config.keyboard.switch, Some(Switch::Caps));
    assert_eq!(
        config.touchpad,
        Some(Touchpad {
            tap: false,
            speed: 0.25,
            ..Touchpad::default()
        })
    );
    assert_eq!(config.mouse, None::<Mouse>);
    assert!(text.contains("# Check it at any time with:  apolline check"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_shortcut_cannot_take_keys_another_one_uses() {
    let dir = std::env::temp_dir().join(format!("apolline-keys-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");

    let (mut settings, _) = Settings::open(path.clone(), None);
    settings.record(Shortcut::Panel, "Super+Space".into());
    assert!(
        settings
            .shortcut_error
            .as_deref()
            .unwrap()
            .contains("Search apps")
    );
    settings.record(Shortcut::Panel, "Mod+Shift+A".into());
    assert_eq!(settings.shortcut_error, None);
    let (config, _) = Config::load(&path);
    assert_eq!(config.shortcuts.panel, "Mod+Shift+A");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn comments_inside_strings_are_not_comments() {
    assert_eq!(
        split_comment(r##" "#86bdf0" # a color"##),
        (r##" "#86bdf0" "##, "# a color")
    );
    assert_eq!(split_comment(" 10"), (" 10", ""));
}

#[test]
fn a_search_finds_a_setting_on_its_page_in_either_language() {
    let dir = std::env::temp_dir().join(format!("apolline-find-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "").unwrap();
    let (mut settings, _) = Settings::open(path.clone(), None);
    let found = |settings: &Settings, query: &str| -> Vec<(Page, String)> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        settings
            .index()
            .into_iter()
            .flat_map(|(page, rows)| rows.into_iter().map(move |row| (page, row)))
            .filter(|(page, row)| find::matches(&words, row, *page))
            .map(|(page, row)| (page, row.title))
            .collect()
    };
    let blur = found(&settings, "blur wallpaper");
    assert!(
        blur.contains(&(Page::Lock, "Blur the wallpaper".to_owned())),
        "{blur:?}"
    );
    let index = settings.index();
    let lock = &index
        .iter()
        .find(|(page, _)| *page == Page::Lock)
        .unwrap()
        .1;
    let row = lock
        .iter()
        .find(|r| r.title == "Blur the wallpaper")
        .unwrap();
    assert_eq!(row.group, "Look", "a row knows its group");

    crate::i18n::choose(Some("ru"));
    assert!(
        !found(&settings, "размыть").is_empty() || !found(&settings, "размытие").is_empty(),
        "Russian words find it too"
    );
    crate::i18n::choose(Some("en"));

    let _ = settings.update(Message::FoundSetting(
        Page::Lock,
        "Blur the wallpaper".into(),
    ));
    assert_eq!(settings.page, Page::Lock);
    assert_eq!(
        settings.spot.as_ref().map(|(_, t)| t.as_str()),
        Some("Blur the wallpaper")
    );
    let _ = settings.update(Message::Page(Page::Bar));
    assert!(settings.spot.is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_color_of_your_own_is_kept_on_enter_and_given_back() {
    let dir = std::env::temp_dir().join(format!("apolline-colors-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "[layout]\ngaps = 8\n").unwrap();
    let (mut settings, _) = Settings::open(path.clone(), None);
    let _ = settings.update(Message::ColorDraft("window", "zzz".into()));
    let _ = settings.update(Message::ColorSet("window"));
    assert!(
        !std::fs::read_to_string(&path).unwrap().contains("[colors]"),
        "nonsense isn't kept"
    );
    let _ = settings.update(Message::ColorDraft("window", "1E1E2E".into()));
    let _ = settings.update(Message::ColorSet("window"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("[colors]\nwindow = \"#1e1e2e\""), "{text}");
    assert_eq!(settings.config.scheme(None).window.to_hex(), "#1e1e2e");
    let _ = settings.update(Message::ColorReset("window"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("colors"), "{text}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_tile_dropped_off_the_board_leaves_quick_settings() {
    let dir = std::env::temp_dir().join(format!("apolline-panel-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "").unwrap();
    let (mut settings, _) = Settings::open(path.clone(), None);
    let held = board::Held {
        name: "night".into(),
        grab: iced::Vector::ZERO,
        at: iced::Point::ORIGIN,
    };
    let _ = settings.update(Message::TilesHold(Some(held)));
    let _ = settings.update(Message::TilesDrop(iced::Point::new(40.0, 900.0), 400.0));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains(
            "[panel]\ntiles = [\"wifi\", \"bluetooth\", \"quiet\", \"awake\", \"power\", \"vpn\"]"
        ),
        "{text}"
    );
    assert!(!settings.config.panel.tiles.iter().any(|t| t == "night"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_clock_format_is_kept_only_when_chrono_reads_it() {
    let dir = std::env::temp_dir().join(format!("apolline-clock-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "").unwrap();
    let (mut settings, _) = Settings::open(path.clone(), None);
    let _ = settings.update(Message::ClockDraft("time", "%Q:%M".into()));
    let _ = settings.update(Message::ClockSet("time"));
    assert!(!std::fs::read_to_string(&path).unwrap().contains("clock"));
    let _ = settings.update(Message::ClockDraft("time", "%-I:%M %p".into()));
    let _ = settings.update(Message::ClockSet("time"));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("[clock]\ntime = \"%-I:%M %p\""), "{text}");
    assert_eq!(settings.config.clock.hours(), "%-I");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_launcher_sources_and_search_site_are_written_when_they_make_sense() {
    let dir = std::env::temp_dir().join(format!("apolline-finds-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "").unwrap();
    let (mut settings, _) = Settings::open(path.clone(), None);
    let _ = settings.update(Message::LauncherFinds("files", false));
    assert!(!settings.config.launcher.finds("files"));
    let _ = settings.update(Message::LauncherFinds("files", true));
    assert!(
        !std::fs::read_to_string(&path).unwrap().contains("finds"),
        "all on says nothing"
    );
    let _ = settings.update(Message::WebDraft("example.com".into()));
    let _ = settings.update(Message::WebSet);
    assert!(
        settings.config.launcher.web.is_none(),
        "not a link with {{query}}"
    );
    let _ = settings.update(Message::WebDraft(
        "https://www.startpage.com/do/search?q={query}".into(),
    ));
    let _ = settings.update(Message::WebSet);
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("startpage")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_sound_is_off_the_themes_or_a_file_of_yours() {
    let dir = std::env::temp_dir().join(format!("apolline-sound-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "").unwrap();
    let (mut settings, _) = Settings::open(path.clone(), None);
    let _ = settings.update(Message::SoundKind("theme"));
    assert_eq!(settings.config.alerts.sound, apolline_config::Sound::Theme);
    let _ = settings.update(Message::SoundKind("file"));
    let _ = settings.update(Message::SoundDraft("~/ding.oga".into()));
    let _ = settings.update(Message::SoundSet);
    assert_eq!(
        settings.config.alerts.sound,
        apolline_config::Sound::File("~/ding.oga".into())
    );
    let _ = settings.update(Message::SoundKind("off"));
    assert!(!std::fs::read_to_string(&path).unwrap().contains("sound"));
    std::fs::remove_dir_all(dir).unwrap();
}
