//! Data unit size from the mandatory keywords (FITS Standard 4.0 Eq. 2 and Eq. 4).

use crate::{BLOCK_LEN, CARD_LEN, Error, Result};

/// Padded size in bytes of the data unit described by `raw` (header cards).
/// Mandatory keywords are looked up wherever they are; their order is not checked.
pub fn data_unit_size(raw: &[u8], hdu: usize) -> Result<u64> {
    let bad = |reason: String| Error::BadSize { hdu, reason };
    let (mut bitpix, mut naxis, mut pcount, mut gcount, mut groups) = (None, None, 0i64, 1i64, false);
    let mut axes: Vec<Option<i64>> = Vec::new();
    for card in raw.chunks_exact(CARD_LEN) {
        let key = &card[..8];
        match key {
            b"BITPIX  " => {
                if card[8] == b'=' && int_value(card).is_none() {
                    return Err(bad("BITPIX is not an integer".into()));
                }
                bitpix = int_value(card);
            }
            b"NAXIS   " => {
                if card[8] == b'=' && int_value(card).is_none() {
                    return Err(bad("NAXIS is not an integer".into()));
                }
                naxis = int_value(card);
            }
            b"PCOUNT  " => {
                if card[8] == b'=' && int_value(card).is_none() {
                    return Err(bad("PCOUNT is not an integer".into()));
                }
                pcount = int_value(card).unwrap_or(0);
            }
            b"GCOUNT  " => {
                if card[8] == b'=' && int_value(card).is_none() {
                    return Err(bad("GCOUNT is not an integer".into()));
                }
                gcount = int_value(card).unwrap_or(1);
            }
            b"GROUPS  " => groups = card[8] == b'=' && value_text(card) == "T",
            b"END     " => break,
            _ => {
                if let Some(n) = key.strip_prefix(b"NAXIS").and_then(axis_number) {
                    if axes.len() < n {
                        axes.resize(n, None);
                    }
                    axes[n - 1] = int_value(card);
                }
            }
        }
    }
    let naxis = naxis.unwrap_or(0);
    // Eq. 1 (primary) has no PCOUNT term; a PCOUNT heap with NAXIS = 0 exists only in conforming extensions.
    if naxis == 0 && (pcount <= 0 || !raw.starts_with(b"XTENSION")) {
        return Ok(0);
    }
    if !(0..=999).contains(&naxis) {
        return Err(bad(format!("NAXIS = {naxis}")));
    }
    let bitpix = bitpix.ok_or_else(|| bad("missing BITPIX".into()))?;
    if bitpix == 0 || bitpix % 8 != 0 {
        return Err(bad(format!("BITPIX = {bitpix}")));
    }
    let mut dims = Vec::with_capacity(naxis as usize);
    for n in 1..=naxis as usize {
        let v = axes.get(n - 1).copied().flatten().ok_or_else(|| bad(format!("missing NAXIS{n}")))?;
        if v < 0 {
            return Err(bad(format!("NAXIS{n} = {v}")));
        }
        dims.push(v as u64);
    }
    if pcount < 0 || gcount < 0 {
        return Err(bad(format!("PCOUNT = {pcount}, GCOUNT = {gcount}")));
    }
    let overflow = || bad("size does not fit in 64 bits".into());
    // Random groups (Eq. 4): NAXIS1 = 0 is not part of the product.
    // With NAXIS = 0 there is no array, only the PCOUNT bytes (Eq. 2).
    let counted = if groups && dims.first() == Some(&0) { &dims[1..] } else { &dims[..] };
    let mut product: u64 = if dims.is_empty() { 0 } else { 1 };
    for &d in counted {
        product = product.checked_mul(d).ok_or_else(overflow)?;
    }
    let bytes = (pcount as u64)
        .checked_add(product)
        .and_then(|n| n.checked_mul(gcount as u64))
        .and_then(|n| n.checked_mul(bitpix.unsigned_abs() / 8))
        .ok_or_else(overflow)?;
    bytes.div_ceil(BLOCK_LEN as u64).checked_mul(BLOCK_LEN as u64).ok_or_else(overflow)
}

fn value_text(card: &[u8]) -> &str {
    let field = &card[9..];
    let end = field.iter().position(|&b| b == b'/').unwrap_or(field.len());
    std::str::from_utf8(&field[..end]).unwrap_or("").trim()
}

fn int_value(card: &[u8]) -> Option<i64> {
    if card[8] != b'=' {
        return None;
    }
    value_text(card).parse().ok()
}

