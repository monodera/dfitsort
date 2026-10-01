"""Compare dfitsort with astropy on every fixture.

Checks, for every HDU astropy can read:
  * `dfitsort table -f json` returns the same value as astropy for every keyword;
  * for tile-compressed images, `dfitsort dump` lists the same keywords as astropy's
    image header.

usage: uv run --no-project --with astropy --with numpy tests/oracle/check_astropy.py PATH/TO/dfitsort
"""

import json
import math
import subprocess
import sys
import warnings
from pathlib import Path

from astropy.io import fits
from astropy.io.fits.card import Undefined
from astropy.io.fits.verify import VerifyWarning

warnings.simplefilter("ignore", VerifyWarning)
FIXTURES = Path(__file__).resolve().parents[1] / "fixtures"
UNREADABLE = {"notfits.txt", "empty.fits", "truncated.fits"}
COMMENTARY = {"", "COMMENT", "HISTORY", "CONTINUE"}


def same(expected, actual) -> bool:
    if expected is None or isinstance(expected, Undefined):
        return actual is None
    if isinstance(expected, bool):
        return actual is expected
    if isinstance(expected, complex):
        return isinstance(actual, list) and len(actual) == 2 and same(expected.real, actual[0]) and same(expected.imag, actual[1])
    if isinstance(expected, (int, float)):
        if isinstance(expected, float) and not math.isfinite(expected):
            return actual is None  # JSON has no infinities: dfitsort emits null
        if isinstance(actual, bool) or not isinstance(actual, (int, float)):
            return False
        if isinstance(expected, int) and isinstance(actual, int):
            return expected == actual
        return math.isclose(expected, actual, rel_tol=1e-12)
    return expected == actual


def run(binary: str, *args: str) -> str:
    return subprocess.run([binary, *args], capture_output=True, text=True, check=True).stdout


def selector(index: int) -> str:
    return "0-0" if index == 0 else str(index)


def stored_header(path: Path, hdu) -> bytes:
    info = hdu.fileinfo()
    with open(path, "rb") as f:
        f.seek(info["hdrLoc"])
        return f.read(info["datLoc"] - info["hdrLoc"])


def check_values(binary: str, path: Path, index: int, header: fits.Header, stored: bytes) -> list[str]:
    # astropy adds EXTEND = T to primary headers of multi-HDU files; compare only stored cards.
    synthetic = {"EXTEND"} if b"EXTEND  =" not in stored else set()
    keys = list(dict.fromkeys(k for k in header.keys() if k not in COMMENTARY | synthetic))
    if not keys:
        return []
    rows = json.loads(run(binary, "table", "-f", "json", "-x", selector(index), "-k", ",".join(keys), str(path)))
    if len(rows) != 1:
        return [f"{path.name}[{index}]: expected one row, got {len(rows)}"]
    values = rows[0]["values"]
    return [
        f"{path.name}[{index}] {key}: astropy {header[key]!r}, dfitsort {values[key]!r}"
        for key in keys
        if not same(header[key], values[key])
    ]


def check_logical_header(binary: str, path: Path, index: int, header: fits.Header) -> list[str]:
    dumped = run(binary, "dump", "-x", str(index), str(path)).splitlines()
    names = {line[:8].rstrip() for line in dumped if not line.startswith("====>")} - COMMENTARY - {"END"}
    expected = set(header.keys()) - COMMENTARY
    if names != expected:
        return [f"{path.name}[{index}] logical header: only dfitsort {sorted(names - expected)}, only astropy {sorted(expected - names)}"]
    return []


def main() -> int:
    binary = sys.argv[1]
    problems: list[str] = []
    checked = 0
    for path in sorted(FIXTURES.iterdir()):
        if path.suffix == ".py" or path.name in UNREADABLE:
            continue
        with fits.open(path) as hdul:
            for index, hdu in enumerate(hdul):
                problems += check_values(binary, path, index, hdu.header, stored_header(path, hdu))
                if isinstance(hdu, fits.CompImageHDU):
                    problems += check_logical_header(binary, path, index, hdu.header)
                checked += 1
    for p in problems:
        print(p)
    print(f"{checked} HDUs checked, {len(problems)} problems")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
