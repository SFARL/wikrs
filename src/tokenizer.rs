//! Inline tokenizer: a text run → a flat stream of inline tokens. Block
//! structure (headings, paragraphs) is the parser's job; this only handles the
//! lexical inline markers. All markers are ASCII, so slicing stays on UTF-8
//! char boundaries. Single linear scan.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inline<'a> {
    Text(&'a str),
    Bold,      // '''
    Italic,    // ''
    LinkOpen,  // [[
    LinkClose, // ]]
    ExtOpen,   // [ that starts an external link: [http://… …]
    ExtClose,  // a single ] (closes an external link; else literal text)
    Pipe,      // |
}

/// Tokenize one inline text run.
pub fn inline(s: &str) -> Vec<Inline<'_>> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut start = 0;
    while i < b.len() {
        // Multi-char spans at `<`: comment / <ref> (dropped), <nowiki> (inner kept).
        if b[i] == b'<' {
            if let Some(span) = tag_span(s, i) {
                if start < i {
                    out.push(Inline::Text(&s[start..i]));
                }
                match span {
                    TagSpan::Drop(end) => i = end,
                    TagSpan::Keep(inner, end) => {
                        if !inner.is_empty() {
                            out.push(Inline::Text(inner));
                        }
                        i = end;
                    }
                    TagSpan::SkipTag(end) => i = end,
                    TagSpan::Space(end) => {
                        out.push(Inline::Text(" "));
                        i = end;
                    }
                }
                start = i;
                continue;
            }
        }
        // drop an inline template {{…}} (nesting-aware), keeping surrounding prose
        if b[i] == b'{' && b.get(i + 1) == Some(&b'{') {
            if let Some(end) = template_end(s, i) {
                if start < i {
                    out.push(Inline::Text(&s[start..i]));
                }
                i = end;
                start = i;
                continue;
            }
        }
        let marker = if b[i] == b'\'' {
            match b[i..].iter().take_while(|&&c| c == b'\'').count() {
                n if n >= 3 => Some((Inline::Bold, 3)),
                2 => Some((Inline::Italic, 2)),
                _ => None, // a lone apostrophe is text
            }
        } else if b[i] == b'[' && b.get(i + 1) == Some(&b'[') {
            Some((Inline::LinkOpen, 2))
        } else if b[i] == b'[' && is_ext_scheme(&s[i + 1..]) {
            Some((Inline::ExtOpen, 1))
        } else if b[i] == b']' && b.get(i + 1) == Some(&b']') {
            Some((Inline::LinkClose, 2))
        } else if b[i] == b']' {
            Some((Inline::ExtClose, 1))
        } else if b[i] == b'|' {
            Some((Inline::Pipe, 1))
        } else {
            None
        };
        match marker {
            Some((tok, len)) => {
                if start < i {
                    out.push(Inline::Text(&s[start..i]));
                }
                out.push(tok);
                i += len;
                start = i;
            }
            None => i += 1,
        }
    }
    if start < b.len() {
        out.push(Inline::Text(&s[start..]));
    }
    out
}

/// How to handle a `<…>` span the inline tokenizer recognizes.
enum TagSpan<'a> {
    /// Drop the span entirely (comment, `<ref>…</ref>`).
    Drop(usize),
    /// Keep this inner text, then skip to the offset (`<nowiki>…</nowiki>`).
    Keep(&'a str, usize),
    /// Skip just this tag (a transparent formatting tag); inner content flows on.
    SkipTag(usize),
    /// Skip this tag and emit a space (a void element like `<br>`).
    Space(usize),
}

/// How the engine treats an HTML tag, by lowercased name.
pub(crate) enum TagKind {
    Ref,
    Nowiki,
    /// Inline formatting (`<b>`, `<span>`, …) and presentational block containers
    /// (`<div>`, `<center>`, `<blockquote>`, `<p>`): drop the tag, keep the inner
    /// text. These carry no text semantics we'd lose by unwrapping them.
    Transparent,
    /// Void element (`<br>`, `<hr>`): a word/line break in plain text.
    Void,
    /// Structural/unknown (`<table>`, `<ul>`, unknown tags): out of range → diagnostic.
    Unsupported,
}

