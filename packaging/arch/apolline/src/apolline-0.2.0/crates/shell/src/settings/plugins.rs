use super::*;
use iced::widget::column;

use apolline_config::{Kind, Plugin, Setting};

use crate::plugins::install::Refused;
use crate::plugins::{Installed, cli, install};

// The Plugins page's own state: what's installed, and what's being typed or done.
#[derive(Default)]
pub(super) struct Shelf {
    pub(super) installed: Vec<Installed>,
    from: String,
    busy: bool,
    note: Option<Result<String, String>>,
    // Values being typed, by plugin and key, written on Enter; secrets go to the keyring.
    drafts: std::collections::HashMap<(String, String), String>,
    removing: Option<String>,
    // A plugin by that name is installed: the one being added replaces it only when asked.
    taken: Option<String>,
}

impl Shelf {
    pub(super) fn new() -> Self {
        Self {
            installed: crate::plugins::installed(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum Ask {
    Enable(String, bool),
    Draft(String, String, String),
    Set(String, String),
    Switch(String, String, bool),
    Choose(String, String, String),
    Kept(Result<String, String>),
    From(String),
    Install,
    Taken(String),
    Replace,
    Installed(Result<String, String>),
    Replaced(String, String),
    Remove(String),
    OpenFolder,
}

impl Settings {
    pub(super) fn plugins(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let mut page = column![self.add_plugin(palette)].spacing(22);
        for found in &self.shelf.installed {
            page = page.push(self.plugin_card(found, palette));
        }
        page.into()
    }

    fn add_plugin(&self, palette: Palette) -> Element<'_, Message> {
        let from = &self.shelf.from;
        let ready = !from.trim().is_empty() && !self.shelf.busy;
        let mut rows = vec![
            warning(
                "A plugin runs its commands as you, with your files and your network. Enable only what you trust.",
            ),
            setting(
                "Add a plugin",
                "A folder with a plugin.toml, or a git link that starts with https://",
                row![
                    text_input(crate::i18n::t("~/Downloads/plugin"), from)
                        .on_input(|from| Message::Plugin(Ask::From(from)))
                        .on_submit(Message::Plugin(Ask::Install))
                        .padding([8, 12])
                        .size(13)
                        .width(240)
                        .style(style::field(palette)),
                    button(
                        row![
                            icon(Icon::Download, 14.0, neutral::text()),
                            text(if self.shelf.busy { "Adding…" } else { "Add" }).size(13)
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center)
                    )
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press_maybe(ready.then_some(Message::Plugin(Ask::Install))),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ),
            setting(
                "Plugins folder",
                "Each plugin is a folder in it",
                button(
                    row![
                        icon(Icon::Folder, 14.0, neutral::text()),
                        text("Open").size(13)
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([8, 14])
                .style(style::secondary)
                .on_press(Message::Plugin(Ask::OpenFolder)),
            ),
        ];
        if let Some(name) = &self.shelf.taken {
            rows.push(self.replace_plugin(name));
        }
        match &self.shelf.note {
            Some(Ok(line)) => rows.push(padded(
                text(line.as_str())
                    .size(12)
                    .color(style::color(style::GOOD)),
            )),
            Some(Err(line)) => rows.push(warning(line)),
            None => {}
        }
        if self.shelf.installed.is_empty() {
            rows.push(note(
                "No plugins yet. examples/plugins in Apolline's source has one to try.",
            ));
        }
        group("Plugins", rows)
    }

    fn replace_plugin(&self, name: &str) -> Element<'_, Message> {
        let said = crate::i18n::fill(
            "“{name}” is installed already. Putting this one in its place turns it off and forgets its secrets.",
            &[("name", &name)],
        );
        let ready = !self.shelf.busy;
        column![
            warning(&said),
            button(
                row![
                    icon(Icon::Download, 14.0, neutral::text()),
                    text("Replace it").size(13)
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([8, 14])
            .style(style::secondary)
            .on_press_maybe(ready.then_some(Message::Plugin(Ask::Replace))),
        ]
        .spacing(10)
        .padding([14, 16])
        .into()
    }

    fn plugin_card<'a>(&'a self, found: &'a Installed, palette: Palette) -> Element<'a, Message> {
        let plugin = &found.plugin;
        let on = self.config.plugins.is_enabled(&plugin.name);
        let name = plugin.name.clone();
        let about = if plugin.about.is_empty() {
            plugin.name.clone()
        } else {
            plugin.about.clone()
        };
        let made = match (plugin.version.is_empty(), plugin.author.is_empty()) {
            (false, false) => crate::i18n::fill(
                "{version} · by {author}",
                &[("version", &plugin.version), ("author", &plugin.author)],
            ),
            (false, true) => plugin.version.clone(),
            (true, false) => crate::i18n::fill("by {author}", &[("author", &plugin.author)]),
            (true, true) => String::new(),
        };
        let mut rows = vec![
            row![
                column![
                    style::verbatim(about).size(14).font(style::medium()),
                    style::verbatim(made)
                        .size(12)
                        .color(style::color(style::faint())),
                ]
                .spacing(3)
                .width(Fill),
                switch(
                    on,
                    move |on| Message::Plugin(Ask::Enable(name.clone(), on)),
                    palette
                ),
            ]
            .spacing(20)
            .align_y(Alignment::Center)
            .padding([14, 16])
            .into(),
        ];
        if !found.problems.is_empty() {
            rows.push(warning(&crate::i18n::count(
                found.problems.len(),
                "A problem in its plugin.toml; apolline check says what",
                "{n} problems in its plugin.toml; apolline check says what",
            )));
        }
        rows.extend(
            plugin
                .settings
                .iter()
                .map(|s| self.plugin_setting(plugin, s, palette)),
        );
        if on {
            rows.extend(cli::placing(plugin).iter().map(|line| note(line)));
        }
        if found.yours() {
            rows.push(self.remove_plugin(&plugin.name));
        }
        group(&plugin.name, rows)
    }

    fn remove_plugin(&self, name: &str) -> Element<'_, Message> {
        let armed = self.shelf.removing.as_deref() == Some(name);
        let label = if armed {
            "Remove it for good"
        } else {
            "Remove"
        };
        padded(
            button(
                row![
                    icon(Icon::Trash, 14.0, neutral::text()),
                    text(label).size(13)
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([8, 14])
            .style(style::danger(armed))
            .on_press(Message::Plugin(Ask::Remove(name.to_owned()))),
        )
    }

    fn plugin_setting<'a>(
        &'a self,
        plugin: &'a Plugin,
        setting: &'a Setting,
        palette: Palette,
    ) -> Element<'a, Message> {
        let (name, key) = (plugin.name.clone(), setting.key.clone());
        let value = self
            .config
            .plugins
            .values
            .get(&plugin.name)
            .and_then(|values| values.get(&setting.key))
            .unwrap_or(&setting.default);
        let draft = self.shelf.drafts.get(&(name.clone(), key.clone()));
        let about = apolline_config::env_name(&setting.key);
        let control: Element<'a, Message> = match &setting.kind {
            Kind::Switch => switch(
                value.as_str() == "true",
                move |on| Message::Plugin(Ask::Switch(name.clone(), key.clone(), on)),
                palette,
            ),
            Kind::Choice(choices) => {
                iced::widget::pick_list(choices.clone(), Some(value.clone()), move |choice| {
                    Message::Plugin(Ask::Choose(name.clone(), key.clone(), choice))
                })
                .text_size(13)
                .padding([7, 12])
                .into()
            }
            kind => {
                let secret = *kind == Kind::Secret;
                let shown = draft.map_or(if secret { "" } else { value.as_str() }, String::as_str);
                let hint = if secret { "Kept in the keyring" } else { "" };
                let (n, k) = (name.clone(), key.clone());
                text_input(crate::i18n::t(hint), shown)
                    .secure(secret)
                    .on_input(move |typed| Message::Plugin(Ask::Draft(n.clone(), k.clone(), typed)))
                    .on_submit(Message::Plugin(Ask::Set(name, key)))
                    .padding([6, 10])
                    .size(13)
                    .width(220)
                    .style(style::field(palette))
                    .into()
            }
        };
        setting_owned(&setting.label, Some(about), control)
    }

    pub(super) fn ask_plugins(&mut self, ask: Ask) -> Task<Message> {
        match ask {
            Ask::Enable(name, on) => {
                let mut enabled: Vec<String> = self
                    .config
                    .plugins
                    .enabled
                    .iter()
                    .filter(|n| **n != name)
                    .cloned()
                    .collect();
                if on {
                    enabled.push(name);
                }
                self.edit(|e| cli::set_enabled(e, &enabled));
            }
            Ask::Draft(name, key, typed) => {
                self.shelf.drafts.insert((name, key), typed);
            }
            Ask::Set(name, key) => return self.set_plugin_value(name, key),
            Ask::Switch(name, key, on) => {
                self.edit(|e| e.set_in(&["plugins", &name], &key, on));
            }
            Ask::Choose(name, key, choice) => {
                self.edit(|e| e.set_in(&["plugins", &name], &key, choice.as_str()));
            }
            Ask::Kept(said) => self.shelf.note = Some(said),
            Ask::From(from) => {
                self.shelf.from = from;
                self.shelf.taken = None;
            }
            Ask::Install => return self.install_plugin(false),
            Ask::Replace => return self.install_plugin(true),
            Ask::Taken(name) => {
                self.shelf.busy = false;
                self.shelf.taken = Some(name);
            }
            Ask::Installed(said) => self.installed(said),
            Ask::Replaced(name, said) => {
                self.installed(Ok(said));
                return self.retire_plugin(name);
            }
            Ask::Remove(name) => return self.remove_plugin_now(name),
            Ask::OpenFolder => {
                let dir = crate::plugins::dir();
                let _ = std::fs::create_dir_all(&dir);
                let opener = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                return Task::future(async move {
                    let _ = tokio::process::Command::new(opener).arg(dir).status().await;
                })
                .discard();
            }
        }
        Task::none()
    }

    // Text and numbers on Enter; a secret goes to the keyring, and empty forgets it.
    fn set_plugin_value(&mut self, name: String, key: String) -> Task<Message> {
        let Some(typed) = self.shelf.drafts.remove(&(name.clone(), key.clone())) else {
            return Task::none();
        };
        let kind = self
            .shelf
            .installed
            .iter()
            .find(|p| p.plugin.name == name)
            .and_then(|p| p.plugin.settings.iter().find(|s| s.key == key))
            .map(|s| s.kind.clone());
        let path = ["plugins", name.as_str()];
        let typed = typed.trim().to_owned();
        match kind {
            Some(Kind::Secret) => {
                return Task::perform(
                    async move {
                        let said = if typed.is_empty() {
                            crate::secrets::plugin_forget(&name, &key).await
                        } else {
                            crate::secrets::plugin_store(&name, &key, &typed).await
                        };
                        said.map(|()| crate::i18n::t("Kept in the keyring").to_owned())
                    },
                    |said| Message::Plugin(Ask::Kept(said)),
                );
            }
            _ if typed.is_empty() => self.edit(|e| e.unset_in(&path, &key)),
            Some(Kind::Number) => match (typed.parse::<i64>(), typed.parse::<f64>()) {
                (Ok(whole), _) => self.edit(|e| e.set_in(&path, &key, whole)),
                (_, Ok(number)) => self.edit(|e| e.set_in(&path, &key, number)),
                _ => {
                    self.shelf.drafts.insert((name, key), typed);
                }
            },
            _ => self.edit(|e| e.set_in(&path, &key, typed.as_str())),
        }
        Task::none()
    }

    fn install_plugin(&mut self, replace: bool) -> Task<Message> {
        let from = self.shelf.from.trim().to_owned();
        if from.is_empty() || self.shelf.busy {
            return Task::none();
        }
        self.shelf.busy = true;
        self.shelf.note = None;
        self.shelf.taken = None;
        // A clone can take a while, so it's off the window's thread.
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || install::install(&from, replace))
                    .await
                    .map_err(|err| Refused::Wrong(err.to_string()))
                    .and_then(|done| done)
            },
            move |done| {
                Message::Plugin(match done {
                    Ok(found) if replace => {
                        Ask::Replaced(found.plugin.name.clone(), installed_note(&found.plugin))
                    }
                    Ok(found) => Ask::Installed(Ok(installed_note(&found.plugin))),
                    Err(Refused::Taken(name)) => Ask::Taken(name),
                    Err(Refused::Wrong(why)) => Ask::Installed(Err(why)),
                })
            },
        )
    }

    fn installed(&mut self, said: Result<String, String>) {
        self.shelf.busy = false;
        if said.is_ok() {
            self.shelf.from.clear();
        }
        self.shelf.note = Some(said);
        self.shelf.installed = crate::plugins::installed();
    }

    // The first press arms it; the second removes the folder and turns it off.
    fn remove_plugin_now(&mut self, name: String) -> Task<Message> {
        if self.shelf.removing.as_deref() != Some(&name) {
            self.shelf.removing = Some(name);
            return Task::none();
        }
        self.shelf.removing = None;
        let removed = install::remove(&name);
        let retired = match removed {
            Ok(()) => self.retire_plugin(name.clone()),
            Err(_) => Task::none(),
        };
        self.shelf.note =
            Some(removed.map(|()| crate::i18n::fill("{name} is removed", &[("name", &name)])));
        self.shelf.installed = crate::plugins::installed();
        retired
    }

    // Off, and its secrets forgotten, so nothing put in its place under the name runs at once or
    // inherits them.
    fn retire_plugin(&mut self, name: String) -> Task<Message> {
        let enabled: Vec<String> = self
            .config
            .plugins
            .enabled
            .iter()
            .filter(|n| **n != name)
            .cloned()
            .collect();
        if enabled != self.config.plugins.enabled {
            self.edit(|e| cli::set_enabled(e, &enabled));
        }
        Task::future(async move {
            if let Err(err) = crate::secrets::plugin_forget_all(&name).await {
                eprintln!("apolline: plugin {name}: {err}");
            }
        })
        .discard()
    }

    // Names of what enabled plugins bring, as "plugin.part", for the boards elsewhere.
    pub(super) fn plugin_parts(&self, parts: fn(&Plugin) -> Vec<String>) -> Vec<String> {
        self.shelf
            .installed
            .iter()
            .filter(|p| self.config.plugins.is_enabled(&p.plugin.name))
            .flat_map(|p| {
                parts(&p.plugin)
                    .into_iter()
                    .map(|part| format!("{}.{part}", p.plugin.name))
            })
            .collect()
    }
}

fn installed_note(plugin: &Plugin) -> String {
    let commands: Vec<&str> = plugin.commands().into_iter().map(|(_, c)| c).collect();
    crate::i18n::fill(
        "{name} is installed. Once enabled, it runs: {commands}",
        &[("name", &plugin.name), ("commands", &commands.join(" · "))],
    )
}

pub(super) fn bar_pieces(plugin: &Plugin) -> Vec<String> {
    plugin.bar.iter().filter_map(|m| m.name.clone()).collect()
}

pub(super) fn widgets(plugin: &Plugin) -> Vec<String> {
    plugin
        .widgets
        .iter()
        .filter_map(|m| m.name.clone())
        .collect()
}

pub(super) fn tiles(plugin: &Plugin) -> Vec<String> {
    plugin.tiles.iter().map(|t| t.name.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugin_is_switched_on_and_its_settings_written() {
        let dir = std::env::temp_dir().join(format!("apolline-plugin-page-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "").unwrap();
        let (mut settings, _) = Settings::open(path.clone(), None);
        let (plugin, _) = Plugin::parse(
            "[plugin]\nname = \"ip\"\n\n[[setting]]\nkey = \"every\"\nkind = \"number\"\n\n[[setting]]\nkey = \"six\"\nkind = \"switch\"\n",
        );
        settings.shelf.installed = vec![Installed {
            plugin,
            dir: dir.clone(),
            problems: Vec::new(),
        }];
        let ask = |ask| Message::Plugin(ask);
        let _ = settings.update(ask(Ask::Enable("ip".into(), true)));
        let _ = settings.update(ask(Ask::Draft("ip".into(), "every".into(), "1.5".into())));
        let _ = settings.update(ask(Ask::Set("ip".into(), "every".into())));
        let _ = settings.update(ask(Ask::Draft("ip".into(), "every".into(), "soon".into())));
        let _ = settings.update(ask(Ask::Set("ip".into(), "every".into())));
        let _ = settings.update(ask(Ask::Switch("ip".into(), "six".into(), true)));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("enabled = [\"ip\"]"), "{text}");
        assert!(
            text.contains("[plugins.ip]\nevery = 1.5\nsix = true"),
            "{text}"
        );
        assert_eq!(settings.plugin_parts(tiles), Vec::<String>::new());

        let _ = settings.update(ask(Ask::Enable("ip".into(), false)));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("enabled"), "{text}");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
