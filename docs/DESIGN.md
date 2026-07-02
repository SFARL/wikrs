# wikrs Design Doc

**Status:** Draft · **Date:** 2026-06-23 · **Name:** `wikrs` (crates.io availability confirmed 2026-07-01)

> **⚠️ Current-state snapshot (2026-07-01) — this document was written before implementation began; where the body disagrees with this box, the box wins:**
> Stage 1 ✅ done (kept as `--engine strip`); **the Stage 2 AST engine is the CLI default** (49.1% of parserTests parse with zero diagnostics; differential: 99.3% word-precision / 0% silent).
> The `dump` module gained XML entity (GeneralRef) handling and **parallel multistream decoding** (`open_multistream` + `--index`).
> CLI: bounded-batch streaming (O(batch) memory), hard failure on dump errors.
> **Scale validation: the full English Wikipedia — 7,189,653 pages, 98.0% clean, 7.4 minutes, zero crashes (5.1× via `--index`).**
> The current architectural truth lives in [WORKLOG.md](../WORKLOG.md) (per-change evidence, Chinese) and the README scoreboard; the rest of this document is the founding design, kept as decision background.

---

## 0. What this document is

The **single source of truth** for architecture and design decisions. After reading it you should be able to answer: what we're building, why it's layered this way, how the modules are cut, the I/O contracts, the error-handling philosophy, the performance targets, and the explicit non-goals.

- Strategic background / decision chain: [PROJECT-HANDOFF.md](PROJECT-HANDOFF.md) (internal, Chinese)
- Per-stage checkpoints and tasks: [stages/](stages/)
- Testing strategy: [TESTING.md](TESTING.md)

Only **stable architectural decisions** live here. The document evolves with the implementation, but every change leaves an entry in [../WORKLOG.md](../WORKLOG.md).

---

## 1. Goal and positioning

A wikitext processing tool in Rust:

- **The floor (near-certain):** a WikiExtractor that is an order of magnitude faster. wikitext → clean plain text, on the strength of Rust alone.
- **The ceiling (the reputation play):** a modern wikitext engine that is fast *and* accurate, preserves structure, and **warns on pathological input instead of silently dropping it**.

The moat is that the problem is "hard enough to scare everyone off" — prior art died on the combination of *correct* and *actively maintained*.

---

## 2. Core strategic decisions (non-negotiable)

| # | Decision | Rationale |
|---|----------|-----------|
| **D1** | **No byte-level MediaWiki compatibility** | The only complete spec is a 6,200-line PHP regex swamp; 100% compatibility = re-implementing all of its bugs = a dead end and a guaranteed abandoned repo. |
| **D2** | **Honest boundaries: high correctness inside a declared support range, explicit errors outside it (never silently wrong)** | The strongest technical narrative available, and the core differentiator vs WikiExtractor. |
| **D3** | **Layered delivery: ship speed first, stack structure on top** | The downside is floored (speed is near-certain); the upside stays open (parse quality carries risk). Release early, get feedback. |
| **D4** | **Speed is the safety-net axis; correctness is the upside axis** | If parsing proves harder than expected, the speed axis still holds the floor — the project cannot become a negative-reputation tombstone. |

---

## 3. Why wikitext is hard (the knot the design must face)

This is a structural problem no amount of engineering effort removes; the design has to confront it head-on:

- **The template system is a text macro processor** (think C preprocessor). Template expansion is **not guaranteed to produce self-contained DOM** — a template may emit just an opening `<table>` tag, or a lone `<tr>`.
- Therefore "parse then expand" and "expand then parse" **both fail** — the two are entangled. Even the official Parsoid (a full-time team, over a decade) never got a clean single-pass architecture past templates and fell back to calling the PHP preprocessor.

**How the design responds:**

