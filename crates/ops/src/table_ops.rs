//! Tables worked on, as a spreadsheet or dplyr would (the tables of metrics, transitions, zonal statistics,
//! evaluations): rows kept by an expression (`table.filter`), a column computed from the others (`table.calc`, the
//! expressions of map algebra over the columns), rows grouped and summarised (`table.aggregate`), two tables joined
//! by a key (`table.join`), rows sorted and the first kept (`table.sort`), columns chosen, dropped and renamed
//! (`table.select`), and a table turned from long to wide or back (`table.pivot`: transitions into a matrix).
//!
//! The values of a table are numbers (none: missing). An expression names a column by its name when the name is
//! one (a letter or `_`, then letters, digits or `_`); the node's other inputs are numbers by their port's name.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::algebra::{Scope, parse};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Filter));
    r.add(Arc::new(Calc));
    r.add(Arc::new(Aggregate));
    r.add(Arc::new(Join));
    r.add(Arc::new(Sort));
    r.add(Arc::new(Select));
    r.add(Arc::new(Pivot));
}

fn table<'a>(args: &'a Args, port: &str) -> Result<&'a Table> {
    match args.get(port) {
        Some(Value::Table(t)) => Ok(t),
        Some(other) => Err(Error::msg(format!("node {:?}: {port} is a {}, not a table", args.node, other.ty().name()))),
        None => Err(Error::msg(format!("node {:?}: no input {port:?}", args.node))),
    }
}

/// A setting naming columns: one name, names separated by commas, or a list; none when not given.
fn names(args: &Args, name: &str) -> Result<Vec<String>> {
    Ok(match args.params.get(name) {
        None | Some(serde_json::Value::Null) => Vec::new(),
        Some(serde_json::Value::String(s)) => {
            s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
        }
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| {
            Error::msg(format!("node {:?}: the setting {name:?}: a name or a list of them ({e})", args.node))
        })?,
    })
}

/// The index of a column, or why there is none.
fn index(t: &Table, name: &str, node: &str) -> Result<usize> {
    t.columns.iter().position(|c| c == name).ok_or_else(|| {
        Error::msg(format!("node {node:?}: the table has no column {name:?} (its columns: {})", t.columns.join(", ")))
    })
}

/// An expression's value at every row of a table: its columns by name, the node's numbers by port.
pub(crate) fn evaluate(args: &Args, t: &Table, text: &str) -> Result<Vec<f64>> {
    let node = args.node;
    let e = parse(text).map_err(|x| Error::msg(format!("node {node:?}: the expression: {}", x.0)))?;
    if e.radius() > 0 {
        return Err(Error::msg(format!(
            "node {node:?}: a table's rows have no neighbours: nbsum and the like are for maps"
        )));
    }
    let columns: Vec<Vec<f64>> = (0..t.columns.len()).map(|j| t.rows.iter().map(|r| r[j]).collect()).collect();
    let mut scope = Scope {
        width: t.rows.len(),
        height: 1,
        maps: BTreeMap::new(),
        numbers: BTreeMap::new(),
        stacks: BTreeMap::new(),
    };
    for (c, values) in t.columns.iter().zip(&columns) {
        scope.maps.insert(c.as_str(), values.as_slice());
    }
    for (port, v) in &args.inputs {
        if matches!(v, Value::Table(_) | Value::Vector(_)) {
            continue;
        }
        let x = v.as_number().ok_or_else(|| {
            Error::msg(format!("node {node:?}: {port} is not a number (an expression's other inputs are)"))
        })?;
        scope.numbers.insert(port.as_str(), x);
    }
    for name in e.names() {
        if !scope.maps.contains_key(name.as_str()) && !scope.numbers.contains_key(name.as_str()) {
            return Err(Error::msg(format!(
                "node {node:?}: the expression names {name:?}, neither a column (the table's: {}) nor an input",
                t.columns.join(", ")
            )));
        }
    }
    e.evaluate(&scope).map_err(|x| Error::msg(format!("node {node:?}: {}", x.0)))
}

/// `table.filter`.
struct Filter;

