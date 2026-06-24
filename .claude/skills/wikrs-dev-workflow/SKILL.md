---
name: wikrs-dev-workflow
description: The required development workflow for the wikrs wikitext engine. Use this whenever implementing, changing, or fixing anything that touches code in this repo — a new extract pass, dump handling, a CLI flag, a parser rule, a perf tweak, a bugfix. It enforces the loop that keeps this project honest: write the unit test first (TDD), run the benchmark, and only call the work "done" when the tests pass AND the benchmark hasn't silently regressed — then record the result in WORKLOG.md and refresh the README results table. Trigger it even when the user just says "add X" or "fix Y" without mentioning tests or benchmarks at all, because skipping the test+bench+record loop is exactly how this project's two selling points — speed and correctness — quietly erode.
---

# wikrs development workflow

wikrs is selling two numbers: **how correct it is** and **how fast it is** (see [PROJECT-HANDOFF.md](../../../docs/PROJECT-HANDOFF.md)). Both regress *silently* — a refactor that drops 8% throughput, a "small" change that breaks a strip case — unless every change is measured and the evidence written down. This workflow makes "done" always mean *tested and measured*, with the proof in a place the next person can read.

Apply it to any change under `src/` or `xtask/`: a new `extract` pass, dump parsing, a CLI flag, a parser rule, a perf tweak, a bugfix.

## The loop

For each unit of work:

1. **Write the failing unit test first.** Describe the behavior you want as a test that fails today. This is TDD — see the broader `superpowers:test-driven-development` skill if you want the full discipline. The point: a change with no test that would fail without it isn't verifiable, so it isn't done.
2. **Make it pass** with the simplest code that could work.
3. **Run the whole suite:** `cargo test --all-features`. Everything green — not just your new test. A change that fixes one thing and reddens another isn't done.
4. **Run the benchmark:** `scripts/bench.sh` (wraps `cargo bench --bench compare`).
5. **Check the done-gate** (below).
6. **Record it:** append a `WORKLOG.md` entry and refresh the README results table.
7. **Commit** code + tests + WORKLOG + README together (commit-message rules below), so the evidence travels with the change.

## The done-gate

A change is done only when **both** hold:

- ✅ **Tests pass.** `cargo test --all-features` is green, and the new behavior is covered by a test that would fail without your change.
- ✅ **The benchmark did not *silently* regress.** Default expectation: improve or hold (within noise).

### Reading the benchmark honestly

The aspiration is *faster every time* — speed is wikrs's floor value, the thing we can almost always win on. But be honest: not every change can be faster. Handling a new wikitext construct, or a correctness fix, can legitimately cost a few percent. So the real rule isn't "always faster" (that would pressure you to fake it or skip hard cases) — it's:

> **No _unexplained_ regression.** A silent slowdown is a bug. A recorded, justified one is a decision.

If a change costs performance, keep it — but say so in the WORKLOG entry: what got slower, by roughly how much, and why it's worth it. If a change is supposed to be faster and isn't, that's a signal to profile before moving on (`cargo flamegraph`).

Criterion auto-compares to your previous local run and prints a `change:` line. Benches are noisy on a busy laptop — treat anything within ~±3–5% as noise, and re-run to confirm a real move. For a clean before/after across a refactor, snapshot first: `scripts/bench.sh --save before`, do the work, then `scripts/bench.sh --baseline before`.

## Recording the result

### WORKLOG.md — append, newest at the bottom

One entry per meaningful change. Keep it skimmable:

```
## [YYYY-MM-DD] <what you built>
- **Change:** <what and why, one or two lines>
- **Tests:** <new tests added; `cargo test` result>
- **Benchmark:** <metric> <old> -> <new> (<+/-%>) — or "no perf-relevant change"
- **Regression?** none / justified: <what got slower and why it's worth it>
```

### README.md — the "Benchmarks & test status" section

Keep it current: the date, the test status, and the latest throughput number(s). This table is the project's **public scoreboard** — the honest, checkable evidence the whole pitch rests on. A stale number there is worse than no number, because it quietly lies. When Stage 2 lands the conformance harness, this section grows to include the three numbers from [docs/TESTING.md](../../../docs/TESTING.md) (`X% identical / Y% structural diff / Z% reported`).

### Git commit message

Two hard rules, so that `git log --oneline` reads as a running performance history — you can scroll the log and *see* the throughput move commit by commit:

- **Subject line ≤ 20 characters.** Terse. Drop long `type(scope):` prefixes if they don't fit — brevity wins here.
- **The subject carries the benchmark number** — the new throughput or the Δ for this change (e.g. `272MB/s` or `+9%`). For a change with no perf impact (docs, a pure bugfix), carry the latest measured number so the trail never breaks.

Everything else goes in the **body**, which is unconstrained: the full `Change / Tests / Benchmark / Regression` lines (same as the WORKLOG entry) and the `Co-Authored-By` trailer. The 20-char limit applies to the **subject only**.

**Example subjects:**
- `dump iter 314MB/s`
- `strip tpl +9%`
- `links 272MB/s`
- `fix nowiki 258MB/s`  ← bugfix, perf unchanged → latest number

## Comparison context

The benchmark sits next to two baselines (details in [docs/TESTING.md](../../../docs/TESTING.md)): `parse_wiki_text` (a Rust parser, in-process via `benches/compare.rs`) and WikiExtractor (Python, end-to-end via `cargo xtask bench-compare`). Until `extract::strip` exists, the bench only reports `parse_wiki_text`'s number; once it lands, wikrs's own number appears in the same group and *that's* the one this workflow tracks over time.

## Why this is worth the friction

It's tempting to treat tests and benchmarks as a tax you pay at the end. But the entire reason wikrs can exist — the reason the handoff calls this a real opening — is that the incumbents are *silently* wrong and slow. The moment wikrs also regresses silently, it becomes just another half-maintained parser fragment. The loop is cheap insurance on the only two things that make this project worth doing.
