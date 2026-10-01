//! HDU selection (`-x SEL`, spec §5.1) and reading of the selected headers.

use crate::compressed::{is_compressed_image, logical_header};
use crate::hdu::HduReader;
use crate::header::Header;
use crate::value::Value;
use crate::{Error, Source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HduSelector {
    /// Primary HDU only (no `-x`).
    Primary,
    /// `-x 0`: primary and every extension.
    All,
    /// `-x N`.
    Index(usize),
    /// `-x N-M`.
    Range(usize, usize),
    /// `-x NAME` (any EXTVER) or `-x NAME,VER`; NAME is stored upper-cased.
    Name { name: String, ver: Option<i64> },
}

impl HduSelector {
    pub fn parse(text: &str) -> Result<HduSelector, String> {
        let s = text.trim();
        if s.is_empty() {
            return Err("empty HDU selection".into());
        }
        if s.bytes().all(|b| b.is_ascii_digit()) {
            let n: usize = s.parse().map_err(|_| format!("bad HDU number {s:?}"))?;
            return Ok(if n == 0 { HduSelector::All } else { HduSelector::Index(n) });
        }
        if let Some((a, b)) = s.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.trim().parse::<usize>(), b.trim().parse::<usize>()) {
                if a > b {
                    return Err(format!("bad HDU range {s:?}"));
                }
                return Ok(HduSelector::Range(a, b));
            }
        }
        // `digits-` followed by anything but digits is a malformed range, not an EXTNAME.
        if s.split_once('-').is_some_and(|(a, _)| !a.trim().is_empty() && a.trim().bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(format!("bad HDU range {s:?}"));
        }
        match s.split_once(',') {
            Some((name, ver)) => {
                let ver = ver.trim().parse().map_err(|_| format!("bad EXTVER in {s:?}"))?;
                Ok(HduSelector::Name { name: name.trim().to_ascii_uppercase(), ver: Some(ver) })
            }
            None => Ok(HduSelector::Name { name: s.to_ascii_uppercase(), ver: None }),
        }
    }

    /// Highest HDU index that can match, when bounded.
    fn last_index(&self) -> Option<usize> {
        match self {
            HduSelector::Primary => Some(0),
            HduSelector::Index(n) => Some(*n),
            HduSelector::Range(_, b) => Some(*b),
            HduSelector::All | HduSelector::Name { .. } => None,
        }
    }

    /// Decision from the index alone; `None` when the header is needed.
    fn matches_index(&self, index: usize) -> Option<bool> {
        match self {
            HduSelector::Primary => Some(index == 0),
            HduSelector::All => Some(true),
            HduSelector::Index(n) => Some(index == *n),
            HduSelector::Range(a, b) => Some((*a..=*b).contains(&index)),
            HduSelector::Name { .. } => None,
        }
    }

    pub fn matches(&self, index: usize, header: &Header) -> bool {
        if let Some(decided) = self.matches_index(index) {
            return decided;
        }
        let HduSelector::Name { name, ver } = self else { unreachable!("decided by index") };
        let extname = match header.get("EXTNAME") {
            Some(Value::Str(s)) => String::from_utf8_lossy(&s).trim().to_ascii_uppercase(),
            _ => return false,
        };
        extname == *name && ver.is_none_or(|v| extver(header) == v)
    }
}

fn extver(header: &Header) -> i64 {
    match header.get("EXTVER") {
        Some(Value::Int(t)) => t.parse().unwrap_or(1),
        _ => 1,
    }
}

/// A selected HDU: physical index and header.
#[derive(Debug, Clone)]
pub struct SelectedHdu {
    pub index: usize,
    pub header: Header,
}

/// The HDUs selected in one file, and the error that stopped reading, if any.
#[derive(Debug)]
pub struct FileHdus {
    pub hdus: Vec<SelectedHdu>,
    pub error: Option<Error>,
}