fn axis_number(suffix: &[u8]) -> Option<usize> {
    let s = std::str::from_utf8(suffix).ok()?.trim_end();
    if s.is_empty() || s.starts_with('0') || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok().filter(|n| (1..=999).contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::header;

    fn size(cards: &[&str]) -> Result<u64> {
        data_unit_size(&header(cards), 0)
    }

    #[test]
    fn images_and_tables() {
        assert_eq!(
            size(&[
                "SIMPLE  =                    T",
                "BITPIX  =                    8",
                "NAXIS   =                    0"
            ])
            .unwrap(),
            0
        );
        let image = [
            "BITPIX  =                   16",
            "NAXIS   =                    2",
            "NAXIS1  =                  100",
            "NAXIS2  =                   30",
        ];
        assert_eq!(size(&image).unwrap(), 8640);
        let heap = [
            "XTENSION= 'BINTABLE'",
            "BITPIX  =                    8",
            "NAXIS   =                    2",
            "NAXIS1  =                    8",
            "NAXIS2  =                  100",
            "PCOUNT  =                 5000",
            "GCOUNT  =                    1",
        ];
        assert_eq!(size(&heap).unwrap(), 8640);
    }

    #[test]
    fn random_groups_skip_naxis1() {
        let groups = [
            "SIMPLE  =                    T",
            "BITPIX  =                  -32",
            "NAXIS   =                    3",
            "NAXIS1  =                    0",
            "NAXIS2  =                    2",
            "NAXIS3  =                    3",
            "GROUPS  =                    T",
            "PCOUNT  =                    4",
            "GCOUNT  =                  100",
        ];
        assert_eq!(size(&groups).unwrap(), 5760); // 4 * 100 * (4 + 6) = 4000 bytes
    }

    #[test]
    fn order_of_mandatory_keywords_is_not_enforced() {
        let cards = [
            "NAXIS2  =                   10",
            "ENDTIME = '23:59:59'",
            "BITPIX  =                    8",
            "NAXIS   =                    2",
            "NAXIS1  =                  300",
        ];
        assert_eq!(size(&cards).unwrap(), 5760);
    }

    #[test]
    fn invalid_sizes() {
        assert!(matches!(
            size(&[
                "BITPIX  =                    8",
                "NAXIS   =                    2",
                "NAXIS1  =                   10"
            ]),
            Err(Error::BadSize { .. })
        ));
        assert!(matches!(
            size(&[
                "BITPIX  =                    7",
                "NAXIS   =                    1",
                "NAXIS1  =                   10"
            ]),
            Err(Error::BadSize { .. })
        ));
        let huge = [
            "BITPIX  =                   64",
            "NAXIS   =                    2",
            "NAXIS1  =  9223372036854775807",
            "NAXIS2  =                    4",
        ];
        assert!(matches!(size(&huge), Err(Error::BadSize { .. })));
    }

    #[test]
    fn naxis_zero_with_pcount() {
        let with_heap = [
            "XTENSION= 'BINTABLE'",
            "BITPIX  =                    8",
            "NAXIS   =                    0",
            "PCOUNT  =                 3000",
            "GCOUNT  =                    1",
        ];
        assert_eq!(size(&with_heap).unwrap(), 5760);
        assert_eq!(size(&["BITPIX  =                    8", "NAXIS   =                    0"]).unwrap(), 0);
        assert!(matches!(
            size(&["XTENSION= 'BINTABLE'", "NAXIS   =                    0", "PCOUNT  =                   10"]),
            Err(Error::BadSize { .. })
        ));
    }

    #[test]
    fn primary_ignores_pcount() {
        let stray = [
            "SIMPLE  =                    T",
            "BITPIX  =                    8",
            "NAXIS   =                    0",
            "PCOUNT  =                 3000",
        ];
        assert_eq!(size(&stray).unwrap(), 0);
        let no_bitpix =
            ["SIMPLE  =                    T", "NAXIS   =                    0", "PCOUNT  =                 3000"];
        assert_eq!(size(&no_bitpix).unwrap(), 0);
        assert_eq!(size(&["SIMPLE  =                    T", "NAXIS   =                    0"]).unwrap(), 0);
    }

    #[test]
    fn degenerate_axes() {
        let negative =
            ["BITPIX  =                    8", "NAXIS   =                    1", "NAXIS1  =                  -10"];
        assert!(matches!(size(&negative), Err(Error::BadSize { .. })));
        let many = ["BITPIX  =                    8", "NAXIS   =                 1000"];
        assert!(matches!(size(&many), Err(Error::BadSize { .. })));
        let empty =
            ["BITPIX  =                    8", "NAXIS   =                    1", "NAXIS1  =                    0"];
        assert_eq!(size(&empty).unwrap(), 0);
    }

    #[test]
    fn malformed_mandatory_values_are_errors() {
        assert!(matches!(
            size(&["BITPIX  =                    8", "NAXIS   = 'two'", "NAXIS1  =                   10",]),
            Err(Error::BadSize { .. })
        ));
        assert!(matches!(
            size(&["BITPIX  = sixteen", "NAXIS   =                    1", "NAXIS1  =                   10",]),
            Err(Error::BadSize { .. })
        ));
        assert!(matches!(
            size(&[
                "BITPIX  =                    8",
                "NAXIS   =                    1",
                "NAXIS1  =                   10",
                "PCOUNT  = x",
            ]),
            Err(Error::BadSize { .. })
        ));
        assert!(matches!(
            size(&[
                "BITPIX  =                    8",
                "NAXIS   =                    1",
                "NAXIS1  =                   10",
                "GCOUNT  = 1.5",
            ]),
            Err(Error::BadSize { .. })
        ));
    }
}
