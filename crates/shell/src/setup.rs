use std::path::Path;

use apolline_config::{Config, Editor};

use crate::fragment::{self, Fragment};

// What `apolline setup` and the welcome page's Set up button both do: keyboard layouts from
// Hyprland, each screen at its fastest, the niri fragment and its include, the terminal and
// autostart. Each line says what was done.
pub fn run() -> Result<Vec<String>, String> {
    let path = apolline_config::default_path();
    let home = apolline_config::config_home();
    let mut done = take_over(&path, &home);
    let (config, _) = Config::load(&path);
    let look = Fragment {
        config: &config,
        mode: config.start(),
        outputs: &[],
        stay: false,
    };
    done.extend(
        fragment::setup(&fragment::path(), &fragment::niri_config(), &look)
            .map_err(|err| err.to_string())?,
    );
    done.extend(crate::terminal::set_up(&home));
    done.extend(crate::autostart::set_up(
        &home,
        Path::new("/etc/xdg/autostart"),
    ));
    Ok(done)
}

// What the config can take from the machine before the fragment is written from it.
fn take_over(path: &Path, home: &Path) -> Vec<String> {
    let Ok(mut editor) = Editor::load(path) else {
        return Vec::new();
    };
    let (config, _) = Config::load(path);
    let found = crate::hypr::keyboard(home);
    let mut took = crate::hypr::take_keyboard(&config, &found, &mut editor);
    for (name, mode) in crate::arrange::fastest_modes(&crate::niri::outputs(), &config.displays) {
        editor.set_display(&name, "mode", mode.as_str());
        took.push(format!("{name} runs at {mode}, its fastest"));
    }
    if took.is_empty() {
        return took;
    }
    match editor.save(path) {
        Ok(()) => took,
        Err(err) => vec![format!("cannot write {}: {err}", path.display())],
    }
}
