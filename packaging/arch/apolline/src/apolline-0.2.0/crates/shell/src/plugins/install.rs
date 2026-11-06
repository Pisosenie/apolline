use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use apolline_config::{Plugin, valid_name};

use super::{Installed, dir};

// Why a plugin wasn't installed.
#[derive(Debug, PartialEq)]
pub enum Refused {
    // One by that name is installed; only an install that says so takes its place.
    Taken(String),
    Wrong(String),
}

impl From<String> for Refused {
    fn from(why: String) -> Self {
        Refused::Wrong(why)
    }
}

// A folder with a plugin.toml, or a git link, copied into your plugins. Nothing of it runs until
// it's enabled.
pub fn install(from: &str, replace: bool) -> Result<Installed, Refused> {
    let plugins = dir();
    std::fs::create_dir_all(&plugins).map_err(|e| format!("{}: {e}", plugins.display()))?;
    if from.starts_with("https://") {
        return clone(from, &plugins, replace);
    }
    if from.contains("://") || from.starts_with("git@") {
        return Err(Refused::Wrong(
            "a plugin's link starts with https://".into(),
        ));
    }
    let from = home(from);
    // Copied out of the plugins folder, it would be deleted on its way in; holding it, it would
    // be copied into itself.
    if within(&from, &plugins) || within(&plugins, &from) {
        return Err(Refused::Wrong(format!(
            "{} is in the plugins folder or holds it; install from a folder of its own",
            from.display()
        )));
    }
    copy_in(&from, &plugins, replace)
}

// Whether `inner` is `outer` or in it, as the files really are.
fn within(inner: &Path, outer: &Path) -> bool {
    match (inner.canonicalize(), outer.canonicalize()) {
        (Ok(inner), Ok(outer)) => inner.starts_with(outer),
        _ => false,
    }
}

// "~/x" as a path under the home folder, as typed into Settings.
pub fn home(path: &str) -> PathBuf {
    match path.trim().strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(rest),
        None => PathBuf::from(path.trim()),
    }
}

