//! Biodiverse's BaseData (landschaft's bio/_basedata.py): the groups x labels sample counts with the groups'
//! coordinates. Coordinates are snapped to the cells as Biodiverse's imports do, and groups and labels are
//! numbered in the sorted order of their names (so sums over them follow the reference runs).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::names::{csv_field, dequote_element, looks_like_number, natural_cmp, numify, perl_number, split_element};
use crate::{Error, Result};

/// Biodiverse's DEFAULT_PRECISION of the snapping.
const PRECISION: f64 = 1e10;

/// Groups x labels sample counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaseData {
    /// Group names, sorted (coordinates joined with `:`).
    pub groups: Vec<String>,
    /// Label names, sorted.
    pub labels: Vec<String>,
    /// The counts, CSR: a row per group, labels ascending.
    pub indptr: Vec<i64>,
    pub indices: Vec<i32>,
    pub counts: Vec<f64>,
    /// Group coordinates, n_groups x n_axes (text axes: their rank).
    pub coords: Vec<f64>,
    /// Cell size per axis (0: kept as given, negative: a text axis) and the grid's origin.
    pub cell_sizes: Vec<f64>,
    pub cell_origins: Vec<f64>,
    pub crs: Option<String>,
    /// The axis values of each group as Biodiverse keeps them (strings).
    pub group_axes: Vec<Vec<String>>,
    /// Label and group properties (Perl strings: None not set, "" set but undefined).
    #[serde(default)]
    pub label_props: super::properties::Properties,
    #[serde(default)]
    pub group_props: super::properties::Properties,
}

/// POSIX round: halves away from zero.
fn round_half_away(x: f64) -> f64 {
    let t = x.trunc();
    let frac = x - t;
    t + if frac.abs() >= 0.5 { x.signum() } else { 0.0 }
}

/// Cell centres as Biodiverse's text import computes them.
pub fn snap(v: f64, size: f64, origin: f64) -> f64 {
    let r = round_half_away(((v - origin) / size) * PRECISION) / PRECISION;
    (size * r.floor() + size / 2.0) + origin
}

/// The values of a group axis of every record: numbers, or text (None: missing).
pub enum Axis {
    Numbers(Vec<f64>),
    Texts(Vec<Option<String>>),
}

/// Codes of values in the order they first appear (pandas' factorize).
fn factorize<T: std::hash::Hash + Eq + Clone>(values: impl IntoIterator<Item = T>) -> (Vec<i64>, Vec<T>) {
    let mut index = HashMap::new();
    let mut uniq = Vec::new();
    let codes = values
        .into_iter()
        .map(|v| {
            *index.entry(v.clone()).or_insert_with(|| {
                uniq.push(v);
                uniq.len() as i64 - 1
            })
        })
        .collect();
    (codes, uniq)
}

impl BaseData {
    pub fn n_groups(&self) -> usize {
        self.groups.len()
    }

    pub fn n_labels(&self) -> usize {
        self.labels.len()
    }

    pub fn n_axes(&self) -> usize {
        self.cell_sizes.len()
    }

    pub fn group_index(&self, name: &str) -> Option<usize> {
        self.groups.binary_search_by(|g| g.as_str().cmp(name)).ok()
    }

    /// Total sample count of each group.
    pub fn group_sample_count(&self) -> Vec<f64> {
        (0..self.n_groups())
            .map(|g| {
                let mut s = 0.0;
                for k in self.indptr[g] as usize..self.indptr[g + 1] as usize {
                    s += self.counts[k];
                }
                s
            })
            .collect()
    }

