//! Stage 1: lossy `wikitext -> plain text` extraction.
//!
//! Deliberately *not* a parser — a fast, targeted stripper that mirrors
//! WikiExtractor's behavior (drop templates/tables/refs, keep link anchor
//! text, strip markup). The real AST engine lands in Stage 2.

mod comments;
mod links;
mod markup;
mod templates;

/// Turn raw wikitext into clean plain text (Stage 1, lossy).
///
/// Pipeline order matters: remove comments/refs first, then templates/tables
/// (so their inner `[[…]]` / `|` never reach later passes), then links, then
/// markup, then collapse runs of blank lines.
pub fn strip(wikitext: &str) -> String {
    let s = comments::strip_comments_refs(wikitext);
    let s = templates::strip_templates_tables(&s);
    let s = links::strip_links(&s);
    let s = markup::strip_markup(&s);
    collapse_blank_lines(&s)
}

/// Collapse runs of blank lines to a single blank line and trim the ends.
fn collapse_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank = 0u32;
    for line in s.lines() {
        if line.trim().is_empty() {
            blank += 1;
            if blank == 1 {
                out.push('\n');
            }
        } else {
            blank = 0;
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

/// Whether stripped output still contains unconverted wikitext markup — the
/// basis of the Stage 1 *conversion rate* (the fraction of pages that come out
/// clean). Honest by design: leftover `{{ }}`, `[[ ]]`, or `{| |}` means a
/// construct we did not handle survived into the supposed "plain text".
pub fn looks_clean(text: &str) -> bool {
    const RESIDUALS: [&str; 6] = ["{{", "}}", "[[", "]]", "{|", "|}"];
    !RESIDUALS.iter().any(|m| text.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_a_small_article() {
        let wt = "'''Hi''' [[World|there]].<ref>x</ref> {{t|a}} end";
        assert_eq!(strip(wt), "Hi there.  end");
    }

    #[test]
    fn looks_clean_flags_residual_markup() {
        assert!(looks_clean("Earth is the third planet."));
        assert!(!looks_clean("Has {{cite}} left over"));
        assert!(!looks_clean("link [[Earth]] survived"));
    }
}
