//! The reader must never panic: feed it thousands of mutated fixtures.

use std::io::Cursor;
use std::path::PathBuf;

use dfitsort_core::compressed::{is_compressed_image, logical_header};
use dfitsort_core::hdu::HduReader;
use dfitsort_core::header::Header;
use dfitsort_core::legacy::{fitsort_line_keyword, fitsort_value};
use dfitsort_core::query::KeySpec;
use dfitsort_core::{CARD_LEN, Source};

/// Deterministic xorshift64 generator, so failures are reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Everything the CLI does with a header, on arbitrary bytes.
fn exercise(bytes: Vec<u8>) {
    let Ok(source) = Source::from_reader(Cursor::new(bytes)) else { return };
    let mut reader = HduReader::new(source);
    let spec = KeySpec::new("DPR.CATG", "ESO");
    for _ in 0..64 {
        let Ok(Some(hdu)) = reader.next_hdu() else { break };
        let raw = if is_compressed_image(&hdu.raw) { logical_header(&hdu.raw) } else { hdu.raw };
        let header = Header::parse(raw);
        for pos in 0..header.names().count() {
            let _ = header.value_at(pos).display_bytes();
        }
        let _ = spec.value(&header);
        for card in header.cards() {
            let _ = fitsort_line_keyword(card);
            let _ = fitsort_value(card);
        }
    }
}

#[test]
fn mutated_fixtures_do_not_panic() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let interesting: &[&[u8]] = &[
        b"=",
        b"'",
        b"&",
        b"/",
        b"END     ",
        b"HIERARCH ",
        b"CONTINUE  ",
        b"NAXIS   = 999",
        b"NAXIS1  = -1",
        b"BITPIX  = 64",
        b"XTENSION",
        b"ZIMAGE  =                    T",
        b"\0",
        b"\xff",
    ];
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "py") {
            continue;
        }
        let original = std::fs::read(&path).unwrap();
        exercise(original.clone());
        for _ in 0..300 {
            let mut bytes = original.clone();
            for _ in 0..1 + rng.below(8) {
                if bytes.is_empty() {
                    break;
                }
                let at = rng.below(bytes.len());
                match rng.below(4) {
                    0 => bytes[at] = rng.next() as u8,
                    1 => {
                        let snippet = interesting[rng.below(interesting.len())];
                        let start = at - at % CARD_LEN + rng.below(CARD_LEN);
                        for (i, &b) in snippet.iter().enumerate() {
                            if let Some(slot) = bytes.get_mut(start + i) {
                                *slot = b;
                            }
                        }
                    }
                    2 => bytes.truncate(at),
                    _ => {
                        let n = rng.below(200);
                        bytes.splice(at..at, std::iter::repeat_n(b' ', n));
                    }
                }
            }
            exercise(bytes);
        }
    }
}