    /// The labels and counts of each group as triplets, sorted by group and label names; the basedata keeps
    /// Biodiverse's name order.
    #[allow(clippy::too_many_arguments)]
    fn new(
        triplets: Vec<(usize, usize, f64)>,
        groups: Vec<String>,
        labels: Vec<String>,
        coords: Vec<f64>,
        cell_sizes: Vec<f64>,
        cell_origins: Vec<f64>,
        crs: Option<String>,
        group_axes: Vec<Vec<String>>,
    ) -> Result<BaseData> {
        let n_axes = cell_sizes.len();
        let mut g_order: Vec<usize> = (0..groups.len()).collect();
        g_order.sort_by(|&a, &b| groups[a].cmp(&groups[b]));
        let mut l_order: Vec<usize> = (0..labels.len()).collect();
        l_order.sort_by(|&a, &b| labels[a].cmp(&labels[b]));
        if g_order.windows(2).any(|w| groups[w[0]] == groups[w[1]]) {
            return Err(Error::msg("group names must be unique"));
        }
        if l_order.windows(2).any(|w| labels[w[0]] == labels[w[1]]) {
            return Err(Error::msg("label names must be unique"));
        }
        let mut g_rank = vec![0; groups.len()];
        for (r, &g) in g_order.iter().enumerate() {
            g_rank[g] = r;
        }
        let mut l_rank = vec![0; labels.len()];
        for (r, &l) in l_order.iter().enumerate() {
            l_rank[l] = r;
        }
        let mut t: Vec<(usize, usize, f64)> = triplets.into_iter().map(|(g, l, v)| (g_rank[g], l_rank[l], v)).collect();
        t.sort_by_key(|&(g, l, _)| (g, l));
        let n_g = groups.len();
        let mut indptr = vec![0i64; n_g + 1];
        for &(g, _, _) in &t {
            indptr[g + 1] += 1;
        }
        for g in 0..n_g {
            indptr[g + 1] += indptr[g];
        }
        Ok(BaseData {
            groups: g_order.iter().map(|&g| groups[g].clone()).collect(),
            labels: l_order.iter().map(|&l| labels[l].clone()).collect(),
            indptr,
            indices: t.iter().map(|&(_, l, _)| l as i32).collect(),
            counts: t.iter().map(|&(_, _, v)| v).collect(),
            coords: g_order.iter().flat_map(|&g| coords[g * n_axes..(g + 1) * n_axes].to_vec()).collect(),
            cell_sizes,
            cell_origins,
            crs,
            group_axes: g_order.iter().map(|&g| group_axes[g].clone()).collect(),
            label_props: Vec::new(),
            group_props: Vec::new(),
        })
    }

