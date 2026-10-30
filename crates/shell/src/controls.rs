use tokio::process::Command;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Controls {
    pub volume: Option<Volume>,
    pub brightness: Option<u8>,
    pub wifi: Option<Radio>,
    pub bluetooth: Option<Radio>,
    pub outputs: Vec<Device>,
    pub inputs: Vec<Device>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub id: u32,
    pub name: String,
    pub default: bool,
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
    Output(u32),
    Input(u32),
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
            outputs: vec![
                Device {
                    id: 47,
                    name: "Speakers".into(),
                    default: false,
                },
                Device {
                    id: 52,
                    name: "WH-1000XM5".into(),
                    default: true,
                },
            ],
            inputs: Vec::new(),
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
            Change::Output(id) => self.outputs.iter_mut().for_each(|d| d.default = d.id == id),
            Change::Input(id) => self.inputs.iter_mut().for_each(|d| d.default = d.id == id),
        }
    }
}

pub fn watch() -> iced::Subscription<Controls> {
    iced::Subscription::run(changes)
}

fn changes() -> impl iced::futures::Stream<Item = Controls> {
    iced::stream::channel(1, async |mut output| {
        use iced::futures::SinkExt;
        let mut last = None;
        loop {
            let now = read().await;
            if last.as_ref() != Some(&now) {
                last = Some(now.clone());
                if output.send(now).await.is_err() {
                    return;
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    })
}

pub async fn read() -> Controls {
    let (volume, brightness, wifi, wifi_network, bluetooth, bluetooth_device, status) = iced::futures::join!(
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
        run("wpctl", &["status"]),
    );
    let (outputs, inputs) = status.as_deref().map(parse_devices).unwrap_or_default();
    Controls {
        outputs,
        inputs,
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
        Change::Output(id) | Change::Input(id) => {
            ("wpctl", vec!["set-default".into(), id.to_string()])
        }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub ssid: String,
    pub signal: u8,
    pub secure: bool,
    pub active: bool,
    pub known: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paired {
    pub mac: String,
    pub name: String,
    pub connected: bool,
}

pub fn sample_networks() -> Vec<Network> {
    [
        ("Polina", 82, true, true, true),
        ("Neighbours", 64, true, false, false),
        ("Cafe: Free", 41, false, false, false),
    ]
    .into_iter()
    .map(|(ssid, signal, secure, active, known)| Network {
        ssid: ssid.into(),
        signal,
        secure,
        active,
        known,
    })
    .collect()
}

pub async fn networks() -> Vec<Network> {
    let (list, known) = iced::futures::join!(
        run(
            "nmcli",
            &[
                "-t",
                "-f",
                "IN-USE,SSID,SIGNAL,SECURITY",
                "device",
                "wifi",
                "list",
                "--rescan",
                "auto"
            ]
        ),
        run("nmcli", &["-t", "-f", "NAME,TYPE", "connection", "show"]),
    );
    let known: Vec<String> = known
        .as_deref()
        .unwrap_or_default()
        .lines()
        .map(fields)
        .filter(|f| f.get(1).is_some_and(|kind| kind.contains("wireless")))
        .filter_map(|f| f.into_iter().next())
        .collect();
    let mut networks: Vec<Network> = Vec::new();
    for line in list.as_deref().unwrap_or_default().lines() {
        let f = fields(line);
        let [used, ssid, signal, security] = f.as_slice() else {
            continue;
        };
        if ssid.is_empty() {
            continue;
        }
        let network = Network {
            ssid: ssid.clone(),
            signal: signal.parse().unwrap_or(0),
            secure: !security.trim().is_empty() && security.trim() != "--",
            active: used.trim() == "*",
            known: known.contains(ssid),
        };
        match networks.iter_mut().find(|n| n.ssid == network.ssid) {
            Some(seen) if seen.signal >= network.signal && !network.active => {}
            Some(seen) => *seen = network,
            None => networks.push(network),
        }
    }
    networks.sort_by_key(|n| (!n.active, std::cmp::Reverse(n.signal)));
    networks
}

pub async fn join(network: Network, password: Option<String>) -> Result<(), String> {
    if network.known && password.is_none() {
        return answer("nmcli", &["connection", "up", "id", &network.ssid], None).await;
    }
    answer(
        "nmcli",
        &["--ask", "device", "wifi", "connect", &network.ssid],
        password.map(|p| format!("{p}\n")),
    )
    .await
}

pub async fn paired() -> Vec<Paired> {
    let (paired, connected) = iced::futures::join!(
        run("bluetoothctl", &["devices", "Paired"]),
        run("bluetoothctl", &["devices", "Connected"]),
    );
    let connected = connected.unwrap_or_default();
    paired
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("Device ")?;
            let (mac, name) = rest.split_once(' ')?;
            Some(Paired {
                mac: mac.to_owned(),
                name: name.trim().to_owned(),
                connected: connected.contains(mac),
            })
        })
        .collect()
}

pub async fn pair(device: Paired) -> Result<(), String> {
    let verb = if device.connected {
        "disconnect"
    } else {
        "connect"
    };
    answer("bluetoothctl", &[verb, &device.mac], None).await
}

fn fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let (Some(next), Some(f)) = (chars.next(), out.last_mut()) {
                    f.push(next);
                }
            }
            ':' => out.push(String::new()),
            _ => {
                if let Some(f) = out.last_mut() {
                    f.push(c);
                }
            }
        }
    }
    out
}

