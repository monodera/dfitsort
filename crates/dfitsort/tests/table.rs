mod common;

use common::{dfitsort, run, stderr, stdout};

const ESO: [&str; 3] = ["eso1.fits", "eso2.fits", "eso3.fits"];

fn table(args: &[&str]) -> String {
    let mut all = vec!["table"];
    all.extend_from_slice(args);
    let out = run(&all);
    assert!(out.status.success(), "{}", stderr(&out));
    stdout(&out)
}

fn with_eso<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let mut v = args.to_vec();
    v.extend(ESO);
    v
}

fn json(args: &[&str]) -> serde_json::Value {
    let mut all = vec!["-f", "json"];
    all.extend_from_slice(args);
    serde_json::from_str(&table(&all)).unwrap()
}

#[test]
fn aligned_text_table() {
    assert_eq!(
        table(&with_eso(&["-k", "DPR.CATG,EXPTIME", "-k", "INS.FILT1.NAME"])),
        "FILE       DPR.CATG  EXPTIME  INS.FILT1.NAME\n\
         eso1.fits  CALIB     10.0     H\n\
         eso2.fits  SCIENCE   20.0     \n\
         eso3.fits  SCIENCE   30.0     Ks\n"
    );
    assert_eq!(
        table(&with_eso(&["-d", "-k", "OBJECT"])),
        "eso1.fits  NGC 254\neso2.fits  NGC 255\neso3.fits  NGC 256\n"
    );
}

#[test]
fn tsv_csv_and_missing_placeholder() {
    assert_eq!(
        table(&with_eso(&["-f", "tsv", "--missing", "-", "-k", "DPR.CATG,INS.FILT1.NAME"])),
        "FILE\tDPR.CATG\tINS.FILT1.NAME\neso1.fits\tCALIB\tH\neso2.fits\tSCIENCE\t-\neso3.fits\tSCIENCE\tKs\n"
    );
    assert_eq!(
        table(&["-f", "csv", "-k", "CPLX,QUOTE", "endkeys.fits"]),
        "FILE,CPLX,QUOTE\nendkeys.fits,\"(1.0, -2.5)\",O'HARA\n"
    );
    assert_eq!(table(&["-f", "csv", "-x", "9", "-k", "A", "mef.fits"]), "FILE,A\n");
}

#[test]
fn json_values_are_typed() {
    let keys = "W_PFDSGN,HUGE,DEXP,CPLX,UNDEF,QUOTE,OBJECT,LONGSTR,AMPLIT,EXTEND,NOPE";
    let v = json(&["-k", keys, "endkeys.fits"]);
    let row = &v[0];
    assert_eq!(row["file"], "endkeys.fits");
    assert_eq!(row["hdu"], 0);
    assert!(row["extname"].is_null());
    let values = &row["values"];
    assert_eq!(values["W_PFDSGN"].to_string(), "6659525521533387424");
    assert_eq!(values["DEXP"], 1500.0);
    assert!(values["HUGE"].is_null(), "non-finite numbers are not valid JSON");
    assert_eq!(values["CPLX"], serde_json::json!([1.0, -2.5]));
    assert!(values["UNDEF"].is_null() && values["NOPE"].is_null());
    assert_eq!(values["QUOTE"], "O'HARA");
    assert_eq!(values["OBJECT"], "first");
    assert_eq!(values["LONGSTR"], "This value is continued on a second card");
    assert_eq!(values["AMPLIT"], "literal &");
    assert_eq!(values["EXTEND"], true);
    assert_eq!(json(&["-x", "9", "-k", "A", "mef.fits"]), serde_json::json!([]));
}

#[test]
fn hierarch_matching_rules() {
    let keys = "DPR.TYPE,DET.DIT,DET.NDIT,ASTRO.METADATA.FIX.DATE,scaling.fiberPitch,pfs_detectorMap_class,HIERARCH TNG DRS BJD,ENDTIME,END-OBS";
    let values = &json(&["-k", keys, "endkeys.fits"])[0]["values"];
    assert_eq!(values["DPR.TYPE"], "OBJECT");
    assert_eq!(values["DET.DIT"], 10.0);
    assert_eq!(values["DET.NDIT"], 6);
    assert_eq!(values["ASTRO.METADATA.FIX.DATE"], "2026-01-01");
    assert_eq!(values["scaling.fiberPitch"], 1.5);
    assert_eq!(values["pfs_detectorMap_class"], "DistortedDetectorMap");
    assert_eq!(values["HIERARCH TNG DRS BJD"], 2459000.5);
    assert_eq!(values["ENDTIME"], "23:59:59");
    assert_eq!(values["END-OBS"], "2026-09-30");
    assert_eq!(json(&["--ns", "TNG", "-k", "DRS.BJD", "endkeys.fits"])[0]["values"]["DRS.BJD"], 2459000.5);
    let env = dfitsort()
        .env("DFITSORT_NS", "TNG")
        .args(["table", "-f", "json", "-k", "DRS.BJD", "endkeys.fits"])
        .output()
        .unwrap();
    let env: serde_json::Value = serde_json::from_slice(&env.stdout).unwrap();
    assert_eq!(env[0]["values"]["DRS.BJD"], 2459000.5);
}