- **Stage 1 sidesteps it:** pure text stripping, no DOM, templates handled by drop/whitelist. The knot is never touched.
- **Stage 2 parses *diagnostically*:** build an AST inside the cleanly-parseable range; on template entanglement or out-of-range structure, emit a `Diagnostic` and degrade (preserving the original source span) — **never pretend the parse succeeded**. This is D2 made concrete.

---

## 4. Architecture overview

```
                         ┌─────────────── Stage 1 (the floor) ───────────────┐
 XML dump (.bz2)         │                                                    │
 ───────────────►  dump::reader ──►  PageStream{title, ns, wikitext}          │
 single-page wikitext (stdin) ──────────────────────────┐                     │
                                                         ▼                     │
                                              extract::strip ──► plain text / JSONL
                         └──────────────────────────────────────────────────┘

                         ┌─────────────── Stage 2 (the ceiling) ─────────────┐
   wikitext ──► tokenizer ──► parser ──► AST ──► render::plain  (replaces Stage 1 strip)
                                          │  └──► render::struct (JSONL; tables/links/structure kept)
                                          └──► diag::Diagnostics (out-of-range warnings, no silent drops)
                         └──────────────────────────────────────────────────┘

                         ┌─────────────── Stage 3 (optional) ────────────────┐
                                          AST ──► render::html
                         └──────────────────────────────────────────────────┘
```

**Key point:** Stage 1's `extract::strip` is a **standalone, deliberately lossy** text stripper, not a parser — its job is to ship the speed value as early as possible. Stage 2 introduces the real tokenizer→parser→AST, and `render::plain` ultimately replaces `extract::strip`. The two are intentionally decoupled so Stage 1's fast-and-rough design can never lock in Stage 2.

---

## 5. Module / crate structure

**Starting point: a single crate `wikrs` (lib + bin).** A **minimal workspace** (root + `xtask`) exists for dev tooling (fetching parserTests, comparison benchmarks), but the library itself is one crate. Splitting into multiple crates (e.g. a reusable `wikrs-dump`) waits until the Stage 2 AST is stable and there's real demand.

Actual layout (deliberately simple; mostly single-file modules; Stage 2 modules built ✓):

