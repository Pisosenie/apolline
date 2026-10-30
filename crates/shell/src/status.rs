use apolline_theme::Mode;

use crate::media::Track;
use crate::power::Battery;

#[derive(Clone, Default)]
pub struct Status {
    pub workspaces: usize,
    pub active: usize,
    pub busy: Vec<usize>,
    pub ids: Vec<u64>,
    pub keyboard: String,
    pub battery: Option<Battery>,
    pub media: Option<Track>,
}

impl Status {
    pub fn sample(mode: Mode) -> Self {
        let (active, busy, media) = match mode {
            Mode::Work => (0, vec![0, 1, 2], Some(("Harbor Lights", "Low Tide"))),
            Mode::Play => (2, vec![0, 2], None),
            Mode::Evening => (3, vec![3], Some(("Night Drive", "Low Tide"))),
        };
        Self {
            workspaces: 5,
            active,
            busy,
            ids: Vec::new(),
            keyboard: "EN".into(),
            battery: Some(Battery {
                percent: 82,
                charging: false,
            }),
            media: media.map(|(title, artist)| Track::sample(title, artist)),
        }
    }

    pub fn from_niri(state: &apolline_niri::State, output: Option<&str>) -> Self {
        let workspaces = match output {
            Some(_) => state.workspaces_on(output),
            None => state.workspaces(),
        };
        Self {
            workspaces: workspaces.len(),
            active: workspaces.iter().position(|w| w.is_active).unwrap_or(0),
            busy: (0..workspaces.len())
                .filter(|&i| state.is_busy(workspaces[i].id))
                .collect(),
            ids: workspaces.iter().map(|w| w.id).collect(),
            keyboard: state.layout().map(short_layout).unwrap_or_default(),
            battery: None,
            media: None,
        }
    }
}

pub fn short_layout(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphabetic())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_names_shorten_to_two_letters() {
        assert_eq!(short_layout("English (US)"), "EN");
        assert_eq!(short_layout("Russian"), "RU");
        assert_eq!(short_layout(""), "");
    }
}