#[test]
fn filters() {
    let files = |out: String| out.lines().skip(1).map(|l| l.split(' ').next().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(files(table(&with_eso(&["-k", "OBJECT", "-w", "DPR.CATG=SCIENCE"]))), ["eso2.fits", "eso3.fits"]);
    assert_eq!(files(table(&with_eso(&["-k", "OBJECT", "-w", "EXPTIME>15", "-w", "EXPTIME<25"]))), ["eso2.fits"]);
    assert_eq!(
        files(table(&with_eso(&["-k", "OBJECT", "--or", "-w", "EXPTIME=10", "-w", "EXPTIME=30"]))),
        ["eso1.fits", "eso3.fits"]
    );
    assert_eq!(files(table(&with_eso(&["-k", "OBJECT", "-w", "OBJECT~254"]))), ["eso1.fits"]);
    assert_eq!(files(table(&with_eso(&["-k", "OBJECT", "-w", "INS.FILT1.NAME!=H"]))), ["eso3.fits"]);
    assert_eq!(run(&["table", "-k", "OBJECT", "-w", "EXPTIME", "eso1.fits"]).status.code(), Some(2));
}

#[test]
fn sorting() {
    let files = |out: String| out.lines().skip(1).map(|l| l.split(' ').next().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(
        files(table(&with_eso(&["-k", "OBJECT", "-s", "EXPTIME:desc"]))),
        ["eso3.fits", "eso2.fits", "eso1.fits"]
    );
    assert_eq!(
        files(table(&with_eso(&["-k", "OBJECT", "-s", "INS.FILT1.NAME"]))),
        ["eso1.fits", "eso3.fits", "eso2.fits"]
    );
    assert_eq!(
        files(table(&with_eso(&["-k", "OBJECT", "-s", "INS.FILT1.NAME:desc"]))),
        ["eso3.fits", "eso1.fits", "eso2.fits"]
    );
    assert_eq!(
        files(table(&with_eso(&["-k", "OBJECT", "-s", "DPR.CATG", "-s", "EXPTIME:desc"]))),
        ["eso1.fits", "eso3.fits", "eso2.fits"]
    );
}

#[test]
fn extensions_and_compressed_images() {
    assert_eq!(
        table(&["-x", "0", "-k", "EXTNAME,DET.CHIP.ID", "mef.fits"]),
        "FILE         EXTNAME  DET.CHIP.ID\nmef.fits              \nmef.fits[1]  CHIP1    CCD-1\nmef.fits[2]  CHIP2    CCD-2\nmef.fits[3]  CHIP3    CCD-3\n"
    );
    let v = json(&["-x", "1-2", "-k", "NAXIS1,BITPIX,OBJECT", "compressed.fits.fz"]);
    assert_eq!(v[0]["hdu"], 1);
    assert_eq!(v[0]["extname"], "SCI");
    assert_eq!(v[0]["values"], serde_json::json!({"NAXIS1": 64, "BITPIX": 16, "OBJECT": "NGC 253"}));
    assert_eq!(v[1]["values"]["BITPIX"], 32);
    assert_eq!(json(&["--compressed", "-x", "1", "-k", "BITPIX", "compressed.fits.fz"])[0]["values"]["BITPIX"], 8);
    let unnamed = json(&["-x", "COMPRESSED_IMAGE", "-k", "NAXIS1", "compressed.fits.fz"]);
    assert_eq!(unnamed.as_array().unwrap().len(), 1);
    assert_eq!(unnamed[0]["hdu"], 3);
    assert_eq!(unnamed[0]["values"]["NAXIS1"], 16);
}

#[test]
fn errors_still_print_the_other_rows() {
    let out = run(&["table", "-k", "OBJECT", "eso1.fits", "missing.fits", "eso2.fits"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stdout(&out), "FILE       OBJECT\neso1.fits  NGC 254\neso2.fits  NGC 255\n");
    assert!(stderr(&out).contains("dfitsort: missing.fits: "));
}

#[test]
fn empty_keyword_specs_are_usage_errors() {
    for keys in ["", ",", "OBJECT,,EXPTIME"] {
        let out = run(&["table", "-k", keys, "eso1.fits"]);
        assert_eq!(out.status.code(), Some(2), "-k {keys:?}");
        assert!(stderr(&out).contains("dfitsort: -k: empty keyword"), "{}", stderr(&out));
    }
}
