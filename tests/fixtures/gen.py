"""Regenerate the astropy-written fixtures.

usage: uv run --no-project --with astropy --with numpy tests/fixtures/gen.py
"""

import gzip
import shutil
import warnings
from pathlib import Path

import numpy as np
from astropy.io import fits
from astropy.io.fits.verify import VerifyWarning

warnings.simplefilter("ignore", VerifyWarning)
HERE = Path(__file__).resolve().parent


def eso_header(i: int, with_filter: bool = True) -> fits.Header:
    h = fits.Header()
    h["ORIGIN"] = ("ESO-PARANAL", "European Southern Observatory")
    h["TELESCOP"] = ("ESO-VLT-U1", "ESO <TEL>")
    h["OBJECT"] = (f"NGC {253 + i}", "Original target.")
    h["EXPTIME"] = (10.0 * i, "Integration time")
    h["MJD-OBS"] = (60000.5 + i, "Obs start")
    h["HIERARCH ESO DPR CATG"] = (["SCIENCE", "CALIB", "SCIENCE"][i % 3], "Observation category")
    h["HIERARCH ESO DPR TYPE"] = (["OBJECT", "FLAT", "STD"][i % 3], "Observation type")
    h["HIERARCH ESO DET DIT"] = (2.5 * i, "Integration Time")
    h["HIERARCH ESO DET NDIT"] = (i + 1, "# of Sub-Integrations")
    if with_filter:
        h["HIERARCH ESO INS FILT1 NAME"] = (["Ks", "H", "J"][i % 3], "Filter name.")
    h["HIERARCH ESO OBS PROG ID"] = ("0118.A-0001(A)", "ESO program identification")
    return h


def main() -> None:
    for i in (1, 2, 3):
        hdu = fits.PrimaryHDU(np.zeros((8, 8), np.int16), eso_header(i, with_filter=i != 2))
        hdu.writeto(HERE / f"eso{i}.fits", overwrite=True)

    hdus = [fits.PrimaryHDU(header=eso_header(1))]
    for n in (1, 2, 3):
        ext = fits.ImageHDU(np.zeros((8, 16), np.int16), name=f"CHIP{n}")
        ext.header["EXTVER"] = n
        ext.header["HIERARCH ESO DET CHIP ID"] = f"CCD-{n}"
        hdus.append(ext)
    fits.HDUList(hdus).writeto(HERE / "mef.fits", overwrite=True)

    rng = np.random.default_rng(7)
    vla = np.array([np.arange(rng.integers(10, 300), dtype=np.int32) for _ in range(50)], dtype=object)
    table = fits.BinTableHDU.from_columns(
        [fits.Column(name="ID", format="J", array=np.arange(50)), fits.Column(name="VLA", format="PJ()", array=vla)],
        name="VLATAB",
    )
    after = fits.ImageHDU(np.ones((10, 10), np.float32), name="AFTERHEAP")
    after.header["MARKER"] = "found-after-heap"
    fits.HDUList([fits.PrimaryHDU(), table, after]).writeto(HERE / "heap.fits", overwrite=True)

    h = eso_header(2)
    h["LONGSTR"] = "x" * 150 + " end"
    h["QUOTED"] = ("it's / not a comment", "real comment")
    h["HIERARCH ESO OBS TARG NAME"] = "O'Brien / field"
    fits.PrimaryHDU(np.zeros((4, 4), np.uint8), h).writeto(HERE / "strings.fits", overwrite=True)
    with open(HERE / "strings.fits", "rb") as src, open(HERE / "strings.fits.gz", "wb") as raw:
        with gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as dst:
            shutil.copyfileobj(src, dst)

    sci = fits.CompImageHDU(np.arange(64 * 64, dtype=np.int16).reshape(64, 64), name="SCI", compression_type="RICE_1")
    sci.header["OBJECT"] = "NGC 253"
    sci.header["EXPTIME"] = 30.0
    # GZIP_2 tiles embed a gzip timestamp: the data bytes of this file (not its headers)
    # change from one run to the next.
    err = fits.CompImageHDU(np.arange(32 * 32, dtype=np.int32).reshape(32, 32), name="ERR", compression_type="GZIP_2")
    fits.HDUList([fits.PrimaryHDU(), sci, err]).writeto(HERE / "compressed.fits.fz", overwrite=True)


if __name__ == "__main__":
    main()
