use crate::edit::accent_text;
use crate::{Accent, Bar, Config, Edge, Editor, Layout};

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub author: Option<String>,
    pub wallpaper: String,
    pub accent: Accent,
    pub layout: Layout,
    pub bar: Bar,
    pub opacity: Option<f32>,
    pub inactive_opacity: Option<f32>,
    pub animations: Option<f32>,
}

impl Theme {
    pub fn parse(text: &str) -> Result<Self, String> {
        let (config, problems) = Config::parse(text);
        if let Some(problem) = problems.first() {
            return Err(format!("line {}: {problem}", problem.line));
        }
        let (name, author) = config
            .theme
            .clone()
            .ok_or("a theme needs [theme] with a name")?;
        Ok(Self::of(&config, &name, author))
    }

    pub fn of(config: &Config, name: &str, author: Option<String>) -> Self {
        Self {
            name: name.to_owned(),
            author,
            wallpaper: config.desktop.wallpaper.clone(),
            accent: config.desktop.accent,
            layout: config.layout,
            bar: config.bar,
            opacity: config.windows.opacity,
            inactive_opacity: config.windows.inactive_opacity,
            animations: config.windows.animations,
        }
    }

    pub fn text(&self) -> String {
        let mut editor = Editor::parse("").expect("an empty file parses");
        editor.set("theme", "name", self.name.as_str());
        if let Some(author) = &self.author {
            editor.set("theme", "author", author.as_str());
        }
        self.write(&mut editor);
        editor.text()
    }

    pub fn apply(&self, editor: &mut Editor) {
        self.write(editor);
        editor.set("desktop", "theme", self.name.as_str());
    }

    fn write(&self, editor: &mut Editor) {
        editor.set("desktop", "wallpaper", self.wallpaper.as_str());
        editor.set("desktop", "accent", accent_text(self.accent));
        editor.set("layout", "gaps", i64::from(self.layout.gaps));
        editor.set("layout", "radius", i64::from(self.layout.radius));
        editor.set("layout", "borders", self.layout.borders);
        editor.set("layout", "shadows", self.layout.shadows);
        let edge = match self.bar.position {
            Edge::Top => "top",
            Edge::Bottom => "bottom",
        };
        editor.set("bar", "position", edge);
        editor.set("bar", "floating", self.bar.floating);
        editor.set("bar", "opacity", round(self.bar.opacity));
        for (key, value) in [
            ("opacity", self.opacity),
            ("inactive_opacity", self.inactive_opacity),
            ("animations", self.animations),
        ] {
            match value {
                Some(value) => editor.set("windows", key, round(value)),
                None => editor.unset("windows", key),
            }
        }
    }
}

fn round(value: f32) -> f64 {
    (f64::from(value) * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLASS: &str = "[theme]\nname = \"Glass\"\nauthor = \"Apolline\"\n\n[desktop]\nwallpaper = \"night.jpg\"\naccent = \"#c084fc\"\n\n[layout]\ngaps = 16\nradius = 18\n\n[windows]\ninactive_opacity = 0.8\n\n[bar]\nopacity = 0.35\n";

    #[test]
    fn a_theme_is_a_look_and_nothing_else() {
        let theme = Theme::parse(GLASS).unwrap();
        assert_eq!(theme.name, "Glass");
        assert_eq!(theme.layout.radius, 18);
        assert_eq!(theme.inactive_opacity, Some(0.8));

        let mut editor = Editor::parse(
            "[[mode]]\nname = \"Play\"\nkeys = { \"Mod+G\" = \"steam\" }\n\n[windows]\nanimations = 2\n",
        )
        .unwrap();
        theme.apply(&mut editor);
        let (config, problems) = Config::parse(&editor.text());
        assert_eq!(problems, vec![]);
        assert_eq!(config.desktop.theme.as_deref(), Some("Glass"));
        assert_eq!(config.layout.gaps, 16);
        assert_eq!(config.bar.opacity, 0.35);
        assert_eq!(
            config.windows.animations, None,
            "the theme's own speed wins"
        );
        assert_eq!(config.modes[0].keys.len(), 1, "modes are left alone");
        assert_eq!(Theme::parse(&theme.text()).unwrap(), theme);
    }

    #[test]
    fn broken_or_nameless_themes_are_refused() {
        assert!(
            Theme::parse("[layout]\ngaps = 4\n")
                .unwrap_err()
                .contains("needs [theme]")
        );
        assert!(
            Theme::parse("[theme]\nname = \"X\"\n[layout]\ngap = 4\n")
                .unwrap_err()
                .starts_with("line 4")
        );
    }
}
