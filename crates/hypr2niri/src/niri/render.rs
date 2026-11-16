// The niri config itself: sections in the order niri's own config has them, and at the end what
// stayed behind, so the file says it.

use super::Out;
use crate::era::Era;
use crate::kdl::Node;

impl Out {
    pub(super) fn render(&self, source: &str, era: &Era) -> String {
        let mut nodes = Vec::new();
        nodes.extend(self.input());
        nodes.extend(self.outputs.iter().cloned());
        nodes.push(self.layout());
        nodes.extend(
            self.workspaces
                .iter()
                .map(|name| Node::leaf(format!("workspace {}", crate::kdl::quote(name)))),
        );
        nodes.extend(self.global_rules());
        if !self.cursor.is_empty() {
            nodes.push(Node::block("cursor", self.cursor.clone()));
        }
        if !self.environment.is_empty() {
            nodes.push(Node::block("environment", self.environment.clone()));
        }
        nodes.extend(self.startup.iter().cloned());
        if self.animations_off {
            nodes.push(Node::block("animations", vec![Node::leaf("off")]));
        }
        nodes.extend(self.rules.iter().cloned());
        nodes.extend(self.layer_rules.iter().cloned());
        nodes.push(Node::block(
            "binds",
            self.binds.iter().map(|(_, node)| node.clone()).collect(),
        ));

        let mut out = format!(
            "// Made by hypr2niri from {source}, written for {}.\n\
             // Needs niri 25.08 or newer (spawn-sh); blur needs 26.04. Check it with:\n\
             //     niri validate -c config.kdl\n",
            era.said()
        );
        if era.mixed() {
            out.push_str(
                "// The config mixes syntax of different Hyprland versions; each line was read as its own version writes it.\n",
            );
        }
        for node in nodes {
            out.push('\n');
            node.render(0, &mut out);
        }
        // Grouped by why, a few lines each, so thirty animation lines say it once.
        let mut behind: Vec<(&str, Vec<&crate::report::Note>)> = Vec::new();
        for note in self.notes.iter().filter(|n| n.outcome.group().0 == 2) {
            let why = note.outcome.group().1;
            match behind.iter_mut().find(|(w, _)| *w == why) {
                Some((_, notes)) => notes.push(note),
                None => behind.push((why, vec![note])),
            }
        }
        if !behind.is_empty() {
            out.push_str("\n// Stayed behind:\n");
            for (why, notes) in behind {
                out.push_str(&format!("//   {why}:\n"));
                for note in notes.iter().take(3) {
                    out.push_str(&format!("//       {}  {}\n", note.at, note.line));
                }
                if notes.len() > 3 {
                    out.push_str(&format!("//       and {} more like it\n", notes.len() - 3));
                }
            }
        }
        out
    }

    fn input(&self) -> Option<Node> {
        let mut keyboard = Vec::new();
        if !self.xkb.is_empty() {
            keyboard.push(Node::block("xkb", self.xkb.clone()));
        }
        keyboard.extend(self.keyboard.iter().cloned());
        let mut children = Vec::new();
        for (name, part) in [
            ("keyboard", keyboard),
            ("touchpad", self.touchpad.clone()),
            ("mouse", self.mouse.clone()),
        ] {
            if !part.is_empty() {
                children.push(Node::block(name, part));
            }
        }
        children.extend(self.input.iter().cloned());
        (!children.is_empty()).then(|| Node::block("input", children))
    }

    // Hyprland's gaps_in is half the gap between two windows; niri's gaps are the whole gap,
    // the same at the edges.
    fn layout(&self) -> Node {
        let mut children = Vec::new();
        if let Some(gaps) = self.gaps_in.map(|g| g * 2).or(self.gaps_out) {
            children.push(Node::leaf(format!("gaps {gaps}")));
        }
        match self.border_size {
            Some(0) => {
                children.push(Node::block("focus-ring", vec![Node::leaf("off")]));
            }
            size => {
                let mut border = vec![Node::leaf("on")];
                border.push(Node::leaf(format!("width {}", size.unwrap_or(1))));
                border.extend(self.border.iter().cloned());
                children.push(Node::block("focus-ring", vec![Node::leaf("off")]));
                children.push(Node::block("border", border));
            }
        }
        if !self.shadow.is_empty() {
            children.push(Node::block("shadow", self.shadow.clone()));
        }
        Node::block("layout", children)
    }

    // Corners, opacity and blur are for every window in Hyprland: in niri, rules for all.
    fn global_rules(&self) -> Vec<Node> {
        let mut all = Vec::new();
        if let Some(radius) = self.rounding.filter(|r| *r > 0) {
            all.push(Node::leaf(format!("geometry-corner-radius {radius}")));
            all.push(Node::leaf("clip-to-geometry true"));
        }
        if let Some(opacity) = self.opacity.0.filter(|o| *o < 1.0) {
            all.push(Node::leaf(format!("opacity {}", super::number(opacity))));
        }
        if self.blur {
            all.push(Node::block(
                "background-effect",
                vec![Node::leaf("blur true")],
            ));
        }
        let mut rules = Vec::new();
        if !all.is_empty() {
            rules
                .push(Node::block("window-rule", all).noted("What Hyprland did for every window."));
        }
        if let Some(opacity) = self.opacity.1.filter(|o| *o < 1.0) {
            rules.push(Node::block(
                "window-rule",
                vec![
                    Node::leaf("match is-active=false"),
                    Node::leaf(format!("opacity {}", super::number(opacity))),
                ],
            ));
        }
        rules
    }
}
