//! Element (label and group) properties, as Biodiverse::ElementProperties (landschaft's bio/_properties.py), and
//! label matrices (bio/_matrix.py).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::basedata::{BaseData, read_rows};
use super::names::{csv_field, dequote_element, join_element, looks_like_number, numify};
use crate::{Error, Result};

/// Properties of elements: per property its name and each element's value (a Perl string: None not set, ""
/// set but undefined).
pub type Properties = Vec<(String, Vec<Option<String>>)>;

/// The properties of a file: their names, and per property each element's value.
pub type Read = (Vec<String>, Vec<HashMap<String, String>>);

/// The property tables of the index engine: names, offsets of their distinct values, the values as texts and as
/// numbers, each element's value (properties x elements).
pub type Tables = (Vec<String>, Vec<i64>, Vec<String>, Vec<f64>, Vec<i32>);

const RESERVED: [&str; 3] = ["INCLUDE", "EXCLUDE", "REMAP"];

/// A Biodiverse element properties file: per property (upper-cased), each element's value (only values that
/// look like numbers; "" for empty fields; a later row overrides an earlier one).
pub fn read_properties(
    text: &str,
    element_columns: &[usize],
    property_columns: &[(String, usize)],
    sep: Option<char>,
    quote: Option<char>,
) -> Result<Read> {
    let rows = read_rows(text, sep, quote);
    let names: Vec<(String, usize)> = property_columns.iter().map(|(n, c)| (n.to_uppercase(), *c)).collect();
    let mut order: Vec<String> = Vec::new();
    let mut records: HashMap<String, HashMap<String, String>> = HashMap::new();
    for row in rows.iter().skip(1) {
        if row.is_empty() {
            continue;
        }
        let cols: Vec<String> =
            element_columns.iter().map(|&c| row.get(c).cloned().flatten().unwrap_or_default()).collect();
        let joined = join_element(&cols);
        let element = if cols.len() <= 1 { dequote_element(&joined) } else { joined };
        let rec = records.entry(element.clone()).or_insert_with(|| {
            order.push(element.clone());
            HashMap::new()
        });
        for (name, col) in &names {
            match row.get(*col).cloned().flatten() {
                None => {
                    rec.insert(name.clone(), String::new());
                }
                Some(v) if looks_like_number(&v) => {
                    rec.insert(name.clone(), v);
                }
                _ => {}
            }
        }
    }
    let names: Vec<String> = names.into_iter().map(|(n, _)| n).collect();
    // per property: element -> value
    let per: Vec<HashMap<String, String>> = names
        .iter()
        .map(|n| order.iter().filter_map(|e| Some((e.clone(), records[e].get(n)?.clone()))).collect())
        .collect();
    Ok((names, per))
}

/// Properties of the given elements as Perl strings (normalise_properties): the reserved columns left out,
/// values that do not look like numbers dropped.
pub fn normalise(names: &[String], per: &[HashMap<String, String>], elements: &[String]) -> Properties {
    names
        .iter()
        .zip(per)
        .filter(|(n, _)| !RESERVED.contains(&n.as_str()))
        .map(|(n, values)| {
            let col = elements
                .iter()
                .map(|e| values.get(e).filter(|v| v.is_empty() || looks_like_number(v)).cloned())
                .collect();
            (n.clone(), col)
        })
        .collect()
}

/// Whether any property has a value set.
pub fn any_set(p: &Properties) -> bool {
    p.iter().any(|(_, col)| col.iter().any(Option::is_some))
}

/// The property tables of the index engine: per property its distinct values (Perl's string order), their
/// numbers and each element's value index (-1 not set, -2 set but undefined).
pub fn tables(p: &Properties) -> Tables {
    let (mut names, mut key_off, mut keys, mut values, mut elem) =
        (Vec::new(), vec![0i64], Vec::new(), Vec::new(), Vec::new());
    for (name, col) in p {
        if col.iter().all(Option::is_none) {
            continue;
        }
        let mut k: Vec<&String> = col.iter().flatten().filter(|v| !v.is_empty()).collect();
        k.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        k.dedup();
        let pos: HashMap<&String, i32> = k.iter().enumerate().map(|(i, v)| (*v, i as i32)).collect();
        names.push(name.clone());
        values.extend(k.iter().map(|v| numify(v)));
        keys.extend(k.iter().map(|v| (*v).clone()));
        key_off.push(keys.len() as i64);
        elem.extend(col.iter().map(|v| match v {
            None => -1,
            Some(s) if s.is_empty() => -2,
            Some(s) => pos[s],
        }));
    }
    (names, key_off, keys, values, elem)
}

/// A property's numbers (pandas' to_numeric, coerced: NaN where not set or not a number).
pub fn numbers(p: &Properties, name: &str) -> Option<Vec<f64>> {
    p.iter().find(|(n, _)| n == name).map(|(_, col)| {
        col.iter()
            .map(|v| match v {
                Some(s) if looks_like_number(s) => numify(s),
                _ => f64::NAN,
            })
            .collect()
    })
}

