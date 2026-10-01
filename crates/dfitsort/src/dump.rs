//! `dfitsort dump`: header cards of the selected HDUs, in dfits layout.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use dfitsort_core::card::trim_end;
use dfitsort_core::select::{FileHdus, HduSelector, read_selected};

use crate::cli::DumpArgs;
use crate::paths::{is_stdin, open, os_bytes};
use crate::run::{self, TEXT_CAPACITY};

pub fn run(args: DumpArgs) -> i32 {
    let selector = match parse_selector(args.select.as_deref()) {
        Ok(s) => s,
        Err(code) => return code,
    };
    run::init_threads(args.jobs);
    let logical = !args.compressed;
    let mut failed = false;
    let stdout = io::stdout();
    let mut out = io::BufWriter::with_capacity(1 << 16, stdout.lock());
    let result = run::ordered(
        &args.files,
        run::LARGE_RESULTS,
        |path: &PathBuf| render(path, &selector, logical),
        |path, (text, error)| {
            out.write_all(&text)?;
            if let Some(msg) = error {
                failed = true;
                eprintln!("dfitsort: {}: {msg}", path.display());
            }
            Ok(())
        },
    )
    .and_then(|()| out.flush());
    run::finish(result, i32::from(failed))
}

/// Parses `-x`; on error prints a message and returns exit status 2.
pub fn parse_selector(select: Option<&str>) -> Result<HduSelector, i32> {
    match select {
        None => Ok(HduSelector::Primary),
        Some(s) => HduSelector::parse(s).map_err(|e| {
            eprintln!("dfitsort: -x: {e}");
            2
        }),
    }
}

fn render(path: &Path, selector: &HduSelector, logical: bool) -> (Vec<u8>, Option<String>) {
    let mut text = Vec::with_capacity(TEXT_CAPACITY);
    let source = match open(path) {
        Ok(s) => s,
        Err(e) => return (text, Some(e.to_string())),
    };
    let FileHdus { hdus, error } = read_selected(source, selector, logical);
    if !is_stdin(path) {
        text.extend_from_slice(b"====> file ");
        text.extend_from_slice(&os_bytes(path.as_os_str()));
        text.extend_from_slice(b" (main) <====\n");
    }
    for hdu in &hdus {
        if hdu.index > 0 {
            text.extend_from_slice(format!("====> xtension {}\n", hdu.index).as_bytes());
        }
        for card in hdu.header.cards() {
            text.extend_from_slice(trim_end(card));
            text.push(b'\n');
        }
    }
    (text, error.map(|e| e.to_string()))
}