/// Reads the HDUs chosen by `selector`. With `logical`, tile-compressed images are
/// returned with their reconstructed image header.
pub fn read_selected(source: Source, selector: &HduSelector, logical: bool) -> FileHdus {
    let mut reader = HduReader::new(source);
    let mut hdus = Vec::new();
    loop {
        let hdu = match reader.next_hdu() {
            Ok(Some(hdu)) => hdu,
            Ok(None) => break,
            Err(e) => return FileHdus { hdus, error: Some(e) },
        };
        if selector.last_index().is_some_and(|last| hdu.index > last) {
            break;
        }
        let truncated = hdu.is_truncated();
        if selector.matches_index(hdu.index) != Some(false) {
            let converted = logical && is_compressed_image(&hdu.raw);
            // The logical header drops EXTNAME = 'COMPRESSED_IMAGE', which astropy still finds by
            // name, so a name selector also tries the stored header of a converted HDU.
            let stored =
                (converted && matches!(selector, HduSelector::Name { .. })).then(|| Header::parse(hdu.raw.clone()));
            let header = Header::parse(if converted { logical_header(&hdu.raw) } else { hdu.raw });
            if selector.matches(hdu.index, &header) || stored.is_some_and(|s| selector.matches(hdu.index, &s)) {
                hdus.push(SelectedHdu { index: hdu.index, header });
            }
        }
        if selector.last_index() == Some(hdu.index) {
            // Stopping here is not an excuse to hide that the file ends inside this header.
            let error = if truncated { reader.next_hdu().err() } else { None };
            return FileHdus { hdus, error };
        }
    }
    FileHdus { hdus, error: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Error, Source,
        testkit::{data, header},
    };
    use std::io::Cursor;

    fn ext(name: &str, ver: i64) -> Vec<u8> {
        let (extname, extver) = (format!("EXTNAME = '{name}'"), format!("EXTVER  = {ver:>20}"));
        let mut f = header(&[
            "XTENSION= 'IMAGE   '",
            "BITPIX  =                    8",
            "NAXIS   =                    1",
            "NAXIS1  =                   10",
            "PCOUNT  =                    0",
            "GCOUNT  =                    1",
            &extname,
            &extver,
        ]);
        f.extend(data(10));
        f
    }

    fn file() -> Vec<u8> {
        let mut f = header(&[
            "SIMPLE  =                    T",
            "BITPIX  =                    8",
            "NAXIS   =                    0",
        ]);
        for (name, ver) in [("SCI", 1), ("ERR", 1), ("SCI", 2)] {
            f.extend(ext(name, ver));
        }
        f
    }

    fn indices(bytes: Vec<u8>, sel: &str) -> Vec<usize> {
        let selector = if sel.is_empty() { HduSelector::Primary } else { HduSelector::parse(sel).unwrap() };
        let source = Source::from_reader(Cursor::new(bytes)).unwrap();
        let out = read_selected(source, &selector, true);
        assert!(out.error.is_none());
        out.hdus.iter().map(|h| h.index).collect()
    }

    #[test]
    fn parsing() {
        assert_eq!(HduSelector::parse("0"), Ok(HduSelector::All));
        assert_eq!(HduSelector::parse("3"), Ok(HduSelector::Index(3)));
        assert_eq!(HduSelector::parse("1-3"), Ok(HduSelector::Range(1, 3)));
        assert_eq!(HduSelector::parse(" 1 - 3"), Ok(HduSelector::Range(1, 3)));
        assert_eq!(HduSelector::parse("sci,2"), Ok(HduSelector::Name { name: "SCI".into(), ver: Some(2) }));
        assert_eq!(HduSelector::parse("SCI-A"), Ok(HduSelector::Name { name: "SCI-A".into(), ver: None }));
        assert!(HduSelector::parse("3-1").is_err());
        for bad in ["1-", "1-x", "12-abc"] {
            assert!(HduSelector::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(HduSelector::parse("CHIP-1"), Ok(HduSelector::Name { name: "CHIP-1".into(), ver: None }));
        assert_eq!(HduSelector::parse("-3"), Ok(HduSelector::Name { name: "-3".into(), ver: None }));
        assert!(HduSelector::parse("SCI,x").is_err());
        assert!(HduSelector::parse(" ").is_err());
        assert_eq!(HduSelector::parse(" sci "), Ok(HduSelector::Name { name: "SCI".into(), ver: None }));
    }

    #[test]
    fn selection() {
        assert_eq!(indices(file(), ""), [0]);
        assert_eq!(indices(file(), "0"), [0, 1, 2, 3]);
        assert_eq!(indices(file(), "2"), [2]);
        assert_eq!(indices(file(), "1-2"), [1, 2]);
        assert_eq!(indices(file(), "sci"), [1, 3]);
        assert_eq!(indices(file(), "SCI,2"), [3]);
        assert_eq!(indices(file(), "SCI,1"), [1]);
        assert!(indices(file(), "9").is_empty());
        assert!(indices(file(), "NOPE").is_empty());
    }

    #[test]
    fn compressed_image_name_matches_the_stored_header() {
        let mut bytes = header(&[
            "SIMPLE  =                    T",
            "BITPIX  =                    8",
            "NAXIS   =                    0",
        ]);
        bytes.extend(header(&[
            "XTENSION= 'BINTABLE'",
            "BITPIX  =                    8",
            "NAXIS   =                    2",
            "NAXIS1  =                    8",
            "NAXIS2  =                    1",
            "PCOUNT  =                    0",
            "GCOUNT  =                    1",
            "TFIELDS =                    1",
            "ZIMAGE  =                    T",
            "ZBITPIX =                   16",
            "ZNAXIS  =                    1",
            "ZNAXIS1 =                   10",
            "EXTNAME = 'COMPRESSED_IMAGE'",
        ]));
        bytes.extend(data(8));
        assert_eq!(indices(bytes.clone(), "COMPRESSED_IMAGE"), [1]);
        assert_eq!(indices(bytes, "compressed_image"), [1]);
    }

    #[test]
    fn error_keeps_hdus_read_so_far() {
        let mut bytes = file();
        bytes.truncate(2880 * 3 + 100); // cuts the header of HDU 2
        let out = read_selected(Source::from_reader(Cursor::new(bytes)).unwrap(), &HduSelector::All, true);
        assert_eq!(out.hdus.iter().map(|h| h.index).collect::<Vec<_>>(), [0, 1, 2]); // the partial header of HDU 2 is kept
        assert!(matches!(out.error, Some(Error::TruncatedHeader { hdu: 2 })));
    }
}
