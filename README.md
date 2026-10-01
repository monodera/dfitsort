# dfitsort

Fast FITS header listing and keyword tables for the command line: a successor of
ESO's beloved `dfits` and `fitsort`, written in Rust.

```console
$ dfitsort table -k DPR.CATG,DPR.TYPE,EXPTIME -w DPR.CATG=SCIENCE *.fits
FILE        DPR.CATG  DPR.TYPE  EXPTIME
f001.fits   SCIENCE   OBJECT    300.0
f007.fits   SCIENCE   STD       10.0
```

- Reads only headers and seeks over data, in parallel: a 5-keyword table of
  10,000 files takes about 45 ms, versus 700 ms for `dfits | fitsort` and 8 s for
  dfitspy (on a 16-thread laptop with NVMe).
- Follows FITS Standard 4.0 and the HIERARCH, CONTINUE and tiled-compression
  conventions, while tolerating the non-conforming headers found in real archives.
- Invoked as `dfits` or `fitsort`, it reproduces the original ESO tools byte for byte,
  so existing scripts keep working.

## Install

```sh
cargo install dfitsort
```

or download a binary from the GitHub releases page. Release archives contain
`dfits` and `fitsort` symlinks. With `cargo install`, create them yourself if you
want the legacy commands:

```sh
ln -s "$(which dfitsort)" ~/.local/bin/dfits
ln -s "$(which dfitsort)" ~/.local/bin/fitsort
```

## Usage

```sh
dfitsort dump file.fits                  # primary header, dfits layout
dfitsort dump -x 0 file.fits             # primary and all extensions
dfitsort dump -x SCI,2 file.fits         # EXTNAME = SCI, EXTVER = 2
dfitsort dump - < file.fits.gz           # stdin, gzip is detected

dfitsort table -k OBJECT,EXPTIME *.fits                    # aligned text
dfitsort table -k DPR.CATG -k "HIERARCH TNG DRS BJD" *.fits
dfitsort table -x 1-4 -k EXTNAME,NAXIS1 mef.fits           # one row per HDU: mef.fits[1] ...
dfitsort table -k OBJECT -w 'EXPTIME>=60' -w OBJECT~NGC *.fits
dfitsort table -k OBJECT,MJD-OBS -s MJD-OBS:desc *.fits
dfitsort table -k OBJECT,EXPTIME -f json *.fits            # also tsv, csv
```

| Option | Meaning |
|---|---|
| `-x SEL` | HDUs: omitted = primary, `0` = all, `N`, `N-M`, `EXTNAME`, `EXTNAME,EXTVER` |
| `-k KEYS` | keywords, comma-separated, repeatable |
| `-w COND` | keep rows where `KEY OP VALUE` holds; OP is `= != < <= > >= ~` (substring); numbers compare numerically; repeatable (AND, or OR with `--or`) |
| `-s KEY[:desc]` | sort, repeatable; missing values last |
| `-f FORMAT` | `text` (default), `tsv`, `csv`, `json` |
| `-d` | no header row |
| `--missing STR` | placeholder for missing keywords |
| `--ns NS` | HIERARCH namespace for dot keywords (default `ESO`, env `DFITSORT_NS`) |
| `--compressed` | show tile-compressed HDUs as stored instead of the image header |
| `-j N` | worker threads |

### Keyword names

Matching ignores case and repeated blanks; the `HIERARCH` prefix is optional.
A name with dots and no blanks is tried as, in order:

| You type | Matches |
|---|---|
| `DPR.CATG` | `HIERARCH ESO DPR CATG` (namespace from `--ns`) |
| `ASTRO.METADATA.FIX.DATE` | `HIERARCH ASTRO METADATA FIX DATE` (no namespace) |
| `scaling.fiberPitch` | `HIERARCH scaling.fiberPitch` (a name containing dots) |
| `"ESO DET DIT"` | `HIERARCH ESO DET DIT` |

String values have `''` unescaped, trailing blanks removed and CONTINUE cards
joined. Numbers keep their exact text (no rounding of 19-digit integers). When a
keyword appears twice, the first occurrence wins.

### Compressed files

For tile-compressed images (`.fz`, and the compressed HDUs inside PFS raw
files), `dump` and `table` use the header of the original image, rebuilt from
the `Z*` keywords as astropy and CFITSIO do. `--compressed` shows the stored
binary-table header instead. gzip files (`.fits.gz`) are read transparently.

### Legacy mode

Invoked as `dfits` or `fitsort` (or `dfitsort legacy dfits|fitsort ...`), the
arguments, output, messages and exit status are those of ESO's dfits.c and
fitsort.c, including fitsort's quirks (verbatim quoted values, the last of
duplicate keywords). Internally the files are still read in parallel, data units
are skipped instead of scanned (so bytes inside an image can no longer be
mistaken for an extension header), gzip input works, and the C limits on line
length and number of keywords are gone.

## Development

```sh
cargo test --workspace
uv run --no-project --with astropy --with numpy tests/oracle/check_astropy.py target/debug/dfitsort
scripts/make_golden.sh /path/to/eso/dfits-and-fitsort-sources   # refresh legacy golden files
scripts/bench.sh /some/scratch/dir [/path/to/eso/c/sources]       # benchmarks
```

Test fixtures are regenerated with `tests/fixtures/gen.py` (astropy) and
`tests/fixtures/gen_raw.py` (byte-exact cards); see their headers.

## License

MIT OR Apache-2.0. See NOTICE for the credit to the original ESO tools.
