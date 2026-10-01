//! Helpers for building FITS byte streams in tests. Not part of the stable API.

use crate::{BLOCK_LEN, CARD_LEN};

/// One 80-byte card, blank-padded.
pub fn card(text: &str) -> Vec<u8> {
    assert!(text.len() <= CARD_LEN, "card longer than 80 bytes: {text}");
    let mut c = text.as_bytes().to_vec();
    c.resize(CARD_LEN, b' ');
    c
}

/// Header made of `cards` plus END, padded with blanks to a whole block.
pub fn header(cards: &[&str]) -> Vec<u8> {
    let mut h: Vec<u8> = cards.iter().flat_map(|c| card(c)).collect();
    h.extend(card("END"));
    pad(&mut h, b' ');
    h
}

/// `n` zero bytes padded to a whole block.
pub fn data(n: usize) -> Vec<u8> {
    let mut d = vec![0u8; n];
    pad(&mut d, 0);
    d
}

fn pad(v: &mut Vec<u8>, fill: u8) {
    let rem = v.len() % BLOCK_LEN;
    if rem != 0 {
        v.resize(v.len() + BLOCK_LEN - rem, fill);
    }
}
