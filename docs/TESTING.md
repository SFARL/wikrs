# wikrs Testing Strategy

**Status:** Draft · **Date:** 2026-06-23 (translated & refreshed 2026-07-01)

> **The goal is not "converge on 100% MediaWiki equivalence" (a dead end) — it is "high fidelity inside a declared range + explicit errors outside it."**
> The test system is the project's lifeline, built from week one: it verifies correctness *and* directly produces the reputation evidence in the README.

---

## The headline formula

> **"X% structurally faithful across N thousand random English Wikipedia pages"** + a clear-eyed account of the rest.

Whether that sentence stands is decided entirely by layers 1 and 2 below.

---

## The four test layers

### Layer 1 — Foundation: MediaWiki's official `parserTests.txt`

- **What:** thousands of `wikitext → expected HTML` pairs from the MediaWiki repo; machine-readable, public.
- **How:**
  1. Fetch `parserTests.txt` and parse it into test cases (each has `!! wikitext` / `!! html` sections).
  2. Run each through our engine and compare.
  3. **What passes = our declared support range; what doesn't = explicitly declared unsupported, with reasons archived.**
- **Output:** a generated support-range inventory — itself evidence of D2's honest boundaries.
- **⚠️ License:** `parserTests.txt` is **GPL** and must **never be vendored** into an MIT/Apache repo. It is downloaded at test time (`cargo xtask fetch-parser-tests` into a **gitignored** `tests/fixtures/`), never committed. See [DESIGN.md](DESIGN.md) §11.
- **Where:** `tests/parser_tests.rs` (case reader + comparison); the fixture is fetched by xtask, **not in the repo**.
- **Stage mapping:** Stage 1 can only pass plain-text-ish cases; coverage climbs once the Stage 2 AST lands. **The pass rate is the progress metric** — plus the `coverage_ratchet`, which pins every cleanly-passing case *by name* so a case can never silently regress while the percentage looks fine.

### Layer 2 — Scale validation: differential testing on real pages (the reputation evidence)

**Landed** (2026-06-27): `cargo xtask diff-fetch` + `diff-report`, core in `src/diff.rs`.

