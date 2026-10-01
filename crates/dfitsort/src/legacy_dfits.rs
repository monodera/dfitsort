//! `dfits` personality: output byte-compatible with ESO dfits.c (2001).

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use dfitsort_core::card::trim_end;
use dfitsort_core::hdu::HduReader;
use dfitsort_core::{Error, Source};

use crate::paths::os_bytes;
use crate::run::{self, TEXT_CAPACITY};

/// `args` includes argv[0]. Returns the exit status: the number of failed files.
pub fn main(args: &[OsString]) -> i32 {
    let stdout = io::stdout();
    let mut out = io::BufWriter::with_capacity(1 << 16, stdout.lock());
    let mut code = 0;
    let result = dfits(args, &mut out, &mut code).and_then(|()| out.flush());
    run::finish(result, code)
}

fn dfits(args: &[OsString], out: &mut impl Write, code: &mut i32) -> io::Result<()> {
    let pname = args.first().map_or_else(|| b"dfits".to_vec(), |a| os_bytes(a).into_owned());
    if args.len() < 2 {
        *code = 1;
        return usage(out, &pname);
    }
    // As in dfits.c: `-x N` must be the first two arguments, `-` the last one.
    let (xtnum, first) = if args[1] == "-x" { (atoi(args.get(2)), 3) } else { (-1, 1) };
    if args.last().is_some_and(|a| a == "-") {
        let (text, message, rc) = match Source::from_reader(io::stdin()) {
            Ok(source) => {
                let mut text = Vec::with_capacity(TEXT_CAPACITY);
                let (rc, message) = dump_hdus(source, xtnum, &mut text);
                (text, message, rc)
            }
            Err(_) => (Vec::new(), Some("error reading input\n".to_string()), 1),
        };
        out.write_all(&text)?;
        if let Some(m) = message {
            out.flush()?;
            eprint!("{m}");
        }
        *code = rc;
        return Ok(());
    }
    let files: Vec<PathBuf> = args.get(first..).unwrap_or_default().iter().map(PathBuf::from).collect();
    run::ordered(
        &files,
        run::LARGE_RESULTS,
        |path| render(path, xtnum),
        |_, (text, message, rc)| {
            out.write_all(&text)?;
            if let Some(m) = message {
                out.flush()?;
                eprint!("{m}");
            }
            *code += rc;
            Ok(())
        },
    )
}

fn usage(out: &mut impl Write, pname: &[u8]) -> io::Result<()> {
    out.write_all(b"\n\nusage: ")?;
    out.write_all(pname)?;
    out.write_all(b" [-x xtnum] <list of FITS files>\nusage: ")?;
    out.write_all(pname)?;
    out.write_all(
        b" [-x xtnum] -\n\nThe former version expects file names.\nThe latter expects data coming in from stdin.\n\n\
-x xtnum specifies the extension header to print\n-x 0     specifies main header + all extensions\n\n\n",
    )
}

/// C `atoi`: optional blanks and sign, then digits; anything else gives 0.
fn atoi(arg: Option<&OsString>) -> i64 {
    let text = arg.map(|a| a.to_string_lossy().into_owned()).unwrap_or_default();
    let t = text.trim_start();
    let (sign, rest) = match t.strip_prefix('-') {
        Some(r) => (-1, r),
        None => (1, t.strip_prefix('+').unwrap_or(t)),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    sign * digits.parse::<i64>().unwrap_or(0)
}

/// Output, stderr message and error count for one file name.
fn render(path: &Path, xtnum: i64) -> (Vec<u8>, Option<String>, i32) {
    let mut text = Vec::with_capacity(TEXT_CAPACITY);
    let source = match Source::open(path) {
        Ok(s) => s,
        Err(_) => return (text, Some(format!("error: cannot open file [{}]\n", path.display())), 1),
    };
    text.extend_from_slice(b"====> file ");
    text.extend_from_slice(&os_bytes(path.as_os_str()));
    text.extend_from_slice(b" (main) <====\n");
    let (rc, message) = dump_hdus(source, xtnum, &mut text);
    (text, message, rc)
}

/// dfits.c `dump_fits_filter`, but seeking over data units instead of scanning them.
/// Errors that dfits.c reports silently (a failed read) get a message here too.
fn dump_hdus(source: Source, xtnum: i64, text: &mut Vec<u8>) -> (i32, Option<String>) {
    let mut reader = HduReader::new(source);
    let cut = match reader.next_hdu() {
        Ok(Some(main)) => {
            if xtnum < 1 {
                push_cards(text, &main.raw);
            }
            main.is_truncated()
        }
        Ok(None) | Err(Error::TooShort) => return (1, Some("error reading input\n".into())),
        Err(Error::NotFits) => return (1, Some("not a FITS file\n".into())),
        Err(e) => return (1, Some(format!("error: {e}\n"))),
    };
    // A header cut before END has printed its cards; the next call reports the error, as C's exit status does.
    if xtnum < 0 && !cut {
        return (0, None);
    }
    loop {
        match reader.next_hdu() {
            Ok(Some(hdu)) => {
                let n = hdu.index as i64;
                if xtnum == 0 || xtnum == n {
                    text.extend_from_slice(format!("====> xtension {n}\n").as_bytes());
                    push_cards(text, &hdu.raw);
                }
                if n == xtnum {
                    // A cut header has printed its cards; the next call yields the error, as for -x 0.
                    return match hdu.is_truncated().then(|| reader.next_hdu()) {
                        Some(Err(e)) => (1, Some(format!("error: {e}\n"))),
                        _ => (0, None),
                    };
                }
            }
            Ok(None) => return (0, None),
            Err(e) => return (1, Some(format!("error: {e}\n"))),
        }
    }
}

fn push_cards(text: &mut Vec<u8>, raw: &[u8]) {
    for card in raw.chunks_exact(80) {
        text.extend_from_slice(trim_end(card));
        text.push(b'\n');
    }
}
