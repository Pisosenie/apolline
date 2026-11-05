use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use apolline_config::{Plugin, valid_name};

use super::{Installed, dir};

// A folder with a plugin.toml, or a git link, copied into your plugins. Nothing of it runs until
// it's enabled.
pub fn install(from: &str) -> Result<Installed, String> {
    if from.starts_with("https://") {
        return clone(from);
    }
    if from.contains("://") || from.starts_with("git@") {
        return Err("a plugin's link starts with https://".into());
    }
    copy_in(&home(from), &dir())
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

fn copy_in(from: &Path, plugins: &Path) -> Result<Installed, String> {
    let plugin = check(from)?;
    let target = plugins.join(&plugin.name);
    if target.exists() {
        std::fs::remove_dir_all(&target).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    copy_folder(from, &target).map_err(|e| format!("{}: {e}", target.display()))?;
    super::read(&target).ok_or_else(|| "the copied plugin doesn't read".into())
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

fn clone(link: &str) -> Result<Installed, String> {
    let plugins = dir();
    std::fs::create_dir_all(&plugins).map_err(|e| e.to_string())?;
    let incoming = plugins.join(format!(".incoming-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&incoming);
    // A link that wants a password fails at once rather than waiting on a prompt nobody sees.
    let cloned = Command::new("git")
        .args(["clone", "--depth", "1", "--quiet", "--", link])
        .arg(&incoming)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    let installed = match cloned {
        Ok(out) if out.status.success() => copy_in(&incoming, &plugins),
        Ok(out) => Err(format!(
            "git couldn't clone it: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(err) => Err(format!("git: {err}")),
    };
    let _ = std::fs::remove_dir_all(&incoming);
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
        let refused = copy_in(&from, &plugins).err();

        std::fs::write(
            from.join("plugin.toml"),
            "[plugin]\nname = \"hello\"\n\n[[bar]]\nname = \"hi\"\ncommand = \"bin/run.sh\"\n",
        )
        .unwrap();
        let installed = copy_in(&from, &plugins);
        let copied = plugins.join("hello/bin/run.sh").exists();
        let git = plugins.join("hello/.git").exists();
        std::fs::remove_dir_all(&base).unwrap();

        assert!(refused.is_some_and(|e| e.contains("lowercase letters")));
        assert_eq!(installed.map(|p| p.plugin.name), Ok("hello".to_owned()));
        assert!(copied && !git);
    }
}
