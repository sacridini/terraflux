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
    r.add(Arc::new(Assert));
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
        .param(PortSpec::optional("expression", Type::Text, "e.g. area / 10000, 0.3 if year < 2010 else 0.5"))
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
                Scope { width: 1, height: 1, maps: BTreeMap::new(), numbers: BTreeMap::new(), stacks: BTreeMap::new() };
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
                scope.stacks.entry(c.0).or_default().push((crate::Key::Name(c.1.to_string()), v.as_slice()));
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

struct Assert;

impl Operator for Assert {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "check.assert",
            "A check that stops the run when it fails, as a test does: `expression` over the node's inputs in map \
             algebra's syntax (the demand met, the areas summing to the total, the probabilities between 0 and 1); \
             with maps among them every cell with a value must pass, else the value must be true (not 0). It fails \
             with its message and what failed (how many cells, the value); with `warn`, it only warns.",
            Access::Global,
        )
        .extra(Type::Any)
        .output(PortSpec::new("ok", Type::Bool, "whether it passed (with warn; else it passed)"))
        .param(PortSpec::new("expression", Type::Text, "what must be true, e.g. abs(allocated - demand) < 1"))
        .param(PortSpec::optional("message", Type::Text, "what it means when it fails"))
        .param(PortSpec::optional("warn", Type::Bool, "only warn when it fails, the run going on (false)"))
        .param(PortSpec::optional("row", Type::Integer, "the row of the tables read, without maps (1)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let expression: String = args.param("expression", String::new())?;
        if expression.trim().is_empty() {
            return Err(Error::msg(format!("node {node:?}: give the expression that must be true")));
        }
        let maps = args.inputs.values().any(|v| matches!(v, Value::Map(_) | Value::Stack(_)));
        let (passed, detail) = if maps {
            let out = crate::algebra::Calc.run(ctx, args)?;
            let Some(Value::Map(m)) = out.get("map") else { return Err(Error::msg(format!("node {node:?}: no map"))) };
            let known: Vec<f64> = m.data.iter().copied().filter(|x| !x.is_nan()).collect();
            let failed = known.iter().filter(|x| **x == 0.0).count();
            (failed == 0, format!("{failed} of {} cells fail", known.len()))
        } else {
            let out = Calc.run(ctx, args)?;
            let x = out.get("value").and_then(Value::as_number).unwrap_or(f64::NAN);
            (x != 0.0 && !x.is_nan(), format!("it is {}", number_text(x)))
        };
        if !passed {
            let message: String = args.param("message", "the check failed".to_string())?;
            let text = format!("{message} ({expression}: {detail})");
            if !args.param("warn", false)? {
                return Err(Error::msg(format!("node {node:?}: {text}")));
            }
            ctx.warn(text);
        }
        let mut out = Outputs::new();
        out.insert("ok".into(), Value::Bool(passed));
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
            ..Default::default()
        };
        let out = run(
            vec![("t", Value::Table(t.clone())), ("k", Value::Integer(3)), ("on", Value::Bool(true))],
            serde_json::json!({ "expression": "t.area * k + max(1, 2) if on else 0" }),
        )
        .unwrap();
        assert!(matches!(out["value"], Value::Number(x) if x == 14.0));
        let out = run(vec![("t", Value::Table(t))], serde_json::json!({ "expression": "t.year", "row": -1 })).unwrap();
        assert!(matches!(out["value"], Value::Number(x) if x == 2010.0));
        assert!(run(vec![], serde_json::json!({ "expression": "nbsum(x, 1)" })).is_err());
        assert!(run(vec![], serde_json::json!({ "expression": "nothing + 1" })).is_err());
    }

    #[test]
    fn a_check_passes_fails_or_warns() {
        let check = |inputs: Vec<(&str, Value)>, params: serde_json::Value| {
            let params = params.as_object().cloned().unwrap_or_default();
            let args = Args {
                node: "c",
                inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
                params: &params,
            };
            let mut ctx = Ctx::default();
            let out = Assert.run(&mut ctx, &args);
            (out, ctx.warnings)
        };
        let (out, _) = check(vec![("a", Value::Number(3.0))], serde_json::json!({ "expression": "a > 2" }));
        assert!(matches!(out.unwrap()["ok"], Value::Bool(true)));
        let (out, _) =
            check(vec![("a", Value::Number(1.0))], serde_json::json!({ "expression": "a > 2", "message": "too few" }));
        let e = out.err().unwrap().to_string();
        assert!(e.contains("too few (a > 2: it is 0)"), "{e}");
        // a map: every cell with a value
        let m = Value::Map(crate::Map::new(Arc::new(terraflux_raster::Grid::new(3, 1)), vec![0.2, 1.5, f64::NAN]));
        let (out, warned) =
            check(vec![("p", m)], serde_json::json!({ "expression": "p >= 0 and p <= 1", "warn": true }));
        assert!(matches!(out.unwrap()["ok"], Value::Bool(false)));
        assert!(warned[0].contains("1 of 2 cells fail"), "{warned:?}");
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
