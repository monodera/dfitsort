//! Logical image header of tile-compressed HDUs (FITS Standard 4.0 §10.1).

use crate::CARD_LEN;
use crate::card::{CardKind, classify, trim_end};
use crate::value::{Value, parse_value};

/// Exact keyword names removed from the logical header.
const DROP: &[&str] = &[
    "XTENSION", "BITPIX", "NAXIS", "PCOUNT", "GCOUNT", "TFIELDS", "THEAP", "CHECKSUM", "DATASUM", "ZIMAGE", "ZCMPTYPE",
    "ZQUANTIZ", "ZDITHER0", "ZSCALE", "ZZERO", "ZMASKCMP", "ZBLANK", "ZSIMPLE", "ZBITPIX", "ZNAXIS", "ZTENSION",
    "ZPCOUNT", "ZGCOUNT",
];
/// Indexed keyword families (prefix followed by digits) removed from the logical header.
const DROP_INDEXED: &[&str] = &[
    "NAXIS", "TTYPE", "TFORM", "TUNIT", "TNULL", "TSCAL", "TZERO", "TDISP", "TDIM", "ZTILE", "ZNAME", "ZVAL", "ZNAXIS",
];
/// Keywords restored under their original names.
const RENAME: &[(&str, &str)] =
    &[("ZEXTEND", "EXTEND"), ("ZBLOCKED", "BLOCKED"), ("ZHECKSUM", "CHECKSUM"), ("ZDATASUM", "DATASUM")];

/// True for an `XTENSION = 'BINTABLE'` header with `ZIMAGE = T`.
pub fn is_compressed_image(raw: &[u8]) -> bool {
    // XTENSION is always the first card; this keeps the check cheap for other HDUs.
    if !raw.starts_with(b"XTENSION= 'BINTABLE'") {
        return false;
    }
    let cards = cards(raw);
    value_of(&cards, "XTENSION") == Some(Value::Str(b"BINTABLE".to_vec()))
        && value_of(&cards, "ZIMAGE") == Some(Value::Logical(true))
}

/// Header of the original image, built from a compressed-image header.
/// The result is a sequence of cards ending with END.
pub fn logical_header(raw: &[u8]) -> Vec<u8> {
    let cards = cards(raw);
    let find = |name: &str| cards.iter().copied().find(|c| key(c) == name.as_bytes());
    let mut out = Vec::with_capacity(raw.len());
    let primary = find("ZSIMPLE");
    match primary {
        Some(c) => out.extend(renamed(c, "SIMPLE")),
        None => out.extend(fixed_card("XTENSION", "'IMAGE   '", "Image extension", false)),
    }
    if let Some(c) = find("ZBITPIX") {
        out.extend(renamed(c, "BITPIX"));
    }
    let naxis = find("ZNAXIS");
    if let Some(c) = naxis {
        out.extend(renamed(c, "NAXIS"));
    }
    let n = match naxis.map(|c| parse_value(&c[10..])) {
        Some(Value::Int(t)) => t.parse::<usize>().unwrap_or(0).min(999),
        _ => 0,
    };
    for i in 1..=n {
        if let Some(c) = find(&format!("ZNAXIS{i}")) {
            out.extend(renamed(c, &format!("NAXIS{i}")));
        }
    }
    if primary.is_none() {
        match find("ZPCOUNT") {
            Some(c) => out.extend(renamed(c, "PCOUNT")),
            None => out.extend(fixed_card("PCOUNT", "0", "number of parameters", true)),
        }
        match find("ZGCOUNT") {
            Some(c) => out.extend(renamed(c, "GCOUNT")),
            None => out.extend(fixed_card("GCOUNT", "1", "number of groups", true)),
        }
    }
    for &c in &cards {
        let k = key(c);
        if is_dropped(k) || is_compressed_extname(c) {
            continue;
        }
        match RENAME.iter().find(|(from, _)| k == from.as_bytes()) {
            Some((_, to)) => out.extend(renamed(c, to)),
            None => out.extend_from_slice(c),
        }
    }
    let mut end = b"END".to_vec();
    end.resize(CARD_LEN, b' ');
    out.extend(end);
    out
}

fn cards(raw: &[u8]) -> Vec<&[u8]> {
    raw.chunks_exact(CARD_LEN).take_while(|c| classify(c) != CardKind::End).collect()
}

/// Bytes 1-8 without trailing blanks.
fn key(card: &[u8]) -> &[u8] {
    trim_end(&card[..8])
}

