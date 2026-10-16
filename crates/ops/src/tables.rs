//! Writing a table (`table.write`): comma-separated values (or tab-separated, or JSON) by the file's extension, for
//! a spreadsheet, R or Python; the separator and the decimal mark as the spreadsheet of the place expects
//! (`;` and `,` where a comma marks the decimals).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
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

#[cfg(test)]
mod tests {
    use super::*;

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
