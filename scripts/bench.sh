#!/usr/bin/env bash
# Run the wikrs benchmark and print a compact, recordable summary.
#
# Criterion compares against your previous local run (stored under
# target/criterion, which is gitignored) and prints a `change:` line. Record the
# numbers per the wikrs-dev-workflow skill: WORKLOG.md entry + README results table.
#
# Usage:
#   scripts/bench.sh                  # run + compare to your last local run
#   scripts/bench.sh --save NAME      # also save results as baseline NAME
#   scripts/bench.sh --baseline NAME  # compare against a saved baseline NAME
set -eo pipefail
cd "$(dirname "$0")/.."

extra=()
case "${1:-}" in
  "")          ;;
  --save)      extra=(--save-baseline "${2:?--save needs a name}") ;;
  --baseline)  extra=(--baseline "${2:?--baseline needs a name}") ;;
  -h|--help)   sed -n '2,11p' "$0"; exit 0 ;;
  *)           echo "unknown arg: $1 (try --help)" >&2; exit 2 ;;
esac

echo "==> cargo bench --bench compare ${extra[*]}"
if out="$(cargo bench --bench compare -- "${extra[@]}" 2>&1)"; then status=0; else status=$?; fi
printf '%s\n' "$out"

echo
echo "==== summary (record these per wikrs-dev-workflow) ===="
printf '%s\n' "$out" | grep -E '^(wikitext/|[[:space:]]+(time|thrpt|change):)' \
  || echo "(no criterion summary lines found)"
exit "$status"
