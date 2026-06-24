//! Serialize an extracted page as plain text or JSON Lines.

use serde::Serialize;

#[derive(Serialize)]
struct Record<'a> {
    title: &'a str,
    text: &'a str,
}

/// One JSON object per line: `{"title":…,"text":…}`.
pub fn to_jsonl(title: &str, text: &str) -> String {
    serde_json::to_string(&Record { title, text }).expect("serialize record")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_has_title_and_text() {
        assert_eq!(
            to_jsonl("Earth", "third planet"),
            r#"{"title":"Earth","text":"third planet"}"#
        );
    }
}
