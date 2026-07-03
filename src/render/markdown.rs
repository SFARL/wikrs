//! AST → GFM markdown (Stage 3 M-line).
//!
//! Correctness contract: the round-trip harness
//! (`tests/markdown_roundtrip.rs`) — pulldown-cmark must parse this module's
//! output back to the same normal form `mdnorm::from_ast` declares. Design
//! choices that keep the round-trip unambiguous: render inline content from
//! mdnorm's *normalized runs* (kills `***`-adjacency ambiguity at the source),
//! `*`-family emphasis only (`_` has intraword rules), tight lists, fenced
//! code with adaptive fence length.

use crate::ast::Node;
use crate::mdnorm::{self, NfBlock, NfInline, NfItem};

/// Render nodes to GFM markdown.
pub fn markdown(nodes: &[Node]) -> String {
    let blocks = mdnorm::from_ast(nodes);
    let mut out = String::new();
    render_blocks(&blocks, 0, &mut out);
    out.trim_end().to_string()
}

fn render_blocks(blocks: &[NfBlock], indent: usize, out: &mut String) {
    // Two same-type lists separated by one blank line would merge in
    // CommonMark; alternating the marker character starts a fresh list.
    let mut flip_list = false;
    let mut prev_list_ordered: Option<bool> = None;
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            out.push('\n'); // blank line between sibling blocks
        }
        if let NfBlock::List { ordered, .. } = b {
            flip_list = prev_list_ordered == Some(*ordered) && !flip_list;
            prev_list_ordered = Some(*ordered);
        } else {
            prev_list_ordered = None;
            flip_list = false;
        }
        render_block_markers(b, indent, flip_list, out);
    }
}

fn render_block_markers(b: &NfBlock, indent: usize, flip_list: bool, out: &mut String) {
    let pad = " ".repeat(indent);
    match b {
        NfBlock::Heading(level, inl) => {
            out.push_str(&pad);
            for _ in 0..*level {
                out.push('#');
            }
            out.push(' ');
            render_inlines(inl, Ctx::Heading, out);
            out.push('\n');
        }
        NfBlock::Para(inl) => {
            out.push_str(&pad);
            render_inlines(inl, Ctx::LineStart, out);
            out.push('\n');
        }
        NfBlock::List { ordered, items } => {
            for item in items {
                render_item(*ordered, flip_list, item, indent, out);
            }
        }
        NfBlock::Code { info, text } => {
            let fence_len = 3.max(longest_backtick_run(text) + 1);
            let fence: String = "`".repeat(fence_len);
            out.push_str(&pad);
            out.push_str(&fence);
            out.push_str(info);
            out.push('\n');
            for line in text.lines() {
                out.push_str(&pad);
                out.push_str(line);
                out.push('\n');
            }
            out.push_str(&pad);
            out.push_str(&fence);
            out.push('\n');
        }
        NfBlock::Table { rows } => {
            let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
            for (ri, row) in rows.iter().enumerate() {
                out.push_str(&pad);
                out.push('|');
                for ci in 0..cols {
                    out.push(' ');
                    if let Some(cell) = row.get(ci) {
                        render_inlines(cell, Ctx::TableCell, out);
                    }
                    out.push_str(" |");
                }
                out.push('\n');
                if ri == 0 {
                    out.push_str(&pad);
                    out.push('|');
                    for _ in 0..cols {
                        out.push_str(" --- |");
                    }
                    out.push('\n');
                }
            }
        }
    }
}

fn render_item(ordered: bool, flip: bool, item: &NfItem, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    let marker = match (ordered, flip) {
        (true, false) => "1. ",
        (true, true) => "1) ",
        (false, false) => "- ",
        (false, true) => "* ",
    };
    out.push_str(&pad);
    out.push_str(marker);
    // Empty item content + sublists: the sublist must not sit on the marker
    // line; an empty item line is fine in GFM.
    render_inlines(&item.content, Ctx::ListItemStart, out);
    out.push('\n');
    // Adjacent same-type sublists inside one item: same merge hazard.
    let mut sub_flip = false;
    let mut prev_ordered: Option<bool> = None;
    for (si, sub) in item.sublists.iter().enumerate() {
        if let NfBlock::List {
            ordered,
            items: sub_items,
        } = sub
        {
            // A list may not BEGIN with an empty item where it would
            // interrupt a paragraph — end this item's text with a blank
            // line first (looseness is normalized away on the NF side).
            if si == 0
                && !item.content.is_empty()
                && sub_items.first().is_some_and(|it| it.content.is_empty())
            {
                out.push('\n');
            }
            sub_flip = prev_ordered == Some(*ordered) && !sub_flip;
            prev_ordered = Some(*ordered);
        }
        render_block_markers(sub, indent + marker.len(), sub_flip, out);
    }
}

