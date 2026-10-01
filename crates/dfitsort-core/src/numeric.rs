//! Numbers: comparison/sorting helpers and JSON number normalisation.

use std::cmp::Ordering;

/// A parsed number. Integers are kept exact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Num {
    Int(i128),
    Real(f64),
}

impl Num {
    fn as_f64(self) -> f64 {
        match self {
            Num::Int(i) => i as f64,
            Num::Real(r) => r,
        }
    }
}

/// Parses FITS integer or real text (`D` exponent accepted).
pub fn parse_num(text: &str) -> Option<Num> {
    let t = text.trim();
    let digits = t.strip_prefix(['+', '-']).unwrap_or(t);
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(i) = t.parse::<i128>() {
            return Some(Num::Int(i));
        }
    }
    json_number(t)?.parse::<f64>().ok().map(Num::Real)
}

/// Compares two numbers; integers exactly, otherwise as `f64`.
pub fn cmp_num(a: Num, b: Num) -> Option<Ordering> {
    match (a, b) {
        (Num::Int(x), Num::Int(y)) => Some(x.cmp(&y)),
        _ => a.as_f64().partial_cmp(&b.as_f64()),
    }
}

/// Rewrites FITS number text as a valid JSON number without losing digits:
/// no leading `+` or zeros, `D` exponent → `E`, `.5` → `0.5`, `5.` → `5.0`.
/// Returns `None` if the text is not a number.
pub fn json_number(text: &str) -> Option<String> {
    let (sign, rest) = split_sign(text.trim());
    let (mantissa, exponent) = match rest.find(['E', 'e', 'D', 'd']) {
        Some(i) => (&rest[..i], Some(&rest[i + 1..])),
        None => (rest, None),
    };
    let (int_part, frac_part) = match mantissa.find('.') {
        Some(i) => (&mantissa[..i], Some(&mantissa[i + 1..])),
        None => (mantissa, None),
    };
    let all_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(int_part) || !frac_part.is_none_or(all_digits) {
        return None;
    }
    if int_part.is_empty() && frac_part.is_none_or(str::is_empty) {
        return None;
    }
    let mut out = String::with_capacity(text.len() + 2);
    out.push_str(sign);
    out.push_str(strip_leading_zeros(int_part));
    if let Some(frac) = frac_part {
        out.push('.');
        out.push_str(if frac.is_empty() { "0" } else { frac });
    }
    if let Some(exp) = exponent {
        let (exp_sign, exp_digits) = split_sign(exp);
        if exp_digits.is_empty() || !all_digits(exp_digits) {
            return None;
        }
        out.push('E');
        out.push_str(exp_sign);
        out.push_str(strip_leading_zeros(exp_digits));
    }
    Some(out)
}

fn split_sign(s: &str) -> (&'static str, &str) {
    match s.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", s.strip_prefix('+').unwrap_or(s)),
    }
}

fn strip_leading_zeros(s: &str) -> &str {
    let t = s.trim_start_matches('0');
    if t.is_empty() { "0" } else { t }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_numbers() {
        for (input, expected) in [
            ("1.5D+03", "1.5E3"),
            (".5", "0.5"),
            ("5.", "5.0"),
            ("+007", "7"),
            ("-0.0", "-0.0"),
            ("1e-05", "1E-5"),
            ("6659525521533387424", "6659525521533387424"),
        ] {
            assert_eq!(json_number(input).as_deref(), Some(expected), "{input}");
        }
        for bad in ["", ".", "E5", "1.2.3", "NaN", "inf", "1E", "0x10", "12:30", "+"] {
            assert_eq!(json_number(bad), None, "{bad}");
        }
    }

    #[test]
    fn parsing_and_comparison() {
        assert_eq!(parse_num("6659525521533387424"), Some(Num::Int(6659525521533387424)));
        assert_eq!(parse_num("1.5D+03"), Some(Num::Real(1500.0)));
        assert_eq!(parse_num("abc"), None);
        assert_eq!(cmp_num(Num::Int(10), Num::Real(9.5)), Some(Ordering::Greater));
        let (a, b) = (parse_num("9007199254740993").unwrap(), parse_num("9007199254740992").unwrap());
        assert_eq!(cmp_num(a, b), Some(Ordering::Greater));
    }
}
