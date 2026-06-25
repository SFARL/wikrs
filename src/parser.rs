//! Parser: wikitext → AST + diagnostics, over a deliberately small but honest
//! subset (paragraphs, headings, bold/italic, internal links). Everything else
//! — templates, tables, refs, lists, HTML — becomes a [`Node::Unsupported`]
//! paired with a diagnostic. We never pretend to have parsed what we didn't.
//!
//! The supported subset grows over time; the parserTests conformance rate (see
//! `docs/TESTING.md`) is the score that climbs as it does.

use std::borrow::Cow;

use crate::ast::Node;
use crate::diag::Diagnostic;
use crate::tokenizer::{self, Inline};

/// Result of a parse: the AST and any diagnostics raised.
#[derive(Debug)]
pub struct Parsed<'a> {
    pub nodes: Vec<Node<'a>>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse wikitext into an AST, reporting out-of-range constructs as diagnostics.
pub fn parse(wikitext: &str) -> Parsed<'_> {
    let mut nodes = Vec::new();
    let mut diagnostics = Vec::new();
    for (start, block) in blocks(wikitext) {
        if let Some(heading) = parse_heading(block) {
            nodes.push(heading);
        } else if let Some(list) = parse_list(block) {
            nodes.push(list);
        } else if let Some((code, msg)) = unsupported_reason(block) {
            diagnostics.push(Diagnostic::unsupported(
                code,
                start..start + block.len(),
                msg,
            ));
            nodes.push(Node::Unsupported(Cow::Borrowed(block)));
        } else {
            nodes.push(Node::Paragraph(parse_inline(&tokenizer::inline(block))));
        }
    }
    Parsed { nodes, diagnostics }
}

/// Split into blank-line-separated blocks, each tagged with its start offset.
fn blocks(s: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut off = 0;
    for line in s.split_inclusive('\n') {
        let here = off;
        off += line.len();
        if line.trim().is_empty() {
            if let Some(st) = start.take() {
                let block = s[st..here].trim_end_matches('\n');
                if !block.is_empty() {
                    out.push((st, block));
                }
            }
        } else if start.is_none() {
            start = Some(here);
        }
    }
    if let Some(st) = start {
        let block = s[st..off].trim_end_matches('\n');
        if !block.is_empty() {
            out.push((st, block));
        }
    }
    out
}

/// `== heading ==` on a single line → a `Heading` node.
fn parse_heading(block: &str) -> Option<Node<'_>> {
    if block.contains('\n') {
        return None;
    }
    let t = block.trim();
    let lead = t.bytes().take_while(|&b| b == b'=').count();
    let trail = t.bytes().rev().take_while(|&b| b == b'=').count();
    let level = lead.min(trail);
    if level == 0 || t.len() <= level * 2 {
        return None;
    }
    let inner = t[level..t.len() - level].trim();
    Some(Node::Heading {
        level: level.min(6) as u8,
        content: parse_inline(&tokenizer::inline(inner)),
    })
}

/// A block whose every line starts with a single `*` or `#` → a flat list.
/// Nested (`**`), mixed, and definition (`:`/`;`) lists return `None` and are
/// left to `unsupported_reason` (honest: we don't parse them yet).
fn parse_list(block: &str) -> Option<Node<'_>> {
    let marker = block.bytes().next()?;
    if marker != b'*' && marker != b'#' {
        return None;
    }
    let mut items = Vec::new();
    for line in block.lines() {
        let lb = line.as_bytes();
        if lb.first() != Some(&marker) || matches!(lb.get(1), Some(b'*' | b'#' | b':' | b';')) {
            return None;
        }
        items.push(parse_inline(&tokenizer::inline(line[1..].trim_start())));
    }
    Some(Node::List {
        ordered: marker == b'#',
        items,
    })
}

/// Assemble inline tokens into nodes, pairing bold/italic/link delimiters.
/// Unclosed delimiters degrade to literal text rather than swallowing the rest.
fn parse_inline<'a>(tokens: &[Inline<'a>]) -> Vec<Node<'a>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            Inline::Text(s) => {
                out.push(Node::Text(Cow::Borrowed(s)));
                i += 1;
            }
            Inline::LinkOpen => match find(tokens, i + 1, Inline::LinkClose) {
                Some(close) => {
                    out.push(make_link(&tokens[i + 1..close]));
                    i = close + 1;
                }
                None => {
                    out.push(Node::Text(Cow::Borrowed("[[")));
                    i += 1;
                }
            },
            Inline::ExtOpen => match find(tokens, i + 1, Inline::ExtClose) {
                Some(close) => {
                    out.push(make_ext_link(&tokens[i + 1..close]));
                    i = close + 1;
                }
                None => {
                    out.push(Node::Text(Cow::Borrowed("[")));
                    i += 1;
                }
            },
            Inline::Bold => match find(tokens, i + 1, Inline::Bold) {
                Some(close) => {
                    out.push(Node::Bold(parse_inline(&tokens[i + 1..close])));
                    i = close + 1;
                }
                None => {
                    out.push(Node::Text(Cow::Borrowed("'''")));
                    i += 1;
                }
            },
            Inline::Italic => match find(tokens, i + 1, Inline::Italic) {
                Some(close) => {
                    out.push(Node::Italic(parse_inline(&tokens[i + 1..close])));
                    i = close + 1;
                }
                None => {
                    out.push(Node::Text(Cow::Borrowed("''")));
                    i += 1;
                }
            },
            // A stray closer or pipe outside a link is just literal text.
            Inline::LinkClose => {
                out.push(Node::Text(Cow::Borrowed("]]")));
                i += 1;
            }
            Inline::ExtClose => {
                out.push(Node::Text(Cow::Borrowed("]")));
                i += 1;
            }
            Inline::Pipe => {
                out.push(Node::Text(Cow::Borrowed("|")));
                i += 1;
            }
        }
    }
    out
}

