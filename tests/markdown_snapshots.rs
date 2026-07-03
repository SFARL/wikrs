//! Stage 3 M-line human-eye layer: lock the markdown rendering of the same
//! representative wikitext the parse/strip snapshots use. The round-trip
//! harness proves the output means what the AST says; this snapshot lets a
//! human see that it also *reads* well.

use wikrs::{parser, render};

#[test]
fn markdown_path_snapshot() {
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
    insta::assert_snapshot!(render::markdown(&parsed.nodes));
}
