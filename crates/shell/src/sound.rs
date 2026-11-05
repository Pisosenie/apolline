use std::path::PathBuf;

use apolline_config::Sound;

use crate::notify::Heard;

const DEFAULT: &str = "message-new-instant";

// The sound a card makes: none when you turned sound off or the app plays its own; your file over
// anything the app asks for; otherwise the app's, or the theme's message sound.
pub fn pick(choice: &Sound, asked: &Heard) -> Option<PathBuf> {
    match (choice, asked) {
        (Sound::Off, _) | (_, Heard::Quiet) => None,
        (Sound::File(path), _) => crate::wallpaper::expand(path).filter(|p| p.is_file()),
        (Sound::Theme, Heard::File(path)) if path.is_file() => Some(path.clone()),
        (Sound::Theme, Heard::Named(name)) => themed(name).or_else(|| themed(DEFAULT)),
        (Sound::Theme, _) => themed(DEFAULT),
    }
}

// A sound by its theme name, from yours first and then the system's freedesktop theme.
fn themed(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') {
        return None;
    }
    let mut roots: Vec<PathBuf> = apolline_config::xdg_dir("XDG_DATA_HOME", ".local/share")
        .map(|data| data.join("sounds"))
        .into_iter()
        .collect();
    roots.push(PathBuf::from("/usr/share/sounds"));
    roots
        .iter()
        .flat_map(|root| {
            ["freedesktop/stereo", "stereo", ""]
                .into_iter()
                .flat_map(move |dir| {
                    ["oga", "ogg", "wav"]
                        .into_iter()
                        .map(move |ext| root.join(dir).join(format!("{name}.{ext}")))
                })
        })
        .find(|path| path.is_file())
}

// Through PipeWire, or PulseAudio's player where pw-play isn't.
pub async fn play(file: PathBuf) {
    for player in ["pw-play", "paplay"] {
        let ran = tokio::process::Command::new(player)
            .arg(&file)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;
        match ran {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            _ => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_wins_and_your_file_beats_the_apps() {
        let mine = std::env::temp_dir().join(format!("apolline-ding-{}.oga", std::process::id()));
        std::fs::write(&mine, b"").unwrap();
        let file = Sound::File(mine.display().to_string());
        assert_eq!(pick(&Sound::Off, &Heard::Nothing), None);
        assert_eq!(pick(&file, &Heard::Quiet), None, "the app plays its own");
        assert_eq!(
            pick(&file, &Heard::Named("bell".into())),
            Some(mine.clone()),
            "your file over what the app asks for"
        );
        assert_eq!(
            pick(&Sound::Theme, &Heard::File(mine.clone())),
            Some(mine.clone())
        );
        assert_eq!(themed("../etc/passwd"), None);
        std::fs::remove_file(mine).unwrap();
    }
}
