#!/usr/bin/env bash
# Benchmarks on the spec §2.1 workloads (needs hyperfine). Not run in CI.
# usage: scripts/bench.sh DATA_DIR [ESO_C_TOOLS_DIR]
set -euo pipefail
data=${1:?usage: $0 DATA_DIR [ESO_C_TOOLS_DIR]}
ctools=${2:-}
root=$(cd "$(dirname "$0")/.." && pwd)
[[ -d "$data/small" ]] || uv run --no-project --with astropy --with numpy "$root/scripts/gen_bench_data.py" "$data"
cargo build --release --quiet --manifest-path "$root/Cargo.toml" -p dfitsort
bin=$(mktemp -d)
trap 'rm -rf "$bin"' EXIT
for name in dfitsort dfits fitsort; do ln -s "$root/target/release/dfitsort" "$bin/$name"; done
if [[ -n "$ctools" ]]; then
    cc -O2 -w -o "$bin/c-dfits" "$ctools/dfits.c"
    cc -O2 -w -o "$bin/c-fitsort" "$ctools/fitsort.c"
fi
keys="DPR.CATG DPR.TYPE EXPTIME OBJECT INS.FILT1.NAME"

cd "$data/small"
cmds=(-n "dfitsort table" "$bin/dfitsort table -k ${keys// /,} *.fits"
    -n "dfitsort table -j 1" "$bin/dfitsort table -j 1 -k ${keys// /,} *.fits"
    -n "legacy dfits | fitsort" "$bin/dfits *.fits | $bin/fitsort $keys")
if [[ -n "$ctools" ]]; then cmds+=(-n "C dfits | fitsort" "$bin/c-dfits *.fits | $bin/c-fitsort $keys"); fi
hyperfine -w 2 -r 10 "${cmds[@]}"

cd "$data/mef"
cmds=(-n "dfitsort dump -x 0" "$bin/dfitsort dump -x 0 *.fits"
    -n "dfitsort table -x 3" "$bin/dfitsort table -x 3 -k EXTNAME,DET.CHIP.ID *.fits")
if [[ -n "$ctools" ]]; then cmds+=(-n "C dfits -x 0" "$bin/c-dfits -x 0 *.fits"); fi
hyperfine -w 2 -r 10 "${cmds[@]}"
