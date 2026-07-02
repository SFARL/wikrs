//! Parser: wikitext → AST + diagnostics, over a deliberately small but honest
//! subset (paragraphs, headings, bold/italic, links, nested & definition lists,
//! refs/nowiki/comments, inline HTML formatting, preformatted). Inline templates
//! are dropped with a `W-TEMPLATE` warning (we don't expand them — that would
//! sacrifice the speed that is the whole point); tables and structural HTML
//! become [`Node::Unsupported`]. We never pretend to have parsed what we didn't.
//!
//! The supported subset grows over time; the parserTests coverage (see
//! `docs/TESTING.md`) is the score that tracks it.

use std::borrow::Cow;

use crate::ast::Node;
use crate::diag::Diagnostic;
use crate::tokenizer::{self, Inline};

/// Result of a parse: the AST and any diagnostics raised.
#[derive(Debug)]
pub struct Parsed<'a> {
    /// The parsed block nodes, in source order.
    pub nodes: Vec<Node<'a>>,
    /// Everything the parser could not (or deliberately does not) handle.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse wikitext into an AST, reporting out-of-range constructs as diagnostics.
pub fn parse(wikitext: &str) -> Parsed<'_> {
    let mut nodes = Vec::new();
    let mut diagnostics = Vec::new();
    for (start, block) in blocks(wikitext) {
        let span = start..start + block.len();
        let node = if let Some(heading) = parse_heading(block) {
            heading
        } else if let Some(list) = parse_list(block) {
            list
        } else if let Some(pre) = parse_pre(block) {
            pre
        } else if let Some(table) = parse_table(block) {
            table
        } else if let Some((code, msg)) = unsupported_reason(&strip_inline_templates(block)) {
            diagnostics.push(Diagnostic::unsupported(code, span.clone(), msg));
            Node::Unsupported(Cow::Borrowed(block))
        } else {
            Node::Paragraph(parse_inline(&tokenizer::inline(block)))
        };
        // We extracted the prose but dropped a template we don't expand — say so.
        if !matches!(node, Node::Unsupported(_)) && block.contains("{{") {
            diagnostics.push(Diagnostic::warning(
                "W-TEMPLATE",
                span,
                "template content dropped (not expanded)",
            ));
        }
        nodes.push(node);
    }
    Parsed { nodes, diagnostics }
}

/// Split into blank-line-separated blocks, each tagged with its start offset.
/// Brace-aware: a blank/heading line *inside* an open `{{…}}` template does NOT
/// end the block (else a multi-line template fragments and leaks). Table-aware: a
/// top-level `{|` starts a table block that accumulates until its matching `|}`,
/// isolating it from surrounding prose (even with no blank-line separators).
fn blocks(s: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut off = 0;
    let mut brace_depth = 0usize;
    let mut table_depth = 0usize;
    for line in s.split_inclusive('\n') {
        let here = off;
        off += line.len();
        let content = line.trim_end_matches('\n');
        // Inside an open `{|…|}` table: accumulate every line (blank lines and
        // headings included) until the matching `|}` closes it, then emit the whole
        // table as one block. (`update_table_brace` scans positionally, so the raw
        // line works — no per-line trim needed.)
        if table_depth > 0 {
            (table_depth, brace_depth) = update_table_brace(table_depth, brace_depth, content);
            if table_depth == 0 {
                if let Some(st) = start.take() {
                    let block = s[st..off].trim_end_matches('\n');
                    if !block.is_empty() {
                        out.push((st, block));
                    }
                }
            }
            continue;
        }
        let at_top = brace_depth == 0;
        // A blank line OR a heading line ends the current block (and a heading is
        // additionally its own one-line block); a top-level `{|` opens a table
        // block. Inside an open template, none of these are boundaries. The
        // first-byte guard keeps the common (prose) line off the `trim_start` path.
        let opens_table = at_top
            && matches!(content.as_bytes().first(), Some(b'{' | b' ' | b'\t'))
            && content.trim_start().starts_with("{|");
        let is_heading = at_top && heading_parts(content).is_some();
        if at_top && (content.trim().is_empty() || is_heading || opens_table) {
            if let Some(st) = start.take() {
                let block = s[st..here].trim_end_matches('\n');
                if !block.is_empty() {
                    out.push((st, block));
                }
            }
            if is_heading {
                out.push((here, content));
            }
            if opens_table {
                start = Some(here);
                (table_depth, brace_depth) = update_table_brace(0, brace_depth, content);
                continue;
            }
        } else if start.is_none() {
            start = Some(here);
        }
        brace_depth = update_brace_depth(brace_depth, content);
    }
    if let Some(st) = start {
        let block = s[st..off].trim_end_matches('\n');
        if !block.is_empty() {
            out.push((st, block));
        }
    }
    out
}