fn value_of(cards: &[&[u8]], name: &str) -> Option<Value> {
    cards.iter().find_map(|c| match classify(c) {
        CardKind::Keyword { raw_name, hierarch: false, value_start } if raw_name == name.as_bytes() => {
            Some(parse_value(&c[value_start..]))
        }
        _ => None,
    })
}

fn is_dropped(key: &[u8]) -> bool {
    DROP.iter().any(|d| key == d.as_bytes())
        || DROP_INDEXED.iter().any(|p| {
            key.len() > p.len() && key.starts_with(p.as_bytes()) && key[p.len()..].iter().all(u8::is_ascii_digit)
        })
}

fn is_compressed_extname(card: &[u8]) -> bool {
    key(card) == b"EXTNAME" && parse_value(&card[10..]) == Value::Str(b"COMPRESSED_IMAGE".to_vec())
}

/// `card` with bytes 1-8 replaced by `name`.
fn renamed(card: &[u8], name: &str) -> Vec<u8> {
    let mut c = card.to_vec();
    let mut n = name.as_bytes().to_vec();
    n.resize(8, b' ');
    c[..8].copy_from_slice(&n);
    c
}

fn fixed_card(name: &str, value: &str, comment: &str, right_justify: bool) -> Vec<u8> {
    let text = if right_justify {
        format!("{name:<8}= {value:>20} / {comment}")
    } else {
        format!("{name:<8}= {value:<20} / {comment}")
    };
    let mut c = text.into_bytes();
    c.resize(CARD_LEN, b' ');
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::header;

    fn lines(raw: &[u8]) -> Vec<String> {
        raw.chunks_exact(CARD_LEN).map(|c| String::from_utf8_lossy(trim_end(c)).into_owned()).collect()
    }

    const TABLE: &[&str] = &[
        "XTENSION= 'BINTABLE'",
        "BITPIX  =                    8",
        "NAXIS   =                    2",
        "NAXIS1  =                    8",
        "NAXIS2  =                   64",
        "PCOUNT  =                 4096",
        "GCOUNT  =                    1",
        "TFIELDS =                    1",
        "TTYPE1  = 'COMPRESSED_DATA'",
        "TFORM1  = '1PB(120)'",
        "ZIMAGE  =                    T",
        "ZBITPIX =                   16 / data type of original image",
        "ZNAXIS  =                    2",
        "ZNAXIS1 =                   64",
        "ZNAXIS2 =                   64",
        "ZTILE1  =                   64",
        "ZTILE2  =                    1",
        "ZCMPTYPE= 'RICE_1  '",
        "ZNAME1  = 'BLOCKSIZE'",
        "ZVAL1   =                   32",
        "EXTNAME = 'SCI     '",
        "OBJECT  = 'NGC 253 '",
        "ZHECKSUM= 'abc'",
        "CHECKSUM= 'tablesum'",
    ];

    #[test]
    fn detects_compressed_images() {
        assert!(is_compressed_image(&header(TABLE)));
        assert!(!is_compressed_image(&header(&TABLE[..10])));
        assert!(!is_compressed_image(&header(&["XTENSION= 'IMAGE   '", "ZIMAGE  =                    T"])));
    }

    #[test]
    fn image_extension() {
        assert_eq!(
            lines(&logical_header(&header(TABLE))),
            [
                "XTENSION= 'IMAGE   '           / Image extension",
                "BITPIX  =                   16 / data type of original image",
                "NAXIS   =                    2",
                "NAXIS1  =                   64",
                "NAXIS2  =                   64",
                "PCOUNT  =                    0 / number of parameters",
                "GCOUNT  =                    1 / number of groups",
                "EXTNAME = 'SCI     '",
                "OBJECT  = 'NGC 253 '",
                "CHECKSUM= 'abc'",
                "END",
            ]
        );
    }

    #[test]
    fn primary_image_and_default_extname() {
        let mut cards = TABLE.to_vec();
        cards.retain(|c| !c.starts_with("EXTNAME"));
        cards.extend([
            "ZSIMPLE =                    T",
            "ZEXTEND =                    T",
            "EXTNAME = 'COMPRESSED_IMAGE'",
        ]);
        let out = lines(&logical_header(&header(&cards)));
        assert_eq!(out[0], "SIMPLE  =                    T");
        assert!(out.contains(&"EXTEND  =                    T".to_string()));
        assert!(
            !out.iter().any(|l| l.starts_with("PCOUNT") || l.starts_with("XTENSION") || l.contains("COMPRESSED_IMAGE"))
        );
    }
}
