//! Layer 1 (see `docs/TESTING.md`): MediaWiki `parserTests.txt` conformance harness.
//!
//! The fixture is **GPL** and fetched at test time
//! (`cargo xtask fetch-parser-tests`) — never committed (see DESIGN.md §11).
//! These tests parse the format and load the real fixture now; the per-case
//! conformance comparison is `#[ignore]`d until the engine can render
//! comparable output (Stage 2: AST -> normalized HTML).

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/parserTests.txt";
/// Committed record of which parserTests cases parse with ZERO diagnostics.
/// Names only — derived facts about wikrs, NOT the GPL fixture body — so it is
/// safe to commit (unlike `FIXTURE`, which is gitignored). See `coverage_ratchet`.
const COVERAGE_BASELINE: &str = "tests/coverage_baseline.txt";

/// One `!! test … !! end` case.
#[derive(Debug, Clone)]
struct ParserTest {
    name: String,
    wikitext: String,
    /// Preferred expected HTML: plain `html`, else `html/php`, else `html/parsoid`.
    html: Option<String>,
}

/// Recognized section markers. A `!!` line whose tag is *not* one of these is
/// treated as verbatim content — important because wikitext (e.g. table
/// headers) can itself contain `!!`.
fn is_section_header(tag: &str) -> bool {
    matches!(
        tag,
        "test" | "end" | "wikitext" | "wikitext/edited" | "options" | "metadata" | "config"
    ) || tag.starts_with("html")
}

/// Parse the parserTests.txt format. Ignores `!! article` fixtures and
/// file-level option blocks (anything outside a `!! test … !! end`).
fn parse_tests(input: &str) -> Vec<ParserTest> {
    let mut out = Vec::new();
    let mut in_test = false;
    let mut section = String::new();
    let mut sections: HashMap<String, String> = HashMap::new();

    for line in input.lines() {
        let header = line
            .strip_prefix("!!")
            .map(str::trim)
            .filter(|t| is_section_header(t));
        match header {
            Some("test") => {
                in_test = true;
                section = "name".to_string();
                sections.clear();
            }
            Some("end") if in_test => {
                out.push(finalize(&sections));
                in_test = false;
            }
            Some(tag) if in_test => {
                section = tag.to_string();
                sections.entry(section.clone()).or_default();
            }
            Some(_) => {} // a recognized header outside a test -> ignore
            None => {
                if in_test {
                    let entry = sections.entry(section.clone()).or_default();
                    entry.push_str(line);
                    entry.push('\n');
                }
            }
        }
    }
    out
}

fn finalize(sections: &HashMap<String, String>) -> ParserTest {
    let get = |k: &str| {
        sections
            .get(k)
            .map(|s| s.trim_end_matches('\n').to_string())
    };
    ParserTest {
        name: get("name").unwrap_or_default().trim().to_string(),
        wikitext: get("wikitext").unwrap_or_default(),
        html: get("html")
            .or_else(|| get("html/php"))
            .or_else(|| get("html/parsoid")),
    }
}

#[test]
fn parses_inline_sample() {
    let sample = "\
!! test
Simple paragraph
!! wikitext
This is a simple paragraph.
!! html
<p>This is a simple paragraph.
</p>
!! end

!! article
Template:Foo
!! text
bar
!! endarticle

!! test
Bold
!! options
parsoid
!! wikitext
'''x'''
!! html/php
<p><b>x</b>
</p>
!! end
";
    let tests = parse_tests(sample);
    assert_eq!(tests.len(), 2, "the !! article block must be ignored");
    assert_eq!(tests[0].name, "Simple paragraph");
    assert_eq!(tests[0].wikitext, "This is a simple paragraph.");
    assert_eq!(
        tests[0].html.as_deref(),
        Some("<p>This is a simple paragraph.\n</p>")
    );
    assert_eq!(tests[1].name, "Bold");
    assert_eq!(tests[1].wikitext, "'''x'''");
    assert_eq!(tests[1].html.as_deref(), Some("<p><b>x</b>\n</p>")); // html/php fallback
}

