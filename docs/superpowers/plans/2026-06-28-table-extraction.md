# `{|` Table Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **PROJECT WORKFLOW (`wikrs-dev-workflow`):** TDD test-first → bench → "done" only when tests pass AND `wikrs_ast` throughput hasn't silently regressed, then record in WORKLOG.md + README. **The local pre-commit gate is the full CI trio: `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test`** (CI enforces all three; fmt has bitten us). One logical commit lands in Task 4 after the bench, with the `wikrs_ast` MB/s in the ≤20-char subject and `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.

**Goal:** Extract the `{|` tables we can cleanly parse — by un-gluing tables from adjacent prose in `blocks()` and making `parse_table` skip multi-line `<ref>…</ref>` so a cited cell doesn't fragment the row — shrinking the U-TABLE bail set on the 17/120 real-table random pages without leaking markup.

**Architecture:** Two localized changes to `src/parser.rs`. (1) `blocks()` gains a `table_depth` counter: a top-level `{|` line starts a table block that accumulates until its matching `|}`, isolating it from surrounding prose. (2) `parse_table` iterates *logical* lines (a newline inside `<ref>…</ref>` doesn't split) so multi-line cites stay in their cell; the conservative `has_multiline_ref` bail is removed. Cells stay contiguous `&str` slices — the zero-copy borrow model is preserved. `render.rs`/`Node::Table` unchanged.

**Tech Stack:** Rust, inline `#[cfg(test)]` tests, `cargo bench --bench compare`, `cargo xtask diff-report`.

**Spec:** `docs/superpowers/specs/2026-06-28-table-extraction-design.md`

---

### Task 1: `blocks()` isolates `{|…|}` tables (un-glue from prose)

**Files:**
- Modify: `src/parser.rs` — `blocks()` (currently ~lines 61-98); add a private `update_table_depth` helper next to `update_brace_depth`.
- Test: `src/parser.rs` `#[cfg(test)] mod tests`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src/parser.rs`:

```rust
#[test]
fn blocks_unglue_table_from_surrounding_prose() {
    // A {| table glued to prose (no blank lines) must split into 3 blocks:
    // prose / the {|…|} table / prose — even without blank separators.
    let wt = "Prior.\n{| class=\"wikitable\"\n|-\n| a\n|}\nAfter.";
    let bs = blocks(wt);
    let texts: Vec<&str> = bs.iter().map(|(_, b)| *b).collect();
    assert_eq!(
        texts,
        vec!["Prior.", "{| class=\"wikitable\"\n|-\n| a\n|}", "After."],
        "got {bs:?}"
    );
}

#[test]
fn blocks_table_with_internal_blank_line_stays_one_block() {
    // A blank line inside the table does not split it.
    let wt = "{|\n| a\n\n| b\n|}";
    let bs = blocks(wt);
    assert_eq!(bs.len(), 1, "got {bs:?}");
    assert_eq!(bs[0].1, "{|\n| a\n\n| b\n|}");
}
```

- [ ] **Step 2: Run the tests, verify they fail**

Run: `cargo test --lib parser::tests::blocks_unglue 2>&1 | tail -15` and `cargo test --lib parser::tests::blocks_table_with 2>&1 | tail -15`
Expected: both FAIL — the current `blocks()` has no table awareness, so the first yields **one** glued block (`"Prior.\n{|…|}\nAfter."`) and the second splits at the blank line into two.

- [ ] **Step 3: Implement table isolation in `blocks()`**

Replace the body of `blocks()` with this (adds a `table_depth` counter and a leading in-table branch; everything else matches the current brace-aware version):

```rust
fn blocks(s: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut off = 0;
    let mut brace_depth = 0usize;
    let mut table_depth = 0usize;
    for line in s.split_inclusive('\n') {
        let here = off;
        off += line.len();
        let content = line.trim_end_matches('\n');
        let trimmed = content.trim_start();
        // Inside an open `{|…|}` table: accumulate every line (blank lines and
        // headings included) until the matching `|}` closes it, then emit the
        // whole table as one block. A table is a standalone unit.
        if table_depth > 0 {
            table_depth = update_table_depth(table_depth, trimmed);
            brace_depth = update_brace_depth(brace_depth, content);
            if table_depth == 0 {
                if let Some(st) = start.take() {
                    let block = s[st..off].trim_end_matches('\n');
                    if !block.is_empty() {
                        out.push((st, block));
                    }
                }
            }
            continue;
        }
        let at_top = brace_depth == 0;
        // A `{|` at top level opens a table block; a blank or heading line ends
        // the current block; a heading is additionally its own one-line block.
        let opens_table = at_top && trimmed.starts_with("{|");
        let is_heading = at_top && heading_parts(content).is_some();
        if at_top && (content.trim().is_empty() || is_heading || opens_table) {
            if let Some(st) = start.take() {
                let block = s[st..here].trim_end_matches('\n');
                if !block.is_empty() {
                    out.push((st, block));
                }
            }
            if is_heading {
                out.push((here, content));
            }
            if opens_table {
                start = Some(here);
                table_depth = update_table_depth(0, trimmed);
                brace_depth = update_brace_depth(brace_depth, content);
                continue;
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
```

Then add this helper immediately after `update_brace_depth`:

```rust
/// Net `{|`/`|}` table-nesting change across one (trimmed) line, scanned
/// left-to-right — mirrors `update_brace_depth`. Each `{|` is +1, each `|}` a
/// saturating −1. Lets `blocks()` keep a multi-line table (even with internal
/// blank lines) in one block and end it at the matching close. Linear.
fn update_table_depth(mut depth: usize, line: &str) -> usize {
    let b = line.as_bytes();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'{' && b[i + 1] == b'|' {
            depth += 1;
            i += 2;
        } else if b[i] == b'|' && b[i + 1] == b'}' {
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

Run: `cargo test --lib 2>&1 | tail -12`
Expected: all lib tests pass — the two new table tests, plus the existing `blocks_keeps_multiline_template_whole`, `blocks_still_splits_normal_paragraphs`, `isolates_headings_without_blank_lines`, and the template-leak tests (a `{{…}}` line starts with `{{`, not `{|`, so `opens_table` stays false — brace behavior is untouched).

(No commit yet — single commit lands in Task 4 after the bench.)

---

### Task 2: `parse_table` skips multi-line `<ref>` (no more `has_multiline_ref` bail)

**Files:**
- Modify: `src/parser.rs` — `parse_table` (use logical lines), **delete** `has_multiline_ref` (~lines 214-248) and its call; add `table_logical_lines` + `ref_opens_body` helpers.
- Test: `src/parser.rs` `#[cfg(test)] mod tests`.

- [ ] **Step 1: Write the failing test**

Add to `mod tests`:

```rust
#[test]
fn parse_table_handles_multiline_ref_in_cell() {
    // A cell with a multi-line <ref>{{cite …}}</ref> used to force a U-TABLE bail
    // (has_multiline_ref). It must now parse: the ref is dropped, plain cells stay.
    let block = "{| class=\"wikitable\"\n|-\n| Alpha\n| 42<ref>{{cite web |title=x\n|url=y}}</ref>\n|-\n| Beta\n| 7\n|}";
    let node = parse_table(block).expect("table should parse, not bail");
    let text = render::plain(std::slice::from_ref(&node));
    assert!(!text.contains("cite web"), "ref leaked: {text:?}");
    assert!(!text.contains("url=y"), "ref param leaked: {text:?}");
    assert!(text.contains("Alpha") && text.contains("Beta"), "lost cells: {text:?}");
    assert!(text.contains("42") && text.contains('7'), "lost data: {text:?}");
}
```

- [ ] **Step 2: Run the test, verify it fails**

Run: `cargo test --lib parser::tests::parse_table_handles_multiline 2>&1 | tail -15`
Expected: FAIL — current `parse_table` calls `has_multiline_ref(block)` (true here) and returns `None`, so `.expect(...)` panics.

- [ ] **Step 3: Implement logical-line iteration; remove the bail**

In `src/parser.rs`: **delete** the `has_multiline_ref` function (the `/// Whether a <ref>…</ref>…` doc + fn, ~lines 214-248). Add these two helpers (e.g. just before `parse_table`):

```rust
/// `block.lines()`, except a newline INSIDE a `<ref>…</ref>` does not split — so a
/// multi-line `<ref>{{cite …}}</ref>` in a cell stays in its row instead of
/// fragmenting it into bogus cells. Each returned line is a contiguous slice of
/// `block` (no trailing newline), so cells stay borrowable.
fn table_logical_lines(block: &str) -> Vec<&str> {
    let b = block.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let mut in_ref = false;
    while i < b.len() {
        if in_ref {
            if b[i] == b'<' && b[i..].len() >= 6 && b[i..i + 6].eq_ignore_ascii_case(b"</ref>") {
                in_ref = false;
                i += 6;
                continue;
            }
            i += 1;
        } else if b[i] == b'\n' {
            out.push(&block[start..i]);
            start = i + 1;
            i += 1;
        } else if b[i] == b'<' && ref_opens_body(&block[i..]) {
            in_ref = true;
            i += 1;
        } else {
            i += 1;
        }
    }
    if start < b.len() {
        out.push(&block[start..]);
    }
    out
}

/// `s` starts with `<`. True if it opens a `<ref …>` that has a body — i.e. a real
/// `<ref>` (not `<references>`) that is not self-closing (`<ref … />`). An unclosed
/// `<ref …>` counts as opening a body (it swallows to end-of-block, as the
/// tokenizer does).
fn ref_opens_body(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 4 || !b[1..4].eq_ignore_ascii_case(b"ref") {
        return false;
    }
    if !matches!(b.get(4), Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')) {
        return false;
    }
    match s.find('>') {
        Some(gt) => !s[..gt].trim_end().ends_with('/'),
        None => true,
    }
}
```

Then change `parse_table` to iterate logical lines and drop the `has_multiline_ref` guard. Replace its head:

```rust
fn parse_table(block: &str) -> Option<Node<'_>> {
    if !block.trim_start().starts_with("{|") {
        return None;
    }
    let mut rows: Vec<Vec<Vec<Node>>> = Vec::new();
    let mut current: Vec<Vec<Node>> = Vec::new();
    let mut started = false;
    for line in table_logical_lines(block) {
        let l = line.trim_start();
```

(The rest of the loop body — the `{|`/`|}`/`|+` skip, `|-` row, `!`/`|` cell `split` arms, the final `if !current.is_empty()` push, and `Some(Node::Table { rows })` — is unchanged. Only the iterator source `block.lines()` → `table_logical_lines(block)` and the deletion of the two `has_multiline_ref` lines at the top change.)

- [ ] **Step 4: Run tests, verify green + clippy**

Run: `cargo test --lib 2>&1 | tail -12`
Expected: all lib tests pass, including the new `parse_table_handles_multiline_ref_in_cell` and the existing `parses_simple_tables` and `table_with_multiline_ref_is_flagged_not_silently_mangled`.

**Required: rewrite the now-obsolete `table_with_multiline_ref_is_flagged_not_silently_mangled` test.** It asserted the *old* bail (multi-line ref table → `U-TABLE`); Task 2 makes that table parse, so the test now fails on its `assert!(…any U-TABLE…)`. Replace the whole test with this (the table parses, the ref is dropped, cell text + lead prose survive, no markup leak):

```rust
#[test]
fn table_with_multiline_ref_in_cell_parses_dropping_the_ref() {
    // A <ref> spanning lines inside a cell used to force U-TABLE (its `|`-prefixed
    // cite params looked like cells). It now parses: the ref is dropped, the cell
    // text + lead prose survive, no citation markup leaks (D2).
    let wt = "Intro prose.\n\n{|\n|-\n| Smith <ref name=a>{{cite web\n| url = http://e.com\n| title = T}}</ref>\n| 1974\n|}";
    let p = parse(wt);
    let text = render::plain(&p.nodes);
    assert!(text.contains("Intro prose"), "lost lead prose: {text:?}");
    assert!(text.contains("Smith"), "lost cell text: {text:?}");
    assert!(text.contains("1974"), "lost cell data: {text:?}");
    assert!(!text.contains("url"), "leaked cite markup: {text:?}");
    assert!(
        !p.diagnostics.iter().any(|d| d.code == "U-TABLE"),
        "table should parse now, not bail: {:?}",
        p.diagnostics
    );
}
```

(A `W-TEMPLATE` warning *will* fire here — the block contains `{{cite web` — which is fine and expected; don't assert against it.) Re-run `cargo test --lib` until green.

Run: `cargo clippy --all-targets -- -D warnings 2>&1 | tail -3`
Expected: clean (no dead-code warning — `has_multiline_ref` is fully removed, not orphaned).

---

### Task 3: Robustness case + full verify (suite + bench + differential)

The `wikrs-dev-workflow` "done" gate.

- [ ] **Step 1: Add a malformed-table robustness case**

In `tests/robustness.rs`, add to the `cases` array in `parser_does_not_panic_on_adversarial_input` (after the `"{{\n\n".repeat(50_000)` line):

```rust
        "{|\n| x\n".repeat(50_000),    // unterminated table flood (table-depth split path)
```

- [ ] **Step 2: Run robustness, verify green and linear**

Run: `cargo test --test robustness 2>&1 | tail -10`
Expected: all pass; `parser_does_not_panic_on_adversarial_input` finishes well under its 30 s bound (`update_table_depth` is one O(n) pass; an unterminated `{|` keeps `table_depth > 0` so the whole input collapses to one block — linear).

- [ ] **Step 3: Full suite + fmt + clippy (the CI trio)**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | grep -E "test result:|error"`
Expected: fmt clean, clippy clean, every target green. (If fmt complains, run `cargo fmt --all` and re-stage — see [[wikrs-run-full-ci-trio-before-commit]].)

- [ ] **Step 4: Throughput gate (D4)**

Run: `cargo bench --bench compare 2>&1 | grep -E "sample_article/(wikrs|parse)|thrpt:"`
Trust gauge: `wikrs_strip` ~120-126 MiB/s (if ~74, machine degraded — rerun cold). Confirm `wikrs_ast` is within noise of ~134 MiB/s. Record the number for the commit/WORKLOG. The new work is two more O(n) per-line scans + ref-aware line splitting on table blocks only — expect no measurable change.

- [ ] **Step 5: Differential acceptance**

Run: `cargo xtask diff-report --cache target/diff-cache-random --show 8 2>&1 | tail -26`
Expected vs the post-template-leak baseline (precision 91.0%, word 99.3%, coverage 32.0%, faithful 115/120, **0% silent**): the **U-TABLE page count drops** (re-check with `cargo run --quiet --example diag_tally target/diff-cache-random`), **mean coverage ticks up**, and **precision + word-precision + 0% silent do NOT regress**.

Then confirm no new table-markup leak (clean output never contains raw table syntax):
```bash
leak=0; for f in target/diff-cache-random/*.wikitext; do \
  ./target/debug/examples/show_page "$f" 2>/dev/null | sed -n '/RENDERED PLAIN/,$p' \
  | grep -qE '\{\||\bclass="wikitable"|!!|\|\|' && leak=$((leak+1)); done; echo "table-markup leaks: $leak / 120"
```
Expected: **0 / 120**. If any page leaks table markup, a newly-parsed table is emitting syntax — stop and debug (`superpowers:systematic-debugging`) before recording.

---

### Task 4: Record results + commit

**Files:** Modify `WORKLOG.md` (append), `README.md` (results table if differential numbers change).

- [ ] **Step 1: Append the WORKLOG entry**

Add a `## [2026-06-28]` entry at the **bottom** of `WORKLOG.md`: the goal (extract cleanly-parseable `{|` tables), the two changes (`blocks()` table isolation via `update_table_depth`; `parse_table` logical-line iteration via `table_logical_lines`, `has_multiline_ref` removed), what still bails honestly (nested tables, multi-line non-ref template cells, indented `:{|`, template-wrapped cells render empty), the bench number (Task 3 Step 4), and the differential before→after (U-TABLE page count, coverage delta, precision/word/silent held, table-markup leaks 0/120).

- [ ] **Step 2: Refresh README if differential numbers moved**

Run: `grep -nE "coverage|U-TABLE|99\.3|115/120|precision" README.md`
If the random-sample line cites coverage/faithful counts that changed, update them to the Task 3 values. Otherwise note "unchanged" in the commit body.

- [ ] **Step 3: Commit (CI trio already green from Task 3)**

```bash
git add src/parser.rs tests/robustness.rs WORKLOG.md README.md
git commit -m "$(cat <<'EOF'
table-extract <AST>MB/s

Extract cleanly-parseable {| tables. blocks() isolates {|…|} as its own block
(new update_table_depth) so a table glued to prose un-glues; parse_table iterates
table_logical_lines so a multi-line <ref>{{cite}}</ref> in a cell stays in its
row (has_multiline_ref bail removed). Cells stay borrow-safe contiguous slices;
render.rs unchanged. Honest bail kept for nested tables / multi-line non-ref
template cells / indented :{| ; template-wrapped cells render empty.

Differential (120 random): U-TABLE pages <before>-><after>, coverage <c0>-><c1>%,
precision/word/0%-silent held, table-markup leaks 0/120. wikrs_ast <AST> MiB/s
(strip gauge <STRIP>, no regression).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```
Replace `<AST>`/`<STRIP>`/`<before>`/`<after>`/`<c0>`/`<c1>` with Task 3 numbers. Subject ≤20 chars (`table-extract 134MB` fits).

- [ ] **Step 4: Push and confirm CI green**

```bash
git push origin main 2>&1 | tail -3
gh run watch "$(gh run list --limit 1 --json databaseId --jq '.[0].databaseId')" --exit-status 2>&1 | tail -15
```
Expected: push fast-forwards; CI run completes **success** (Format ✓ Clippy ✓ Test ✓).

---

## Self-Review

**Spec coverage:**
- Component 1 (blocks() isolates `{|…|}`) → Task 1 (`update_table_depth` + in-table branch). ✓
- Component 2 (ref-span-aware cells, remove `has_multiline_ref`) → Task 2 (`table_logical_lines`/`ref_opens_body`, `parse_table` uses logical lines). ✓
- Borrow-safety (cells are contiguous slices) → `table_logical_lines` returns `&str` slices; no owned strings. ✓
- Still-bails (nested, multi-line non-ref templates, indented `:{|`) → unchanged `else { return None }` in `parse_table`; documented in Task 4 WORKLOG. ✓
- Testing/acceptance (unit, integration via diff, robustness, bench, ratchet) → Tasks 1-3. ✓
- Record → Task 4. ✓

**Placeholder scan:** code blocks are complete; the only fill-ins are measured numbers (`<AST>`/`<STRIP>`/`<before>`/`<after>`/`<c0>`/`<c1>`), captured in Task 3 and substituted in Task 4 — not design gaps.

**Type consistency:** `update_table_depth(usize, &str) -> usize` matches `update_brace_depth`'s shape and its call sites. `table_logical_lines(&str) -> Vec<&str>` feeds the existing `for line in …` loop (was `block.lines()`, also `&str` items). `ref_opens_body(&str) -> bool`. `parse_table` still returns `Option<Node<'_>>`. `render::plain(std::slice::from_ref(&node))` matches `render::plain(&[Node])`. `has_multiline_ref` removed everywhere (defn + call). No dangling references.
