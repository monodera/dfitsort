//! Modern command line (clap).

use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::dump;

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
}

#[derive(Args)]
pub struct DumpArgs {
    /// HDUs: 0 = all, N, N-M, EXTNAME or EXTNAME,EXTVER [default: primary]
    #[arg(short = 'x', value_name = "SEL")]
    pub select: Option<String>,
    /// Show tile-compressed HDUs as stored instead of the image header
    #[arg(long)]
    pub compressed: bool,
    /// Number of worker threads [default: all cores]
    #[arg(short = 'j', long = "jobs", value_name = "N")]
    pub jobs: Option<usize>,
    /// FITS files; `-` reads one FITS stream from stdin
    #[arg(required = true, value_name = "FILES")]
    pub files: Vec<PathBuf>,
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
    }
}
