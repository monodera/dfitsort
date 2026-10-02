mod cli;
mod dump;
mod legacy_dfits;
mod legacy_fitsort;
mod output;
mod pager;
mod paths;
mod run;
mod table;

use std::ffi::OsString;
use std::path::Path;

fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    std::process::exit(dispatch(args));
}

/// Chooses the personality from argv[0], or from `dfitsort legacy TOOL ...`.
fn dispatch(args: Vec<OsString>) -> i32 {
    let personality = args.first().and_then(|a| Path::new(a).file_stem()).and_then(|s| s.to_str()).unwrap_or("");
    match personality {
        "dfits" => legacy_dfits::main(&args),
        "fitsort" => legacy_fitsort::main(&args),
        _ if args.get(1).is_some_and(|a| a == "legacy") => legacy(&args),
        _ => cli::run(args),
    }
}

/// `dfitsort legacy TOOL ARGS...` runs TOOL as if invoked with ARGS (argv[0] = TOOL).
fn legacy(args: &[OsString]) -> i32 {
    match args.get(2).and_then(|t| t.to_str()) {
        Some("dfits") => legacy_dfits::main(&args[2..]),
        Some("fitsort") => legacy_fitsort::main(&args[2..]),
        _ => {
            eprintln!("usage: dfitsort legacy dfits|fitsort [ARGS...]");
            2
        }
    }
}
