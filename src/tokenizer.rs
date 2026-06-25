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
    }
}
