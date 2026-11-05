use std::process::ExitCode;

use apolline_config::{Config, Editor, Plugin};

use super::{Installed, install, installed};

pub const USAGE: &str =
    "apolline plugin list | install PATH|LINK | remove NAME | enable NAME | disable NAME";

pub fn main(args: &[String]) -> ExitCode {
    let name = args.get(1).map(String::as_str);
    let done = match (args.first().map(String::as_str), name) {
        (None | Some("list"), _) => {
            list();
            Ok(())
        }
        (Some("install"), Some(from)) => install::install(from).map(|found| installed_one(&found)),
        (Some("remove"), Some(name)) => install::remove(name).and_then(|()| {
            println!("{name} is removed");
            switch(name, false)
        }),
        (Some("enable"), Some(name)) => enable(name),
        (Some("disable"), Some(name)) => switch(name, false).map(|()| println!("{name} is off")),
        _ => {
            eprintln!("usage: {USAGE}");
            return ExitCode::from(2);
        }
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("apolline: {err}");
            ExitCode::FAILURE
        }
    }
}

fn list() {
    let path = apolline_config::default_path();
    let (config, _) = Config::load(&path);
    let all = installed();
    if all.is_empty() {
        println!("No plugins yet; `apolline plugin install PATH|LINK` adds one");
    }
    for found in &all {
        let plugin = &found.plugin;
        let mark = if config.plugins.is_enabled(&plugin.name) {
            '*'
        } else {
            ' '
        };
        let version = if plugin.version.is_empty() {
            String::new()
        } else {
            format!(" {}", plugin.version)
        };
        println!("{mark} {}{version} — {}", plugin.name, plugin.about);
        if !plugin.author.is_empty() {
            println!("    by {}", plugin.author);
        }
        if !found.yours() {
            println!("    from a package, in {}", found.dir.display());
        }
        if !found.problems.is_empty() {
            println!(
                "    {} problem(s); `apolline check` says which",
                found.problems.len()
            );
        }
    }
}

// What it will run, said before it's ever enabled.
fn installed_one(found: &Installed) {
    let plugin = &found.plugin;
    println!("{} is installed in {}", plugin.name, found.dir.display());
    println!("Once enabled, it runs these as you, in that folder:");
    for (what, command) in plugin.commands() {
        println!("  {what}: {command}");
    }
    println!("`apolline plugin enable {}` turns it on", plugin.name);
}

fn enable(name: &str) -> Result<(), String> {
    let found = installed()
        .into_iter()
        .find(|p| p.plugin.name == name)
        .ok_or_else(|| format!("no plugin called “{name}” is installed"))?;
    switch(name, true)?;
    println!("{name} is on");
    for line in placing(&found.plugin) {
        println!("  {line}");
    }
    Ok(())
}

// Where its parts go, since nothing is placed for you.
pub fn placing(plugin: &Plugin) -> Vec<String> {
    let name = &plugin.name;
    let mut lines = Vec::new();
    for piece in &plugin.bar {
        let piece = piece.name.as_deref().unwrap_or_default();
        lines.push(format!(
            "On the bar: add “{name}.{piece}” to [bar] left, center or right"
        ));
    }
    for widget in &plugin.widgets {
        let widget = widget.name.as_deref().unwrap_or_default();
        lines.push(format!(
            "On the desktop: add “{name}.{widget}” to [desktop] widgets"
        ));
    }
    for tile in &plugin.tiles {
        lines.push(format!(
            "In quick settings: add “{name}.{}” to [panel] tiles",
            tile.name
        ));
    }
    if !plugin.launchers.is_empty() {
        lines.push("In the launcher: it answers searches now".into());
    }
    lines
}

// Turns it on or off in [plugins] enabled.
pub fn switch(name: &str, on: bool) -> Result<(), String> {
    let path = apolline_config::default_path();
    let (config, _) = Config::load(&path);
    let mut enabled: Vec<String> = config
        .plugins
        .enabled
        .iter()
        .filter(|n| *n != name)
        .cloned()
        .collect();
    if on {
        enabled.push(name.to_owned());
    }
    if enabled == config.plugins.enabled {
        return Ok(());
    }
    let mut editor = Editor::load(&path)?;
    set_enabled(&mut editor, &enabled);
    editor
        .save(&path)
        .map_err(|err| format!("{}: {err}", path.display()))
}

pub fn set_enabled(editor: &mut Editor, enabled: &[String]) {
    if enabled.is_empty() {
        editor.unset("plugins", "enabled");
    } else {
        editor.set_list("plugins", "enabled", enabled);
    }
}

// `apolline check`: the enabled plugins' own files, after the config.
pub fn problems(config: &Config, all: &[Installed]) -> Vec<String> {
    all.iter()
        .filter(|p| config.plugins.is_enabled(&p.plugin.name))
        .flat_map(|p| {
            p.problems
                .iter()
                .map(|problem| format!("{}:{}: {problem}", p.manifest().display(), problem.line))
        })
        .collect()
}
