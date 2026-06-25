# Bliki (comparison baseline)

[Bliki](https://github.com/axkr/info.bliki.wikipedia_parser) (`info.bliki.wiki`,
the "Java Wikipedia API") is a mature wikitext → HTML engine — it even expands
templates and parser functions. It's the closest prior art to wikrs's Stage 2/3
ambition, and notable for being **largely discontinued upstream** (XWiki keeps a
fork). We bench against it as a third baseline (after `parse_wiki_text` and
WikiExtractor) — see [docs/TESTING.md](../../docs/TESTING.md) and DESIGN §12.

## Setup

```bash
./tools/bliki/setup.sh        # needs a JDK; fetches jars via coursier
cargo xtask bench-bliki        # render the sample article N times, report MB/s
```

The jars (`lib/`) and compiled harness (`out/`) are **gitignored** — only
`BlikiBench.java` and this README are committed.

## What the number means

Bliki renders full HTML and attempts template expansion, so it does *more* work
than wikrs's Stage 1 `strip` (lossy text). It is **not** an apples-to-apples
race. Even so, on the sample article Bliki runs at **~0.4 MB/s** vs wikrs strip
**~118 MB/s** — roughly two to three orders of magnitude slower. The point isn't
"wikrs does the same thing faster" (it does less, for now); it's that the JVM,
feature-rich path is far too slow for the bulk-extraction use case wikrs targets.
