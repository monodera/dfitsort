"""Generate the benchmark data sets of spec §2.1 (about 2 GB).

usage: uv run --no-project --with astropy --with numpy scripts/gen_bench_data.py DATA_DIR
  DATA_DIR/small: 10,000 single-HDU files with ESO-like headers (~290 cards)
  DATA_DIR/mef:   100 files with 4 image extensions of 4 MB each
"""

import random
import sys
import warnings
from multiprocessing import Pool
from pathlib import Path

import numpy as np
from astropy.io import fits
from astropy.io.fits.verify import VerifyWarning

warnings.simplefilter("ignore", VerifyWarning)
ROOT = Path(sys.argv[1])
CATG = ["SCIENCE", "CALIB", "ACQUISITION", "TECHNICAL"]
TYPE = ["OBJECT", "FLAT", "BIAS", "DARK", "WAVE", "STD"]
FILT = ["J", "H", "Ks", "NB2.17", "Br_gamma"]


def eso_header(i: int, rng: random.Random) -> fits.Header:
    h = fits.Header()
    h["ORIGIN"] = ("ESO-PARANAL", "European Southern Observatory")
    h["TELESCOP"] = ("ESO-VLT-U1", "ESO <TEL>")
    h["OBJECT"] = (f"NGC {rng.randint(1, 7840)}", "Original target.")
    h["EXPTIME"] = (rng.choice([1.0, 10.0, 60.0, 300.0]), "Integration time")
    h["MJD-OBS"] = (60000 + rng.random() * 1000, "Obs start")
    h["HIERARCH ESO DPR CATG"] = (rng.choice(CATG), "Observation category")
    h["HIERARCH ESO DPR TYPE"] = (rng.choice(TYPE), "Observation type")
    h["HIERARCH ESO DET DIT"] = (rng.choice([1.0, 2.5, 10.0]), "Integration Time")
    h["HIERARCH ESO INS FILT1 NAME"] = (rng.choice(FILT), "Filter name.")
    h["HIERARCH ESO OBS NAME"] = (f"OB_{i:06d}", "OB name")
    for k in range(270):
        h[f"HIERARCH ESO INS TEMP{k} VAL"] = (round(rng.uniform(-200, 30), 3), "temperature")
    return h


def make_small(i: int) -> None:
    hdu = fits.PrimaryHDU(np.zeros((64, 64), np.int16), eso_header(i, random.Random(i)))
    hdu.writeto(ROOT / "small" / f"small_{i:05d}.fits", overwrite=True)


def make_mef(i: int) -> None:
    hdus = [fits.PrimaryHDU(header=eso_header(i, random.Random(100000 + i)))]
    for e in range(4):
        ext = fits.ImageHDU(np.zeros((2048, 1024), np.int16), name=f"CHIP{e + 1}")
        ext.header["HIERARCH ESO DET CHIP ID"] = f"CCD-{e + 1}"
        hdus.append(ext)
    fits.HDUList(hdus).writeto(ROOT / "mef" / f"mef_{i:04d}.fits", overwrite=True)


if __name__ == "__main__":
    (ROOT / "small").mkdir(parents=True, exist_ok=True)
    (ROOT / "mef").mkdir(parents=True, exist_ok=True)
    with Pool() as pool:
        pool.map(make_small, range(10000), chunksize=200)
        pool.map(make_mef, range(100), chunksize=5)
