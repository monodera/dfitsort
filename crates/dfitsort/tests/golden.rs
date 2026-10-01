//! Legacy personalities against outputs recorded from the original ESO C tools.
#![cfg(unix)]

mod common;

use std::path::PathBuf;
use std::process::Command;

#[test]
fn legacy_output_matches_the_original_c_tools() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bin = common::legacy_bin();
    let out = tempfile::tempdir().unwrap();
    let status = Command::new("bash")
        .arg(root.join("scripts/run_golden_cases.sh"))
        .arg(bin.path())
        .arg(out.path())
        .status()
        .unwrap();
    assert!(status.success());
    let mut checked = 0;
    for entry in std::fs::read_dir(root.join("tests/golden")).unwrap() {
        let path = entry.unwrap().path();
        if !matches!(path.extension().and_then(|e| e.to_str()), Some("stdout" | "rc")) {
            continue;
        }
        let name = path.file_name().unwrap();
        let expected = std::fs::read(&path).unwrap();
        let actual = std::fs::read(out.path().join(name)).unwrap_or_default();
        assert!(
            expected == actual,
            "{} differs\n--- expected (C)\n{}\n--- actual\n{}",
            name.to_string_lossy(),
            String::from_utf8_lossy(&expected),
            String::from_utf8_lossy(&actual)
        );
        checked += 1;
    }
    assert!(checked >= 2, "no golden files found");
}
