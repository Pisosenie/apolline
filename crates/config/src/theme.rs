use crate::edit::accent_text;
use crate::{Accent, Appear, Bar, Blur, Config, Edge, Editor, Layout, Motion, Styles, Surface};

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub author: Option<String>,
    pub wallpaper: String,
    pub accent: Accent,
    pub palette: String,
    pub layout: Layout,
    pub bar: Bar,
    pub opacity: Option<f32>,
    pub inactive_opacity: Option<f32>,
    pub animations: Option<f32>,
    pub motion: Option<Motion>,
    pub appear: Option<Appear>,
    pub blur: Option<Blur>,
    // Each surface's size, corners and glass, and the launcher as a list or a grid.
    pub style: Styles,
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
            palette: config.desktop.palette.clone(),
            layout: config.layout,
            bar: config.bar.clone(),
            opacity: config.windows.opacity,
            inactive_opacity: config.windows.inactive_opacity,
            animations: config.windows.animations,
            motion: config.windows.motion,
            appear: config.windows.appear,
            blur: config.windows.blur,
            style: config.style,
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

    pub fn tweaks(&self, now: &Theme) -> Vec<&'static str> {
        let mut changed = Vec::new();
        let mut differ = |name: &'static str, same: bool| {
            if !same {
                changed.push(name);
            }
        };
        differ("wallpaper", self.wallpaper == now.wallpaper);
        differ("accent", self.accent == now.accent);
        differ("colors", self.palette == now.palette);
        differ("gaps", self.layout.gaps == now.layout.gaps);
        differ("corners", self.layout.radius == now.layout.radius);
        differ(
            "borders",
            self.layout.borders == now.layout.borders
                && self.layout.gradient == now.layout.gradient,
        );
        differ("shadows", self.layout.shadows == now.layout.shadows);
        differ(
            "bar",
            self.bar.position == now.bar.position
                && self.bar.floating == now.bar.floating
                && self.bar.opacity == now.bar.opacity
                && self.bar.blur == now.bar.blur,
        );
        differ(
            "window opacity",
            self.opacity == now.opacity && self.inactive_opacity == now.inactive_opacity,
        );
        differ(
            "motion",
            self.animations == now.animations
                && self.motion == now.motion
                && self.appear == now.appear,
        );
        differ("blur", self.blur == now.blur);
        differ("surfaces", self.style == now.style);
        changed
    }

    pub fn dress(&self, config: &mut Config) {
        config.desktop.wallpaper = self.wallpaper.clone();
        config.desktop.accent = self.accent;
        config.desktop.palette = self.palette.clone();
        config.layout = self.layout;
        config.bar.position = self.bar.position;
        config.bar.floating = self.bar.floating;
        config.bar.opacity = self.bar.opacity;
        config.bar.blur = self.bar.blur;
        config.windows.opacity = self.opacity;
        config.windows.inactive_opacity = self.inactive_opacity;
        config.windows.animations = self.animations;
        config.windows.motion = self.motion;
        config.windows.appear = self.appear;
        config.windows.blur = self.blur;
        config.style = self.style;
    }

    fn write(&self, editor: &mut Editor) {
        editor.set("desktop", "wallpaper", self.wallpaper.as_str());
        editor.set("desktop", "accent", accent_text(self.accent));
        if self.palette == "dark" {
            editor.unset("desktop", "palette");
        } else {
            editor.set("desktop", "palette", self.palette.as_str());
        }
        editor.set("layout", "gaps", i64::from(self.layout.gaps));
        editor.set("layout", "radius", i64::from(self.layout.radius));
        editor.set("layout", "borders", self.layout.borders);
        if self.layout.gradient {
            editor.set("layout", "gradient", true);
        } else {
            editor.unset("layout", "gradient");
        }
        editor.set("layout", "shadows", self.layout.shadows);
        let edge = match self.bar.position {
            Edge::Top => "top",
            Edge::Bottom => "bottom",
        };
        editor.set("bar", "position", edge);
        editor.set("bar", "floating", self.bar.floating);
        editor.set("bar", "opacity", round(self.bar.opacity));
        if self.bar.blur {
            editor.set("bar", "blur", true);
        } else {
            editor.unset("bar", "blur");
        }
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
        let surfaces: [(&str, Surface); 5] = [
            ("bar", self.style.bar),
            ("launcher", self.style.launcher),
            ("panel", self.style.panel),
            ("notifications", self.style.cards),
            ("osd", self.style.osd),
        ];
        for (name, surface) in surfaces {
            let path = ["style", name];
            match surface.radius {
                Some(radius) => editor.set_in(&path, "radius", i64::from(radius)),
                None => editor.unset_in(&path, "radius"),
            }
            for (key, value) in [("opacity", surface.opacity), ("size", surface.size)] {
                match value {
                    Some(value) => editor.set_in(&path, key, round(value)),
                    None => editor.unset_in(&path, key),
                }
            }
        }
        if self.style.grid {
            editor.set_in(&["style", "launcher"], "grid", true);
        } else {
            editor.unset_in(&["style", "launcher"], "grid");
        }
        for (key, value) in [
            ("motion", self.motion.map(Motion::key)),
            ("appear", self.appear.map(Appear::key)),
            ("blur", self.blur.map(Blur::key)),
        ] {
            match value {
                Some(value) => editor.set("windows", key, value),
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
    fn a_theme_carries_the_shape_of_each_surface() {
        let sharp = Theme::parse(
            "[theme]\nname = \"Sharp\"\n\n[style.bar]\nradius = 0\nsize = 0.9\n\n[style.notifications]\nradius = 2\n",
        )
        .unwrap();
        assert_eq!(sharp.style.bar.radius, Some(0));
        assert_eq!(Theme::parse(&sharp.text()).unwrap(), sharp);

        let mut editor =
            Editor::parse("[style.launcher]\nradius = 30\n\n[style.osd]\nopacity = 0.5\n").unwrap();
        sharp.apply(&mut editor);
        let (config, problems) = Config::parse(&editor.text());
        assert_eq!(problems, vec![]);
        assert_eq!(config.style.bar.size, Some(0.9));
        assert_eq!(config.style.cards.radius, Some(2));
        assert_eq!(
            config.style.launcher.radius, None,
            "the theme's look, not what was there"
        );
        assert!(
            !editor.text().contains("[style.osd]"),
            "an emptied table goes"
        );
    }

    #[test]
    fn dressing_matches_applying() {
        let theme = Theme::parse(GLASS).unwrap();
        let file = "[bar]\nseconds = true\n[layout]\ngaps = 2\n";
        let mut editor = Editor::parse(file).unwrap();
        theme.apply(&mut editor);
        let (mut applied, _) = Config::parse(&editor.text());
        let (mut dressed, _) = Config::parse(file);
        theme.dress(&mut dressed);
        applied.desktop.theme = None;
        assert_eq!(dressed, applied);
        assert!(dressed.bar.seconds, "what a theme doesn't carry stays");
    }

    #[test]
    fn tweaks_name_what_differs_from_the_theme() {
        let theme = Theme::parse(GLASS).unwrap();
        let mut editor = Editor::parse("").unwrap();
        theme.apply(&mut editor);
        editor.set("layout", "gaps", 4);
        let (config, _) = Config::parse(&editor.text());
        let now = Theme::of(&config, "now", None);
        assert_eq!(theme.tweaks(&now), ["gaps"]);
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
