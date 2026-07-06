//! Serialize an extracted page as plain text or JSON Lines.

use serde::Serialize;

use crate::ast::Node;
use crate::diag::{Diagnostic, Severity};

#[derive(Serialize)]
struct Record<'a> {
    title: &'a str,
    text: &'a str,
    /// `None` (key absent) when the engine cannot diagnose (strip) — an empty
    /// array would falsely claim "checked, found nothing".
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostics: Option<Vec<DiagView<'a>>>,
}

/// Wire form of a [`Diagnostic`]: flat byte span, lowercase severity.
#[derive(Serialize)]
struct DiagView<'a> {
    code: &'static str,
    severity: &'static str,
    start: usize,
    end: usize,
    message: &'a str,
}

fn diag_views(diags: &[Diagnostic]) -> Vec<DiagView<'_>> {
    diags
        .iter()
        .map(|d| DiagView {
            code: d.code,
            severity: match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Unsupported => "unsupported",
            },
            start: d.span.start,
            end: d.span.end,
            message: &d.message,
        })
        .collect()
}

/// One JSON object per line: `{"title":…,"text":…,"diagnostics":[…]}`.
/// `diagnostics: None` omits the key entirely (strip engine — can't diagnose).
pub fn to_jsonl(title: &str, text: &str, diagnostics: Option<&[Diagnostic]>) -> String {
    serde_json::to_string(&Record {
        title,
        text,
        diagnostics: diagnostics.map(diag_views),
    })
    .expect("serialize record")
}

#[derive(Serialize)]
struct SectionsRecord<'a> {
    title: &'a str,
    sections: Vec<Section>,
    diagnostics: Vec<DiagView<'a>>,
}

#[derive(Serialize)]
struct Section {
    level: u8,
    heading: String,
    text: String,
}

/// One markdown document per page: escaped `# title`, blank line, body.
/// Title escaping reuses the renderer's own rules via a single-Text render —
/// one escaping path, no drift.
pub fn to_markdown(title: &str, body: &str) -> String {
    use std::borrow::Cow;
    let title_md =
        crate::render::markdown(&[Node::Paragraph(vec![Node::Text(Cow::Borrowed(title))])]);
    let mut out = String::with_capacity(title_md.len() + body.len() + 8);
    out.push_str("# ");
    out.push_str(&title_md);
    out.push('\n');
    if !body.is_empty() {
        out.push('\n');
        out.push_str(body);
    }
    out.push('\n');
    out
}

