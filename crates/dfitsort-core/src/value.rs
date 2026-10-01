//! Parsing of keyword value fields (FITS Standard 4.0 §4.2).

use std::borrow::Cow;

use crate::card::trim_end;
use crate::numeric::json_number;

/// A keyword value. Numbers keep their original text so no digits are lost.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// String with `''` unescaped and trailing blanks removed (raw bytes; may be non-UTF-8).
    Str(Vec<u8>),
    Logical(bool),
    /// Integer, original text.
    Int(String),
    /// Real, original text (may use a `D` exponent).
    Real(String),
    /// Complex `(re, im)`, original component texts.
    Complex(String, String),
    /// Blank value field.
    Undefined,
}

impl Value {
    /// Text used for display, comparison and sorting.
    pub fn display_bytes(&self) -> Cow<'_, [u8]> {
        match self {
            Value::Str(s) => Cow::Borrowed(s),
            Value::Logical(true) => Cow::Borrowed(b"T"),
            Value::Logical(false) => Cow::Borrowed(b"F"),
            Value::Int(t) | Value::Real(t) => Cow::Borrowed(t.as_bytes()),
            Value::Complex(re, im) => Cow::Owned(format!("({re}, {im})").into_bytes()),
            Value::Undefined => Cow::Borrowed(b""),
        }
    }
}

/// Parses the bytes that follow the value indicator.
pub fn parse_value(field: &[u8]) -> Value {
    let Some(start) = field.iter().position(|&b| b != b' ') else {
        return Value::Undefined;
    };
    let field = &field[start..];
    if field[0] == b'\'' {
        return Value::Str(parse_string(&field[1..]));
    }
    let end = field.iter().position(|&b| b == b'/').unwrap_or(field.len());
    classify_token(trim_end(&field[..end]))
}

fn parse_string(after_quote: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(after_quote.len());
    let mut i = 0;
    while i < after_quote.len() {
        let b = after_quote[i];
        if b == b'\'' {
            if after_quote.get(i + 1) == Some(&b'\'') {
                out.push(b'\'');
                i += 2;
                continue;
            }
            break;
        }
        out.push(b);
        i += 1;
    }
    let keep = trim_end(&out).len();
    out.truncate(keep);
    out
}

fn classify_token(token: &[u8]) -> Value {
    if token.is_empty() {
        return Value::Undefined;
    }
    let Ok(text) = std::str::from_utf8(token) else {
        return Value::Str(token.to_vec());
    };
    match text {
        "T" => return Value::Logical(true),
        "F" => return Value::Logical(false),
        _ => {}
    }
    if let Some((re, im)) = text.strip_prefix('(').and_then(|t| t.strip_suffix(')')).and_then(|t| t.split_once(',')) {
        let (re, im) = (re.trim(), im.trim());
        if json_number(re).is_some() && json_number(im).is_some() {
            return Value::Complex(re.to_string(), im.to_string());
        }
    }
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        return Value::Int(text.to_string());
    }
    if json_number(text).is_some() {
        return Value::Real(text.to_string());
    }
    Value::Str(token.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &[u8]) -> Value {
        Value::Str(text.to_vec())
    }

    #[test]
    fn strings() {
        assert_eq!(parse_value(b" 'SCIENCE '  / cat"), s(b"SCIENCE"));
        assert_eq!(parse_value(b" 'O''HARA'"), s(b"O'HARA"));
        assert_eq!(parse_value(b" 'a / b' / c"), s(b"a / b"));
        assert_eq!(parse_value(b" '  lead'"), s(b"  lead"));
        assert_eq!(parse_value(b" ''"), s(b""));
        assert_eq!(parse_value(b" 'unterminated"), s(b"unterminated"));
    }

    #[test]
    fn scalars() {
        assert_eq!(parse_value(b"                    T"), Value::Logical(true));
        assert_eq!(parse_value(b" 6659525521533387424 / big"), Value::Int("6659525521533387424".into()));
        assert_eq!(parse_value(b" -32"), Value::Int("-32".into()));
        assert_eq!(parse_value(b" 1.5D+03"), Value::Real("1.5D+03".into()));
        assert_eq!(parse_value(b" .5"), Value::Real(".5".into()));
        assert_eq!(parse_value(b" (1.0, -2.5)"), Value::Complex("1.0".into(), "-2.5".into()));
        assert_eq!(parse_value(b"      / only comment"), Value::Undefined);
        assert_eq!(parse_value(b""), Value::Undefined);
        assert_eq!(parse_value(b" 12:30:00"), s(b"12:30:00"));
    }

    #[test]
    fn display() {
        assert_eq!(&*Value::Complex("1".into(), "2".into()).display_bytes(), b"(1, 2)");
        assert_eq!(&*Value::Logical(false).display_bytes(), b"F");
        assert_eq!(&*Value::Undefined.display_bytes(), b"");
    }
}
