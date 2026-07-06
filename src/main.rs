//! `wikrs` command-line interface: a Wikimedia dump -> clean text / JSON Lines.

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, ValueEnum};
use rayon::prelude::*;

use wikrs::diag::Severity;
use wikrs::{diag, dump, extract, output, parser, render};

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
    /// Stage 3 (LLM output): GFM markdown per page — `# title` plus a
    /// structure-preserving body, validated by a round-trip conformance
    /// harness (requires the `ast` engine).
    Markdown,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Engine {
    /// Stage 1: fast, lossy text stripper.
    Strip,
    /// Stage 2: tokenizer → parser → AST → plain text (honest; flags out-of-range).
    Ast,
}

/// Severity tier for `--fail-on`.
#[derive(Debug, Clone, Copy, PartialEq, ValueEnum)]
enum FailOn {
    /// Any diagnostic at all. Strict: unexpanded templates are warnings, so
    /// this fires on nearly every real Wikipedia page.
    Warning,
    /// Only constructs the parser refused to guess at (plus genuine errors) —
    /// the useful gate for "was anything silently out of range".
    Unsupported,
}

/// One rendered page plus what the parser reported about it. `diags` is `None`
/// for the strip engine: Stage 1 cannot diagnose, which is different from
/// "diagnosed and found nothing" (`Some` of an empty list).
struct Rendered {
    title: String,
    text: String,
    diags: Option<Vec<diag::Diagnostic>>,
}

/// Fast, honest wikitext extraction.
#[derive(Debug, Parser)]
#[command(name = "wikrs", version, about)]
struct Cli {
    /// Path to a Wikimedia XML dump (`.xml` or `.xml.bz2`). Must be a
    /// single-revision `pages-articles` dump; a multi-revision
    /// `pages-meta-history` dump is rejected, not silently concatenated.
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

    /// Print a conversion-rate and diagnostics summary to stderr instead of
    /// writing pages.
    #[arg(long)]
    stats: bool,

    /// Exit non-zero if any page produced a diagnostic at or above this tier.
    /// Needs `--engine ast` (strip cannot diagnose).
    #[arg(long, value_enum)]
    fail_on: Option<FailOn>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let ast_only = matches!(cli.format, Format::Sections | Format::Markdown);
    if ast_only && matches!(cli.engine, Engine::Strip) {
        anyhow::bail!(
            "--format {:?} needs the AST; use --engine ast (the default)",
            cli.format
        );
    }
    if ast_only && cli.stats {
        anyhow::bail!("--stats measures plain-text conversion; use --format text or jsonl");
    }
    if cli.fail_on.is_some() && matches!(cli.engine, Engine::Strip) {
        anyhow::bail!("--fail-on needs diagnostics; use --engine ast (the default)");
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
    let (mut zero_diag, mut warned, mut unsupported) = (0usize, 0usize, 0usize);
    let mut failing = 0usize;
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

        let rendered: Vec<Rendered> = batch
            .into_par_iter()
            .map(|p| match cli.engine {
                Engine::Strip => Rendered {
                    text: extract::strip(&p.text),
                    title: p.title,
                    diags: None,
                },
                Engine::Ast => {
                    // One parse per page: text, sections, markdown, and the
                    // diagnostics all come off this single AST (which does not
                    // outlive this closure).
                    let parsed = parser::parse(&p.text);
                    let text = match cli.format {
                        Format::Sections => {
                            output::to_sections_jsonl(&p.title, &parsed.nodes, &parsed.diagnostics)
                        }
                        Format::Markdown => {
                            output::to_markdown(&p.title, &render::markdown(&parsed.nodes))
                        }
                        Format::Text | Format::Jsonl => render::plain(&parsed.nodes),
                    };
                    Rendered {
                        title: p.title,
                        text,
                        diags: Some(parsed.diagnostics),
                    }
                }
            })
            .collect();

        for r in &rendered {
            total += 1;
            if let Some(diags) = &r.diags {
                if diags.is_empty() {
                    zero_diag += 1;
                }
                if diags
                    .iter()
                    .any(|d| matches!(d.severity, Severity::Warning))
                {
                    warned += 1;
                }
                if diags
                    .iter()
                    .any(|d| matches!(d.severity, Severity::Unsupported | Severity::Error))
                {
                    unsupported += 1;
                }
                let hit = match cli.fail_on {
                    Some(FailOn::Warning) => !diags.is_empty(),
                    Some(FailOn::Unsupported) => diags
                        .iter()
                        .any(|d| !matches!(d.severity, Severity::Warning)),
                    None => false,
                };
                if hit {
                    failing += 1;
                }
            }
            if cli.stats {
                if extract::looks_clean(&r.text) {
                    clean += 1;
                }
            } else {
                match cli.format {
                    Format::Text | Format::Sections | Format::Markdown => {
                        writeln!(w, "{}", r.text)?
                    }
                    Format::Jsonl => writeln!(
                        w,
                        "{}",
                        output::to_jsonl(&r.title, &r.text, r.diags.as_deref())
                    )?,
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
        // What the parser *knows*, next to what the residual heuristic *sees* —
        // strip has no diagnostics, so the tier line would be meaningless there.
        if matches!(cli.engine, Engine::Ast) {
            eprintln!("zero-diag={zero_diag} warned={warned} unsupported={unsupported}");
        }
    } else {
        w.flush()?;
        // The page text on stdout must stay clean, but the run must not LOOK
        // clean when it wasn't: one stderr line says what was flagged.
        if warned + unsupported > 0 {
            eprintln!(
                "wikrs: {warned} page(s) with warnings, {unsupported} page(s) with \
                 unsupported constructs ({zero_diag}/{total} zero-diagnostic)"
            );
        }
    }
    if let (Some(tier), true) = (cli.fail_on, failing > 0) {
        let name = match tier {
            FailOn::Warning => "warning",
            FailOn::Unsupported => "unsupported",
        };
        anyhow::bail!("{failing} page(s) with {name}+ diagnostics (--fail-on {name})");
    }
    Ok(())
}
