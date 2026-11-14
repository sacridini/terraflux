//! Tables from and to files (`table.read`, `table.write`): comma-separated values (or tab-separated, or JSON) by
//! the file's extension, from and for a spreadsheet, R or Python; the separator and the decimal mark as the
//! spreadsheet of the place writes them (`;` and `,` where a comma marks the decimals).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Read));
    r.add(Arc::new(Write));
}

/// A value as written: a whole number as it is, others in the shortest form that reads back the same, the
/// decimal mark asked; nothing for none.
fn number(v: f64, comma: bool) -> String {
    if v.is_nan() {
        return String::new();
    }
    let s = format!("{v}");
    if comma { s.replace('.', ",") } else { s }
}

/// A name quoted when it holds the separator, a quote or a line's end.
fn quoted(name: &str, sep: char) -> String {
    if name.contains(sep) || name.contains('"') || name.contains('\n') {
        format!("\"{}\"", name.replace('"', "\"\""))
    } else {
        name.to_string()
    }
}

/// The table as text: separated values (a header first) or JSON (a list of rows, each column's value by its
/// name, null for none).
pub fn table_text(t: &Table, format: &str, sep: char, comma: bool) -> String {
    if format == "json" {
        let rows: Vec<serde_json::Value> = t
            .rows
            .iter()
            .map(|r| {
                let o: serde_json::Map<String, serde_json::Value> = t
                    .columns
                    .iter()
                    .zip(r)
                    .map(|(c, v)| match t.label(c, *v) {
                        Some(name) => (c.clone(), name.into()),
                        None => {
                            (c.clone(), serde_json::Number::from_f64(*v).map_or(serde_json::Value::Null, Into::into))
                        }
                    })
                    .collect();
                serde_json::Value::Object(o)
            })
            .collect();
        return serde_json::to_string_pretty(&rows).unwrap_or_default() + "\n";
    }
    let s = sep.to_string();
    let mut out = t.columns.iter().map(|c| quoted(c, sep)).collect::<Vec<_>>().join(&s);
    out.push('\n');
    for r in &t.rows {
        let cells: Vec<String> = t
            .columns
            .iter()
            .zip(r)
            .map(|(c, v)| t.label(c, *v).map_or_else(|| number(*v, comma), |name| quoted(name, sep)))
            .collect();
        out.push_str(&cells.join(&s));
        out.push('\n');
    }
    out
}

struct Write;

impl Operator for Write {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "table.write",
            "Write a table to a file, its format by the extension: comma-separated values (.csv, the default), \
             tab-separated (.tsv, .txt) or JSON (.json, a list of rows); a value missing is left empty (null in \
             JSON). For a spreadsheet that reads a comma as the decimal mark: separator ; and decimal ,.",
            Access::None,
        )
        .input(PortSpec::new("table", Type::Table, "the table"))
        .output(PortSpec::new("path", Type::Text, "the file written"))
        .param(PortSpec::new("path", Type::Text, "the file, relative to the model's folder"))
        .param(PortSpec::optional("separator", Type::Text, "between the values: , ; or tab (default by the extension)"))
        .param(PortSpec::optional("decimal", Type::Text, "the decimal mark: . (default) or ,"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let t = match args.get("table") {
            Some(Value::Table(t)) => t,
            Some(_) => return Err(Error::msg(format!("node {:?}: the input is not a table", args.node))),
            None => return Err(Error::msg(format!("node {:?}: connect the table to write", args.node))),
        };
        let path: String = args.param("path", String::new())?;
        if path.trim().is_empty() {
            return Err(Error::msg(format!("node {:?}: choose the file", args.node)));
        }
        let mut path = std::path::PathBuf::from(path.trim());
        if path.extension().is_none() {
            path.set_extension("csv");
        }
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let format = if ext == "json" { "json" } else { "text" };
        let sep: Option<String> = args.opt_param("separator")?;
        let sep = match sep.as_deref().map(str::trim) {
            None | Some("") => {
                if ext == "tsv" || ext == "txt" {
                    '\t'
                } else {
                    ','
                }
            }
            Some("tab" | "\\t" | "\t") => '\t',
            Some(s) if s.chars().count() == 1 => s.chars().next().unwrap(),
            Some(s) => {
                return Err(Error::msg(format!("node {:?}: the separator {s:?} is not one character", args.node)));
            }
        };
        let decimal: String = args.param("decimal", ".".to_string())?;
        let comma = match decimal.trim() {
            "." | "" => false,
            "," => true,
            d => return Err(Error::msg(format!("node {:?}: the decimal mark {d:?} is neither . nor ,", args.node))),
        };
        if comma && sep == ',' {
            return Err(Error::msg(format!(
                "node {:?}: a comma cannot mark both the decimals and the columns (separator ;)",
                args.node
            )));
        }
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| Error::msg(format!("{}: {e}", dir.display())))?;
        }
        std::fs::write(&path, table_text(t, format, sep, comma))
            .map_err(|e| Error::msg(format!("{}: {e}", path.display())))?;
        Ok(outputs([("path", Value::Text(path.to_string_lossy().into_owned()))]))
    }
}

