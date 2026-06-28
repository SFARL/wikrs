# Template-Fragmentation Leak — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **PROJECT WORKFLOW:** This repo's required loop is `wikrs-dev-workflow` — TDD test-first, then bench, and "done" means tests pass AND `wikrs_ast` throughput has not silently regressed, then record in WORKLOG.md + refresh README. Commits follow the project convention (one logical change, ≤20-char subject carrying the `wikrs_ast` MB/s number, WORKLOG bundled, `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`) — so the single commit lands in Task 4 *after* the bench, not per-task.

**Goal:** Make `blocks()` brace-aware so a multi-line `{{…}}` template (with internal blank lines) stays one block and is dropped cleanly, eliminating the raw-markup leak that currently tanks precision (10/120 random pages, as low as 6.4%) and false-flags U-TABLE.

**Architecture:** Single localized change to `src/parser.rs::blocks()`: track `{{` nesting depth across the line loop (same ordered logic as `template_end`/`strip_inline_templates`); a blank or heading line ends a block only at depth 0. No change to `render.rs` or the borrow/span model — blocks still slice the original `&str`.

**Tech Stack:** Rust, `cargo test` (inline `#[cfg(test)]` modules), `cargo bench --bench compare` (criterion), `cargo xtask diff-report` (differential acceptance).

**Spec:** `docs/superpowers/specs/2026-06-28-template-fragmentation-leak-design.md`

---

### Task 1: Brace-aware block splitter (the fix)

**Files:**
- Modify: `src/parser.rs` — `blocks()` (currently lines 58-91); add a private `update_brace_depth` helper next to it.
- Test: `src/parser.rs` — `#[cfg(test)] mod tests` (the existing module at ~line 587; `use super::*` and `use crate::render` are already imported).

- [ ] **Step 1: Write the failing tests**

Add these three tests inside `mod tests` in `src/parser.rs`:

```rust
#[test]
fn blocks_keeps_multiline_template_whole() {
    // A {{…}} with an internal blank line must stay ONE block, not fragment.
    let wt = "{{infobox\n|a=1\n\n|b=2\n}}";
    let bs = blocks(wt);
    assert_eq!(bs.len(), 1, "expected one block, got {bs:?}");
    assert_eq!(bs[0].1, "{{infobox\n|a=1\n\n|b=2\n}}");
}

#[test]
fn blocks_still_splits_normal_paragraphs() {
    // Regression guard: prose with no open template still splits on blank lines.
    let bs = blocks("Para one.\n\nPara two.");
    assert_eq!(bs.len(), 2, "got {bs:?}");
    assert_eq!(bs[0].1, "Para one.");
    assert_eq!(bs[1].1, "Para two.");
}

#[test]
fn multiline_template_is_dropped_not_leaked() {
    // A {{#invoke:…}} with internal blank lines used to fragment, leak its body
    // as text, and false-flag U-TABLE. It must now drop cleanly: no leak, a
    // W-TEMPLATE warning, and NO U-TABLE.
    let wt = "Intro.\n\n{{#invoke:Sports table|main\n|name_A=Alpha\n\n|win_A=2 |loss_A=0\n}}\n\nOutro.";
    let p = parse(wt);
    let text = render::plain(&p.nodes);
    assert!(!text.contains("{{"), "leaked template markup: {text:?}");
    assert!(!text.contains("name_A"), "leaked template param: {text:?}");
    assert!(text.contains("Intro."), "lost prose: {text:?}");
    assert!(text.contains("Outro."), "lost prose: {text:?}");
    let codes: Vec<&str> = p.diagnostics.iter().map(|d| d.code).collect();
    assert!(codes.contains(&"W-TEMPLATE"), "expected W-TEMPLATE, got {codes:?}");
    assert!(!codes.contains(&"U-TABLE"), "false U-TABLE flag, got {codes:?}");
}
```

- [ ] **Step 2: Run the tests, verify they fail the right way**

Run: `cargo test --lib parser::tests 2>&1 | tail -25`
Expected: `blocks_keeps_multiline_template_whole` FAILS (old splitter returns 2 blocks), `multiline_template_is_dropped_not_leaked` FAILS (output contains `{{`/`name_A`, and a `U-TABLE` is present). `blocks_still_splits_normal_paragraphs` PASSES (no behavior change for brace-free input).

- [ ] **Step 3: Implement the fix**

In `src/parser.rs`, replace the body of `blocks()` (lines 58-91) with the brace-aware version, and add the `update_brace_depth` helper immediately after it:

```rust
/// Split into blank-line-separated blocks, each tagged with its start offset.
/// Brace-aware: a blank or heading line *inside* an open `{{…}}` template does
/// NOT end the block — otherwise a multi-line template (big infobox,
/// `{{#invoke:…}}`) fragments, defeats template-dropping, and leaks its body.
fn blocks(s: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut off = 0;
    let mut brace_depth = 0usize;
    for line in s.split_inclusive('\n') {
        let here = off;
        off += line.len();
        let content = line.trim_end_matches('\n');
        let at_top = brace_depth == 0;
        // A heading line is only a real heading at top level; inside a template
        // `== x ==`-looking text is template content, not a section break.
        let is_heading = at_top && heading_parts(content).is_some();
        if at_top && (content.trim().is_empty() || is_heading) {
            if let Some(st) = start.take() {
                let block = s[st..here].trim_end_matches('\n');
                if !block.is_empty() {
                    out.push((st, block));
                }
            }
            if is_heading {
                out.push((here, content));
            }
        } else if start.is_none() {
            start = Some(here);
        }
        brace_depth = update_brace_depth(brace_depth, content);
    }
    if let Some(st) = start {
        let block = s[st..off].trim_end_matches('\n');
        if !block.is_empty() {
            out.push((st, block));
        }
    }
    out
}

/// Net `{{`/`}}` nesting change across one line, scanned left-to-right — the
/// SAME ordered logic as `template_end`/`strip_inline_templates` so the splitter
/// and the stripper always agree where a template ends. Each `{{` is +1, each
/// `}}` a saturating −1 (a stray `}}` in prose can't underflow). Linear.
fn update_brace_depth(mut depth: usize, line: &str) -> usize {
    let b = line.as_bytes();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'{' && b[i + 1] == b'{' {
            depth += 1;
            i += 2;
        } else if b[i] == b'}' && b[i + 1] == b'}' {
            depth = depth.saturating_sub(1);
            i += 2;
        } else {
            i += 1;
        }
    }
    depth
}
```

- [ ] **Step 4: Run tests, verify green**

Run: `cargo test --lib 2>&1 | tail -15`
Expected: all `parser::tests` pass (including the 3 new ones) and the whole lib suite is green — in particular the existing `isolates_headings_without_blank_lines` still passes (brace-free input ⇒ depth stays 0 ⇒ unchanged behavior).

Then: `cargo clippy --all-targets 2>&1 | tail -5` — expected: no warnings.

(No commit yet — per the project workflow, the single commit lands in Task 4 after the bench.)

---

### Task 2: Robustness guard for the brace-aware path

**Files:**
- Modify: `tests/robustness.rs` — add a case to `parser_does_not_panic_on_adversarial_input` (the array around lines 49-63).

- [ ] **Step 1: Add the adversarial case**

In the `cases` array inside `parser_does_not_panic_on_adversarial_input`, add this line (e.g. after the `"{|".repeat(100_000)` entry):

```rust
        "{{\n\n".repeat(50_000),       // open templates + blank lines (brace-aware split path)
```

Rationale: this is the exact shape the new brace-awareness reasons about — unbalanced `{{` interleaved with blank lines. Depth rises and never returns to 0, so the whole input collapses into one block; `update_brace_depth` scans it once (O(n)) and `strip_inline_templates` fails to strip it once (O(n)). Must stay linear and not hang.

- [ ] **Step 2: Run robustness, verify green and linear**

Run: `cargo test --test robustness 2>&1 | tail -12`
Expected: all robustness tests PASS; `parser_does_not_panic_on_adversarial_input` finishes well under its 30 s assertion (the new case adds milliseconds, not seconds). If it hangs, the depth tracking is non-linear — stop and debug (see `superpowers:systematic-debugging`).

---

### Task 3: Verify — full suite + bench gate + differential acceptance

This is the `wikrs-dev-workflow` "done" gate: tests pass AND throughput has not silently regressed, with measured differential proof the leak is fixed.

- [ ] **Step 1: Full test suite**

Run: `cargo test 2>&1 | tail -20`
Expected: every target green (lib, robustness, parser_tests, cli, dump, snapshots, diff_report).

- [ ] **Step 2: Throughput gate (D4 — no silent regression)**

Run: `cargo bench --bench compare 2>&1 | grep -E "(sample_article/(wikrs|parse)|thrpt:)"`
Trust gauge: `wikrs_strip` must read its healthy ~120–126 MiB/s — if it reads ~74, the machine is thermally degraded; rerun cold. With strip healthy, confirm `wikrs_ast` is within noise of its current ~135 MiB/s baseline (brace counting is a cheap per-line scan; expect no measurable change). Record the `wikrs_ast` number for the commit subject + WORKLOG. If ast drops clearly below ~130 with strip healthy, investigate before proceeding.

- [ ] **Step 3: Differential acceptance (the leak is gone)**

The 120-page random cache is at `target/diff-cache-random/` (re-fetch if absent: `cargo xtask diff-fetch --titles tests/diff/titles-random.txt --out target/diff-cache-random`).

Run: `cargo xtask diff-report --cache target/diff-cache-random --show 6 2>&1 | tail -25`
Expected vs. the pre-fix baseline (precision 88.5%, word 97.7%, coverage 32.2%, faithful 106/120, 0% silent): **mean precision rises**, the catastrophic pages (`2010-11 Maltese…` 6.4%, `INS_Prachand` 20%) recover, and **word-precision and `0% silent` do NOT regress**.

Then confirm the raw-leak count is zero:
```bash
leak=0; for f in target/diff-cache-random/*.wikitext; do \
  cargo run --quiet --example show_page "$f" 2>/dev/null | sed -n '/RENDERED PLAIN/,$p' | grep -q '{{' && leak=$((leak+1)); done; echo "pages leaking {{: $leak / 120"
```
Expected: **`0 / 120`** (was 10/120). If any page still leaks, it's a template form the brace logic missed — debug before recording.

---

### Task 4: Record results + commit

**Files:**
- Modify: `WORKLOG.md` (append entry), `README.md` (results table, if it cites the differential numbers).
- Add: `examples/diag_tally.rs`, `examples/show_page.rs` (the acceptance-measurement dev tools, already written).

- [ ] **Step 1: Append the WORKLOG entry**

Add a `## [2026-06-28]` entry at the **bottom** of `WORKLOG.md` (append-only, newest last) covering: the bug (multi-line template fragmentation leak, 10/120 random pages, precision→6.4%, masquerading as U-TABLE), the root cause (`blocks()` not brace-aware), the fix (brace-aware splitter, `update_brace_depth`), the bench number from Task 3 Step 2 (and the strip trust-gauge reading), and the differential before/after from Task 3 Step 3 (precision delta, leaks 10→0, 0% silent held).

- [ ] **Step 2: Refresh the README results table if affected**

Run: `grep -nE "precision|coverage|silent|faithful|differential" README.md`
If the results table cites the random-sample differential numbers (precision/coverage/silent), update them to the post-fix values measured in Task 3. If it doesn't reference them, leave it unchanged and note that in the commit body.

- [ ] **Step 3: Commit (project convention)**

Stage the specific files (NOT `-A`) and commit with the bench number in the subject:
```bash
git add src/parser.rs tests/robustness.rs WORKLOG.md README.md examples/diag_tally.rs examples/show_page.rs
git commit -m "$(cat <<'EOF'
tmpl-leak fix <AST>MB/s

Make blocks() brace-aware: a blank/heading line inside an open {{…}} no longer
ends the block, so multi-line templates (big infoboxes, {{#invoke:…}}) stay
whole and are dropped cleanly instead of fragmenting and leaking their body.
Fixes the random-sample precision killer (10/120 pages leaked {{; precision as
low as 6.4%; masqueraded as U-TABLE). Differential: precision <before>→<after>,
leaks 10→0, 0% silent held. wikrs_ast <AST> MiB/s (strip gauge <STRIP>, no
regression). Adds examples/{diag_tally,show_page}.rs acceptance tooling.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```
Replace `<AST>`, `<STRIP>`, `<before>`, `<after>` with the Task 3 numbers. Keep the subject ≤20 chars (`tmpl-leak fix 158MB` fits).

- [ ] **Step 4: Push**

Run: `git push origin main 2>&1 | tail -3`
Expected: fast-forward push succeeds (these are new local commits on `main`; no force).

---

## Self-Review

**Spec coverage:**
- Root cause (splitter not brace-aware) → Task 1 fix. ✓
- Approach A (brace-aware `blocks()`, ordered depth logic) → Task 1 Step 3 (`update_brace_depth` mirrors `template_end`). ✓
- Edge: unclosed `{{` collapses to one block, stays linear → Task 2 robustness guard. ✓
- Edge: heading inside template not promoted → Task 1 `is_heading = at_top && …`. ✓
- Scope: `render.rs` untouched, `{|` out of scope → no task touches them. ✓
- Testing/acceptance (unit, integration, differential re-run, robustness, bench) → Tasks 1-3. ✓
- Record (WORKLOG + README) → Task 4. ✓

**Placeholder scan:** Code blocks are complete; the only fill-ins are the measured numbers (`<AST>`/`<STRIP>`/`<before>`/`<after>`) which are intentionally captured at execution time in Task 3 and substituted in Task 4 — not design gaps.

**Type consistency:** `blocks()` returns `Vec<(usize, &str)>` (used as `bs[0].1` in tests, matches). `update_brace_depth(usize, &str) -> usize` called as `brace_depth = update_brace_depth(brace_depth, content)`. `heading_parts`, `parse`, `render::plain`, `Diagnostic.code` all exist as used. Helper name consistent throughout.
