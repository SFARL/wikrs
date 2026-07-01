//! Safety net (TESTING.md layer 3): the extractor (`extract::strip`) AND the
//! Stage-2 AST parser (`parser::parse`, the CLI default) must never panic, hang,
//! or blow up on pathological input — and must stay roughly *linear* (a 2 MB
//! adversarial input finishes fast, not in quadratic time). CI-runnable
//! counterpart to the `cargo fuzz` targets, which need nightly.

use wikrs::extract::strip;
use wikrs::parser::parse;

#[test]
fn does_not_panic_on_adversarial_input() {
    let cases = [
        "{{".repeat(100_000),    // unbalanced open templates
        "}}".repeat(100_000),    // unbalanced close
        "[[".repeat(100_000),    // unterminated links
        "<ref>".repeat(100_000), // unterminated refs
        "<!--".repeat(100_000),  // unterminated comments
        "{|".repeat(100_000),    // unterminated tables
        "'".repeat(200_000),     // emphasis runs
        "=".repeat(200_000),     // heading markers
        "<".repeat(200_000),     // lone angle brackets
        "[[a|".repeat(50_000),   // half-open piped links
        "café☕".repeat(50_000), // multibyte, no markup
    ];
    for c in &cases {
        let _ = strip(c); // must simply return
    }
}

#[test]
fn stays_linear_on_2mb_input() {
    let chunk = "{{t|[[L|x]]}}'''b''' <ref>r</ref> == H ==\n* item\n";
    let big = chunk.repeat(2_000_000 / chunk.len());
    let start = std::time::Instant::now();
    let _ = strip(&big);
    let elapsed = start.elapsed();
    // Linear strip handles 2 MB in well under a second; 10 s catches a
    // quadratic regression with enormous margin (even on a loaded CI box).
    assert!(
        elapsed.as_secs() < 10,
        "strip too slow on 2 MB: {elapsed:?}"
    );
}

#[test]
fn parser_does_not_panic_on_adversarial_input() {
    // The AST parser (the CLI default) must be as robust as strip on malformed
    // input — never panic, hang, or blow up.
    let cases = [
        "{{".repeat(100_000),                  // unbalanced open templates
        "}}".repeat(100_000),                  // unbalanced close
        "[[".repeat(100_000),                  // unterminated links
        "<ref>".repeat(100_000),               // unterminated refs
        "<!--".repeat(100_000),                // unterminated comments
        "{|".repeat(100_000),                  // unterminated tables
        "{{\n\n".repeat(50_000), // open templates + blank lines (brace-aware split path)
        "{|\n| x\n".repeat(50_000), // unterminated table flood (table-depth split path)
        "'".repeat(200_000),     // emphasis runs
        "=".repeat(200_000),     // heading markers
        "<".repeat(200_000),     // lone angle brackets
        "[[a|".repeat(50_000),   // half-open piped links
        "café☕".repeat(50_000), // multibyte, no markup
        "| c || d\n".repeat(100_000), // a flood of table-cell lines
        "* item\n".repeat(100_000), // a flood of flat list items
        "[[File:a|[[x]]".repeat(50_000), // nested media, unbalanced outer ]] (depth-match table path)
        "[[File:a|[[x]] cap]]".repeat(50_000), // balanced nested media captions
    ];
    let start = std::time::Instant::now();
    for c in &cases {
        let _ = parse(c); // must simply return
    }
    // Linear: every case finishes fast. A quadratic regression (re-scanning an
    // unbalanced `'`/`[[`/`{{` run) would blow past this by orders of magnitude.
    assert!(
        start.elapsed().as_secs() < 30,
        "parser too slow on adversarial input: {:?}",
        start.elapsed()
    );
}

#[test]
fn parser_survives_deeply_nested_links() {
    // Balanced nested links drive `parse_inline`/`make_link` recursion (one level
    // per nesting); a deep one must not overflow the stack. Linear-size input.
    let deep = format!("{}{}", "[[a|".repeat(50_000), "]]".repeat(50_000));
    let _ = parse(&deep);
}

#[test]
fn parser_stays_linear_on_2mb_input() {
    let chunk = "{{t|[[L|x]]}}'''b''' <ref>r</ref> == H ==\n* item\n| c || d\n";
    let big = chunk.repeat(2_000_000 / chunk.len());
    let start = std::time::Instant::now();
    let _ = parse(&big);
    assert!(start.elapsed().as_secs() < 10, "parse too slow on 2 MB");
}
