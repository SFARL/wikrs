//! Remove HTML comments, `<ref>` citations, and `<nowiki>` wrappers.

/// Remove `<!-- … -->`, `<ref …>…</ref>` / `<ref … />`, and `<nowiki>…</nowiki>`
/// (keeping nowiki's inner text). Tag matching is case-insensitive.
pub fn strip_comments_refs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(after_open) = tail.strip_prefix("<!--") {
            rest = match after_open.find("-->") {
                Some(j) => &after_open[j + 3..],
                None => "", // unterminated comment: drop the remainder
            };
        } else if let Some(after) = skip_ref(tail) {
            rest = after;
        } else if let Some((inner, after)) = nowiki(tail) {
            out.push_str(inner);
            rest = after;
        } else {
            // a lone '<' that starts nothing we strip — keep it and move on
            out.push('<');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// `tail` starts with `<`. If it opens a `<ref …>`/`<ref … />`, return the slice
/// after the whole element (which is dropped); otherwise `None`.
fn skip_ref(tail: &str) -> Option<&str> {
    let b = tail.as_bytes();
    if b.len() < 4 || !b[1..4].eq_ignore_ascii_case(b"ref") {
        return None;
    }
    // require a boundary after "ref" so we don't match e.g. "<references>"
    match b.get(4).copied() {
        Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/') => {}
        _ => return None,
    }
    // quote-aware close scan shared with the tokenizer: a `>` inside a quoted
    // attribute value must not end the tag (else the ` B` after
    // `<ref name="a>b" />` is swallowed to the next </ref> — or to EOF).
    let (gt, self_closing) = crate::tokenizer::tag_open_end(tail, 4)?;
    if self_closing {
        return Some(&tail[gt..]);
    }
    match find_ci(&tail[gt..], "</ref>") {
        Some(c) => Some(&tail[gt + c + "</ref>".len()..]),
        None => Some(""), // unterminated: drop the remainder
    }
}

/// `tail` starts with `<`. If it opens `<nowiki>`, return (inner text, slice after).
fn nowiki(tail: &str) -> Option<(&str, &str)> {
    const OPEN: &str = "<nowiki>";
    const CLOSE: &str = "</nowiki>";
    let b = tail.as_bytes();
    if b.len() < OPEN.len() || !b[..OPEN.len()].eq_ignore_ascii_case(OPEN.as_bytes()) {
        return None;
    }
    let inner = &tail[OPEN.len()..];
    match find_ci(inner, CLOSE) {
        Some(c) => Some((&inner[..c], &inner[c + CLOSE.len()..])),
        None => Some((inner, "")),
    }
}

/// Case-insensitive substring search. `needle_lower` must be ASCII-lowercase.
/// No allocation; needles here are tiny (`</ref>`, `</nowiki>`).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_comments_refs_nowiki() {
        assert_eq!(strip_comments_refs("a<!-- x -->b"), "ab");
        assert_eq!(strip_comments_refs("a<ref name=q>cite</ref>b"), "ab");
        assert_eq!(strip_comments_refs("a<ref name=q />b"), "ab");
        assert_eq!(strip_comments_refs("a<nowiki>[[x]]</nowiki>b"), "a[[x]]b");
    }

    #[test]
    fn quoted_gt_in_ref_attr_does_not_swallow_tail() {
        // The `>` inside a quoted attribute value is not the tag close. A naive
        // `find('>')` saw `<ref name="a` as a non-self-closing open ref and
        // swallowed everything to the next `</ref>` — or to the END OF THE PAGE
        // when there is none. B must survive.
        assert_eq!(strip_comments_refs(r#"A <ref name="a>b" /> B"#), "A  B");
        assert_eq!(
            strip_comments_refs(r#"A <ref name="a>b">cite</ref> B"#),
            "A  B"
        );
    }

    #[test]
    fn case_insensitive_and_keeps_lone_lt() {
        assert_eq!(strip_comments_refs("a<REF>x</Ref>b"), "ab");
        assert_eq!(strip_comments_refs("2 < 3 is true"), "2 < 3 is true");
        assert_eq!(
            strip_comments_refs("see <references /> here"),
            "see <references /> here"
        );
    }
}
