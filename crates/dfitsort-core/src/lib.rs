//! FITS header reading for dfitsort.

pub mod card;
pub mod error;
pub mod source;
#[doc(hidden)]
pub mod testkit;

pub use error::{Error, Result};
pub use source::Source;

/// Size of a FITS block in bytes.
pub const BLOCK_LEN: usize = 2880;
/// Size of a header card (keyword record) in bytes.
pub const CARD_LEN: usize = 80;
