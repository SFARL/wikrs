//! Render an AST to output. Stage 2 starts with plain text — the same product
//! as Stage 1's `extract::strip`, but driven by a real parse so it can preserve
//! structure. Structured (JSONL) and HTML renderers come later.

use crate::ast::Node;

/// Render nodes to clean plain text.
pub fn plain(nodes: &[Node]) -> String {
    let mut out = String::new();
    render_into(nodes, &mut out);
    out.trim().to_string()
}

fn render_into(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match node {
            Node::Text(s) => out.push_str(s),
            Node::Bold(children) | Node::Italic(children) => render_into(children, out),
            Node::Link { label, .. } => render_into(label, out),
            Node::Heading { content, .. } => {
                render_into(content, out);
                out.push_str("\n\n");
            }
            Node::Paragraph(children) => {
                render_into(children, out);
                out.push_str("\n\n");
            }
            Node::List { items, .. } => {
                for item in items {
                    render_into(item, out);
                    out.push('\n');
                }
                out.push('\n');
            }
            // Dropped from plain text; surfaced separately via diagnostics.
            Node::Unsupported(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    fn text(s: &str) -> Node<'_> {
        Node::Text(Cow::Borrowed(s))
    }

    #[test]
    fn renders_ast_to_plain_text() {
        let ast = [
            Node::Heading {
                level: 2,
                content: vec![text("History")],
            },
            Node::Paragraph(vec![
                text("Earth is the "),
                Node::Bold(vec![text("third")]),
                text(" "),
                Node::Link {
                    target: Cow::Borrowed("Planet"),
                    label: vec![text("planet")],
                },
                text("."),
            ]),
        ];
        assert_eq!(plain(&ast), "History\n\nEarth is the third planet.");
    }
}