/// A symmetric matrix of values between labels (Biodiverse's), elements sorted; NaN for missing pairs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelMatrix {
    pub elements: Vec<String>,
    /// n x n, row-major.
    pub values: Vec<f64>,
    /// The value of a missing (a, a).
    pub self_similarity: Option<f64>,
}

/// How a matrix file is read (landschaft's read_matrix).
#[derive(Debug, Clone)]
pub struct MatrixImport {
    pub element_columns: Vec<usize>,
    pub values_start_col: Option<usize>,
    pub sparse: bool,
    pub row_columns: Vec<usize>,
    pub col_columns: Vec<usize>,
    pub value_column: Option<usize>,
    pub sep: Option<char>,
    pub quote: Option<char>,
    pub self_similarity: Option<f64>,
}

impl Default for MatrixImport {
    fn default() -> Self {
        MatrixImport {
            element_columns: vec![1, 2],
            values_start_col: None,
            sparse: false,
            row_columns: Vec::new(),
            col_columns: Vec::new(),
            value_column: None,
            sep: None,
            quote: None,
            self_similarity: None,
        }
    }
}

/// A Biodiverse matrix file (full, a triangle, or sparse pairs): the first value read for a pair wins.
pub fn read_matrix(text: &str, o: &MatrixImport) -> Result<LabelMatrix> {
    let rows = read_rows(text, o.sep, o.quote);
    let rows = &rows[1.min(rows.len())..];
    let field = |r: &[Option<String>], c: usize| r.get(c).cloned().flatten().unwrap_or_default();
    let name = |r: &[Option<String>], cols: &[usize]| {
        dequote_element(&cols.iter().map(|&c| csv_field(&field(r, c))).collect::<Vec<_>>().join(":"))
    };
    let mut pairs: Vec<(String, String, f64)> = Vec::new();
    if o.sparse {
        let Some(vc) = o.value_column.filter(|_| !o.row_columns.is_empty() && !o.col_columns.is_empty()) else {
            return Err(Error::msg("sparse matrices need row_columns, col_columns and value_column"));
        };
        for r in rows.iter().filter(|r| !r.is_empty()) {
            let (a, b) = (name(r, &o.row_columns), name(r, &o.col_columns));
            match r.get(vc).cloned().flatten() {
                Some(v) if looks_like_number(&v) => pairs.push((b, a, numify(&v))),
                _ => {}
            }
        }
    } else {
        let cols = &o.element_columns;
        let start = o.values_start_col.unwrap_or_else(|| cols.last().map_or(0, |c| c + 1));
        let mut labels = Vec::new();
        let mut data: Vec<Vec<Option<String>>> = Vec::new();
        for r in rows.iter().filter(|r| !r.is_empty()) {
            labels.push(name(r, cols));
            data.push(r.get(start..).map_or(Vec::new(), <[Option<String>]>::to_vec));
        }
        for (i, vals) in data.iter().enumerate() {
            for (j, lj) in labels.iter().enumerate() {
                match vals.get(j).cloned().flatten() {
                    Some(v) if !v.is_empty() && looks_like_number(&v) => {
                        pairs.push((labels[i].clone(), lj.clone(), numify(&v)))
                    }
                    _ => {}
                }
            }
        }
    }
    let mut names: Vec<String> = pairs.iter().flat_map(|p| [p.0.clone(), p.1.clone()]).collect();
    names.sort();
    names.dedup();
    let n = names.len();
    let idx: HashMap<&String, usize> = names.iter().enumerate().map(|(i, e)| (e, i)).collect();
    let mut v = vec![f64::NAN; n * n];
    for (a, b, val) in &pairs {
        let (i, j) = (idx[a], idx[b]);
        if v[i * n + j].is_nan() {
            v[i * n + j] = *val;
            v[j * n + i] = *val;
        }
    }
    Ok(LabelMatrix { elements: names, values: v, self_similarity: o.self_similarity })
}

impl BaseData {
    /// Label properties assigned (assign_element_properties): labels not in the basedata ignored.
    pub fn set_label_properties(&mut self, names: &[String], per: &[HashMap<String, String>]) {
        self.label_props = normalise(names, per, &self.labels);
    }

    /// Group properties assigned.
    pub fn set_group_properties(&mut self, names: &[String], per: &[HashMap<String, String>]) {
        self.group_props = normalise(names, per, &self.groups);
    }
}

/// The parsed rows of a delimited text (the separator and quote guessed when not given).
pub fn rows_of(text: &str, sep: Option<char>, quote: Option<char>) -> Vec<Vec<Option<String>>> {
    read_rows(text, sep, quote)
}
