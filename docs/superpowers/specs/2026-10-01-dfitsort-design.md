# dfitsort — design spec

Date: 2026-10-01
Status: draft, awaiting review

## 1. Goal

A fast, standards-aware Rust replacement for ESO's `dfits` / `fitsort` (Nicolas
Devillard, 1996–2001) and an alternative to the Python `dfitspy`, for listing FITS
headers and tabulating keyword values across many files.

Success criteria:

- Scripts written for the legacy tools keep working unchanged when the binary is
  invoked as `dfits` / `fitsort` (byte-identical output on standard-conforming files).
- The modern mode follows FITS Standard 4.0 and the registered conventions
  (HIERARCH, CONTINUE, tiled image compression), and reads real Subaru, ESO and
  LSST-stack headers correctly.
- Speed: on a local NVMe, ≥10× faster than `dfits | fitsort` for a 10k-file table,
  and comparable to qfits `dfits` (which seeks over data) for multi-extension dumps.

Non-goals (v1): reading or decompressing pixel data, writing or modifying headers,
FITS validation (that is `fitsverify`'s job), remote/HTTP access.

## 2. Background

### 2.1 Spike results (throwaway prototype, 2026-09-30)

Synthetic ESO-like data on an i7-1260P (16 threads), local NVMe. Warm and cold
cache gave the same times on this machine.

| Workload | C ESO tools | qfits C | dfitspy | CFITSIO (`fitsio` crate, 16 thr) | Rust prototype |
|---|---:|---:|---:|---:|---:|
| 10k small files, 5-keyword table | 706 ms | – | 8.45 s | 113 ms | **50 ms** (16 thr) / 154 ms (1 thr) |
| 100 MEF (1.6 GB), dump all headers | 321 ms | 5.4 ms | – | – | **5.1 ms** |
| 100 MEF, table from ext 3 | 168 ms | – | 782 ms | 8.5 ms | **3.5 ms** |

Findings that shaped this design:

- A hand-written block scanner beats CFITSIO by 2–4× on small files, and CFITSIO
  refuses files whose mandatory keywords are out of order. → No CFITSIO dependency.
- ESO C `dfits` scans data units 80 bytes at a time looking for `XTENSION`. It is
  slow, and bytes inside a data array can be misread as an extension header.
  → Skip data units using their computed size.
- qfits `dfits` detects END by prefix, so it stops at `ENDTIME`. → Match END exactly.

### 2.2 Standards and conventions consulted

- FITS Standard 4.0: https://fits.gsfc.nasa.gov/standard40/fits_standard40aa-le.pdf
  (§3 structure, §4.1–4.2 keyword records and values, §4.2.1.2 CONTINUE,
  §4.4 mandatory/reserved keywords, §6 random groups, §7.3.5 heap, §10.1 tiled
  image compression, App. K INHERIT).
- ESO HIERARCH convention: https://fits.gsfc.nasa.gov/registry/hierarch_keyword.html
- ESO Data Interface Control Document v8 (2025), §4.4.1: hierarchical keywords are
  upper case, tokens separated by a single space; the dot form `INS.FILT1.NAME` is
  the official short form of `HIERARCH ESO INS FILT1 NAME`; ESO does not use CONTINUE.
  https://archive.eso.org/cms/tools-documentation/dicb/ESO-044156_8_Data_Interface_Control_Document.pdf
- Subaru FITS rules: standard 8-character keywords only, instrument prefixes
  (`W_` PFS, `T_` HSC, `S_` Suprime-Cam), hyphens are common (`DATA-TYP`, `EXP-ID`).
  https://subarutelescope.org/DATA/fits/header/regulation.html
- Real-world variations a reader must accept:
  - Namespace-less, mixed-case HIERARCH written by the LSST stack
    (`HIERARCH ASTRO METADATA FIX DATE`, `HIERARCH pfs_detectorMap_class`).
  - Keyword names that contain dots (PFS `scaling.fiberPitch`).
  - Other namespaces (`HIERARCH TNG …`, `HIERARCH CAHA …`).
  - Tile-compressed HDUs inside PFS raw `.fits` files.
- Library behaviour we align with:
  - CFITSIO and astropy both take a HIERARCH name as column 10 up to the first `=`,
    trimmed.
  - Both match keywords case-insensitively and do not collapse internal blanks.
  - Both return the first occurrence of a duplicated keyword.
- Prior art:
  - WCSTools `gethead`: conditions, missing-value placeholder.
  - astropy `fitsheader -f`: table formats.
  - dfitspy: `--grep`.
  - There are unrelated GitHub projects named `dfits-rs` / `fitsort-rs`, which is
    why this tool is named `dfitsort`.

## 3. Decisions

| Topic | Decision |
|---|---|
| Compatibility | One binary. Modern subcommands, plus exact legacy behaviour when invoked as `dfits` / `fitsort` |
| v1 extras | Logical header of `.fz` images, filter conditions, output formats (text/tsv/csv/json) |
| Deferred | Wildcard keywords, INHERIT, COMMENT/HISTORY values, tiled *table* compression, Python bindings |
| Distribution | CLI first (`cargo install`, prebuilt binaries); library crate kept separate so PyO3 bindings can be added later |
| Name | `dfitsort` (crate and binary) |
| FITS I/O | Own header reader; no CFITSIO |
| License | MIT OR Apache-2.0. NOTICE credits ESO dfits/fitsort (BSD-3), whose behaviour is reimplemented. No dfitspy (GPLv3) code is used |

## 4. Architecture

Cargo workspace:

```
dfitsort/
├── Cargo.toml            # workspace
├── crates/
│   ├── dfitsort-core/    # library: no CLI, no output formatting
│   └── dfitsort/         # binary: CLI, parallelism, output, legacy mode
├── tests/fixtures/       # small FITS files + generator scripts
├── scripts/              # golden-output and benchmark scripts
└── docs/
```

### 4.1 `dfitsort-core` modules

| Module | Responsibility |
|---|---|
| `source` | Opens a path or stdin. Detects gzip (magic `1f 8b`) and wraps it in `flate2` (multi-member). Provides `read_exact_block` and `skip(n)`: a seek for plain files, read-and-discard for streams |
| `hdu` | `HduReader`: iterates HDUs. Returns the raw header bytes (cards up to and including END) and the HDU's physical index. Computes the padded data size and skips it lazily |
| `card` | Classifies one 80-byte record. Splits name / value / comment. Recognises HIERARCH |
| `value` | Parses value text into `Value::{Str, Logical, Int, Real, Complex, Undefined}` while keeping the original text. Joins CONTINUE |
| `header` | `Header`: ordered list of parsed cards plus a name index (normalised name → first position). Built from raw bytes |
| `compressed` | Builds the logical image header for `ZIMAGE=T` binary tables |
| `query` | Normalises user keyword specs into an ordered list of candidate names. Resolves them against a `Header` |
| `filter` | Parses `KEY OP VALUE` conditions and evaluates them on a `Header` |
| `legacy` | Byte-exact ports of the dfits.c framing and the fitsort.c text parser and value extractor |

The core API is synchronous and works per file. The CLI decides how to
parallelise. Raw header bytes are kept so `dump` can print cards verbatim.

### 4.2 `dfitsort` binary

- `clap` (derive) for the modern CLI. Hand-written argument parsing for the legacy
  personalities, to mimic the C programs exactly.
- `rayon` processes files in parallel. Output order always follows the input order:
  files are processed in chunks (e.g. 1024), collected per chunk, then written
  sequentially.
- Uses a buffered stdout. A broken pipe (e.g. `| head`) exits quietly with status 0.
- `-j N` sets the thread count (default: all cores; `-j 1` is sequential).

Dependencies: `rayon`, `flate2`, `clap`, `serde_json`, and `thiserror` in core.
Dev-dependencies: whatever the tests need.

## 5. Modern CLI

```
dfitsort dump  [-x SEL] [--compressed] [-j N] <FILES>... | -
dfitsort table -k KEYS [-k KEYS]... [-x SEL] [-w COND]... [--or]
               [-f text|tsv|csv|json] [-d] [-s KEY[:desc]]...
               [--ns NS] [--missing STR] [--compressed] [-j N] <FILES>...
dfitsort legacy dfits   <legacy dfits args>
dfitsort legacy fitsort <legacy fitsort args>
```

### 5.1 HDU selection `-x SEL`

| SEL | Meaning |
|---|---|
| omitted | primary HDU only |
| `0` | primary and all extensions (legacy meaning) |
| `N` | extension N (1-based; primary is 0 in physical numbering) |
| `N-M` | extensions N through M |
| `NAME` or `NAME,VER` | the HDU(s) whose EXTNAME (and EXTVER, default 1) match, case-insensitive |

SEL is parsed as follows: all digits → number; `digits-digits` → range;
anything else → EXTNAME, with an optional `,VER`. A range or name that matches
nothing selects no rows and is not an error, the same as legacy `-x N` beyond the
last extension.

HDU numbers are physical: an fpack file's empty primary is 0 and the compressed
image is 1, the same as astropy.

### 5.2 `dump`

Prints each selected HDU's cards verbatim, each right-stripped, one per line. It
uses the legacy framing lines so existing consumers (including legacy `fitsort`)
keep working:

```
====> file <path> (main) <====
<cards of primary>
====> xtension <N>
<cards of extension N>
```

For a `ZIMAGE=T` HDU the logical image header is printed (§7) unless `--compressed`
is given. `-` reads one FITS stream from stdin.

### 5.3 `table`

- `-k` takes comma-separated keyword specs and may be repeated. Each spec becomes
  a column, headed with the spec exactly as the user typed it.
- One row per selected HDU. The first column is the file path as given, with
  `[N]` appended for extension HDUs (e.g. `img.fits[3]`).
- `-d` omits the header row.
- `--missing STR` prints STR for absent keywords. The default is empty in
  text/tsv/csv and `null` in JSON.
- `-s KEY[:desc]` sorts rows (repeatable, stable, in priority order).
  - It compares numerically when both values parse as numbers, otherwise as strings.
  - Missing values sort last.
  - KEY may be a keyword that is not displayed.
- `--ns NS` sets the HIERARCH namespace used by dot specs (default `ESO`; `--ns ''`
  means none). The environment variable `DFITSORT_NS` provides the same default.

### 5.4 Output formats `-f`

| Format | Shape |
|---|---|
| `text` (default) | Columns padded with spaces, two-space gutter, no tabs |
| `tsv` | Tab-separated, no padding, no quoting. Tabs and newlines in values become a space |
| `csv` | RFC 4180 quoting |
| `json` | Array of `{"file": str, "hdu": int, "extname": str\|null, "values": {spec: typed\|null}}` |

JSON typing:

- Logical → `true`/`false`.
- Int and Real → a JSON number made from the original text, normalised to valid
  JSON syntax: strip a leading `+`, strip leading zeros, `D` exponent → `E`,
  `.5` → `0.5`, `5.` → `5.0`. Precision is never lost: a 19-digit integer stays
  19 digits. Text that still is not a valid JSON number after normalisation falls
  back to a string.
- Complex → `[re, im]`.
- Undefined or missing → `null`.
- Strings: if the bytes are not valid UTF-8, decode them as Latin-1.

In text, tsv and csv, value bytes are written as they are.

### 5.5 Filter conditions `-w COND`

- **Syntax.** `KEY OP VALUE`, where OP is the first of `<=`, `>=`, `!=`, `=`, `<`,
  `>`, `~` found in the string. KEY uses the same spec syntax as `-k`.
- **Numeric comparison.** Used for `= != < <= > >=` when both sides parse as
  numbers (`D` exponent accepted; integers compared exactly).
- **String comparison.** Used otherwise, case-sensitive, on the normalised string
  value: `''` unescaped, trailing blanks removed.
- **Substring.** `~` tests for a case-sensitive substring.
- **Missing keywords.** A condition on a missing keyword is false, including `!=`.
- **Combining.** Several `-w` are ANDed; `--or` ORs them.
- **Scope.** Conditions are evaluated per HDU row, so with `-x 0` each HDU is
  filtered on its own.

### 5.6 Exit status (modern mode)

| Status | Meaning |
|---|---|
| 0 | Every file processed |
| 1 | One or more files failed (each reported on stderr; the rest still processed) |
| 2 | Usage error |

## 6. Header parsing rules (modern mode)

### 6.1 HDU scanning

- **Header blocks.** A header is a sequence of 2880-byte blocks. The first HDU must
  start with `SIMPLE  =`, otherwise the file is reported as "not a FITS file".
- **END.** A card is END when bytes 1–8 are exactly `END` followed by five spaces.
  The rest of the card is not checked (the standard requires blanks there; we are
  lenient). Prefix matching is never used: `ENDTIME`, `END-OBS` and `ENDFRM` exist
  in real data.
- **Data size.** Computed by Standard Eq. 2 as
  `|BITPIX|/8 × GCOUNT × (PCOUNT + ΠNAXISn)`, with PCOUNT defaulting to 0 and
  GCOUNT to 1, then padded to a multiple of 2880.
  - Random groups (`NAXIS1 = 0` and `GROUPS = T`) use Eq. 4, which leaves NAXIS1
    out of the product.
  - All arithmetic is in `u64` with overflow checks. Negative or overflowing sizes
    are a per-file error.
- **Mandatory keywords** are found wherever they appear in the header; their order
  is not enforced.
- **Truncation.** EOF inside a header, or a missing END, is a per-file error. EOF
  inside the data area is not an error unless a later HDU was requested.
- **gzip input.** Skipping a data unit means decompressing it. This is inherent to
  the format and should be documented.

### 6.2 Card classification

| Condition (in order) | Kind | Name | Value text |
|---|---|---|---|
| bytes 1–9 equal `HIERARCH ` (any case) and the card contains `=` | HIERARCH keyword | bytes 10 up to the first `=`: trimmed, internal blank runs collapsed to one, upper-cased | after the first `=`, free format |
| bytes 9–10 are `= ` | standard keyword | bytes 1–8 right-trimmed, upper-cased | bytes 11–80 |
| byte 9 is `=` and byte 10 is not a blank (non-conforming) | standard keyword (lenient; legacy fitsort accepts it too) | bytes 1–8 right-trimmed | bytes 10–80 |
| `CONTINUE` with blanks in bytes 9–10 | continuation | – | bytes 11–80 |
| anything else (COMMENT, HISTORY, blank name, …) | commentary | – | – |

The original bytes of each card are kept for display. Name normalisation is used
only for matching.

### 6.3 Values

- **Value field.** The value is the text after the value indicator up to an
  unquoted `/`. An empty or blank value field is `Undefined`.
- **Strings.**
  - Text between the opening quote and the matching closing quote, where `''`
    inside the string is an escaped quote and is unescaped.
  - Leading blanks are kept and trailing blanks are removed, as Standard §4.2.1.1
    requires. `''` is the empty string.
- **CONTINUE (Standard §4.2.1.2).**
  - When a string value ends in `&` and the next card is a conforming CONTINUE,
    remove the `&` and append the CONTINUE card's string. Repeat while the
    condition holds.
  - Otherwise `&` is literal.
  - A CONTINUE card that doesn't follow a continued string ("orphaned") counts as
    commentary.
  - This works for HIERARCH keywords too.
- **Other types.**
  - Logical: `T` / `F`.
  - Integer: optional sign and digits.
  - Real: digits with `.` and/or an `E`/`D` exponent.
  - Complex: `(re, im)`.
  - Anything else is kept as `Str` of the raw token. This is lenient: it does not
    raise an error.
- **Original text.** Every value keeps its original text. Numbers are converted
  only for comparison, sorting and JSON output.
- **Duplicate keywords.** The first occurrence wins, as in CFITSIO and astropy.
  The Standard says the value is indeterminate.

### 6.4 Keyword specs (user side)

A spec is normalised by:

1. Upper-casing it.
2. Removing a leading `HIERARCH ` and collapsing blank runs.
3. Choosing candidates:
   - If the spec has no blank and contains `.`, the candidates are tried in order:
     1. `NS A B C` (when NS is non-empty)
     2. `A B C`
     3. `A.B.C` taken literally
   - Otherwise the spec itself is the only candidate.

Resolution against a header picks the first candidate that is present, separately
for each HDU. Examples:

| Spec | Matches |
|---|---|
| `DPR.CATG` | `HIERARCH ESO DPR CATG` |
| `ASTRO.METADATA.FIX.DATE` | `HIERARCH ASTRO METADATA FIX DATE` (via candidate 2) |
| `scaling.fiberPitch` | `HIERARCH scaling.fiberPitch` (via candidate 3) |
| `"ESO DET DIT"`, `"HIERARCH ESO DET DIT"`, `"hierarch eso det dit"` | the same card |
| `DATA-TYP` | `DATA-TYP` |

Collapsing internal blanks (`HIERARCH ESO  DET DIT` matches `DET.DIT`) is more
lenient than CFITSIO or astropy. It is harmless, because two names that differ
only in blank runs cannot coexist meaningfully.

## 7. Tiled image compression (logical header)

This applies to an HDU with `XTENSION = 'BINTABLE'` and `ZIMAGE = T`, following
Standard §10.1 and astropy's `_bintable_header_to_image_header`.

- **Mandatory keywords.**
  - `XTENSION` becomes `'IMAGE   '`, or `SIMPLE = T` when `ZSIMPLE` is present.
  - `BITPIX ← ZBITPIX`, `NAXIS ← ZNAXIS`, `NAXISn ← ZNAXISn`.
  - `PCOUNT ← ZPCOUNT` (default 0) and `GCOUNT ← ZGCOUNT` (default 1); both are
    omitted when the result is a primary header.
- **Restored names.** `ZEXTEND → EXTEND`, `ZBLOCKED → BLOCKED`,
  `ZHECKSUM → CHECKSUM`, `ZDATASUM → DATASUM`.
- **Removed.**
  - Table structure keywords: `TFIELDS`, `TTYPEn`, `TFORMn`, `TUNITn`, `TNULLn`,
    `TSCALn`, `TZEROn`, `TDISPn`, `TDIMn`, `THEAP`.
  - The original `CHECKSUM` / `DATASUM`.
  - Compression keywords: `ZIMAGE`, `ZCMPTYPE`, `ZTILEn`, `ZQUANTIZ`, `ZDITHER0`,
    `ZNAMEn`, `ZVALn`, `ZSCALE`, `ZZERO`, `ZMASKCMP`, `ZBLANK`.
  - `EXTNAME = 'COMPRESSED_IMAGE'`.
- **Kept.** All other cards, verbatim and in their original order.
- The rebuilt cards are synthesised in fixed format.
- The physical data size used for skipping still comes from the real BINTABLE
  keywords.

`--compressed` turns this off and shows the BINTABLE header as stored. Legacy mode
never applies it.

## 8. Legacy mode

Selected when the basename of argv[0] (without `.exe`) is `dfits` or `fitsort`, or
through `dfitsort legacy dfits|fitsort …`.

### 8.1 `dfits`

- **Arguments**, exactly as ESO dfits.c:
  - `dfits [-x xtnum] files…` or `dfits [-x xtnum] -`.
  - `-x` must come first, and `-` must be the last argument.
  - With no arguments, it prints the usage text and exits with status 1.
- **Output** is byte-identical to ESO dfits.c:
  - `====> file NAME (main) <====`, then the right-stripped cards.
  - `====> xtension N` before each extension.
  - Error messages on stderr.
  - The exit status is the number of failed files.
- **Same output, improved internals:**
  - It seeks over data units instead of scanning them, so bytes in data are never
    misread as an `XTENSION` header.
  - It reads gzip files, including from stdin.
  - It handles files larger than 2 GB.
  - It processes files in parallel.
- No `.fz` reconstruction.

### 8.2 `fitsort`

Byte-identical to fitsort.c, including its quirks:

- **Arguments.** `fitsort [-d] KEY…`, with keys upper-cased. `A.B.C` maps only to
  `HIERARCH ESO A B C`. Full names given with blanks are matched literally.
- **Records.** A line starting with `====>` starts a record whose name is the
  third whitespace token: the file name, or the extension number for
  `====> xtension N`. A line starting with `SIMPLE  =` starts an unnamed record.
- **Keyword name.** The text before the first `=`, right-trimmed, compared
  exactly.
- **Value.** Quoted values are taken verbatim between the first and last quote,
  with trailing blanks kept and `''` left as is. Unquoted values are the first
  token. No CONTINUE joining. The last occurrence wins.
- **Output.**
  - Each cell is printed as `%-Ws\t` with W the column width, header row
    included; `-d` drops the header row.
  - Missing values print as a padded single space.
  - With no records, it prints `*** error: no input data corresponding to dfits output`
    on stdout and exits with status 255 (C's `return -1`).
- **Removed limits.** The C limits (128-character lines, 512 keywords) do not
  apply.

### 8.3 Installation

Release archives contain `dfitsort` plus `dfits` and `fitsort` symlinks. With
`cargo install`, the README documents the commands
`ln -s "$(which dfitsort)" ~/.local/bin/dfits` and
`ln -s "$(which dfitsort)" ~/.local/bin/fitsort`.
`dfitsort legacy …` covers systems without symlinks.

## 9. Errors and robustness

- **Per-file errors** are reported on stderr and processing continues:
  - cannot open
  - not FITS
  - truncated header
  - missing END
  - bad size
  - gzip error
- **Message format:** `dfitsort: <path>: <message>` in modern mode. Legacy mode uses
  the C messages.
- **No panics on input.** The parser must not panic on any byte sequence. A fuzz
  target (`cargo fuzz`) for `HduReader` and `Header` checks this.
- **Lenient by default.** Out-of-order mandatory keywords, non-ASCII bytes,
  non-blank bytes after END, lowercase names and missing blanks around `=` are all
  accepted silently.

## 10. Performance design

- **Reading headers:**
  - The first read takes 32 KiB in one call, which covers most single-HDU headers.
  - Further header blocks are read on demand into a per-thread buffer, which is
    reused.
  - Data units are skipped by seeking. There is no mmap: it costs more than `read`
    on small files.
- **Table mode** stores only the requested values per row, never whole headers.
  - text format holds all rows, because column widths must be known before
    printing.
  - tsv, csv and json stream chunk by chunk.
- **dump mode** writes each chunk of buffers as soon as the chunk is complete.
- **Benchmarks** (`scripts/bench.sh`, hyperfine) cover the spike workloads plus
  `.fz` and `.gz` sets. Regression target: within 20% of the spike numbers in §2.1
  on the same machine. Benchmarks are not run in CI.

## 11. Testing

- **Unit tests** in `dfitsort-core`:
  - card classification
  - value parsing, including CONTINUE and quote escapes
  - data-size formula: images, binary table with heap, random groups, overflow
  - spec normalisation and candidate resolution
  - the condition parser and evaluator
  - the `.fz` keyword mapping
- **Fixtures** (`tests/fixtures/`, under 2 MB in total):
  - Generated by `tests/fixtures/gen.py`, run with `uv run --with astropy --with numpy`.
  - A raw-card writer covers the cases astropy normalises away: ENDTIME before
    NAXISn, HIERARCH spacing and case variants, a fake `XTENSION` inside data, and
    namespace-less or dotted HIERARCH.
  - `.fz` fixtures come from astropy `CompImageHDU` with RICE and GZIP.
  - `.gz` copies of some fixtures.
  - The generated files are committed.
- **Golden tests (legacy).**
  - Expected outputs of the original C tools are committed under `tests/golden/`.
  - `scripts/make_golden.sh` regenerates them by compiling ESO dfits.c/fitsort.c
    from a user-supplied path.
  - Tests compare byte-for-byte.
- **Oracle test (modern).** `tests/oracle/check_astropy.py` compares
  `dfitsort table -f json` and `dfitsort dump` (logical `.fz` header) against
  astropy for every fixture. It runs in CI via uv.
- **CI** (GitHub Actions, Linux and macOS): `cargo fmt --check`,
  `cargo clippy -D warnings`, `cargo test`, the oracle test, and a short fuzz smoke
  run.

## 12. Distribution

- `cargo install dfitsort`. MSRV 1.85 (edition 2024).
- GitHub Releases with prebuilt binaries for x86_64/aarch64 Linux (musl, static)
  and x86_64/aarch64 macOS. Archives include the `dfits` / `fitsort` symlinks.
- Windows compiles on a best-effort basis but is not a tested target in v1.

## 13. Future work (not in v1)

- PyO3 bindings (`dfitsort-py`, wheels via maturin) as a faster dfitspy.
- `--inherit`: fall back to the primary header for INHERIT=T extensions. It will
  be opt-in; note that ESO's default differs from the FITS convention.
- Wildcard specs (`NAXIS*`, `DET.*`).
- Extracting COMMENT/HISTORY values.
- Tiled table compression.
- `@filelist`.
- A `keys` subcommand that lists the keywords in a file.
