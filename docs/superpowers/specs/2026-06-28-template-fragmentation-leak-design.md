# Design — Fix multi-line template fragmentation leak

**Date:** 2026-06-28
**Status:** approved (brainstorming) → ready for writing-plans
**Scope:** `src/parser.rs::blocks()` only. `render.rs` untouched.

## Problem

On the representative random differential sample (120 ns0 pages, `tests/diff/titles-random.txt`), wikrs's AST engine leaks **raw template markup into its plain-text output**. Measured with `examples/diag_tally.rs` + `examples/show_page.rs` on `target/diff-cache-random/`:

- **10/120 pages emit `{{` into their output** (clean output never contains `{{`).
- These leaks drag mean precision to 88.5% and tank individual pages: `2010-11 Maltese…Knock-Out` **6.4%**, `INS_Prachand` 20%.
- The leak **masquerades as `U-TABLE`**: 8 of 25 U-TABLE pages have *zero* real `{|` tables — the diagnostic misattributes the cause, which nearly led us to build a table parser instead of fixing the real bug.

Example leaked output (Maltese page), raw template body emitted verbatim:

```
{{#invoke:Sports table|main|style=WDL
|name_RBT=Rabat Ajax
|win_RBT=2 |draw_RBT=1 |loss_RBT=0 |gf_RBT=5 |ga_RBT=2
```

## Root cause

`blocks()` (src/parser.rs:58) splits the document into blank-line-separated blocks and is **not brace-aware**. A `{{…}}` template that contains blank lines — common in large infoboxes and `{{#invoke:Sports table|…}}` — is **fragmented** at those blank lines:

- The first fragment ends with an **unclosed `{{`** (its matching `}}` is in a later fragment).
- Later fragments are bare `|param=value` lines.

Both downstream consumers handle a *whole* `{{…}}` correctly but fail on a fragment:

- `strip_inline_templates()` (used to sanitize a block before classification) brace-matches `{{`→`}}`; with no `}}` in the fragment it strips nothing.
- The first fragment then trips `unsupported_reason()` (its `|`-prefixed lines → `U-TABLE`), becomes `Node::Unsupported(block)`.
- `render::plain` renders `Node::Unsupported` via `extract::strip_raw` (render.rs:57), which *also* brace-matches and *also* can't remove the unclosed `{{` → **the body leaks as text**.

So the bug is entirely in the splitter fragmenting templates; every downstream stage is correct on whole templates.

## Approaches considered

- **A — brace-aware `blocks()` splitter (CHOSEN).** Track `{{` depth while scanning lines; a blank/heading line ends a block only when depth is balanced. The whole template stays in one block and is stripped cleanly. Localized to one function; preserves the borrow/`span` model (blocks still slice the original `&str` with correct offsets); stays linear.
- **B — document-level template pre-pass.** Remove all `{{…}}` from the whole document before splitting. Conceptually clean, but it *shifts every byte offset* (so `W-TEMPLATE` and future diagnostics lose their source location), forces a whole-document allocation, and breaks the zero-copy borrow model. Rejected: too invasive for the span/diagnostic model.
- **C — post-split fragment merge.** After splitting, detect a block with unbalanced open `{{` and merge following blocks until braces balance. Re-implements brace tracking at the wrong layer; hacky. Rejected.

## Design (Approach A)

Modify **only** `blocks()`:

1. Maintain a running `brace_depth: usize` across the line loop.
2. For each line, scan its bytes left-to-right and update depth **in order**: `{{` → `depth += 1` (advance 2), `}}` → `depth = depth.saturating_sub(1)` (advance 2), else advance 1. This is the *same ordered nesting logic* as `strip_inline_templates` / `template_end`, so the splitter and the stripper always agree on where a template ends. `saturating_sub` floors at 0 so a stray `}}` in prose can't underflow. (Ordered processing is required, not batch "add all opens then subtract all closes": a line `}}{{` at depth 0 must end at depth 1, not 0.)
3. The existing block-ending condition (`content.trim().is_empty() || is_heading`) gains **`&& brace_depth == 0`**: a blank line or heading line inside an open template is treated as template content, not a block boundary.
4. Everything else in `blocks()` is unchanged — blocks are still `&str` slices of the source with their start offset.

Net effect: a multi-line `{{…}}` (with internal blank lines) is delivered to `parse()` as **one block**, where `strip_inline_templates` removes it cleanly → the block becomes empty/prose → no false `U-TABLE`, no `Node::Unsupported`, no leak. The `W-TEMPLATE` warning still fires (the whole template is in one block; `block.contains("{{")` holds) with a correct span.

### Edge cases

- **Unclosed `{{` (malformed / DoS input like `{{`×N):** depth stays > 0, so no later blank line ends a block → the remainder collapses into one large block. This is acceptable: still linear, still flagged, never panics. The robustness suite must stay green.
- **`{{` inside `<nowiki>` or `<!-- comment -->`:** naive counting miscounts it. This exactly matches the existing behavior of `strip_inline_templates` (which also counts braces naively), so it introduces no new inconsistency. Documented as a known minor limitation; YAGNI.
- **Headings inside a template:** gating the heading-split on `depth == 0` means a `== x ==`-looking line inside an open template is no longer mis-promoted to a one-line heading block. Correct and consistent with the blank-line gate.

### Out of scope

- `render.rs` / `Node::Unsupported` behavior (per scope decision: real `{|` tables still fall back to `strip_raw`).
- `{|` tables that span blank lines (belongs to the separate table-extraction work).
- nowiki/comment-escaped braces (known limitation above).

## Testing / acceptance

- **Unit (`blocks()`):** a `{{infobox … }}` with an internal blank line yields exactly **one** block; a normal blank-line-separated paragraph pair still yields two.
- **Integration (`parse()`):** a Maltese-style `{{#invoke:…}}`-with-blank-lines input → rendered output contains **no** `{{`, emits a `W-TEMPLATE` warning, and emits **no** `U-TABLE` diagnostic.
- **Differential acceptance:** re-fetch + re-run `diff-report --cache target/diff-cache-random` on the 120 random pages → the 10 `{{`-leaking pages drop to **0**, the catastrophic-precision pages (6.4%, 20%) recover, mean precision rises; word-precision and `0% silent` must not regress.
- **Robustness gate:** `tests/robustness.rs` stays green (linear, no panic) — brace counting is O(n).
- **Performance gate (D4):** `cargo bench --bench compare` shows no `wikrs_ast` throughput regression beyond noise (strip as the thermal control). Brace counting is a cheap per-line addition.

## Risks

- **Linearity (D4):** brace counting is a single extra pass folded into the existing line loop — O(n), no re-scan. Must verify no measurable throughput hit.
- **Over-merging on malformed input:** bounded by the robustness suite; one big block is acceptable and linear.
- **Span correctness:** blocks still slice the original string, so spans/offsets remain exact (unlike approach B).