/// Net `{{`/`}}` nesting change across one line, scanned left-to-right — the
/// SAME ordered logic as `template_end` / `strip_inline_templates`, so the
/// splitter and the stripper always agree where a template ends. Each `{{` is
/// +1, each `}}` a saturating −1 (a stray `}}` in prose can't underflow). Linear.
fn update_brace_depth(mut depth: usize, line: &str) -> usize {
    let b = line.as_bytes();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'{' && b[i + 1] == b'{' {
            depth += 1;
            i += 2;
        } else if b[i] == b'}' && b[i + 1] == b'}' {
            depth = depth.saturating_sub(1);
            i += 2;
        } else {
            i += 1;
        }
    }
    depth
}

/// Single left-to-right pass updating BOTH table-nesting (`{|`/`|}`) and
/// brace-nesting (`{{`/`}}`) depth across one line. Table markers are only
/// counted at brace depth 0, so a `|}`/`{|` *inside* a `{{template|…|}}` — whose
/// `|}}` contains the bytes `|}` — does not falsely close (or open) a table.
/// This keeps a multi-line table whose cells hold templates (`{{frac|1|12|}}`,
/// `{{convert|…}}`) as one block instead of fragmenting mid-row and leaking the
/// real `|}`. Brace state threads across lines (templates can span them). Linear.
fn update_table_brace(mut table: usize, mut brace: usize, line: &str) -> (usize, usize) {
    let b = line.as_bytes();
    let mut i = 0;
    while i + 1 < b.len() {
        match (b[i], b[i + 1]) {
            (b'{', b'{') => brace += 1,
            (b'}', b'}') => brace = brace.saturating_sub(1),
            (b'{', b'|') if brace == 0 => table += 1,
            (b'|', b'}') if brace == 0 => table = table.saturating_sub(1),
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    (table, brace)
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

/// A block whose every line starts with one or more list markers (`*`/`#`/`:`/`;`)
/// → a (possibly nested) list. Each line's leading run of markers is its depth;
/// deeper lines nest under the preceding shallower item. Definition markers
/// (`:`/`;`) fold into an unordered list (text kept, not the term/definition
/// split). Irregular nesting — a block that starts mid-depth, or jumps a level —
/// returns `None` and stays Unsupported rather than inventing the missing
/// parent (D2: we don't fake structure we didn't see).
fn parse_list(block: &str) -> Option<Node<'_>> {
    let mut lines: Vec<(&str, &str)> = Vec::new();
    for line in block.lines() {
        let prefix_len = line
            .bytes()
            .take_while(|b| matches!(b, b'*' | b'#' | b':' | b';'))
            .count();
        if prefix_len == 0 {
            return None;
        }
        lines.push((&line[..prefix_len], line[prefix_len..].trim_start()));
    }
    build_list(&lines, 0)
}

/// Build the list at marker depth `depth` from `(prefix, content)` lines. Every
/// line is guaranteed `prefix.len() > depth`; the first must be exactly
/// `depth + 1` deep, else the nesting is irregular and we bail (caller → None).
fn build_list<'a>(lines: &[(&'a str, &'a str)], depth: usize) -> Option<Node<'a>> {
    if lines.first()?.0.len() != depth + 1 {
        return None;
    }
    let ordered = lines[0].0.as_bytes()[depth] == b'#';
    let mut items: Vec<Vec<Node<'a>>> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let (prefix, content) = lines[i];
        if prefix.len() != depth + 1 {
            return None;
        }
        let mut item = parse_inline(&tokenizer::inline(content));
        i += 1;
        // The contiguous deeper-prefixed lines that follow nest under this item.
        let nested_start = i;
        while i < lines.len() && lines[i].0.len() > depth + 1 {
            i += 1;
        }
        if i > nested_start {
            item.push(build_list(&lines[nested_start..i], depth + 1)?);
        }
        items.push(item);
    }
    Some(Node::List { ordered, items })
}

/// A block whose every line is leading-space indented → a preformatted block
/// (de-indented one space per line; inline wiki markup inside still parses). A
/// template/table/tag inside falls through to the diagnostic path instead.
fn parse_pre(block: &str) -> Option<Node<'_>> {
    if !block.lines().all(|l| l.starts_with(' ')) {
        return None;
    }
    if block.contains("{|") || has_tag(block) {
        return None;
    }
    let lines = block
        .lines()
        .map(|l| parse_inline(&tokenizer::inline(&l[1..])))
        .collect();
    Some(Node::Preformatted(lines))
}

/// `block.lines()`, except a newline INSIDE a `<ref>…</ref>` does not split — so a
/// multi-line `<ref>{{cite …}}</ref>` in a cell stays in its row instead of
/// fragmenting it into bogus cells. Each returned line is a contiguous slice of
/// `block` (no trailing newline), so cells stay borrowable.
fn table_logical_lines(block: &str) -> Vec<&str> {
    let b = block.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let mut in_ref = false;
    while i < b.len() {
        if in_ref {
            if b[i] == b'<' && b[i..].len() >= 6 && b[i..i + 6].eq_ignore_ascii_case(b"</ref>") {
                in_ref = false;
                i += 6;
                continue;
            }
            i += 1;
        } else if b[i] == b'\n' {
            out.push(&block[start..i]);
            start = i + 1;
            i += 1;
        } else if b[i] == b'<' && ref_opens_body(&block[i..]) {
            in_ref = true;
            i += 1;
        } else {
            i += 1;
        }
    }
    if start < b.len() {
        out.push(&block[start..]);
    }
    out
}

