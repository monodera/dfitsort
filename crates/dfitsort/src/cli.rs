//! Modern command line (clap).

use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{dump, table};

/// The exit status section of `--help`, as a literal for `concat!`.
macro_rules! exit_status {
    () => {
        "Exit status:
  0  all files processed
  1  some file failed (the others are still processed)
  2  usage error
  A closed stdout (e.g. `| head`) exits 0."
    };
}

const LEGACY: &str = "Invoked as `dfits` or `fitsort` (for example through a symlink), or as
`dfitsort legacy dfits|fitsort ARGS...`, it behaves exactly like the original ESO tools.";

#[derive(Parser)]
#[command(
    name = "dfitsort",
    version,
    about = "Fast FITS header listing and keyword tables (after ESO dfits/fitsort)",
    after_help = LEGACY,
    after_long_help = concat!(
        "Legacy mode:
  Invoked as `dfits` or `fitsort` (for example through a symlink), or as
  `dfitsort legacy dfits|fitsort ARGS...`, it behaves exactly like the original
  ESO tools: same arguments, output, messages and exit status (dfits: the number
  of failed files; fitsort: 255 when there is no record). Two additions: `-p` as
  the first argument, right after `-x N` (dfits) or `-d` (fitsort), or as the last
  argument pages the output as --pager does, and `-h` or `--help` as the only
  argument prints the usage.

",
        exit_status!(),
        "

Run `dfitsort dump --help` or `dfitsort table --help` for options and examples."
    )
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print header cards of the selected HDUs (like dfits)
    #[command(after_long_help = concat!(
        "Examples:
  dfitsort dump file.fits               primary header, dfits layout
  dfitsort dump -x 0 file.fits          primary and all extensions
  dfitsort dump -x SCI,2 file.fits      EXTNAME = SCI, EXTVER = 2
  dfitsort dump - < file.fits.gz        stdin; gzip is detected
  dfitsort dump -p -x 0 *.fits          page through $PAGER

",
        exit_status!()
    ))]
    Dump(DumpArgs),
    /// Tabulate keyword values, one row per selected HDU (like dfits | fitsort)
    #[command(after_long_help = concat!(
        "Keyword names:
  Matching ignores case and repeated blanks; the HIERARCH prefix is optional, so
  \"ESO DET DIT\" matches HIERARCH ESO DET DIT. A name with dots and no blanks is
  tried as, in order:
    DPR.CATG  ->  HIERARCH ESO DPR CATG (namespace from --ns), HIERARCH DPR CATG,
                  then DPR.CATG itself (a name containing dots)
  A name starting with HIERARCH. spells out the whole keyword: --ns is not added.
  When a keyword appears twice in a header, the first occurrence wins.

Examples:
  dfitsort table -k OBJECT,EXPTIME *.fits                    aligned text
  dfitsort table -k DPR.CATG -k \"HIERARCH TNG DRS BJD\" *.fits
  dfitsort table -x 1-4 -k EXTNAME,NAXIS1 mef.fits           one row per HDU
  dfitsort table -k OBJECT -w 'EXPTIME>=60' -w OBJECT~NGC *.fits
  dfitsort table -k OBJECT,MJD-OBS -s MJD-OBS:desc *.fits
  dfitsort table -k OBJECT,EXPTIME -f json *.fits            also tsv, csv

",
        exit_status!()
    ))]
    Table(TableArgs),
}

#[derive(Args)]
pub struct DumpArgs {
    /// HDUs: 0 = all, N, N-M, EXTNAME or EXTNAME,EXTVER [default: primary]
    #[arg(short = 'x', value_name = "SEL")]
    pub select: Option<String>,
    /// Show tile-compressed HDUs as stored instead of the image header
    #[arg(long)]
    pub compressed: bool,
    /// Number of worker threads (0 = all cores) [default: all cores]
    #[arg(short = 'j', long = "jobs", value_name = "N")]
    pub jobs: Option<usize>,
    /// Show the output in $PAGER (default less) when stdout is a terminal
    ///
    /// Unix only. less runs with LESS=FRX unless LESS is set, so it quits at once
    /// when the output fits on one screen. Per-file errors are printed after the
    /// pager exits.
    #[arg(short = 'p', long = "pager", verbatim_doc_comment)]
    pub pager: bool,
    /// FITS files; `-` reads one FITS stream from stdin
    #[arg(required = true, value_name = "FILES")]
    pub files: Vec<PathBuf>,
}

