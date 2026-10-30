use apolline_theme::{Mode, Rgba};
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
    Camera = "camera",
    Check = "check",
    ChevronRight = "chevron-right",
    Coffee = "coffee",
    Code = "file-code",
    Command = "command",
    Done = "circle-check",
    External = "external-link",
    Gamepad = "gamepad-2",
    Image = "image",
    Info = "info",
    Keyboard = "keyboard",
    Layers = "layers",
    Lock = "lock",
    LogOut = "log-out",
    Monitor = "monitor",
    Moon = "moon",
    Music = "music",
    Palette = "palette",
    PanelTop = "panel-top",
    Pause = "pause",
    Play = "play",
    Plus = "plus",
    Pointer = "mouse-pointer-2",
    Power = "power",
    Restart = "rotate-ccw",
    Search = "search",
    Send = "send",
    Settings = "settings",
    SkipBack = "skip-back",
    SkipForward = "skip-forward",
    Sun = "sun",
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
    pub fn of(mode: Mode) -> Self {
        match mode {
            Mode::Work => Icon::Briefcase,
            Mode::Play => Icon::Gamepad,
            Mode::Evening => Icon::Moon,
        }
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

pub fn icon<'a>(icon: Icon, size: f32, color: Rgba) -> Svg<'a> {
    let tint = style::color(color);
    svg(svg::Handle::from_memory(icon.bytes()))
        .width(size)
        .height(size)
        .style(move |_, _| svg::Style { color: Some(tint) })
}
