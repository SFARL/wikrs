//! Offline integration smoke test for the layer-2 differential pipeline
//! (`docs/TESTING.md` layer 2). It runs real wikitext through the same path
//! `xtask diff-report` uses — `parser::parse` -> `render::plain` ->
//! `diff::classify` — against hand-written ground-truth prose, so CI verifies the
//! end-to-end bucketing WITHOUT network. The full N-page run is
//! `cargo xtask diff-report` (manual / scheduled, per TESTING.md).

use wikrs::diag::Severity;
use wikrs::diff::{self, Bucket};

/// Render a page the way `diff-report` does, and report whether wikrs flagged
/// anything `Unsupported`.
fn run(wikitext: &str) -> (String, bool) {
    let parsed = wikrs::parser::parse(wikitext);
    let text = wikrs::render::plain(&parsed.nodes);
    let has_unsupported = parsed
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Unsupported);
    (text, has_unsupported)
}

#[test]
fn clean_prose_is_faithful_against_superset_truth() {
    // wikrs emits clean prose (bold stripped, links flattened); the ground truth
    // is a superset with an extra, template-expanded sentence. The page is
    // Faithful — never penalized for the content wikrs omits by design.
    let (text, unsupported) = run("The '''quick''' brown [[fox]] jumps over the lazy dog. \
         It was a [[bright]] cold day in April.");
    assert!(!unsupported, "clean prose should not be flagged");
    let truth = "The quick brown fox jumps over the lazy dog. It was a bright cold \
                 day in April. Foxes are small carnivorous mammals.";
    assert_eq!(diff::classify(&text, truth, unsupported), Bucket::Faithful);
}

#[test]
fn unsupported_construct_lands_in_reported() {
    // An HTML table is out of declared range -> U-HTML (Unsupported) -> Reported,
    // regardless of how the fallback text compares. Honesty precedence.
    let (text, unsupported) = run("<table><tr><td>cell</td></tr></table>");
    assert!(unsupported, "an HTML table should be flagged Unsupported");
    assert_eq!(diff::classify(&text, "cell", unsupported), Bucket::Reported);
}

#[test]
fn silent_disagreement_is_divergent() {
    // Clean parse, no diagnostic, but the emitted prose is absent from the
    // ground truth: the silent-error bucket the project exists to drive to zero.
    let (text, unsupported) = run("Paris is the capital of France and a major city.");
    assert!(!unsupported);
    let truth = "Berlin is the largest city and capital of Germany.";
    assert_eq!(diff::classify(&text, truth, unsupported), Bucket::Divergent);
}
