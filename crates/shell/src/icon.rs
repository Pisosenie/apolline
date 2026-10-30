use apolline_theme::Rgba;
use iced::widget::{Svg, svg};

use crate::style;

macro_rules! icons {
    ($($name:ident = $file:literal,)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Icon {
            $($name,)*
        }

        impl Icon {
            fn bytes(self) -> &'static [u8] {
                match self {
                    $(Icon::$name => include_bytes!(concat!("../../../assets/icons/", $file, ".svg")),)*
                }
            }
        }
    };
}

icons! {
    AppWindow = "app-window",
    Arrow = "arrow-right",
    ArrowDown = "arrow-down",
    ArrowUp = "arrow-up",
    Book = "book-open",
    BatteryCharging = "battery-charging",
    BatteryFull = "battery-full",
    BatteryLow = "battery-low",
    BatteryMedium = "battery-medium",
    BatteryWarning = "battery-warning",
    Bell = "bell",
    BellOff = "bell-off",
    Bluetooth = "bluetooth",
    BluetoothOff = "bluetooth-off",
    Briefcase = "briefcase",
    Brush = "paintbrush",
    Camera = "camera",
    Check = "check",
    ChevronRight = "chevron-right",
    Circle = "circle-dashed",
    Clipboard = "clipboard",
    Coffee = "coffee",
    Download = "download",
    Folder = "folder-open",
    Code = "file-code",
    CodeXml = "code-xml",
    Command = "command",
    Done = "circle-check",
    External = "external-link",
    Gamepad = "gamepad-2",
    Headphones = "headphones",
    Image = "image",
    Info = "info",
    Keyboard = "keyboard",
    Layers = "layers",
    Link = "link",
    Lock = "lock",
    Mic = "mic",
    MicOff = "mic-off",
    LogOut = "log-out",
    Monitor = "monitor",
    Moon = "moon",
    Music = "music",
    Palette = "palette",
    PanelTop = "panel-top",
    Pause = "pause",
    Pin = "pin",
    Play = "play",
    Plus = "plus",
    Pointer = "mouse-pointer-2",
    Power = "power",
    Presentation = "presentation",
    Redo = "redo-2",
    Restart = "rotate-ccw",
    Search = "search",
    Send = "send",
    Settings = "settings",
    SkipBack = "skip-back",
    SkipForward = "skip-forward",
    Sparkles = "sparkles",
    Sun = "sun",
    Target = "target",
    Undo = "undo-2",
    Warning = "triangle-alert",
    Volume = "volume-2",
    VolumeLow = "volume-1",
    VolumeMuted = "volume-x",
    Wifi = "wifi",
    WifiOff = "wifi-off",
    X = "x",
    Zap = "zap",
}

impl Icon {
    pub fn named(name: &str) -> Self {
        MODE_ICONS
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(Icon::Sparkles, |(_, icon)| *icon)
    }

    pub fn battery(percent: u8, charging: bool) -> Self {
        match (charging, percent) {
            (true, _) => Icon::BatteryCharging,
            (_, 0..=10) => Icon::BatteryWarning,
            (_, 11..=35) => Icon::BatteryLow,
            (_, 36..=75) => Icon::BatteryMedium,
            _ => Icon::BatteryFull,
        }
    }

    pub fn volume(level: f32, muted: bool) -> Self {
        match (muted, level) {
            (true, _) => Icon::VolumeMuted,
            (_, l) if l <= 0.0 => Icon::VolumeMuted,
            (_, l) if l < 0.5 => Icon::VolumeLow,
            _ => Icon::Volume,
        }
    }
}

pub const MODE_ICONS: [(&str, Icon); 16] = [
    ("briefcase", Icon::Briefcase),
    ("gamepad", Icon::Gamepad),
    ("moon", Icon::Moon),
    ("target", Icon::Target),
    ("presentation", Icon::Presentation),
    ("code", Icon::CodeXml),
    ("book", Icon::Book),
    ("music", Icon::Music),
    ("headphones", Icon::Headphones),
    ("coffee", Icon::Coffee),
    ("sparkles", Icon::Sparkles),
    ("sun", Icon::Sun),
    ("zap", Icon::Zap),
    ("palette", Icon::Palette),
    ("monitor", Icon::Monitor),
    ("camera", Icon::Camera),
];

pub fn icon<'a>(icon: Icon, size: f32, color: Rgba) -> Svg<'a> {
    let tint = style::color(color);
    svg(svg::Handle::from_memory(icon.bytes()))
        .width(size)
        .height(size)
        .style(move |_, _| svg::Style { color: Some(tint) })
}
