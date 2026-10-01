//! `KEY OP VALUE` row conditions (spec §5.5).

use std::cmp::Ordering;

use crate::header::Header;
use crate::numeric::{cmp_num, parse_num};
use crate::query::KeySpec;
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Contains,
}

/// Two-character operators come first so that `<=` is not read as `<`.
const OPS: [(&str, Op); 7] =
    [("<=", Op::Le), (">=", Op::Ge), ("!=", Op::Ne), ("=", Op::Eq), ("<", Op::Lt), (">", Op::Gt), ("~", Op::Contains)];

#[derive(Debug, Clone)]
pub struct Condition {
    key: KeySpec,
    op: Op,
    rhs: String,
}

impl Condition {
    /// Parses `KEY OP VALUE`; OP is the first operator found in the text.
    /// One pair of surrounding quotes (`'` or `"`) is removed from VALUE.
    pub fn parse(text: &str, ns: &str) -> Result<Condition, String> {
        let (pos, symbol, op) = text
            .char_indices()
            .find_map(|(i, _)| OPS.iter().find(|(s, _)| text[i..].starts_with(s)).map(|&(s, op)| (i, s, op)))
            .ok_or_else(|| format!("no operator in condition {text:?} (use = != < <= > >= ~)"))?;
        let key = text[..pos].trim();
        if key.is_empty() {
            return Err(format!("missing keyword in condition {text:?}"));
        }
        let rhs = unquote(text[pos + symbol.len()..].trim());
        Ok(Condition { key: KeySpec::new(key, ns), op, rhs: rhs.to_string() })
    }

    pub fn key(&self) -> &KeySpec {
        &self.key
    }

    /// False when the keyword is missing or undefined, whatever the operator.
    pub fn matches(&self, header: &Header) -> bool {
        match self.key.value(header) {
            None | Some(Value::Undefined) => false,
            Some(v) => self.matches_text(&String::from_utf8_lossy(&v.display_bytes())),
        }
    }

    fn matches_text(&self, lhs: &str) -> bool {
        if self.op == Op::Contains {
            return lhs.contains(self.rhs.as_str());
        }
        let ord = match (parse_num(lhs), parse_num(&self.rhs)) {
            (Some(a), Some(b)) => match cmp_num(a, b) {
                Some(o) => o,
                None => return false,
            },
            _ => lhs.cmp(self.rhs.as_str()),
        };
        match self.op {
            Op::Eq => ord == Ordering::Equal,
            Op::Ne => ord != Ordering::Equal,
            Op::Lt => ord == Ordering::Less,
            Op::Le => ord != Ordering::Greater,
            Op::Gt => ord == Ordering::Greater,
            Op::Ge => ord != Ordering::Less,
            Op::Contains => unreachable!("handled above"),
        }
    }
}

fn unquote(s: &str) -> &str {
    for q in ['\'', '"'] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return &s[1..s.len() - 1];
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::header;

    fn m(condition: &str) -> bool {
        let hd = Header::parse(header(&[
            "EXPTIME =                 60.0",
            "HIERARCH ESO DPR CATG = 'SCIENCE '",
            "OBJECT  = 'NGC 253 '",
            "BIGINT  =     9007199254740993",
            "SIMPLE  =                    T",
            "UNDEF   =",
        ]));
        Condition::parse(condition, "ESO").unwrap().matches(&hd)
    }

    #[test]
    fn numeric() {
        assert!(m("EXPTIME>=60"));
        assert!(m("EXPTIME=6e1"));
        assert!(!m("EXPTIME>60"));
        assert!(m("exptime<100"));
        assert!(m("BIGINT>9007199254740992"));
    }

    #[test]
    fn strings() {
        assert!(m("DPR.CATG=SCIENCE"));
        assert!(m("DPR.CATG='SCIENCE'"));
        assert!(!m("DPR.CATG=science"));
        assert!(m("OBJECT~253"));
        assert!(m("OBJECT!=BIAS"));
        assert!(m("SIMPLE=T"));
    }

    #[test]
    fn missing_or_undefined_never_match() {
        assert!(!m("NOPE!=X"));
        assert!(!m("UNDEF="));
        assert!(!m("NOPE.KEY=1"));
    }

    #[test]
    fn parse_errors() {
        assert!(Condition::parse("EXPTIME", "ESO").is_err());
        assert!(Condition::parse("=5", "ESO").is_err());
    }
}
