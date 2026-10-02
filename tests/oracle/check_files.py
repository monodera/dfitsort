"""Run check_astropy.py's per-HDU checks on arbitrary files (e.g. real archive data).

Checks, for every HDU astropy can read, the same things as check_astropy.py does on the fixtures.

usage: uv run --no-project --with astropy --with numpy tests/oracle/check_files.py PATH/TO/dfitsort FILE...
"""

import sys
from pathlib import Path

from astropy.io import fits

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_astropy as oracle


def main() -> int:
    binary, *files = sys.argv[1:]
    problems: list[str] = []
    checked = 0
    for name in files:
        path = Path(name)
        with fits.open(path) as hdul:
            for index, hdu in enumerate(hdul):
                problems += oracle.check_values(binary, path, index, hdu.header, oracle.stored_header(path, hdu))
                if isinstance(hdu, fits.CompImageHDU):
                    problems += oracle.check_logical_header(binary, path, index, hdu.header)
                checked += 1
    for p in problems:
        print(p)
    print(f"{len(files)} files, {checked} HDUs checked, {len(problems)} problems")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
