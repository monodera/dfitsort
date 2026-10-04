//! `fitsort` personality: byte-compatible with ESO fitsort.c (2001).

use std::ffi::OsString;
use std::io::{self, Write};

use dfitsort_core::legacy::{fitsort_key, fitsort_read, fitsort_write};

use crate::pager;
use crate::paths::os_bytes;

/// `args` includes argv[0]; `-p` (not in fitsort.c) first, after `-d` or last pages the output,
/// and `-h` or `--help` (not in fitsort.c either) as the only argument prints the usage.
/// Returns 0, or 255 when the input held no dfits output.
pub fn main(args: &[OsString]) -> i32 {
    if matches!(args, [_, flag] if flag == "-h" || flag == "--help") {
        let text = b"Input data is received from stdin\n\n\
-d omits the header line\n-p pages the output through $PAGER (first, after -d, or last)\n\n";
        return i32::from(usage(&mut io::stdout().lock(), &args[0], text).is_err());
    }
    let (page, args) = pager::legacy_p(args, |a| usize::from(a.get(1).is_some_and(|x| x == "-d")));
    let (mut out, pager) = pager::start(page);
    let mut code = 0;
    let result = fitsort(&args, &mut out, &mut code).and_then(|()| out.flush());
    pager.finish(out, result, code)
}

/// fitsort.c's usage line, then `text`.
fn usage(out: &mut impl Write, pname: &OsString, text: &[u8]) -> io::Result<()> {
    out.write_all(b"\n\nuse : ")?;
    out.write_all(&os_bytes(pname))?;
    out.write_all(b" [-d] KEY1 KEY2 ... KEYn\n")?;
    out.write_all(text)
}

fn fitsort(args: &[OsString], out: &mut impl Write, code: &mut i32) -> io::Result<()> {
    if args.len() < 2 {
        let pname = args.first().map_or_else(|| OsString::from("fitsort"), Clone::clone);
        return usage(
            out,
            &pname,
            b"Input data is received from stdin\nSee man page for more details and examples\n\n",
        );
    }
    let mut keys = &args[1..];
    let print_header = keys[0] != "-d";
    if !print_header {
        keys = &keys[1..];
    }
    let labels: Vec<Vec<u8>> = keys.iter().map(|k| os_bytes(k).to_ascii_uppercase()).collect();
    let specs: Vec<Vec<u8>> = keys.iter().map(|k| fitsort_key(&os_bytes(k))).collect();
    let table = fitsort_read(io::stdin().lock(), &specs)?;
    *code = fitsort_write(out, &labels, &table, print_header)?;
    Ok(())
}
