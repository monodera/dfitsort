# dfitsort

A small command-line tool, written in Rust, for listing FITS headers and putting
keywords into tables. It is a personal hobby project: I wanted a faster
`dfits | fitsort` for my own data, and for now I am its main user. If you find it
useful too, you are very welcome to use it.

The idea and much of the behaviour come from `dfits` and `fitsort` by Nicolas
Devillard at ESO (see [Acknowledgements](#acknowledgements)).

```console
$ dfitsort table -k DPR.CATG,DPR.TYPE,EXPTIME -w DPR.CATG=SCIENCE *.fits
FILE       DPR.CATG  DPR.TYPE  EXPTIME
f001.fits  SCIENCE   OBJECT    300.0
f007.fits  SCIENCE   STD       10.0
```

Columns are padded to the widest cell plus a two-space gutter.

- It reads only the headers and skips over the data, several files at a time. In
  my tests a 5-keyword table of 10,000 small files took about 45 ms, against
  700 ms for `dfits | fitsort` (synthetic files on a 16-thread laptop with NVMe).
- It tries to follow FITS Standard 4.0 and the HIERARCH, CONTINUE and
  tiled-compression conventions, while accepting the non-conforming headers found
  in real archives.
- Invoked as `dfits` or `fitsort`, it aims to print exactly what the original ESO
  tools print on standard-conforming files, so existing scripts keep working (known
  differences are listed under "Legacy mode").
- I have checked it against astropy and the original C tools on the real data I had
  at hand (Subaru Suprime-Cam and PFS raw frames, ESO MUSE products, HSC pipeline
  coadds: 639 files, 2,368 HDUs) and found no differences. Data from other
  instruments may well contain cases I have not seen.

## Status and support

This is a personal project that I maintain in my spare time, mainly for my own use.
Bug reports and suggestions are welcome as GitHub issues, and small fixes as pull
requests, but I may be slow to reply and cannot promise fixes or new features.

## Install

```sh
cargo install --git https://github.com/monodera/dfitsort dfitsort
```

There is no crates.io package or prebuilt binary yet; I plan to publish it on
crates.io later. To get the legacy commands,
create the symlinks yourself:

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
dfitsort dump -p -x 0 *.fits             # page through $PAGER (default less)

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
| `-s KEY[:desc]` | sort, repeatable; ascending puts numbers before text (`:desc` reverses this); missing values always last |
| `-f FORMAT` | `text` (default), `tsv`, `csv`, `json` |
| `-d` | no header row |
| `--missing STR` | placeholder for missing keywords |
| `--ns NS` | HIERARCH namespace for dot keywords (default `ESO`, env `DFITSORT_NS`) |
| `--compressed` | show tile-compressed HDUs as stored instead of the image header |
| `-j N` | worker threads (0 = all cores, the default) |
| `-p`, `--pager` | show the output in `$PAGER` (default `less`, run with `LESS=FRX` unless `LESS` is set) when stdout is a terminal; per-file errors are printed after the pager exits |

### Keyword names

Matching ignores case and repeated blanks; the `HIERARCH` prefix is optional.
A name with dots and no blanks is tried as, in order:

| You type | Matches |
|---|---|
| `DPR.CATG` | `HIERARCH ESO DPR CATG` (namespace from `--ns`) |
| `ASTRO.METADATA.FIX.DATE` | `HIERARCH ASTRO METADATA FIX DATE` (no namespace) |
| `scaling.fiberPitch` | `HIERARCH scaling.fiberPitch` (a name containing dots) |
| `"ESO DET DIT"` | `HIERARCH ESO DET DIT` |

A dot name that starts with `HIERARCH.` spells out the whole keyword, so
`HIERARCH.ESO.PRO.CATG` matches `HIERARCH ESO PRO CATG` and `--ns` is not added.

String values have `''` unescaped, trailing blanks removed and CONTINUE cards
joined. Numbers keep their exact text (no rounding of 19-digit integers). When a
keyword appears twice, the first occurrence wins.

### Compressed files

For tile-compressed images (`.fz`, and the compressed HDUs inside PFS raw
files), `dump` and `table` use the header of the original image, rebuilt from
the `Z*` keywords as astropy and CFITSIO do. `--compressed` shows the stored
binary-table header instead. gzip files (`.fits.gz`) are read transparently; since
data cannot be skipped in a compressed stream, reading them means decompressing the
data units too, which is slower than seeking in plain files.

### Unusable data sizes

A header whose data size cannot be computed (for example a non-numeric `NAXIS`) is
still shown, and then reported as an error, which ends the dump of that file. The
exit status can therefore depend on how far reading goes: `-x 1` stops at HDU 1 and
never looks at what follows, while `-x NAME` has to read on until it has seen every HDU.

### Legacy mode

Invoked as `dfits` or `fitsort` (or `dfitsort legacy dfits|fitsort ...`), the
arguments, output, messages and exit status are those of ESO's dfits.c and
fitsort.c, including fitsort's quirks (verbatim quoted values, the last of
duplicate keywords). Internally the files are still read in parallel, data units
are skipped instead of scanned (so bytes inside an image can no longer be
mistaken for an extension header), gzip input works, and the C limits on line
length and number of keywords are gone.

Byte identity holds for standard-conforming files. Known differences:

- Errors that the C tools handle silently get a message on stderr.
- A header cut before `END`: the cards read so far are printed, a message goes to stderr and the exit status is 1 (C prints the same cards; it exits 1 for a cut primary but 0 for a cut extension).
- Bytes after the last HDU are ignored (C `dfits -x 0` on such a file exits 1, dfitsort 0).
- An unusable data size stops `-x 0` with an error, for any HDU (C scans on).
- `-p` as the first argument (`dfits -p ...`, `fitsort -p [-d] ...`) pages the output as `--pager` does; C dfits takes it as a file name and C fitsort as the keyword `-P`. Anywhere else, `-p` keeps its C meaning.

### Exit status

| Mode | Status |
|---|---|
| modern | 0 = all files processed, 1 = some file failed (the others are still processed), 2 = usage error; a closed stdout (e.g. `\| head`) exits 0 |
| legacy `dfits` | number of failed files |
| legacy `fitsort` | 255 when there is no record |

## Development

```sh
cargo test --workspace
uv run --no-project --with astropy --with numpy tests/oracle/check_astropy.py target/debug/dfitsort
scripts/make_golden.sh /path/to/eso/dfits-and-fitsort-sources   # refresh legacy golden files
scripts/bench.sh /some/scratch/dir [/path/to/eso/c/sources]       # benchmarks

# the same checks on your own files
uv run --no-project --with astropy --with numpy tests/oracle/check_files.py target/debug/dfitsort data/*.fits
scripts/compare_legacy.sh target/debug/dfitsort /path/to/eso/c/sources data/*.fits -- OBJECT EXPTIME
```

Test fixtures are regenerated with `tests/fixtures/gen.py` (astropy) and
`tests/fixtures/gen_raw.py` (byte-exact cards); see their headers.

## Acknowledgements

dfitsort exists because of `dfits` and `fitsort`, written by Nicolas Devillard at
the European Southern Observatory between 1996 and 2001 as part of
[eclipse](https://github.com/ndevilla/eclipse) (ESO C Library for an Image
Processing Environment, later transformed by ESO into the Common Pipeline Library).
For a quarter of a century, `dfits *.fits | fitsort ...` has been how many
astronomers look at their data. dfitsort's legacy mode reimplements their
behaviour, and their output is the reference for its golden tests. Thank you,
Nicolas.

The C sources used as that reference are the copies preserved by Grant Tremblay in
[eso_fits_tools](https://github.com/granttremblay/eso_fits_tools), after the tools
disappeared from ESO's website. They are distributed under ESO's BSD-style
license, reproduced in NOTICE.

Other software and documents that shaped dfitsort:

| Project | How it was used |
|---|---|
| [dfitspy](https://github.com/Romain-Thomas-Shef/dfitspy) by Romain Thomas ([JOSS paper](https://doi.org/10.21105/joss.01249)) | Python take on dfits/fitsort; its `--grep` informed `-w`; benchmark baseline. No code used (GPLv3). |
| [astropy](https://www.astropy.org/) | Reference for header parsing in the oracle tests and for generating fixtures; [`fitsheader -f`](https://docs.astropy.org/en/stable/io/fits/usage/scripts.html) inspired the table formats. |
| [CFITSIO](https://heasarc.gsfc.nasa.gov/fitsio/) | Behaviour aligned with for HIERARCH names, duplicate keywords and rebuilding tile-compressed image headers; benchmarked during design. |
| qfits `dfits` (in eclipse) | Benchmark baseline; showed that seeking over data units is the fast path. |
| [WCSTools](http://tdc-www.harvard.edu/wcstools/) `gethead` | Inspired the conditions and the missing-value placeholder. |
| [FITS Standard 4.0](https://fits.gsfc.nasa.gov/fits_standard.html), the [HIERARCH convention](https://fits.gsfc.nasa.gov/registry/hierarch_keyword.html), the [ESO Data Interface Control Document](https://archive.eso.org/cms/tools-documentation/dicb/ESO-044156_8_Data_Interface_Control_Document.pdf) and the [Subaru FITS rules](https://subarutelescope.org/DATA/fits/header/regulation.html) | Rules for structure, keyword names and values. |

Related Rust projects: [dfits-rs](https://github.com/TrystanScottLambert/dfits-rs)
and [fitsort-rs](https://github.com/TrystanScottLambert/fitsort-rs) by Trystan
Scott Lambert are independent rewrites of the two tools. dfitsort shares no code
with them, and is named differently so the projects are not confused.

dfitsort was developed with [Claude Code](https://claude.com/claude-code),
Anthropic's agentic coding tool: the design, implementation, tests and reviews
were carried out by Claude Code under my direction.

## License

MIT OR Apache-2.0. See NOTICE for the credit to the original ESO tools.
