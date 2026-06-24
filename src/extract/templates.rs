//! Drop templates `{{…}}` and tables `{|…|}`, honoring nesting.

/// Remove `{{…}}` and `{|…|}` regions, honoring nesting. Lossy by design — this
/// is Stage 1, not a parser. Copies UTF-8 *slices* (never single bytes) so
/// multibyte text is preserved; ASCII delimiters never collide with UTF-8
/// continuation bytes.
pub fn strip_templates_tables(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut depth = 0usize;
    let mut seg_start = 0usize;
    while i + 1 < b.len() {
        let open = b[i] == b'{' && (b[i + 1] == b'{' || b[i + 1] == b'|');
        let close = (b[i] == b'}' || b[i] == b'|') && b[i + 1] == b'}';
        if open {
            if depth == 0 {
                out.push_str(&s[seg_start..i]);
            }
            depth += 1;
            i += 2;
        } else if close && depth > 0 {
            depth -= 1;
            i += 2;
            if depth == 0 {
                seg_start = i;
            }
        } else {
            i += 1;
        }
    }
    if depth == 0 {
        out.push_str(&s[seg_start..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_templates_and_tables() {
        assert_eq!(strip_templates_tables("a{{cite|x}}b"), "ab");
        assert_eq!(strip_templates_tables("a{{outer{{inner}}}}b"), "ab");
        assert_eq!(strip_templates_tables("a{|\n|r\n|}b"), "ab");
        assert_eq!(strip_templates_tables("plain text"), "plain text");
        assert_eq!(strip_templates_tables("café {{t}} ☕"), "café  ☕"); // UTF-8 safe
    }
}
