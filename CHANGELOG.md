# Changelog

Notable changes to wikrs. Loosely follows [Keep a Changelog](https://keepachangelog.com/); versioning is SemVer.

## [0.1.0] — unreleased

First usable release: the Stage 1 plain-text extractor.

### Added
- **Dump reader** (`dump`): stream `<page>` from `.xml` and multistream `.xml.bz2` at constant memory, skipping redirects and filtering to the main namespace.
- **Extractor** (`extract::strip`): wikitext → clean plain text — drops comments/refs/nowiki, templates, and tables; keeps link anchor text; strips markup. Deliberately lossy (not a parser).
- **CLI** (`wikrs`): `--input`, `--format text|jsonl`, `--stats`; parallel via rayon.
- **Conversion-rate metric** (`extract::looks_clean`, `--stats`): reported over the 1077 MediaWiki parserTests cases — 98.1% clean (a leniency floor, not a correctness measure).
- **Benchmarks**: criterion microbench, plus `xtask bench-compare` — **≈22× faster than WikiExtractor** on an 8 MB dump.
- **Robustness**: `strip` never panics and stays linear on 2 MB adversarial input; `cargo fuzz` target under `fuzz/`.

### Not yet (planned)
- Structured AST that preserves tables and links, with diagnostics on out-of-range input (Stage 2).
- AST → HTML rendering (Stage 3, optional).

### Non-goals
- Byte-level MediaWiki/Parsoid compatibility; full template/Lua expansion; editing wikitext.
