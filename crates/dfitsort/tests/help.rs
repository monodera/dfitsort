//! `-h` gives the option summary; `--help` adds the details and examples.

mod common;

use common::{run, stdout};

#[test]
fn long_help_adds_details_to_the_summary() {
    for (args, details) in [
        (vec![], vec!["Legacy mode:", "right after `-x N` (dfits) or `-d` (fitsort), or as the last", "Exit status:"]),
        (vec!["dump"], vec!["Examples:", "LESS=FRX", "Exit status:"]),
        (
            vec!["table"],
            vec!["Keyword names:", "HIERARCH ESO DPR CATG", "substring", "json output uses null", "Exit status:"],
        ),
    ] {
        let short = run(&[args.as_slice(), &["-h"]].concat());
        let long = run(&[args.as_slice(), &["--help"]].concat());
        assert_eq!(short.status.code(), Some(0), "{args:?}");
        assert_eq!(long.status.code(), Some(0), "{args:?}");
        for text in details {
            assert!(!stdout(&short).contains(text), "{args:?} -h: {text}");
            assert!(stdout(&long).contains(text), "{args:?} --help: {text}");
        }
    }
}