/// `s` starts with `<`. True if it opens a `<ref …>` that has a body — a real
/// `<ref>` (not `<references>`) that isn't self-closing (`<ref … />`). An unclosed
/// `<ref …>` counts as opening a body (it swallows to end-of-block, as the
/// tokenizer does).
fn ref_opens_body(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 4 || !b[1..4].eq_ignore_ascii_case(b"ref") {
        return false;
    }
    if !matches!(b.get(4), Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')) {
        return false;
    }
    match s.find('>') {
        Some(gt) => !s[..gt].trim_end().ends_with('/'),
        None => true,
    }
}

/// A `{| … |}` block → a table (rows × cells of inline content). Cell attributes
/// are dropped; a `<ref>…</ref>` in a cell (even multi-line) is skipped over so it
/// doesn't fragment the row, then dropped by the tokenizer. We bail (`None` →
/// Unsupported) rather than fake structure: on a non-table-markup line (a true
/// multi-line cell, a nested table) and on spanning-cell grids (colspan/rowspan),
/// which we can't flatten faithfully into rows×cells.
fn parse_table(block: &str) -> Option<Node<'_>> {
    if !block.trim_start().starts_with("{|") {
        return None;
    }
    // Spanning cells make a grid we can't flatten faithfully — bail (honest
    // U-TABLE) rather than emit a plausible-but-misaligned table that silently
    // diverges from the rendered grid. (This is what keeps the random sample at
    // 0% silent; see WORKLOG 2026-06-28.)
    if block.contains("colspan") || block.contains("rowspan") {
        return None;
    }
    let mut rows: Vec<Vec<Vec<Node>>> = Vec::new();
    let mut current: Vec<Vec<Node>> = Vec::new();
    let mut started = false;
    for line in table_logical_lines(block) {
        let l = line.trim_start();
        if l.is_empty() {
            continue; // blank line within the table (spacing between rows)
        }
        if l.starts_with("{|") || l.starts_with("|}") || l.starts_with("|+") {
            continue; // open / close / caption
        } else if l.starts_with("|-") {
            if started {
                rows.push(std::mem::take(&mut current));
            }
            started = true;
        } else if let Some(rest) = l.strip_prefix('!') {
            // Header cells separate on `!!` *or* `||` (MediaWiki allows both in a
            // `!` row); split on both so a trailing `||` run doesn't leak as text.
            for part in rest.split("!!") {
                for cell in part.split("||") {
                    current.push(parse_inline(&tokenizer::inline(cell_content(cell))));
                }
            }
            started = true;
        } else if let Some(rest) = l.strip_prefix('|') {
            for cell in rest.split("||") {
                current.push(parse_inline(&tokenizer::inline(cell_content(cell))));
            }
            started = true;
        } else {
            return None; // not table markup (multi-line cell, …) → Unsupported
        }
    }
    if !current.is_empty() {
        rows.push(current);
    }
    Some(Node::Table { rows })
}

/// A table cell's content: the part after the attribute pipe — the first `|` not
/// inside `[[…]]` or `{{…}}` — or the whole cell if there is none. This is
/// MediaWiki's own rule for separating cell attributes from cell content.
fn cell_content(cell: &str) -> &str {
    let b = cell.as_bytes();
    let (mut link, mut tmpl) = (0i32, 0i32);
    let mut i = 0;
    while i < b.len() {
        let two = i + 1 < b.len();
        if two && b[i] == b'[' && b[i + 1] == b'[' {
            link += 1;
            i += 2;
        } else if two && b[i] == b']' && b[i + 1] == b']' {
            link -= 1;
            i += 2;
        } else if two && b[i] == b'{' && b[i + 1] == b'{' {
            tmpl += 1;
            i += 2;
        } else if two && b[i] == b'}' && b[i + 1] == b'}' {
            tmpl -= 1;
            i += 2;
        } else if b[i] == b'|' && link == 0 && tmpl == 0 {
            return cell[i + 1..].trim();
        } else {
            i += 1;
        }
    }
    cell.trim()
}

