//! Legacy behaviour that differs from (improves on) the C tools, plus invocation paths.
#![cfg(unix)]

mod common;

use common::{dfitsort, fixtures, legacy_bin, run, stdout};
use std::fs::File;
use std::process::{Command, Output};

fn legacy(bin: &tempfile::TempDir, tool: &str, args: &[&str]) -> Output {
    Command::new(bin.path().join(tool)).args(args).current_dir(fixtures()).output().unwrap()
}

#[test]
fn dfits_never_mistakes_data_for_an_extension() {
    let bin = legacy_bin();
    let text = stdout(&legacy(&bin, "dfits", &["-x", "0", "endkeys.fits"]));
    assert_eq!(text.matches("====> xtension").count(), 2);
    assert!(!text.contains("FAKE"));
    assert!(text.contains("====> xtension 2\nXTENSION= 'IMAGE   '"));
}

#[test]
fn dfits_prints_the_cards_of_a_header_cut_before_end() {
    let bin = legacy_bin();
    let out = legacy(&bin, "dfits", &["truncated.fits"]);
    assert_eq!(out.status.code(), Some(1));
    let text = stdout(&out);
    assert!(text.starts_with("====> file truncated.fits (main) <====\nSIMPLE  ="));
    assert!(text.ends_with("KEY32   =                   32\n"));
}

#[test]
fn dfits_reports_an_extension_header_cut_before_end() {
    let bin = legacy_bin();
    let cards = |cards: &[&str]| -> Vec<u8> { cards.iter().flat_map(|c| format!("{c:<80}").into_bytes()).collect() };
    let mut bytes = cards(&[
        "SIMPLE  =                    T",
        "BITPIX  =                    8",
        "NAXIS   =                    0",
        "END",
    ]);
    bytes.resize(2880, b' ');
    bytes.extend(cards(&[
        "XTENSION= 'IMAGE   '",
        "BITPIX  =                    8",
        "NAXIS   =                    0",
        "PCOUNT  =                    0",
        "GCOUNT  =                    1",
    ]));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cut.fits");
    std::fs::write(&path, bytes).unwrap();
    let path = path.to_str().unwrap();
    for args in [vec!["-x", "1", path], vec!["-x", "0", path]] {
        let out = legacy(&bin, "dfits", &args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        assert!(stdout(&out).contains("====> xtension 1\nXTENSION= 'IMAGE   '"), "{args:?}");
        assert!(stdout(&out).ends_with("GCOUNT  =                    1\n"), "{args:?}");
        assert!(!out.stderr.is_empty(), "{args:?}");
    }
}

#[test]
fn dfits_reads_gzip_files_and_streams() {
    let bin = legacy_bin();
    let plain = stdout(&legacy(&bin, "dfits", &["strings.fits"]));
    let gz = stdout(&legacy(&bin, "dfits", &["strings.fits.gz"]));
    assert_eq!(gz.replacen("strings.fits.gz", "strings.fits", 1), plain);
    let piped = Command::new(bin.path().join("dfits"))
        .arg("-")
        .stdin(File::open(fixtures().join("strings.fits.gz")).unwrap())
        .output()
        .unwrap();
    assert_eq!(stdout(&piped), plain.split_once('\n').unwrap().1);
}

#[test]
fn legacy_subcommand_matches_the_symlinks() {
    let bin = legacy_bin();
    let via_link = legacy(&bin, "dfits", &["-x", "0", "mef.fits"]);
    let via_sub = dfitsort().args(["legacy", "dfits", "-x", "0", "mef.fits"]).output().unwrap();
    assert_eq!(via_link.stdout, via_sub.stdout);
    assert_eq!(dfitsort().args(["legacy", "nope"]).output().unwrap().status.code(), Some(2));
}

#[test]
fn legacy_fitsort_subcommand_in_a_pipe() {
    let piped = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "{} legacy dfits eso1.fits | {} legacy fitsort OBJECT",
            env!("CARGO_BIN_EXE_dfitsort"),
            env!("CARGO_BIN_EXE_dfitsort")
        ))
        .current_dir(fixtures())
        .output()
        .unwrap();
    assert_eq!(stdout(&piped), "FILE     \tOBJECT  \t\neso1.fits\tNGC 254 \t\n");
}

#[test]
fn fitsort_has_no_fixed_line_or_name_limits() {
    let bin = legacy_bin();
    let dir = tempfile::tempdir().unwrap();
    let long = dir.path().join(format!("{}.fits", "n".repeat(150)));
    std::fs::copy(fixtures().join("eso1.fits"), &long).unwrap();
    let script = format!("dfits '{}' | fitsort OBJECT", long.display());
    let out = Command::new("bash")
        .arg("-c")
        .arg(script)
        .env("PATH", format!("{}:{}", bin.path().display(), std::env::var("PATH").unwrap()))
        .output()
        .unwrap();
    let text = stdout(&out);
    assert!(text.lines().nth(1).unwrap().starts_with(&format!("{}\t", long.display())), "{text}");
}

#[test]
fn legacy_help_alone_prints_the_usage_and_exits_0() {
    let bin = legacy_bin();
    for (tool, usage) in [("dfits", "dfits [-x xtnum] <list of FITS files>"), ("fitsort", "fitsort [-d] KEY1")] {
        let bare = legacy(&bin, tool, &[]);
        assert_eq!(bare.status.code(), Some(if tool == "dfits" { 1 } else { 0 }), "{tool}");
        assert!(!stdout(&bare).contains("-p "), "{tool}: the C usage text stays as it is");
        for flag in ["-h", "--help"] {
            let out = legacy(&bin, tool, &[flag]);
            assert_eq!(out.status.code(), Some(0), "{tool} {flag}");
            let text = stdout(&out);
            assert!(text.contains(usage), "{tool} {flag}: {text}");
            assert!(text.contains("-p "), "{tool} {flag}: {text}");
            assert!(out.stderr.is_empty(), "{tool} {flag}");
        }
    }
    let sub = run(&["legacy", "dfits", "-h"]);
    assert_eq!(sub.status.code(), Some(0));
    assert!(stdout(&sub).contains("usage: dfits [-x xtnum]"));
}

#[test]
fn legacy_help_among_other_arguments_keeps_its_c_meaning() {
    let bin = legacy_bin();
    let out = legacy(&bin, "dfits", &["-h", "eso1.fits"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("cannot open file [-h]"));
}
