use super::*;

use apolline_config::Launcher;

fn source_label(name: &str) -> &'static str {
    match name {
        "apps" => "Apps and games",
        "windows" => "Open windows",
        "modes" => "Modes",
        "actions" => "Actions",
        "calculator" => "Sums and units",
        "files" => "Files, after /",
        "emoji" => "Emoji, after :",
        "run" => "Commands to run",
        _ => "The web",
    }
}

impl Settings {
    pub(super) fn launcher_finds(&self) -> Element<'_, Message> {
        let palette = self.palette();
        let finds = &self.config.launcher;
        let chips = Row::with_children(Launcher::FINDS.map(|name| {
            let on = finds.finds(name);
            button(text(source_label(name)).size(12))
                .padding([6, 12])
                .style(style::choice(on))
                .on_press(Message::LauncherFinds(name, !on))
                .into()
        }))
        .spacing(6)
        .wrap();
        let web = self
            .web_draft
            .as_deref()
            .unwrap_or(finds.web.as_deref().unwrap_or_default());
        let field = text_input("https://duckduckgo.com/?q={query}", web)
            .on_input(Message::WebDraft)
            .on_submit(Message::WebSet)
            .padding([6, 10])
            .size(13)
            .width(280)
            .font(style::mono())
            .style(style::field(palette));
        group(
            "The launcher",
            vec![
                labelled("What it looks through", chips),
                setting(
                    "Web search",
                    "A link with {query} where the words go; empty is DuckDuckGo",
                    field,
                ),
            ],
        )
    }

    pub(super) fn set_finds(&mut self, name: &'static str, on: bool) {
        let finds: Vec<String> = Launcher::FINDS
            .iter()
            .filter(|f| {
                if **f == name {
                    on
                } else {
                    self.config.launcher.finds(f)
                }
            })
            .map(|f| (*f).to_owned())
            .collect();
        if finds.len() == Launcher::FINDS.len() {
            self.edit(|e| e.unset("launcher", "finds"));
        } else {
            self.edit(|e| e.set_list("launcher", "finds", &finds));
        }
    }

    pub(super) fn set_web(&mut self) {
        let Some(draft) = self.web_draft.take() else {
            return;
        };
        let draft = draft.trim().to_owned();
        if draft.is_empty() {
            self.edit(|e| e.unset("launcher", "web"));
        } else if (draft.starts_with("https://") || draft.starts_with("http://"))
            && draft.contains("{query}")
        {
            self.edit(|e| e.set("launcher", "web", draft.as_str()));
        } else {
            self.web_draft = Some(draft);
        }
    }
}

impl Settings {
    pub(super) fn terminal_group(&self) -> Element<'_, Message> {
        let mut choices: Vec<String> = crate::terminal::installed()
            .into_iter()
            .map(str::to_owned)
            .collect();
        if !choices.contains(&self.config.terminal) {
            choices.insert(0, self.config.terminal.clone());
        }
        let missing = !crate::launcher::on_path(
            self.config
                .terminal
                .split_whitespace()
                .next()
                .unwrap_or_default(),
        );
        let mut rows = vec![setting(
            "Terminal",
            "Mod+T opens it, and console apps like htop run in it",
            iced::widget::pick_list(
                choices,
                Some(self.config.terminal.clone()),
                Message::Terminal,
            )
            .text_size(13)
            .padding([7, 12]),
        )];
        if missing {
            rows.push(warning(&crate::i18n::fill(
                "{terminal} isn't installed",
                &[("terminal", &self.config.terminal)],
            )));
        }
        group("Terminal", rows)
    }
}
