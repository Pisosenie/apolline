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
    Calculator = "calculator",
    Camera = "camera",
    Check = "check",
    ChevronLeft = "chevron-left",
    ChevronRight = "chevron-right",
    Circle = "circle-dashed",
    Clipboard = "clipboard",
    Cloud = "cloud",
    CloudDrizzle = "cloud-drizzle",
    CloudFog = "cloud-fog",
    CloudLightning = "cloud-lightning",
    CloudRain = "cloud-rain",
    CloudSnow = "cloud-snow",
    CloudSun = "cloud-sun",
    Coffee = "coffee",
    Download = "download",
    File = "file",
    Fingerprint = "fingerprint",
    Folder = "folder-open",
    Code = "file-code",
    Copy = "copy",
    Cpu = "cpu",
    CodeXml = "code-xml",
    Command = "command",
    Done = "circle-check",
    EyeOff = "eye-off",
    External = "external-link",
    Globe = "globe",
    Gamepad = "gamepad-2",
    Gauge = "gauge",
    Highlighter = "highlighter",
    Headphones = "headphones",
    Image = "image",
    Info = "info",
    Keyboard = "keyboard",
    Laptop = "laptop",
    Layers = "layers",
    Leaf = "leaf",
    Link = "link",
    Lock = "lock",
    Mic = "mic",
    MicOff = "mic-off",
    LogOut = "log-out",
    Monitor = "monitor",
    MemoryStick = "memory-stick",
    MoveUpRight = "move-up-right",
    ListOrdered = "list-ordered",
    MapPin = "map-pin",
    Moon = "moon",
    Mouse = "mouse",
    Music = "music",
    Pencil = "pencil",
    Palette = "palette",
    PanelTop = "panel-top",
    Pause = "pause",
    Pin = "pin",
    Pipette = "pipette",
    Play = "play",
    Plus = "plus",
    Pointer = "mouse-pointer-2",
    Power = "power",
    Presentation = "presentation",
    Redo = "redo-2",
    Restart = "rotate-ccw",
    Save = "save",
    Scan = "scan-line",
    Search = "search",
    Send = "send",
    Settings = "settings",
    SkipBack = "skip-back",
    SkipForward = "skip-forward",
    Smartphone = "smartphone",
    Speaker = "speaker",
    Square = "square",
    Sparkles = "sparkles",
    Sun = "sun",
    Target = "target",
    Terminal = "square-terminal",
    Thermometer = "thermometer",
    Trash = "trash-2",
    Type = "type",
    Undo = "undo-2",
    Video = "video",
    Warning = "triangle-alert",
    Volume = "volume-2",
    Watch = "watch",
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
