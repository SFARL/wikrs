//! Developer tasks for wikrs. Run via `cargo xtask <subcommand>`.
//!
//! Dev-only helpers (fetching the GPL parser tests, building a sample dump,
//! comparison benchmarks). Never part of the published crate.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};

const PARSER_TESTS_DEST: &str = "tests/fixtures/parserTests.txt";
const SAMPLE_ARTICLE: &str = "tests/fixtures/sample_article.wikitext";
const DIFF_TITLES: &str = "tests/diff/titles.txt";
const DIFF_CACHE: &str = "tests/diff/cache";
/// REST Parsoid HTML endpoint (ground truth) — percent-encoded title appended.
const REST_HTML: &str = "https://en.wikipedia.org/api/rest_v1/page/html/";
/// Raw-wikitext endpoint (wikrs input); title goes in the `title` query param.
const RAW_WIKITEXT: &str = "https://en.wikipedia.org/w/index.php";
/// Wikipedia asks every client to send a descriptive User-Agent.
const USER_AGENT: &str =
    "wikrs-dev-diff/0.1 (https://github.com/SFARL/wikrs; differential harness)";

/// wikrs developer tasks.
#[derive(Parser)]
#[command(name = "xtask", about)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Download MediaWiki's parserTests.txt (GPL — not vendored) into tests/fixtures/.
    FetchParserTests {
        /// Source URL (MediaWiki core mirror, raw).
        #[arg(
            long,
            default_value = "https://raw.githubusercontent.com/wikimedia/mediawiki/master/tests/parser/parserTests.txt"
        )]
        url: String,
    },
    /// Generate a synthetic multi-page dump from the sample article (for benching).
    MakeSampleDump {
        /// How many pages to write.
        #[arg(long, default_value_t = 5000)]
        pages: usize,
        /// Output path (under target/, gitignored).
        #[arg(long, default_value = "target/bench-dump.xml")]
        out: PathBuf,
    },
    /// Compare wikrs vs WikiExtractor on a dump (wall-clock + throughput).
    BenchCompare {
        /// Path to a dump (.xml or .xml.bz2). Make one with `make-sample-dump`.
        dump: PathBuf,
        /// Python interpreter that has `wikiextractor` installed.
        #[arg(long, default_value = "tools/wikiextractor/.venv/bin/python")]
        wikiextractor_python: PathBuf,
    },
    /// Benchmark the Bliki engine (Java) on a wikitext file (set up via tools/bliki/setup.sh).
    BenchBliki {
        /// Wikitext file to render repeatedly.
        #[arg(default_value = "tests/fixtures/sample_article.wikitext")]
        wikitext: PathBuf,
        /// Iterations.
        #[arg(long, default_value_t = 3000)]
        iters: usize,
    },
    /// Fetch real pages (wikitext + Parsoid HTML ground truth) into the
    /// gitignored diff cache. Needs network; run before `diff-report`.
    DiffFetch {
        /// Newline-delimited page titles (names only; `#` comments allowed).
        #[arg(long, default_value = DIFF_TITLES)]
        titles: PathBuf,
        /// Cache directory (gitignored).
        #[arg(long, default_value = DIFF_CACHE)]
        out: PathBuf,
        /// Only fetch the first N titles.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Diff cached pages (wikrs render vs ground-truth prose) into the three
    /// headline numbers. Offline; reads what `diff-fetch` cached.
    DiffReport {
        /// Cache directory written by `diff-fetch`.
        #[arg(long, default_value = DIFF_CACHE)]
        cache: PathBuf,
        /// Print up to this many lowest-precision pages (inspect these first).
        #[arg(long, default_value_t = 10)]
        show: usize,
    },
    /// Sample N random main-namespace (ns0) titles into a pinned list. The random
    /// API has no seed, so reproducibility comes from pinning the result. Network.
    DiffSample {
        /// How many titles to sample.
        #[arg(long, default_value_t = 50)]
        count: usize,
        /// Write the list here (with a header). Omit to print titles to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::FetchParserTests { url } => fetch_parser_tests(&url),
        Cmd::MakeSampleDump { pages, out } => make_sample_dump(pages, &out),
        Cmd::BenchCompare {
            dump,
            wikiextractor_python,
        } => bench_compare(&dump, &wikiextractor_python),
        Cmd::BenchBliki { wikitext, iters } => bench_bliki(&wikitext, iters),
        Cmd::DiffFetch { titles, out, limit } => diff_fetch(&titles, &out, limit),
        Cmd::DiffReport { cache, show } => diff_report(&cache, show),
        Cmd::DiffSample { count, out } => diff_sample(count, out.as_deref()),
    }
}

