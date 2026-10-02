#!/usr/bin/env bash
# Compares dfitsort's legacy mode with the original ESO C tools on arbitrary files:
# stdout byte for byte and the exit status, for dfits (primary, -x 1, -x 2, -x 0) and,
# when keys are given, for `dfits FILE... | fitsort KEY...`.
# usage: scripts/compare_legacy.sh DFITSORT DIR FILE... [-- FITSORT_KEY...]
#        (DIR contains dfits.c and fitsort.c)
set -uo pipefail
dfitsort=${1:?usage: $0 DFITSORT DIR_WITH_dfits.c_AND_fitsort.c FILE... [-- KEY...]}
src=${2:?usage: $0 DFITSORT DIR_WITH_dfits.c_AND_fitsort.c FILE... [-- KEY...]}
shift 2
files=()
while [[ $# -gt 0 && $1 != -- ]]; do files+=("$1"); shift; done
[[ $# -gt 0 ]] && shift
keys=("$@")
[[ ${#files[@]} -gt 0 ]] || { echo "$0: no FILE given" >&2; exit 2; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cc -O2 -w -o "$work/dfits" "$src/dfits.c" || exit 2
cc -O2 -w -o "$work/fitsort" "$src/fitsort.c" || exit 2

now() { date +%s.%N; }
diffs=0
report() { # label, C rc, dfitsort rc, C seconds, dfitsort seconds
    if cmp -s "$work/c.out" "$work/r.out" && [[ $2 == "$3" ]]; then
        printf 'SAME  %-22s rc=%s  %9d bytes  C %.3fs  dfitsort %.3fs\n' "$1" "$2" "$(wc -c < "$work/c.out")" "$4" "$5"
    else
        diffs=$((diffs + 1))
        printf 'DIFF  %-22s rc C=%s dfitsort=%s\n' "$1" "$2" "$3"
        diff "$work/c.out" "$work/r.out" | head -20
        echo "C stderr:"; head -5 "$work/c.err"
        echo "dfitsort stderr:"; head -5 "$work/r.err"
    fi
}

for n in "" 1 2 0; do
    x=()
    [[ -n $n ]] && x=(-x "$n")
    t0=$(now); "$work/dfits" "${x[@]}" "${files[@]}" > "$work/c.out" 2> "$work/c.err"; crc=$?; t1=$(now)
    "$dfitsort" legacy dfits "${x[@]}" "${files[@]}" > "$work/r.out" 2> "$work/r.err"; rrc=$?; t2=$(now)
    report "dfits${n:+ -x $n}" "$crc" "$rrc" "$(echo "$t1 - $t0" | awk '{print $1 - $3}')" "$(echo "$t2 - $t1" | awk '{print $1 - $3}')"
done

if [[ ${#keys[@]} -gt 0 ]]; then
    t0=$(now); "$work/dfits" "${files[@]}" 2> /dev/null | "$work/fitsort" "${keys[@]}" > "$work/c.out" 2> "$work/c.err"; crc=$?; t1=$(now)
    "$work/dfits" "${files[@]}" 2> /dev/null | "$dfitsort" legacy fitsort "${keys[@]}" > "$work/r.out" 2> "$work/r.err"; rrc=$?; t2=$(now)
    report "fitsort" "$crc" "$rrc" "$(echo "$t1 - $t0" | awk '{print $1 - $3}')" "$(echo "$t2 - $t1" | awk '{print $1 - $3}')"
fi

echo "${#files[@]} files, $diffs differences"
[[ $diffs -eq 0 ]]
