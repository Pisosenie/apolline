use tokio::process::Command;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Controls {
    pub volume: Option<Volume>,
    pub brightness: Option<u8>,
    pub wifi: Option<Radio>,
    pub bluetooth: Option<Radio>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    pub level: f32,
    pub muted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Radio {
    pub on: bool,
    pub connected: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Volume(f32),
    Mute(bool),
    Brightness(u8),
    Wifi(bool),
    Bluetooth(bool),
}

impl Controls {
    pub fn sample() -> Self {
        Self {
            volume: Some(Volume {
                level: 0.62,
                muted: false,
            }),
            brightness: Some(70),
            wifi: Some(Radio {
                on: true,
                connected: Some("Polina".into()),
            }),
            bluetooth: Some(Radio {
                on: true,
                connected: Some("WH-1000XM5".into()),
            }),
        }
    }

    pub fn apply(&mut self, change: &Change) {
        match *change {
            Change::Volume(level) => {
                if let Some(volume) = &mut self.volume {
                    volume.level = level;
                    volume.muted = false;
                }
            }
            Change::Mute(muted) => {
                if let Some(volume) = &mut self.volume {
                    volume.muted = muted;
                }
            }
            Change::Brightness(percent) => self.brightness = Some(percent),
            Change::Wifi(on) => {
                if let Some(wifi) = &mut self.wifi {
                    wifi.on = on;
                    wifi.connected = wifi.connected.take().filter(|_| on);
                }
            }
            Change::Bluetooth(on) => {
                if let Some(bluetooth) = &mut self.bluetooth {
                    bluetooth.on = on;
                    bluetooth.connected = bluetooth.connected.take().filter(|_| on);
                }
            }
        }
    }
}

pub async fn read() -> Controls {
    let (volume, brightness, wifi, wifi_network, bluetooth, bluetooth_device) = iced::futures::join!(
        run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]),
        run("brightnessctl", &["-m", "-c", "backlight"]),
        run("nmcli", &["-t", "-f", "WIFI", "radio"]),
        run(
            "nmcli",
            &[
                "-t",
                "-f",
                "ACTIVE,SSID",
                "device",
                "wifi",
                "list",
                "--rescan",
                "no"
            ]
        ),
        run("bluetoothctl", &["show"]),
        run("bluetoothctl", &["devices", "Connected"]),
    );
    Controls {
        volume: volume.as_deref().and_then(parse_volume),
        brightness: brightness.as_deref().and_then(parse_brightness),
        wifi: wifi.as_deref().map(|radio| Radio {
            on: radio.trim() == "enabled",
            connected: wifi_network.as_deref().and_then(parse_network),
        }),
        bluetooth: bluetooth
            .as_deref()
            .and_then(parse_powered)
            .map(|on| Radio {
                on,
                connected: bluetooth_device.as_deref().and_then(parse_device),
            }),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nudge {
    VolumeUp,
    VolumeDown,
    Mute,
    MicMute,
    BrightnessUp,
    BrightnessDown,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Osd {
    Volume(Volume),
    Mic(bool),
    Brightness(u8),
    Layout(String),
}

impl Nudge {
    pub fn parse(words: &[&str]) -> Option<Self> {
        Some(match words {
            ["volume", "up"] => Nudge::VolumeUp,
            ["volume", "down"] => Nudge::VolumeDown,
            ["volume", "mute"] => Nudge::Mute,
            ["mic", "mute"] => Nudge::MicMute,
            ["brightness", "up"] => Nudge::BrightnessUp,
            ["brightness", "down"] => Nudge::BrightnessDown,
            _ => return None,
        })
    }

    fn commands(self) -> &'static [&'static [&'static str]] {
        const SINK: &str = "@DEFAULT_AUDIO_SINK@";
        match self {
            Nudge::VolumeUp => &[
                &["wpctl", "set-mute", SINK, "0"],
                &["wpctl", "set-volume", "-l", "1.0", SINK, "5%+"],
            ],
            Nudge::VolumeDown => &[&["wpctl", "set-volume", SINK, "5%-"]],
            Nudge::Mute => &[&["wpctl", "set-mute", SINK, "toggle"]],
            Nudge::MicMute => &[&["wpctl", "set-mute", "@DEFAULT_AUDIO_SOURCE@", "toggle"]],
            Nudge::BrightnessUp => &[&["brightnessctl", "-c", "backlight", "set", "5%+"]],
            Nudge::BrightnessDown => {
                &[&["brightnessctl", "-c", "backlight", "-n", "1", "set", "5%-"]]
            }
        }
    }

    pub fn sample(self, controls: &mut Controls) -> Osd {
        let volume = controls.volume.get_or_insert(Volume {
            level: 0.5,
            muted: false,
        });
        match self {
            Nudge::VolumeUp => {
                volume.level = step(volume.level, 0.05);
                volume.muted = false;
            }
            Nudge::VolumeDown => volume.level = step(volume.level, -0.05),
            Nudge::Mute => volume.muted = !volume.muted,
            Nudge::MicMute => return Osd::Mic(true),
            Nudge::BrightnessUp | Nudge::BrightnessDown => {
                let step: i16 = if self == Nudge::BrightnessUp { 5 } else { -5 };
                let level = i16::from(controls.brightness.unwrap_or(50)) + step;
                let level = level.clamp(1, 100) as u8;
                controls.brightness = Some(level);
                return Osd::Brightness(level);
            }
        }
        Osd::Volume(*volume)
    }
}

fn step(level: f32, by: f32) -> f32 {
    ((level + by) * 100.0).round().clamp(0.0, 100.0) / 100.0
}

pub async fn nudge(nudge: Nudge) -> Option<Osd> {
    for command in nudge.commands() {
        run(command[0], &command[1..]).await?;
    }
    match nudge {
        Nudge::VolumeUp | Nudge::VolumeDown | Nudge::Mute => {
            let text = run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]).await?;
            parse_volume(&text).map(Osd::Volume)
        }
        Nudge::MicMute => {
            let text = run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SOURCE@"]).await?;
            parse_volume(&text).map(|mic| Osd::Mic(mic.muted))
        }
        Nudge::BrightnessUp | Nudge::BrightnessDown => {
            let text = run("brightnessctl", &["-m", "-c", "backlight"]).await?;
            parse_brightness(&text).map(Osd::Brightness)
        }
    }
}

