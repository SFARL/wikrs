# Changelog

Notable changes to wikrs. Loosely follows [Keep a Changelog](https://keepachangelog.com/); versioning is SemVer.

## [0.4.0] — 2026-07-06

A correctness-and-honesty release: seven fixes that stop the engine from producing output that *looks* clean while the parser already knew it was unreliable. The parser's diagnostics now travel with the output, and several silent-corruption paths are closed. `ast` throughput also rose ~13% (118 → ~136 MiB/s on the sample article) as a side effect of the scan cleanups.

### Added
- **Structured diagnostics in the output.** `--format jsonl` and `--format sections` now carry a per-page `diagnostics` array — each entry `{"code","severity","start","end","message"}` with a byte span into the page's wikitext. With `--engine strip` the key is **absent** (Stage 1 cannot diagnose — distinct from an empty array meaning "checked, found nothing").
- **`--stats` diagnostic tiers.** Alongside the residual-markup clean rate, `--stats` now reports `zero-diag=`, `warned=`, `unsupported=` — what the parser *knows*, not just what the byte heuristic *sees*. Plain `--format text` prints a one-line stderr summary when any page was flagged (stdout stays pure article text).
- **`--fail-on warning|unsupported`.** Exit non-zero when any page produced a diagnostic at or above the given tier — a pipeline gate. `unsupported` is the useful tier (unexpanded templates are warnings, so they fire on nearly every real page); requires `--engine ast`.
- **Peak RSS in `bench-compare`.** `cargo xtask bench-compare` now reports each side's peak resident memory (via `/usr/bin/time`), so the README's memory figures are reproducible from the same command that produces the speed figures.

### Changed
- **Dump reader rejects malformed input instead of guessing (breaking).** A page whose `<ns>` is missing or unparseable is now a hard error (was silently treated as namespace 0 — fabricating articles); a page with more than one `<text>` element (a `pages-meta-history` dump) is a hard error (was silently concatenating revisions). Paired `<redirect …></redirect>` is now recognized in addition to the self-closing form. Inputs that previously produced (possibly wrong) output with exit 0 may now fail loudly — feed a single-revision `pages-articles` dump.
- **jsonl/sections output schema.** Both formats gained the `diagnostics` field described above. Consumers that pinned the exact object shape must account for it.

### Fixed
- **Quote-aware tag close** (`tokenizer`, both engines): a `>` inside a quoted attribute value (`<ref name="a>b" />`) is no longer mistaken for the tag close, which had swallowed everything to the next `</ref>` — or to the end of the page — with zero diagnostics. One shared close scanner now backs `ref`, `nowiki`, formatting tags, and the Stage 1 strippers.
- **Unclosed `{{`** in inline text now drops to end of block like every other path, instead of leaking a literal `{{…` tail into otherwise-clean output (still `W-TEMPLATE`-flagged).
- **Uppercase `COLSPAN`/`ROWSPAN`** spanning-cell grids now bail honestly (`U-TABLE`) instead of being silently flattened into misaligned rows; a prose mention of "colspan" after the content pipe no longer needlessly bails a parseable table.
- **Table cell attribute junk** (`data-x="]]" | text`) no longer leaks into cell content: the attribute/content splitter is quote-aware and its bracket depths saturate at zero (a stray `]]`/`}}` could previously drive them negative and hide the real separator).
- **`has_tag`** no longer false-flags a structural tag that appears **inside** a comment/`<ref>`/`<nowiki>` body (it reuses the tokenizer's span logic), so blocks that were needlessly dropped as `U-HTML` now parse.
- **Differential metric** (`diff`): the order-independent word-precision fallback that rescues table-cell reordering is now applied only to pages that actually rendered a table — a page with no table whose words are merely scrambled is correctly `Divergent`, not falsely faithful.

## [0.3.0] — 2026-07-03

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