/// The records of separated values (RFC 4180): fields in quotes may hold the separator, line breaks and doubled
/// quotes; CRLF or LF end a record. A blank line is a record of one empty field. The reason when a quote never closes.
pub fn csv_records(text: &str, sep: char) -> std::result::Result<Vec<Vec<String>>, String> {
    let text = text.trim_start_matches('\u{feff}');
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut any = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                c => field.push(c),
            }
            continue;
        }
        match c {
            '"' => {
                quoted = true;
                any = true;
            }
            c if c == sep => {
                row.push(std::mem::take(&mut field));
                any = true;
            }
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                any = false;
            }
            c => {
                field.push(c);
                any = true;
            }
        }
    }
    if quoted {
        return Err("a quote is never closed".into());
    }
    if any {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

/// A text column as codes: 1, 2... by the order of first appearance of each text, NaN for none; and the texts.
pub(crate) fn coded(cells: impl Iterator<Item = Option<String>>) -> (Vec<f64>, Vec<String>) {
    let mut names: Vec<String> = Vec::new();
    let mut at: std::collections::HashMap<String, usize> = Default::default();
    let codes = cells
        .map(|c| match c {
            None => f64::NAN,
            Some(s) => {
                let k = *at.entry(s.clone()).or_insert_with(|| {
                    names.push(s);
                    names.len()
                });
                k as f64
            }
        })
        .collect();
    (codes, names)
}

/// A table of columns (each the numbers of its rows), the dictionaries of the text ones by column.
fn table_of_columns(columns: Vec<String>, cols: Vec<Vec<f64>>, labels: Vec<(String, Vec<String>)>) -> Table {
    let n = cols.first().map_or(0, Vec::len);
    let mut t = Table::new(columns);
    t.rows = (0..n).map(|i| cols.iter().map(|c| c[i]).collect()).collect();
    t.labels = labels.into_iter().collect();
    t
}

/// The separator of a file of values, by its first line: the one of `;`, tab and `,` it has most of.
fn separator(first: &str) -> char {
    [';', '\t', ','].into_iter().max_by_key(|&c| (first.matches(c).count(), c == ',')).unwrap_or(',')
}

/// A field as a number: none for an empty one or NA; `comma` the decimal mark (else the point); `None` for text.
fn parse(field: &str, comma: bool) -> Option<f64> {
    let f = field.trim();
    if f.is_empty() || ["na", "nan", "null", "none", "-"].contains(&f.to_lowercase().as_str()) {
        return Some(f64::NAN);
    }
    let f = if comma { f.replace('.', "").replace(',', ".") } else { f.to_string() };
    f.parse::<f64>().ok()
}

/// A table from the text of a file: separated values (`sep`, else found from the header; a header first) or JSON
/// (a list of rows, each a map of a column's name to its value); `comma` whether a comma marks the decimals (else
/// found: with a separator other than the comma, values such as 12,5). A column where every value is a number (or
/// none) is numbers; any other is text: its texts coded 1, 2... by first appearance, in [`Table::labels`] (numbers
/// among them as the text they are written as).
pub fn table_of_text(text: &str, json: bool, sep: Option<char>, comma: Option<bool>) -> Result<Table> {
    if json {
        let rows: Vec<serde_json::Map<String, serde_json::Value>> =
            serde_json::from_str(text).map_err(|e| Error::msg(format!("not a list of rows: {e}")))?;
        let mut columns: Vec<String> = Vec::new();
        for r in &rows {
            for k in r.keys() {
                if !columns.contains(k) {
                    columns.push(k.clone());
                }
            }
        }
        let mut cols = Vec::new();
        let mut labels = Vec::new();
        for c in &columns {
            let cells = rows.iter().map(|r| r.get(c).filter(|v| !v.is_null()));
            if cells.clone().flatten().all(|v| v.is_number()) {
                cols.push(cells.map(|v| v.and_then(|v| v.as_f64()).unwrap_or(f64::NAN)).collect());
            } else {
                let (codes, names) = coded(cells.map(|v| {
                    v.map(|v| match v {
                        serde_json::Value::String(s) => s.clone(),
                        v => v.to_string(),
                    })
                }));
                cols.push(codes);
                labels.push((c.clone(), names));
            }
        }
        return Ok(table_of_columns(columns, cols, labels));
    }
    let text = text.trim_start_matches('\u{feff}');
    let header = text.lines().find(|l| !l.trim().is_empty()).ok_or_else(|| Error::msg("the file is empty"))?;
    let sep = sep.unwrap_or_else(|| separator(header));
    let mut records = csv_records(text, sep)
        .map_err(Error::msg)?
        .into_iter()
        .filter(|r| !(r.len() == 1 && r[0].trim().is_empty()))
        .map(|r| r.into_iter().map(|f| f.trim().to_string()).collect::<Vec<String>>());
    let columns = records.next().ok_or_else(|| Error::msg("the file is empty"))?;
    let rows: Vec<Vec<String>> = records.collect();
    // a comma for the decimals: asked, or values of a comma between digits where the comma does not separate
    let comma = comma.unwrap_or_else(|| {
        sep != ','
            && rows.iter().flatten().any(|f| {
                let b = f.as_bytes();
                b.iter().enumerate().any(|(i, &c)| {
                    c == b',' && i > 0 && b[i - 1].is_ascii_digit() && b.get(i + 1).is_some_and(u8::is_ascii_digit)
                })
            })
    });
    let mut cols = Vec::new();
    let mut labels = Vec::new();
    for (j, c) in columns.iter().enumerate() {
        fn field(r: &[String], j: usize) -> &str {
            r.get(j).map(String::as_str).unwrap_or("")
        }
        if rows.iter().all(|r| parse(field(r, j), comma).is_some()) {
            cols.push(rows.iter().map(|r| parse(field(r, j), comma).unwrap_or(f64::NAN)).collect());
        } else {
            // (the empty value and NA are none here too)
            let (codes, names) =
                coded(rows.iter().map(|r| {
                    Some(field(r, j)).filter(|f| !parse(f, comma).is_some_and(f64::is_nan)).map(String::from)
                }));
            cols.push(codes);
            labels.push((c.clone(), names));
        }
    }
    Ok(table_of_columns(columns, cols, labels))
}

/// A table read from a file, by its extension (`.json`: JSON; else separated values).
pub fn read_table(path: &std::path::Path) -> Result<Table> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::msg(format!("{}: {e}", path.display())))?;
    let json = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"));
    table_of_text(&text, json, None, None).map_err(|e| Error::msg(format!("{}: {e}", path.display())))
}

