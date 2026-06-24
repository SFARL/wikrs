#!/usr/bin/env bash
# Set up WikiExtractor (the Python comparison baseline) in a local, gitignored venv.
#
# Pinned to Python 3.10: WikiExtractor 3.0.6 uses an inline (?i) regex flag that
# Python 3.11+ rejects as a hard error. Requires `uv` (https://docs.astral.sh/uv/).
set -euo pipefail
cd "$(dirname "$0")"

uv venv --python 3.10 .venv
uv pip install --python .venv/bin/python wikiextractor
.venv/bin/python -m wikiextractor.WikiExtractor --help >/dev/null

echo "OK: WikiExtractor ready at tools/wikiextractor/.venv (gitignored)."