pub async fn change(change: Change) {
    let (program, args): (&str, Vec<String>) = match change {
        Change::Volume(level) => (
            "wpctl",
            vec![
                "set-volume".into(),
                "-l".into(),
                "1.0".into(),
                "@DEFAULT_AUDIO_SINK@".into(),
                format!("{:.2}", level.clamp(0.0, 1.0)),
            ],
        ),
        Change::Mute(muted) => (
            "wpctl",
            vec![
                "set-mute".into(),
                "@DEFAULT_AUDIO_SINK@".into(),
                if muted { "1" } else { "0" }.into(),
            ],
        ),
        Change::Brightness(percent) => (
            "brightnessctl",
            vec![
                "-c".into(),
                "backlight".into(),
                "set".into(),
                format!("{}%", percent.clamp(1, 100)),
            ],
        ),
        Change::Wifi(on) => (
            "nmcli",
            vec!["radio".into(), "wifi".into(), on_off(on).into()],
        ),
        Change::Bluetooth(on) => ("bluetoothctl", vec!["power".into(), on_off(on).into()]),
    };
    if let Change::Volume(_) = change {
        let _ = run("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "0"]).await;
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if run(program, &args).await.is_none() {
        eprintln!("apolline: {program} {} failed", args.join(" "));
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

async fn run(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .kill_on_drop(true)
        .output()
        .await
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_volume(text: &str) -> Option<Volume> {
    let rest = text.trim().strip_prefix("Volume:")?.trim();
    let level = rest.split_whitespace().next()?.parse().ok()?;
    Some(Volume {
        level,
        muted: rest.contains("[MUTED]"),
    })
}

fn parse_brightness(text: &str) -> Option<u8> {
    let line = text.lines().next()?;
    let percent = line.split(',').nth(3)?.trim_end_matches('%');
    percent.parse().ok()
}

fn parse_network(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("yes:"))
        .map(|ssid| ssid.replace("\\:", ":"))
        .filter(|ssid| !ssid.is_empty())
}

fn parse_powered(text: &str) -> Option<bool> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("Powered:"))
        .map(|value| value.trim() == "yes")
}

fn parse_device(text: &str) -> Option<String> {
    let line = text.lines().find(|line| line.starts_with("Device "))?;
    let name = line.splitn(3, ' ').nth(2)?.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_the_tools_print() {
        assert_eq!(
            parse_volume("Volume: 0.45 [MUTED]\n"),
            Some(Volume {
                level: 0.45,
                muted: true
            })
        );
        assert_eq!(parse_volume("Volume: 1.00\n").map(|v| v.muted), Some(false));
        assert_eq!(parse_volume("nonsense"), None);
        assert_eq!(
            parse_brightness("intel_backlight,backlight,12000,50%,24000\n"),
            Some(50)
        );
        assert_eq!(
            parse_network("no:Neighbours\nyes:Home\\:5G\n").as_deref(),
            Some("Home:5G")
        );
        assert_eq!(parse_network("no:Neighbours\n"), None);
        assert_eq!(
            parse_powered("Controller 00:11 (public)\n\tName: box\n\tPowered: yes\n"),
            Some(true)
        );
        assert_eq!(
            parse_device("Device 88:C9:E8:00:00:01 WH-1000XM5\n").as_deref(),
            Some("WH-1000XM5")
        );
        assert_eq!(parse_device(""), None);
    }

    #[test]
    fn media_keys_parse_and_round_trip() {
        let keys = [
            ("volume up", Nudge::VolumeUp),
            ("volume down", Nudge::VolumeDown),
            ("volume mute", Nudge::Mute),
            ("mic mute", Nudge::MicMute),
            ("brightness up", Nudge::BrightnessUp),
            ("brightness down", Nudge::BrightnessDown),
        ];
        for (line, nudge) in keys {
            let words: Vec<&str> = line.split(' ').collect();
            assert_eq!(Nudge::parse(&words), Some(nudge));
        }
        assert_eq!(Nudge::parse(&["volume", "loud"]), None);
        let mut controls = Controls::sample();
        assert_eq!(
            Nudge::VolumeUp.sample(&mut controls),
            Osd::Volume(Volume {
                level: 0.67,
                muted: false
            })
        );
        assert_eq!(
            Nudge::BrightnessDown.sample(&mut controls),
            Osd::Brightness(65)
        );
    }

    #[test]
    fn turning_a_radio_off_forgets_the_connection() {
        let mut controls = Controls::sample();
        controls.apply(&Change::Wifi(false));
        assert_eq!(
            controls.wifi,
            Some(Radio {
                on: false,
                connected: None
            })
        );
        controls.apply(&Change::Volume(0.3));
        assert_eq!(controls.volume.map(|v| v.level), Some(0.3));
    }
}
