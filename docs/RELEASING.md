# Releasing wikrs

The mechanical checklist for cutting a release. Derived from the 0.1.0/0.2.0
releases (see WORKLOG entries of 2026-07-02); every step below was hit in
practice, including the gotchas.

## Versioning policy (pre-1.0)

- New user-facing capability (a new `--format`, a new module) → **minor** bump
  (`0.2.0` → `0.3.0`).
- Fixes/docs/tests only → **patch** bump.
- Breaking a documented CLI flag or public API → minor bump **plus** a
  CHANGELOG "Changed/Removed" entry. (Cargo treats `0.x.y` → `0.x.(y+1)` as
  compatible, so anything breaking must not be a patch.)

## Pre-flight (all of these, in order)

1. **Clean tree on `main`, synced:** `git status` clean, `git pull --ff-only`.
2. **The CI trio, locally:** `cargo fmt --all -- --check` &&
   `cargo clippy --all-targets --all-features` (0 warnings) &&
   `cargo test --all-features` (all green — includes the coverage ratchet and,
   when `tests/fixtures/parserTests.txt` is fetched, the markdown round-trip
   harness; fetch it via `cargo xtask fetch-parser-tests` so the release runs
   the full 1071-case conformance check, not the soft-skip).
3. **Benchmark:** `scripts/bench.sh` — no unexplained regression vs the last
   recorded numbers (README scoreboard). Machine-wide noise shows up in the
   `parse_wiki_text` reference line too; judge by the wikrs/reference *ratio*,
   and re-run on an idle machine before concluding anything.
4. **Fuzz the surfaces touched since the last release**, ≥15 minutes each,
   zero findings: `cargo +nightly fuzz run <target> -- -max_total_time=900`
   (targets: `strip`, `parse`, `markdown_roundtrip`).
5. **Full-dump smoke of new user-facing output:**
   `./target/release/wikrs --input target/realdump/simplewiki-articles.xml --format <new> | head`
   plus a wall-clock number for the WORKLOG.

## Cutting the release (X.Y.Z as the example)

6. **Bump the version — two files:** `version = "X.Y.Z"` in `Cargo.toml`
   **and** the path-dep pin in `xtask/Cargo.toml`
   (`wikrs = { version = "X.Y.Z", path = ".." }` — forgetting it fails the
   dry-run; bit us in 0.2.0). Then `cargo build -q` to refresh `Cargo.lock`.
7. **CHANGELOG:** retitle `## [Unreleased]` → `## [X.Y.Z] — YYYY-MM-DD`.
8. **Commit** (subject carries the current benchmark number, ≤20 chars):
   `vX.Y.Z <n>MB/s`, body = one-line release summary. Include `Cargo.toml`,
   `Cargo.lock`, `xtask/Cargo.toml`, `CHANGELOG.md`.
9. **Validate the package:** `cargo publish --dry-run` — must be clean
   (requires the committed tree; check the file count/size line for surprises;
   `Cargo.toml` `exclude` keeps `.github/.claude/.cargo/tools/scripts/WORKLOG`
   out).
10. **Push and wait for CI:** `git push origin main`, then
    `gh run list --limit 1` until `completed success`. **Do not publish on a
    red or running CI.**
11. **Tag:** `git tag vX.Y.Z && git push origin vX.Y.Z`.
12. **Publish:** `cargo publish` (crates.io token must be configured — the
    account owner runs this). Verify:
    `curl -s -A "wikrs-release (contact email)" https://crates.io/api/v1/crates/wikrs`
    → `max_version == X.Y.Z`.
13. **GitHub Release:**
    `gh release create vX.Y.Z --title "wikrs X.Y.Z — <headline>" --notes "<CHANGELOG section + validation numbers>"`.
14. **Record it:** WORKLOG release entry (what shipped, the mechanical steps'
    outcomes, any surprises) → commit `log vX.Y.Z <n>MB/s` → push.

## If something goes wrong

- **Published a bad version:** `cargo yank --version X.Y.Z` (yank hides it
  from new resolutions; crates.io versions can never be deleted or reused —
  fix forward with X.Y.(Z+1)).
- **Tag pushed but publish failed:** fix, re-run `cargo publish`; the tag can
  stay (it points at the intended commit). Never force-move a pushed tag.
- **Dry-run warns `X.Y.Z already exists`:** you forgot the version bump —
  see step 6 (this was the 0.1.1 post-release confusion: the warning on an
  *unchanged* tree is expected and harmless; on a tree with new content it
  means "bump first").
