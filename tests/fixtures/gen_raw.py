"""Regenerate the hand-written (byte-exact) fixtures. Standard library only.

usage: uv run --no-project tests/fixtures/gen_raw.py
"""

from pathlib import Path

HERE = Path(__file__).resolve().parent
BLOCK = 2880


def card(text: str) -> bytes:
    assert len(text) <= 80, text
    return text.ljust(80).encode("ascii")


def kv(name: str, value, comment: str | None = None) -> str:
    text = f"{name:<8}= {value:>20}"
    return text if comment is None else f"{text} / {comment}"


def header(cards: list[str]) -> bytes:
    raw = b"".join(card(c) for c in cards) + card("END")
    return raw + b" " * (-len(raw) % BLOCK)


def data(nbytes: int, fake_xtension_at: int | None = None) -> bytes:
    buf = bytearray(nbytes)
    if fake_xtension_at is not None:
        buf[fake_xtension_at : fake_xtension_at + 80] = card("XTENSION= 'FAKE    '           / bytes inside a data array")
    return bytes(buf) + b"\0" * (-nbytes % BLOCK)


def image_ext(name: str, bitpix: int, axes: list[int], extra: tuple[str, ...] = ()) -> bytes:
    cards = ["XTENSION= 'IMAGE   '           / Image extension", kv("BITPIX", bitpix), kv("NAXIS", len(axes))]
    cards += [kv(f"NAXIS{i}", n) for i, n in enumerate(axes, 1)]
    cards += [kv("PCOUNT", 0), kv("GCOUNT", 1), f"EXTNAME = '{name:<8}'", *extra]
    nbytes = abs(bitpix) // 8
    for n in axes:
        nbytes *= n
    return header(cards) + data(nbytes)


def simple(bitpix: int, axes: list[int], extra: list[str]) -> list[str]:
    cards = ["SIMPLE  =                    T / conforms to FITS standard", kv("BITPIX", bitpix), kv("NAXIS", len(axes))]
    return cards + [kv(f"NAXIS{i}", n) for i, n in enumerate(axes, 1)] + extra


ENDKEYS_PRIMARY = [
    "SIMPLE  =                    T / conforms to FITS standard",
    kv("BITPIX", 16),
    kv("NAXIS", 2),
    "ENDTIME = '23:59:59'           / END-prefixed keyword before NAXISn",
    "END-OBS = '2026-09-30'         / END-prefixed (Nobeyama style)",
    kv("NAXIS1", 100),
    kv("NAXIS2", 100),
    kv("EXTEND", "T"),
    kv("ENDFRM", 3, "numeric END-prefixed keyword"),
    "HIERARCH ESO DPR CATG = 'SCIENCE ' / standard ESO form",
    "HIERARCH ESO DPR TYPE= 'OBJECT'    / no blank before =",
    "HIERARCH ESO INS FILT1 NAME =   'Ks' / extra blanks after =",
    "HIERARCH ESO  DET DIT = 10.0     / double blank inside the name",
    "HIERARCH ESO det ndit = 6        / lowercase tokens",
    "HIERARCH TNG DRS BJD = 2459000.5 / other namespace",
    "HIERARCH ASTRO METADATA FIX DATE = '2026-01-01' / namespace-less (LSST)",
    "HIERARCH pfs_detectorMap_class = 'DistortedDetectorMap' / mixed case",
    "HIERARCH scaling.fiberPitch = 1.5 / dotted name (PFS)",
    "HIERARCH LONGKEYWORDNAME = 42",
    "HIERARCH ESO OBS NAME = 'END of night'",
    "DATA-TYP= 'OBJECT  '           / Subaru style",
    kv("W_PFDSGN", 6659525521533387424, "19-digit integer"),
    kv("DEXP", "1.5D+03", "D exponent"),
    "CPLX    = (1.0, -2.5)          / complex",
    "UNDEF   =                      / undefined value",
    "QUOTE   = 'O''HARA'",
    "OBJECT  = 'first   '",
    "OBJECT  = 'second  '           / duplicate keyword",
    "LONGSTR = 'This value is continued &'",
    "CONTINUE  'on a second card'",
    "AMPLIT  = 'literal &'          / & not followed by CONTINUE",
    "COMMENT   = this is commentary, not a value",
    "CONTINUE  'orphaned continuation'",
]

ENDKEYS_EXT1 = [
    "XTENSION= 'IMAGE   '           / Image extension",
    kv("BITPIX", 16),
    kv("NAXIS", 2),
    "ENDTIME = '00:00:01'",
    kv("NAXIS1", 50),
    kv("NAXIS2", 40),
    kv("PCOUNT", 0),
    kv("GCOUNT", 1),
    "EXTNAME = 'SCI     '",
]


def main() -> None:
    endkeys = header(ENDKEYS_PRIMARY) + data(100 * 100 * 2)
    endkeys += header(ENDKEYS_EXT1) + data(50 * 40 * 2, fake_xtension_at=1600)
    endkeys += image_ext("LAST", -32, [10])
    (HERE / "endkeys.fits").write_bytes(endkeys)

    groups = header(
        simple(
            -32,
            [0, 2, 3],
            [
                kv("GROUPS", "T"),
                kv("PCOUNT", 4),
                kv("GCOUNT", 5),
                kv("PTYPE1", "'PAR1'"),
                kv("PTYPE2", "'PAR2'"),
                kv("PTYPE3", "'PAR3'"),
                kv("PTYPE4", "'PAR4'"),
            ],
        )
    )
    groups += data(4 * 5 * (4 + 2 * 3)) + image_ext("AFTERGROUPS", 8, [10])
    (HERE / "groups.fits").write_bytes(groups)

    temps = [f"HIERARCH ESO INS TEMP{k} VAL = {k}.5" for k in range(3000)]
    big = header(simple(16, [10, 10], temps)) + data(200) + image_ext("AFTERBIG", 8, [4])
    (HERE / "bigheader.fits").write_bytes(big)

    trailing = header(simple(16, [10, 10], [])) + data(200) + bytes(1000) + b"garbage after the last HDU"
    (HERE / "trailing.fits").write_bytes(trailing)

    cards = simple(8, [], [kv(f"KEY{k}", k) for k in range(40)])
    (HERE / "truncated.fits").write_bytes(b"".join(card(c) for c in cards)[:BLOCK])

    naxis0ext = header(simple(8, [], [kv("EXTEND", "T")]))
    naxis0ext += header(
        [
            "XTENSION= 'BINTABLE'           / NAXIS = 0: no data despite PCOUNT",
            kv("BITPIX", 8),
            kv("NAXIS", 0),
            kv("PCOUNT", 3000),
            kv("GCOUNT", 1),
            kv("TFIELDS", 0),
            "EXTNAME = 'WEIRD   '",
        ]
    )
    naxis0ext += image_ext("AFTER", 16, [10, 10])
    (HERE / "naxis0ext.fits").write_bytes(naxis0ext)

    (HERE / "notfits.txt").write_bytes(b"This is not a FITS file.\n" * 10)
    (HERE / "empty.fits").write_bytes(b"")


if __name__ == "__main__":
    main()