/// Assemble inline tokens into nodes, pairing bold/italic/link delimiters.
/// Unclosed delimiters degrade to literal text rather than swallowing the rest.
fn parse_inline<'a>(tokens: &[Inline<'a>]) -> Vec<Node<'a>> {
    let mut out = Vec::new();
    let mut i = 0;
    // Once a closer variant has no occurrence ahead, every later opener of that
    // variant also has none — remember it and degrade in O(1) instead of
    // re-scanning to the end each time (keeps unbalanced input like `[[a|`×N
    // linear, not O(n^2)).
    let (mut no_link, mut no_ext, mut no_bold, mut no_italic) = (false, false, false, false);
    // Depth-matched `]]` for each `[[`, so a media/category caption that nests a
    // `[[link]]` closes at its OUTER `]]` (dropping the whole media link) instead
    // of the inner one — which would leak the caption tail + the outer `]]`.
    // Normal links keep first-`]]` matching below (deep nesting stays shallow and
    // linear); only the media-drop path reads this table. Skipped when the block
    // has no link at all, so link-free prose pays nothing.
    let media_close = if tokens.iter().any(|t| matches!(t, Inline::LinkOpen)) {
        link_close_matches(tokens)
    } else {
        Vec::new()
    };
    while i < tokens.len() {
        match tokens[i] {
            Inline::Text(s) => {
                out.push(Node::Text(Cow::Borrowed(s)));
                i += 1;
            }
            Inline::LinkOpen => {
                let found = if no_link {
                    None
                } else {
                    find(tokens, i + 1, Inline::LinkClose)
                };
                match found {
                    Some(close) => {
                        // A media/category link (File:/Image:/Category:) may wrap a
                        // nested `[[link]]` in its caption; first-`]]` matching would
                        // stop at the INNER `]]`, leaking the caption tail + outer
                        // `]]`. Extend to the depth-matched `]]` so make_link drops
                        // the whole span. (`unwrap_or(close)` keeps unbalanced input
                        // on the flat close — still linear, still dropped.)
                        let close = if link_target_is_nonprose(tokens, i + 1) {
                            media_close[i].unwrap_or(close)
                        } else {
                            close
                        };
                        out.push(make_link(&tokens[i + 1..close]));
                        i = close + 1;
                    }
                    None => {
                        no_link = true;
                        out.push(Node::Text(Cow::Borrowed("[[")));
                        i += 1;
                    }
                }
            }
            Inline::ExtOpen => {
                let found = if no_ext {
                    None
                } else {
                    find(tokens, i + 1, Inline::ExtClose)
                };
                match found {
                    Some(close) => {
                        out.push(make_ext_link(&tokens[i + 1..close]));
                        i = close + 1;
                    }
                    None => {
                        no_ext = true;
                        out.push(Node::Text(Cow::Borrowed("[")));
                        i += 1;
                    }
                }
            }
            Inline::Bold => {
                let found = if no_bold {
                    None
                } else {
                    find(tokens, i + 1, Inline::Bold)
                };
                match found {
                    Some(close) => {
                        out.push(Node::Bold(parse_inline(&tokens[i + 1..close])));
                        i = close + 1;
                    }
                    None => {
                        no_bold = true;
                        out.push(Node::Text(Cow::Borrowed("'''")));
                        i += 1;
                    }
                }
            }
            Inline::Italic => {
                let found = if no_italic {
                    None
                } else {
                    find(tokens, i + 1, Inline::Italic)
                };
                match found {
                    Some(close) => {
                        out.push(Node::Italic(parse_inline(&tokens[i + 1..close])));
                        i = close + 1;
                    }
                    None => {
                        no_italic = true;
                        out.push(Node::Text(Cow::Borrowed("''")));
                        i += 1;
                    }
                }
            }
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

/// For every `[[` in `tokens`, the index of its depth-matched `]]` (or `None`
/// when unbalanced). One linear pass with an open-index stack — so even a flood
/// of nested/unbalanced media links stays O(n): the caller looks up the close in
/// O(1) instead of re-scanning per link.
fn link_close_matches(tokens: &[Inline]) -> Vec<Option<usize>> {
    let mut matched = vec![None; tokens.len()];
    let mut open_stack: Vec<usize> = Vec::new();
    for (idx, t) in tokens.iter().enumerate() {
        match t {
            Inline::LinkOpen => open_stack.push(idx),
            Inline::LinkClose => {
                if let Some(open) = open_stack.pop() {
                    matched[open] = Some(idx);
                }
            }
            _ => {}
        }
    }
    matched
}

/// Whether the link whose inner tokens start at `start` is a non-prose
/// media/category link. The namespace (`File:`/`Image:`/`Category:`) lives in
/// the target, i.e. the first token, before any pipe or nested `[[`; a maximal
/// `Text` run never splits it, so peeking that one token suffices.
fn link_target_is_nonprose(tokens: &[Inline], start: usize) -> bool {
    matches!(tokens.get(start), Some(Inline::Text(s)) if is_nonprose_target(s))
}

/// Build a `Link` from the tokens between `[[` and `]]` (split on the first `|`).
fn make_link<'a>(inner: &[Inline<'a>]) -> Node<'a> {
    match inner.iter().position(|t| matches!(t, Inline::Pipe)) {
        Some(p) => {
            let target = concat_text(&inner[..p]);
            // File:/Image: media and Category: tags render as non-prose — drop
            // them entirely so their params/names never leak into the text.
            // Mirrors the Stage 1 stripper's `internal_text`.
            if is_nonprose_target(&target) {
                return Node::Text(Cow::Borrowed(""));
            }
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
            if is_nonprose_target(&target) {
                return Node::Text(Cow::Borrowed(""));
            }
            Node::Link {
                label: vec![Node::Text(target.clone())],
                target,
            }
        }
    }
}

