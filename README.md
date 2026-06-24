# wikrs

**Fast, honest wikitext extraction and parsing — in Rust.**

> **Status: 🚧 Early / design phase.** Documentation-first; code lands in stages (see [Roadmap](#roadmap)). Not yet released to crates.io.

---

## What is this?

The text inside a Wikipedia [XML dump](https://dumps.wikimedia.org) isn't HTML — it's **wikitext**, MediaWiki's markup language. Anyone training a model or building RAG over Wikipedia has to strip that wikitext into clean text first.

The de-facto tool for that, [WikiExtractor](https://github.com/attardi/wikiextractor) (Python), is slow and **silently** drops or mangles complex templates and tables.

**wikrs** is a Rust take, delivered in two tiers:

- **Floor — a faster WikiExtractor.** wikitext → clean plain text, roughly an order of magnitude faster (it's Rust). Drop-in for the "I just need the text" use case.
- **Ceiling — a modern wikitext engine.** A structured AST that preserves tables, link anchor text, and document structure — and that **emits a diagnostic when it hits input it can't faithfully handle, instead of silently corrupting the output.**

## Why is this hard? (and why that's the moat)

wikitext has no clean grammar. The only complete spec is MediaWiki's ~6,200-line PHP regex engine, and its template system is a *text macro processor*: template expansion isn't guaranteed to produce self-contained markup (a template can emit just an opening `<table>`, or a lone `<tr>`). So "parse then expand" and "expand then parse" both fail — they're entangled. Even MediaWiki's official Parsoid, with a full-time team over a decade, fell back to calling the PHP preprocessor.

So wikrs **does not chase byte-level MediaWiki compatibility** — that path is a tar pit where you reimplement two decades of bugs. Instead:

> **High correctness within an honestly-declared support range, and explicit diagnostics outside it.**

That honesty — telling you *exactly* what it couldn't parse — is the core difference from tools that are silently wrong.

## Non-goals

- ❌ Byte-level MediaWiki / Parsoid compatibility
- ❌ Full template / Lua (Scribunto) expansion
- ❌ Editing or emitting wikitext — wikrs is read-direction only: wikitext → text / AST / HTML

## Roadmap

| Stage | What | Status |
|------:|------|--------|
| **1** | Plain-text extractor — wikitext → clean text, benchmarked against WikiExtractor | 🔜 next |
| **2** | Structured AST + diagnostics — preserves structure, warns on pathological input | 📋 planned |
| **3** | *(optional)* AST → HTML rendering | 💤 later |

The headline metric we're building toward: **"X% structurally identical to Parsoid on N random Wikipedia pages"**, plus a clear-eyed account of the rest. See [docs/TESTING.md](docs/TESTING.md).

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

TBD — intended to be dual-licensed **MIT OR Apache-2.0** (the Rust ecosystem convention) before the first public release.
