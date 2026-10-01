//! Paths: raw bytes for output (paths need not be UTF-8) and stdin handling.

use std::borrow::Cow;
use std::ffi::OsStr;
use std::io;
use std::path::Path;

use dfitsort_core::Source;

pub fn is_stdin(path: &Path) -> bool {
    path.as_os_str() == "-"
}

/// Opens a file, or stdin for `-`.
pub fn open(path: &Path) -> io::Result<Source> {
    if is_stdin(path) { Source::from_reader(io::stdin()) } else { Source::open(path) }
}

#[cfg(unix)]
pub fn os_bytes(s: &OsStr) -> Cow<'_, [u8]> {
    use std::os::unix::ffi::OsStrExt;
    Cow::Borrowed(s.as_bytes())
}

#[cfg(not(unix))]
pub fn os_bytes(s: &OsStr) -> Cow<'_, [u8]> {
    match s.to_string_lossy() {
        Cow::Borrowed(t) => Cow::Borrowed(t.as_bytes()),
        Cow::Owned(t) => Cow::Owned(t.into_bytes()),
    }
}
