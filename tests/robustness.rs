//! Safety net (TESTING.md layer 3): `extract::strip` must never panic, hang, or
//! blow up on pathological input — and must stay roughly *linear* (a 2 MB
//! adversarial input must finish fast, not in quadratic time). This is the
//! CI-runnable counterpart to the `cargo fuzz` target, which needs nightly.

use wikrs::extract::strip;

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