struct Read;

impl Operator for Read {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "table.read",
            "A table from a file: comma-separated values (.csv), tab-separated (.tsv, .txt), with ; as Excel writes \
             them where a comma marks the decimals (the separator and the decimal mark found by themselves), or \
             JSON (.json, a list of rows). A header names the columns; an empty value, or NA, is none. A column \
             with a value that is not a number is text: kept as names (coded 1, 2... by first appearance, with \
             the names beside the codes).",
            Access::None,
        )
        .output(PortSpec::new("table", Type::Table, "the table"))
        .param(PortSpec::new("path", Type::Text, "the file, relative to the model's folder"))
        .param(PortSpec::optional("separator", Type::Text, "between the values: , ; or tab (default: found)"))
        .param(PortSpec::optional("decimal", Type::Text, "the decimal mark: . or , (default: found)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let path: String = args.param("path", String::new())?;
        if path.trim().is_empty() {
            return Err(Error::msg(format!("node {:?}: choose the file", args.node)));
        }
        let path = std::path::PathBuf::from(path.trim());
        let text = std::fs::read_to_string(&path).map_err(|e| Error::msg(format!("{}: {e}", path.display())))?;
        let json = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"));
        let sep: Option<String> = args.opt_param("separator")?;
        let sep = match sep.as_deref().map(str::trim) {
            None | Some("") => None,
            Some("tab" | "\\t" | "\t") => Some('\t'),
            Some(s) if s.chars().count() == 1 => s.chars().next(),
            Some(s) => {
                return Err(Error::msg(format!("node {:?}: the separator {s:?} is not one character", args.node)));
            }
        };
        let decimal: Option<String> = args.opt_param("decimal")?;
        let comma = match decimal.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(".") => Some(false),
            Some(",") => Some(true),
            Some(d) => {
                return Err(Error::msg(format!("node {:?}: the decimal mark {d:?} is neither . nor ,", args.node)));
            }
        };
        let t = table_of_text(&text, json, sep, comma)
            .map_err(|e| Error::msg(format!("node {:?}: {}: {e}", args.node, path.display())))?;
        Ok(outputs([("table", Value::Table(t))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_read_as_spreadsheets_write_them() {
        let t =
            table_of_text("name,year,\"area, ha\"\nforest,2000,12.5\npasture,2010,NA\n", false, None, None).unwrap();
        assert_eq!(t.columns, ["name", "year", "area, ha"]);
        assert!(t.is_text("name") && !t.is_text("year"));
        assert_eq!(t.labels["name"], ["forest", "pasture"]);
        assert_eq!(t.rows[0], [1.0, 2000.0, 12.5]);
        assert!(t.rows[1][2].is_nan() && t.rows[1][0] == 2.0);
        // Excel where a comma marks the decimals: ; and ,
        let t = table_of_text("\u{feff}class;area\n1;1.234,5\n2;7,25\n", false, None, None).unwrap();
        assert_eq!(t.rows, [[1.0, 1234.5], [2.0, 7.25]]);
        let t = table_of_text("a\tb\n1\t2\n", false, None, None).unwrap();
        assert_eq!(t.rows, [[1.0, 2.0]]);
        // what table.write writes reads back the same
        let mut w = Table::new(vec!["x".into(), "y".into()]);
        w.rows = vec![vec![1.0, 0.1], vec![f64::NAN, 3.0]];
        for (sep, comma) in [(',', false), (';', true), ('\t', false)] {
            let r = table_of_text(&table_text(&w, "text", sep, comma), false, None, None).unwrap();
            assert_eq!(r.columns, w.columns);
            assert_eq!(r.rows[0], w.rows[0]);
            assert!(r.rows[1][0].is_nan() && r.rows[1][1] == 3.0);
        }
        let r = table_of_text(&table_text(&w, "json", ',', false), true, None, None).unwrap();
        assert_eq!(r.rows[0], w.rows[0]);
    }

    /// A table with text (names in the order of first appearance, one with a comma, a quote, a line break, one that
    /// looks like a number among others) written and read back is the same, in every format.
    #[test]
    fn text_columns_are_kept_and_round_trip() {
        let mut w = Table::new(vec!["cover, type".into(), "ha".into(), "n".into()]);
        w.labels.insert(
            "cover, type".into(),
            vec!["forest, dense".into(), "say \"hi\"".into(), "two\nlines".into(), "2000".into()],
        );
        w.rows = vec![
            vec![1.0, 1.5, 1.0],
            vec![2.0, f64::NAN, 2.0],
            vec![f64::NAN, 3.0, 3.0],
            vec![3.0, 4.0, 4.0],
            vec![1.0, 5.0, 5.0],
            vec![4.0, 6.0, 6.0],
        ];
        let same = |r: &Table| {
            assert_eq!(r.columns, w.columns);
            assert_eq!(r.labels, w.labels);
            assert_eq!(r.rows.len(), w.rows.len());
            for (a, b) in r.rows.iter().flatten().zip(w.rows.iter().flatten()) {
                assert!(a == b || (a.is_nan() && b.is_nan()), "{a} {b}");
            }
        };
        for (sep, comma) in [(',', false), ('\t', false), (';', true)] {
            same(&table_of_text(&table_text(&w, "text", sep, comma), false, None, None).unwrap());
        }
        same(&table_of_text(&table_text(&w, "json", ',', false), true, None, None).unwrap());
    }

    /// A column of numbers and texts is all text (the numbers as written); JSON too; a column of none is numbers.
    #[test]
    fn a_mixed_column_is_text() {
        let t = table_of_text("a,b,c\n12,1,\nx,2,\n12,3,\n", false, None, None).unwrap();
        assert_eq!(t.labels["a"], ["12", "x"]);
        assert_eq!(t.column("a").unwrap(), [1.0, 2.0, 1.0]);
        assert!(!t.is_text("b") && !t.is_text("c") && t.rows[0][2].is_nan());
        let t = table_of_text(r#"[{"a": 12, "b": 1}, {"a": "x", "b": 2.5}, {"b": null}]"#, true, None, None).unwrap();
        assert_eq!(t.labels["a"], ["12", "x"]);
        assert_eq!(t.rows[0][0], 1.0);
        assert!(t.rows[2][0].is_nan() && t.rows[2][1].is_nan() && !t.is_text("b"));
    }

    fn table() -> Table {
        let mut t = Table::new(vec!["class".into(), "area, ha".into()]);
        t.rows.push(vec![1.0, 12.5]);
        t.rows.push(vec![2.0, f64::NAN]);
        t
    }

    #[test]
    fn tables_are_written_as_spreadsheets_read_them() {
        assert_eq!(table_text(&table(), "text", ',', false), "class,\"area, ha\"\n1,12.5\n2,\n");
        assert_eq!(table_text(&table(), "text", ';', true), "class;area, ha\n1;12,5\n2;\n");
        assert_eq!(table_text(&table(), "text", '\t', false), "class\tarea, ha\n1\t12.5\n2\t\n");
        let j: serde_json::Value = serde_json::from_str(&table_text(&table(), "json", ',', false)).unwrap();
        assert_eq!(j[0]["area, ha"], 12.5);
        assert!(j[1]["area, ha"].is_null());
    }
}