fn fetch_parser_tests(url: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all("tests/fixtures").context("create tests/fixtures")?;
    eprintln!("Downloading parserTests.txt (GPL — not committed) from {url}");
    let status = Command::new("curl")
        .args(["-fSL", "--retry", "3", "-o", PARSER_TESTS_DEST, url])
        .status()
        .context("running curl (is it installed?)")?;
    if !status.success() {
        bail!("curl failed with status {status}");
    }
    let bytes = std::fs::metadata(PARSER_TESTS_DEST)?.len();
    println!("Saved {PARSER_TESTS_DEST} ({bytes} bytes). GPL + .gitignored — do not commit it.");
    Ok(())
}

fn make_sample_dump(pages: usize, out: &Path) -> anyhow::Result<()> {
    let article = std::fs::read_to_string(SAMPLE_ARTICLE).context("read sample article")?;
    let escaped = xml_escape(&article);
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let mut f = BufWriter::new(File::create(out).context("create dump")?);
    writeln!(
        f,
        r#"<mediawiki xmlns="http://www.mediawiki.org/xml/export-0.11/" version="0.11" xml:lang="en">"#
    )?;
    writeln!(
        f,
        r#"<siteinfo><sitename>wikrs-bench</sitename><namespaces><namespace key="0" case="first-letter" /></namespaces></siteinfo>"#
    )?;
    // One tag per line: WikiExtractor's parser is line-oriented (real dumps are
    // pretty-printed this way), so single-line pages would extract 0 articles.
    for i in 1..=pages {
        writeln!(f, "<page>")?;
        writeln!(f, "<title>Examplia {i}</title>")?;
        writeln!(f, "<ns>0</ns>")?;
        writeln!(f, "<id>{i}</id>")?;
        writeln!(f, "<revision>")?;
        writeln!(f, "<id>{i}</id>")?;
        writeln!(f, "<text xml:space=\"preserve\">{escaped}</text>")?;
        writeln!(f, "</revision>")?;
        writeln!(f, "</page>")?;
    }
    writeln!(f, "</mediawiki>")?;
    f.flush()?;
    let bytes = std::fs::metadata(out)?.len();
    println!(
        "wrote {} ({pages} pages, {:.1} MB)",
        out.display(),
        bytes as f64 / 1e6
    );
    Ok(())
}

fn bench_compare(dump: &Path, python: &Path) -> anyhow::Result<()> {
    if !dump.exists() {
        bail!(
            "dump not found: {} (run `cargo xtask make-sample-dump`)",
            dump.display()
        );
    }
    let mb = std::fs::metadata(dump)?.len() as f64 / 1e6;
    eprintln!("dump: {mb:.1} MB\nbuilding wikrs --release ...");
    if !Command::new("cargo")
        .args(["build", "--release", "-q", "-p", "wikrs"])
        .status()?
        .success()
    {
        bail!("cargo build --release failed");
    }

    let wikrs_time = time_cmd(
        Command::new("target/release/wikrs")
            .arg("--input")
            .arg(dump)
            .args(["--format", "text"]),
    )
    .context("running wikrs")?;
    report("wikrs", wikrs_time, mb);

    let we = time_cmd(
        Command::new(python)
            .args(["-m", "wikiextractor.WikiExtractor"])
            .arg(dump)
            .args(["-o", "-", "-q"]),
    );
    match we {
        Ok(we_time) => {
            report("wikiextractor", we_time, mb);
            println!(
                "speedup: {:.1}x faster",
                we_time.as_secs_f64() / wikrs_time.as_secs_f64()
            );
        }
        Err(e) => eprintln!("wikiextractor run failed ({e}); reported wikrs only"),
    }
    Ok(())
}

