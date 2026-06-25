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
        } else if let Some(pre) = parse_pre(block) {
            nodes.push(pre);
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
        let content = line.trim_end_matches('\n');
        let is_heading = heading_parts(content).is_some();
        // A blank line OR a heading line ends the current block; a heading is
        // additionally its own one-line block (real wikitext rarely blank-pads
        // headings, so this is what keeps them from gluing onto prose).
        if content.trim().is_empty() || is_heading {
            if let Some(st) = start.take() {
                let block = s[st..here].trim_end_matches('\n');
                if !block.is_empty() {
                    out.push((st, block));
                }
            }
            if is_heading {
                out.push((here, content));
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

/// If `line` is a single-line heading (`== … ==`), return `(level, inner text)`.
fn heading_parts(line: &str) -> Option<(u8, &str)> {
    if line.contains('\n') {
        return None;
    }
    let t = line.trim();
    let lead = t.bytes().take_while(|&b| b == b'=').count();
    let trail = t.bytes().rev().take_while(|&b| b == b'=').count();
    let level = lead.min(trail);
    if level == 0 || t.len() <= level * 2 {
        return None;
    }
    Some((level.min(6) as u8, t[level..t.len() - level].trim()))
}

/// `== heading ==` on a single line → a `Heading` node.
fn parse_heading(block: &str) -> Option<Node<'_>> {
    let (level, inner) = heading_parts(block)?;
    Some(Node::Heading {
        level,
        content: parse_inline(&tokenizer::inline(inner)),
    })
}

/// A block whose every line starts with a single list marker (`*`/`#`/`:`/`;`)
/// → a flat list (bulleted, numbered, or definition). Nested lists (`**`, `:*`,
/// …) return `None` and stay Unsupported — we don't preserve nesting yet.
fn parse_list(block: &str) -> Option<Node<'_>> {
    let first = block.bytes().next()?;
    if !matches!(first, b'*' | b'#' | b':' | b';') {
        return None;
    }
    let mut items = Vec::new();
    for line in block.lines() {
        let lb = line.as_bytes();
        if !matches!(lb.first(), Some(b'*' | b'#' | b':' | b';'))
            || matches!(lb.get(1), Some(b'*' | b'#' | b':' | b';'))
        {
            return None;
        }
        items.push(parse_inline(&tokenizer::inline(line[1..].trim_start())));
    }
    Some(Node::List {
        ordered: first == b'#',
        items,
    })
}

/// A block whose every line is leading-space indented → a preformatted block
/// (de-indented one space per line; inline wiki markup inside still parses). A
/// template/table/tag inside falls through to the diagnostic path instead.
fn parse_pre(block: &str) -> Option<Node<'_>> {
    if !block.lines().all(|l| l.starts_with(' ')) {
        return None;
    }
    if block.contains("{{") || block.contains("{|") || has_tag(block) {
        return None;
    }
    let lines = block
        .lines()
        .map(|l| parse_inline(&tokenizer::inline(&l[1..])))
        .collect();
    Some(Node::Preformatted(lines))
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

/// Whether the block contains an HTML tag the tokenizer can't handle inline.
/// Comments, `<ref>`, `<nowiki>`, and transparent/void formatting tags are
/// handled there; only structural/unknown tags (`<div>`, `<table>`, …) count.
fn has_tag(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'<' {
            if s[i..].starts_with("<!--") {
                i += 4;
                continue;
            }
            let mut j = i + 1;
            if b.get(j) == Some(&b'/') {
                j += 1;
            }
            let name_start = j;
            while j < b.len() && b[j].is_ascii_alphabetic() {
                j += 1;
            }
            if j > name_start
                && matches!(
                    tokenizer::tag_kind(&s[name_start..j].to_ascii_lowercase()),
                    tokenizer::TagKind::Unsupported
                )
            {
                return true;
            }
        }
        i += 1;
    }
    false
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
    fn isolates_headings_without_blank_lines() {
        let p = parse("Intro text.\n== History ==\nMore text.");
        assert!(matches!(p.nodes[0], Node::Paragraph(_)));
        assert!(matches!(p.nodes[1], Node::Heading { level: 2, .. }));
        assert!(matches!(p.nodes[2], Node::Paragraph(_)));
        assert_eq!(
            render::plain(&p.nodes),
            "Intro text.\n\nHistory\n\nMore text."
        );
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
        // definition lists (;/:) parse as flat lists too
        let d = parse("; term\n: definition");
        assert!(d.diagnostics.is_empty(), "diags: {:?}", d.diagnostics);
        assert_eq!(render::plain(&d.nodes), "term\ndefinition");
        // nested lists stay honestly Unsupported
        let n = parse("* a\n** nested");
        assert!(n.diagnostics.iter().any(|d| d.code == "U-LIST"));
    }

    #[test]
    fn handles_refs_nowiki_comments() {
        let p =
            parse("Text<ref name=x>cite</ref> and <!-- hidden --> a <nowiki>[[literal]]</nowiki>.");
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert_eq!(render::plain(&p.nodes), "Text and  a [[literal]].");
        // a tag we don't handle is still honestly Unsupported
        let t = parse("a <div>html</div> b");
        assert!(t.diagnostics.iter().any(|d| d.code == "U-HTML"));
    }

    #[test]
    fn keeps_inner_of_transparent_html_tags() {
        let p = parse("Use <code>x</code> and <b>'''bold'''</b> and a<br>break.");
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert_eq!(render::plain(&p.nodes), "Use x and bold and a break.");
    }

    #[test]
    fn parses_preformatted_blocks() {
        let p = parse(" code line one\n code [[link|two]]");
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert!(matches!(p.nodes[0], Node::Preformatted(_)));
        assert_eq!(render::plain(&p.nodes), "code line one\ncode two");
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
