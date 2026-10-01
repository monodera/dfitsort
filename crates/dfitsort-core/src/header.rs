//! Parsed view of one header: keyword lookup and value extraction.

use std::ops::Range;

use crate::CARD_LEN;
use crate::card::{CardKind, classify, normalized_eq, push_normalized};
use crate::value::{Value, parse_value};

/// A valued keyword: where its raw name and value field are within `Header::raw`.
#[derive(Debug, Clone)]
struct Entry {
    name: Range<usize>,
    card: usize,
    value_start: usize,
}

/// A header's cards plus an index of its valued keywords. Names are normalised and
/// values parsed only when looked up, so building a header is cheap.
#[derive(Debug, Clone)]
pub struct Header {
    raw: Vec<u8>,
    entries: Vec<Entry>,
}

impl Header {
    /// Builds a header from raw cards; anything after the END card is dropped.
    pub fn parse(raw: Vec<u8>) -> Header {
        let mut entries = Vec::with_capacity(raw.len() / CARD_LEN);
        let mut ncards = 0;
        for (i, card) in raw.chunks_exact(CARD_LEN).enumerate() {
            ncards = i + 1;
            match classify(card) {
                CardKind::End => break,
                CardKind::Keyword { raw_name, value_start, .. } => {
                    let start = i * CARD_LEN + (raw_name.as_ptr() as usize - card.as_ptr() as usize);
                    entries.push(Entry { name: start..start + raw_name.len(), card: i, value_start });
                }
                CardKind::Continue | CardKind::Commentary => {}
            }
        }
        let mut raw = raw;
        raw.truncate(ncards * CARD_LEN);
        Header { raw, entries }
    }

    /// The cards (up to and including END) as stored.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    pub fn cards(&self) -> std::slice::ChunksExact<'_, u8> {
        self.raw.chunks_exact(CARD_LEN)
    }

    /// Normalised names of all valued keywords in card order (duplicates included).
    pub fn names(&self) -> impl Iterator<Item = String> + '_ {
        self.entries.iter().map(|e| {
            let mut name = String::new();
            push_normalized(&self.raw[e.name.clone()], &mut name);
            name
        })
    }

    /// Position of the first keyword whose normalised name is `name` (itself normalised).
    pub fn find(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|e| normalized_eq(&self.raw[e.name.clone()], name))
    }

    /// Value of the keyword at `pos` (as returned by `find`), joining CONTINUE
    /// cards as described in Standard §4.2.1.2.
    pub fn value_at(&self, pos: usize) -> Value {
        let entry = &self.entries[pos];
        let mut value = parse_value(&self.card(entry.card)[entry.value_start..]);
        if let Value::Str(text) = &mut value {
            let mut next = entry.card + 1;
            while text.last() == Some(&b'&') && next < self.raw.len() / CARD_LEN {
                let card = self.card(next);
                if classify(card) != CardKind::Continue {
                    break;
                }
                let Value::Str(more) = parse_value(&card[10..]) else { break };
                text.pop();
                text.extend_from_slice(&more);
                next += 1;
            }
        }
        value
    }

    /// Value of the first keyword whose normalised name is `name`.
    pub fn get(&self, name: &str) -> Option<Value> {
        self.find(name).map(|pos| self.value_at(pos))
    }

    fn card(&self, i: usize) -> &[u8] {
        &self.raw[i * CARD_LEN..(i + 1) * CARD_LEN]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::header;

    fn h(cards: &[&str]) -> Header {
        Header::parse(header(cards))
    }

    fn s(text: &[u8]) -> Option<Value> {
        Some(Value::Str(text.to_vec()))
    }

    #[test]
    fn first_occurrence_wins() {
        assert_eq!(h(&["OBJECT  = 'first'", "OBJECT  = 'second'"]).get("OBJECT"), s(b"first"));
    }

    #[test]
    fn continue_strings_are_joined() {
        let hd = h(&[
            "LONGSTR = 'abc &'",
            "CONTINUE  'def&'",
            "CONTINUE  'ghi' / last",
            "OTHER   = 'x&'",
            "COMMENT not a continuation",
            "CONTINUE  'orphan'",
            "LIT     = 'a&'",
        ]);
        assert_eq!(hd.get("LONGSTR"), s(b"abc defghi"));
        assert_eq!(hd.get("OTHER"), s(b"x&"));
        assert_eq!(hd.get("LIT"), s(b"a&"));
    }

    #[test]
    fn hierarch_names_are_normalised() {
        let hd = h(&["HIERARCH ESO  det DIT = 10.0", "HIERARCH ESO OBS TARG NAME = 'very long &'", "CONTINUE  'name'"]);
        assert_eq!(hd.get("ESO DET DIT"), Some(Value::Real("10.0".into())));
        assert_eq!(hd.get("ESO OBS TARG NAME"), s(b"very long name"));
        assert_eq!(hd.names().collect::<Vec<_>>(), ["ESO DET DIT", "ESO OBS TARG NAME"]);
    }

    #[test]
    fn commentary_is_not_a_value_and_padding_is_dropped() {
        let hd = h(&["COMMENT = 'x'", "HISTORY y"]);
        assert_eq!(hd.get("COMMENT"), None);
        assert_eq!(hd.cards().count(), 3);
    }
}