/// First index `>= from` whose token matches `target`'s variant.
fn find(tokens: &[Inline], from: usize, target: Inline) -> Option<usize> {
    tokens[from..]
        .iter()
        .position(|t| std::mem::discriminant(t) == std::mem::discriminant(&target))
        .map(|p| from + p)
}

/// Build a `Link` from the tokens between `[[` and `]]` (split on the first `|`).
fn make_link<'a>(inner: &[Inline<'a>]) -> Node<'a> {
    match inner.iter().position(|t| matches!(t, Inline::Pipe)) {
        Some(p) => {
            let target = concat_text(&inner[..p]);
            let label = parse_inline(&inner[p + 1..]);
            let label = if label.is_empty() {
                vec![Node::Text(target.clone())]
            } else {
                label
            };
            Node::Link { target, label }
        }
        None => {
            let target = concat_text(inner);
            Node::Link {
                label: vec![Node::Text(target.clone())],
                target,
            }
        }
    }
}

/// Build a `Link` from an external link `[url label]` (URL = up to the first
/// whitespace; the rest is the label). A bare `[url]` gets an empty label, so it
/// renders to nothing in plain text — matching the Stage 1 extractor.
fn make_ext_link<'a>(inner: &[Inline<'a>]) -> Node<'a> {
    let raw = concat_text(inner);
    if let Some((url, label)) = raw.split_once(char::is_whitespace) {
        Node::Link {
            target: Cow::Owned(url.to_string()),
            label: vec![Node::Text(Cow::Owned(label.trim_start().to_string()))],
        }
    } else {
        Node::Link {
            target: Cow::Owned(raw.into_owned()),
            label: Vec::new(),
        }
    }
}

/// Concatenate the `Text` tokens (a link target is plain text). Borrows when it
/// is a single run; allocates only when joining several.
fn concat_text<'a>(tokens: &[Inline<'a>]) -> Cow<'a, str> {
    match tokens {
        [Inline::Text(s)] => Cow::Borrowed(s),
        _ => {
            let mut s = String::new();
            for t in tokens {
                if let Inline::Text(x) = t {
                    s.push_str(x);
                }
            }
            Cow::Owned(s)
        }
    }
}

/// If a block contains an unhandled construct, return its diagnostic code and
/// message. Conservative on purpose: a block we can't fully handle is flagged,
/// not mangled.
fn unsupported_reason(block: &str) -> Option<(&'static str, String)> {
    if block.contains("{{") {
        return Some(("U-TEMPLATE", "templates are not parsed yet".into()));
    }
    if block.contains("{|") {
        return Some(("U-TABLE", "tables are not parsed yet".into()));
    }
    if has_tag(block) {
        return Some(("U-HTML", "HTML/ref tags are not parsed yet".into()));
    }
    for line in block.lines() {
        let l = line.trim_start();
        if l.starts_with(['*', '#', ':', ';']) {
            return Some(("U-LIST", "lists are not parsed yet".into()));
        }
        if l.starts_with('|') || l.starts_with('!') {
            return Some(("U-TABLE", "table markup is not parsed yet".into()));
        }
        if line.starts_with([' ', '\t']) {
            return Some(("U-PRE", "preformatted blocks are not parsed yet".into()));
        }
    }
    None
}

/// A `<` that opens a tag (`<tag`, `</tag`, `<!--`), not a literal `<` in prose.
fn has_tag(s: &str) -> bool {
    let b = s.as_bytes();
    (0..b.len()).any(|i| {
        b[i] == b'<'
            && matches!(b.get(i + 1), Some(c) if c.is_ascii_alphabetic() || *c == b'!' || *c == b'/')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render;

    #[test]
    fn parses_subset_to_ast_and_text() {
        let wt = "== History ==\n\nEarth is the '''third''' [[Planet|planet]].";
        let p = parse(wt);
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert_eq!(
            render::plain(&p.nodes),
            "History\n\nEarth is the third planet."
        );
        assert!(matches!(p.nodes[0], Node::Heading { level: 2, .. }));
    }

    #[test]
    fn parses_external_links() {
        let p = parse("See [https://nasa.gov NASA] and [https://x.org].");
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert_eq!(render::plain(&p.nodes), "See NASA and .");
    }

    #[test]
    fn parses_simple_lists() {
        let p = parse("* first\n* '''second'''");
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert!(matches!(p.nodes[0], Node::List { ordered: false, .. }));
        assert_eq!(render::plain(&p.nodes), "first\nsecond");
        // nested lists stay honestly Unsupported
        let n = parse("* a\n** nested");
        assert!(n.diagnostics.iter().any(|d| d.code == "U-LIST"));
    }

    #[test]
    fn flags_unsupported_blocks_with_diagnostics() {
        let wt = "Intro paragraph.\n\n{{Infobox|x}}\n\n* a\n** nested";
        let p = parse(wt);
        let codes: Vec<_> = p.diagnostics.iter().map(|d| d.code).collect();
        assert!(codes.contains(&"U-TEMPLATE"), "codes: {codes:?}");
        assert!(codes.contains(&"U-LIST"), "codes: {codes:?}");
        assert!(matches!(p.nodes[0], Node::Paragraph(_)));
        assert!(p
            .nodes
            .iter()
            .any(|n| matches!(n, Node::Unsupported(s) if s.contains("Infobox"))));
    }
}
