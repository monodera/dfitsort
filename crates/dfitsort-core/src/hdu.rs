//! Sequential reading of HDU headers, skipping data units.

use crate::card::{CardKind, classify};
use crate::size::data_unit_size;
use crate::{BLOCK_LEN, CARD_LEN, Error, Result, Source};

/// One HDU's header as stored in the file.
#[derive(Debug, Clone)]
pub struct RawHdu {
    /// Physical HDU number (primary = 0).
    pub index: usize,
    /// Header cards up to and including END (a multiple of 80 bytes).
    pub raw: Vec<u8>,
    /// Padded size of the following data unit in bytes.
    pub data_size: u64,
}

/// Iterates over the HDUs of one FITS stream.
pub struct HduReader {
    source: Source,
    next_index: usize,
    pending_skip: u64,
    finished: bool,
    /// A data-size error found after a header was returned; reported by the next call.
    deferred: Option<Error>,
}

impl HduReader {
    pub fn new(source: Source) -> Self {
        HduReader { source, next_index: 0, pending_skip: 0, finished: false, deferred: None }
    }

    /// Reads the next HDU header; `Ok(None)` after the last HDU. Bytes after the
    /// last HDU that do not start with `XTENSION` are ignored (Standard §3.5), and
    /// so is a data unit cut short by EOF. A cut `XTENSION` header is an error. A header
    /// whose data size cannot be computed is still returned; the size error comes from the
    /// next call, which ends the file.
    pub fn next_hdu(&mut self) -> Result<Option<RawHdu>> {
        if let Some(e) = self.deferred.take() {
            self.finished = true;
            return Err(e);
        }
        if self.finished {
            return Ok(None);
        }
        let result = self.read_hdu();
        if !matches!(result, Ok(Some(_))) {
            self.finished = true;
        }
        result
    }