/// Whether a `[[…]]` target renders as non-prose and should be dropped entirely:
/// `File:`/`Image:` media and `Category:` membership tags (neither appears in
/// Parsoid's body text). A leading-colon link (`[[:Category:…]]`) has an empty
/// first segment here, so it stays a normal visible link.
fn is_nonprose_target(target: &str) -> bool {
    let ns = target.split(':').next().unwrap_or("").trim();
    ns.eq_ignore_ascii_case("file")
        || ns.eq_ignore_ascii_case("image")
        || ns.eq_ignore_ascii_case("category")
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

/// Remove inline `{{…}}` template spans (nesting-aware) so block classification
/// isn't fooled by markup *inside* a template (which we drop anyway). Borrows
/// when there's nothing to strip.
fn strip_inline_templates(s: &str) -> Cow<'_, str> {
    if !s.contains("{{") {
        return Cow::Borrowed(s);
    }
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut seg = 0;
    while i + 1 < b.len() {
        if b[i] == b'{' && b[i + 1] == b'{' {
            out.push_str(&s[seg..i]);
            let mut depth = 0usize;
            let mut closed = false;
            while i + 1 < b.len() {
                if b[i] == b'{' && b[i + 1] == b'{' {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'}' && b[i + 1] == b'}' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        closed = true;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            // An unclosed `{{` consumes to end of block (mirrors blocks() and
            // the tokenizer). This also keeps `seg` on a char boundary: the +1
            // byte-stepping above can otherwise stop INSIDE a trailing
            // multibyte char and the `s[seg..]` below would panic (fuzz find).
            if !closed {
                i = b.len();
            }
            seg = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&s[seg..]);
    Cow::Owned(out)
}

/// If a block contains an unhandled construct, return its diagnostic code and
/// message. Conservative on purpose: a block we can't fully handle is flagged,
/// not mangled.
fn unsupported_reason(block: &str) -> Option<(&'static str, String)> {
    if block.contains("{|") {
        return Some(("U-TABLE", "tables are not parsed yet".into()));
    }
    if has_tag(block) {
        return Some(("U-HTML", "HTML/ref tags are not parsed yet".into()));
    }
    for line in block.lines() {
        let l = line.trim_start();
        if l.starts_with(['*', '#', ':', ';']) {
            return Some(("U-LIST", "irregular list nesting not parsed".into()));
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
    fn blocks_keeps_multiline_template_whole() {
        // A {{…}} with an internal blank line must stay ONE block, not fragment.
        let wt = "{{infobox\n|a=1\n\n|b=2\n}}";
        let bs = blocks(wt);
        assert_eq!(bs.len(), 1, "expected one block, got {bs:?}");
        assert_eq!(bs[0].1, "{{infobox\n|a=1\n\n|b=2\n}}");
    }

    #[test]
    fn blocks_still_splits_normal_paragraphs() {
        // Regression guard: prose with no open template still splits on blank lines.
        let bs = blocks("Para one.\n\nPara two.");
        assert_eq!(bs.len(), 2, "got {bs:?}");
        assert_eq!(bs[0].1, "Para one.");
        assert_eq!(bs[1].1, "Para two.");
    }

    #[test]
    fn blocks_unglue_table_from_surrounding_prose() {
        // A {| table glued to prose (no blank lines) must split into 3 blocks:
        // prose / the {|…|} table / prose — even without blank separators.
        let wt = "Prior.\n{| class=\"wikitable\"\n|-\n| a\n|}\nAfter.";
        let bs = blocks(wt);
        let texts: Vec<&str> = bs.iter().map(|(_, b)| *b).collect();
        assert_eq!(
            texts,
            vec!["Prior.", "{| class=\"wikitable\"\n|-\n| a\n|}", "After."],
            "got {bs:?}"
        );
    }

    #[test]
    fn blocks_table_with_internal_blank_line_stays_one_block() {
        // A blank line inside the table does not split it.
        let wt = "{|\n| a\n\n| b\n|}";
        let bs = blocks(wt);
        assert_eq!(bs.len(), 1, "got {bs:?}");
        assert_eq!(bs[0].1, "{|\n| a\n\n| b\n|}");
    }

    #[test]
    fn multiline_template_is_dropped_not_leaked() {
        // A {{#invoke:…}} with internal blank lines used to fragment, leak its body
        // as text, and false-flag U-TABLE. It must now drop cleanly: no leak, a
        // W-TEMPLATE warning, and NO U-TABLE.
        let wt = "Intro.\n\n{{#invoke:Sports table|main\n|name_A=Alpha\n\n|win_A=2 |loss_A=0\n}}\n\nOutro.";
        let p = parse(wt);
        let text = render::plain(&p.nodes);
        assert!(!text.contains("{{"), "leaked template markup: {text:?}");
        assert!(!text.contains("name_A"), "leaked template param: {text:?}");
        assert!(text.contains("Intro."), "lost prose: {text:?}");
        assert!(text.contains("Outro."), "lost prose: {text:?}");
        let codes: Vec<&str> = p.diagnostics.iter().map(|d| d.code).collect();
        assert!(
            codes.contains(&"W-TEMPLATE"),
            "expected W-TEMPLATE, got {codes:?}"
        );
        assert!(
            !codes.contains(&"U-TABLE"),
            "false U-TABLE flag, got {codes:?}"
        );
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
    fn drops_nonprose_links() {
        // File/Image media and Category tags render as non-prose — their params
        // and names must not leak into the text. Mirrors the Stage 1 stripper.
        let p = parse("a [[File:Pic.jpg|thumb|alt=x|cap]] b");
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert_eq!(render::plain(&p.nodes), "a  b");
        assert_eq!(
            render::plain(&parse("[[Image:Y.png|right|200px]]").nodes),
            ""
        );
        // Category membership tags are invisible in body prose — dropped.
        assert_eq!(
            render::plain(&parse("[[Category:Living people]]").nodes),
            ""
        );
        assert_eq!(
            render::plain(&parse("x [[Category:1959 births]] y").nodes),
            "x  y"
        );
        // a normal internal link still keeps its anchor text; a leading-colon
        // link to a category page IS visible prose.
        assert_eq!(
            render::plain(&parse("see [[Earth|our planet]]").nodes),
            "see our planet"
        );
        assert_eq!(
            render::plain(&parse("[[:Category:Physics|physics]]").nodes),
            "physics"
        );
    }

    #[test]
    fn unclosed_template_with_trailing_multibyte_does_not_panic() {
        // Fuzz crash (parse target, first hour): an unclosed `{{` runs the
        // strip_inline_templates scanner to len-1, which can sit INSIDE a
        // multibyte char (`²`) → byte-slice panic. Correct semantics: an
        // unclosed template consumes to end of block (mirrors blocks()/the
        // tokenizer), never leaving a mid-char index.
        // minimal crash repro: 2-byte char right at the end — must not panic,
        // and the unclosed template consumes to end of block.
        assert_eq!(strip_inline_templates("{{a²"), "");
        assert_eq!(
            strip_inline_templates("{{Infobox\n| mass = 5.97e24 kg ²"),
            ""
        );
        // a properly closed template still strips exactly, tail intact
        assert_eq!(strip_inline_templates("{{t|x}} 8.87 m/s²"), " 8.87 m/s²");
        // end-to-end: the crashing shape parses without panicking, and the
        // unexpanded template is flagged (W-TEMPLATE), not silent. (The
        // tokenizer's unclosed-`{{` degrade-to-literal render behavior is
        // pre-existing and out of scope here.)
        let p = parse("{{a²");
        assert!(p.diagnostics.iter().any(|d| d.code == "W-TEMPLATE"));
    }

    #[test]
    fn drops_media_link_with_nested_caption_link() {
        // Real-dump bug (simplewiki "Air"; a leaked `]]` on 6.7% of pages): a
        // File caption wrapping a nested `[[wikilink]]` made the flat first-`]]`
        // matcher close the media link at the INNER `]]`, leaking the caption
        // tail + the outer `]]` as literal text. The whole media link — caption
        // and nested links — must drop cleanly (like the non-nested case).
        assert_eq!(
            render::plain(&parse("[[File:Fan.jpg|thumb|A [[wikt:fan|fan]] moves air.]]").nodes),
            ""
        );
        // Body prose around it survives; the caption/tail and its `]]` don't leak.
        let out = render::plain(
            &parse("Intro.\n\n[[File:Fan.jpg|thumb|A [[wikt:fan|fan]] moves air.]]\n\nBody.").nodes,
        );
        assert!(
            !out.contains("]]") && !out.contains("moves air"),
            "leaked: {out:?}"
        );
        assert!(
            out.contains("Intro.") && out.contains("Body."),
            "lost prose: {out:?}"
        );
        // Two-level nesting drops whole; a normal link right after still renders.
        let out =
            render::plain(&parse("[[File:X.jpg|thumb|see [[Earth]]]] and [[Mars|planet]]").nodes);
        assert!(
            !out.contains("]]") && !out.contains("Earth"),
            "leaked: {out:?}"
        );
        assert!(out.contains("planet"), "lost link: {out:?}");
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
    }

    #[test]
    fn parses_nested_lists() {
        // well-formed nesting parses cleanly into a tree — no diagnostics
        let p = parse("* a\n** b\n** c\n* d");
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert_eq!(render::plain(&p.nodes), "a\nb\nc\nd");
        let Node::List {
            items,
            ordered: false,
        } = &p.nodes[0]
        else {
            panic!("expected unordered List, got {:?}", p.nodes[0]);
        };
        // the first item "a" carries a nested List of two items [b, c]
        let nested = items[0]
            .iter()
            .find_map(|n| match n {
                Node::List { items, .. } => Some(items),
                _ => None,
            })
            .expect("first item should hold a sublist");
        assert_eq!(nested.len(), 2);
        // marker types can change with depth: a numbered list under a bullet
        let m = parse("* top\n*# one\n*# two");
        assert!(m.diagnostics.is_empty(), "diags: {:?}", m.diagnostics);
        // irregular nesting (a depth jump with no parent) stays honestly Unsupported
        let bad = parse("** orphan\n* root");
        assert!(
            bad.diagnostics.iter().any(|d| d.code == "U-LIST"),
            "diags: {:?}",
            bad.diagnostics
        );
    }

    #[test]
    fn handles_refs_nowiki_comments() {
        let p =
            parse("Text<ref name=x>cite</ref> and <!-- hidden --> a <nowiki>[[literal]]</nowiki>.");
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert_eq!(render::plain(&p.nodes), "Text and  a [[literal]].");
        // a structural tag we can't flatten to text is still honestly Unsupported
        let t = parse("a <table>html</table> b");
        assert!(t.diagnostics.iter().any(|d| d.code == "U-HTML"));
    }

    #[test]
    fn keeps_inner_of_transparent_html_tags() {
        let p = parse("Use <code>x</code> and <b>'''bold'''</b> and a<br>break.");
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert_eq!(render::plain(&p.nodes), "Use x and bold and a break.");
    }

    #[test]
    fn keeps_inner_of_transparent_block_tags() {
        // div/center/blockquote/p are presentational containers with no text
        // semantics: keep the inner text, drop the wrapper, no diagnostic — the
        // same treatment <code>/<b> already get, just for block-level containers.
        for wt in [
            "<div id=\"rock\">HTML rocks</div>",
            "<center>'''foo'''</center>",
            "<blockquote>a quote</blockquote>",
            "<p>para</p>",
        ] {
            let p = parse(wt);
            assert!(
                p.diagnostics.is_empty(),
                "{wt:?} -> diags {:?}",
                p.diagnostics
            );
        }
        assert_eq!(
            render::plain(&parse("<div id=\"rock\">HTML rocks</div>").nodes),
            "HTML rocks"
        );
        assert_eq!(
            render::plain(&parse("<center>'''foo'''</center>").nodes),
            "foo"
        );
        // a genuinely structural tag we can't flatten to text stays flagged
        let t = parse("<table><tr><td>x</td></tr></table>");
        assert!(
            t.diagnostics.iter().any(|d| d.code == "U-HTML"),
            "diags: {:?}",
            t.diagnostics
        );
    }

    #[test]
    fn keeps_inner_of_noinclude_onlyinclude() {
        // On the page itself, <noinclude>/<onlyinclude> content SHOWS — keep it,
        // drop the tags. (<includeonly>, which hides content, stays Unsupported:
        // hiding is unsafe across our per-block tokenizer, so we honestly flag it.)
        let p = parse("a<noinclude>b</noinclude>c");
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert_eq!(render::plain(&p.nodes), "abc");
        let o = parse("Goodbye <onlyinclude>Hello world</onlyinclude>");
        assert!(o.diagnostics.is_empty(), "diags: {:?}", o.diagnostics);
        assert_eq!(render::plain(&o.nodes), "Goodbye Hello world");
        // <includeonly> stays honestly flagged (we don't fake hiding its content)
        assert!(parse("x<includeonly>y</includeonly>")
            .diagnostics
            .iter()
            .any(|d| d.code == "U-HTML"));
    }

    #[test]
    fn keeps_inner_of_html_lists() {
        // HTML list tags unwrap to their text; items stay separated by the source
        // newlines between them (we synthesize no bullets — same as wiki lists).
        let p = parse("<ul>\n<li>One</li>\n<li>Two</li>\n</ul>");
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert_eq!(render::plain(&p.nodes), "One\nTwo");
    }

    #[test]
    fn parses_preformatted_blocks() {
        let p = parse(" code line one\n code [[link|two]]");
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert!(matches!(p.nodes[0], Node::Preformatted(_)));
        assert_eq!(render::plain(&p.nodes), "code line one\ncode two");
    }

    #[test]
    fn parses_simple_tables() {
        let p = parse(
            "{| class=\"wikitable\"\n|-\n! Name !! Age\n|-\n| Alice || 30\n|-\n| Bob || 25\n|}",
        );
        assert!(p.diagnostics.is_empty(), "diags: {:?}", p.diagnostics);
        assert!(matches!(p.nodes[0], Node::Table { .. }));
        assert_eq!(render::plain(&p.nodes), "Name\tAge\nAlice\t30\nBob\t25");
        // a cell attribute is dropped, keeping the content
        let a = parse("{|\n| style=\"x\" | hi || [[A|link]]\n|}");
        assert_eq!(render::plain(&a.nodes), "hi\tlink");
        // a multi-line cell makes the table Unsupported (strip-fallback)
        let c = parse("{|\n| cell line one\nstill the cell\n|}");
        assert!(c.diagnostics.iter().any(|d| d.code == "U-TABLE"));
    }

    #[test]
    fn table_cell_template_with_pipe_brace_stays_one_table() {
        // Real-dump bug (simplewiki "Inch", the top `|}` leak): a cell holds
        // {{frac|1|12|}}, whose `|}}` contains the bytes `|}`. A brace-blind
        // table-depth counter reads that as the table close, fragments the table
        // mid-row, and leaks the content + the real `|}`. A `{{…}}` must never
        // open or close a table.
        let wt = "{| class=\"wikitable\"\n|-\n! A !! B\n| x || {{frac|1|12|}} || y\n|}";
        let out = render::plain(&parse(wt).nodes);
        assert!(!out.contains("|}"), "leaked table close: {out:?}");
        assert!(
            !out.contains("{{") && !out.contains("}}"),
            "leaked template: {out:?}"
        );
        assert!(!out.contains("||"), "leaked raw row markup: {out:?}");
    }

    #[test]
    fn table_header_row_splits_on_both_separators_no_leak() {
        // A header row may mix `!!` and `||` separators; both must split, else the
        // trailing `||`-separated cells leak as raw text.
        let node = parse_table("{|\n! a !! b || c\n|}").expect("parses");
        let text = render::plain(std::slice::from_ref(&node));
        assert!(!text.contains("||"), "leaked || markup: {text:?}");
        assert_eq!(text.trim_end(), "a\tb\tc");
    }

    #[test]
    fn table_with_spanning_cells_bails_honestly() {
        // A colspan/rowspan grid can't be flattened faithfully — bail (U-TABLE)
        // rather than emit a plausible-but-misaligned table that silently diverges.
        let p = parse("{|\n! colspan=2 | Title\n|-\n| a || b\n|}");
        assert!(
            p.diagnostics.iter().any(|d| d.code == "U-TABLE"),
            "colspan grid should bail, got {:?}",
            p.diagnostics
        );
    }

    #[test]
    fn parse_table_handles_multiline_ref_in_cell() {
        // A cell with a multi-line <ref>{{cite …}}</ref> used to force a U-TABLE
        // bail (has_multiline_ref). It must now parse: ref dropped, plain cells stay.
        let block = "{| class=\"wikitable\"\n|-\n| Alpha\n| 42<ref>{{cite web |title=x\n|url=y}}</ref>\n|-\n| Beta\n| 7\n|}";
        let node = parse_table(block).expect("table should parse, not bail");
        let text = render::plain(std::slice::from_ref(&node));
        assert!(!text.contains("cite web"), "ref leaked: {text:?}");
        assert!(!text.contains("url=y"), "ref param leaked: {text:?}");
        assert!(
            text.contains("Alpha") && text.contains("Beta"),
            "lost cells: {text:?}"
        );
        assert!(
            text.contains("42") && text.contains('7'),
            "lost data: {text:?}"
        );
    }

    #[test]
    fn table_with_multiline_ref_in_cell_parses_dropping_the_ref() {
        // A <ref> spanning lines inside a cell used to force U-TABLE (its `|`-prefixed
        // cite params looked like cells). It now parses: the ref is dropped, the cell
        // text + lead prose survive, no citation markup leaks (D2).
        let wt = "Intro prose.\n\n{|\n|-\n| Smith <ref name=a>{{cite web\n| url = http://e.com\n| title = T}}</ref>\n| 1974\n|}";
        let p = parse(wt);
        let text = render::plain(&p.nodes);
        assert!(text.contains("Intro prose"), "lost lead prose: {text:?}");
        assert!(text.contains("Smith"), "lost cell text: {text:?}");
        assert!(text.contains("1974"), "lost cell data: {text:?}");
        assert!(!text.contains("url"), "leaked cite markup: {text:?}");
        assert!(
            !p.diagnostics.iter().any(|d| d.code == "U-TABLE"),
            "table should parse now, not bail: {:?}",
            p.diagnostics
        );
    }

    #[test]
    fn drops_inline_templates_with_warning() {
        let p = parse("Real prose with a {{convert|6051|km}} inside.");
        // prose extracted, template dropped from the output
        assert_eq!(render::plain(&p.nodes), "Real prose with a  inside.");
        // but we honestly flag that content was dropped (a Warning, not Unsupported)
        let w = p
            .diagnostics
            .iter()
            .find(|d| d.code == "W-TEMPLATE")
            .unwrap();
        assert_eq!(w.severity, crate::diag::Severity::Warning);
    }

    #[test]
    fn flags_unsupported_blocks_with_diagnostics() {
        let wt = "Intro paragraph.\n\n{{Infobox|x}}\n\n<table>raw block</table>";
        let p = parse(wt);
        let codes: Vec<_> = p.diagnostics.iter().map(|d| d.code).collect();
        assert!(codes.contains(&"W-TEMPLATE"), "codes: {codes:?}"); // template dropped (warning)
        assert!(codes.contains(&"U-HTML"), "codes: {codes:?}"); // structural HTML unsupported
        assert!(matches!(p.nodes[0], Node::Paragraph(_)));
        // the structural-HTML block stays Unsupported, kept verbatim
        assert!(p
            .nodes
            .iter()
            .any(|n| matches!(n, Node::Unsupported(s) if s.contains("raw"))));
    }
}
