//! Table writers: text (aligned), tsv, csv and json.

use std::io::{self, Write};

use dfitsort_core::numeric::json_number;
use dfitsort_core::value::Value;

use crate::cli::Format;
use crate::table::Row;

pub trait RowWriter {
    fn row(&mut self, row: &Row) -> io::Result<()>;
    /// Writes anything buffered (and the header when there were no rows), then flushes.
    fn finish(&mut self) -> io::Result<()>;
    /// Flushes what has been written so far, so that a following stderr line appears after it.
    fn flush(&mut self) -> io::Result<()>;
}

pub fn writer<'a, W: Write + 'a>(
    format: Format,
    out: W,
    labels: &[&str],
    header: bool,
    missing: &str,
) -> Box<dyn RowWriter + 'a> {
    let mut heading = vec![b"FILE".to_vec()];
    heading.extend(labels.iter().map(|l| l.as_bytes().to_vec()));
    let heading = header.then_some(heading);
    let missing = missing.as_bytes().to_vec();
    match format {
        Format::Text => Box::new(Text { out, heading, missing, rows: Vec::new() }),
        Format::Tsv => Box::new(Delimited { out, heading, missing, csv: false }),
        Format::Csv => Box::new(Delimited { out, heading, missing, csv: true }),
        Format::Json => Box::new(Json { out, labels: labels.iter().map(|l| l.to_string()).collect(), count: 0 }),
    }
}

fn cells(row: &Row, missing: &[u8]) -> Vec<Vec<u8>> {
    let mut cells = vec![row.label()];
    cells.extend(row.values.iter().map(|v| match v {
        Some(v) => v.display_bytes().into_owned(),
        None => missing.to_vec(),
    }));
    cells
}

/// Columns padded with spaces, two-space gutter; holds all rows to size the columns.
struct Text<W> {
    out: W,
    heading: Option<Vec<Vec<u8>>>,
    missing: Vec<u8>,
    rows: Vec<Vec<Vec<u8>>>,
}

