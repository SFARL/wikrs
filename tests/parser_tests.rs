//! Layer 1 (see `docs/TESTING.md`): MediaWiki `parserTests.txt` conformance harness.
//!
//! The fixture is **GPL** and fetched at test time
//! (`cargo xtask fetch-parser-tests`) — never committed (see DESIGN.md §11).
//! These tests parse the format and load the real fixture now; the per-case
//! conformance comparison is `#[ignore]`d until the engine can render
//! comparable output (Stage 2: AST -> normalized HTML).

use std::collections::HashMap;
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/parserTests.txt";

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
