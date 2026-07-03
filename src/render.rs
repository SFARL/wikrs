//! Render an AST to output. Stage 2 starts with plain text — the same product
//! as Stage 1's `extract::strip`, but driven by a real parse so it can preserve
//! structure. Stage 3 adds [`markdown`], validated by the round-trip harness.

mod markdown;

pub use markdown::markdown;

use crate::ast::Node;

/// Render nodes to clean plain text.
pub fn plain(nodes: &[Node]) -> String {
    let mut out = String::new();
    render_into(nodes, &mut out);
    // Decode HTML entities once over the whole rendered output — cheaper than
    // per-Text-node, and both `render_into` and the strip fallback emit raw text.
    crate::entities::decode(out.trim()).into_owned()
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
                render_list_items(items, out);
                out.push('\n');
            }
            Node::Preformatted(lines) => {
                for line in lines {
                    render_into(line, out);
                    out.push('\n');
                }
                out.push('\n');
            }
            Node::Table { rows } => {
                for row in rows {
                    for (i, cell) in row.iter().enumerate() {
                        if i > 0 {
                            out.push('\t');
                        }
                        render_into(cell, out);
                    }
                    out.push('\n');
                }
                out.push('\n');
            }
            // A block we couldn't give structure: fall back to a best-effort
            // text strip (Stage 1) so its prose isn't lost. The diagnostic still
            // records that we couldn't structure it.
            Node::Unsupported(s) => {
                let text = crate::extract::strip_raw(s);
                if !text.is_empty() {
                    out.push_str(&text);
                    out.push_str("\n\n");
                }
            }
        }
    }
}

/// Render list items as one text line each, recursing into nested sublists so a
/// `* a / ** b` tree flattens to `a\nb` (plain text carries no bullet depth).
fn render_list_items(items: &[Vec<Node>], out: &mut String) {
    for item in items {
        for node in item {
            if !matches!(node, Node::List { .. }) {
                render_into(std::slice::from_ref(node), out);
            }
        }
        out.push('\n');
        for node in item {
            if let Node::List { items, .. } = node {
                render_list_items(items, out);
            }
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
