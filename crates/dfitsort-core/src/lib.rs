//! FITS header reading for dfitsort.
//!
//! This is an internal crate of the `dfitsort` command-line tool, published on
//! crates.io only so that `cargo install dfitsort` can build. It is not meant
//! as a general-purpose FITS library, and its API may change in any release.

pub mod card;
pub mod compressed;
pub mod error;
pub mod filter;
pub mod hdu;
pub mod header;
pub mod legacy;
pub mod numeric;
pub mod query;
pub mod select;
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
