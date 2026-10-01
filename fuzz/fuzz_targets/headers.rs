#![no_main]

use std::io::Cursor;

use dfitsort_core::Source;
use dfitsort_core::compressed::{is_compressed_image, logical_header};
use dfitsort_core::hdu::HduReader;
use dfitsort_core::header::Header;
use dfitsort_core::legacy::{fitsort_line_keyword, fitsort_value};
use dfitsort_core::query::KeySpec;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = Source::from_reader(Cursor::new(data.to_vec())) else { return };
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
});
