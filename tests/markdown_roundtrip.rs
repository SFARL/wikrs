//! Stage 3 M-line round-trip harness (M1). For every input AST:
//! `mdnorm::from_ast(ast)` must equal `pulldown_nf(render::markdown(ast))`.
//! The renderer's output is judged by an INDEPENDENT GFM implementation —
//! the renderer cannot grade its own homework. Contract table:
//! `docs/superpowers/plans/2026-07-02-markdown-roundtrip.md` §0.

#[path = "support/pulldown_nf.rs"]
mod pulldown_nf;

use std::borrow::Cow;
use std::path::Path;
use wikrs::ast::Node;
use wikrs::{mdnorm, parser, render};

fn check(label: &str, nodes: &[Node]) -> Result<(), String> {
    let intent = mdnorm::from_ast(nodes);
    let md = render::markdown(nodes);
    let md_for_panic = md.clone();
    let actual = std::panic::catch_unwind(move || pulldown_nf::markdown_to_nf(&md_for_panic))
        .map_err(|_| format!("[{label}] pulldown_nf panicked on our markdown:\n{md}"))?;
    if intent == actual {
        Ok(())
    } else {
        Err(format!(
            "[{label}] round-trip mismatch\n--- markdown ---\n{md}\n--- intent ---\n{intent:#?}\n--- actual ---\n{actual:#?}"
        ))
    }
}

fn check_wikitext(label: &str, wt: &str) -> Result<(), String> {
    check(label, &parser::parse(wt).nodes)
}

fn text(s: &str) -> Node<'static> {
    Node::Text(Cow::Owned(s.to_string()))
}

#[test]
fn hand_built_cases_roundtrip() {
    let cases: Vec<(&str, Vec<Node>)> = vec![
        ("para", vec![Node::Paragraph(vec![text("plain prose")])]),
        (
            "escaping bomb",
            vec![Node::Paragraph(vec![text(
                "a*b _c_ [d] <e> `f` 1. g # h & i | j \\k",
            )])],
        ),
        (
            "heading level",
            vec![Node::Heading {
                level: 2,
                content: vec![text("History")],
            }],
        ),
        (
            "bold italic nesting",
            vec![Node::Paragraph(vec![
                Node::Bold(vec![text("two")]),
                text(" moons "),
                Node::Italic(vec![Node::Bold(vec![text("Alpha")])]),
            ])],
        ),
        (
            "links",
            vec![Node::Paragraph(vec![
                Node::Link {
                    target: Cow::Borrowed("terrestrial planet"),
                    label: vec![text("planet")],
                },
                text(" and "),
                Node::Link {
                    target: Cow::Borrowed("https://e.org/a?b=1&c=(2)"),
                    label: vec![],
                },
            ])],
        ),
        (
            "nested list",
            vec![Node::List {
                ordered: false,
                items: vec![
                    vec![
                        text("a"),
                        Node::List {
                            ordered: true,
                            items: vec![vec![text("b")]],
                        },
                    ],
                    vec![text("c")],
                ],
            }],
        ),
        (
            "pre",
            vec![Node::Preformatted(vec![
                vec![text("line<1")],
                vec![text("line2")],
            ])],
        ),
        (
            "unsupported fence",
            vec![Node::Unsupported(Cow::Borrowed("{{Infobox|x=```\ny}}"))],
        ),
        (
            "table",
            vec![Node::Table {
                rows: vec![
                    vec![vec![text("Property")], vec![text("Va|ue")]],
                    vec![vec![text("Radius")], vec![text("6,051 km")]],
                ],
            }],
        ),
    ];
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|(label, nodes)| check(label, nodes).err())
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} hand cases failed:\n\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n\n")
    );
}

#[test]
fn sample_article_roundtrips() {
    let wt = std::fs::read_to_string("tests/fixtures/sample_article.wikitext").unwrap();
    if let Err(e) = check_wikitext("sample_article", &wt) {
        panic!("{e}");
    }
}

/// Every parserTests wikitext (GPL fixture, fetched at test time; soft-skip
/// when missing — same policy as tests/parser_tests.rs).
#[test]
fn parser_tests_corpus_roundtrips() {
    const FIXTURE: &str = "tests/fixtures/parserTests.txt";
    if !Path::new(FIXTURE).exists() {
        eprintln!("SKIP: {FIXTURE} missing — run `cargo xtask fetch-parser-tests`.");
        return;
    }
    let input = std::fs::read_to_string(FIXTURE).unwrap();
    let mut failures = Vec::new();
    let mut total = 0usize;
    for (i, wt) in extract_wikitext_sections(&input).iter().enumerate() {
        total += 1;
        if let Err(e) = check_wikitext(&format!("case #{i}"), wt) {
            failures.push(e);
        }
    }
    eprintln!(
        "markdown round-trip over parserTests: {}/{total} ok",
        total - failures.len()
    );
    assert!(
        failures.is_empty(),
        "{}/{} parserTests inputs failed round-trip; first 5:\n\n{}",
        failures.len(),
        total,
        failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

/// Tiny format reader: `!! test`…`!! end` blocks, `!! wikitext` section body.
/// (The full-fidelity format parser lives in tests/parser_tests.rs; this
/// harness only needs the raw wikitext bodies.)
fn extract_wikitext_sections(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut in_test, mut in_wt) = (false, false);
    let mut cur = String::new();
    for line in input.lines() {
        let tag = line.strip_prefix("!!").map(str::trim);
        match tag {
            Some("test") => in_test = true,
            Some("end") => {
                if in_test && !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                in_test = false;
                in_wt = false;
            }
            Some("wikitext") if in_test => {
                in_wt = true;
                cur.clear();
            }
            Some(t)
                if in_test
                    && (t.starts_with("html")
                        || matches!(t, "options" | "metadata" | "config" | "wikitext/edited")) =>
            {
                in_wt = false;
            }
            _ if in_wt => {
                if !cur.is_empty() {
                    cur.push('\n');
                }
                cur.push_str(line);
            }
            _ => {}
        }
    }
    out
}
