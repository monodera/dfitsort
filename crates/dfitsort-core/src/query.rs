//! User keyword specs and their resolution against a header (spec §6.4).

use crate::card::normalize_name;
use crate::header::Header;
use crate::value::Value;

/// A keyword as the user typed it, with the normalised names to try in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySpec {
    label: String,
    candidates: Vec<String>,
}

impl KeySpec {
    /// `ns` is the HIERARCH namespace for dot specs (`"ESO"`, or `""` for none).
    pub fn new(spec: &str, ns: &str) -> KeySpec {
        let mut norm = normalize_name(spec);
        // `HIERARCH.ESO.DET.DIT` spells out the whole name, like `HIERARCH ESO DET DIT`: no namespace added.
        let mut explicit = false;
        if let Some(rest) = norm.strip_prefix("HIERARCH ") {
            norm = rest.to_string();
        } else if let Some(rest) = norm.strip_prefix("HIERARCH.").filter(|r| !r.is_empty()) {
            norm = rest.to_string();
            explicit = true;
        }
        let mut candidates = Vec::new();
        if !norm.contains(' ') && norm.contains('.') {
            let joined = norm.split('.').filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
            if !joined.is_empty() {
                let ns = normalize_name(ns);
                if !explicit && !ns.is_empty() {
                    candidates.push(format!("{ns} {joined}"));
                }
                candidates.push(joined);
            }
        }
        if !candidates.contains(&norm) {
            candidates.push(norm);
        }
        KeySpec { label: spec.to_string(), candidates }
    }

    /// The spec exactly as given (used as column heading).
    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn candidates(&self) -> &[String] {
        &self.candidates
    }

    /// Position (for `Header::value_at`) of the first candidate present in `header`.
    pub fn resolve(&self, header: &Header) -> Option<usize> {
        self.candidates.iter().find_map(|c| header.find(c))
    }

    pub fn value(&self, header: &Header) -> Option<Value> {
        self.resolve(header).map(|pos| header.value_at(pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::header;

    #[test]
    fn candidates() {
        assert_eq!(KeySpec::new("dpr.catg", "ESO").candidates(), ["ESO DPR CATG", "DPR CATG", "DPR.CATG"]);
        assert_eq!(
            KeySpec::new("ASTRO.METADATA.FIX.DATE", "").candidates(),
            ["ASTRO METADATA FIX DATE", "ASTRO.METADATA.FIX.DATE"]
        );
        assert_eq!(KeySpec::new("hierarch eso  det dit", "ESO").candidates(), ["ESO DET DIT"]);
        assert_eq!(KeySpec::new("DATA-TYP", "ESO").candidates(), ["DATA-TYP"]);
        assert_eq!(KeySpec::new("DET.DIT", "tng").candidates()[0], "TNG DET DIT");
        assert_eq!(KeySpec::new("dpr.catg", "ESO").label(), "dpr.catg");
    }

    #[test]
    fn hierarch_dot_prefix_names_the_whole_keyword() {
        // Like "HIERARCH ESO PRO CATG": the namespace is already spelled out, so --ns is not added.
        assert_eq!(KeySpec::new("HIERARCH.ESO.PRO.CATG", "ESO").candidates(), ["ESO PRO CATG", "ESO.PRO.CATG"]);
        assert_eq!(KeySpec::new("hierarch.tng.drs.bjd", "ESO").candidates(), ["TNG DRS BJD", "TNG.DRS.BJD"]);
        assert_eq!(KeySpec::new("HIERARCH.EXPTIME", "ESO").candidates(), ["EXPTIME"]);
        // Nothing after the prefix: kept as a plain dot spec.
        assert_eq!(KeySpec::new("HIERARCH.", "ESO").candidates(), ["ESO HIERARCH", "HIERARCH", "HIERARCH."]);
    }

    #[test]
    fn resolution_order() {
        let hd = Header::parse(header(&[
            "HIERARCH ASTRO METADATA FIX DATE = '2026-01-01'",
            "HIERARCH scaling.fiberPitch = 1.5",
            "HIERARCH ESO DPR CATG = 'SCIENCE'",
            "HIERARCH TNG DRS BJD = 2459000.5",
        ]));
        let v = |spec: &str, ns: &str| KeySpec::new(spec, ns).value(&hd);
        assert_eq!(v("DPR.CATG", "ESO"), Some(Value::Str(b"SCIENCE".to_vec())));
        assert_eq!(v("ASTRO.METADATA.FIX.DATE", "ESO"), Some(Value::Str(b"2026-01-01".to_vec())));
        assert_eq!(v("scaling.fiberPitch", "ESO"), Some(Value::Real("1.5".into())));
        assert_eq!(v("DRS.BJD", "TNG"), Some(Value::Real("2459000.5".into())));
        assert_eq!(v("HIERARCH TNG DRS BJD", "ESO"), Some(Value::Real("2459000.5".into())));
        assert_eq!(v("HIERARCH.ESO.DPR.CATG", "ESO"), Some(Value::Str(b"SCIENCE".to_vec())));
        assert_eq!(v("HIERARCH.ASTRO.METADATA.FIX.DATE", "ESO"), Some(Value::Str(b"2026-01-01".to_vec())));
        assert_eq!(v("NOPE.KEY", "ESO"), None);
    }
}
