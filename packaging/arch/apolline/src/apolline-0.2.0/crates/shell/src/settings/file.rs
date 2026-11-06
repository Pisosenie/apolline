use super::*;

impl Settings {
    pub(super) fn stage(&mut self, change: impl FnOnce(&mut Editor)) {
        let Ok(editor) = &mut self.editor else {
            return;
        };
        change(editor);
        let (config, diagnostics) = Config::parse(&editor.text());
        self.config = config;
        self.diagnostics = diagnostics;
    }

    pub(super) fn edit(&mut self, change: impl FnOnce(&mut Editor)) {
        self.stage(change);
        self.save();
    }

    pub(super) fn save(&mut self) {
        let Ok(editor) = &self.editor else {
            return;
        };
        match editor.save(&self.path) {
            Ok(()) => {
                self.stamp = stamp(&self.path);
                self.error = None;
                self.remember(editor.text());
            }
            Err(err) => self.error = Some(format!("could not save: {err}")),
        }
    }

    pub(super) fn remember(&mut self, text: String) {
        if text != self.saved {
            self.undo.push(std::mem::replace(&mut self.saved, text));
            if self.undo.len() > HISTORY {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }

    pub(super) fn step(&mut self, back: bool) {
        let (from, to) = if back {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(text) = from.pop() else {
            return;
        };
        self.revert = None;
        to.push(std::mem::replace(&mut self.saved, text.clone()));
        self.editor = Editor::parse(&text);
        let (config, diagnostics) = Config::parse(&text);
        self.config = config;
        self.diagnostics = diagnostics;
        self.theme_note = None;
        match std::fs::write(&self.path, &text) {
            Ok(()) => self.stamp = stamp(&self.path),
            Err(err) => self.error = Some(format!("could not save: {err}")),
        }
    }

    pub(super) fn reload(&mut self) {
        self.stamp = stamp(&self.path);
        self.editor = Editor::load(&self.path);
        if let Ok(editor) = &self.editor {
            self.remember(editor.text());
        }
        let (config, diagnostics) = Config::load(&self.path);
        if self.editor.is_ok() {
            self.config = config;
        }
        self.diagnostics = diagnostics;
        self.niri_binds = read_niri_binds();
    }
}

pub(super) fn stamp(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}
