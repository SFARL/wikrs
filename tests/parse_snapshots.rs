//! Stage 2 AST-path snapshot: parse representative wikitext, render it to plain
//! text, and list the diagnostic codes. Shows the honest current behavior —
//! supported constructs render; out-of-range ones become `Unsupported` (dropped
//! from plain text, surfaced as diagnostics). The snapshot shifts (review with
//! `cargo insta review`) as the supported subset grows.

use wikrs::{parser, render};

#[test]
fn parse_path_snapshot() {
    let wikitext = "\
'''Earth''' is the [[Planet|third planet]] from the Sun.

== History ==

Formed long ago. See [https://example.org/overview the overview].

* accretion of [[planetesimal]]s
* differentiation into a [[planetary core|core]]

The planet {{convert|6051|km}} has '''two''' moons.{{citation needed}}

{| class=\"wikitable\"
! Property !! Value
|}";
    let parsed = parser::parse(wikitext);
    let codes: Vec<&str> = parsed.diagnostics.iter().map(|d| d.code).collect();
    let report = format!(
        "=== render::plain ===\n{}\n\n=== diagnostics: {codes:?} ===",
        render::plain(&parsed.nodes),
    );
    insta::assert_snapshot!(report);
}