impl Operator for Filter {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "table.filter",
            "The rows of a table where an expression is true, as dplyr's filter: the columns by their names, the \
             expressions of map algebra (class == 3 and area > 10; isnull(x) for a missing value), the node's other \
             inputs as numbers by their names. A row where the expression is missing is left out.",
            Access::None,
        )
        .input(PortSpec::new("table", Type::Table, "the table"))
        .extra(Type::Any)
        .output(PortSpec::new("table", Type::Table, "its rows where the expression is true"))
        .param(PortSpec::new("expression", Type::Text, "what a row kept is, e.g. class == 3 and area > 10"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let t = table(args, "table")?;
        let text: String =
            args.opt_param("expression")?.ok_or_else(|| Error::msg(format!("node {:?}: no expression", args.node)))?;
        let keep = evaluate(args, t, &text)?;
        let mut out = Table::new(t.columns.clone());
        out.rows =
            t.rows.iter().zip(&keep).filter(|(_, k)| !k.is_nan() && **k != 0.0).map(|(r, _)| r.clone()).collect();
        Ok(outputs([("table", Value::Table(out))]))
    }
}

/// `table.calc`.
struct Calc;

impl Operator for Calc {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "table.calc",
            "A column computed row by row from an expression over the others, as a spreadsheet's field calculator or \
             dplyr's mutate: the expressions of map algebra (area / 10000, if class == 1 then cells else 0), the \
             node's other inputs as numbers by their names. A column of the same name is replaced, else it is added \
             at the end.",
            Access::None,
        )
        .input(PortSpec::new("table", Type::Table, "the table"))
        .extra(Type::Any)
        .output(PortSpec::new("table", Type::Table, "the table with the column"))
        .param(PortSpec::new("column", Type::Text, "the column's name"))
        .param(PortSpec::new("expression", Type::Text, "its value at every row, e.g. area / 10000"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let t = table(args, "table")?;
        let column: String =
            args.opt_param("column")?.ok_or_else(|| Error::msg(format!("node {:?}: no column's name", args.node)))?;
        let text: String =
            args.opt_param("expression")?.ok_or_else(|| Error::msg(format!("node {:?}: no expression", args.node)))?;
        let values = evaluate(args, t, &text)?;
        let mut out = t.clone();
        match out.columns.iter().position(|c| *c == column) {
            Some(j) => out.rows.iter_mut().zip(&values).for_each(|(r, v)| r[j] = *v),
            None => {
                out.columns.push(column);
                out.rows.iter_mut().zip(&values).for_each(|(r, v)| r.push(*v));
            }
        }
        Ok(outputs([("table", Value::Table(out))]))
    }
}

/// The statistics `table.aggregate` gives.
const STATS: [&str; 7] = ["sum", "mean", "count", "min", "max", "sd", "median"];

