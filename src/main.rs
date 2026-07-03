//! `wikrs` command-line interface: a Wikimedia dump -> clean text / JSON Lines.

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, ValueEnum};
use rayon::prelude::*;

use wikrs::{dump, extract, output, parser, render};

/// Batch bounds for the streaming pipeline: read up to this many article pages
/// (or this many bytes of raw wikitext, whichever fills first), render the
/// batch in parallel, write it in dump order, repeat. Memory stays O(batch) —
/// a 20 GB enwiki dump must never be buffered whole. 4096 pages keeps every
/// core busy between batch boundaries; the byte cap bounds the batch when
/// individual pages are unusually large.
const BATCH_PAGES: usize = 4096;
const BATCH_BYTES: usize = 32 << 20; // 32 MiB of raw wikitext

#[derive(Debug, Clone, Copy, PartialEq, ValueEnum)]
enum Format {
    Text,
    Jsonl,
    /// Stage 3 (LLM output): one JSON object per page with flat, level-tagged
    /// sections for RAG chunking (requires the `ast` engine).
    Sections,
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

    /// Companion multistream index (`…-multistream-index.txt[.bz2]`). Enables
    /// parallel bz2 decoding of a multistream dump — several times faster on
    /// multi-core, byte-identical output.
    #[arg(long)]
    index: Option<PathBuf>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Extraction engine: `ast` (Stage 2, structured + honest diagnostics) or
    /// `strip` (Stage 1, fast/lossy, no diagnostics).
    #[arg(long, value_enum, default_value_t = Engine::Ast)]
    engine: Engine,

    /// Print a conversion-rate summary to stderr instead of writing pages.
    #[arg(long)]
    stats: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.format == Format::Sections && matches!(cli.engine, Engine::Strip) {
        anyhow::bail!("--format sections needs the AST; use --engine ast (the default)");
    }
    if cli.format == Format::Sections && cli.stats {
        anyhow::bail!("--stats measures plain-text conversion; use --format text or jsonl");
    }

    // Stream the dump in bounded batches: read sequentially (one decompressor),
    // render each batch in parallel, write in dump order. A dump read/decode
    // error is a hard error — silently skipping pages (the old
    // `filter_map(Result::ok)`) would truncate output with exit code 0, the
    // exact silent failure wikrs exists to avoid.
    let mut pages = match &cli.index {
        Some(index) => dump::open_multistream(&cli.input, index)?,
        None => dump::open(&cli.input)?,
    };
    let stdout = io::stdout();
    let mut w = io::BufWriter::new(stdout.lock());
    let (mut total, mut clean, mut read) = (0usize, 0usize, 0usize);
    loop {
        let mut batch: Vec<dump::Page> = Vec::with_capacity(BATCH_PAGES);
        let mut bytes = 0usize;
        for res in pages.by_ref() {
            let page = res.with_context(|| {
                format!(
                    "reading dump {} (after {read} page(s))",
                    cli.input.display()
                )
            })?;
            read += 1;
            if !page.is_article() {
                continue;
            }
            bytes += page.text.len();
            batch.push(page);
            if batch.len() >= BATCH_PAGES || bytes >= BATCH_BYTES {
                break;
            }
        }
        if batch.is_empty() {
            break;
        }

        let rendered: Vec<(String, String)> = batch
            .into_par_iter()
            .map(|p| {
                let text = match (cli.format, cli.engine) {
                    // The whole JSON line is built here: sectioning needs the
                    // AST, which does not outlive this closure.
                    (Format::Sections, _) => {
                        output::to_sections_jsonl(&p.title, &parser::parse(&p.text).nodes)
                    }
                    (_, Engine::Strip) => extract::strip(&p.text),
                    (_, Engine::Ast) => render::plain(&parser::parse(&p.text).nodes),
                };
                (p.title, text)
            })
            .collect();

        for (title, text) in &rendered {
            total += 1;
            if cli.stats {
                if extract::looks_clean(text) {
                    clean += 1;
                }
            } else {
                match cli.format {
                    Format::Text | Format::Sections => writeln!(w, "{text}")?,
                    Format::Jsonl => writeln!(w, "{}", output::to_jsonl(title, text))?,
                }
            }
        }
    }

    if cli.stats {
        let pct = if total == 0 {
            0.0
        } else {
            100.0 * clean as f64 / total as f64
        };
        eprintln!("pages={total} clean={clean} ({pct:.1}% clean conversion)");
        return Ok(());
    }
    w.flush()?;
    Ok(())
}
