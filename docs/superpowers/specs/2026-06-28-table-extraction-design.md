# Design — `{|` table extraction (precise subset)

**Date:** 2026-06-28
**Status:** approved (brainstorming) → ready for writing-plans
**Scope:** `src/parser.rs` — `blocks()` (table isolation) and `parse_table` (ref-span-aware cell parsing). `render.rs` / `Node::Table` unchanged.

## Problem

On the representative random sample (120 ns0 pages), **17/120 pages have real `{|` tables** that wikrs currently flags `U-TABLE` and drops. Empirically (via `examples/show_page.rs` on the cache), the bail causes are **not exotic grammar** (no colspan/nesting needed), they are:

1. **Prose/list glued to the table** with no blank line — the block doesn't start with `{|`, so `parse_table` returns `None` at its first check. Examples: heptathlon `"Prior to…\n{| class="`, X_Factor `";Colour key\n{|"` and `"'''…'''\n:{|"`.
2. **Multi-line `<ref>{{cite …}}</ref>` inside a cell** — `parse_table` splits cells per *physical line*, so a ref body spanning lines fragments the cell; `has_multiline_ref` bails defensively to avoid misreading the cite's `|` params as cells. This is the dominant cause (heptathlon 8/9 tables).
3. **Other non-markup lines inside** (true multi-line cells, multi-line non-ref templates) → the `else { return None }` bail.

**Honest payoff bound (kept explicit):** table extraction is *bounded*. Tables are a small slice of the template-capped (~32%) coverage, and many cells are **template-wrapped** (`{{flagathlete|…}}`, `{{Color box|…}}`, cite refs) which wikrs drops — so even a parsed table yields *partial* data (numbers, dates, `[[links]]`, bold survive; template-wrapped names/flags/cites do not). The goal is to extract the tables we *can* extract cleanly and shrink the bail set to genuinely-complex cases — **not** 100% table fidelity. Precision must not regress (no leaking table markup).

## Root cause

- `blocks()` isolates headings as their own blocks but **does not isolate `{|…|}` tables** — a table glued to adjacent prose (no blank line) lands in one mixed block that doesn't start with `{|`.
- `parse_table` parses **per physical line** (`block.lines()` → `strip_prefix('|'|'!')` → `split("||"|"!!")`), so any cell whose content spans physical lines (multi-line ref/template) breaks the line model; `has_multiline_ref` is the conservative guard that turns that into an honest `U-TABLE` instead of a silent mangle.

## Approaches considered

- **A — span-skipping cell detection in `parse_table` (CHOSEN for component 2).** Make row/cell delimiter detection skip over `<ref>…</ref>` (and the `[[…]]`/`{{…}}` spans `cell_content` already skips). Cells stay contiguous `&str` slices of the block → borrow-safe. Removes the `has_multiline_ref` bail. Moderate, localized rewrite of one function.
- **B — text pre-strip of refs (REJECTED).** Strip `<ref>…</ref>` from the block into an owned `String` before parsing. Conceptually simple but **breaks the zero-copy borrow model**: `Node<'a>` borrows the source, so cells can't borrow a temporary `String`; it would force owned cells or a buffer-keeping refactor of `parse()`. Rejected.
- **C — keep bailing on refs, only un-glue (REJECTED as too little).** Only fix component 1. Leaves the dominant bail (multi-line refs, 8/9 heptathlon) unaddressed. Rejected — the user chose the subset that includes ref handling.

## Design

### Component 1 — `blocks()` isolates `{|…|}` tables

`blocks()` already tracks `brace_depth` (`{{`). Add a parallel `table_depth` (`{|`/`|}`):

- A line whose trimmed start is `{|`, seen at top level (`brace_depth == 0 && table_depth == 0`), **ends the current block and begins a table block** (mirrors how a heading line forces a boundary).
- `{|` at a line start increments `table_depth`; `|}` at a line start decrements it (saturating at 0).
- While `table_depth > 0` (or `brace_depth > 0`), blank and heading lines are **not** block boundaries — the table is one unit, even across internal blank lines.
- The table block ends after the `|}` line that returns `table_depth` to 0.

Result: `prose\n{|…|}\nprose` → three blocks (prose / table / prose); `parse_table` then sees a clean `{|`-starting block. Nested `{|` (depth ≥ 2) stays inside the one outer block; `parse_table` bails on it (out of scope), staying `U-TABLE`.

### Component 2 — `parse_table` ref-span-aware cell parsing

Replace the per-physical-line cell split with delimiter detection that **skips `<ref>…</ref>` spans** (reusing the `[[…]]`/`{{…}}` depth-tracking already in `cell_content`, lifted to the row/cell level):

- Row (`|-`), cell (`|`, `!`), and inline-cell (`||`, `!!`) delimiters, and physical line breaks, are recognized **only when not inside** a `<ref>…</ref>`, `[[…]]`, or `{{…}}` span.
- Each cell is the **contiguous `&str` slice** between delimiters → `parse_inline(&tokenizer::inline(cell))`. The tokenizer drops the `<ref>`/`{{` content, so a multi-line cite inside a cell simply vanishes (consistent with inline ref handling) instead of fragmenting the row.
- `has_multiline_ref` and its call are **removed** (the span-skip makes them unnecessary).

**Borrow constraint (why span-skip, not text-strip):** `Node<'a>` borrows the source block, so every cell must be a contiguous slice of it. Span-skipping preserves that (cells are still slices); stripping refs into a new `String` would not (approach B), which is why B is rejected.

### Still bails — honest `U-TABLE` (unchanged behavior)

Nested tables (`{|` inside a cell), multi-line **non-ref** template cells, and anything the scanner can't structure → `parse_table` returns `None` → `U-TABLE`. Template-wrapped cells render empty (tokenizer drops them, existing behavior). `|+` captions and per-cell attributes are still dropped.

## Testing / acceptance

- **Unit (`blocks()`):** `"Prior.\n{| class=\"wikitable\"\n|-\n| a\n|}\nAfter."` → 3 blocks (prose / `{|…|}` / prose); a normal blank-separated pair still → 2 blocks; an internal blank line inside a table stays one block.
- **Unit (`parse_table`):** a table whose cell contains a multi-line `<ref>{{cite web |title=x\n|url=y}}</ref>` parses to a `Node::Table` (no bail), the ref is dropped, and adjacent plain cells (numbers/dates/`[[links]]`/bold) survive; a nested-`{|` cell still returns `None`.
- **Integration (`parse()`):** the heptathlon Records + Results shapes produce `Node::Table` with no `U-TABLE`, no leaked `|`/`!`/`class=`/`{|` markup in rendered output.
- **Differential acceptance:** re-run `diff-report --cache target/diff-cache-random` → `U-TABLE` page count drops from 17 (real-table pages), mean coverage ticks up, and **precision + 0% silent do NOT regress**; the `{{`-leak count stays 0 and a new table-markup-leak check (`|`/`!!`/`class=` in output) is 0.
- **Robustness:** add a malformed-table case (e.g. `"{|\n|x\n".repeat(50_000)`, unterminated `{|` flood) to `tests/robustness.rs` — linear, no panic.
- **Performance gate (D4):** `cargo bench --bench compare` — `wikrs_ast` no regression vs ~134 MiB/s (strip as thermal gauge).
- **Ratchet:** `coverage_ratchet` must not regress; any parserTests that newly pass are a deliberate, blessed baseline diff.

## Risks

- **Borrow lifetimes:** the span-skip keeps cells as slices of the block; verify no owned-string creep. If a transformation is ever needed, it must live at/above `parse()`, not inside `parse_table`.
- **Over-extraction:** a span-aware scanner could merge content a stricter parser would reject. Mitigation: bail (return `None`) on structures the scanner can't cleanly delimit, preferring an honest `U-TABLE` over a mangled table (D2).
- **Linearity (D4):** delimiter scanning with span-skipping is a single pass over the block — O(n). Confirm the robustness/bench gates.
- **Silent regression:** the differential's `0% silent` + the leak checks are the guard that a newly-parsed-but-wrong table doesn't leak markup or fabricate structure.
