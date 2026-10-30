use apolline_theme::Mode;

pub struct Status {
    pub workspaces: usize,
    pub active: usize,
    pub busy: Vec<usize>,
    pub keyboard: &'static str,
    pub battery: Option<u8>,
    pub media: Option<&'static str>,
}

impl Status {
    pub fn sample(mode: Mode) -> Self {
        let (active, busy, media) = match mode {
            Mode::Work => (0, vec![0, 1, 2], Some("Harbor Lights — Low Tide")),
            Mode::Play => (2, vec![0, 2], None),
            Mode::Evening => (3, vec![3], Some("Night Drive — Low Tide")),
        };
        Self {
            workspaces: 5,
            active,
            busy,
            keyboard: "EN",
            battery: Some(82),
            media,
        }
    }
}
