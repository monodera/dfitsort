//! `-p`: paging through `$PAGER` when stdout is a terminal (util-linux `script` provides one).
#![cfg(target_os = "linux")]

mod common;

use common::{dfitsort, fixtures, legacy_bin, run, stderr, stdout};
use std::path::Path;
use std::process::{Command, Stdio};

/// Runs `cmd` with sh on a pseudo-terminal in the fixtures directory; returns the
/// exit status and everything written to the terminal (stdout and stderr, `\r\n` as `\n`).
fn on_terminal(cmd: &str, pager: &str) -> (Option<i32>, String) {
    let out = Command::new("script")
        .args(["-qec", cmd, "/dev/null"])
        .current_dir(fixtures())
        .env("PAGER", pager)
        .env_remove("LESS")
        .stdin(Stdio::null())
        .output()
        .expect("util-linux script");
    (out.status.code(), stdout(&out).replace("\r\n", "\n"))
}

fn quote(path: &Path) -> String {
    format!("'{}'", path.display())
}

fn bin() -> String {
    format!("'{}'", env!("CARGO_BIN_EXE_dfitsort"))
}

/// A pager that saves its input to `paged.txt` in `dir`.
fn saving_pager(dir: &Path) -> String {
    format!("cat > {}", quote(&dir.join("paged.txt")))
}

fn paged(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("paged.txt")).unwrap()
}

#[test]
fn without_a_terminal_the_option_is_ignored() {
    for (plain, paging) in [
        (vec!["dump", "-x", "0", "mef.fits"], vec!["dump", "-p", "-x", "0", "mef.fits"]),
        (
            vec!["table", "-k", "OBJECT", "eso1.fits", "eso2.fits"],
            vec!["table", "--pager", "-k", "OBJECT", "eso1.fits", "eso2.fits"],
        ),
    ] {
        let expected = run(&plain);
        let out = dfitsort().args(&paging).env("PAGER", "echo PAGED").output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{paging:?}");
        assert_eq!(out.stdout, expected.stdout, "{paging:?}");
    }
}

#[test]
fn dump_and_table_go_through_the_pager() {
    for args in [
        vec!["dump", "-x", "0", "mef.fits", "eso1.fits"],
        vec!["table", "-k", "OBJECT,EXPTIME", "eso1.fits", "eso2.fits"],
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (code, screen) =
            on_terminal(&format!("{} {} -p {}", bin(), args[0], args[1..].join(" ")), &saving_pager(dir.path()));
        assert_eq!(code, Some(0), "{args:?}");
        assert_eq!(screen, "", "{args:?}");
        assert_eq!(paged(dir.path()), stdout(&run(&args)), "{args:?}");
    }
}

#[test]
fn less_options_default_to_frx_unless_set() {
    let dir = tempfile::tempdir().unwrap();
    let pager = format!("echo \"LESS=$LESS\"; {}", saving_pager(dir.path()));
    let (_, screen) = on_terminal(&format!("{} dump -p eso1.fits", bin()), &pager);
    assert_eq!(screen, "LESS=FRX\n");
    let (_, screen) = on_terminal(&format!("LESS=S {} dump -p eso1.fits", bin()), &pager);
    assert_eq!(screen, "LESS=S\n");
}

#[test]
fn file_errors_appear_after_the_pager_exits() {
    let dir = tempfile::tempdir().unwrap();
    let pager = format!("{}; echo PAGER_DONE", saving_pager(dir.path()));
    let (code, screen) = on_terminal(&format!("{} dump -p eso1.fits notfits.txt eso2.fits", bin()), &pager);
    assert_eq!(code, Some(1));
    let done = screen.find("PAGER_DONE").expect("pager ran");
    let error = screen.find("dfitsort: notfits.txt:").expect("error shown");
    assert!(done < error, "{screen}");
    let text = paged(dir.path());
    assert!(text.contains("====> file eso1.fits") && text.contains("====> file eso2.fits"));
    assert!(!text.contains("not a FITS file"));
}

