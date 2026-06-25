//! `wikrs` command-line interface: a Wikimedia dump -> clean text / JSON Lines.

use std::io::{self, Write};
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use rayon::prelude::*;

use wikrs::{dump, extract, output, parser, render};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Jsonl,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Engine {
    /// Stage 1: fast, lossy text stripper.
    Strip,
    /// Stage 2: tokenizer → parser → AST → plain text (honest; flags out-of-range).
    Ast,
}

/// Fast, honest wikitext extraction.
#[derive(Debug, Parser)]
#[command(name = "wikrs", version, about)]
struct Cli {
    /// Path to a Wikimedia XML dump (`.xml` or `.xml.bz2`).
    #[arg(long)]
    input: PathBuf,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Extraction engine: `strip` (Stage 1, fast/lossy) or `ast` (Stage 2 parser).
    #[arg(long, value_enum, default_value_t = Engine::Strip)]
    engine: Engine,

    /// Print a conversion-rate summary to stderr instead of writing pages.
    #[arg(long)]
    stats: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Read sequentially (one decompressor), strip in parallel.
    let pages: Vec<dump::Page> = dump::open(&cli.input)?
        .filter_map(Result::ok)
        .filter(dump::Page::is_article)
        .collect();

    let rendered: Vec<(String, String)> = pages
        .par_iter()
        .map(|p| {
            let text = match cli.engine {
                Engine::Strip => extract::strip(&p.text),
                Engine::Ast => render::plain(&parser::parse(&p.text).nodes),
            };
            (p.title.clone(), text)
        })
        .collect();

    if cli.stats {
        let total = rendered.len();
        let clean = rendered
            .iter()
            .filter(|(_, t)| extract::looks_clean(t))
            .count();
        let pct = if total == 0 {
            0.0
        } else {
            100.0 * clean as f64 / total as f64
        };
        eprintln!("pages={total} clean={clean} ({pct:.1}% clean conversion)");
        return Ok(());
    }

    let stdout = io::stdout();
    let mut w = io::BufWriter::new(stdout.lock());
    for (title, text) in &rendered {
        match cli.format {
            Format::Text => writeln!(w, "{text}")?,
            Format::Jsonl => writeln!(w, "{}", output::to_jsonl(title, text))?,
        }
    }
    w.flush()?;
    Ok(())
}
