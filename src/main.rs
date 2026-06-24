//! `wikrs` command-line interface.
//!
//! Argument wiring and output formats are implemented in Stage 1
//! (see `docs/stages/stage-1-extractor.md`, Task 4).

use clap::Parser;

/// Fast, honest wikitext extraction and parsing.
#[derive(Debug, Parser)]
#[command(name = "wikrs", version, about)]
struct Cli {
    /// Path to a Wikimedia XML dump (`.xml` or `.xml.bz2`).
    #[arg(long)]
    input: Option<std::path::PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    anyhow::bail!(
        "wikrs is not implemented yet (input = {:?}). See docs/stages/stage-1-extractor.md",
        cli.input
    );
}
