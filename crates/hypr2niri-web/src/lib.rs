// hypr2niri in a web page. The page writes the config into memory it got from `alloc`, calls a
// convert, and reads back one text: the niri config, the era, then a line per note, all apart
// by the record separator (0x1e); a note's fields apart by the unit separator (0x1f).
//
// A hyprland.lua is run in the page by a Lua there, through the same sandbox; what it notes comes
// here as lines of key, value, at and block, apart by the unit separator, one per line.

use hypr2niri::Line;
use hypr2niri::niri::{self, Screens};

const RECORD: char = '\u{1e}';
const UNIT: char = '\u{1f}';

#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

/// # Safety
/// `ptr` and `len` are what `alloc` handed out, or a result's own.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn release(ptr: *mut u8, len: usize) {
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len) });
}

/// A hyprland.conf's text. Returns the result's address and length packed as hi << 32 | lo.
///
/// # Safety
/// `ptr` and `len` point at `len` bytes from `alloc`, which this takes and frees.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn convert_conf(ptr: *mut u8, len: usize) -> u64 {
    let text = unsafe { take(ptr, len) };
    give(conf(&text))
}

/// What the Lua sandbox noted, as described above.
///
/// # Safety
/// As for `convert_conf`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn convert_lua(ptr: *mut u8, len: usize) -> u64 {
    let text = unsafe { take(ptr, len) };
    give(lua(&text))
}

pub fn conf(text: &str) -> String {
    let lines = hypr2niri::conf::read_text(text, "hyprland.conf");
    answer(&lines, false, "hyprland.conf")
}

pub fn lua(noted: &str) -> String {
    let lines: Vec<Line> = noted
        .lines()
        .filter_map(|row| {
            let mut fields = row.split(UNIT);
            Some(Line {
                key: fields.next()?.to_owned(),
                value: fields.next()?.to_owned(),
                at: fields.next()?.to_owned(),
                block: fields.next().and_then(|b| b.parse().ok()).unwrap_or(0),
            })
        })
        .collect();
    answer(&lines, true, "hyprland.lua")
}

fn answer(lines: &[Line], lua: bool, name: &str) -> String {
    if lines.is_empty() {
        return format!("{RECORD}nothing to read{RECORD}");
    }
    let converted = niri::convert(lines, lua, name, &Screens::default());
    let mut out = format!("{}{RECORD}{}", converted.kdl, converted.era.said());
    for note in &converted.notes {
        let (group, said) = note.outcome.group();
        out.push(RECORD);
        out.push_str(&format!(
            "{group}{UNIT}{}{UNIT}{}{UNIT}{said}",
            note.at, note.line
        ));
    }
    out
}

unsafe fn take(ptr: *mut u8, len: usize) -> String {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len) };
    String::from_utf8_lossy(&bytes).into_owned()
}

fn give(text: String) -> u64 {
    let mut bytes = text.into_bytes();
    bytes.shrink_to_fit();
    let (ptr, len) = (bytes.as_mut_ptr(), bytes.len());
    std::mem::forget(bytes);
    ((ptr as u64) << 32) | len as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pasted_config_comes_back_as_config_era_and_notes() {
        let out = conf("bind = SUPER, Q, exec, kitty\nwindowrule = match:class kitty, float on\n");
        let parts: Vec<&str> = out.split(RECORD).collect();
        assert!(
            parts[0].contains("Mod+Q { spawn-sh \"kitty\"; }"),
            "{}",
            parts[0]
        );
        assert_eq!(parts[1], "Hyprland 0.53 or newer");
        assert!(parts[2].starts_with(&format!("0{UNIT}hyprland.conf:1{UNIT}")));
        let noted = format!("bind{UNIT}SUPER, C, killactive, {UNIT}hyprland.lua:3{UNIT}0\n");
        assert!(lua(&noted).contains("Mod+C { close-window; }"));
    }
}
