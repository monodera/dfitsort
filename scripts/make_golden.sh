#!/usr/bin/env bash
# Regenerates tests/golden from the original ESO C tools.
# usage: scripts/make_golden.sh DIR   (DIR contains dfits.c and fitsort.c)
set -euo pipefail
src=${1:?usage: $0 DIR_WITH_dfits.c_AND_fitsort.c}
root=$(cd "$(dirname "$0")/.." && pwd)
bin=$(mktemp -d)
trap 'rm -rf "$bin"' EXIT
cc -O2 -w -o "$bin/dfits" "$src/dfits.c"
cc -O2 -w -o "$bin/fitsort" "$src/fitsort.c"
rm -f "$root"/tests/golden/*.stdout "$root"/tests/golden/*.rc
"$root/scripts/run_golden_cases.sh" "$bin" "$root/tests/golden"
