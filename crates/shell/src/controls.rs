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