/// Classify an HTML tag name (lowercased).
pub(crate) fn tag_kind(name_lower: &str) -> TagKind {
    match name_lower {
        "ref" => TagKind::Ref,
        "nowiki" => TagKind::Nowiki,
        "br" | "hr" | "wbr" => TagKind::Void,
        // Inline formatting.
        "b" | "i" | "em" | "strong" | "span" | "code" | "tt" | "small" | "big" | "sub" | "sup"
        | "u" | "s" | "strike" | "del" | "ins" | "abbr" | "cite" | "q" | "var" | "kbd" | "samp"
        | "mark" | "dfn" | "bdi" | "bdo" | "time" | "data" | "font"
        // Presentational block containers: no text semantics → unwrap to text.
        | "center" | "div" | "blockquote" | "p" => TagKind::Transparent,
        _ => TagKind::Unsupported,
    }
}

/// At a `<` (offset `i`): classify the tag and say how to handle it. Comment/ref
/// dropped, nowiki inner kept, formatting tags skipped, `<br>`→space; structural
/// or unknown tags return `None` so the block-level check reports them.
fn tag_span(s: &str, i: usize) -> Option<TagSpan<'_>> {
    let rest = &s[i..];
    if rest.starts_with("<!--") {
        let end = rest.find("-->").map_or(s.len(), |j| i + j + 3);
        return Some(TagSpan::Drop(end));
    }
    let b = rest.as_bytes();
    let mut j = 1;
    if b.get(j) == Some(&b'/') {
        j += 1;
    }
    let name_start = j;
    while j < b.len() && b[j].is_ascii_alphabetic() {
        j += 1;
    }
    if j == name_start {
        return None;
    }
    match tag_kind(&rest[name_start..j].to_ascii_lowercase()) {
        TagKind::Ref => ref_end(rest).map(|e| TagSpan::Drop(i + e)),
        TagKind::Nowiki => nowiki_span(rest).map(|(inner, e)| TagSpan::Keep(inner, i + e)),
        TagKind::Transparent => tag_close(rest, j).map(|e| TagSpan::SkipTag(i + e)),
        TagKind::Void => tag_close(rest, j).map(|e| TagSpan::Space(i + e)),
        TagKind::Unsupported => None,
    }
}

/// From `rest` (which starts at `<`), with `j` past the tag name, return the
/// offset within `rest` just past the tag's closing `>`. Skips any `>` inside a
/// quoted attribute value (`<div style="a>b">` closes at the second `>`, not the
/// first) — without this, a transparent tag with a complex attribute would mangle
/// its body. `None` if the tag is never closed.
fn tag_close(rest: &str, j: usize) -> Option<usize> {
    let b = rest.as_bytes();
    let mut k = j;
    let mut quote = 0u8; // 0 = outside quotes, else the open quote byte
    while k < b.len() {
        match b[k] {
            q if q == quote => quote = 0,
            b'"' | b'\'' if quote == 0 => quote = b[k],
            b'>' if quote == 0 => return Some(k + 1),
            _ => {}
        }
        k += 1;
    }
    None
}

/// `rest` starts with `<`. If it opens a `<ref …>`/`<ref … />`, return the
/// offset within `rest` just past the whole element.
fn ref_end(rest: &str) -> Option<usize> {
    let b = rest.as_bytes();
    if b.len() < 4 || !b[1..4].eq_ignore_ascii_case(b"ref") {
        return None;
    }
    if !matches!(b.get(4), Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')) {
        return None;
    }
    let gt = rest.find('>')?;
    if rest[..gt].trim_end().ends_with('/') {
        return Some(gt + 1);
    }
    match find_ci(&rest[gt + 1..], "</ref>") {
        Some(c) => Some(gt + 1 + c + "</ref>".len()),
        None => Some(rest.len()),
    }
}

