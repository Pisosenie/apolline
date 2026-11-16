// What became of each line of a Hyprland config, and the report that says it.

#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub at: String,
    pub line: String,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Moved(String),
    // niri, or whatever runs on it, does this already.
    Here(String),
    Left(String),
}

impl Outcome {
    // Which part of the report it goes in, and what it says.
    pub fn group(&self) -> (usize, &str) {
        match self {
            Self::Moved(said) => (0, said),
            Self::Here(why) => (1, why),
            Self::Left(why) => (2, why),
        }
    }
}

// Three parts under `titles`: what came over, what was there already, what stays behind, each
// line with where it was written. `also` adds lines with no place of their own to the first.
pub fn report(notes: &[Note], also: &[String], titles: [&str; 3]) -> String {
    let mut out = String::new();
    for (group, title) in titles.into_iter().enumerate() {
        let notes: Vec<(&Note, &str)> = notes
            .iter()
            .filter_map(|note| {
                let (at, said) = note.outcome.group();
                (at == group).then_some((note, said))
            })
            .collect();
        let also: &[String] = if group == 0 { also } else { &[] };
        let count = notes.len() + also.len();
        if count == 0 {
            continue;
        }
        out.push_str(&format!("{title} ({count})\n"));
        for (note, said) in notes {
            if group == 0 {
                out.push_str(&format!("  {:<22} {said}\n", note.at));
            } else {
                out.push_str(&format!(
                    "  {:<22} {}\n  {:<22} {said}\n",
                    note.at,
                    short(&note.line),
                    ""
                ));
            }
        }
        for said in also {
            out.push_str(&format!("  {:<22} {said}\n", ""));
        }
    }
    out
}

fn short(line: &str) -> String {
    match line.char_indices().nth(70) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_owned(),
    }
}
