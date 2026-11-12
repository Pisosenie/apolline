use std::fs;
use std::path::{Path, PathBuf};

// One setting of a Hyprland config, read the way Hyprland reads it: sourced files in place,
// $variables filled in, and blocks flattened, so `input { touchpad { tap-to-click = true } }`
// is input:touchpad:tap-to-click.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub key: String,
    pub value: String,
    // Where it was written, as file:line.
    pub at: String,
}

const DEPTH: u8 = 4;

pub fn file(config_home: &Path) -> PathBuf {
    config_home.join("hypr/hyprland.conf")
}

pub fn read(path: &Path) -> Vec<Line> {
    let mut reader = Reader::default();
    reader.file(path, 0);
    reader.lines
}

#[derive(Default)]
struct Reader {
    lines: Vec<Line>,
    variables: Vec<(String, String)>,
}

impl Reader {
    fn file(&mut self, path: &Path, depth: u8) {
        let Ok(text) = fs::read_to_string(path) else {
            return;
        };
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let mut scope: Vec<String> = Vec::new();
        for (number, raw) in text.lines().enumerate() {
            let line = uncomment(raw);
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line == "}" {
                scope.pop();
                continue;
            }
            if let Some(block) = line.strip_suffix('{') {
                scope.push(block.trim().to_owned());
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), self.fill(value.trim()));
            if let Some(variable) = key.strip_prefix('$') {
                self.variables.retain(|(name, _)| name != variable);
                self.variables.push((variable.to_owned(), value));
                // Longest first, so $mod doesn't eat the start of $modShift.
                self.variables
                    .sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
                continue;
            }
            if key == "source" && scope.is_empty() {
                if depth < DEPTH {
                    for sourced in sourced(&value, path) {
                        self.file(&sourced, depth + 1);
                    }
                }
                continue;
            }
            let key = scope
                .iter()
                .map(String::as_str)
                .chain(std::iter::once(key))
                .collect::<Vec<_>>()
                .join(":");
            self.lines.push(Line {
                key,
                value,
                at: format!("{name}:{}", number + 1),
            });
        }
    }

    fn fill(&self, value: &str) -> String {
        self.variables
            .iter()
            .fold(value.to_owned(), |value, (name, with)| {
                value.replace(&format!("${name}"), with)
            })
    }
}

// "##" is a "#" in Hyprland; a lone one starts a comment.
fn uncomment(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '#' {
            if chars.peek() == Some(&'#') {
                chars.next();
                out.push('#');
                continue;
            }
            break;
        }
        out.push(c);
    }
    out
}

pub fn sourced(value: &str, from: &Path) -> Vec<PathBuf> {
    let path = match value.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map_or_else(|| PathBuf::from(value), |h| PathBuf::from(h).join(rest)),
        None => PathBuf::from(value),
    };
    let path = if path.is_relative() {
        from.parent().unwrap_or(Path::new(".")).join(path)
    } else {
        path
    };
    if !path.to_string_lossy().contains('*') {
        return vec![path];
    }
    let (Some(dir), Some(pattern)) = (path.parent(), path.file_name()) else {
        return Vec::new();
    };
    let pattern = pattern.to_string_lossy();
    let (start, end) = pattern.split_once('*').unwrap_or((&pattern, ""));
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .is_some_and(|n| n.starts_with(start) && n.ends_with(end))
        })
        .collect();
    files.sort();
    files
}

// The last value written for a key, as Hyprland takes it.
pub fn last<'a>(lines: &'a [Line], key: &str) -> Option<&'a str> {
    lines
        .iter()
        .rev()
        .find(|line| line.key == key)
        .map(|line| line.value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_variables_and_blocks_are_read_as_hyprland_does() {
        let dir = std::env::temp_dir().join(format!("apolline-hyprconf-{}", std::process::id()));
        fs::create_dir_all(dir.join("conf.d")).unwrap();
        fs::write(
            dir.join("hyprland.conf"),
            "$mod = SUPER\n$modShift = SUPER SHIFT\nsource = ./conf.d/*.conf\nbind = $mod, Q, exec, kitty # a terminal\nbind = $modShift, C, killactive\ngeneral {\n    col.active_border = rgba(33ccffee)\n    snap {\n        enabled = true\n    }\n}\nexec-once = echo ##1\n",
        )
        .unwrap();
        fs::write(
            dir.join("conf.d/input.conf"),
            "input {\n    kb_layout = us,ru\n}\n$term = foot\nbind = $mod, Return, exec, $term\n",
        )
        .unwrap();
        let lines = read(&dir.join("hyprland.conf"));
        let pairs: Vec<(&str, &str)> = lines
            .iter()
            .map(|l| (l.key.as_str(), l.value.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("input:kb_layout", "us,ru"),
                ("bind", "SUPER, Return, exec, foot"),
                ("bind", "SUPER, Q, exec, kitty"),
                ("bind", "SUPER SHIFT, C, killactive"),
                ("general:col.active_border", "rgba(33ccffee)"),
                ("general:snap:enabled", "true"),
                ("exec-once", "echo #1"),
            ]
        );
        assert_eq!(lines[0].at, "input.conf:2");
        assert_eq!(lines[2].at, "hyprland.conf:4");
        assert_eq!(last(&lines, "input:kb_layout"), Some("us,ru"));
        fs::remove_dir_all(dir).unwrap();
    }
}