    /// From the group axis values, labels (None: a group-only record) and counts of records (landschaft's
    /// _from_records): numbers snapped when the cell size is > 0, kept when 0; text axes when < 0. Counts are
    /// summed per group and label in record order.
    #[allow(clippy::too_many_arguments)]
    pub fn from_records(
        axes: Vec<Axis>,
        labels: Vec<Option<String>>,
        counts: Vec<f64>,
        cell_sizes: Vec<f64>,
        cell_origins: Option<Vec<f64>>,
        binarise: bool,
        allow_empty_groups: bool,
        allow_empty_labels: bool,
        crs: Option<String>,
    ) -> Result<BaseData> {
        let n_axes = cell_sizes.len();
        if axes.len() != n_axes {
            return Err(Error::msg("one cell size per group axis is needed"));
        }
        let origins = cell_origins.unwrap_or_else(|| vec![0.0; n_axes]);
        let n = counts.len();
        let (mut codes, mut names, mut coords_u, mut text_axes) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for (k, axis) in axes.into_iter().enumerate() {
            let (size, origin) = (cell_sizes[k], origins[k]);
            match axis {
                Axis::Numbers(v) if size >= 0.0 => {
                    // np.unique: sorted (NaN last, 0 and -0 one value), then names in that order factorized
                    let canon = |x: f64| if x.is_nan() { f64::NAN } else { x + 0.0 };
                    let v: Vec<f64> =
                        v.iter().map(|&x| canon(if size > 0.0 { snap(x, size, origin) } else { x })).collect();
                    let mut uniq: Vec<f64> = v.clone();
                    uniq.sort_by(f64::total_cmp);
                    uniq.dedup_by(|a, b| a.total_cmp(b).is_eq());
                    let pos = |x: f64| uniq.binary_search_by(|u| u.total_cmp(&x)).unwrap();
                    let nm: Vec<String> = uniq.iter().map(|&u| perl_number(u)).collect();
                    let (inv2, nm2) = factorize(nm);
                    codes.push(v.iter().map(|&x| inv2[pos(x)]).collect::<Vec<i64>>());
                    coords_u.push(nm2.iter().map(|s| numify(s)).collect::<Vec<f64>>());
                    names.push(nm2);
                }
                axis => {
                    let strs: Vec<String> = match axis {
                        Axis::Numbers(v) => v.iter().map(|&x| perl_number(x)).collect(),
                        Axis::Texts(t) => t.into_iter().map(Option::unwrap_or_default).collect(),
                    };
                    let (inv, uniq) = factorize(strs);
                    codes.push(inv);
                    if size < 0.0 {
                        text_axes.push(k);
                        coords_u.push(vec![0.0; uniq.len()]);
                    } else {
                        coords_u.push(uniq.iter().map(|s| numify(s)).collect());
                    }
                    names.push(uniq);
                }
            }
        }
        // groups: a mixed-radix key over the axis codes
        let base = |k: usize| names[k].len().max(1) as i64;
        let key: Vec<i64> = (0..n).map(|r| (0..n_axes).fold(0i64, |acc, k| acc * base(k) + codes[k][r])).collect();
        let mut g_keys = key.clone();
        g_keys.sort_unstable();
        g_keys.dedup();
        let g_inv: Vec<usize> = key.iter().map(|k| g_keys.binary_search(k).unwrap()).collect();
        let n_g = g_keys.len();
        let mut g_codes = vec![vec![0i64; n_axes]; n_g];
        for (g, &gk) in g_keys.iter().enumerate() {
            let mut rem = gk;
            for k in (0..n_axes).rev() {
                g_codes[g][k] = rem % base(k);
                rem /= base(k);
            }
        }
        let (l_codes, l_names) = {
            let mut index = HashMap::new();
            let mut uniq: Vec<String> = Vec::new();
            let codes: Vec<i64> = labels
                .iter()
                .map(|l| match l {
                    None => -1,
                    Some(s) => *index.entry(s.clone()).or_insert_with(|| {
                        uniq.push(s.clone());
                        uniq.len() as i64 - 1
                    }),
                })
                .collect();
            (codes, uniq)
        };
        let n_l = l_names.len();
        let lbase = n_l.max(1) as i64;
        // collate in record order
        let pairs: Vec<(i64, f64)> =
            (0..n).filter(|&r| l_codes[r] >= 0).map(|r| (g_inv[r] as i64 * lbase + l_codes[r], counts[r])).collect();
        let mut k_uniq: Vec<i64> = pairs.iter().map(|p| p.0).collect();
        k_uniq.sort_unstable();
        k_uniq.dedup();
        let mut sums = vec![0.0; k_uniq.len()];
        for &(p, c) in &pairs {
            let i = k_uniq.binary_search(&p).unwrap();
            sums[i] += c;
        }
        if binarise {
            sums.iter_mut().for_each(|s| *s = f64::from(u8::from(*s != 0.0)));
        }
        let mut group_used = vec![false; n_g];
        let mut label_used = vec![false; n_l];
        for (i, &p) in k_uniq.iter().enumerate() {
            if sums[i] != 0.0 {
                group_used[(p / lbase) as usize] = true;
                label_used[(p % lbase) as usize] = true;
            }
        }
        if allow_empty_groups {
            group_used.iter_mut().for_each(|u| *u = true);
        }
        if allow_empty_labels {
            for &c in l_codes.iter().filter(|c| **c >= 0) {
                label_used[c as usize] = true;
            }
        }
        let compact = |used: &[bool]| -> Vec<i64> {
            let mut next = 0;
            used.iter()
                .map(|&u| {
                    if u {
                        next += 1;
                        next - 1
                    } else {
                        -1
                    }
                })
                .collect()
        };
        let (g_map, l_map) = (compact(&group_used), compact(&label_used));
        let triplets: Vec<(usize, usize, f64)> = k_uniq
            .iter()
            .zip(&sums)
            .filter(|(_, s)| **s != 0.0)
            .map(|(&p, &s)| (g_map[(p / lbase) as usize] as usize, l_map[(p % lbase) as usize] as usize, s))
            .collect();
        let used: Vec<usize> = (0..n_g).filter(|&g| group_used[g]).collect();
        let parts: Vec<Vec<String>> =
            used.iter().map(|&g| (0..n_axes).map(|k| names[k][g_codes[g][k] as usize].clone()).collect()).collect();
        let gnames: Vec<String> = parts
            .iter()
            .map(|pp| {
                if n_axes == 1 {
                    dequote_element(&csv_field(&pp[0]))
                } else {
                    pp.iter().map(|x| csv_field(x)).collect::<Vec<_>>().join(":")
                }
            })
            .collect();
        let coords: Vec<f64> = used
            .iter()
            .flat_map(|&g| (0..n_axes).map(|k| coords_u[k][g_codes[g][k] as usize]).collect::<Vec<_>>())
            .collect();
        let lnames: Vec<String> = (0..n_l).filter(|&l| label_used[l]).map(|l| l_names[l].clone()).collect();
        let mut bd = BaseData::new(triplets, gnames, lnames, coords, cell_sizes, origins, crs, parts)?;
        // text axes: the rank of each value in reverse natural order
        for k in text_axes {
            let mut uniq: Vec<String> = bd.group_axes.iter().map(|a| a[k].clone()).collect();
            uniq.sort_by(|a, b| natural_cmp(b, a));
            uniq.dedup();
            for g in 0..bd.n_groups() {
                bd.coords[g * n_axes + k] = uniq.iter().position(|u| *u == bd.group_axes[g][k]).unwrap() as f64;
            }
        }
        Ok(bd)
    }

