//! Markdown round-trip normal form (Stage 3 M-line).
//!
//! `#[doc(hidden)]`: dev/test plumbing, no semver promise. Both sides of the
//! round-trip harness map into these types; equality here is the definition of
//! "the markdown means what the AST says". The wikrs side (this module's
//! [`from_ast`]) states the *declared intent*; the pulldown-cmark side
//! (`tests/support/pulldown_nf.rs`) states what our emitted markdown *actually
//! means* to an independent GFM implementation. The mapping contract table
//! lives in `docs/superpowers/plans/2026-07-02-markdown-roundtrip.md` §0.

use crate::ast::Node;

/// A block in the round-trip normal form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NfBlock {
    /// `level` 1–6, inline content.
    Heading(u8, Vec<NfInline>),
    /// Paragraph of inline content (dropped when empty after normalization).
    Para(Vec<NfInline>),
    /// Flat list; nesting lives inside items.
    List {
        /// `true` = ordered (`1.`); start number is not compared.
        ordered: bool,
        /// The list's items.
        items: Vec<NfItem>,
    },
    /// Fenced code block: `Preformatted` (`info: ""`) and the `Unsupported`
    /// visible marker (`info: "wikitext"`, verbatim source).
    Code {
        /// Fence info string.
        info: String,
        /// Literal text (trailing newline trimmed).
        text: String,
    },
    /// Rows of cells of inline content; the GFM head row is folded back in.
    Table {
        /// All rows including the header row.
        rows: Vec<Vec<Vec<NfInline>>>,
    },
}

/// One list item: inline content plus any nested sublists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NfItem {
    /// The item's own inline content.
    pub content: Vec<NfInline>,
    /// Nested lists (always `NfBlock::List`).
    pub sublists: Vec<NfBlock>,
}

/// Inline content: styled text runs and links. Formatting nesting order is
/// deliberately flattened to per-run flags — the semantics is "which text has
/// which style", not tree shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NfInline {
    /// A styled text run.
    Run {
        /// The text (entities decoded, whitespace collapsed).
        text: String,
        /// Effective bold flag.
        bold: bool,
        /// Effective italic flag.
        italic: bool,
    },
    /// A link; the label holds `Run`s only.
    Link {
        /// The destination as it appears in the markdown.
        href: String,
        /// The visible label.
        label: Vec<NfInline>,
    },
}

/// The pinned internal-link href rule: entities decoded first (MediaWiki
/// title semantics — `[[WW&nbsp;II]]` targets "WW II"), spaces → `_`,
/// RFC 3986 path charset kept **except `&`** (percent-encoded so a markdown
/// destination can never be re-read as an HTML entity), the rest
/// percent-encoded, `./` prefix forecloses scheme injection. External targets
/// (tokenizer-vetted schemes) pass through.
pub fn md_href(target: &str) -> String {
    if ["http://", "https://", "ftp://", "mailto:", "//"]
        .iter()
        .any(|p| target.starts_with(p))
    {
        // Pass through, but percent-encode control bytes (CommonMark input
        // normalization would rewrite them — NUL → U+FFFD), `<`/`>`
        // (unrepresentable in either markdown destination form), `|` (splits
        // GFM table cells even inside a destination), and any `&` that forms
        // an entity-shaped reference (CommonMark decodes entities inside
        // destinations; a bare query separator `&a=1` has no `;` and is kept).
        let mut href = String::with_capacity(target.len());
        for (i, ch) in target.char_indices() {
            match ch {
                '\0'..='\u{1f}' | '\u{7f}' | '<' | '>' | '|' | '\\' => {
                    href.push_str(&format!("%{:02X}", ch as u32));
                }
                '&' if entity_shaped(&target.as_bytes()[i + 1..]) => href.push_str("%26"),
                _ => href.push(ch),
            }
        }
        return href;
    }
    let decoded = crate::entities::decode(target);
    let mut href = String::with_capacity(decoded.len() + 2);
    href.push_str("./");
    for &b in decoded.as_bytes() {
        match b {
            b' ' => href.push('_'),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => href.push(b as char),
            b'-' | b'.' | b'_' | b'~' | b'!' | b'$' | b'\'' | b'(' | b')' | b'*' | b'+' | b','
            | b';' | b'=' | b':' | b'@' | b'/' => href.push(b as char),
            _ => href.push_str(&format!("%{b:02X}")),
        }
    }
    href
}