fn time_cmd(cmd: &mut Command) -> anyhow::Result<Duration> {
    let start = Instant::now();
    let status = cmd.stdout(Stdio::null()).stderr(Stdio::null()).status()?;
    let elapsed = start.elapsed();
    if !status.success() {
        bail!("command exited with {status}");
    }
    Ok(elapsed)
}

fn report(name: &str, d: Duration, mb: f64) {
    println!(
        "{name:<14} {:>7.2} s   {:>7.1} MB/s",
        d.as_secs_f64(),
        mb / d.as_secs_f64()
    );
}

fn bench_bliki(wikitext: &Path, iters: usize) -> anyhow::Result<()> {
    if !Path::new("tools/bliki/out/BlikiBench.class").exists() {
        bail!("Bliki is not set up — run `tools/bliki/setup.sh` first");
    }
    let status = Command::new("java")
        .args(["-cp", "tools/bliki/lib/*:tools/bliki/out", "BlikiBench"])
        .arg(wikitext)
        .arg(iters.to_string())
        .status()
        .context("running java (is a JDK installed?)")?;
    if !status.success() {
        bail!("Bliki harness exited with {status}");
    }
    Ok(())
}

/// Percent-encode a page title for a URL (query value or path segment).
/// Conservative: only RFC-3986 unreserved characters pass through.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Filesystem-safe stem for a title's cache files (non-alphanumerics -> `_`).
/// Readable by design; a curated, distinct title list makes collisions a
/// non-issue.
fn slugify(title: &str) -> String {
    title
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

/// GET `url` as text via curl (same dependency-light pattern as
/// `fetch-parser-tests`), with the polite User-Agent Wikipedia expects.
fn curl_text(url: &str) -> anyhow::Result<String> {
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--retry",
            "2",
            "--max-time",
            "30",
            "-A",
            USER_AGENT,
            url,
        ])
        .output()
        .context("running curl (is it installed?)")?;
    if !out.status.success() {
        bail!(
            "curl failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    String::from_utf8(out.stdout).context("response was not UTF-8")
}

/// Extract visible prose text from Parsoid HTML. Selecting a *superset* of
/// prose-bearing elements is deliberately safe for the precision metric: extra
/// truth text can only make wikrs's output easier to corroborate, and the
/// shingles are de-duplicated downstream.
fn html_to_text(html: &str) -> String {
    use scraper::{Html, Selector};
    let doc = Html::parse_document(html);
    let sel = Selector::parse(
        "p, h1, h2, h3, h4, h5, h6, li, dd, dt, caption, th, td, blockquote, figcaption",
    )
    .expect("static prose selector");
    let mut out = String::new();
    for el in doc.select(&sel) {
        for chunk in el.text() {
            out.push_str(chunk);
        }
        out.push('\n');
    }
    out
}

/// `diff-fetch`: pull wikitext + Parsoid HTML for each title into the cache.
fn diff_fetch(titles_path: &Path, out_dir: &Path, limit: Option<usize>) -> anyhow::Result<()> {
    let list = std::fs::read_to_string(titles_path)
        .with_context(|| format!("read titles {}", titles_path.display()))?;
    let mut titles: Vec<&str> = list
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if let Some(n) = limit {
        titles.truncate(n);
    }
    std::fs::create_dir_all(out_dir).context("create diff cache dir")?;
    let total = titles.len();
    eprintln!("fetching {total} page(s) -> {}", out_dir.display());

    let mut cached = 0usize;
    for (i, title) in titles.iter().enumerate() {
        let enc = percent_encode(title);
        let fetch = || -> anyhow::Result<(String, String)> {
            let wikitext = curl_text(&format!("{RAW_WIKITEXT}?title={enc}&action=raw"))?;
            let html = curl_text(&format!("{REST_HTML}{enc}"))?;
            Ok((wikitext, html))
        };
        match fetch() {
            Ok((wikitext, html)) => {
                let slug = slugify(title);
                std::fs::write(out_dir.join(format!("{slug}.wikitext")), wikitext)?;
                std::fs::write(
                    out_dir.join(format!("{slug}.truth.txt")),
                    html_to_text(&html),
                )?;
                cached += 1;
                eprintln!("  [{}/{total}] {title}", i + 1);
            }
            Err(e) => eprintln!("  [{}/{total}] SKIP {title}: {e}", i + 1),
        }
    }
    println!("cached {cached}/{total} page(s) in {}", out_dir.display());
    Ok(())
}

/// `diff-report`: classify every cached page and print the three headline
/// numbers (+ the separate coverage figure, + the divergent list to inspect).
fn diff_report(cache_dir: &Path, show: usize) -> anyhow::Result<()> {
    use wikrs::diag::Severity;
    use wikrs::diff::{self, Bucket, Report};

    if !cache_dir.exists() {
        bail!(
            "cache {} not found — run `cargo xtask diff-fetch` first",
            cache_dir.display()
        );
    }
    let mut slugs: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(cache_dir).context("read cache dir")? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("wikitext") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                slugs.push(stem.to_owned());
            }
        }
    }
    slugs.sort();
    if slugs.is_empty() {
        bail!(
            "no cached pages in {} — run `cargo xtask diff-fetch`",
            cache_dir.display()
        );
    }

    let mut report = Report::default();
    let mut per_page: Vec<(f64, Bucket, String)> = Vec::new();
    // Fidelity overlay, measured on *every* page independent of the buckets. Even
    // a page that is `Reported` (it flagged some out-of-range construct) still has
    // prose wikrs extracted, and the question that matters is whether *that* prose
    // is faithful. Page-level bucketing alone hides this on real articles, which
    // almost always contain at least one flagged construct.
    let mut precision_sum = 0.0f64;
    let mut word_precision_sum = 0.0f64;
    let mut coverage_sum = 0.0f64;
    let mut faithful_text = 0usize;
    let mut empty_output = 0usize;

    for slug in &slugs {
        let wikitext = std::fs::read_to_string(cache_dir.join(format!("{slug}.wikitext")))?;
        let truth = std::fs::read_to_string(cache_dir.join(format!("{slug}.truth.txt")))
            .with_context(|| format!("missing truth.txt for {slug}"))?;

        let parsed = wikrs::parser::parse(&wikitext);
        let text = wikrs::render::plain(&parsed.nodes);
        let has_unsupported = parsed
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Unsupported);
        // A rendered table is the one legitimate source of word reordering —
        // it licenses the order-robust word-precision fallback for this page.
        let has_table = parsed
            .nodes
            .iter()
            .any(|n| matches!(n, wikrs::ast::Node::Table { .. }));

        let bucket = diff::classify(&text, &truth, has_unsupported, has_table);
        report.record(bucket);

        let prec = diff::precision(&text, &truth);
        precision_sum += prec;
        word_precision_sum += diff::word_precision(&text, &truth);
        coverage_sum += diff::coverage(&text, &truth);
        if diff::is_faithful(&text, &truth, has_table) {
            faithful_text += 1;
        }
        if text.split_whitespace().next().is_none() {
            empty_output += 1;
        }
        per_page.push((prec, bucket, slug.clone()));
    }

    let (x, y, z) = report.percentages();
    let total = report.total();
    let n = total as f64;
    println!("\nwikrs differential — {total} page(s)\n");
    println!("page buckets (a page is Reported if it flags ANY unsupported construct):");
    println!(
        "  {x:5.1}%  identical (faithful, zero diagnostics)   [{}]",
        report.faithful
    );
    println!(
        "  {y:5.1}%  structural diff (silent)                 [{}]",
        report.divergent
    );
    println!(
        "  {z:5.1}%  reported (>=1 unsupported construct)     [{}]",
        report.reported
    );
    println!("\nextracted-prose fidelity (every page, independent of the buckets):");
    println!(
        "  mean precision: {:5.1}%   (of what wikrs emits, how much the article corroborates)",
        100.0 * precision_sum / n
    );
    println!(
        "  word precision: {:5.1}%   (order-independent — robust to table-cell reordering)",
        100.0 * word_precision_sum / n
    );
    println!(
        "  mean coverage:  {:5.1}%   (of article prose, how much wikrs emits — rest = templates, by design)",
        100.0 * coverage_sum / n
    );
    println!(
        "  faithful prose: {faithful_text}/{total} pages \
         (shingle >= 90%; word >= 97% only for pages with a rendered table)"
    );
    println!("  empty output:   {empty_output}/{total} pages");
    per_page.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!(
        "\nlowest-precision pages (inspect first — real wikrs bug, or just dropped templates?):"
    );
    for (prec, bucket, slug) in per_page.iter().take(show) {
        let tag = match bucket {
            Bucket::Faithful => 'F',
            Bucket::Divergent => 'D', // silent — precision low, no diagnostic
            Bucket::Reported => 'R',  // honestly flagged
        };
        println!("  {:5.1}%  [{tag}]  {slug}", 100.0 * prec);
    }
    Ok(())
}