#[test]
fn quitting_the_pager_early_exits_quietly() {
    // Enough output to fill the pipe, so that writing fails after the pager has gone.
    let files = vec!["bigheader.fits"; 200].join(" ");
    let (code, screen) = on_terminal(&format!("{} dump -p {files}", bin()), "head -c 10 >/dev/null");
    assert_eq!(code, Some(0));
    assert_eq!(screen, "");
}

#[test]
fn an_empty_or_cat_pager_writes_directly() {
    for pager in ["", "cat"] {
        let (code, screen) = on_terminal(&format!("{} dump -p notfits.txt eso1.fits", bin()), pager);
        assert_eq!(code, Some(1), "PAGER={pager:?}");
        // Unpaged, the error comes before the next file, as without -p.
        let error = screen.find("dfitsort: notfits.txt:").expect("error shown");
        let next = screen.find("====> file eso1.fits").expect("eso1.fits shown");
        assert!(error < next, "PAGER={pager:?}: {screen}");
    }
}

#[test]
fn a_missing_pager_command_falls_back_to_stdout() {
    let expected = stdout(&run(&["dump", "eso1.fits"]));
    for cmd in [
        format!("{} dump -p eso1.fits", bin()),
        // PAGER unset and no less on PATH.
        format!("env -u PAGER PATH=/nonexistent {} dump -p eso1.fits", bin()),
    ] {
        let (code, screen) = on_terminal(&cmd, "no-such-pager");
        assert_eq!(code, Some(0), "{cmd}");
        assert!(screen.starts_with("dfitsort: cannot run pager "), "{cmd}: {screen}");
        assert!(screen.ends_with(&expected), "{cmd}: {screen}");
    }
}

#[test]
fn a_shell_pager_that_cannot_run_is_reported() {
    let (code, screen) = on_terminal(&format!("{} dump -p eso1.fits", bin()), "no-such-pager -S");
    assert_eq!(code, Some(1));
    assert!(screen.contains("dfitsort: cannot run pager no-such-pager -S"), "{screen}");
}

#[test]
fn an_interrupt_while_paging_leaves_the_pager_in_charge() {
    let dir = tempfile::tempdir().unwrap();
    let pager = format!("kill -INT $PPID; sleep 0.2; {}", saving_pager(dir.path()));
    let (code, _) = on_terminal(&format!("{} dump -p -x 0 mef.fits", bin()), &pager);
    assert_eq!(code, Some(0));
    assert_eq!(paged(dir.path()), stdout(&run(&["dump", "-x", "0", "mef.fits"])));
}

#[test]
fn legacy_tools_page_with_a_leading_p() {
    let legacy = legacy_bin();
    let dfits = quote(&legacy.path().join("dfits"));
    let fitsort = quote(&legacy.path().join("fitsort"));
    let plain = |cmd: &str| -> Vec<u8> {
        Command::new("sh").args(["-c", cmd]).current_dir(fixtures()).output().unwrap().stdout
    };
    for (paging, unpaged) in [
        (format!("{dfits} -p -x 0 mef.fits"), format!("{dfits} -x 0 mef.fits")),
        (
            format!("{dfits} eso1.fits eso2.fits | {fitsort} -p -d OBJECT"),
            format!("{dfits} eso1.fits eso2.fits | {fitsort} -d OBJECT"),
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (code, screen) = on_terminal(&paging, &saving_pager(dir.path()));
        assert_eq!(code, Some(0), "{paging}");
        assert_eq!(screen, "", "{paging}");
        assert_eq!(paged(dir.path()).into_bytes(), plain(&unpaged), "{paging}");
    }
}

#[test]
fn legacy_p_counts_only_as_the_first_argument() {
    let legacy = legacy_bin();
    let out = Command::new(legacy.path().join("dfits"))
        .args(["-x", "0", "-p", "eso1.fits"])
        .current_dir(fixtures())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("cannot open file [-p]"));
    let out =
        Command::new(legacy.path().join("dfits")).args(["-p", "eso1.fits"]).current_dir(fixtures()).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(stdout(&out).starts_with("====> file eso1.fits (main) <===="));
}