/// Whitespace-collapse, merge same-style neighbors, trim the sequence edges,
/// drop empties. Both sides call this — it IS the declared inline
/// normalization.
pub fn normalize_inlines(inlines: Vec<NfInline>) -> Vec<NfInline> {
    // 1. collapse whitespace inside runs; recurse into link labels.
    let flat: Vec<NfInline> = inlines
        .into_iter()
        .map(|i| match i {
            NfInline::Run { text, bold, italic } => NfInline::Run {
                text: collapse_ws(&text),
                bold,
                italic,
            },
            NfInline::Link { href, label } => NfInline::Link {
                href,
                label: normalize_inlines(label),
            },
        })
        .collect();
    // 2. merge adjacent same-style runs FIRST — the pulldown side splits text
    //    at entities/escapes, and peeling before merging would strip style
    //    from a lone punctuation fragment that belongs to a styled phrase.
    let flat = merge_runs(flat);
    // 3. styled-run edges must be alphanumeric: peel edge whitespace,
    //    punctuation, and symbols into plain runs. CommonMark's flanking
    //    rules make emphasis with a space edge unparseable (`* and *`) and
    //    emphasis with a punctuation edge unclosable next to a letter
    //    (`*hot!*x`); with alphanumeric edges every delimiter boundary is
    //    valid by construction. The peeled characters render identically —
    //    styled edge punctuation is not visually distinguishable — so the
    //    contract declares them unstyled.
    let mut peeled: Vec<NfInline> = Vec::with_capacity(flat.len());
    for i in flat {
        match i {
            NfInline::Run { text, bold, italic } if (bold || italic) && !text.is_empty() => {
                let core_start = text
                    .find(|c: char| c.is_alphanumeric())
                    .unwrap_or(text.len());
                let core_end = text
                    .rfind(|c: char| c.is_alphanumeric())
                    .map_or(0, |p| p + text[p..].chars().next().unwrap().len_utf8());
                if core_start >= core_end {
                    // no alphanumeric core: the whole run is unstylable
                    peeled.push(plain_run(&text));
                    continue;
                }
                if core_start > 0 {
                    peeled.push(plain_run(&text[..core_start]));
                }
                peeled.push(NfInline::Run {
                    text: text[core_start..core_end].to_string(),
                    bold,
                    italic,
                });
                if core_end < text.len() {
                    peeled.push(plain_run(&text[core_end..]));
                }
            }
            other => peeled.push(other),
        }
    }
    // 4. merge again (peeling created plain runs next to plain neighbors).
    let mut merged = merge_runs(peeled);
    // 5. trim sequence edges + drop empty runs.
    if let Some(NfInline::Run { text, .. }) = merged.first_mut() {
        *text = text.trim_start().to_string();
    }
    if let Some(NfInline::Run { text, .. }) = merged.last_mut() {
        *text = text.trim_end().to_string();
    }
    merged.retain(|i| !matches!(i, NfInline::Run { text, .. } if text.is_empty()));
    merged
}

/// Does `rest` (the bytes after a `&`) look like an HTML entity reference —
/// `#digits;`, `#x hex;`, or `name;`? Conservative superset of what CommonMark
/// decodes: over-matching only means a `%26` both sides agree on.
fn entity_shaped(rest: &[u8]) -> bool {
    let body = match rest.first() {
        Some(b'#') => match rest.get(1) {
            Some(b'x') | Some(b'X') => &rest[2..],
            _ => &rest[1..],
        },
        _ => rest,
    };
    let mut len = 0;
    for &b in body {
        match b {
            b';' => return len > 0,
            _ if b.is_ascii_alphanumeric() => len += 1,
            _ => return false,
        }
    }
    false
}

