//! Byte sources: plain files (seekable), gzip-compressed files and streams.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use flate2::read::MultiGzDecoder;

const BUF_LEN: usize = 32 * 1024;
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// Where FITS bytes come from.
pub struct Source {
    inner: Inner,
}

enum Inner {
    File(BufReader<File>),
    Stream(Box<dyn Read + Send>),
}

impl Source {
    /// Opens a file, transparently decompressing gzip.
    pub fn open(path: &Path) -> io::Result<Source> {
        let mut file = BufReader::with_capacity(BUF_LEN, File::open(path)?);
        if file.fill_buf()?.starts_with(&GZIP_MAGIC) {
            return Ok(Source::stream(MultiGzDecoder::new(file)));
        }
        Ok(Source { inner: Inner::File(file) })
    }

    /// Wraps a non-seekable stream such as stdin, transparently decompressing gzip.
    pub fn from_reader<R: Read + Send + 'static>(reader: R) -> io::Result<Source> {
        let mut buffered = BufReader::with_capacity(BUF_LEN, reader);
        // A pipe may deliver fewer bytes than requested; the magic is checked on
        // whatever arrived first, which in practice always includes two bytes.
        if buffered.fill_buf()?.starts_with(&GZIP_MAGIC) {
            return Ok(Source::stream(MultiGzDecoder::new(buffered)));
        }
        Ok(Source::stream(buffered))
    }

    fn stream<R: Read + Send + 'static>(reader: R) -> Source {
        Source { inner: Inner::Stream(Box::new(reader)) }
    }

    /// Reads until `buf` is full or EOF; returns the number of bytes read.
    pub fn read_full(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut filled = 0;
        while filled < buf.len() {
            let n = match &mut self.inner {
                Inner::File(f) => f.read(&mut buf[filled..]),
                Inner::Stream(s) => s.read(&mut buf[filled..]),
            };
            match n {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(filled)
    }

    /// Skips `n` bytes. Skipping past EOF is not an error; the next read returns 0.
    pub fn skip(&mut self, n: u64) -> io::Result<()> {
        if n == 0 {
            return Ok(());
        }
        match &mut self.inner {
            Inner::File(f) => {
                let n = i64::try_from(n).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "skip too large"))?;
                f.seek_relative(n)
            }
            Inner::Stream(s) => {
                let mut limited = (&mut **s).take(n);
                io::copy(&mut limited, &mut io::sink()).map(|_| ())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Source;
    use std::io::{Cursor, Write};

    const DATA: &[u8] = b"0123456789abcdefghij";

    fn gz(bytes: &[u8]) -> Vec<u8> {
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(bytes).unwrap();
        enc.finish().unwrap()
    }

    fn temp_with(bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(bytes).unwrap();
        f
    }

    fn rest_after_skip(mut s: Source, skip: u64) -> Vec<u8> {
        s.skip(skip).unwrap();
        let mut buf = vec![0u8; 100];
        let n = s.read_full(&mut buf).unwrap();
        buf.truncate(n);
        buf
    }

    #[test]
    fn plain_file_read_and_skip() {
        let f = temp_with(DATA);
        assert_eq!(rest_after_skip(Source::open(f.path()).unwrap(), 5), b"56789abcdefghij");
    }

    #[test]
    fn gzip_file_is_decompressed() {
        let f = temp_with(&gz(DATA));
        assert_eq!(rest_after_skip(Source::open(f.path()).unwrap(), 10), b"abcdefghij");
    }

    #[test]
    fn streams_plain_and_gzip() {
        let s = Source::from_reader(Cursor::new(DATA.to_vec())).unwrap();
        assert_eq!(rest_after_skip(s, 0), DATA);
        let s = Source::from_reader(Cursor::new(gz(DATA))).unwrap();
        assert_eq!(rest_after_skip(s, 18), b"ij");
    }

    #[test]
    fn skip_past_eof_is_not_an_error() {
        let f = temp_with(DATA);
        assert_eq!(rest_after_skip(Source::open(f.path()).unwrap(), 1000), b"");
        let s = Source::from_reader(Cursor::new(DATA.to_vec())).unwrap();
        assert_eq!(rest_after_skip(s, 1000), b"");
    }

    #[test]
    fn directory_is_an_error() {
        let d = tempfile::tempdir().unwrap();
        assert!(Source::open(d.path()).is_err());
    }
}