#[derive(Args)]
pub struct TableArgs {
    /// Keywords, comma-separated and repeatable: EXPTIME, DPR.CATG, "HIERARCH TNG DRS BJD", ...
    ///
    /// See "Keyword names" below for how names are matched.
    #[arg(
        short = 'k',
        long = "keys",
        verbatim_doc_comment,
        required = true,
        value_delimiter = ',',
        value_name = "KEYS"
    )]
    pub keys: Vec<String>,
    /// HDUs: 0 = all, N, N-M, EXTNAME or EXTNAME,EXTVER [default: primary]
    #[arg(short = 'x', value_name = "SEL")]
    pub select: Option<String>,
    /// Keep rows where KEY OP VALUE holds (OP: = != < <= > >= ~); repeatable, ANDed
    ///
    /// ~ means VALUE is a substring (case-sensitive). When both sides are numbers
    /// they compare numerically, otherwise as text. A missing or undefined keyword
    /// fails every condition, != included. With --or, any condition may hold.
    /// Quote the condition for the shell, e.g. -w 'EXPTIME>=60'.
    #[arg(short = 'w', long = "where", verbatim_doc_comment, value_name = "COND")]
    pub conditions: Vec<String>,
    /// Combine -w conditions with OR instead of AND
    #[arg(long)]
    pub or: bool,
    /// Output format
    #[arg(short = 'f', long = "format", value_enum, default_value_t = Format::Text)]
    pub format: Format,
    /// Do not print the header row
    #[arg(short = 'd', long = "no-header")]
    pub no_header: bool,
    /// Sort rows by KEY, append :desc for descending; repeatable
    ///
    /// Later keys break ties; rows still tied keep the input order. Ascending puts
    /// numbers (compared numerically) before text; :desc reverses this. Rows missing
    /// the keyword always come last.
    #[arg(short = 's', long = "sort", verbatim_doc_comment, value_name = "KEY[:desc]")]
    pub sort: Vec<String>,
    /// HIERARCH namespace used for dot keywords ('' for none)
    #[arg(long, env = "DFITSORT_NS", default_value = "ESO")]
    pub ns: String,
    /// Text printed for missing keywords (text, tsv, csv)
    ///
    /// json output uses null instead.
    #[arg(long, verbatim_doc_comment, value_name = "STR", default_value = "")]
    pub missing: String,
    /// Use tile-compressed HDUs as stored instead of the image header
    #[arg(long)]
    pub compressed: bool,
    /// Number of worker threads (0 = all cores) [default: all cores]
    #[arg(short = 'j', long = "jobs", value_name = "N")]
    pub jobs: Option<usize>,
    /// Show the output in $PAGER (default less) when stdout is a terminal
    ///
    /// Unix only. less runs with LESS=FRX unless LESS is set, so it quits at once
    /// when the output fits on one screen. Per-file errors are printed after the
    /// pager exits.
    #[arg(short = 'p', long = "pager", verbatim_doc_comment)]
    pub pager: bool,
    /// FITS files; `-` reads one FITS stream from stdin
    #[arg(required = true, value_name = "FILES")]
    pub files: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Text,
    Tsv,
    Csv,
    Json,
}

/// Runs the modern CLI; returns the exit status.
pub fn run(args: Vec<OsString>) -> i32 {
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            let code = if e.use_stderr() { 2 } else { 0 };
            let _ = e.print();
            return code;
        }
    };
    match cli.command {
        Command::Dump(args) => dump::run(args),
        Command::Table(args) => table::run(args),
    }
}
