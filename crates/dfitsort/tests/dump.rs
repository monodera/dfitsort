mod common;

use common::{dfitsort, fixtures, run, stderr, stdout};
use std::fs::File;

#[test]
fn primary_header_by_default() {
    let out = run(&["dump", "eso1.fits"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.starts_with("====> file eso1.fits (main) <====\nSIMPLE  =                    T"));
    assert!(text.contains("HIERARCH ESO DPR CATG = 'CALIB"));
    assert!(text.ends_with("END\n"));
    assert!(!text.contains("xtension"));
}

#[test]
fn hdu_selection() {
    let all = stdout(&run(&["dump", "-x", "0", "mef.fits"]));
    assert_eq!(all.matches("====> xtension").count(), 3);
    let chip2 = stdout(&run(&["dump", "-x", "chip2", "mef.fits"]));
    assert!(chip2.contains("====> xtension 2\n") && !chip2.contains("xtension 1") && !chip2.contains("SIMPLE"));
    let none = run(&["dump", "-x", "9", "mef.fits"]);
    assert!(none.status.success());
    assert_eq!(stdout(&none), "====> file mef.fits (main) <====\n");
    for bad in ["3-1", "1-", "1-x"] {
        assert_eq!(run(&["dump", "-x", bad, "mef.fits"]).status.code(), Some(2), "-x {bad}");
    }
}

#[test]
fn extnames_may_contain_dashes() {
    let out = run(&["dump", "-x", "CHIP-1", "mef.fits"]);
    assert!(out.status.success());
    assert_eq!(stdout(&out), "====> file mef.fits (main) <====\n");
}

#[test]
fn compressed_images_show_the_image_header() {
    let logical = stdout(&run(&["dump", "-x", "1", "compressed.fits.fz"]));
    assert!(logical.contains("XTENSION= 'IMAGE   '"));
    assert!(logical.contains("NAXIS1  =                   64"));
    assert!(logical.contains("EXTNAME = 'SCI"));
    assert!(!logical.contains("ZBITPIX") && !logical.contains("TFORM1"));
    let stored = stdout(&run(&["dump", "--compressed", "-x", "1", "compressed.fits.fz"]));
    assert!(stored.contains("XTENSION= 'BINTABLE'") && stored.contains("ZBITPIX"));
}

#[test]
fn gzip_and_stdin() {
    let plain = stdout(&run(&["dump", "strings.fits"]));
    let gz = stdout(&run(&["dump", "strings.fits.gz"]));
    assert_eq!(gz.replacen("strings.fits.gz", "strings.fits", 1), plain);
    let piped =
        dfitsort().args(["dump", "-"]).stdin(File::open(fixtures().join("strings.fits.gz")).unwrap()).output().unwrap();
    assert!(piped.status.success());
    assert_eq!(stdout(&piped), plain.split_once('\n').unwrap().1);
}

#[test]
fn awkward_but_valid_files() {
    let endkeys = stdout(&run(&["dump", "-x", "0", "endkeys.fits"]));
    assert_eq!(endkeys.matches("====> xtension").count(), 2);
    assert!(!endkeys.contains("FAKE") && endkeys.contains("EXTNAME = 'LAST"));
    for file in ["trailing.fits", "groups.fits", "bigheader.fits", "heap.fits"] {
        let out = run(&["dump", "-x", "0", file]);
        assert!(out.status.success() && out.stderr.is_empty(), "{file}: {}", stderr(&out));
    }
    assert!(stdout(&run(&["dump", "-x", "0", "groups.fits"])).contains("EXTNAME = 'AFTERGROUPS'"));
    assert!(stdout(&run(&["dump", "-x", "0", "bigheader.fits"])).contains("EXTNAME = 'AFTERBIG'"));
    assert!(stdout(&run(&["dump", "-x", "0", "heap.fits"])).contains("MARKER  = 'found-after-heap'"));
}

#[test]
fn bad_files_are_reported_and_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let dir_arg = dir.path().to_str().unwrap();
    let out = run(&[
        "dump",
        "eso1.fits",
        "notfits.txt",
        "missing.fits",
        "empty.fits",
        "truncated.fits",
        dir_arg,
        "eso2.fits",
    ]);
    assert_eq!(out.status.code(), Some(1));
    let text = stdout(&out);
    assert!(text.contains("====> file eso1.fits (main) <====\nSIMPLE"));
    assert!(text.contains("====> file eso2.fits (main) <====\nSIMPLE"));
    let errors = stderr(&out);
    // The header cut before END still prints the cards that were read.
    assert!(text.contains("====> file truncated.fits (main) <====\nSIMPLE  =                    T"));
    assert!(text.contains("KEY32   =                   32\n====> file eso2.fits"));
    for name in ["notfits.txt", "missing.fits", "empty.fits", "truncated.fits", dir_arg] {
        assert!(errors.contains(&format!("dfitsort: {name}: ")), "no error for {name}: {errors}");
    }
}

