//! Parallel per-file work with output in input order, and exit-status helpers.

use std::io;

use rayon::prelude::*;

/// Files processed in parallel before their output is written: many when each
/// result is small (table rows), fewer when each holds whole headers (dump).
/// Per-file work is tiny; a small chunk bounds memory and starts output early.
pub const SMALL_RESULTS: usize = 16384;
pub const LARGE_RESULTS: usize = 64;

/// Initial size of a per-file output buffer; most headers fit without regrowing.
pub const TEXT_CAPACITY: usize = 32 * 1024;

pub fn init_threads(jobs: Option<usize>) {
    if let Some(n) = jobs.filter(|&n| n > 0) {
        // Fails only if the pool was already built, which is harmless.
        let _ = rayon::ThreadPoolBuilder::new().num_threads(n).build_global();
    }
}

/// Runs `work` on every item in parallel and hands the results to `sink` in input order.
pub fn ordered<I: Sync, T: Send>(
    items: &[I],
    chunk: usize,
    work: impl Fn(&I) -> T + Sync,
    mut sink: impl FnMut(&I, T) -> io::Result<()>,
) -> io::Result<()> {
    for chunk in items.chunks(chunk) {
        let results: Vec<T> = chunk.par_iter().map(&work).collect();
        for (item, result) in chunk.iter().zip(results) {
            sink(item, result)?;
        }
    }
    Ok(())
}

/// Exit status after writing output: `code` on success, 0 if stdout was closed
/// early (e.g. `| head`), 1 on other write errors.
pub fn finish(result: io::Result<()>, code: i32) -> i32 {
    match result {
        Ok(()) => code,
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => 0,
        Err(e) => {
            eprintln!("dfitsort: write error: {e}");
            1
        }
    }
}
