#!/usr/bin/env bash
# Runs every case in tests/golden/cases.tsv with `dfits` and `fitsort` taken from
# BIN_DIR, writing NAME.stdout and NAME.rc into OUT_DIR.
# usage: scripts/run_golden_cases.sh BIN_DIR OUT_DIR
set -uo pipefail
bindir=$(cd "${1:?usage: $0 BIN_DIR OUT_DIR}" && pwd)
mkdir -p "${2:?usage: $0 BIN_DIR OUT_DIR}"
out=$(cd "$2" && pwd)
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root/tests/fixtures"
while IFS=$'\t' read -r name cmd; do
    [[ -z "$name" || "$name" == \#* ]] && continue
    PATH="$bindir:$PATH" bash -c "$cmd" >"$out/$name.stdout" 2>/dev/null
    echo $? >"$out/$name.rc"
done <"$root/tests/golden/cases.tsv"
