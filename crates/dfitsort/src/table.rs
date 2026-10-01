//! `dfitsort table`: one row per selected HDU with the requested keyword values.

use std::cmp::Ordering;
use std::io;
use std::path::{Path, PathBuf};

use dfitsort_core::filter::Condition;
use dfitsort_core::header::Header;
use dfitsort_core::numeric::{cmp_num, parse_num};
use dfitsort_core::query::KeySpec;
use dfitsort_core::select::{HduSelector, read_selected};
use dfitsort_core::value::Value;

use crate::cli::TableArgs;
use crate::dump::parse_selector;
use crate::output;
use crate::paths::{open, os_bytes};
use crate::run;

/// One output row.
pub struct Row {
    /// File path bytes as given on the command line.
    pub path: Vec<u8>,
    pub hdu: usize,
    pub extname: Option<String>,
    /// One entry per `-k` spec; `None` when the keyword is missing.
    pub values: Vec<Option<Value>>,
    sort_values: Vec<Option<Value>>,
}

impl Row {
    /// First-column text: the path, plus `[N]` for extensions.
    pub fn label(&self) -> Vec<u8> {
        let mut label = self.path.clone();
        if self.hdu > 0 {
            label.extend_from_slice(format!("[{}]", self.hdu).as_bytes());
        }
        label
    }
}

struct SortKey {
    spec: KeySpec,
    desc: bool,
}

/// Everything needed to turn one file into rows.
struct Query {
    selector: HduSelector,
    logical: bool,
    keys: Vec<KeySpec>,
    conditions: Vec<Condition>,
    any: bool,
    sort: Vec<SortKey>,
}

pub fn run(args: TableArgs) -> i32 {
    let selector = match parse_selector(args.select.as_deref()) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let conditions = match args.conditions.iter().map(|c| Condition::parse(c, &args.ns)).collect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("dfitsort: -w: {e}");
            return 2;
        }
    };
    let query = Query {
        selector,
        logical: !args.compressed,
        keys: args.keys.iter().map(|k| KeySpec::new(k.trim(), &args.ns)).collect(),
        conditions,
        any: args.or,
        sort: args.sort.iter().map(|s| parse_sort(s, &args.ns)).collect(),
    };
    run::init_threads(args.jobs);
    let labels: Vec<&str> = query.keys.iter().map(KeySpec::label).collect();
    let stdout = io::stdout();
    let mut writer = output::writer(
        args.format,
        io::BufWriter::with_capacity(1 << 16, stdout.lock()),
        &labels,
        !args.no_header,
        &args.missing,
    );
    let mut failed = false;
    let mut held: Vec<Row> = Vec::new();
    let mut emit = || -> io::Result<()> {
        run::ordered(
            &args.files,
            run::SMALL_RESULTS,
            |path: &PathBuf| rows_for(path, &query),
            |path, (rows, error)| {
                if let Some(msg) = error {
                    failed = true;
                    eprintln!("dfitsort: {}: {msg}", path.display());
                }
                if query.sort.is_empty() {
                    rows.iter().try_for_each(|r| writer.row(r))
                } else {
                    held.extend(rows);
                    Ok(())
                }
            },
        )?;
        sort_rows(&mut held, &query.sort);
        held.iter().try_for_each(|r| writer.row(r))?;
        writer.finish()
    };
    let result = emit();
    run::finish(result, i32::from(failed))
}

fn rows_for(path: &Path, q: &Query) -> (Vec<Row>, Option<String>) {
    let source = match open(path) {
        Ok(s) => s,
        Err(e) => return (Vec::new(), Some(e.to_string())),
    };
    let file = read_selected(source, &q.selector, q.logical);
    let rows = file
        .hdus
        .iter()
        .filter(|h| passes(&q.conditions, q.any, &h.header))
        .map(|h| Row {
            path: os_bytes(path.as_os_str()).into_owned(),
            hdu: h.index,
            extname: match h.header.get("EXTNAME") {
                Some(Value::Str(s)) => Some(String::from_utf8_lossy(&s).into_owned()),
                _ => None,
            },
            values: q.keys.iter().map(|k| k.value(&h.header)).collect(),
            sort_values: q.sort.iter().map(|k| k.spec.value(&h.header)).collect(),
        })
        .collect();
    (rows, file.error.map(|e| e.to_string()))
}

fn passes(conditions: &[Condition], any: bool, header: &Header) -> bool {
    if conditions.is_empty() {
        true
    } else if any {
        conditions.iter().any(|c| c.matches(header))
    } else {
        conditions.iter().all(|c| c.matches(header))
    }
}

fn parse_sort(text: &str, ns: &str) -> SortKey {
    match text.rsplit_once(':') {
        Some((key, dir)) if dir.eq_ignore_ascii_case("desc") => SortKey { spec: KeySpec::new(key, ns), desc: true },
        Some((key, dir)) if dir.eq_ignore_ascii_case("asc") => SortKey { spec: KeySpec::new(key, ns), desc: false },
        _ => SortKey { spec: KeySpec::new(text, ns), desc: false },
    }
}

/// Stable sort; numbers compare numerically, missing values always last.
fn sort_rows(rows: &mut [Row], keys: &[SortKey]) {
    rows.sort_by(|a, b| {
        for (i, key) in keys.iter().enumerate() {
            let ord = match (&a.sort_values[i], &b.sort_values[i]) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(x), Some(y)) => {
                    let o = compare_values(x, y);
                    if key.desc { o.reverse() } else { o }
                }
            };
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    });
}

fn compare_values(a: &Value, b: &Value) -> Ordering {
    let (ta, tb) = (a.display_bytes(), b.display_bytes());
    let num = |t: &[u8]| std::str::from_utf8(t).ok().and_then(parse_num);
    if let (Some(x), Some(y)) = (num(&ta), num(&tb)) {
        if let Some(o) = cmp_num(x, y) {
            return o;
        }
    }
    ta.cmp(&tb)
}
