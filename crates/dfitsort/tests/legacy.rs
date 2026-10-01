//! Legacy behaviour that differs from (improves on) the C tools, plus invocation paths.
#![cfg(unix)]

mod common;

use common::{dfitsort, fixtures, legacy_bin, stdout};
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
