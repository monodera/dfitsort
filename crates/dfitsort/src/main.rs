mod cli;
mod dump;
mod output;
mod paths;
mod run;
mod table;

use std::ffi::OsString;

fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    std::process::exit(cli::run(args));
}