/// A statistic of values, the missing ones left out (none left: missing; a count of none: 0).
fn statistic(stat: &str, values: &[f64]) -> f64 {
    let v: Vec<f64> = values.iter().copied().filter(|x| !x.is_nan()).collect();
    let n = v.len() as f64;
    match stat {
        "count" => n,
        _ if v.is_empty() => f64::NAN,
        "sum" => v.iter().sum(),
        "mean" => v.iter().sum::<f64>() / n,
        "min" => v.iter().copied().fold(f64::INFINITY, f64::min),
        "max" => v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        // (n - 1 in the denominator, as R's sd and pandas' std)
        "sd" if v.len() < 2 => f64::NAN,
        "sd" => {
            let m = v.iter().sum::<f64>() / n;
            (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
        }
        "median" => {
            let mut s = v;
            s.sort_by(f64::total_cmp);
            let k = s.len() / 2;
            if s.len() % 2 == 1 { s[k] } else { (s[k - 1] + s[k]) / 2.0 }
        }
        _ => f64::NAN,
    }
}

/// A row's key of some columns, in an order (missing values last).
fn key_cmp(a: &[f64], b: &[f64]) -> std::cmp::Ordering {
    for (x, y) in a.iter().zip(b) {
        let o = match (x.is_nan(), y.is_nan()) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => x.total_cmp(y),
        };
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    std::cmp::Ordering::Equal
}

/// `table.aggregate`.
struct Aggregate;

impl Operator for Aggregate {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "table.aggregate",
            "Rows grouped by the values of some columns and summarised, as dplyr's group_by and summarise: a row per \
             group (the groups in the order of their values), its columns n (the rows in the group) and, for every \
             column summarised and statistic, column_statistic (area_sum, area_mean). Missing values are left out \
             of the statistics; sd is the sample's (n - 1, as R's sd). Without columns to group by, the whole table \
             is one group.",
            Access::None,
        )
        .input(PortSpec::new("table", Type::Table, "the table"))
        .output(PortSpec::new("table", Type::Table, "a row per group"))
        .param(PortSpec::optional(
            "by",
            Type::Any,
            "the columns to group by: a name, names separated by commas or a list (none: one group)",
        ))
        .param(PortSpec::optional("columns", Type::Any, "the columns summarised (default: every other)"))
        .param(PortSpec::optional(
            "stats",
            Type::Any,
            "the statistics: sum, mean, count, min, max, sd, median (default sum)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let t = table(args, "table")?;
        let by: Vec<usize> = names(args, "by")?.iter().map(|b| index(t, b, node)).collect::<Result<_>>()?;
        let mut columns = names(args, "columns")?;
        if columns.is_empty() {
            columns = t.columns.iter().enumerate().filter(|(j, _)| !by.contains(j)).map(|(_, c)| c.clone()).collect();
        }
        let cols: Vec<usize> = columns.iter().map(|c| index(t, c, node)).collect::<Result<_>>()?;
        let mut stats = names(args, "stats")?;
        if stats.is_empty() {
            stats = vec!["sum".into()];
        }
        if let Some(s) = stats.iter().find(|s| !STATS.contains(&s.as_str())) {
            return Err(Error::msg(format!(
                "node {node:?}: no statistic {s:?} (the statistics: {})",
                STATS.join(", ")
            )));
        }
        // the groups in the order of their keys
        let mut order: Vec<usize> = (0..t.rows.len()).collect();
        let key = |r: usize| by.iter().map(|&j| t.rows[r][j]).collect::<Vec<f64>>();
        order.sort_by(|&a, &b| key_cmp(&key(a), &key(b)).then(a.cmp(&b)));
        let mut groups: Vec<(Vec<f64>, Vec<usize>)> = Vec::new();
        for r in order {
            let k = key(r);
            match groups.last_mut() {
                Some((g, rows)) if key_cmp(g, &k) == std::cmp::Ordering::Equal => rows.push(r),
                _ => groups.push((k, vec![r])),
            }
        }
        if by.is_empty() && groups.is_empty() {
            groups.push((Vec::new(), Vec::new()));
        }
        let mut head: Vec<String> = by.iter().map(|&j| t.columns[j].clone()).collect();
        head.push("n".into());
        for (c, _) in columns.iter().zip(&cols) {
            for s in &stats {
                head.push(format!("{c}_{s}"));
            }
        }
        let mut out = Table::new(head);
        for (k, rows) in groups {
            let mut row = k;
            row.push(rows.len() as f64);
            for &j in &cols {
                let values: Vec<f64> = rows.iter().map(|&r| t.rows[r][j]).collect();
                for s in &stats {
                    row.push(statistic(s, &values));
                }
            }
            out.rows.push(row);
        }
        Ok(outputs([("table", Value::Table(out))]))
    }
}

/// `table.join`.
struct Join;

impl Operator for Join {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "table.join",
            "Two tables joined by key columns, as dplyr's joins: every row of the left with each row of the right of \
             the same key (left: the left's rows without one kept, the right's columns missing; inner: only the rows \
             of keys in both; full: the right's rows without one added too). The right's other columns follow the \
             left's; one of a name the left has takes a suffix.",
            Access::None,
        )
        .input(PortSpec::new("left", Type::Table, "the left table"))
        .input(PortSpec::new("right", Type::Table, "the right table"))
        .output(PortSpec::new("table", Type::Table, "the tables joined"))
        .param(PortSpec::new(
            "on",
            Type::Any,
            "the key columns, of the same names in both: a name, names separated by commas or a list",
        ))
        .param(PortSpec::optional(
            "right_on",
            Type::Any,
            "the right's key columns, when they are named otherwise (as many as on)",
        ))
        .param(
            PortSpec::optional("how", Type::Text, "left, inner or full (default left)")
                .choices(["left", "inner", "full"]),
        )
        .param(PortSpec::optional(
            "suffix",
            Type::Text,
            "added to a right's column of a name the left has (default _right)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let (l, r) = (table(args, "left")?, table(args, "right")?);
        let on = names(args, "on")?;
        if on.is_empty() {
            return Err(Error::msg(format!("node {node:?}: give the key columns (on)")));
        }
        let mut right_on = names(args, "right_on")?;
        if right_on.is_empty() {
            right_on = on.clone();
        }
        if right_on.len() != on.len() {
            return Err(Error::msg(format!("node {node:?}: on and right_on name as many columns")));
        }
        let how: String = args.param("how", "left".to_string())?;
        if !["left", "inner", "full"].contains(&how.as_str()) {
            return Err(Error::msg(format!("node {node:?}: how is left, inner or full, not {how:?}")));
        }
        let suffix: String = args.param("suffix", "_right".to_string())?;
        let lk: Vec<usize> = on.iter().map(|c| index(l, c, node)).collect::<Result<_>>()?;
        let rk: Vec<usize> = right_on.iter().map(|c| index(r, c, node)).collect::<Result<_>>()?;
        let others: Vec<usize> = (0..r.columns.len()).filter(|j| !rk.contains(j)).collect();
        let mut head = l.columns.clone();
        for &j in &others {
            let c = &r.columns[j];
            head.push(if l.columns.contains(c) { format!("{c}{suffix}") } else { c.clone() });
        }
        // the right's rows by key (a missing value matches nothing)
        let bits = |row: &[f64], k: &[usize]| -> Option<Vec<u64>> {
            k.iter().map(|&j| (!row[j].is_nan()).then(|| (row[j] + 0.0).to_bits())).collect()
        };
        let mut index_r: BTreeMap<Vec<u64>, Vec<usize>> = BTreeMap::new();
        for (i, row) in r.rows.iter().enumerate() {
            if let Some(k) = bits(row, &rk) {
                index_r.entry(k).or_default().push(i);
            }
        }
        let mut out = Table::new(head);
        let mut matched = vec![false; r.rows.len()];
        for row in &l.rows {
            let hits = bits(row, &lk).and_then(|k| index_r.get(&k)).cloned().unwrap_or_default();
            if hits.is_empty() {
                if how != "inner" {
                    let mut x = row.clone();
                    x.extend(others.iter().map(|_| f64::NAN));
                    out.rows.push(x);
                }
                continue;
            }
            for i in hits {
                matched[i] = true;
                let mut x = row.clone();
                x.extend(others.iter().map(|&j| r.rows[i][j]));
                out.rows.push(x);
            }
        }
        if how == "full" {
            for (i, row) in r.rows.iter().enumerate().filter(|(i, _)| !matched[*i]) {
                let mut x = vec![f64::NAN; l.columns.len()];
                for (a, &b) in lk.iter().zip(&rk) {
                    x[*a] = row[b];
                }
                x.extend(others.iter().map(|&j| r.rows[i][j]));
                out.rows.push(x);
            }
        }
        Ok(outputs([("table", Value::Table(out))]))
    }
}

/// `table.sort`.
struct Sort;

impl Operator for Sort {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "table.sort",
            "The rows of a table sorted by some columns (the first, then the next where it ties; missing values \
             last), as dplyr's arrange, and the first rows kept (top: the ten largest patches, say).",
            Access::None,
        )
        .input(PortSpec::new("table", Type::Table, "the table"))
        .output(PortSpec::new("table", Type::Table, "its rows sorted"))
        .param(PortSpec::new("by", Type::Any, "the columns to sort by: a name, names separated by commas or a list"))
        .param(PortSpec::optional("descending", Type::Bool, "from the largest (default false)"))
        .param(PortSpec::optional("top", Type::Integer, "the first rows kept (default every row)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let t = table(args, "table")?;
        let by: Vec<usize> = names(args, "by")?.iter().map(|b| index(t, b, node)).collect::<Result<_>>()?;
        if by.is_empty() {
            return Err(Error::msg(format!("node {node:?}: give the columns to sort by (by)")));
        }
        let descending: bool = args.param("descending", false)?;
        let top: Option<usize> = args.opt_param("top")?;
        let mut rows = t.rows.clone();
        let key = |r: &Vec<f64>| by.iter().map(|&j| r[j]).collect::<Vec<f64>>();
        // (stable: rows of the same key in their order; missing values last either way)
        rows.sort_by(|a, b| {
            let (ka, kb) = (key(a), key(b));
            if descending {
                for (x, y) in ka.iter().zip(&kb) {
                    let o = match (x.is_nan(), y.is_nan()) {
                        (true, true) => std::cmp::Ordering::Equal,
                        (true, false) => std::cmp::Ordering::Greater,
                        (false, true) => std::cmp::Ordering::Less,
                        _ => y.total_cmp(x),
                    };
                    if o != std::cmp::Ordering::Equal {
                        return o;
                    }
                }
                std::cmp::Ordering::Equal
            } else {
                key_cmp(&ka, &kb)
            }
        });
        if let Some(n) = top {
            rows.truncate(n);
        }
        let mut out = Table::new(t.columns.clone());
        out.rows = rows;
        Ok(outputs([("table", Value::Table(out))]))
    }
}

/// `table.select`.
struct Select;

impl Operator for Select {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "table.select",
            "Columns of a table chosen (in the order given), left out, or renamed, as dplyr's select and rename.",
            Access::None,
        )
        .input(PortSpec::new("table", Type::Table, "the table"))
        .output(PortSpec::new("table", Type::Table, "its columns chosen"))
        .param(PortSpec::optional("columns", Type::Any, "the columns kept, in this order (default every one)"))
        .param(PortSpec::optional("drop", Type::Any, "the columns left out"))
        .param(PortSpec::optional("rename", Type::Any, "new names: { old = \"new\", ... }"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let t = table(args, "table")?;
        let mut keep = names(args, "columns")?;
        if keep.is_empty() {
            keep = t.columns.clone();
        }
        let drop = names(args, "drop")?;
        for d in &drop {
            index(t, d, node)?;
        }
        keep.retain(|c| !drop.contains(c));
        let cols: Vec<usize> = keep.iter().map(|c| index(t, c, node)).collect::<Result<_>>()?;
        let rename: BTreeMap<String, String> = args.param("rename", BTreeMap::new())?;
        for old in rename.keys() {
            if !keep.contains(old) {
                return Err(Error::msg(format!("node {node:?}: no column {old:?} to rename among those kept")));
            }
        }
        let head: Vec<String> = keep.iter().map(|c| rename.get(c).cloned().unwrap_or_else(|| c.clone())).collect();
        let mut out = Table::new(head);
        out.rows = t.rows.iter().map(|r| cols.iter().map(|&j| r[j]).collect()).collect();
        Ok(outputs([("table", Value::Table(out))]))
    }
}

/// A number in a column's name: the whole name, or what follows a prefix.
fn number_in(name: &str, prefix: &str) -> Option<f64> {
    name.strip_prefix(prefix).unwrap_or(name).trim().parse().ok()
}

/// `table.pivot`.
struct Pivot;

impl Operator for Pivot {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "table.pivot",
            "A table from long to wide or back, as tidyr's pivot_wider and pivot_longer. Wider: a row per value of \
             the id columns, a column per value of the names column (named prefix and the value: to_1, to_2...) \
             holding the values column (transitions from, to, cells into a matrix). Longer: the columns given (or \
             every column but the id ones) into rows of two columns, the names and the values; a table's values \
             being numbers, the name of a column is the number in it (2011 of y2011 with prefix y; 1 of to_1), else \
             its place among them (1, 2...).",
            Access::None,
        )
        .input(PortSpec::new("table", Type::Table, "the table"))
        .output(PortSpec::new("table", Type::Table, "the table pivoted"))
        .param(PortSpec::new("direction", Type::Text, "wider or longer").choices(["wider", "longer"]))
        .param(PortSpec::optional("id", Type::Any, "the columns that make a row (wider), or kept as they are (longer)"))
        .param(PortSpec::optional("names", Type::Text, "wider: the column whose values become columns"))
        .param(PortSpec::optional("values", Type::Text, "wider: the column whose values fill them"))
        .param(PortSpec::optional("prefix", Type::Text, "the new columns' names before the value (wider), or before the number in the names (longer) (default: the names column's name and _)"))
        .param(PortSpec::optional("fill", Type::Number, "wider: the value of a combination absent (default none)"))
        .param(PortSpec::optional("columns", Type::Any, "longer: the columns turned into rows (default every one but the id ones)"))
        .param(PortSpec::optional("names_to", Type::Text, "longer: the column of the names (default name)"))
        .param(PortSpec::optional("values_to", Type::Text, "longer: the column of the values (default value)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let t = table(args, "table")?;
        let direction: String = args
            .opt_param("direction")?
            .ok_or_else(|| Error::msg(format!("node {node:?}: give the direction (wider or longer)")))?;
        let id: Vec<usize> = names(args, "id")?.iter().map(|c| index(t, c, node)).collect::<Result<_>>()?;
        let out = match direction.as_str() {
            "wider" => {
                let col = |name: &str| -> Result<usize> {
                    let n: String = args
                        .opt_param(name)?
                        .ok_or_else(|| Error::msg(format!("node {node:?}: wider needs the column of the {name}")))?;
                    index(t, &n, node)
                };
                let (names_c, values_c) = (col("names")?, col("values")?);
                let prefix: String = args.param("prefix", format!("{}_", t.columns[names_c]))?;
                let fill: f64 = args.opt_param::<f64>("fill")?.unwrap_or(f64::NAN);
                // the names in the order of their values, the rows in the order of their keys
                let mut levels: Vec<f64> = t.rows.iter().map(|r| r[names_c]).filter(|x| !x.is_nan()).collect();
                levels.sort_by(f64::total_cmp);
                levels.dedup();
                let mut keys: Vec<Vec<f64>> = t.rows.iter().map(|r| id.iter().map(|&j| r[j]).collect()).collect();
                keys.sort_by(|a, b| key_cmp(a, b));
                keys.dedup_by(|a, b| key_cmp(a, b) == std::cmp::Ordering::Equal);
                let mut head: Vec<String> = id.iter().map(|&j| t.columns[j].clone()).collect();
                head.extend(levels.iter().map(|v| format!("{prefix}{}", crate::value::class_label(*v))));
                let mut out = Table::new(head);
                for k in &keys {
                    let mut row = k.clone();
                    for lv in &levels {
                        let hit = t.rows.iter().find(|r| {
                            r[names_c] == *lv
                                && key_cmp(&id.iter().map(|&j| r[j]).collect::<Vec<_>>(), k)
                                    == std::cmp::Ordering::Equal
                        });
                        row.push(hit.map_or(fill, |r| r[values_c]));
                    }
                    out.rows.push(row);
                }
                out
            }
            "longer" => {
                let mut cols: Vec<usize> =
                    names(args, "columns")?.iter().map(|c| index(t, c, node)).collect::<Result<_>>()?;
                if cols.is_empty() {
                    cols = (0..t.columns.len()).filter(|j| !id.contains(j)).collect();
                }
                let prefix: String = args.param("prefix", String::new())?;
                let names_to: String = args.param("names_to", "name".to_string())?;
                let values_to: String = args.param("values_to", "value".to_string())?;
                // the number in each name, when every one has one (after a prefix common to them), else its place
                let common =
                    if prefix.is_empty() { common_prefix(cols.iter().map(|&j| t.columns[j].as_str())) } else { prefix };
                let parsed: Option<Vec<f64>> = cols.iter().map(|&j| number_in(&t.columns[j], &common)).collect();
                let labels: Vec<f64> = parsed.unwrap_or_else(|| (1..=cols.len()).map(|k| k as f64).collect());
                let mut head: Vec<String> = id.iter().map(|&j| t.columns[j].clone()).collect();
                head.push(names_to);
                head.push(values_to);
                let mut out = Table::new(head);
                for r in &t.rows {
                    for (&j, label) in cols.iter().zip(&labels) {
                        let mut row: Vec<f64> = id.iter().map(|&i| r[i]).collect();
                        row.push(*label);
                        row.push(r[j]);
                        out.rows.push(row);
                    }
                }
                out
            }
            other => return Err(Error::msg(format!("node {node:?}: the direction is wider or longer, not {other:?}"))),
        };
        Ok(outputs([("table", Value::Table(out))]))
    }
}

/// The longest start the names share, cut after its last letter or `_` (the prefix before their numbers: `to_` of
/// to_1, to_12).
fn common_prefix<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let names: Vec<&str> = names.collect();
    let Some(first) = names.first() else { return String::new() };
    let mut n = first.len();
    for x in &names[1..] {
        n = n.min(first.bytes().zip(x.bytes()).take_while(|(a, b)| a == b).count());
    }
    let p = &first[..n];
    let cut = p.rfind(|c: char| !c.is_ascii_digit() && c != '.' && c != '-').map_or(0, |k| k + 1);
    p[..cut].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(columns: &[&str], rows: &[&[f64]]) -> Table {
        let mut t = Table::new(columns.iter().map(|c| c.to_string()).collect());
        t.rows = rows.iter().map(|r| r.to_vec()).collect();
        t
    }

    fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Result<Table> {
        let r = Registry::standard();
        let params = params.as_object().cloned().unwrap_or_default();
        let args =
            Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
        match r.get(op).unwrap().run(&mut Ctx::default(), &args)?.remove("table") {
            Some(Value::Table(t)) => Ok(t),
            _ => panic!("no table"),
        }
    }

    fn patches() -> Table {
        t(
            &["class", "area", "edge"],
            &[&[1.0, 12.0, 4.0], &[3.0, 20.0, 9.0], &[3.0, 5.0, 2.0], &[1.0, f64::NAN, 3.0], &[3.0, 40.0, 11.0]],
        )
    }

    #[test]
    fn filter_and_calc_by_expressions() {
        let out = run(
            "table.filter",
            vec![("table", Value::Table(patches()))],
            serde_json::json!({ "expression": "class == 3 and area > 10" }),
        )
        .unwrap();
        assert_eq!(out.rows, vec![vec![3.0, 20.0, 9.0], vec![3.0, 40.0, 11.0]]);
        // a number input; a missing value left out
        let out = run(
            "table.filter",
            vec![("table", Value::Table(patches())), ("least", Value::Number(10.0))],
            serde_json::json!({ "expression": "area >= least" }),
        )
        .unwrap();
        assert_eq!(out.rows.len(), 3);
        let out = run(
            "table.calc",
            vec![("table", Value::Table(patches()))],
            serde_json::json!({ "column": "ratio", "expression": "edge / area" }),
        )
        .unwrap();
        assert_eq!(out.columns, ["class", "area", "edge", "ratio"]);
        assert_eq!(out.rows[0][3], 4.0 / 12.0);
        assert!(out.rows[3][3].is_nan());
        let out = run(
            "table.calc",
            vec![("table", Value::Table(patches()))],
            serde_json::json!({ "column": "area", "expression": "coalesce(area, 0) * 2" }),
        )
        .unwrap();
        assert_eq!(out.columns.len(), 3, "replaced");
        assert_eq!(out.rows[3][1], 0.0);
        assert!(
            run(
                "table.filter",
                vec![("table", Value::Table(patches()))],
                serde_json::json!({ "expression": "nope > 1" })
            )
            .unwrap_err()
            .0
            .contains("neither a column")
        );
        assert!(
            run(
                "table.filter",
                vec![("table", Value::Table(patches()))],
                serde_json::json!({ "expression": "nbsum(area, 1) > 1" })
            )
            .is_err()
        );
    }

    #[test]
    fn aggregate_by_groups() {
        let out = run(
            "table.aggregate",
            vec![("table", Value::Table(patches()))],
            serde_json::json!({ "by": "class", "columns": ["area"], "stats": "sum, mean, count, min, max, sd, median" }),
        )
        .unwrap();
        assert_eq!(
            out.columns,
            ["class", "n", "area_sum", "area_mean", "area_count", "area_min", "area_max", "area_sd", "area_median"]
        );
        // class 1: 12 and a missing value; class 3: 20, 5, 40
        assert_eq!(out.rows[0][..7], [1.0, 2.0, 12.0, 12.0, 1.0, 12.0, 12.0]);
        assert!(out.rows[0][7].is_nan(), "one value: no sd");
        assert_eq!(out.rows[0][8], 12.0);
        assert_eq!(out.rows[1][..7], [3.0, 3.0, 65.0, 65.0 / 3.0, 3.0, 5.0, 40.0]);
        let m = 65.0 / 3.0;
        let sd = (((20.0 - m) * (20.0 - m) + (5.0 - m) * (5.0 - m) + (40.0 - m) * (40.0 - m)) / 2.0f64).sqrt();
        assert!((out.rows[1][7] - sd).abs() < 1e-12);
        assert_eq!(out.rows[1][8], 20.0);
        // no groups: the whole table
        let out =
            run("table.aggregate", vec![("table", Value::Table(patches()))], serde_json::json!({ "stats": ["count"] }))
                .unwrap();
        assert_eq!(out.columns, ["n", "class_count", "area_count", "edge_count"]);
        assert_eq!(out.rows, vec![vec![5.0, 5.0, 4.0, 5.0]]);
        assert!(
            run("table.aggregate", vec![("table", Value::Table(patches()))], serde_json::json!({ "stats": "mode" }))
                .is_err()
        );
    }

    #[test]
    fn joins_left_inner_and_full() {
        let names = t(&["class", "cost"], &[&[1.0, 100.0], &[2.0, 50.0], &[3.0, 10.0]]);
        let left = t(&["class", "area"], &[&[3.0, 20.0], &[4.0, 7.0], &[1.0, 12.0]]);
        let j = |how: &str| {
            run(
                "table.join",
                vec![("left", Value::Table(left.clone())), ("right", Value::Table(names.clone()))],
                serde_json::json!({ "on": "class", "how": how }),
            )
            .unwrap()
        };
        let l = j("left");
        assert_eq!(l.columns, ["class", "area", "cost"]);
        assert_eq!(l.rows[0], [3.0, 20.0, 10.0]);
        assert!(l.rows[1][2].is_nan());
        assert_eq!(l.rows.len(), 3);
        assert_eq!(j("inner").rows.len(), 2);
        let f = j("full");
        assert_eq!(f.rows.len(), 4);
        assert_eq!(f.rows[3][0], 2.0, "the right's row without a match");
        assert!(f.rows[3][1].is_nan());
        // keys named otherwise; a name both have takes the suffix
        let other = t(&["id", "area"], &[&[1.0, 99.0]]);
        let o = run(
            "table.join",
            vec![("left", Value::Table(left)), ("right", Value::Table(other))],
            serde_json::json!({ "on": "class", "right_on": "id", "how": "inner" }),
        )
        .unwrap();
        assert_eq!(o.columns, ["class", "area", "area_right"]);
        assert_eq!(o.rows, vec![vec![1.0, 12.0, 99.0]]);
    }

    #[test]
    fn sort_select_and_pivot() {
        let s = run(
            "table.sort",
            vec![("table", Value::Table(patches()))],
            serde_json::json!({ "by": "area", "descending": true, "top": 3 }),
        )
        .unwrap();
        let areas: Vec<f64> = s.rows.iter().map(|r| r[1]).collect();
        assert_eq!(areas, [40.0, 20.0, 12.0]);
        let s =
            run("table.sort", vec![("table", Value::Table(patches()))], serde_json::json!({ "by": ["class", "edge"] }))
                .unwrap();
        assert_eq!(s.rows.iter().map(|r| r[2]).collect::<Vec<_>>(), [3.0, 4.0, 2.0, 9.0, 11.0]);
        let sel = run(
            "table.select",
            vec![("table", Value::Table(patches()))],
            serde_json::json!({ "columns": ["edge", "class"], "rename": { "edge": "perimeter" } }),
        )
        .unwrap();
        assert_eq!(sel.columns, ["perimeter", "class"]);
        assert_eq!(sel.rows[0], [4.0, 1.0]);
        let sel = run("table.select", vec![("table", Value::Table(patches()))], serde_json::json!({ "drop": "edge" }))
            .unwrap();
        assert_eq!(sel.columns, ["class", "area"]);
        // transitions into a matrix and back
        let tr = t(
            &["from", "to", "cells"],
            &[&[1.0, 1.0, 50.0], &[1.0, 2.0, 5.0], &[2.0, 2.0, 30.0], &[2.0, 1.0, 1.0], &[3.0, 3.0, 9.0]],
        );
        let w = run(
            "table.pivot",
            vec![("table", Value::Table(tr))],
            serde_json::json!({ "direction": "wider", "id": "from", "names": "to", "values": "cells", "fill": 0 }),
        )
        .unwrap();
        assert_eq!(w.columns, ["from", "to_1", "to_2", "to_3"]);
        assert_eq!(w.rows, vec![vec![1.0, 50.0, 5.0, 0.0], vec![2.0, 1.0, 30.0, 0.0], vec![3.0, 0.0, 0.0, 9.0]]);
        let l = run(
            "table.pivot",
            vec![("table", Value::Table(w))],
            serde_json::json!({ "direction": "longer", "id": "from", "names_to": "to", "values_to": "cells" }),
        )
        .unwrap();
        assert_eq!(l.columns, ["from", "to", "cells"]);
        assert_eq!(l.rows.len(), 9);
        assert_eq!(l.rows[1], [1.0, 2.0, 5.0], "the number in to_2");
        assert_eq!(common_prefix(["y1997", "y2011"].into_iter()), "y");
        assert_eq!(common_prefix(["to_1", "to_12"].into_iter()), "to_");
    }
}