#[test]
fn loads_real_fixture_and_counts_cases() {
    if !Path::new(FIXTURE).exists() {
        eprintln!(
            "SKIP: {FIXTURE} missing — run `cargo xtask fetch-parser-tests` (GPL, not committed)."
        );
        return;
    }
    let text = std::fs::read_to_string(FIXTURE).unwrap();
    let tests = parse_tests(&text);
    eprintln!("parserTests.txt: parsed {} cases", tests.len());
    assert!(
        tests.len() > 500,
        "expected hundreds of cases, got {}",
        tests.len()
    );
    assert!(
        tests.iter().all(|t| !t.name.is_empty()),
        "every parsed case should have a name"
    );
}

/// Stage 1 conversion rate: run `extract::strip` over every real parserTests
/// wikitext snippet and report the fraction that comes out clean (no residual
/// markup). This is the honest "how much can we actually convert" number — it
/// will climb as the extractor handles more constructs.
#[test]
fn stage1_conversion_rate() {
    if !Path::new(FIXTURE).exists() {
        eprintln!("SKIP: {FIXTURE} missing — run `cargo xtask fetch-parser-tests`.");
        return;
    }
    let text = std::fs::read_to_string(FIXTURE).unwrap();
    let tests = parse_tests(&text);
    let total = tests.len();
    let clean = tests
        .iter()
        .filter(|t| wikrs::extract::looks_clean(&wikrs::extract::strip(&t.wikitext)))
        .count();
    let pct = 100.0 * clean as f64 / total as f64;
    eprintln!("Stage 1 clean conversion over parserTests: {clean}/{total} ({pct:.1}%)");
    assert!(total > 500);
    assert!(
        pct > 90.0,
        "clean conversion regressed below floor: {pct:.1}%"
    );
}

/// Stage 2 coverage: the fraction of real parserTests cases that parse with
/// ZERO diagnostics — i.e. entirely within wikrs's declared support range. An
/// honest "how much can we fully handle" number (coverage, **not** correctness;
/// correctness-vs-expected-HTML is the separate Stage 3 conformance metric). It
/// starts low and climbs as the supported subset grows.
#[test]
fn stage2_coverage_rate() {
    if !Path::new(FIXTURE).exists() {
        eprintln!("SKIP: {FIXTURE} missing — run `cargo xtask fetch-parser-tests`.");
        return;
    }
    let text = std::fs::read_to_string(FIXTURE).unwrap();
    let tests = parse_tests(&text);
    let total = tests.len();
    let mut supported = 0;
    let mut hist: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for t in &tests {
        let diags = wikrs::parser::parse(&t.wikitext).diagnostics;
        if diags.is_empty() {
            supported += 1;
        }
        for d in &diags {
            *hist.entry(d.code).or_default() += 1;
        }
    }
    let pct = 100.0 * supported as f64 / total as f64;
    let mut blocking: Vec<_> = hist.into_iter().collect();
    blocking.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
    eprintln!("Stage 2 coverage (zero diagnostics): {supported}/{total} ({pct:.1}%)");
    eprintln!("blocking diagnostics: {blocking:?}");
    assert!(total > 500);
    assert!(pct > 20.0, "Stage 2 coverage regressed: {pct:.1}%");
}

/// Compare the blessed baseline against the current passing set. Returns
/// `(regressed, added)`: names that were in the baseline but no longer pass (a
/// backward-compat break), and names that newly pass but aren't blessed yet.
/// `BTreeSet::difference` yields sorted output, so both lists are deterministic.
fn ratchet_diff(
    baseline: &BTreeSet<String>,
    current: &BTreeSet<String>,
) -> (Vec<String>, Vec<String>) {
    let regressed = baseline.difference(current).cloned().collect();
    let added = current.difference(baseline).cloned().collect();
    (regressed, added)
}

#[test]
fn ratchet_diff_reports_regressions_and_additions() {
    let baseline: BTreeSet<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
    let current: BTreeSet<String> = ["a", "c", "d"].iter().map(|s| s.to_string()).collect();
    let (regressed, added) = ratchet_diff(&baseline, &current);
    assert_eq!(regressed, vec!["b".to_string()], "b dropped out of the set");
    assert_eq!(added, vec!["d".to_string()], "d newly entered the set");
}

