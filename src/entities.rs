//! HTML entity decoding for the text wikrs emits.
//!
//! Wikitext prose is littered with character entities (`&nbsp;`, `&amp;`,
//! `&eacute;`, numeric `&#160;`). The extractor used to pass them through
//! verbatim — so `9.02&nbsp;AU` came out as the literal word "nbsp" glued to the
//! number, which the layer-2 differential flagged as un-corroborated output. We
//! decode the common set here, where text is rendered. Unknown or malformed
//! entities are left literal, so `AT&T` stays `AT&T`.

use std::borrow::Cow;

/// Decode the HTML entities common in Wikipedia prose. Fast path: text with no
/// `&` is returned borrowed, untouched.
pub fn decode(s: &str) -> Cow<'_, str> {
    if !s.contains('&') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let at = &rest[amp..];
        match decode_entity(at) {
            Some((ch, len)) => {
                out.push(ch);
                rest = &at[len..];
            }
            None => {
                out.push('&');
                rest = &at[1..];
            }
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// `s` starts with `&`. Returns the decoded char and the byte length consumed
/// (through the closing `;`), or `None` if it isn't a recognized entity.
fn decode_entity(s: &str) -> Option<(char, usize)> {
    let semi = s[1..].find(';')? + 1; // index of ';' within s
    let body = &s[1..semi];
    if body.is_empty() || body.len() > 10 {
        return None;
    }
    let ch = match body.strip_prefix('#') {
        Some(num) => {
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse::<u32>().ok()?,
            };
            char::from_u32(code)?
        }
        None => named(body)?,
    };
    // Normalize the non-breaking space to a plain space — cleaner for the text
    // wikrs emits, and what downstream NLP expects.
    let ch = if ch == '\u{00A0}' { ' ' } else { ch };
    Some((ch, semi + 1))
}

/// The named entities common in Wikipedia prose. Unknown names return `None` and
/// are left literal.
fn named(name: &str) -> Option<char> {
    Some(match name {
        "nbsp" => '\u{00A0}', // normalized to a plain space by `decode_entity`
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "ndash" => '\u{2013}',
        "mdash" => '\u{2014}',
        "minus" => '\u{2212}',
        "hellip" => '\u{2026}',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "ldquo" => '\u{201C}',
        "rdquo" => '\u{201D}',
        "times" => '\u{00D7}',
        "deg" => '\u{00B0}',
        "middot" => '\u{00B7}',
        "bull" => '\u{2022}',
        "prime" => '\u{2032}',
        "Prime" => '\u{2033}',
        "frac12" => '\u{00BD}',
        "frac14" => '\u{00BC}',
        "frac34" => '\u{00BE}',
        "copy" => '\u{00A9}',
        "reg" => '\u{00AE}',
        "trade" => '\u{2122}',
        "eacute" => '\u{00E9}',
        "egrave" => '\u{00E8}',
        "agrave" => '\u{00E0}',
        "uuml" => '\u{00FC}',
        "ouml" => '\u{00F6}',
        "auml" => '\u{00E4}',
        "szlig" => '\u{00DF}',
        "ntilde" => '\u{00F1}',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_ampersand_is_borrowed_untouched() {
        assert!(matches!(decode("plain text"), Cow::Borrowed("plain text")));
    }

    #[test]
    fn decodes_named_entities() {
        assert_eq!(decode("9.02&nbsp;AU"), "9.02 AU"); // nbsp -> plain space
        assert_eq!(decode("AT&amp;T"), "AT&T");
        assert_eq!(decode("&lt;div&gt;"), "<div>");
        assert_eq!(decode("caf&eacute;"), "café");
        assert_eq!(decode("5&nbsp;&ndash;&nbsp;10"), "5 \u{2013} 10");
    }

    #[test]
    fn decodes_numeric_entities() {
        assert_eq!(decode("&#65;&#x42;"), "AB");
        assert_eq!(decode("&#160;"), " "); // numeric nbsp normalized to space too
    }

    #[test]
    fn leaves_unknown_and_malformed_literal() {
        assert_eq!(decode("Tom & Jerry"), "Tom & Jerry"); // bare & kept
        assert_eq!(decode("a&bogus;b"), "a&bogus;b"); // unknown name kept
        assert_eq!(decode("R&D"), "R&D"); // no ';' kept
        assert_eq!(decode("&#999999999;"), "&#999999999;"); // out-of-range kept literal
    }
}