```
wikrs/
├── Cargo.toml                  # lib + bin; workspace includes xtask (excludes fuzz)
├── src/
│   ├── lib.rs                  # public API root (pub mod …)
│   ├── main.rs                 # bin `wikrs`: CLI (clap + rayon)
│   ├── dump.rs                 # streaming XML dump reader (multistream .bz2)
│   ├── extract.rs (+ extract/) # Stage 1: lossy strip (comments/templates/links/markup passes)
│   ├── output.rs               # text / jsonl output
│   ├── tokenizer.rs            # Stage 2: inline tokenizer (handwritten, single-pass linear) ✓
│   ├── parser.rs               # Stage 2: block-level + inline assembly → AST + diagnostics ✓
│   ├── ast.rs                  # `Node<'a>` (Cow, borrow-friendly) ✓
│   ├── render.rs               # `render::plain` (struct/html pending) ✓
│   └── diag.rs                 # `Diagnostic` / `Severity` / error codes ✓
├── benches/compare.rs          # criterion: parse_wiki_text vs wikrs_strip vs wikrs_ast
├── fuzz/                       # cargo-fuzz targets (strip, parse; own workspace)
├── xtask/                      # dev tasks (fetch-parser-tests / bench-compare / bench-bliki / diff-*)
├── tools/                      # comparison baselines (wikiextractor / bliki; artifacts gitignored)
└── tests/                      # cli / dump_open / parser_tests(coverage) / snapshots / robustness / diff_report
```

**One responsibility per module; things that change together live together; prefer small, focused files.** `dump` is fully decoupled from wikitext syntax and independently testable. The tokenizer is handwritten (not `logos`) — full control over linear worst-case complexity and error recovery.

### Dependency choices (initial; changes recorded in the WORKLOG)

| Purpose | crate | Notes |
|---------|-------|-------|
| Streaming XML | `quick-xml` | event-based, zero-copy-friendly; multi-GB dumps must stream |
| Decompression | `bzip2` / `flate2` | dumps are multistream `.bz2`; streams can be decoded in parallel |
| Parallelism | `rayon` | per-page parallelism, saturates cores |
| Fast byte scanning | `memchr` / `bstr` | strip hot paths |
| CLI | `clap` (derive) | |
| Library errors | `thiserror` | typed errors inside the lib |
| App errors | `anyhow` | bin layer |
| Snapshot tests | `insta` | regression protection |
| Benchmarks | `criterion` | the speed numbers |
| Fuzzing | `cargo-fuzz` (libFuzzer) | no crashes / no hangs / no OOM |

> Handwritten tokenizer vs `logos` was left open here and settled at Stage 2 kickoff: **handwritten** (validated linear on adversarial input; see the robustness suite).

---

## 6. Input / output contracts

### Input
- **Wikimedia XML dump**: `pages-articles-multistream.xml.bz2`. Streamed iteration, **constant memory per page**. Defaults to namespace 0 (articles) only, skipping redirects.
- **Single-page wikitext**: stdin or `--file`, for development/debugging and library users.

### Output (switched by `--format`)
- `text` (default, Stage 1): clean plain text per article.
- `jsonl`: one `{title, text, …}` per line — the most common shape for training/RAG pipelines.
- `ast-json` (Stage 2): the structured AST.
- `html` (Stage 3).

### Filters / behavior switches
- `--namespaces 0`, `--skip-redirects` (default on), `--min-text-len`, `--templates drop|whitelist`.
- The **concrete WikiExtractor-parity behavior** (what is stripped, what is kept) is itemized in [stages/stage-1-extractor.md](stages/stage-1-extractor.md).

---

## 7. Error and diagnostic philosophy (D2 made concrete)

Never drop silently. Processing produces structured diagnostics:

```
Diagnostic {
  severity: Error | Warning | Unsupported,
  code:     &'static str,   // stable code, e.g. "E-TPL-NESTED", "U-TABLE-FROM-TEMPLATE"
  span:     Range<usize>,   // byte range into the source, locatable
  message:  String,
}
```

- **A genuine error inside the supported range** → `Error`.
- **Recoverable odd input** → `Warning`; degrade and continue.
- **A construct we declare unsupported** (e.g. a template emitting half a table) → `Unsupported`; keep the original span, keep going.

The CLI prints a summary on exit: `X pages fully clean / Y pages with Warnings / Z pages hit Unsupported`. **Those three numbers are the most persuasive reputation evidence in the README** (see TESTING.md, layer 2).

---

## 8. Performance targets and levers

- **Target:** full English dump from WikiExtractor's "hours" to "tens of minutes", benchmarkable (wall-clock + MB/s throughput + peak memory). *(Beaten in practice: 7.4 minutes with parallel multistream decoding — see the snapshot box above.)*
- **Levers:** streaming end-to-end (never load the dump whole), constant memory per page, `rayon` per-page parallelism, `memchr`/`bstr` hot paths, borrow-first `&str` AST (zero-copy where possible), no backtracking regexes.
- **Safety requirement** (benchmarked against MediaWiki itself): on 2 MB of adversarial input, worst-case time stays **linear, not quadratic**. Enforced by fuzzing and the robustness suite (TESTING.md, layer 3).

---

## 9. Naming (locked: `wikrs`)

**Status (checked 2026-06-24, re-confirmed 2026-07-01):** `wikrs` is available on crates.io; the private GitHub repo [`SFARL/wikrs`](https://github.com/SFARL/wikrs) exists; crate name matches repo name. **Locked.** `mwx` / `mwparser` / `unwiki` were also available at the time and are archived as alternates.

| Candidate | Meaning | Call |
|-----------|---------|------|
| **`wikrs`** ✅ chosen | wiki + rs | Reads instantly as "Rust wiki tool"; doesn't lock into either the extraction or the parsing layer; room to grow. |
| `mwx` / `mwparser` | mw = MediaWiki | alternates, available |
| `unwiki` | unwrap the wiki | alternate, available |

---

## 10. Non-goals (YAGNI)

Explicitly **not** doing — written into the README to manage expectations:

- ❌ Byte-level MediaWiki compatibility (D1).
- ❌ Full template expansion / Lua (Scribunto) execution.
- ❌ Visual-editor round-tripping (Parsoid's `data-*` annotations).
- ❌ Writing or editing wikitext. This project is **read-direction only**: wikitext → text/AST/HTML.

---

## 11. License and legal boundaries (important)

- **wikrs itself:** dual-licensed `MIT OR Apache-2.0` (the Rust ecosystem convention; maximizes adoption).
- **MediaWiki is GPL-2.0-or-later (copyleft),** and its `parserTests.txt` is likewise GPL. That gives D1 a **legal** reason on top of the engineering one:
  - **Never copy or transliterate MediaWiki's PHP parser code into wikrs** — GPL code in the tree would infect wikrs into GPL, conflicting with the dual license above. Clean-room reimplementation from **observed behavior / specs** only.
  - **`parserTests.txt` never enters the repo:** vendoring a GPL fixture into an MIT/Apache repo would take on GPL redistribution obligations. It is **downloaded on demand at test time** (an xtask fetches it into a gitignored directory); we do not redistribute it. See [TESTING.md](TESTING.md), layer 1.
- **Wikipedia article content** (the differential tests' ground truth) is CC BY-SA: dumps and fetched pages also **stay out of the repo** (`.gitignore` covers `*.bz2` / cache directories).

---

## 12. Prior art and competition

wikrs is not the first non-MediaWiki wikitext tool. Seeing prior art clearly *is* part of the positioning.

| Tool | Language | What it does | Completeness | Out-of-range behavior | Status |
|------|----------|--------------|--------------|----------------------|--------|
| **WikiExtractor** | Python | dump → plain text (lossy) | low | silent drops | slow maintenance |
| **parse_wiki_text** | Rust | wikitext → AST | medium (read-only) | declared boundaries | unmaintained (2018) |
| **Bliki** (`info.bliki.wiki`) | Java | wikitext → HTML (with **template expansion**/parser functions/tables/TOC/footnotes) | **high** | best-effort, silent | upstream abandoned; XWiki maintains a fork |
| wikitextparser | Python | manipulate templates/tables/links | medium | — | active |
| wikitextprocessor | Python | full template expansion + Lua | highest | — | active |
| Parsoid | PHP/JS | official wikitext↔HTML | highest (official) | falls back to the PHP preprocessor | officially maintained |

**Bliki is the one to remember:** the closest predecessor to wikrs's Stage 2/3 ambitions — a seriously complete wikitext→HTML engine (it even expands templates) — **and then upstream abandoned it**, leaving XWiki on life support. A living specimen of "prior art dies on *actively maintained*", and confirmation the gap is real.

**wikrs's differentiation is not "more features"** (today both Bliki and wikitextprocessor do more than wikrs). It is the three-way intersection almost nobody occupies at once:

1. **Rust speed + bulk extraction** (measured ~32× WikiExtractor end-to-end on the full simplewiki dump; Bliki/Parsoid don't compete in this arena);
2. **Honest diagnostics** (`Unsupported` + source span, warnings outside the declared range — none of the tools above have this);
3. **New + actively maintained** (filling the vacuum Bliki and parse_wiki_text left behind).

> Free distribution slot: after release, add wikrs to MediaWiki's [Alternative parsers](https://www.mediawiki.org/wiki/Alternative_parsers) list.
> How to run the three comparison baselines (parse_wiki_text / WikiExtractor / **Bliki**): see [TESTING.md](TESTING.md).