- **What:** a batch of real English Wikipedia pages; wikrs's extracted prose vs ground truth, diffed at the text level.
- **Ground truth:** the Wikipedia REST API (`/page/html/{title}`) serving Parsoid's official HTML, with visible prose extracted via `scraper`.
- **Why text-level, not DOM:** Stage 2 renders plain text only (`render::plain`); wikrs has no HTML renderer (the Stage 3 HTML plan was descoped 2026-07-02 — Stage 3 is LLM-facing output now). So the comparison is **wikrs plain text vs the visible prose of Parsoid's HTML**.
- **Diff method:** both sides normalize into 3-word shingle sets; compute **precision** (how much of wikrs's output the article corroborates) and **coverage** (how much of the article's prose wikrs reproduces).
- **Key design — precision-led, template omission not penalized:** wikrs drops templates by design (the D4 moat), so its output is a **subset** of Parsoid's prose. The headline is precision ("is what wikrs emits correct"); coverage is reported separately (the template-omission gap is **not a failure**).
- **Page buckets vs reality:** `classify` puts each page into `Faithful`/`Divergent`/`Reported`. But "Reported" is page-level — any single out-of-range construct (a `{|` table, `<math>`, a gallery) flags the whole page, and every real featured article has at least one → **the page buckets collapse to 0/0/100 on real pages**: honest but uninformative.
- **The real headline (the fidelity overlay, per-page, bucket-independent)** — seed run (18 featured articles, 2026-06-27) measured `precision ~91% / coverage ~49% / 13-of-18 faithful / 0 silent outliers`; precision is a **conservative floor** (the gap was `<math>`/entity/tokenization noise, not garbling — tight clustering, zero outliers). The page-level 0/0/100 is kept as a **transparency layer** (every page shows what was skipped — the direct contrast with WikiExtractor's silent errors).
- **Where:** `src/diff.rs` (normalization/precision/coverage/classify/Report; zero deps); `xtask diff-fetch`/`diff-report`; `tests/diff_report.rs` (offline integration smoke, no network in CI).
- **Sampling:** `tests/diff/titles.txt` (18 featured titles, names only, committed, reproducible) + `tests/diff/titles-random.txt` (random ns0 titles pinned by `cargo xtask diff-sample`). Page content (CC BY-SA) is fetched at run time into a gitignored cache, never committed. **The random sample is the honest one:** the initial 25-page random run measured precision ~82% with 40% silent structural-diff (the featured sample had been masking markup leaks on simple pages). **Those leaks are fixed** — after entity decoding, File/Category dropping, order-robust metrics, the `]]` File-caption fix, multi-line-template fragmentation, and the table brace fix, the 120-page random sample measures **99.3% word-precision, 0% silent, 115/120 fully faithful** (`cargo xtask diff-report --cache target/diff-cache-random`). Full-dump conversion rates are in the README (98.0% clean on both full simplewiki and full enwiki).

### Layer 3 — Safety net: fuzzing (`cargo-fuzz`)

- **Goal:** feed malformed wikitext; guarantee **no crashes / no hangs / no OOM**.
- **Hard bar** (benchmarked against MediaWiki itself): **2 MB of adversarial input, worst-case time linear, not quadratic.** This is the Rust-vs-Python/PHP safety story.
- **Targets:** `fuzz/fuzz_targets/parse.rs` (the default engine's full parse→render path), `fuzz/fuzz_targets/strip.rs`.
- **Run:** `cargo +nightly fuzz run parse`; CI runs the deterministic robustness suite (`tests/robustness.rs`) instead — adversarial patterns with linearity time-bounds.
- **Any crash is a P0**: panics/timeouts go into the corpus as regressions. (The parse target found a real UTF-8 slice panic in its first hour; fixed, then soaked 26M executions clean.)

### Layer 4 — Regression protection: snapshot tests (`insta`)

- **Goal:** never break what already works.
- **How:** a set of representative wikitext fragments (links, tables, lists, nested templates, refs, pathological input) locked with `insta::assert_snapshot!`.
- **Where:** `tests/snapshots/`. Review changes with `cargo insta review`.

---

## Supporting layers (not counted in the four, but required)

### Unit tests
Every module tests in place: `dump` (small XML in; page splitting / redirect skipping / ns filtering out), `tokenizer`, `parser`, `extract`, `diag`. `dump` is syntax-independent and must be testable alone.

### Benchmarks (`criterion`) — the hard evidence for the speed story
- **Where:** `benches/compare.rs`.
- **The founding comparison** (the project's reason to exist): **the same dump through wikrs vs WikiExtractor**, reporting wall-clock + MB/s + peak memory.
- See [stages/stage-1-extractor.md](stages/stage-1-extractor.md) for the benchmark tasks. If this number doesn't hold, Stage 1 isn't done.

### CI
- `cargo fmt --check` + `cargo clippy -D warnings` + `cargo test`.
- The layer-2 differential report runs on demand (slow; not per-PR).
- Fuzzing runs locally/nightly; CI runs the deterministic robustness suite.

---

## Comparison baselines

| Baseline | What it is | How to run | Status |
|----------|-----------|-----------|--------|
| **MediaWiki `parserTests.txt`** | correctness oracle (wikitext→expected HTML), **GPL** | `cargo xtask fetch-parser-tests` (not committed) → `cargo test --test parser_tests` | ✅ 1077 cases loaded; **Stage 2 zero-diagnostic coverage 49.1%** (`stage2_coverage_rate`); the HTML expectations anchor parse-side semantics only — per-case HTML equivalence died with the HTML renderer (Stage 3 re-scoped to LLM output; Markdown's external anchor is a round-trip harness instead) |
| **Full real-dump validation** | the scale layer: conversion rate / memory / throughput on real heterogeneous corpora (`--stats` residual-markup floor) | download a dump → `wikrs --input <dump.xml.bz2> [--index <ms-index>] --stats` | ✅ **full enwiki: 7,189,653 pages, 98.0% clean, 7.4 min (`--index` parallel) / 38 min (single-stream), zero crashes**; full simplewiki identically 98.0% (consistent across corpora). Surfaced and fixed: the `]]` File-caption leak, `{{…\|}}` table fragmentation, silent dump-entity loss |
| **`parse_wiki_text`** | the most serious community Rust parser (0.1.5/2018, unmaintained); speed baseline | `cargo bench --bench compare` (dev-dependency, **not shipped**) | ✅ sample article ~306 MiB/s; alongside `wikrs_strip` (~122) and `wikrs_ast` (~120) |
| **WikiExtractor** | the de-facto Python extractor; speed + behavior baseline | `tools/wikiextractor/setup.sh` (venv, **pinned to Python 3.10**) → `cargo xtask bench-compare <dump>` | ✅ full real simplewiki (1.67 GB): **wikrs ~32× faster** (322 vs 10.2 MB/s, WikiExtractor at its default 9-process parallelism; the 8.3 MB synthetic dump shows ~22× — small inputs are dominated by wikrs's startup overhead) |
| **Bliki** (Java, via XWiki) | mature wikitext→HTML engine (with **template expansion**); upstream abandoned | `tools/bliki/setup.sh` (JDK + coursier) → `cargo xtask bench-bliki` | ✅ sample ~**0.4 MB/s** (wikrs strip ~122 — roughly **300×** apart; it does more, far slower) |

> `parse_wiki_text` / WikiExtractor / **Bliki** are **dev-side comparisons only** — never shipped, their licenses never touch wikrs (Bliki's jars and build products are gitignored). parserTests is GPL, hence downloaded at test time and never vendored ([DESIGN.md](DESIGN.md) §11; prior-art table in §12).
> WikiExtractor 3.0.6 uses inline `(?i)` regex flags that error on Python 3.11+, hence the 3.10 pin (managed by uv; system Python untouched).

## Command quick reference

| Task | Command |
|------|---------|
| All unit + integration tests | `cargo test` |
| Fetch parserTests.txt (GPL, not committed) | `cargo xtask fetch-parser-tests` |
| Run parserTests loading/coverage (1077 cases) | `cargo test --test parser_tests` |
| Review snapshot diffs | `cargo insta review` |
| Fuzz the default engine | `cargo +nightly fuzz run parse` |
| Criterion benchmark (all baselines) | `cargo bench --bench compare` |
| Install WikiExtractor (Python baseline) | `tools/wikiextractor/setup.sh` |
| wikrs vs WikiExtractor end-to-end | `cargo xtask bench-compare <dump>` |
| Install Bliki (Java baseline) | `tools/bliki/setup.sh` |
| Run the Bliki benchmark | `cargo xtask bench-bliki` |
| Sample & pin random ns0 titles | `cargo xtask diff-sample --count N --out tests/diff/titles-random.txt` |
| Fetch real pages into the diff cache (gitignored) | `cargo xtask diff-fetch` |
| Differential report (precision/coverage) | `cargo xtask diff-report` |

> Commands marked `xtask` are custom dev tasks.

---

## Per-stage test gates (DoD summary)

| Stage | Must have | Bar |
|-------|-----------|-----|
| 1 extractor | unit + snapshots + **vs-WikiExtractor benchmark** | behavior aligned with WikiExtractor (itemized) and **an order of magnitude faster**, reproducibly |
| 2 AST | all four layers + the differential report | parserTests coverage on target + the precision/coverage numbers (precision-led) + fuzz clean |
| 3 LLM output | sections: unit + CLI e2e + full-dump line validation (**shipped 0.2.0**: 281,799 pages, 0 bad lines); markdown: **round-trip harness first** (render → pulldown-cmark → compare vs source AST), then fuzz the same property | sections schema exact per `stages/stage-3-llm-output.md`; markdown round-trip structural equality inside the declared range |

Detailed checkpoints live in each stage doc.
