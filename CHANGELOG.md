# Changelog

Notable changes to wikrs. Loosely follows [Keep a Changelog](https://keepachangelog.com/); versioning is SemVer.

## [Unreleased]

### Added
- **`--format markdown`** (Stage 3, LLM-facing output): one GFM markdown document per page — escaped `# title` plus a structure-preserving body (headings from `=` count, `**bold**`/`*italic*`, `[label](./Page_title)` links, nested lists, pipe tables, fenced code). Out-of-range constructs render as visible ` ```wikitext ` fenced blocks carrying the verbatim source — never silently dropped. Requires the `ast` engine; `--engine strip` or `--stats` combinations are explicit errors.
- **Round-trip conformance harness** (`tests/markdown_roundtrip.rs`): every emitted document is parsed back by an independent GFM implementation (pulldown-cmark, dev-dependency only) and must reproduce exactly the normal form the AST declares — green over all 1,071 MediaWiki parserTests inputs, plus a dedicated fuzz target (`markdown_roundtrip`).

## [0.2.0] — 2026-07-02

### Added
- **`--format sections`** (Stage 3, LLM-facing output): one JSON object per page with the article split into flat, level-tagged sections for RAG chunking — `{"title", "sections": [{"level", "heading", "text"}]}`. `level` is the heading's `=` count (`==` → 2); prose before the first heading is the lead (`level: 0`, empty heading), omitted when the page starts with a heading. Requires the `ast` engine; combining with `--engine strip` or `--stats` is an explicit error. Schema contract: `docs/stages/stage-3-llm-output.md`.

## [0.1.1] — 2026-07-02

Docs-and-tests patch; no code changes.

- README: release status corrected (the crate **is** on crates.io — `cargo install wikrs` / `cargo add wikrs`); benchmark instructions now use `cargo bench --bench compare` (the `scripts/` directory is repo-only, excluded from the crate); WORKLOG link made absolute for the crates.io README rendering.
- Tests: new end-to-end CLI test for `--index` — parallel multistream decoding must produce byte-identical output to the sequential path through the real binary.

## [0.1.0] — 2026-07-01

First release: a fast, honest wikitext extraction engine.

### Extraction engines
- **`ast` engine (Stage 2, the default)** — `wikitext → tokenizer → parser → AST → plain text`, with **honest diagnostics**: constructs outside the declared support range are flagged (`Unsupported`) and dropped templates are flagged (`W-TEMPLATE` warning) rather than silently mangled. Supports paragraphs, headings, bold/italic, internal + external links, flat & definition lists, preformatted blocks, simple tables, refs/nowiki/comments, and inline HTML formatting tags. **~49% of the 1077 MediaWiki parserTests parse with zero diagnostics** (and climbing); blocks it can't structure fall back to the strip path, so prose is never lost. Runs at roughly the same throughput as `strip` (it also emits diagnostics).
- **`strip` engine (Stage 1, `--engine strip`)** — a fast, lossy text stripper (five byte-scanning passes). **≈22× faster than WikiExtractor** on an 8 MB dump.
- Templates are **not expanded** — by design. Expansion would require a Lua/Scribunto environment plus the full template corpus, and would cost ~2 orders of magnitude in speed (cf. Bliki at ~0.4 MB/s) — surrendering wikrs's core advantage.

### Core
- **Dump reader** (`dump`): stream `<page>` from `.xml` and multistream `.xml.bz2` at constant memory; skips redirects, filters to the main namespace.
- **CLI** (`wikrs`): `--input`, `--format text|jsonl`, `--engine ast|strip`, `--stats`, `--index`. Streams the dump in bounded batches (bounded memory, parallel rendering via rayon) and **fails loudly on dump read errors** instead of silently skipping pages. With `--index <multistream-index>`, the bz2 streams of a multistream dump are **decoded in parallel** — full enwiki in ~7.4 min instead of ~38 (identical output).
- **AST** (`Node<'a>`, borrow-friendly `Cow`) and **diagnostics** (`Diagnostic` / `Severity` = Error · Warning · Unsupported, with source spans).

### Quality
- **Tests**: per-pass unit tests, parser/tokenizer tests, CLI integration, snapshot tests (`insta`).
- **Coverage metric**: the fraction of parserTests that parse with zero diagnostics — an honest "how much we fully handle," tracked over time.
- **Robustness**: never panics, stays linear on 2 MB adversarial input; `cargo fuzz` target.
- **Benchmarks**: criterion microbench + `xtask bench-compare` (vs WikiExtractor) + `xtask bench-bliki` (vs Bliki).

### Non-goals
- Byte-level MediaWiki/Parsoid compatibility; template/Lua expansion; editing wikitext.

### Next (planned)
- More of the wikitext surface (nested lists, complex tables).
- AST → HTML rendering (Stage 3); differential conformance vs Parsoid (the "X% structurally identical" number).
