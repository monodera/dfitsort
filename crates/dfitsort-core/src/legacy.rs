//! Byte-exact ports of ESO fitsort.c (2001) behaviour, for the `fitsort` personality.

use std::io::{self, BufRead, Write};

use crate::card::trim_end;

/// Upper-cases a key and expands `A.B.C` to `HIERARCH ESO A B C`
/// (fitsort.c `expand_hierarch_keyword`).
pub fn fitsort_key(arg: &[u8]) -> Vec<u8> {
    let up = arg.to_ascii_uppercase();
    if !up.contains(&b'.') {
        return up;
    }
    let mut out = b"HIERARCH ESO".to_vec();
    for token in up.split(|&b| b == b'.').filter(|t| !t.is_empty()) {
        out.push(b' ');
        out.extend_from_slice(token);
    }
    out
}

/// Keyword of a dfits line as fitsort.c sees it: the text before the first `=`
/// (leading `=` skipped, as `strtok` does), trailing blanks removed. Lines without
/// a following `=` never match: in C the token would still hold the newline.
pub fn fitsort_line_keyword(line: &[u8]) -> Option<&[u8]> {
    let start = line.iter().position(|&b| b != b'=')?;
    let rest = &line[start..];
    let end = rest.iter().position(|&b| b == b'=')?;
    Some(trim_end(&rest[..end]))
}

/// fitsort.c `getkeywordvalue`: quoted values verbatim between the first and the
/// last quote (trailing blanks and `''` kept), otherwise the first token.
pub fn fitsort_value(line: &[u8]) -> Vec<u8> {
    let Some(eq) = line.iter().position(|&b| b == b'=') else { return Vec::new() };
    let mut tmp = Vec::with_capacity(80);
    let mut quote = false;
    for &ch in line.iter().take(80).skip(eq + 1) {
        if ch == b'/' && !quote {
            break;
        }
        if ch == b'\'' {
            quote = !quote;
        }
        tmp.push(ch);
    }
    match (tmp.iter().position(|&b| b == b'\''), tmp.iter().rposition(|&b| b == b'\'')) {
        (Some(first), Some(last)) if last > first => tmp[first + 1..last].to_vec(),
        (Some(first), _) => tmp[first + 1..].to_vec(),
        _ => tmp.split(|b| b.is_ascii_whitespace()).find(|t| !t.is_empty()).map(<[u8]>::to_vec).unwrap_or_default(),
    }
}

/// One record (file or extension) of fitsort input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FitsortRecord {
    pub name: Vec<u8>,
    pub values: Vec<Option<Vec<u8>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FitsortTable {
    pub records: Vec<FitsortRecord>,
    /// True once a `====>` line was seen (fitsort then prints the FILE column).
    pub printnames: bool,
}

/// Parses dfits output. `keys` are already passed through [`fitsort_key`].
pub fn fitsort_read(mut input: impl BufRead, keys: &[Vec<u8>]) -> io::Result<FitsortTable> {
    let mut records: Vec<FitsortRecord> = Vec::new();
    let mut printnames = false;
    let mut line = Vec::with_capacity(128);
    while read_line(&mut input, &mut line)? {
        if line.starts_with(b"====>") {
            printnames = true;
            let name = line.split(|b| b.is_ascii_whitespace()).filter(|t| !t.is_empty()).nth(2).unwrap_or_default();
            records.push(FitsortRecord { name: name.to_vec(), values: vec![None; keys.len()] });
            read_line(&mut input, &mut line)?; // fitsort.c absorbs the line after a `====>` line
        } else if line.starts_with(b"SIMPLE  =") {
            records.push(FitsortRecord { name: Vec::new(), values: vec![None; keys.len()] });
        } else if let Some(kw) = fitsort_line_keyword(&line) {
            if let (Some(i), Some(rec)) = (keys.iter().position(|k| k.as_slice() == kw), records.last_mut()) {
                rec.values[i] = Some(fitsort_value(&line));
            }
        }
    }
    Ok(FitsortTable { records, printnames })
}

/// Reads one line without its newline into `line` (reusing the buffer); false at EOF.
fn read_line(input: &mut impl BufRead, line: &mut Vec<u8>) -> io::Result<bool> {
    line.clear();
    if input.read_until(b'\n', line)? == 0 {
        return Ok(false);
    }
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    Ok(true)
}