/// Runs `dump` on `names` (copies of a fixture) and checks the echoed `====> file` lines.
#[cfg(unix)]
fn dump_copies(names: &[&[u8]]) {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<_> = names.iter().map(|n| dir.path().join(OsStr::from_bytes(n))).collect();
    for path in &paths {
        std::fs::copy(fixtures().join("eso1.fits"), path).unwrap();
    }
    let out = dfitsort().arg("dump").args(&paths).output().unwrap();
    assert!(out.status.success());
    for path in &paths {
        let mut line = b"====> file ".to_vec();
        line.extend_from_slice(path.as_os_str().as_bytes());
        assert!(out.stdout.windows(line.len()).any(|w| w == line.as_slice()));
    }
}

#[cfg(unix)]
#[test]
fn paths_with_spaces() {
    dump_copies(&[b"with space.fits"]);
}

/// Only Linux accepts arbitrary bytes in file names (APFS rejects non-UTF-8 names).
#[cfg(target_os = "linux")]
#[test]
fn paths_with_non_utf8_bytes() {
    dump_copies(&[b"with space.fits", b"caf\xe9.fits"]);
}

#[test]
fn closed_stdout_exits_quietly() {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = dfitsort()
        .arg("dump")
        .args(std::iter::repeat_n("bigheader.fits", 200))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut first = [0u8; 100];
    child.stdout.take().unwrap().read_exact(&mut first).unwrap(); // dropping stdout closes the pipe
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "status {:?}", out.status);
    assert!(out.stderr.is_empty(), "stderr: {}", stderr(&out));
}

/// A header of blank-padded cards plus END, padded to a whole block.
#[cfg(unix)]
fn fits_header(cards: &[&str]) -> Vec<u8> {
    let mut v: Vec<u8> = cards.iter().chain(&["END"]).flat_map(|c| format!("{c:<80}").into_bytes()).collect();
    v.resize(v.len().div_ceil(2880) * 2880, b' ');
    v
}

#[cfg(unix)]
#[test]
fn fifo_input_with_large_data_units() {
    let mut bytes = fits_header(&[
        "SIMPLE  =                    T",
        "BITPIX  =                    8",
        "NAXIS   =                    1",
        "NAXIS1  =               100000",
    ]);
    bytes.resize(bytes.len() + 100_000usize.div_ceil(2880) * 2880, 0);
    bytes.extend(fits_header(&[
        "XTENSION= 'IMAGE   '",
        "BITPIX  =                    8",
        "NAXIS   =                    0",
    ]));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pipe");
    assert!(std::process::Command::new("mkfifo").arg(&path).status().unwrap().success());
    let writer_path = path.clone();
    let writer = std::thread::spawn(move || std::fs::write(writer_path, bytes).unwrap());
    let out = dfitsort().args(["dump", "-x", "0"]).arg(&path).output().unwrap();
    writer.join().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("====> xtension 1\nXTENSION= 'IMAGE   '"));
}