    /// The axis values of a group (strings).
    pub fn axes_of(&self, g: usize) -> Vec<String> {
        if !self.group_axes.is_empty() {
            return self.group_axes[g].clone();
        }
        if self.n_axes() > 1 { split_element(&self.groups[g]) } else { vec![self.groups[g].clone()] }
    }
}

/// Text::CSV_XS's truth of a field: not empty, not "0".
fn perl_true(v: Option<&str>) -> bool {
    v.is_some_and(|s| !s.is_empty() && s != "0")
}

/// The quote of a text: the more frequent of " and ' with an even count (landschaft's _guess_quote).
fn guess_quote(text: &str) -> char {
    let mut best: Option<(usize, char)> = None;
    for q in ['"', '\''] {
        let k = text.matches(q).count();
        if k > 0 && k % 2 == 0 && best.is_none_or(|(b, _)| k > b) {
            best = Some((k, q));
        }
    }
    best.map_or('"', |b| b.1)
}

/// Python's csv.reader of a text: rows of fields (empty unquoted fields None, Text::CSV_XS's blank_is_undef).
pub fn read_csv(text: &str, sep: char, quote: char) -> Vec<Vec<Option<String>>> {
    let mut rows = Vec::new();
    let mut row: Vec<Option<String>> = Vec::new();
    let mut field = String::new();
    let (mut quoted, mut in_quotes, mut started) = (false, false, false);
    let mut chars = text.chars().peekable();
    let end_field = |row: &mut Vec<Option<String>>, field: &mut String, quoted: bool| {
        let f = std::mem::take(field);
        row.push(if f.is_empty() && !quoted { None } else { Some(f) });
    };
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == quote {
                if chars.peek() == Some(&quote) {
                    field.push(quote);
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            _ if c == quote && field.is_empty() && !quoted => {
                in_quotes = true;
                quoted = true;
                started = true;
            }
            _ if c == sep => {
                end_field(&mut row, &mut field, quoted);
                quoted = false;
                started = true;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                if started || !field.is_empty() || !row.is_empty() {
                    end_field(&mut row, &mut field, quoted);
                }
                rows.push(std::mem::take(&mut row));
                quoted = false;
                started = false;
            }
            _ => {
                field.push(c);
                started = true;
            }
        }
    }
    if started || !field.is_empty() || !row.is_empty() {
        end_field(&mut row, &mut field, quoted);
        rows.push(row);
    }
    rows
}

