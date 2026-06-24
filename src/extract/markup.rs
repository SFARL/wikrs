//! Strip headings, bold/italic, list markers, and any leftover HTML tags.

/// Per line: unwrap headings (`== H ==` → `H`), drop leading list markers
/// (`* # : ;`), remove bold/italic apostrophes, and strip any remaining
/// `<tag>`/`</tag>`.
pub fn strip_markup(s: &str) -> String {
    s.split('\n').map(strip_line).collect::<Vec<_>>().join("\n")
}

fn strip_line(line: &str) -> String {
    let trimmed = line.trim_end().trim_start();
    let body = if trimmed.len() > 2 && trimmed.starts_with('=') && trimmed.ends_with('=') {
        trimmed.trim_matches('=').trim()
    } else {
        trimmed.trim_start_matches(['*', '#', ':', ';', ' '])
    };
    strip_tags(&strip_emphasis(body))
}

fn strip_emphasis(s: &str) -> String {
    s.replace("'''''", "").replace("'''", "").replace("''", "")
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        rest = match rest[i..].find('>') {
            Some(j) => &rest[i + j + 1..],
            None => {
                out.push_str(&rest[i..]); // lone '<' with no '>': keep it
                return out;
            }
        };
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_inline_and_block_markup() {
        assert_eq!(strip_markup("== History =="), "History");
        assert_eq!(strip_markup("'''bold''' and ''italic''"), "bold and italic");
        assert_eq!(strip_markup("* one\n* two"), "one\ntwo");
        assert_eq!(strip_markup("# a\n## b"), "a\nb");
        assert_eq!(strip_markup("x <b>y</b> z"), "x y z");
    }
}
