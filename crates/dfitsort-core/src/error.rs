//! Error type for header reading.

use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("not a FITS file")]
    NotFits,
    #[error("not a FITS file (empty or shorter than one card)")]
    TooShort,
    #[error("HDU {hdu}: header ends before the END card")]
    TruncatedHeader { hdu: usize },
    #[error("HDU {hdu}: invalid data size ({reason})")]
    BadSize { hdu: usize, reason: String },
}

pub type Result<T> = std::result::Result<T, Error>;
