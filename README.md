# wikrs

**Fast, honest wikitext extraction and parsing — in Rust.**

> **Status: 🚧 Stage 2 engine is the default.** wikitext → clean text (or a structured AST) with **honest diagnostics**, ~22× WikiExtractor, CLI, tested. ~40% of MediaWiki parserTests parse with zero diagnostics, climbing. Not yet on crates.io.

---

## What is this?

The text inside a Wikipedia [XML dump](https://dumps.wikimedia.org) isn't HTML — it's **wikitext**, MediaWiki's markup language. Anyone training a model or building RAG over Wikipedia has to strip that wikitext into clean text first.

The de-facto tool for that, [WikiExtractor](https://github.com/attardi/wikiextractor) (Python), is slow and **silently** drops or mangles complex templates and tables.

**wikrs** is a Rust take, delivered in two tiers:

- **Floor — a faster WikiExtractor.** wikitext → clean plain text, **measured ~22× faster** than WikiExtractor on an 8 MB dump (it's Rust; see [Benchmarks](#benchmarks--test-status)). Drop-in for the "I just need the text" use case.
- **Ceiling — a modern wikitext engine.** A structured AST that preserves tables, link anchor text, and document structure — and that **emits a diagnostic when it hits input it can't faithfully handle, instead of silently corrupting the output.**

## Usage

Stage 1 (the extractor) works today. From source:

```bash
cargo build --release

# wikitext dump  ->  clean plain text (or JSON Lines)
./target/release/wikrs --input enwiki-latest-pages-articles-multistream.xml.bz2 --format jsonl > out.jsonl

# report the conversion rate instead of writing pages
./target/release/wikrs --input dump.xml.bz2 --stats
```

`--format text` (default) emits one article's plain text per record; `--format jsonl` emits `{"title":…,"text":…}` per line. Both `.xml` and multistream `.xml.bz2` are accepted. The default **`ast`** engine (Stage 2 parser: structured, honest diagnostics, faster than the old stripper) handles real articles; pass `--engine strip` for the Stage 1 fast/lossy path.

## Why is this hard? (and why that's the moat)

wikitext has no clean grammar. The only complete spec is MediaWiki's ~6,200-line PHP regex engine, and its template system is a *text macro processor*: template expansion isn't guaranteed to produce self-contained markup (a template can emit just an opening `<table>`, or a lone `<tr>`). So "parse then expand" and "expand then parse" both fail — they're entangled. Even MediaWiki's official Parsoid, with a full-time team over a decade, fell back to calling the PHP preprocessor.

So wikrs **does not chase byte-level MediaWiki compatibility** — that path is a tar pit where you reimplement two decades of bugs. Instead:

> **High correctness within an honestly-declared support range, and explicit diagnostics outside it.**

That honesty — telling you *exactly* what it couldn't parse — is the core difference from tools that are silently wrong.

## Non-goals

- ❌ Byte-level MediaWiki / Parsoid compatibility
- ❌ Full template / Lua (Scribunto) expansion
- ❌ Editing or emitting wikitext — wikrs is read-direction only: wikitext → text / AST / HTML

## Known differences vs WikiExtractor

wikrs's Stage 1 extractor is deliberately lossy, like WikiExtractor — but the exact choices differ. Current behavior:

- Templates (`{{…}}`) and tables (`{|…|}`) are dropped (nesting-aware).
- Internal links keep their visible text: `[[A|text]]`→`text`, `[[A]]`→`A`.
- `[[File:…]]` / `[[Image:…]]` are dropped, caption included.
- External links keep their label: `[url text]`→`text`; a bare `[url]` is dropped.
- `<ref>…</ref>`, HTML comments, and `<nowiki>` are removed (nowiki keeps its inner text).
- Headings, list markers, and bold/italic are reduced to their text.

Anything beyond this is honestly out of scope for Stage 1 — structure-preserving extraction (tables, link graph) is Stage 2. Behavior is tracked in [docs/stages/stage-1-extractor.md](docs/stages/stage-1-extractor.md).

## Roadmap

| Stage | What | Status |
|------:|------|--------|
| **1** | Plain-text extractor — wikitext → clean text, benchmarked against WikiExtractor | ✅ done (0.1.0, unreleased) |
| **2** | Structured AST + diagnostics — preserves structure, warns on pathological input | 🛠 in progress (~40% coverage; **now the CLI default**) |
| **3** | *(optional)* AST → HTML rendering | 💤 later |

The headline metric we're building toward: **"X% structurally identical to Parsoid on N random Wikipedia pages"**, plus a clear-eyed account of the rest. See [docs/TESTING.md](docs/TESTING.md).

## Benchmarks & test status

> Kept current on every change via the project's `wikrs-dev-workflow` skill. Methodology: [docs/TESTING.md](docs/TESTING.md).

_Last updated: 2026-06-24_

- **Tests:** green — `cargo test --all-features`
- **⚡ vs WikiExtractor** (end-to-end, 8.3 MB synthetic dump = 5000 articles, Apple Silicon): wikrs **~22× faster** — ~0.18 s / 47 MB/s vs WikiExtractor ~3.9 s / 2.1 MB/s. Reproduce: `cargo xtask make-sample-dump && cargo xtask bench-compare target/bench-dump.xml`. *(Synthetic dump = the sample article repeated; real heterogeneous dumps will differ — the order of magnitude is the point.)*
- **Sample-article throughput** (criterion, `benches/compare.rs`):

  | Implementation | Throughput | Notes |
  |---|---|---|
  | `wikrs` AST path (parse→plain, **default**) | ~174 MiB/s | Stage 2 engine — **faster than strip**; structured where it can, strip-fallback for Unsupported blocks |
  | `wikrs::extract::strip` | ~118 MiB/s | Stage 1 extractor → clean text (five allocating passes) |
  | `parse_wiki_text` (reference) | ~308 MiB/s | community Rust parser → AST (no text out), 2018 |

  > The Stage 2 **AST path** (parse → plain text) is ~1.5× faster than the Stage 1 `strip` while producing both text **and** diagnostics. It **does not expand templates** — it drops them with a `W-TEMPLATE` warning and keeps the surrounding prose. Expanding templates (à la Bliki) would mean a Lua/Scribunto engine and ~2 orders of magnitude slower (Bliki runs at ~0.4 MB/s) — surrendering the one advantage wikrs has. So: honest drop + flag, keep the speed.

  Run it yourself: `scripts/bench.sh`.
- **Stage 1 conversion rate** (parserTests, 1077 real cases): **98.1%** of pages strip to output with no residual bracket markup (`{{`, `[[`, `{|`). This is a *leniency floor* — it catches markup that **leaked**, not correctness; true correctness-vs-Parsoid is Stage 2. Check it with `wikrs --stats` or `cargo test --test parser_tests stage1_conversion_rate`.
- **Stage 2 parser coverage** (parserTests, 1077 cases): **39.6%** parse with **zero diagnostics** — fully inside the engine's declared support range (paragraphs, headings, bold/italic, internal + external links, flat & definition lists, preformatted blocks, simple tables, refs/nowiki/comments, inline HTML formatting tags). Inline templates are **dropped with a `W-TEMPLATE` warning** (prose kept, honestly flagged → *not* counted as fully supported — which is why this number went slightly *down* when template handling got more honest, not up). Track: `cargo test --test parser_tests stage2_coverage_rate`.
- **Robustness:** `strip` never panics and stays linear — 2 MB of adversarial input in ~150 ms (`tests/robustness.rs`, runs in CI). Deeper fuzzing: `cargo +nightly fuzz run strip`.

## Documentation

| Doc | Contents |
|-----|----------|
| [docs/DESIGN.md](docs/DESIGN.md) | Architecture, module layout, I/O contracts, error philosophy, non-goals |
| [docs/TESTING.md](docs/TESTING.md) | Four-layer test strategy + benchmarks |
| [docs/stages/](docs/stages/) | Per-stage checkpoints and tasks |
| [docs/PROJECT-HANDOFF.md](docs/PROJECT-HANDOFF.md) | Strategic context & decision log |

> Design docs are currently written in Chinese; they'll be translated as the project approaches a public release.

## Status & contributing

Pre-release and moving fast — the design docs above are the source of truth. No stability guarantees yet. Issues and discussion welcome once the repo goes public.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
