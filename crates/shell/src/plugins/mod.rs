use std::path::{Path, PathBuf};
use std::process::Stdio;

use apolline_config::{Config, Diagnostic, Module, Piece, Plugin};

pub mod cli;
pub mod install;
pub mod search;
pub mod tiles;

// Plugins are folders with a plugin.toml: yours first, then the ones a package put in.
const SYSTEM: &str = "/usr/share/apolline/plugins";
// The most of a command's output that's read, so a runaway one can't fill memory.
pub const OUTPUT: u64 = 64 * 1024;

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
    // Secret settings by key, read from the keyring as its own commands start, never kept here.
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

// Bar pieces and widgets as [[module]]s called "plugin.name", run the same way: only those
// placed on a bar or the desktop, since each runs its command over and over.
pub fn modules(loaded: &[Loaded], config: &Config) -> Vec<Module> {
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
        .filter(|module| {
            module
                .name
                .as_deref()
                .is_some_and(|name| placed(config, name))
        })
        .collect()
}

fn placed(config: &Config, name: &str) -> bool {
    config.bar.has(&Piece::Module(name.to_owned()))
        || config.desktop.widgets.iter().any(|widget| widget == name)
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

    // Sets up a command to run as this plugin's: in its folder, with its settings.
    fn apply(&self, command: &mut tokio::process::Command) {
        command
            .current_dir(&self.dir)
            .envs(self.env.iter().cloned());
    }

    // Its secrets too, only for the commands of its own the shell waits on: what a click or a
    // launcher result starts may be an app that runs for hours, and it doesn't get them.
    async fn reveal(&self, command: &mut tokio::process::Command) {
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

    // One of its commands the shell waits on, in a process group of its own: ending it ends
    // whatever it started too, not only sh.
    pub async fn command(&self, line: &str) -> tokio::process::Command {
        let mut command = crate::modules::in_group(line);
        self.apply(&mut command);
        self.reveal(&mut command).await;
        command
    }

    // A click's command and the like: started and let go, as the shell's own are, in a group
    // of its own so nothing meant for the shell's reaches it.
    pub async fn spawn(self, line: String) {
        let mut command = tokio::process::Command::new("sh");
        command
            .args(["-c", &format!("{line} &")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .process_group(0);
        self.apply(&mut command);
        if let Err(err) = command.status().await {
            eprintln!("apolline: plugin {}: {err}", self.plugin);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_plugin_reads_cleanly() {
        let text = include_str!("../../../../examples/plugins/ip/plugin.toml");
        let (plugin, problems) = Plugin::parse(text);
        assert_eq!(problems, []);
        assert_eq!(plugin.pieces(), ["address", "line"]);
        assert_eq!(plugin.tile_names(), ["dummy"]);
        assert!(crate::icon::Icon::by_name(plugin.tiles[0].icon.as_deref().unwrap()).is_some());
    }

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
        assert_eq!(modules(&[], &config), []);
        let loaded = Loaded {
            runs: Runs::of(&found[0], &config),
            plugin: found[0].plugin.clone(),
        };
        assert_eq!(loaded.runs.dir, folder);
        assert_eq!(
            loaded.runs.env,
            [("APOLLINE_SERVICE".to_owned(), "ifconfig.me".to_owned())]
        );
        let loose = modules(std::slice::from_ref(&loaded), &config);
        assert_eq!(loose, [], "not placed, it doesn't run");
        let (placed, _) = Config::parse("[bar.\"DP-1\"]\nright = [\"ip.address\"]\n");
        let modules = modules(std::slice::from_ref(&loaded), &placed);
        assert_eq!(modules[0].name.as_deref(), Some("ip.address"));
        assert_eq!(modules[0].plugin.as_deref(), Some("ip"));
        assert!(runs(&[loaded], Some("ip")).is_some());
    }
}
