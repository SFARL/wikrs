# Changelog

Notable changes to wikrs. Loosely follows [Keep a Changelog](https://keepachangelog.com/); versioning is SemVer.

## [0.1.0] — unreleased

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