async fn answer(program: &str, args: &[&str], input: Option<String>) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    let mut child = Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|err| format!("{program}: {err}"))?;
    if let (Some(mut stdin), Some(input)) = (child.stdin.take(), input) {
        let _ = stdin.write_all(input.as_bytes()).await;
    }
    let output = tokio::time::timeout(std::time::Duration::from_secs(45), child.wait_with_output())
        .await
        .map_err(|_| "it took too long".to_owned())?
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&output.stderr).into_owned()
        + &String::from_utf8_lossy(&output.stdout);
    Err(said
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("it didn't work")
        .trim_start_matches("Error: ")
        .to_owned())
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

fn parse_devices(text: &str) -> (Vec<Device>, Vec<Device>) {
    let (mut outputs, mut inputs) = (Vec::new(), Vec::new());
    let mut audio = false;
    let mut list: Option<bool> = None;
    for line in text.lines() {
        if !line.starts_with([' ', '│', '├', '└']) {
            audio = line.trim() == "Audio";
            list = None;
            continue;
        }
        let item = line.trim_start_matches(|c: char| c.is_whitespace() || "│├└─".contains(c));
        if line.contains('─') {
            list = match item.trim() {
                "Sinks:" => Some(true),
                "Sources:" => Some(false),
                _ => None,
            };
            continue;
        }
        let (Some(sink), true) = (list, audio) else {
            continue;
        };
        let (default, item) = match item.strip_prefix('*') {
            Some(rest) => (true, rest.trim_start()),
            None => (false, item),
        };
        let Some((id, rest)) = item.split_once(". ") else {
            continue;
        };
        let Ok(id) = id.trim().parse() else {
            continue;
        };
        let name = rest.split(" [").next().unwrap_or(rest).trim().to_owned();
        let device = Device { id, name, default };
        if sink {
            outputs.push(device);
        } else {
            inputs.push(device);
        }
    }
    (outputs, inputs)
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
    fn nmcli_fields_keep_escaped_colons() {
        assert_eq!(fields("*:Cafe\\: Free:41:"), ["*", "Cafe: Free", "41", ""]);
        assert_eq!(fields(" :Polina:82:WPA2"), [" ", "Polina", "82", "WPA2"]);
    }

    #[test]
    fn wpctl_status_lists_outputs_and_inputs() {
        let status = "PipeWire 'pipewire-0' [1.4.2, dzorin@laptop, cookie:1]
 └─ Clients:
        33. WirePlumber                         [1.4.2, dzorin@laptop, pid:1022]

Audio
 ├─ Devices:
 │      42. Built-in Audio                      [alsa]
 │
 ├─ Sinks:
 │      47. Built-in Audio Analog Stereo        [vol: 0.40]
 │  *   52. WH-1000XM5                          [vol: 0.55]
 │
 ├─ Sources:
 │  *   48. Built-in Audio Analog Stereo        [vol: 1.00]
 │
 ├─ Filters:
 │
 └─ Streams:
        61. Firefox

Video
 ├─ Devices:
 │      50. Integrated Camera                   [v4l2]
 ├─ Sinks:
 │
 ├─ Sources:
 │  *   53. Integrated Camera (V4L2)
";
        let (outputs, inputs) = parse_devices(status);
        assert_eq!(
            outputs,
            [
                Device {
                    id: 47,
                    name: "Built-in Audio Analog Stereo".into(),
                    default: false
                },
                Device {
                    id: 52,
                    name: "WH-1000XM5".into(),
                    default: true
                },
            ]
        );
        assert_eq!(inputs.len(), 1, "cameras are not microphones");
        assert!(inputs[0].default);
    }

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