/// The separator of the first lines: the one giving the most (and as many on every line) fields
/// (landschaft's _guess_separator).
fn guess_separator(lines: &[&str], quote: char) -> char {
    let (mut best, mut best_n) = (',', 1);
    for sep in [',', '\t', ';', ' '] {
        if !lines.iter().any(|l| l.contains(sep)) {
            continue;
        }
        let counts: Vec<usize> =
            read_csv(&lines.join("\n"), sep, quote).iter().filter(|r| !r.is_empty()).map(Vec::len).collect();
        if !counts.is_empty() && counts.iter().all(|&c| c > 1 && c == counts[0]) && counts[0] > best_n {
            (best, best_n) = (sep, counts[0]);
        }
    }
    if best_n == 1 {
        for sep in [',', '\t', ';', ' '] {
            let n = read_csv(lines.first().copied().unwrap_or(""), sep, quote).first().map_or(0, Vec::len);
            if n > best_n {
                (best, best_n) = (sep, n);
            }
        }
    }
    best
}

/// The rows of a delimited text (landschaft's _read_rows): the quote and separator guessed from the first lines
/// when not given; empty unquoted fields None.
pub fn read_rows(text: &str, sep: Option<char>, quote: Option<char>) -> Vec<Vec<Option<String>>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first: String = text.chars().take(20000).collect();
    let head: Vec<&str> = first.lines().take(11).collect();
    let head = if head.is_empty() { vec![""] } else { head };
    let quote = quote.unwrap_or_else(|| guess_quote(&head.join("\n")));
    let sep = sep.unwrap_or_else(|| guess_separator(&head, quote));
    read_csv(text, sep, quote)
}

/// How Biodiverse's text import reads a table (landschaft's BaseData.from_table).
#[derive(Debug, Clone, Default)]
pub struct TableImport {
    pub group_columns: Vec<String>,
    pub label_columns: Vec<String>,
    pub sample_count_columns: Vec<String>,
    pub cell_sizes: Option<Vec<f64>>,
    pub cell_origins: Option<Vec<f64>>,
    pub sep: Option<char>,
    pub quote: Option<char>,
    pub include_columns: Vec<String>,
    pub exclude_columns: Vec<String>,
    pub binarise_counts: bool,
    pub allow_empty_groups: bool,
    pub allow_empty_labels: bool,
    pub crs: Option<String>,
}

