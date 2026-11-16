// Just enough KDL to write a niri config: nodes with their arguments on one line, children in
// braces, and a comment above when there's something to say.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
    // The node's name with its arguments and properties: `mode "1920x1080"`, `gaps 16`.
    pub head: String,
    pub children: Vec<Node>,
    pub comment: Option<String>,
}

impl Node {
    pub fn leaf(head: impl Into<String>) -> Self {
        Node {
            head: head.into(),
            ..Node::default()
        }
    }

    pub fn block(head: impl Into<String>, children: Vec<Node>) -> Self {
        Node {
            head: head.into(),
            children,
            comment: None,
        }
    }

    pub fn noted(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    pub fn render(&self, depth: usize, out: &mut String) {
        let indent = "    ".repeat(depth);
        if let Some(comment) = &self.comment {
            for line in comment.lines() {
                out.push_str(&format!("{indent}// {line}\n"));
            }
        }
        if self.children.is_empty() {
            out.push_str(&format!("{indent}{}\n", self.head));
            return;
        }
        out.push_str(&format!("{indent}{} {{\n", self.head));
        for child in &self.children {
            child.render(depth + 1, out);
        }
        out.push_str(&format!("{indent}}}\n"));
    }
}

// A KDL string: quotes and backslashes escaped, so a regex's \. stays one.
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes_render_as_niri_reads_them() {
        let node = Node::block(
            "window-rule",
            vec![
                Node::leaf(format!(
                    "match app-id={}",
                    quote(r"^org\.gnome\.Calculator$")
                )),
                Node::leaf("open-floating true").noted("float"),
            ],
        );
        let mut out = String::new();
        node.render(0, &mut out);
        assert_eq!(
            out,
            "window-rule {\n    match app-id=\"^org\\\\.gnome\\\\.Calculator$\"\n    // float\n    open-floating true\n}\n"
        );
        assert_eq!(quote("say \"hi\""), "\"say \\\"hi\\\"\"");
    }
}
