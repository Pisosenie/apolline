use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capture {
    Area,
    Screen,
    Window,
    Color,
    Record,
    Mark,
}

impl Capture {
    pub fn command(self) -> Option<&'static str> {
        match self {
            Capture::Area => Some("niri msg action screenshot"),
            Capture::Screen => Some("sleep 0.3; niri msg action screenshot-screen"),
            Capture::Window => Some("sleep 0.3; niri msg action screenshot-window"),
            Capture::Color | Capture::Record | Capture::Mark => None,
        }
    }
}

pub async fn pick_color() -> Option<String> {
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    let out = tokio::process::Command::new("niri")
        .args(["msg", "-j", "pick-color"])
        .output()
        .await
        .ok()?;
    hex(&String::from_utf8_lossy(&out.stdout))
}

pub fn hex(reply: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(reply.trim()).ok()?;
    let rgb = value["rgb"].as_array()?;
    let channel = |i: usize| -> Option<u8> {
        Some((rgb.get(i)?.as_f64()?.clamp(0.0, 1.0) * 255.0).round() as u8)
    };
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        channel(0)?,
        channel(1)?,
        channel(2)?
    ))
}

pub struct Recording {
    child: Child,
    pub started: Instant,
    pub file: PathBuf,
}

impl Recording {
    pub fn start(output: Option<&str>) -> Result<Self, String> {
        let dir = videos().ok_or("no home folder")?;
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let file = dir.join(format!(
            "Recording {}.mp4",
            chrono::Local::now().format("%Y-%m-%d %H.%M.%S")
        ));
        let mut command = Command::new("wf-recorder");
        if let Some(output) = output {
            command.args(["-o", output]);
        }
        let child = crate::child::tie(&mut command, libc::SIGINT)
            .arg("-f")
            .arg(&file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| match err.kind() {
                std::io::ErrorKind::NotFound => {
                    "Install wf-recorder to record the screen".to_owned()
                }
                _ => format!("wf-recorder: {err}"),
            })?;
        Ok(Self {
            child,
            started: Instant::now(),
            file,
        })
    }

    pub fn quit(&mut self) -> bool {
        self.child.try_wait().is_ok_and(|status| status.is_some())
    }

    pub fn stop(self) -> PathBuf {
        let Self {
            mut child, file, ..
        } = self;
        let _ = Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status();
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        file
    }
}

fn videos() -> Option<PathBuf> {
    std::env::var_os("XDG_VIDEOS_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Videos")))
}

// The last few seconds of the screen and the sound, kept in memory by gpu-screen-recorder
// while a mode asks for it; `save` writes them out.
pub struct Replay {
    child: Option<Child>,
    pub seconds: u16,
}

impl Replay {
    // None when gpu-screen-recorder isn't installed: `apolline doctor` says so, not every mode switch.
    pub fn start(seconds: u16) -> Option<Result<Self, String>> {
        if !crate::launcher::on_path("gpu-screen-recorder") {
            return None;
        }
        let dir = videos()?.join("Replays");
        if let Err(err) = std::fs::create_dir_all(&dir) {
            return Some(Err(format!("{}: {err}", dir.display())));
        }
        let child = crate::child::tie(&mut Command::new("gpu-screen-recorder"), libc::SIGINT)
            .args([
                "-w",
                "screen",
                "-f",
                "60",
                "-c",
                "mp4",
                "-a",
                "default_output",
            ])
            .arg("-r")
            .arg(seconds.to_string())
            .arg("-o")
            .arg(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| format!("gpu-screen-recorder: {err}"));
        Some(child.map(|child| Self {
            child: Some(child),
            seconds,
        }))
    }

    pub fn running(&mut self) -> bool {
        self.child
            .as_mut()
            .is_some_and(|child| child.try_wait().is_ok_and(|status| status.is_none()))
    }

    pub fn save(&self) -> bool {
        self.child
            .as_ref()
            .is_some_and(|child| signal(child, libc::SIGUSR1))
    }
}

impl Drop for Replay {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            signal(&child, libc::SIGINT);
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

fn signal(child: &Child, signal: i32) -> bool {
    let Ok(pid) = i32::try_from(child.id()) else {
        return false;
    };
    // SAFETY: plain syscall on our own child, which isn't reaped while we hold it.
    unsafe { libc::kill(pid, signal) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picked_color_becomes_hex() {
        assert_eq!(
            hex(r#"{"rgb":[0.592156862745098,0.6352941176470588,0.6862745098039216]}"#).as_deref(),
            Some("#97a2af")
        );
        assert_eq!(hex("null"), None, "a cancelled pick");
    }
}
