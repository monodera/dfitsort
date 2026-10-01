#![allow(dead_code)]

use std::path::PathBuf;
use std::process::{Command, Output};

pub fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// `dfitsort` with the fixtures directory as working directory.
pub fn dfitsort() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_dfitsort"));
    cmd.current_dir(fixtures());
    cmd
}

pub fn run(args: &[&str]) -> Output {
    dfitsort().args(args).output().unwrap()
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A temporary directory holding `dfits` and `fitsort` symlinks to the binary.
#[cfg(unix)]
pub fn legacy_bin() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in ["dfits", "fitsort"] {
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_dfitsort"), dir.path().join(name)).unwrap();
    }
    dir
}