/// Where inline content is being emitted — governs which characters are
/// position-hazardous.
#[derive(Clone, Copy, PartialEq)]
enum Ctx {
    /// First content on a paragraph line.
    LineStart,
    /// Inside an ATX heading (`#` must be escaped — a trailing run would
    /// read as the closing sequence).
    Heading,
    /// First content after a list marker (`- ` / `1. `).
    ListItemStart,
    /// Inside a GFM table cell (`|` must be escaped).
    TableCell,
}

fn render_inlines(inlines: &[NfInline], ctx: Ctx, out: &mut String) {
    for (idx, inl) in inlines.iter().enumerate() {
        let at_start = idx == 0 && ctx != Ctx::TableCell;
        match inl {
            NfInline::Run { text, bold, italic } => {
                // Styled runs have alphanumeric edges (mdnorm peels the
                // rest), which makes every `*` delimiter boundary valid
                // under CommonMark's flanking rules by construction.
                if *bold {
                    out.push_str("**");
                }
                if *italic {
                    out.push('*');
                }
                push_escaped_text(text, at_start && !*bold && !*italic, ctx, out);
                if *italic {
                    out.push('*');
                }
                if *bold {
                    out.push_str("**");
                }
            }
            NfInline::Link { href, label } => {
                // A trailing `!` would fuse with our `[` into image syntax.
                if out.ends_with('!') {
                    out.pop();
                    out.push_str("\\!");
                }
                out.push('[');
                render_inlines(label, Ctx::TableCell, out); // never line-start inside []
                out.push_str("](");
                push_href(href, out);
                out.push(')');
            }
        }
    }
}

/// Escape so pulldown reads this back as literal text. Inline set always;
/// line-start hazards only when the run opens a block line (our runs contain
/// no newlines — whitespace was collapsed); `|` only inside table cells.
fn push_escaped_text(text: &str, at_line_start: bool, ctx: Ctx, out: &mut String) {
    for (i, ch) in text.char_indices() {
        match ch {
            '\\' | '*' | '_' | '[' | ']' | '`' => {
                out.push('\\');
                out.push(ch);
            }
            '<' => out.push_str("&lt;"),
            '&' => out.push_str("&amp;"),
            // `|` escapes everywhere: inside a table cell it splits the
            // cell; in contiguous lines (list item + sublist) it can conjure
            // a GFM table out of thin air (`x|` + `- |` = header + delimiter).
            '|' => out.push_str("\\|"),
            '#' if ctx == Ctx::Heading => {
                out.push('\\');
                out.push(ch);
            }
            '#' | '>' | '-' | '+' | '=' | '~' if at_line_start && i == 0 => {
                out.push('\\');
                out.push(ch);
            }
            '.' | ')' if at_line_start && leading_digits(text, i) => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
}

/// `text[..i]` is all digits and nonempty (ordered-list lookalike `12. x`).
fn leading_digits(text: &str, i: usize) -> bool {
    i > 0 && text[..i].bytes().all(|b| b.is_ascii_digit())
}

/// Link destination: angle-wrap when it contains characters that break the
/// plain `(dest)` form. No character transforms here — those belong to
/// `md_href`, the shared contract (it already excludes `<`/`>`/`|`/controls).
fn push_href(href: &str, out: &mut String) {
    let needs_angle = href.contains([' ', '(', ')']);
    if needs_angle {
        out.push('<');
    }
    out.push_str(href);
    if needs_angle {
        out.push('>');
    }
}

fn longest_backtick_run(text: &str) -> usize {
    let mut max = 0;
    let mut cur = 0;
    for ch in text.chars() {
        if ch == '`' {
            cur += 1;
            max = max.max(cur);
        } else {
            cur = 0;
        }
    }
    max
}
