# wikrs

**Fast, honest wikitext extraction and parsing — in Rust.**

> **Status: 🚧 Stage 2 engine is the default.** wikitext → clean text (or a structured AST) with **honest diagnostics**, ~32× WikiExtractor, CLI, tested — **validated on the full English Wikipedia** (7.19M articles, 98.0% clean conversion, **7.4 minutes** on a laptop with parallel multistream decoding). ~49% of MediaWiki parserTests parse with zero diagnostics, climbing. Not yet on crates.io.

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

`--format text` (default) emits one article's plain text per record; `--format jsonl` emits `{"title":…,"text":…}` per line. Both `.xml` and multistream `.xml.bz2` are accepted. The default **`ast`** engine (Stage 2 parser: structured, honest diagnostics) handles real articles; pass `--engine strip` for the Stage 1 fast/lossy path.

The CLI **streams** the dump in bounded batches — memory is bounded regardless of dump size — and a dump read/decode error is a **hard error with a real exit code**, never a silently skipped page. For a *multistream* dump, pass the companion index via `--index` to decode the bz2 streams **in parallel**:

```bash
./target/release/wikrs --input enwiki-…-multistream.xml.bz2 \
    --index enwiki-…-multistream-index.txt.bz2 --stats
```

The full English Wikipedia (7.19M articles, 26.4 GB `.bz2`) runs end-to-end in **~7.4 minutes on an Apple-Silicon laptop** with `--index` (5.1×, ~560 MB peak RAM), or ~38 minutes single-stream without it (~210 MB peak) — identical output either way.

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
| **2** | Structured AST + diagnostics — preserves structure, warns on pathological input | 🛠 in progress (~49% coverage; **now the CLI default**) |
| **3** | *(optional)* AST → HTML rendering | 💤 later |

