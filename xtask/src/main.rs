//! Developer tasks for wikrs. Run via `cargo xtask <subcommand>`.
//!
//! Dev-only helpers (fetching the GPL parser tests, comparison benchmarks).
//! Never part of the published crate.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};

const PARSER_TESTS_DEST: &str = "tests/fixtures/parserTests.txt";

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
    /// Compare wikrs vs WikiExtractor on a dump slice (wall-clock + throughput).
    BenchCompare {
        /// Path to a dump slice (.xml or .xml.bz2).
        dump: PathBuf,
        /// Python interpreter that has `wikiextractor` installed.
        #[arg(long, default_value = "tools/wikiextractor/.venv/bin/python")]
        wikiextractor_python: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::FetchParserTests { url } => fetch_parser_tests(&url),
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

fn bench_compare(dump: &Path, _python: &Path) -> anyhow::Result<()> {
    if !dump.exists() {
        bail!("dump not found: {}", dump.display());
    }
    // TODO(after Stage 1 plan Task 7 lands the CLI): under `/usr/bin/time -l`,
    // time `wikrs --input <dump> --format text` and
    // `<python> -m wikiextractor.WikiExtractor <dump> -o -`, then print a
    // wall-clock / MB-per-sec / peak-RSS comparison table.
    bail!("bench-compare is a skeleton — implement once the wikrs CLI exists (Stage 1 Task 7)");
}