impl<W: Write> RowWriter for Text<W> {
    fn row(&mut self, row: &Row) -> io::Result<()> {
        self.rows.push(cells(row, &self.missing).iter().map(|c| printable(c)).collect());
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        let all: Vec<&Vec<Vec<u8>>> = self.heading.iter().chain(self.rows.iter()).collect();
        let ncol = all.first().map_or(0, |r| r.len());
        let mut widths = vec![0usize; ncol];
        for r in &all {
            for (w, c) in widths.iter_mut().zip(r.iter()) {
                *w = (*w).max(width(c));
            }
        }
        for r in &all {
            for (i, c) in r.iter().enumerate() {
                self.out.write_all(c)?;
                if i + 1 < ncol {
                    let pad = widths[i] - width(c) + 2;
                    self.out.write_all(&b" ".repeat(pad))?;
                }
            }
            self.out.write_all(b"\n")?;
        }
        self.out.flush()
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

/// Control bytes (a newline inside a string value, say) would break the table layout.
fn printable(cell: &[u8]) -> Vec<u8> {
    cell.iter().map(|&b| if b < 0x20 || b == 0x7f { b' ' } else { b }).collect()
}

fn width(cell: &[u8]) -> usize {
    std::str::from_utf8(cell).map_or(cell.len(), |s| s.chars().count())
}

/// TSV (tabs, no padding) or CSV (RFC 4180 quoting); streams rows.
struct Delimited<W> {
    out: W,
    heading: Option<Vec<Vec<u8>>>,
    missing: Vec<u8>,
    csv: bool,
}

impl<W: Write> Delimited<W> {
    fn line(&mut self, cells: &[Vec<u8>]) -> io::Result<()> {
        for (i, c) in cells.iter().enumerate() {
            if i > 0 {
                self.out.write_all(if self.csv { b"," } else { b"\t" })?;
            }
            if self.csv {
                write_csv_field(&mut self.out, c)?;
            } else {
                let clean: Vec<u8> =
                    c.iter().map(|&b| if matches!(b, b'\t' | b'\n' | b'\r') { b' ' } else { b }).collect();
                self.out.write_all(&clean)?;
            }
        }
        self.out.write_all(b"\n")
    }

    fn heading_once(&mut self) -> io::Result<()> {
        match self.heading.take() {
            Some(h) => self.line(&h),
            None => Ok(()),
        }
    }
}

impl<W: Write> RowWriter for Delimited<W> {
    fn row(&mut self, row: &Row) -> io::Result<()> {
        self.heading_once()?;
        let cells = cells(row, &self.missing);
        self.line(&cells)
    }

    fn finish(&mut self) -> io::Result<()> {
        self.heading_once()?;
        self.out.flush()
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

fn write_csv_field(out: &mut impl Write, field: &[u8]) -> io::Result<()> {
    if !field.iter().any(|b| matches!(b, b',' | b'"' | b'\n' | b'\r')) {
        return out.write_all(field);
    }
    out.write_all(b"\"")?;
    for &b in field {
        out.write_all(if b == b'"' { b"\"\"" } else { std::slice::from_ref(&b) })?;
    }
    out.write_all(b"\"")
}

/// JSON array of `{"file", "hdu", "extname", "values"}` objects; streams rows.
struct Json<W> {
    out: W,
    labels: Vec<String>,
    count: usize,
}

impl<W: Write> RowWriter for Json<W> {
    fn row(&mut self, row: &Row) -> io::Result<()> {
        self.out.write_all(if self.count == 0 { b"[\n" } else { b",\n" })?;
        self.count += 1;
        let mut s = format!(
            "{{\"file\":{},\"hdu\":{},\"extname\":{},\"values\":{{",
            json_str(&String::from_utf8_lossy(&row.path)),
            row.hdu,
            row.extname.as_deref().map_or_else(|| "null".to_string(), json_str),
        );
        for (i, (label, value)) in self.labels.iter().zip(&row.values).enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&json_str(label));
            s.push(':');
            s.push_str(&json_value(value.as_ref()));
        }
        s.push_str("}}");
        self.out.write_all(s.as_bytes())
    }

    fn finish(&mut self) -> io::Result<()> {
        self.out.write_all(if self.count == 0 { b"[]\n" } else { b"\n]\n" })?;
        self.out.flush()
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).expect("serialising a string cannot fail")
}

/// UTF-8 when valid, otherwise Latin-1.
fn text_of(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

fn json_value(value: Option<&Value>) -> String {
    // JSON has no infinities: a real that overflows a double (1E999) becomes null.
    let number = |t: &str| match json_number(t) {
        Some(n) if n.parse::<f64>().is_ok_and(|v| !v.is_finite()) => "null".to_string(),
        Some(n) => n,
        None => json_str(t),
    };
    match value {
        None | Some(Value::Undefined) => "null".into(),
        Some(Value::Logical(b)) => b.to_string(),
        Some(Value::Int(t)) | Some(Value::Real(t)) => number(t),
        Some(Value::Complex(re, im)) => format!("[{},{}]", number(re), number(im)),
        Some(Value::Str(s)) => json_str(&text_of(s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_bytes_become_spaces_in_text_cells() {
        assert_eq!(printable(b"a\nb\tc\x7fd\x1b[0m caf\xc3\xa9"), b"a b c d [0m caf\xc3\xa9");
    }

    #[test]
    fn non_finite_json_numbers_are_null() {
        assert_eq!(json_value(Some(&Value::Real("1E999".into()))), "null");
        assert_eq!(json_value(Some(&Value::Real("-1.0D999".into()))), "null");
        assert_eq!(json_value(Some(&Value::Complex("1.5".into(), "1E999".into()))), "[1.5,null]");
        assert_eq!(json_value(Some(&Value::Real("1.5D+03".into()))), "1.5E3");
    }
}