    fn read_hdu(&mut self) -> Result<Option<RawHdu>> {
        let index = self.next_index;
        self.source.skip(std::mem::take(&mut self.pending_skip))?;
        let mut block = vec![0u8; BLOCK_LEN];
        let mut n = self.source.read_full(&mut block)?;
        if index == 0 {
            if n < CARD_LEN {
                return Err(Error::TooShort);
            }
            if !block.starts_with(b"SIMPLE  =") {
                return Err(Error::NotFits);
            }
        } else if !block[..n].starts_with(b"XTENSION") {
            return Ok(None);
        }
        let mut raw = Vec::with_capacity(BLOCK_LEN);
        loop {
            // A short last block (a file not padded to 2880 bytes) is accepted when END is
            // among its whole cards.
            for card in block[..n].chunks_exact(CARD_LEN) {
                raw.extend_from_slice(card);
                if classify(card) == CardKind::End {
                    self.next_index += 1;
                    let hdu = match data_unit_size(&raw, index) {
                        Ok(data_size) => {
                            self.pending_skip = data_size;
                            RawHdu { index, raw, data_size }
                        }
                        Err(e) => {
                            self.deferred = Some(e);
                            RawHdu { index, raw, data_size: 0 }
                        }
                    };
                    return Ok(Some(hdu));
                }
            }
            if n < BLOCK_LEN {
                return Err(Error::TruncatedHeader { hdu: index });
            }
            n = self.source.read_full(&mut block)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{card, data, header};
    use std::io::{Cursor, Write};

    fn primary(extra: &[&str]) -> Vec<u8> {
        let mut cards = vec![
            "SIMPLE  =                    T",
            "BITPIX  =                   16",
            "NAXIS   =                    2",
            "NAXIS1  =                   10",
            "NAXIS2  =                   10",
        ];
        cards.extend_from_slice(extra);
        let mut f = header(&cards);
        f.extend(data(200));
        f
    }

    fn image_ext(name: &str) -> Vec<u8> {
        let extname = format!("EXTNAME = '{name}'");
        let mut f = header(&[
            "XTENSION= 'IMAGE   '",
            "BITPIX  =                   16",
            "NAXIS   =                    2",
            "NAXIS1  =                  100",
            "NAXIS2  =                   30",
            "PCOUNT  =                    0",
            "GCOUNT  =                    1",
            &extname,
        ]);
        f.extend(data(6000));
        f
    }

    fn read_all(mut reader: HduReader) -> Vec<RawHdu> {
        let mut hdus = Vec::new();
        while let Some(h) = reader.next_hdu().unwrap() {
            hdus.push(h);
        }
        hdus
    }

    fn stream(bytes: Vec<u8>) -> HduReader {
        HduReader::new(Source::from_reader(Cursor::new(bytes)).unwrap())
    }

    fn file(bytes: &[u8]) -> (tempfile::NamedTempFile, HduReader) {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(bytes).unwrap();
        let reader = HduReader::new(Source::open(f.path()).unwrap());
        (f, reader)
    }

    fn contains(raw: &[u8], text: &str) -> bool {
        raw.windows(text.len()).any(|w| w == text.as_bytes())
    }

    #[test]
    fn primary_and_extensions() {
        let mut bytes = primary(&[]);
        bytes.extend(image_ext("A"));
        bytes.extend(image_ext("B"));
        let hdus = read_all(stream(bytes.clone()));
        assert_eq!(hdus.iter().map(|h| (h.index, h.data_size)).collect::<Vec<_>>(), [(0, 2880), (1, 8640), (2, 8640)]);
        assert!(contains(&hdus[2].raw, "EXTNAME = 'B'"));
        assert!(hdus[0].raw.ends_with(&card("END")));
        let (_f, reader) = file(&bytes);
        assert_eq!(read_all(reader).len(), 3);
    }

    #[test]
    fn end_prefixed_keywords_do_not_end_the_header() {
        let mut bytes = primary(&["ENDTIME = '23:59:59'", "END-OBS = 'x'", "AFTER   = 1"]);
        bytes.extend(image_ext("NEXT"));
        let hdus = read_all(stream(bytes));
        assert_eq!(hdus.len(), 2);
        assert!(contains(&hdus[0].raw, "AFTER   = 1"));
    }

    #[test]
    fn bytes_in_data_that_look_like_a_header_are_skipped() {
        let mut bytes = header(&[
            "SIMPLE  =                    T",
            "BITPIX  =                    8",
            "NAXIS   =                    1",
            "NAXIS1  =                 3000",
        ]);
        let mut d = data(3000);
        d[1600..1680].copy_from_slice(&card("XTENSION= 'FAKE    '"));
        bytes.extend(d);
        bytes.extend(image_ext("REAL"));
        let hdus = read_all(stream(bytes));
        assert_eq!(hdus.len(), 2);
        assert!(contains(&hdus[1].raw, "EXTNAME = 'REAL'"));
    }

    #[test]
    fn trailing_bytes_after_the_last_hdu_are_ignored() {
        for tail in [vec![0u8; 1000], vec![0u8; 2880], b"JUNK".repeat(720), Vec::new()] {
            let mut bytes = primary(&[]);
            bytes.extend(&tail);
            assert_eq!(read_all(stream(bytes)).len(), 1);
        }
        let mut cut = primary(&[]);
        cut.extend(image_ext("CUT"));
        cut.truncate(cut.len() - 4000); // data of the extension cut short
        assert_eq!(read_all(stream(cut)).len(), 2);
    }

    #[test]
    fn header_larger_than_the_initial_read() {
        let cards: Vec<String> = (0..3000).map(|k| format!("HIERARCH ESO INS TEMP{k} VAL = {k}.5")).collect();
        let refs: Vec<&str> = cards.iter().map(String::as_str).collect();
        let mut bytes = primary(&refs);
        bytes.extend(image_ext("AFTERBIG"));
        let (_f, reader) = file(&bytes);
        let hdus = read_all(reader);
        assert_eq!(hdus.len(), 2);
        assert_eq!(hdus[0].raw.len(), (5 + 3000 + 1) * CARD_LEN);
        assert!(contains(&hdus[1].raw, "AFTERBIG"));
    }

    #[test]
    fn gzip_stream() {
        let mut bytes = primary(&[]);
        bytes.extend(image_ext("GZ"));
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(&bytes).unwrap();
        assert_eq!(read_all(stream(enc.finish().unwrap())).len(), 2);
    }

    #[test]
    fn unpadded_files_and_bad_sizes() {
        let mut unpadded: Vec<u8> =
            ["SIMPLE  =                    T", "BITPIX  =                    8", "NAXIS   =                    0"]
                .iter()
                .flat_map(|c| card(c))
                .collect();
        unpadded.extend(card("END"));
        let hdus = read_all(stream(unpadded));
        assert_eq!(hdus.len(), 1);
        assert_eq!(hdus[0].raw.len(), 4 * CARD_LEN);
        let mut reader =
            stream(header(&["SIMPLE  =                    T", "BITPIX  =                    8", "NAXIS   = 'two'"]));
        let hdu = reader.next_hdu().unwrap().unwrap();
        assert!(contains(&hdu.raw, "NAXIS   = 'two'"));
        assert!(matches!(reader.next_hdu(), Err(Error::BadSize { hdu: 0, .. })));
        assert!(matches!(reader.next_hdu(), Ok(None)));
    }

    #[test]
    fn errors() {
        assert!(matches!(stream(Vec::new()).next_hdu(), Err(Error::TooShort)));
        assert!(matches!(stream(b"hello".to_vec()).next_hdu(), Err(Error::TooShort)));
        assert!(matches!(stream(vec![b'x'; 3000]).next_hdu(), Err(Error::NotFits)));
        let no_end: Vec<u8> = (0..36).flat_map(|k| card(&format!("KEY{k:<5}= {k}"))).collect();
        let mut truncated = card("SIMPLE  =                    T");
        truncated.extend(&no_end[80..]);
        let mut reader = stream(truncated);
        assert!(matches!(reader.next_hdu(), Err(Error::TruncatedHeader { hdu: 0 })));
        assert!(matches!(reader.next_hdu(), Ok(None)));
        let mut cut_ext = primary(&[]);
        cut_ext.extend(&image_ext("CUT")[..400]); // cut before its END card
        let mut reader = stream(cut_ext);
        assert!(reader.next_hdu().unwrap().is_some());
        assert!(matches!(reader.next_hdu(), Err(Error::TruncatedHeader { hdu: 1 })));
    }
}