/// One JSON object per line with the page split into flat, level-tagged
/// sections — the RAG-chunking contract pinned in
/// `docs/stages/stage-3-llm-output.md`. Every top-level `Heading` starts a new
/// section (`level` = its `=` count); prose before the first heading is the
/// lead (`level: 0`, empty heading), omitted when the page starts with a
/// heading. A heading directly followed by another keeps its section with
/// empty text. Not a rendering: pure AST serialization — `heading`/`text` go
/// through the same `render::plain` the differential already anchors.
pub fn to_sections_jsonl(title: &str, nodes: &[Node], diagnostics: &[Diagnostic]) -> String {
    let mut sections = Vec::new();
    let (mut level, mut heading) = (0u8, String::new());
    let mut start = 0;
    for (i, node) in nodes.iter().enumerate() {
        if let Node::Heading {
            level: next_level,
            content,
        } = node
        {
            // Flush the running section; the lead exists only if the page has
            // content before its first heading.
            if i > 0 || level != 0 {
                sections.push(Section {
                    level,
                    heading: std::mem::take(&mut heading),
                    text: crate::render::plain(&nodes[start..i]),
                });
            }
            level = *next_level;
            heading = crate::render::plain(content);
            start = i + 1;
        }
    }
    // Trailing flush: remaining content, or a page-final (possibly empty)
    // headed section. An empty page yields no sections at all — no empty-lead
    // shell.
    if start < nodes.len() || level != 0 {
        sections.push(Section {
            level,
            heading,
            text: crate::render::plain(&nodes[start..]),
        });
    }
    serde_json::to_string(&SectionsRecord {
        title,
        sections,
        diagnostics: diag_views(diagnostics),
    })
    .expect("serialize sections")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Node;
    use std::borrow::Cow;

    fn text(s: &str) -> Node<'_> {
        Node::Text(Cow::Borrowed(s))
    }

    fn heading(level: u8, s: &str) -> Node<'_> {
        Node::Heading {
            level,
            content: vec![text(s)],
        }
    }

    fn para(s: &str) -> Node<'_> {
        Node::Paragraph(vec![text(s)])
    }

    #[test]
    fn sections_flat_split_with_lead() {
        // Lead (level 0, empty heading), then h2, then h3 — flat, not nested.
        let nodes = [
            para("Lead prose."),
            heading(2, "History"),
            para("Old times."),
            heading(3, "Details"),
            para("Fine print."),
        ];
        assert_eq!(
            to_sections_jsonl("A \"B\"", &nodes, &[]),
            r#"{"title":"A \"B\"","sections":[{"level":0,"heading":"","text":"Lead prose."},{"level":2,"heading":"History","text":"Old times."},{"level":3,"heading":"Details","text":"Fine print."}],"diagnostics":[]}"#
        );
    }

    #[test]
    fn sections_no_lead_when_page_starts_with_heading() {
        let nodes = [heading(2, "Only"), para("Body.")];
        assert_eq!(
            to_sections_jsonl("T", &nodes, &[]),
            r#"{"title":"T","sections":[{"level":2,"heading":"Only","text":"Body."}],"diagnostics":[]}"#
        );
    }

    #[test]
    fn sections_keep_empty_between_consecutive_headings() {
        // A heading immediately followed by another is real structure — keep it
        // with empty text, downstream chunkers decide.
        let nodes = [heading(2, "Empty"), heading(2, "Full"), para("x")];
        assert_eq!(
            to_sections_jsonl("T", &nodes, &[]),
            r#"{"title":"T","sections":[{"level":2,"heading":"Empty","text":""},{"level":2,"heading":"Full","text":"x"}],"diagnostics":[]}"#
        );
    }

    #[test]
    fn sections_heading_is_plain_rendered() {
        // Inline formatting and entities inside a heading reduce to plain text.
        let nodes = [
            Node::Heading {
                level: 2,
                content: vec![Node::Bold(vec![text("Bold")]), text(" &amp; more")],
            },
            para("x"),
        ];
        assert_eq!(
            to_sections_jsonl("T", &nodes, &[]),
            r#"{"title":"T","sections":[{"level":2,"heading":"Bold & more","text":"x"}],"diagnostics":[]}"#
        );
    }

    #[test]
    fn sections_from_parsed_wikitext() {
        // Through the real parser: levels are the `=` count, lead is level 0.
        let parsed =
            crate::parser::parse("Lead.\n\n== History ==\n\nOld.\n\n=== Deep ===\n\nFine.");
        let line = to_sections_jsonl("Page", &parsed.nodes, &parsed.diagnostics);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        let secs = v["sections"].as_array().unwrap();
        assert_eq!(secs.len(), 3, "lead + 2 headings: {line}");
        assert_eq!(
            (secs[0]["level"].as_u64(), secs[0]["text"].as_str()),
            (Some(0), Some("Lead."))
        );
        assert_eq!(secs[1]["heading"].as_str(), Some("History"));
        assert_eq!(secs[2]["level"].as_u64(), Some(3));
    }

    #[test]
    fn markdown_page_has_escaped_h1_then_body() {
        assert_eq!(
            to_markdown("A*B", "**Earth** is here."),
            "# A\\*B\n\n**Earth** is here.\n"
        );
    }

    #[test]
    fn jsonl_has_title_and_text() {
        // No diagnostics available (strip): the key is absent, not an empty
        // array — absence means "not checked", [] means "checked, clean".
        assert_eq!(
            to_jsonl("Earth", "third planet", None),
            r#"{"title":"Earth","text":"third planet"}"#
        );
        assert_eq!(
            to_jsonl("Earth", "third planet", Some(&[])),
            r#"{"title":"Earth","text":"third planet","diagnostics":[]}"#
        );
    }

    #[test]
    fn jsonl_serializes_diagnostics_with_span_and_severity() {
        let diags = [
            crate::diag::Diagnostic::unsupported("U-TABLE", 3..17, "tables are not parsed yet"),
            crate::diag::Diagnostic::warning("W-TEMPLATE", 20..31, "template dropped"),
        ];
        assert_eq!(
            to_jsonl("T", "body", Some(&diags)),
            r#"{"title":"T","text":"body","diagnostics":[{"code":"U-TABLE","severity":"unsupported","start":3,"end":17,"message":"tables are not parsed yet"},{"code":"W-TEMPLATE","severity":"warning","start":20,"end":31,"message":"template dropped"}]}"#
        );
    }
}
