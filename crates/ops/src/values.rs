//! Values that are not maps: a number from an expression over the node's inputs (map algebra's syntax and functions,
//! a table's row as `table.column`), and a text with the inputs in it (`out/{species}_{year}.tif`), for the settings
//! that read values, the paths written in a loop, the case of a switch.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::algebra::{Scope, parse};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Calc));
}

/// A number as a text: a whole number without its point, others as Rust writes them (their shortest form).
pub fn number_text(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 9e15 { format!("{}", x as i64) } else { x.to_string() }
}

/// A value of an input as a text in a template: a text as it is, numbers, true or false.
fn as_text(v: &Value) -> Option<String> {
    Some(match v {
        Value::Text(t) => t.clone(),
        Value::Number(x) => number_text(*x),
        Value::Integer(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Json(serde_json::Value::String(s)) => s.clone(),
        Value::Json(j @ (serde_json::Value::Number(_) | serde_json::Value::Bool(_))) => j.to_string(),
        _ => return None,
    })
}

/// A template's `{name}` replaced by the text of `name` (`{{` and `}}` are braces), or why not.
pub fn fill(template: &str, text_of: &dyn Fn(&str) -> Option<String>) -> std::result::Result<String, String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let c = rest.as_bytes()[i];
        if rest[i + 1..].starts_with(c as char) {
            out.push(c as char);
            rest = &rest[i + 2..];
            continue;
        }
        if c == b'}' {
            return Err(format!("a lone }} in {template:?} (}}}} for one)"));
        }
        let end = rest[i..].find('}').ok_or_else(|| format!("a {{ without its }} in {template:?}"))? + i;
        let name = rest[i + 1..end].trim();
        out.push_str(&text_of(name).ok_or_else(|| format!("{{{name}}}: no input {name:?} of a number or a text"))?);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

struct Calc;

impl Operator for Calc {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "value.calc",
            "A number or a text from the node's inputs, not a map: `expression` in map algebra's syntax and \
             functions over the inputs that are numbers (true and false as 1 and 0), a table's value as \
             table.column (of the row `row`); `text` a template of the inputs, {name} replaced by each one's value \
             ({value} by the expression's): the path a loop writes (out/{species}_{year}.tif), the number a setting \
             reads, the case of a switch.",
            Access::None,
        )
        .extra(Type::Any)
        .output(PortSpec::new("value", Type::Number, "the expression's value"))
        .output(PortSpec::new("text", Type::Text, "the text"))
        .param(PortSpec::optional("expression", Type::Text, "e.g. area / 10000, if year < 2010 then 0.3 else 0.5"))
        .param(PortSpec::optional("text", Type::Text, "e.g. out/{species}_{year}.tif"))
        .param(PortSpec::optional(
            "row",
            Type::Integer,
            "the row of the tables read (1 the first, -1 the last; 1)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let fail = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let expression: Option<String> = args.opt_param("expression")?;
        let template: Option<String> = args.opt_param("text")?;
        if expression.is_none() && template.is_none() {
            return Err(fail("give an expression, a text or both".into()));
        }
        let mut out = Outputs::new();
        let mut value = None;
        if let Some(text) = &expression {
            let e = parse(text).map_err(|x| fail(format!("the expression: {}", x.0)))?;
            if e.radius() > 0 {
                return Err(fail("a value has no neighbours: nbsum and the like are for maps".into()));
            }
            let row: i64 = args.param("row", 1)?;
            let mut scope =
                Scope { width: 1, height: 1, maps: BTreeMap::new(), numbers: BTreeMap::new(), layers: BTreeMap::new() };
            let mut cells: Vec<(&str, &str, f64)> = Vec::new();
            for (port, v) in &args.inputs {
                match v {
                    Value::Table(t) => {
                        let n = t.rows.len() as i64;
                        let k = if row < 0 { n + row } else { row - 1 };
                        if !(0..n).contains(&k) {
                            if e.names().contains(port) {
                                return Err(fail(format!("{port} has {n} rows, no row {row}")));
                            }
                            continue;
                        }
                        for (j, c) in t.columns.iter().enumerate() {
                            cells.push((port.as_str(), c.as_str(), t.rows[k as usize][j]));
                        }
                    }
                    Value::Bool(b) => {
                        scope.numbers.insert(port.as_str(), f64::from(u8::from(*b)));
                    }
                    v => {
                        if let Some(x) = v.as_number() {
                            scope.numbers.insert(port.as_str(), x);
                        }
                    }
                }
            }
            let one: Vec<[f64; 1]> = cells.iter().map(|c| [c.2]).collect();
            for (c, v) in cells.iter().zip(&one) {
                scope.layers.insert((c.0, c.1), v.as_slice());
            }
            for name in e.names() {
                let known = scope.numbers.contains_key(name.as_str()) || cells.iter().any(|c| c.0 == name);
                if !known {
                    return Err(fail(format!("the expression names {name:?}, not an input of a number or a table")));
                }
            }
            let x = e.evaluate(&scope).map_err(|x| fail(x.0))?[0];
            value = Some(x);
            out.insert("value".to_string(), Value::Number(x));
        }
        if let Some(t) = &template {
            let text_of = |name: &str| -> Option<String> {
                if name == "value" && value.is_some() {
                    return value.map(number_text);
                }
                args.inputs.get(name).and_then(as_text)
            };
            out.insert("text".to_string(), Value::Text(fill(t, &text_of).map_err(fail)?));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Result<Outputs> {
        let params = params.as_object().cloned().unwrap_or_default();
        let args =
            Args { node: "v", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
        Calc.run(&mut Ctx::default(), &args)
    }

    #[test]
    fn a_number_from_numbers_and_a_tables_row() {
        let t = crate::Table {
            columns: vec!["year".into(), "area".into()],
            rows: vec![vec![2000.0, 4.0], vec![2010.0, 6.0]],
        };
        let out = run(
            vec![("t", Value::Table(t.clone())), ("k", Value::Integer(3)), ("on", Value::Bool(true))],
            serde_json::json!({ "expression": "if on then t.area * k + max(1, 2) else 0" }),
        )
        .unwrap();
        assert!(matches!(out["value"], Value::Number(x) if x == 14.0));
        let out = run(vec![("t", Value::Table(t))], serde_json::json!({ "expression": "t.year", "row": -1 })).unwrap();
        assert!(matches!(out["value"], Value::Number(x) if x == 2010.0));
        assert!(run(vec![], serde_json::json!({ "expression": "nbsum(x, 1)" })).is_err());
        assert!(run(vec![], serde_json::json!({ "expression": "nothing + 1" })).is_err());
    }

    #[test]
    fn a_text_of_the_inputs() {
        let out = run(
            vec![
                ("species", Value::Text("Ara macao".into())),
                ("year", Value::Number(2020.0)),
                ("step", Value::Integer(3)),
            ],
            serde_json::json!({ "text": "out/{species}_{year}_{step}_{value}.tif {{x}}", "expression": "year / 8" }),
        )
        .unwrap();
        assert!(matches!(&out["text"], Value::Text(t) if t == "out/Ara macao_2020_3_252.5.tif {x}"));
        assert!(run(vec![], serde_json::json!({ "text": "{missing}" })).is_err());
        assert!(run(vec![], serde_json::json!({ "text": "a { b" })).is_err());
        assert!(run(vec![], serde_json::json!({})).is_err());
    }
}