/// `rest` starts with `<`. If it opens `<nowiki>`, return (inner text, offset
/// within `rest` just past `</nowiki>`).
fn nowiki_span(rest: &str) -> Option<(&str, usize)> {
    const OPEN: &str = "<nowiki>";
    const CLOSE: &str = "</nowiki>";
    let b = rest.as_bytes();
    if b.len() < OPEN.len() || !b[..OPEN.len()].eq_ignore_ascii_case(OPEN.as_bytes()) {
        return None;
    }
    let inner = &rest[OPEN.len()..];
    match find_ci(inner, CLOSE) {
        Some(c) => Some((&inner[..c], OPEN.len() + c + CLOSE.len())),
        None => Some((inner, rest.len())),
    }
}

/// Case-insensitive substring search; `needle_lower` must be ASCII-lowercase.
fn find_ci(haystack: &str, needle_lower: &str) -> Option<usize> {
    let (h, n) = (haystack.as_bytes(), needle_lower.as_bytes());
    if n.is_empty() {
        return Some(0);
    }
    if h.len() < n.len() {
        return None;
    }
    (0..=h.len() - n.len()).find(|&i| {
        h[i..i + n.len()]
            .iter()
            .zip(n)
            .all(|(&c, &m)| c.to_ascii_lowercase() == m)
    })
}

/// Brace-match an inline template from `{{` at offset `i`; return the offset
/// just past the matching `}}` (nesting-aware), or `None` if unterminated.
fn template_end(s: &str, i: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut depth = 0usize;
    let mut j = i;
    while j + 1 < b.len() {
        if b[j] == b'{' && b[j + 1] == b'{' {
            depth += 1;
            j += 2;
        } else if b[j] == b'}' && b[j + 1] == b'}' {
            depth -= 1;
            j += 2;
            if depth == 0 {
                return Some(j);
            }
        } else {
            j += 1;
        }
    }
    None
}

/// Whether `s` begins with a URL scheme that starts an external link.
fn is_ext_scheme(s: &str) -> bool {
    const SCHEMES: [&str; 5] = ["http://", "https://", "ftp://", "mailto:", "//"];
    SCHEMES.iter().any(|p| s.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::Inline::*;
    use super::*;

    #[test]
    fn tokenizes_markers_and_text() {
        assert_eq!(
            inline("a '''b''' c"),
            vec![Text("a "), Bold, Text("b"), Bold, Text(" c")]
        );
        assert_eq!(
            inline("[[X|y]]"),
            vec![LinkOpen, Text("X"), Pipe, Text("y"), LinkClose]
        );
        assert_eq!(inline("''i''"), vec![Italic, Text("i"), Italic]);
        assert_eq!(inline("plain"), vec![Text("plain")]);
        assert_eq!(
            inline("[http://x lbl]"),
            vec![ExtOpen, Text("http://x lbl"), ExtClose]
        );
        // <ref> and comments drop; <nowiki> keeps inner text literally
        assert_eq!(inline("a<ref>x</ref>b"), vec![Text("a"), Text("b")]);
        assert_eq!(inline("a<!-- c -->b"), vec![Text("a"), Text("b")]);
        assert_eq!(
            inline("a<nowiki>[[x]]</nowiki>b"),
            vec![Text("a"), Text("[[x]]"), Text("b")]
        );
        // transparent formatting tags drop (inner flows); <br> → space
        assert_eq!(inline("<b>x</b>"), vec![Text("x")]);
        assert_eq!(inline("a<br>b"), vec![Text("a"), Text(" "), Text("b")]);
        assert_eq!(inline("<span>'''y'''</span>"), vec![Bold, Text("y"), Bold]);
        // inline templates are dropped (nesting-aware), prose around them kept
        assert_eq!(inline("a{{t|x}}b"), vec![Text("a"), Text("b")]);
        assert_eq!(inline("a{{o{{i}}}}b"), vec![Text("a"), Text("b")]);
    }
}
