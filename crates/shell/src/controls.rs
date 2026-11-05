use tokio::process::Command;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Controls {
    pub volume: Option<Volume>,
    pub mic: Option<Volume>,
    pub brightness: Option<u8>,
    pub wifi: Option<Radio>,
    pub bluetooth: Option<Radio>,
    pub outputs: Vec<Device>,
    pub inputs: Vec<Device>,
    // The apps playing sound, each with its own level.
    pub streams: Vec<Stream>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stream {
    pub id: u32,
    pub name: String,
    pub volume: Volume,
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
    MicVolume(f32),
    MicMute(bool),
    AppVolume(u32, f32),
    AppMute(u32, bool),
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
            mic: Some(Volume {
                level: 0.8,
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
            streams: vec![Stream {
                id: 63,
                name: "Firefox".into(),
                volume: Volume {
                    level: 0.8,
                    muted: false,
                },
            }],
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
            Change::MicVolume(level) => {
                if let Some(mic) = &mut self.mic {
                    mic.level = level;
                    mic.muted = false;
                }
            }
            Change::MicMute(muted) => {
                if let Some(mic) = &mut self.mic {
                    mic.muted = muted;
                }
            }
            Change::AppVolume(id, level) => {
                if let Some(stream) = self.streams.iter_mut().find(|s| s.id == id) {
                    stream.volume.level = level;
                    stream.volume.muted = false;
                }
            }
            Change::AppMute(id, muted) => {
                if let Some(stream) = self.streams.iter_mut().find(|s| s.id == id) {
                    stream.volume.muted = muted;
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

// What the sound server said changed, a burst at a time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Heard {
    // An output or the server's defaults: the volume on the bar.
    pub output: bool,
    // A microphone, a sound card or an app's own stream, shown in quick settings.
    pub other: bool,
    // An app started or stopped recording.
    pub recording: bool,
}

impl Heard {
    fn all() -> Self {
        Self {
            output: true,
            other: true,
            recording: true,
        }
    }

    // A line of `pactl subscribe`: "Event 'change' on sink #47".
    fn with(self, line: &str) -> Self {
        let on = |what: &str| line.contains(&format!(" on {what}"));
        Self {
            output: self.output || on("sink #") || on("server"),
            other: self.other
                || on("server")
                || on("source #")
                || on("sink-input #")
                || on("card #"),
            recording: self.recording || on("source-output #"),
        }
    }

    pub fn any(self) -> bool {
        self != Self::default()
    }

    pub fn and(self, other: Self) -> Self {
        Self {
            output: self.output || other.output,
            other: self.other || other.other,
            recording: self.recording || other.recording,
        }
    }
}

// One `pactl subscribe` for the whole shell: the bar's volume, quick settings and who's recording
// are read again when it says they changed, never on a timer. Without a pulse server to listen
// to, everything is read every half minute.
pub fn heard() -> iced::Subscription<Heard> {
    iced::Subscription::run(listen)
}

fn listen() -> impl iced::futures::Stream<Item = Heard> {
    use iced::futures::SinkExt;
    use tokio::io::{AsyncBufReadExt, BufReader};
    iced::stream::channel(1, async |mut output| {
        loop {
            let mut command = Command::new("pactl");
            command
                .arg("subscribe")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true);
            crate::child::tie(command.as_std_mut(), libc::SIGTERM);
            let mut child = command.spawn().ok();
            let mut events = child
                .as_mut()
                .and_then(|child| child.stdout.take())
                .map(|out| BufReader::new(out).lines());
            // Everything at first: it may have changed while no one listened.
            let mut heard = Some(Heard::all());
            while let Some(now) = heard {
                if output.send(now).await.is_err() {
                    return;
                }
                heard = match &mut events {
                    Some(lines) => burst(lines).await,
                    None => None,
                };
            }
            // No pactl, no pulse server, or it restarted: look again in a while.
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    })
}

// Waits for an event worth reading again for, then lets its burst pass. None once pactl has
// nothing more to say.
async fn burst(
    lines: &mut tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>,
) -> Option<Heard> {
    let mut heard = Heard::default();
    while !heard.any() {
        heard = heard.with(&lines.next_line().await.ok()??);
    }
    let settle = std::time::Duration::from_millis(100);
    while let Ok(Ok(Some(line))) = tokio::time::timeout(settle, lines.next_line()).await {
        heard = heard.with(&line);
    }
    Some(heard)
}

pub async fn volume() -> Option<Volume> {
    run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .await
        .as_deref()
        .and_then(parse_volume)
}

// Wi-Fi and Bluetooth come from their own D-Bus watches, not from here.
pub async fn read() -> Controls {
    let (volume, mic, brightness, status) = iced::futures::join!(
        run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]),
        run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SOURCE@"]),
        run("brightnessctl", &["-m", "-c", "backlight"]),
        run("wpctl", &["status"]),
    );
    let (outputs, inputs) = status.as_deref().map(parse_devices).unwrap_or_default();
    // Each app's level at once, not one wpctl after another.
    let streams = status.as_deref().map(parse_streams).unwrap_or_default();
    let streams =
        iced::futures::future::join_all(streams.into_iter().map(|(id, name)| async move {
            let volume = run("wpctl", &["get-volume", &id.to_string()]).await;
            let volume = volume.as_deref().and_then(parse_volume)?;
            Some(Stream { id, name, volume })
        }))
        .await
        .into_iter()
        .flatten()
        .collect();
    Controls {
        streams,
        mic: mic
            .as_deref()
            .and_then(parse_volume)
            .filter(|_| !inputs.is_empty()),
        outputs,
        inputs,
        volume: volume.as_deref().and_then(parse_volume),
        brightness: brightness.as_deref().and_then(parse_brightness),
        wifi: None,
        bluetooth: None,
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
    Note(String),
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
        Change::MicVolume(level) => (
            "wpctl",
            vec![
                "set-volume".into(),
                "-l".into(),
                "1.0".into(),
                "@DEFAULT_AUDIO_SOURCE@".into(),
                format!("{:.2}", level.clamp(0.0, 1.0)),
            ],
        ),
        Change::MicMute(muted) => (
            "wpctl",
            vec![
                "set-mute".into(),
                "@DEFAULT_AUDIO_SOURCE@".into(),
                if muted { "1" } else { "0" }.into(),
            ],
        ),
        Change::AppVolume(id, level) => (
            "wpctl",
            vec![
                "set-volume".into(),
                id.to_string(),
                format!("{:.2}", level.clamp(0.0, 1.0)),
            ],
        ),
        Change::AppMute(id, muted) => (
            "wpctl",
            vec![
                "set-mute".into(),
                id.to_string(),
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
        // The shell switches radios over D-Bus itself, to hear back when it fails.
        Change::Wifi(_) | Change::Bluetooth(_) => return,
        Change::Output(id) | Change::Input(id) => {
            ("wpctl", vec!["set-default".into(), id.to_string()])
        }
    };
    match change {
        Change::Volume(_) => {
            let _ = run("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "0"]).await;
        }
        Change::MicVolume(_) => {
            let _ = run("wpctl", &["set-mute", "@DEFAULT_AUDIO_SOURCE@", "0"]).await;
        }
        _ => {}
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if run(program, &args).await.is_none() {
        eprintln!("apolline: {program} {} failed", args.join(" "));
    }
}

pub async fn run(program: &str, args: &[&str]) -> Option<String> {
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

// The apps playing sound: under Audio's Streams, each with its ports below it; a playing one has
// a port going out (">").
fn parse_streams(text: &str) -> Vec<(u32, String)> {
    let mut audio = false;
    let mut streams = false;
    let mut found: Vec<(u32, String, bool)> = Vec::new();
    for line in text.lines() {
        if !line.starts_with([' ', '│', '├', '└']) {
            audio = line.trim() == "Audio";
            streams = false;
            continue;
        }
        let item = line.trim_start_matches(|c: char| c.is_whitespace() || "│├└─".contains(c));
        if line.contains('─') {
            streams = item.trim() == "Streams:";
            continue;
        }
        if !(audio && streams) {
            continue;
        }
        let Some((id, rest)) = item.split_once(". ") else {
            continue;
        };
        let Ok(id) = id.trim().parse::<u32>() else {
            continue;
        };
        // A port: "64. output_FL > alsa_output…".
        if rest.contains(" > ") || rest.contains(" < ") {
            if rest.contains(" > ")
                && let Some(last) = found.last_mut()
            {
                last.2 = true;
            }
            continue;
        }
        let name = rest.split(" [").next().unwrap_or(rest).trim().to_owned();
        found.push((id, name, false));
    }
    found
        .into_iter()
        .filter(|(_, _, playing)| *playing)
        .map(|(id, name, _)| (id, name))
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_event_wakes_only_what_shows_it() {
        let heard = |line: &str| Heard::default().with(line);
        assert_eq!(
            heard("Event 'change' on sink #47"),
            Heard {
                output: true,
                ..Heard::default()
            }
        );
        assert_eq!(
            heard("Event 'new' on sink-input #63"),
            Heard {
                other: true,
                ..Heard::default()
            }
        );
        assert_eq!(
            heard("Event 'remove' on source-output #12"),
            Heard {
                recording: true,
                ..Heard::default()
            }
        );
        assert!(heard("Event 'change' on server #-1").output);
        assert!(
            !heard("Event 'change' on client #88").any(),
            "an app connecting"
        );
        assert!(!heard("Event 'new' on module #31").any());
        let both = heard("Event 'change' on sink #1").and(heard("Event 'change' on source #2"));
        assert!(both.output && both.other && !both.recording);
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
    fn the_apps_playing_are_the_streams_with_a_port_going_out() {
        let status = "Audio
 ├─ Sinks:
 │  *   47. Speakers                [vol: 0.45]
 │
 └─ Streams:
        63. Firefox
             64. output_FL       > alsa_output.pci:playback_FL	[active]
             65. output_FR       > alsa_output.pci:playback_FR	[active]
        70. spotify
             71. output_FL       > alsa_output.pci:playback_FL	[active]
        80. Zoom
             81. input_MONO      < alsa_input.pci:capture_MONO	[active]

Video
 └─ Streams:
        90. OBS
             91. output        > something
";
        assert_eq!(
            parse_streams(status),
            [(63, "Firefox".to_owned()), (70, "spotify".to_owned())]
        );
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
