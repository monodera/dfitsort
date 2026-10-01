//! FITS header reading for dfitsort.

pub mod card;
pub mod compressed;
pub mod error;
pub mod hdu;
pub mod header;
pub mod numeric;
pub mod query;
pub mod size;
pub mod source;
#[doc(hidden)]
pub mod testkit;
pub mod value;

pub use error::{Error, Result};
pub use source::Source;

/// Size of a FITS block in bytes.
pub const BLOCK_LEN: usize = 2880;
/// Size of a header card (keyword record) in bytes.
pub const CARD_LEN: usize = 80;
