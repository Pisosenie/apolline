use std::path::{Path, PathBuf};
use std::process::Stdio;

use apolline_config::{Config, Diagnostic, Module, Plugin};

pub mod cli;
pub mod install;
pub mod search;
pub mod tiles;

// Plugins are folders with a plugin.toml: yours first, then the ones a package put in.
const SYSTEM: &str = "/usr/share/apolline/plugins";

pub struct Installed {
    pub plugin: Plugin,
    pub dir: PathBuf,
    pub problems: Vec<Diagnostic>,
}

impl Installed {
    pub fn manifest(&self) -> PathBuf {
        self.dir.join("plugin.toml")
    }

    // Packaged plugins are the package manager's to remove.
    pub fn yours(&self) -> bool {
        self.dir.starts_with(dir())
    }
}

// An enabled plugin as the shell runs it.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub plugin: Plugin,
    pub runs: Runs,
}

// How a plugin's commands run: in its folder, with its settings as APOLLINE_ variables.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Runs {
    pub plugin: String,
    pub dir: PathBuf,
    pub env: Vec<(String, String)>,
    // Secret settings by key, read from the keyring each time a command starts, never kept.
    pub secrets: Vec<String>,
}

pub fn dir() -> PathBuf {
    apolline_config::xdg_dir("XDG_DATA_HOME", ".local/share")
        .unwrap_or_default()
        .join("apolline")
        .join("plugins")
}

pub fn installed() -> Vec<Installed> {
    let mut all: Vec<Installed> = Vec::new();
    for base in [dir(), PathBuf::from(SYSTEM)] {
        for found in installed_in(&base) {
            if !all.iter().any(|p| p.plugin.name == found.plugin.name) {
                all.push(found);
            }
        }
    }
    all.sort_by(|a, b| a.plugin.name.cmp(&b.plugin.name));
    all
}

fn installed_in(base: &Path) -> Vec<Installed> {
    let Ok(entries) = std::fs::read_dir(base) else {
        return Vec::new();
    };
    // Dot folders are installs on their way in.
    let mut found: Vec<Installed> = entries
        .flatten()
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|entry| read(&entry.path()))
        .collect();
    found.sort_by(|a, b| a.dir.cmp(&b.dir));
    found
}

pub fn read(dir: &Path) -> Option<Installed> {
    let text = std::fs::read_to_string(dir.join("plugin.toml")).ok()?;
    let (plugin, problems) = Plugin::parse(&text);
    (!plugin.name.is_empty()).then(|| Installed {
        plugin,
        dir: dir.to_path_buf(),
        problems,
    })
}

// The plugins [plugins] enabled names, in that order, as they run.
pub fn load(config: &Config) -> Vec<Loaded> {
    let mut all = installed();
    config
        .plugins
        .enabled
        .iter()
        .filter_map(|name| {
            let at = all.iter().position(|p| &p.plugin.name == name)?;
            let found = all.swap_remove(at);
            let runs = Runs::of(&found, config);
            Some(Loaded {
                plugin: found.plugin,
                runs,
            })
        })
        .collect()
}

// Bar pieces and widgets as [[module]]s called "plugin.name", run the same way.
pub fn modules(loaded: &[Loaded]) -> Vec<Module> {
    loaded
        .iter()
        .flat_map(|loaded| {
            let plugin = &loaded.plugin;
            plugin
                .bar
                .iter()
                .chain(&plugin.widgets)
                .map(|module| Module {
                    name: module.name.as_ref().map(|n| format!("{}.{n}", plugin.name)),
                    plugin: Some(plugin.name.clone()),
                    ..module.clone()
                })
        })
        .collect()
}

pub fn runs<'a>(loaded: &'a [Loaded], plugin: Option<&str>) -> Option<&'a Runs> {
    let plugin = plugin?;
    loaded
        .iter()
        .find(|l| l.plugin.name == plugin)
        .map(|l| &l.runs)
}

impl Runs {
    pub fn of(installed: &Installed, config: &Config) -> Runs {
        let plugin = &installed.plugin;
        Runs {
            plugin: plugin.name.clone(),
            dir: installed.dir.clone(),
            env: plugin.environment(config.plugins.values.get(&plugin.name)),
            secrets: plugin.secrets().map(|s| s.key.clone()).collect(),
        }
    }

    // Sets up a command to run as this plugin's: its folder, its settings, its secrets.
    pub async fn apply(&self, command: &mut tokio::process::Command) {
        command
            .current_dir(&self.dir)
            .envs(self.env.iter().cloned());
        if self.secrets.is_empty() {
            return;
        }
        // An unset secret is still there, empty, like any other setting.
        let found = crate::secrets::plugin_find(&self.plugin, &self.secrets).await;
        for key in &self.secrets {
            let value = found
                .iter()
                .find(|(k, _)| k == key)
                .map_or("", |(_, v)| v.as_str());
            command.env(apolline_config::env_name(key), value);
        }
    }

    // A click's command and the like: started and let go, as the shell's own are.
    pub async fn spawn(self, line: String) {
        let mut command = tokio::process::Command::new("sh");
        command
            .args(["-c", &format!("{line} &")])
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        self.apply(&mut command).await;
        if let Err(err) = command.status().await {
            eprintln!("apolline: plugin {}: {err}", self.plugin);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_plugins_bring_their_pieces_as_modules() {
        let base = std::env::temp_dir().join(format!("apolline-plugins-{}", std::process::id()));
        let folder = base.join("ip");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("plugin.toml"),
            "[plugin]\nname = \"ip\"\n\n[[bar]]\nname = \"address\"\ncommand = \"./ip.sh\"\n\n[[setting]]\nkey = \"service\"\ndefault = \"ifconfig.me\"\n",
        )
        .unwrap();
        let found = installed_in(&base);
        std::fs::remove_dir_all(&base).unwrap();
        assert_eq!(found.len(), 1);
        let (config, _) = Config::parse("[plugins]\nenabled = [\"ip\"]\n");
        let loaded = Loaded {
            runs: Runs::of(&found[0], &config),
            plugin: found[0].plugin.clone(),
        };
        assert_eq!(loaded.runs.dir, folder);
        assert_eq!(
            loaded.runs.env,
            [("APOLLINE_SERVICE".to_owned(), "ifconfig.me".to_owned())]
        );
        let modules = modules(std::slice::from_ref(&loaded));
        assert_eq!(modules[0].name.as_deref(), Some("ip.address"));
        assert_eq!(modules[0].plugin.as_deref(), Some("ip"));
        assert!(runs(&[loaded], Some("ip")).is_some());
    }
}
