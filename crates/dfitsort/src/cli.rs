//! Modern command line (clap).

use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{dump, table};

#[derive(Parser)]
#[command(
    name = "dfitsort",
    version,
    about = "Fast FITS header listing and keyword tables (successor of ESO dfits/fitsort)",
    after_help = "Invoked as `dfits` or `fitsort` (for example through a symlink), or as\n`dfitsort legacy dfits|fitsort ARGS...`, it behaves exactly like the original ESO tools."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print header cards of the selected HDUs (like dfits)
    Dump(DumpArgs),
    /// Tabulate keyword values, one row per selected HDU (like dfits | fitsort)
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
    /// FITS files; `-` reads one FITS stream from stdin
    #[arg(required = true, value_name = "FILES")]
    pub files: Vec<PathBuf>,
}

#[derive(Args)]
pub struct TableArgs {
    /// Keywords, comma-separated and repeatable: EXPTIME, DPR.CATG, "HIERARCH TNG DRS BJD", ...
    #[arg(short = 'k', long = "keys", required = true, value_delimiter = ',', value_name = "KEYS")]
    pub keys: Vec<String>,
    /// HDUs: 0 = all, N, N-M, EXTNAME or EXTNAME,EXTVER [default: primary]
    #[arg(short = 'x', value_name = "SEL")]
    pub select: Option<String>,
    /// Keep rows where KEY OP VALUE holds (OP: = != < <= > >= ~); repeatable, ANDed
    #[arg(short = 'w', long = "where", value_name = "COND")]
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
    #[arg(short = 's', long = "sort", value_name = "KEY[:desc]")]
    pub sort: Vec<String>,
    /// HIERARCH namespace used for dot keywords ('' for none)
    #[arg(long, env = "DFITSORT_NS", default_value = "ESO")]
    pub ns: String,
    /// Text printed for missing keywords (text, tsv, csv)
    #[arg(long, value_name = "STR", default_value = "")]
    pub missing: String,
    /// Use tile-compressed HDUs as stored instead of the image header
    #[arg(long)]
    pub compressed: bool,
    /// Number of worker threads (0 = all cores) [default: all cores]
    #[arg(short = 'j', long = "jobs", value_name = "N")]
    pub jobs: Option<usize>,
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