/// Names of cases that currently parse with ZERO diagnostics.
fn current_passing_set(tests: &[ParserTest]) -> BTreeSet<String> {
    tests
        .iter()
        .filter(|t| wikrs::parser::parse(&t.wikitext).diagnostics.is_empty())
        .map(|t| t.name.clone())
        .collect()
}

/// Read the committed baseline (skips the `#` header and blank lines).
fn load_baseline() -> BTreeSet<String> {
    std::fs::read_to_string(COVERAGE_BASELINE)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Rewrite the baseline from the current passing set (sorted, with a header).
fn write_baseline(set: &BTreeSet<String>) {
    let mut body = String::from(
        "# wikrs coverage baseline — names of parserTests cases that parse with ZERO\n\
         # diagnostics today. Derived facts about wikrs, NOT the GPL fixture body.\n\
         # The `coverage_ratchet` test fails if any listed case regresses. Re-bless with:\n\
         #   BLESS_COVERAGE=1 cargo test --test parser_tests coverage_ratchet\n",
    );
    for name in set {
        body.push_str(name);
        body.push('\n');
    }
    std::fs::write(COVERAGE_BASELINE, body).unwrap();
}

/// Backward-compatibility ratchet: cases that parse cleanly today must keep
/// parsing cleanly. The committed baseline is the auditable record of *which*
/// parserTests cases pass; this test fails if any of them regresses (a silent
/// coverage drop the single Stage-2 percentage would hide), and also fails if
/// new cases pass without being blessed — so the record stays exact and every
/// coverage change is a deliberate, reviewed baseline diff.
///
/// Re-bless after an intended change: `BLESS_COVERAGE=1 cargo test --test
/// parser_tests coverage_ratchet`.
///
/// Name-keyed: if two cases share a name the set holds one entry — a rare blind
/// spot accepted to keep the baseline a human-readable "which tests pass" list.
#[test]
fn coverage_ratchet() {
    if !Path::new(FIXTURE).exists() {
        eprintln!("SKIP: {FIXTURE} missing — run `cargo xtask fetch-parser-tests`.");
        return;
    }
    let tests = parse_tests(&std::fs::read_to_string(FIXTURE).unwrap());
    let current = current_passing_set(&tests);

    if std::env::var_os("BLESS_COVERAGE").is_some() {
        write_baseline(&current);
        eprintln!(
            "blessed {} passing cases -> {COVERAGE_BASELINE}",
            current.len()
        );
        return;
    }

    let baseline = load_baseline();
    assert!(
        !baseline.is_empty(),
        "{COVERAGE_BASELINE} missing/empty — bless once with \
         `BLESS_COVERAGE=1 cargo test --test parser_tests coverage_ratchet`"
    );
    let (regressed, added) = ratchet_diff(&baseline, &current);
    assert!(
        regressed.is_empty(),
        "BACKWARD-COMPAT REGRESSION: {} case(s) that used to parse cleanly now emit \
         diagnostics:\n  {}\nIf this is intended, re-bless the baseline.",
        regressed.len(),
        regressed.join("\n  "),
    );
    assert!(
        added.is_empty(),
        "COVERAGE IMPROVED: {} new case(s) parse cleanly but aren't blessed yet:\n  {}\n\
         Record them: BLESS_COVERAGE=1 cargo test --test parser_tests coverage_ratchet",
        added.len(),
        added.join("\n  "),
    );
}

/// Per-case conformance against wikrs. Enabled once the engine can produce
/// comparable (normalized HTML) output — Stage 2. Until then it would report
/// ~0% and only add noise, so it is ignored by default.
#[test]
#[ignore = "enable after Stage 2 render::html lands; needs normalized DOM compare"]
fn conformance_against_wikrs() {
    let text = std::fs::read_to_string(FIXTURE).expect("run `cargo xtask fetch-parser-tests`");
    let tests = parse_tests(&text);
    let total = tests.len();
    let passed = 0usize;
    // for t in &tests {
    //     let got = normalize(&wikrs::render_html(&t.wikitext));
    //     if t.html.as_deref().map(normalize) == Some(got) { passed += 1; }
    // }
    eprintln!("conformance: {passed}/{total} (harness not yet wired to engine)");
}