/// Extract page titles from a `list=random` API response.
fn parse_random_titles(json: &str) -> anyhow::Result<Vec<String>> {
    let v: serde_json::Value = serde_json::from_str(json).context("parse random API JSON")?;
    let arr = v["query"]["random"]
        .as_array()
        .context("unexpected random API shape (no query.random array)")?;
    Ok(arr
        .iter()
        .filter_map(|item| item["title"].as_str().map(str::to_owned))
        .collect())
}

/// `diff-sample`: pin N random main-namespace titles into a reproducible list.
fn diff_sample(count: usize, out: Option<&Path>) -> anyhow::Result<()> {
    let mut titles: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while titles.len() < count {
        let want = (count - titles.len()).min(20);
        let url = format!(
            "https://en.wikipedia.org/w/api.php?action=query&format=json&list=random\
             &rnnamespace=0&rnfilterredir=nonredirects&rnlimit={want}"
        );
        let batch = parse_random_titles(&curl_text(&url)?)?;
        if batch.is_empty() {
            bail!("random API returned no titles");
        }
        for t in batch {
            if seen.insert(t.clone()) {
                titles.push(t);
                if titles.len() >= count {
                    break;
                }
            }
        }
    }
    titles.sort();
    let header = format!(
        "# wikrs differential — {} random ns0 titles, pinned for reproducibility\n\
         # (the random API has no seed). Generated by `cargo xtask diff-sample`.\n\
         # Page CONTENT (CC-BY-SA) is fetched at run time by `diff-fetch` into the\n\
         # gitignored cache, never committed.\n",
        titles.len()
    );
    let body = format!("{header}{}\n", titles.join("\n"));
    match out {
        Some(path) => {
            std::fs::write(path, body)?;
            println!("wrote {} title(s) -> {}", titles.len(), path.display());
        }
        None => print!("{body}"),
    }
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_random_api_titles() {
        let json = r#"{"batchcomplete":"","query":{"random":[
            {"id":1,"ns":0,"title":"Earth"},{"id":2,"ns":0,"title":"Mars"}]}}"#;
        assert_eq!(parse_random_titles(json).unwrap(), vec!["Earth", "Mars"]);
    }

    #[test]
    fn parse_random_titles_errors_on_bad_shape() {
        assert!(parse_random_titles(r#"{"oops":true}"#).is_err());
    }
}