The headline metric: a **precision/coverage differential vs Parsoid** on real pages (now landing — see [Benchmarks](#benchmarks--test-status)), plus a clear-eyed account of the rest. See [docs/TESTING.md](docs/TESTING.md).

## Benchmarks & test status

> Kept current on every change via the project's `wikrs-dev-workflow` skill. Methodology: [docs/TESTING.md](docs/TESTING.md).

_Last updated: 2026-07-01_

- **Tests:** green — `cargo test --all-features`
- **⚡ vs WikiExtractor** (end-to-end, identical `bench-compare` harness): on the **full real simplewiki dump** (1.67 GB, 281,799 articles, 2026-06 snapshot, 10-core Apple Silicon) wikrs is **~32× faster** — 5.2 s / **322 MB/s** vs WikiExtractor 164 s / 10.2 MB/s. Single-core, wikrs is **~150 MiB/s** end-to-end (it parallelises across cores; WikiExtractor streaming to one stdout does not). On the original 8.3 MB *synthetic* dump the figure was ~22× — **conservative, not inflated**: tiny inputs are dominated by wikrs's process-start overhead (a 16 MB slice shows only ~5× for the same reason), so the real full-dump gap is *wider*. **Fairness:** WikiExtractor ran with its **default parallelism** (`cpu_count() − 1` = 9 worker processes on the test machine; we verified its default beats `--processes 1` by ~2.2×), so this is parallel-vs-parallel on identical hardware, not wikrs-parallel vs WE-single-core. Reproduce: `cargo xtask bench-compare <dump.xml>` (real) or `cargo xtask make-sample-dump && cargo xtask bench-compare target/bench-dump.xml` (synthetic).
- **Sample-article throughput** (criterion, `benches/compare.rs`):

  | Implementation | Throughput | Notes |
  |---|---|---|
  | `wikrs` AST path (parse→plain, **default**) | ~119 MiB/s | Stage 2 engine — **≈ strip throughput** while also emitting diagnostics; structured where it can, strip-fallback for Unsupported blocks |
  | `wikrs::extract::strip` | ~122 MiB/s | Stage 1 extractor → clean text (five allocating passes) |
  | `parse_wiki_text` (reference) | ~306 MiB/s | community Rust parser → AST (no text out), 2018 |

  > The Stage 2 **AST path** (parse → plain text) runs at **roughly the same throughput as the Stage 1 `strip`** while producing both text **and** diagnostics — the af0c5f0 DoS-robustness fix traded ~10% AST throughput for linear-on-adversarial-input safety, which is why it lands on par with strip rather than ahead of it. It **does not expand templates** — it drops them with a `W-TEMPLATE` warning and keeps the surrounding prose. Expanding templates (à la Bliki) would mean a Lua/Scribunto engine and ~2 orders of magnitude slower (Bliki runs at ~0.4 MB/s) — surrendering the one advantage wikrs has. So: honest drop + flag, keep the speed.

  Run it yourself: `scripts/bench.sh`.
- **Conversion rate (residual-markup floor):** on the **full English Wikipedia** (enwiki 2026-06, **7,189,653 articles**, 26.4 GB `.bz2` streamed whole) **98.0% of pages convert clean** — no leaked `{{`/`[[`/`{|` markup in the output (`wikrs --input dump.xml.bz2 --stats`; 7.4 min with `--index` parallel decoding, 38 min single-stream; zero crashes). The full **simplewiki** dump (281,799 articles) measures an identical **98.0%**, so the rate generalizes across corpora rather than being tuned to one wiki. On the 1,077 synthetic parserTests cases it's **98.1%** (`cargo test --test parser_tests stage1_conversion_rate`). This is a *leniency floor* — it catches markup that **leaked**, not correctness-vs-Parsoid (that's the Stage 2 differential below). Fixing the dominant **`]]`** leak — File/image captions with a nested `[[wikilink]]`, where flat matching closed the media link at the *inner* `]]` and leaked the caption tail + outer `]]` — plus a brace-aware table-depth counter (so a `{{frac|1|12|}}` in a cell no longer fragments the table) took clean conversion from 91.9% to **98.0%** (measured on simplewiki, before the enwiki run). The residual is led by **`|}`** (1.2%) — overwhelmingly colspan/rowspan **grid tables that wikrs deliberately flags** (`U-TABLE`) rather than silently flatten, not corruption.
- **Stage 2 parser coverage** (parserTests, 1077 cases): **49.1%** parse with **zero diagnostics** — fully inside the engine's declared support range (paragraphs, headings, bold/italic, internal + external links, flat, nested & definition lists, preformatted blocks, simple tables, refs/nowiki/comments, inline HTML formatting tags, presentational HTML containers `<div>`/`<center>`/`<blockquote>`/`<p>`, shown transclusion tags `<noinclude>`/`<onlyinclude>`, and HTML lists `<ul>`/`<ol>`/`<li>` unwrapped to their text). Inline templates are **dropped with a `W-TEMPLATE` warning** (prose kept, honestly flagged → *not* counted as fully supported). Track: `cargo test --test parser_tests stage2_coverage_rate`.
- **Stage 2 differential — the "three numbers"** (layer 2 of [docs/TESTING.md](docs/TESTING.md); the headline Stage-2 DoD): wikrs's extracted prose vs **Parsoid's** rendered HTML over a fixed, committed sample of real pages. Seed run (**18 featured-class articles**, fetched 2026-06-27):
    - **99.7% word-precision** (93.2% strict phrase-precision) — of the words wikrs emits, 99.7% are corroborated by the article (**18/18 pages fully faithful, zero silent**). The phrase/word gap is table-cell reordering (same words, different adjacency than Parsoid's grid), which order-robust word-precision correctly treats as faithful — not garbling.
    - **~49% coverage** — wikrs extracts ~half of each article's prose; the rest is **template-expanded content dropped by design** (the speed moat, made measurable).
    - **100% transparently reported** — every real article trips ≥1 out-of-range construct (`{|` table, `<math>`, gallery) and wikrs **flags each** rather than silently skipping. This *is* the honest contrast with WikiExtractor's silent errors — not a failure.

    Reproduce: `cargo xtask diff-fetch && cargo xtask diff-report` (pages cached gitignored; only the names-only title lists are committed). **Representative evidence** — on **120 random ns0 pages** (`cargo xtask diff-sample`): **99.3% word-precision** (order-independent), **0% silent structural-diff**, **96% of pages fully faithful** (115/120). The strict 3-gram phrase-precision is 91.3% — the gap is table-cell reordering, which order-robust word-precision treats as faithful. **Zero silent errors** is the headline: across 120 real pages wikrs never emits content absent from the article; the rest is honestly flagged (Reported). A real sample now — not yet N-thousand, but evidence.
- **Backward-compatibility ratchet:** the **529** cleanly-passing cases are pinned by name in [`tests/coverage_baseline.txt`](tests/coverage_baseline.txt) (names only — derived facts about wikrs, not the GPL fixture). `cargo test --test parser_tests coverage_ratchet` **fails if any pinned case regresses**, so coverage can only ratchet *up* and every change to it is a deliberate, reviewed baseline diff. The single coverage *percentage* can rise while individual cases silently break; this catches that. Re-bless an intended change: `BLESS_COVERAGE=1 cargo test --test parser_tests coverage_ratchet`.
- **Robustness:** `strip` never panics and stays linear — 2 MB of adversarial input in ~150 ms (`tests/robustness.rs`, runs in CI). Deeper fuzzing: `cargo +nightly fuzz run strip`.

## Documentation

| Doc | Contents |
|-----|----------|
| [docs/DESIGN.md](docs/DESIGN.md) | Architecture, module layout, I/O contracts, error philosophy, non-goals (English) |
| [docs/TESTING.md](docs/TESTING.md) | Four-layer test strategy + benchmarks (English) |
| [docs/stages/](docs/stages/) | Per-stage checkpoints and tasks *(internal dev history, Chinese)* |
| [docs/PROJECT-HANDOFF.md](docs/PROJECT-HANDOFF.md) | Strategic context & decision log *(internal dev history, Chinese)* |
| [WORKLOG.md](WORKLOG.md) | Per-change evidence log — every fix with its measured before/after *(internal dev history, Chinese)* |

## Status & contributing

Pre-1.0 and moving fast — the design docs above are the source of truth. No API stability guarantees yet. Issues and discussion welcome.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
