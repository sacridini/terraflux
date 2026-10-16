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
                    .map(|(c, v)| {
                        (c.clone(), serde_json::Number::from_f64(*v).map_or(serde_json::Value::Null, Into::into))
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
        out.push_str(&r.iter().map(|v| number(*v, comma)).collect::<Vec<_>>().join(&s));
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

/// The fields of a line of separated values: quoted ones (`"a, b"`, a quote doubled within) as they are.
fn fields(line: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => out.push(std::mem::take(&mut field)),
            c => field.push(c),
        }
    }
    out.push(field);
    out.into_iter().map(|f| f.trim().to_string()).collect()
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
/// found: with a separator other than the comma, values such as 12,5). The columns that are text (no field a
/// number) are left out, and named.
pub fn table_of_text(text: &str, json: bool, sep: Option<char>, comma: Option<bool>) -> Result<(Table, Vec<String>)> {
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
        let numeric: Vec<bool> =
            columns.iter().map(|c| rows.iter().any(|r| r.get(c).is_some_and(|v| v.is_number()))).collect();
        let mut t = Table::new(columns.iter().zip(&numeric).filter(|(_, n)| **n).map(|(c, _)| c.clone()).collect());
        for r in &rows {
            t.rows.push(t.columns.iter().map(|c| r.get(c).and_then(|v| v.as_f64()).unwrap_or(f64::NAN)).collect());
        }
        let text_columns = columns.into_iter().zip(numeric).filter(|(_, n)| !n).map(|(c, _)| c).collect();
        return Ok((t, text_columns));
    }
    let text = text.trim_start_matches('\u{feff}');
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header = lines.next().ok_or_else(|| Error::msg("the file is empty"))?;
    let sep = sep.unwrap_or_else(|| separator(header));
    let columns = fields(header, sep);
    let rows: Vec<Vec<String>> = lines.map(|l| fields(l, sep)).collect();
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
    let numeric: Vec<bool> = (0..columns.len())
        .map(|j| rows.iter().any(|r| r.get(j).and_then(|f| parse(f, comma)).is_some_and(|v| !v.is_nan())))
        .collect();
    let keep: Vec<usize> = (0..columns.len()).filter(|&j| numeric[j]).collect();
    let mut t = Table::new(keep.iter().map(|&j| columns[j].clone()).collect());
    for r in &rows {
        t.rows.push(keep.iter().map(|&j| r.get(j).and_then(|f| parse(f, comma)).unwrap_or(f64::NAN)).collect());
    }
    let text_columns = (0..columns.len()).filter(|&j| !numeric[j]).map(|j| columns[j].clone()).collect();
    Ok((t, text_columns))
}

/// A table read from a file, by its extension (`.json`: JSON; else separated values).
pub fn read_table(path: &std::path::Path) -> Result<(Table, Vec<String>)> {
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
             JSON (.json, a list of rows). A header names the columns; an empty value, or NA, is none. The columns \
             of text (names, not numbers) are left out: a table is of numbers.",
            Access::None,
        )
        .output(PortSpec::new("table", Type::Table, "the table"))
        .param(PortSpec::new("path", Type::Text, "the file, relative to the model's folder"))
        .param(PortSpec::optional("separator", Type::Text, "between the values: , ; or tab (default: found)"))
        .param(PortSpec::optional("decimal", Type::Text, "the decimal mark: . or , (default: found)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
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
        let (t, text_columns) = table_of_text(&text, json, sep, comma)
            .map_err(|e| Error::msg(format!("node {:?}: {}: {e}", args.node, path.display())))?;
        if !text_columns.is_empty() {
            ctx.warn(format!(
                "node {:?}: the columns of text left out (a table is of numbers): {}",
                args.node,
                text_columns.join(", ")
            ));
        }
        Ok(outputs([("table", Value::Table(t))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_read_as_spreadsheets_write_them() {
        let (t, text) =
            table_of_text("name,year,\"area, ha\"\nforest,2000,12.5\npasture,2010,NA\n", false, None, None).unwrap();
        assert_eq!(t.columns, ["year", "area, ha"]);
        assert_eq!(text, ["name"]);
        assert_eq!(t.rows[0], [2000.0, 12.5]);
        assert!(t.rows[1][1].is_nan());
        // Excel where a comma marks the decimals: ; and ,
        let (t, _) = table_of_text("\u{feff}class;area\n1;1.234,5\n2;7,25\n", false, None, None).unwrap();
        assert_eq!(t.rows, [[1.0, 1234.5], [2.0, 7.25]]);
        let (t, _) = table_of_text("a\tb\n1\t2\n", false, None, None).unwrap();
        assert_eq!(t.rows, [[1.0, 2.0]]);
        // what table.write writes reads back the same
        let mut w = Table::new(vec!["x".into(), "y".into()]);
        w.rows = vec![vec![1.0, 0.1], vec![f64::NAN, 3.0]];
        for (sep, comma) in [(',', false), (';', true), ('\t', false)] {
            let (r, _) = table_of_text(&table_text(&w, "text", sep, comma), false, None, None).unwrap();
            assert_eq!(r.columns, w.columns);
            assert_eq!(r.rows[0], w.rows[0]);
            assert!(r.rows[1][0].is_nan() && r.rows[1][1] == 3.0);
        }
        let (r, _) = table_of_text(&table_text(&w, "json", ',', false), true, None, None).unwrap();
        assert_eq!(r.rows[0], w.rows[0]);
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
