//! Classification of single 80-byte header cards (FITS Standard 4.0 §4.1,
//! ESO HIERARCH convention).

/// What a card is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind<'a> {
    /// A keyword with a value. `raw_name` is not normalised; the value field
    /// starts at byte offset `value_start` within the card.
    Keyword { raw_name: &'a [u8], hierarch: bool, value_start: usize },
    /// `CONTINUE` long-string continuation; its value field starts at byte 10.
    Continue,
    /// COMMENT, HISTORY, blank keyword or anything else without a value.
    Commentary,
    /// The END card.
    End,
}

const END: &[u8] = b"END     ";

/// Classifies one card. Never panics, whatever the bytes.
pub fn classify(card: &[u8]) -> CardKind<'_> {
    if card.len() >= 8 && &card[..8] == END {
        return CardKind::End;
    }
    if card.len() < 10 {
        return CardKind::Commentary;
    }
    if card[..9].eq_ignore_ascii_case(b"HIERARCH ") {
        if let Some(eq) = card[9..].iter().position(|&b| b == b'=') {
            let raw_name = &card[9..9 + eq];
            if raw_name.iter().any(|b| !b.is_ascii_whitespace()) {
                return CardKind::Keyword { raw_name, hierarch: true, value_start: 9 + eq + 1 };
            }
        }
        return CardKind::Commentary;
    }
    let name = trim_end(&card[..8]);
    if is_commentary_name(name) {
        return CardKind::Commentary;
    }
    if card[8] == b'=' {
        // "= " is the standard value indicator; '=' without the blank is
        // non-conforming but accepted (legacy fitsort accepts it too).
        let value_start = if card[9] == b' ' { 10 } else { 9 };
        return CardKind::Keyword { raw_name: name, hierarch: false, value_start };
    }
    if name == b"CONTINUE" && card[8] == b' ' && card[9] == b' ' {
        return CardKind::Continue;
    }
    CardKind::Commentary
}

fn is_commentary_name(name: &[u8]) -> bool {
    name.is_empty() || name.eq_ignore_ascii_case(b"COMMENT") || name.eq_ignore_ascii_case(b"HISTORY")
}

/// `bytes` without trailing ASCII spaces.
pub fn trim_end(bytes: &[u8]) -> &[u8] {
    let n = bytes.iter().rposition(|&b| b != b' ').map_or(0, |i| i + 1);
    &bytes[..n]
}

/// Appends the normalised form of a keyword name: ASCII upper-case, surrounding
/// blanks removed, internal blank runs collapsed to one space.
/// Non-ASCII bytes become U+FFFD; queries are normalised the same way, so they still match.
pub fn push_normalized(raw: &[u8], out: &mut String) {
    let start = out.len();
    let mut gap = false;
    for &b in raw {
        if b.is_ascii_whitespace() {
            gap = out.len() > start;
            continue;
        }
        if gap {
            out.push(' ');
            gap = false;
        }
        out.push(if b.is_ascii() { b.to_ascii_uppercase() as char } else { char::REPLACEMENT_CHARACTER });
    }
}

/// Whether `raw` normalises (see [`push_normalized`]) to `target`, without allocating
/// for ASCII names.
pub fn normalized_eq(raw: &[u8], target: &str) -> bool {
    if !raw.is_ascii() {
        let mut name = String::new();
        push_normalized(raw, &mut name);
        return name == target;
    }
    let mut expected = target.bytes();
    let (mut gap, mut started) = (false, false);
    for &b in raw {
        if b.is_ascii_whitespace() {
            gap = started;
            continue;
        }
        if gap {
            if expected.next() != Some(b' ') {
                return false;
            }
            gap = false;
        }
        if expected.next() != Some(b.to_ascii_uppercase()) {
            return false;
        }
        started = true;
    }
    expected.next().is_none()
}

/// Normalised form of a keyword name (see [`push_normalized`]).
pub fn normalize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    push_normalized(name.as_bytes(), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::card;

    fn kw(text: &str) -> (String, bool, usize) {
        let c = card(text);
        match classify(&c) {
            CardKind::Keyword { raw_name, hierarch, value_start } => {
                let mut name = String::new();
                push_normalized(raw_name, &mut name);
                (name, hierarch, value_start)
            }
            other => panic!("expected a keyword, got {other:?} for {text:?}"),
        }
    }

    #[test]
    fn standard_keywords() {
        assert_eq!(kw("EXPTIME =                 10.0 / s"), ("EXPTIME".into(), false, 10));
        assert_eq!(kw("DATA-TYP= 'OBJECT  '").0, "DATA-TYP");
        assert_eq!(kw("FOO     =5"), ("FOO".into(), false, 9));
    }

    #[test]
    fn hierarch_keywords() {
        assert_eq!(kw("HIERARCH ESO DPR CATG = 'SCIENCE'"), ("ESO DPR CATG".into(), true, 23));
        assert_eq!(kw("HIERARCH ESO DPR TYPE= 'OBJECT'").0, "ESO DPR TYPE");
        assert_eq!(kw("HIERARCH ESO  DET DIT = 10.0").0, "ESO DET DIT");
        assert_eq!(kw("HIERARCH ESO det ndit = 6").0, "ESO DET NDIT");
        assert_eq!(kw("hierarch eso tel airm = 1.2").0, "ESO TEL AIRM");
        assert_eq!(kw("HIERARCH scaling.fiberPitch = 1.5").0, "SCALING.FIBERPITCH");
    }

    #[test]
    fn end_is_matched_exactly() {
        assert_eq!(classify(&card("END")), CardKind::End);
        for text in ["ENDTIME = '23:59:59'", "END-OBS = '23:59:59'", "ENDFRM  =                    3"] {
            assert!(matches!(classify(&card(text)), CardKind::Keyword { .. }), "{text}");
        }
    }

    #[test]
    fn commentary_cards() {
        for text in [
            "COMMENT   = not a value",
            "HISTORY created",
            "        just text",
            "HIERARCH no equals sign here",
            "NOVALUE  plain text",
        ] {
            assert_eq!(classify(&card(text)), CardKind::Commentary, "{text}");
        }
    }

    #[test]
    fn continue_cards() {
        assert_eq!(classify(&card("CONTINUE  'more text'")), CardKind::Continue);
        assert!(matches!(classify(&card("CONTINUE= 'x'")), CardKind::Keyword { .. }));
    }

    #[test]
    fn short_input_does_not_panic() {
        assert_eq!(classify(b"END"), CardKind::Commentary);
        assert_eq!(classify(b""), CardKind::Commentary);
    }

    #[test]
    fn normalisation() {
        assert_eq!(normalize_name("  hierarch  eso dpr  "), "HIERARCH ESO DPR");
        assert!(normalized_eq(b" eso  det\tdit ", "ESO DET DIT"));
        assert!(!normalized_eq(b"ESO DET DIT", "ESO DET"));
        assert!(!normalized_eq(b"ESO DET", "ESO DET DIT"));
        assert!(normalized_eq("caf\u{e9}".as_bytes(), &normalize_name("caf\u{e9}")));
        let mut shared = String::from("FIRST");
        push_normalized(b"  second  name ", &mut shared);
        assert_eq!(shared, "FIRSTSECOND NAME");
        assert_eq!(trim_end(b"AB  "), b"AB");
    }
}