// What's wrong with a plugin before anything of it is copied.
pub fn check(folder: &Path) -> Result<Plugin, String> {
    let manifest = folder.join("plugin.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|_| format!("{} has no plugin.toml", folder.display()))?;
    let (plugin, problems) = Plugin::parse(&text);
    if let Some(problem) = problems.first() {
        return Err(format!(
            "{}:{}: {problem}",
            manifest.display(),
            problem.line
        ));
    }
    // Its name becomes a folder, so nothing in it may lead somewhere else.
    if !valid_name(&plugin.name) {
        return Err(format!("“{}” can't be a plugin's name", plugin.name));
    }
    Ok(plugin)
}

fn copy_in(from: &Path, plugins: &Path, replace: bool) -> Result<Installed, Refused> {
    let plugin = check(from)?;
    let target = plugins.join(&plugin.name);
    if target.exists() && !replace {
        return Err(Refused::Taken(plugin.name));
    }
    // Copied whole into a dot folder the shell passes over, then put in place in one step, so a
    // copy that fails leaves what was there as it was.
    let incoming = plugins.join(format!(".incoming-{}-{}", plugin.name, std::process::id()));
    let _ = std::fs::remove_dir_all(&incoming);
    let placed = copy_folder(from, &incoming).and_then(|()| put(&incoming, &target));
    let _ = std::fs::remove_dir_all(&incoming);
    placed.map_err(|e| format!("{}: {e}", target.display()))?;
    super::read(&target).ok_or_else(|| Refused::Wrong("the copied plugin doesn't read".into()))
}

// The new folder where the old one was; the old is moved aside first, and back if that fails.
fn put(incoming: &Path, target: &Path) -> std::io::Result<()> {
    if !target.exists() {
        return std::fs::rename(incoming, target);
    }
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    let outgoing = target.with_file_name(format!(".outgoing-{name}-{}", std::process::id()));
    std::fs::rename(target, &outgoing)?;
    if let Err(err) = std::fs::rename(incoming, target) {
        let _ = std::fs::rename(&outgoing, target);
        return Err(err);
    }
    let _ = std::fs::remove_dir_all(&outgoing);
    Ok(())
}

// Files and folders, as they are; links and git's own folder stay behind.
fn copy_folder(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
        let kind = entry.file_type()?;
        let path = entry.path();
        if kind.is_dir() && entry.file_name() != ".git" {
            copy_folder(&path, &to.join(entry.file_name()))?;
        } else if kind.is_file() {
            std::fs::copy(&path, to.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn clone(link: &str, plugins: &Path, replace: bool) -> Result<Installed, Refused> {
    let cloning = plugins.join(format!(".cloning-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cloning);
    // A link that wants a password fails at once rather than waiting on a prompt nobody sees.
    let cloned = Command::new("git")
        .args(["clone", "--depth", "1", "--quiet", "--", link])
        .arg(&cloning)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    let installed = match cloned {
        Ok(out) if out.status.success() => copy_in(&cloning, plugins, replace),
        Ok(out) => Err(Refused::Wrong(format!(
            "git couldn't clone it: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))),
        Err(err) => Err(Refused::Wrong(format!("git: {err}"))),
    };
    let _ = std::fs::remove_dir_all(&cloning);
    installed
}

// Only plugins in your own folder; packaged ones go with their package.
pub fn remove(name: &str) -> Result<(), String> {
    let found = super::installed()
        .into_iter()
        .find(|p| p.plugin.name == name)
        .ok_or_else(|| format!("no plugin called “{name}” is installed"))?;
    if !found.yours() {
        return Err(format!(
            "“{name}” came with a package in {}; remove it with the package manager",
            found.dir.display()
        ));
    }
    std::fs::remove_dir_all(&found.dir).map_err(|e| format!("{}: {e}", found.dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugin_is_checked_then_copied_whole() {
        let base = std::env::temp_dir().join(format!("apolline-install-{}", std::process::id()));
        let (from, plugins) = (base.join("from"), base.join("plugins"));
        std::fs::create_dir_all(from.join("bin")).unwrap();
        std::fs::create_dir_all(from.join(".git")).unwrap();
        std::fs::write(from.join("bin/run.sh"), "echo hi").unwrap();
        std::fs::write(from.join(".git/HEAD"), "x").unwrap();
        std::fs::write(from.join("plugin.toml"), "[plugin]\nname = \"../up\"\n").unwrap();
        let refused = copy_in(&from, &plugins, false).err();

        std::fs::write(
            from.join("plugin.toml"),
            "[plugin]\nname = \"hello\"\n\n[[bar]]\nname = \"hi\"\ncommand = \"bin/run.sh\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(&plugins).unwrap();
        let installed = copy_in(&from, &plugins, false);
        let copied = plugins.join("hello/bin/run.sh").exists();
        let git = plugins.join("hello/.git").exists();
        let left: Vec<_> = std::fs::read_dir(&plugins).unwrap().flatten().collect();
        std::fs::remove_dir_all(&base).unwrap();

        assert!(matches!(refused, Some(Refused::Wrong(e)) if e.contains("lowercase letters")));
        assert_eq!(installed.map(|p| p.plugin.name), Ok("hello".to_owned()));
        assert!(copied && !git);
        assert_eq!(left.len(), 1, "nothing but the plugin is left behind");
    }

    #[test]
    fn a_plugin_by_a_taken_name_replaces_it_only_when_asked() {
        let base = std::env::temp_dir().join(format!("apolline-replace-{}", std::process::id()));
        let (from, plugins) = (base.join("from"), base.join("plugins"));
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(plugins.join("hello")).unwrap();
        std::fs::write(plugins.join("hello/old.sh"), "old").unwrap();
        std::fs::write(from.join("plugin.toml"), "[plugin]\nname = \"hello\"\n").unwrap();
        std::fs::write(from.join("new.sh"), "new").unwrap();

        let refused = copy_in(&from, &plugins, false).err();
        let kept = plugins.join("hello/old.sh").exists();
        let replaced = copy_in(&from, &plugins, true).map(|p| p.plugin.name);
        let swapped =
            !plugins.join("hello/old.sh").exists() && plugins.join("hello/new.sh").exists();
        let inside = within(&plugins.join("hello"), &plugins) && !within(&from, &plugins);
        let left = std::fs::read_dir(&plugins).unwrap().count();
        std::fs::remove_dir_all(&base).unwrap();

        assert_eq!(refused, Some(Refused::Taken("hello".into())));
        assert!(kept, "the one installed stays as it was");
        assert_eq!(replaced, Ok("hello".into()));
        assert!(swapped && inside);
        assert_eq!(left, 1);
    }
}
