//! Reduce links to their human-visible text.

/// Which kind of link starts next, and where.
enum Hit {
    Internal(usize),
    External(usize),
}

/// `[[Target|text]]`→`text`, `[[Target]]`→`Target`, `[[File:…]]`/`[[Image:…]]`→
/// dropped; `[url text]`→`text`, `[url]`→dropped.
pub fn strip_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        // Pick whichever link starts first (external only if strictly earlier).
        let hit = match (rest.find("[["), find_external(rest)) {
            (Some(i), Some(e)) if e < i => Hit::External(e),
            (Some(i), _) => Hit::Internal(i),
            (None, Some(e)) => Hit::External(e),
            (None, None) => {
                out.push_str(rest);
                break;
            }
        };
        match hit {
            Hit::Internal(i) => {
                out.push_str(&rest[..i]);
                match rest[i + 2..].find("]]") {
                    Some(j) => {
                        out.push_str(&internal_text(&rest[i + 2..i + 2 + j]));
                        rest = &rest[i + 2 + j + 2..];
                    }
                    None => rest = "", // unterminated: drop the remainder
                }
            }
            Hit::External(e) => {
                out.push_str(&rest[..e]);
                match rest[e + 1..].find(']') {
                    Some(j) => {
                        out.push_str(external_text(&rest[e + 1..e + 1 + j]));
                        rest = &rest[e + 1 + j + 1..];
                    }
                    None => rest = "",
                }
            }
        }
    }
    out
}

/// Index of a `[` that opens an external link `[http…]` (not `[[`).
fn find_external(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    (0..b.len())
        .find(|&i| b[i] == b'[' && b.get(i + 1) != Some(&b'[') && s[i + 1..].starts_with("http"))
}

/// Inner text of `[[ … ]]`. Drops `File:`/`Image:`/`Category:` (non-prose);
/// otherwise keeps the text after the last `|`, or the target if there's no `|`.
fn internal_text(inner: &str) -> String {
    let target = inner.split('|').next().unwrap_or("");
    let ns = target
        .split(':')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if matches!(ns.as_str(), "file" | "image" | "category") {
        return String::new();
    }
    inner.rsplit('|').next().unwrap_or(inner).to_string()
}

/// Inner text of `[url text]`: the part after the first whitespace, else "".
fn external_text(inner: &str) -> &str {
    match inner.split_once(char::is_whitespace) {
        Some((_, text)) => text,
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_anchor_text() {
        assert_eq!(
            strip_links("see [[Earth|our planet]] now"),
            "see our planet now"
        );
        assert_eq!(strip_links("see [[Earth]] now"), "see Earth now");
        assert_eq!(strip_links("x [https://a.com label] y"), "x label y");
        assert_eq!(strip_links("x [https://a.com] y"), "x  y"); // bare url dropped
        assert_eq!(strip_links("a [[File:p.jpg|thumb|cap]] b"), "a  b"); // file dropped
        assert_eq!(strip_links("p [[Category:Living people]] q"), "p  q"); // category dropped
    }
}