/// Prints the table like fitsort.c. Returns the exit status: 255 (C `return -1`)
/// when there were no records.
pub fn fitsort_write(
    out: &mut impl Write,
    labels: &[Vec<u8>],
    table: &FitsortTable,
    print_header: bool,
) -> io::Result<i32> {
    let mut widths: Vec<usize> = labels.iter().map(Vec::len).collect();
    let mut name_width = 0;
    for r in &table.records {
        name_width = name_width.max(r.name.len());
        for (w, v) in widths.iter_mut().zip(&r.values) {
            if let Some(v) = v {
                *w = (*w).max(v.len());
            }
        }
    }
    if print_header {
        if table.printnames {
            cell(out, b"FILE", name_width)?;
        }
        for (label, &w) in labels.iter().zip(&widths) {
            cell(out, label, w)?;
        }
        out.write_all(b"\n")?;
    }
    if table.records.is_empty() {
        out.write_all(b"*** error: no input data corresponding to dfits output\n")?;
        return Ok(255);
    }
    for r in &table.records {
        if table.printnames {
            cell(out, &r.name, name_width)?;
        }
        for (v, &w) in r.values.iter().zip(&widths) {
            cell(out, v.as_deref().unwrap_or(b" ".as_slice()), w)?;
        }
        out.write_all(b"\n")?;
    }
    Ok(0)
}

/// C `printf("%-*s\t", width, s)`.
fn cell(out: &mut impl Write, s: &[u8], width: usize) -> io::Result<()> {
    out.write_all(s)?;
    for _ in s.len()..width {
        out.write_all(b" ")?;
    }
    out.write_all(b"\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys() {
        assert_eq!(fitsort_key(b"dpr.catg"), b"HIERARCH ESO DPR CATG");
        assert_eq!(fitsort_key(b"naxis1"), b"NAXIS1");
        assert_eq!(fitsort_key(b"a..b"), b"HIERARCH ESO A B");
    }

    #[test]
    fn line_keywords() {
        assert_eq!(fitsort_line_keyword(b"OBJECT  = 'x'"), Some(&b"OBJECT"[..]));
        assert_eq!(fitsort_line_keyword(b"HIERARCH ESO DPR CATG = 'x'"), Some(&b"HIERARCH ESO DPR CATG"[..]));
        assert_eq!(fitsort_line_keyword(b"COMMENT no equals"), None);
    }

    #[test]
    fn values() {
        assert_eq!(fitsort_value(b"OBJECT  = 'NGC 253 '           / name"), b"NGC 253 ");
        assert_eq!(fitsort_value(b"QUOTED  = 'it''s / not a comment' / real"), b"it''s / not a comment");
        assert_eq!(fitsort_value(b"EXPTIME =                 10.0 / s"), b"10.0");
        assert_eq!(fitsort_value(b"UNDEF   ="), b"");
        assert_eq!(fitsort_value(b"BROKEN  = 'no end"), b"no end");
    }

    #[test]
    fn table() {
        let input = b"====> file a.fits (main) <====\nSIMPLE  =                    T\nNAXIS1  =                  100\nNAXIS2  =                  200\n\
====> file bb.fits (main) <====\nSIMPLE  =                    T\nNAXIS1  =                   20\n";
        let keys = vec![fitsort_key(b"NAXIS2"), fitsort_key(b"NAXIS1")];
        let table = fitsort_read(&input[..], &keys).unwrap();
        let mut out = Vec::new();
        assert_eq!(fitsort_write(&mut out, &keys, &table, true).unwrap(), 0);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "FILE   \tNAXIS2\tNAXIS1\t\na.fits \t200   \t100   \t\nbb.fits\t      \t20    \t\n"
        );
    }

    #[test]
    fn no_records() {
        let table = fitsort_read(&b"nothing\n"[..], &[]).unwrap();
        let mut out = Vec::new();
        assert_eq!(fitsort_write(&mut out, &[], &table, true).unwrap(), 255);
        assert_eq!(out, b"\n*** error: no input data corresponding to dfits output\n");
    }
}