/// Merge adjacent same-style runs, re-collapsing whitespace across each seam.
fn merge_runs(runs: Vec<NfInline>) -> Vec<NfInline> {
    let mut merged: Vec<NfInline> = Vec::with_capacity(runs.len());
    for i in runs {
        match (merged.last_mut(), &i) {
            (
                Some(NfInline::Run {
                    text: t0,
                    bold: b0,
                    italic: i0,
                }),
                NfInline::Run { text, bold, italic },
            ) if b0 == bold && i0 == italic => {
                t0.push_str(text);
                *t0 = collapse_ws(t0);
            }
            _ => merged.push(i),
        }
    }
    merged
}

/// Code-block text normalization: trailing whitespace trimmed, NUL → U+FFFD,
/// line endings to LF (CommonMark treats `\r` and `\r\n` as line endings, so
/// they cannot round-trip literally through a fenced block).
fn code_norm(s: &str) -> String {
    s.trim_end()
        .replace('\0', "\u{FFFD}")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

fn plain_run(s: &str) -> NfInline {
    NfInline::Run {
        text: s.to_string(),
        bold: false,
        italic: false,
    }
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !in_ws {
                out.push(' ');
            }
            in_ws = true;
        } else {
            // CommonMark input normalization replaces U+0000 with U+FFFD in
            // any conforming consumer; the contract compares accordingly.
            out.push(if ch == '\0' { '\u{FFFD}' } else { ch });
            in_ws = false;
        }
    }
    out
}

/// AST → NF: the declared intent. Shares ONLY [`md_href`] + entity decoding
/// with the renderer (both are contract entries); everything the harness must
/// judge independently — escaping, indentation, fences, level arithmetic — is
/// not here.
pub fn from_ast(nodes: &[Node]) -> Vec<NfBlock> {
    let mut out = Vec::new();
    for node in nodes {
        match node {
            Node::Heading { level, content } => {
                out.push(NfBlock::Heading(
                    (*level).clamp(1, 6),
                    inline_nf(content, false, false),
                ));
            }
            Node::Paragraph(children) => {
                let inl = inline_nf(children, false, false);
                if !inl.is_empty() {
                    out.push(NfBlock::Para(inl));
                }
            }
            Node::List { ordered, items } => {
                if let Some(list) = list_nf(*ordered, items) {
                    out.push(list);
                }
            }
            Node::Preformatted(lines) => {
                let text = lines
                    .iter()
                    .map(|l| plain_text(l))
                    .collect::<Vec<_>>()
                    .join("\n");
                out.push(NfBlock::Code {
                    info: String::new(),
                    text: code_norm(&text),
                });
            }
            Node::Unsupported(s) => out.push(NfBlock::Code {
                info: "wikitext".to_string(),
                text: code_norm(s),
            }),
            Node::Table { rows } => {
                // GFM forces rectangular tables (the header row fixes the
                // column count), so the contract compares rows padded to the
                // table's max width with empty cells. A table with no rows —
                // or only zero-cell rows (`{| |- |-`) — has no GFM form and
                // is dropped.
                let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
                if cols > 0 {
                    out.push(NfBlock::Table {
                        rows: rows
                            .iter()
                            .map(|r| {
                                let mut row: Vec<Vec<NfInline>> =
                                    r.iter().map(|c| inline_nf(c, false, false)).collect();
                                row.resize(cols, Vec::new());
                                row
                            })
                            .collect(),
                    });
                }
            }
            // Stray top-level inline (parser wraps prose in Paragraph; defensive).
            other => {
                let inl = inline_nf(std::slice::from_ref(other), false, false);
                if !inl.is_empty() {
                    out.push(NfBlock::Para(inl));
                }
            }
        }
    }
    out
}

/// Items with neither content nor sublists are dropped (CommonMark forbids an
/// empty item interrupting a paragraph, so `- t` + nested empty `-` cannot
/// round-trip — and an empty wikitext bullet carries nothing); a list left
/// with zero items is dropped with it.
fn list_nf(ordered: bool, items: &[Vec<Node>]) -> Option<NfBlock> {
    let items: Vec<NfItem> = items
        .iter()
        .map(|item| {
            let mut content = Vec::new();
            let mut sublists = Vec::new();
            for n in item {
                if let Node::List { ordered, items } = n {
                    sublists.extend(list_nf(*ordered, items));
                } else {
                    walk_inline(std::slice::from_ref(n), false, false, false, &mut content);
                }
            }
            NfItem {
                content: normalize_inlines(content),
                sublists,
            }
        })
        .filter(|it| !it.content.is_empty() || !it.sublists.is_empty())
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(NfBlock::List { ordered, items })
    }
}