impl BaseData {
    /// Delimited text read like Biodiverse's `import_data` (columns by name or by 0-based position).
    pub fn from_table(text: &str, o: &TableImport) -> Result<BaseData> {
        let rows = read_rows(text, o.sep, o.quote);
        let Some((header, body)) = rows.split_first() else { return Err(Error::msg("empty table")) };
        let header: Vec<String> = header.iter().map(|h| h.clone().unwrap_or_default()).collect();
        let col = |c: &String| -> Result<usize> {
            if let Some(i) = header.iter().position(|h| h == c) {
                return Ok(i);
            }
            c.parse().map_err(|_| Error::msg(format!("column {c:?} not found")))
        };
        let gcols: Vec<usize> = o.group_columns.iter().map(col).collect::<Result<_>>()?;
        let n_axes = gcols.len();
        let sizes = o.cell_sizes.clone().unwrap_or_else(|| vec![0.0; n_axes]);
        if sizes.len() != n_axes {
            return Err(Error::msg(format!(
                "Number of group columns differs from cellsizes ({n_axes} != {})",
                sizes.len()
            )));
        }
        let lcols: Vec<usize> = o.label_columns.iter().map(col).collect::<Result<_>>()?;
        let scols: Vec<usize> = o.sample_count_columns.iter().map(col).collect::<Result<_>>()?;
        let icols: Vec<usize> = o.include_columns.iter().map(col).collect::<Result<_>>()?;
        let ecols: Vec<usize> = o.exclude_columns.iter().map(col).collect::<Result<_>>()?;
        let mut num: Vec<Vec<f64>> = vec![Vec::new(); n_axes];
        let mut txt: Vec<Vec<Option<String>>> = vec![Vec::new(); n_axes];
        let (mut labels, mut counts) = (Vec::new(), Vec::new());
        for row in body {
            if row.is_empty() {
                continue;
            }
            let get = |i: usize| row.get(i).and_then(|v| v.as_deref());
            if !ecols.is_empty() && ecols.iter().any(|&c| perl_true(get(c))) {
                continue;
            }
            if !icols.is_empty() && !icols.iter().any(|&c| perl_true(get(c))) {
                continue;
            }
            let mut gvals: Vec<Option<String>> = Vec::new();
            let mut skip = false;
            for (k, &c) in gcols.iter().enumerate() {
                let v = get(c);
                if sizes[k] >= 0.0 {
                    if matches!(v, None | Some("NA") | Some("")) {
                        skip = true;
                        break;
                    }
                    if !v.is_some_and(looks_like_number) {
                        return Err(Error::msg(format!("Non-numeric group field in column {c} ({v:?})")));
                    }
                    gvals.push(v.map(str::to_string));
                } else {
                    gvals.push(Some(v.unwrap_or("").to_string()));
                }
            }
            if skip {
                continue;
            }
            let joined = lcols.iter().map(|&c| csv_field(get(c).unwrap_or(""))).collect::<Vec<_>>().join(":");
            let lab = if lcols.len() <= 1 { dequote_element(&joined) } else { joined };
            let cnt = if scols.is_empty() {
                1.0
            } else {
                let mut total: Option<f64> = None;
                for &c in &scols {
                    let v = get(c).unwrap_or("0");
                    if !looks_like_number(v) {
                        return Err(Error::msg(format!("Field {c} does not look like a number")));
                    }
                    total = Some(total.map_or(numify(v), |t| t + numify(v)));
                }
                total.unwrap()
            };
            if cnt == 0.0 && !o.allow_empty_groups {
                continue;
            }
            for (k, v) in gvals.into_iter().enumerate() {
                if sizes[k] > 0.0 {
                    num[k].push(numify(v.as_deref().unwrap()));
                } else {
                    txt[k].push(v);
                }
            }
            labels.push(Some(lab));
            counts.push(cnt);
        }
        let axes = (0..n_axes)
            .map(|k| {
                if sizes[k] > 0.0 {
                    Axis::Numbers(std::mem::take(&mut num[k]))
                } else {
                    Axis::Texts(std::mem::take(&mut txt[k]))
                }
            })
            .collect();
        BaseData::from_records(
            axes,
            labels,
            counts,
            sizes,
            o.cell_origins.clone(),
            o.binarise_counts,
            o.allow_empty_groups,
            o.allow_empty_labels,
            o.crs.clone(),
        )
    }

    /// Point records read like Biodiverse's feature import (landschaft's BaseData.from_points): each point in
    /// the cell holding it (a plain floor), its label the label columns joined with `:`, its count 1 or a count
    /// column's.
    pub fn from_points(
        xy: &[[f64; 2]],
        labels: Vec<String>,
        counts: Vec<f64>,
        cell_sizes: [f64; 2],
        cell_origins: [f64; 2],
        crs: Option<String>,
    ) -> Result<BaseData> {
        let [sx, sy] = cell_sizes;
        let [ox, oy] = cell_origins;
        let snap_floor = |v: f64, s: f64, o: f64| if s > 0.0 { o + ((v - o) / s).floor() * s + s / 2.0 } else { v };
        let x = xy.iter().map(|p| snap_floor(p[0], sx, ox)).collect();
        let y = xy.iter().map(|p| snap_floor(p[1], sy, oy)).collect();
        let mut bd = BaseData::from_records(
            vec![Axis::Numbers(x), Axis::Numbers(y)],
            labels.into_iter().map(Some).collect(),
            counts,
            vec![0.0, 0.0],
            Some(vec![ox, oy]),
            false,
            false,
            false,
            crs,
        )?;
        bd.cell_sizes = vec![sx, sy];
        Ok(bd)
    }
}
