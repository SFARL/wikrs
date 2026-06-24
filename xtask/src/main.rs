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
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::FetchParserTests { url } => fetch_parser_tests(&url),
        Cmd::MakeSampleDump { pages, out } => make_sample_dump(pages, &out),
        Cmd::BenchCompare {
            dump,
            wikiextractor_python,
        } => bench_compare(&dump, &wikiextractor_python),
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

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