fn inline_nf(nodes: &[Node], bold: bool, italic: bool) -> Vec<NfInline> {
    let mut out = Vec::new();
    walk_inline(nodes, bold, italic, false, &mut out);
    normalize_inlines(out)
}

fn walk_inline(nodes: &[Node], bold: bool, italic: bool, in_label: bool, out: &mut Vec<NfInline>) {
    for node in nodes {
        match node {
            Node::Text(s) => out.push(NfInline::Run {
                text: crate::entities::decode(s).into_owned(),
                bold,
                italic,
            }),
            Node::Bold(children) => walk_inline(children, true, italic, in_label, out),
            Node::Italic(children) => walk_inline(children, bold, true, in_label, out),
            // A link with an empty target (`[[]]`) is degenerate wikitext:
            // flatten to its visible text (contract normalization).
            Node::Link { target, label } if target.trim().is_empty() => {
                let text = plain_text(label);
                out.push(NfInline::Run { text, bold, italic });
            }
            // Markdown cannot nest links: a link inside another link's label
            // flattens to its visible text (contract normalization).
            Node::Link { target, label } if in_label => {
                let text = if label.is_empty() {
                    target.to_string()
                } else {
                    plain_text(label)
                };
                out.push(NfInline::Run { text, bold, italic });
            }
            Node::Link { target, label } => {
                let href = md_href(target);
                let label_nf = if label.is_empty() {
                    vec![NfInline::Run {
                        text: target.to_string(),
                        bold: false,
                        italic: false,
                    }]
                } else {
                    let mut inner = Vec::new();
                    walk_inline(label, false, false, true, &mut inner);
                    normalize_inlines(inner)
                };
                out.push(NfInline::Link {
                    href,
                    label: label_nf,
                });
            }
            // Block nodes in inline position: flatten to text (defensive).
            other => out.push(NfInline::Run {
                text: plain_text(std::slice::from_ref(other)),
                bold,
                italic,
            }),
        }
    }
}

fn plain_text(nodes: &[Node]) -> String {
    let mut s = String::new();
    collect_text(nodes, &mut s);
    s
}

fn collect_text(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text(s) => out.push_str(&crate::entities::decode(s)),
            Node::Bold(c) | Node::Italic(c) => collect_text(c, out),
            Node::Link { label, target } => {
                if label.is_empty() {
                    out.push_str(target);
                } else {
                    collect_text(label, out);
                }
            }
            Node::Heading { content, .. } => collect_text(content, out),
            Node::Paragraph(c) => collect_text(c, out),
            Node::List { items, .. } => items.iter().for_each(|i| collect_text(i, out)),
            Node::Preformatted(lines) => lines.iter().for_each(|l| collect_text(l, out)),
            Node::Table { rows } => rows.iter().flatten().for_each(|c| collect_text(c, out)),
            Node::Unsupported(s) => out.push_str(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styled_edges_are_alphanumeric_after_normalization() {
        // "whitespace/punctuation carries no style": edges peel to plain runs.
        let runs = normalize_inlines(vec![NfInline::Run {
            text: " !hot stuff! ".to_string(),
            bold: false,
            italic: true,
        }]);
        assert_eq!(
            runs,
            vec![
                NfInline::Run {
                    text: "!".to_string(),
                    bold: false,
                    italic: false
                },
                NfInline::Run {
                    text: "hot stuff".to_string(),
                    bold: false,
                    italic: true
                },
                NfInline::Run {
                    text: "!".to_string(),
                    bold: false,
                    italic: false
                },
            ]
        );
        // no alphanumeric core → style dropped entirely
        let runs = normalize_inlines(vec![NfInline::Run {
            text: "!!!".to_string(),
            bold: true,
            italic: false,
        }]);
        assert_eq!(
            runs,
            vec![NfInline::Run {
                text: "!!!".to_string(),
                bold: false,
                italic: false
            }]
        );
    }
}
