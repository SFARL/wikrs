# WikiExtractor (comparison baseline)

[WikiExtractor](https://github.com/attardi/wikiextractor) is the de-facto Python
tool wikrs is benchmarked against. It lives here as a local, **gitignored** venv
so the comparison harness (`cargo xtask bench-compare`) can invoke it.

## Setup

```bash
./tools/wikiextractor/setup.sh
```

Requires [`uv`](https://docs.astral.sh/uv/). The script pins **Python 3.10**.

## Why Python 3.10 (not newer)

WikiExtractor 3.0.6 compiles a regex with an inline `(?i)` flag in the middle of
the pattern. Python **3.11+ rejects that as a hard error**
(`re.error: global flags not at the start of the expression`); 3.10 only warns.
`uv` fetches a managed CPython 3.10, so no system Python change is needed.

## Use

```bash
# once the wikrs CLI exists (Stage 1, Task 7):
cargo xtask bench-compare path/to/slice.xml.bz2
# defaults to tools/wikiextractor/.venv/bin/python
```
