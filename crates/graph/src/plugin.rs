//! The settings of a plugin node (`plugin.wasm`) that are checked before a run: its interface, window radius,
//! outputs and fuel. The operator reads them again when it runs, with the same rules.

use serde_json::{Map, Value};

use crate::{setting_ref, valid_port};

/// The largest radius of a `window` plugin (a window of 511 x 511 cells per input).
pub const MAX_RADIUS: u64 = 255;

/// The most instructions per cell a plugin may be given (`fuel`).
pub const MAX_FUEL: u64 = 100_000_000;

/// What is wrong in the settings of a plugin node: the setting and the problem. Settings that read a value
/// (`$name`, `{from = ...}`) are not known until the run, and are left alone.
pub fn plugin_problems(params: &Map<String, Value>) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    let known = |k: &str| params.get(k).filter(|v| setting_ref(v).is_none() && !v.is_null());
    let interface = match known("interface") {
        None => Some("cell"),
        Some(Value::String(s)) if matches!(s.as_str(), "cell" | "window" | "map") => Some(s.as_str()),
        Some(v) => {
            out.push(("interface", format!("the interface is cell, window or map, not {v}")));
            None
        }
    };
    if params.get("interface").is_some_and(|v| setting_ref(v).is_some()) {
        return out;
    }
    if let Some(v) = known("radius") {
        match v.as_u64() {
            Some(r) if (1..=MAX_RADIUS).contains(&r) => {}
            _ => out.push(("radius", format!("the radius is a whole number from 1 to {MAX_RADIUS}, not {v}"))),
        }
    } else if interface == Some("window") && !params.contains_key("radius") {
        out.push(("radius", "the window interface needs a radius (1 or more)".into()));
    }
    if let Some(v) = known("fuel") {
        match v.as_u64() {
            Some(f) if (1..=MAX_FUEL).contains(&f) => {}
            _ => out.push(("fuel", format!("the fuel (instructions per cell) is a whole number from 1 to {MAX_FUEL}"))),
        }
    }
    let mut names: Vec<String> = Vec::new();
    let mut name = |what: &'static str, n: &str, out: &mut Vec<(&'static str, String)>| {
        if !valid_port(n) {
            out.push((what, format!("{n:?} is not a valid name (letters, digits, _ and -)")));
        } else if names.iter().any(|x| x == n) {
            out.push((what, format!("two outputs are named {n:?}")));
        } else {
            names.push(n.to_string());
        }
    };
    let declares = known("outputs").is_some() || known("table_outputs").is_some();
    if declares && let Some(i) = interface.filter(|i| *i != "map") {
        out.push(("outputs", format!("outputs and table_outputs are for the map interface, not {i}")));
    }
    match known("outputs") {
        None => {}
        Some(Value::Array(a)) => {
            if a.is_empty() {
                out.push(("outputs", "outputs lists no name".into()));
            }
            for v in a {
                match v.as_str() {
                    Some(n) => name("outputs", n, &mut out),
                    None => out.push(("outputs", format!("an output name is a text, not {v}"))),
                }
            }
        }
        Some(v) => out.push(("outputs", format!("outputs is a list of names, not {v}"))),
    }
    match known("table_outputs") {
        None => {}
        Some(Value::Object(o)) => {
            for (n, cols) in o {
                name("table_outputs", n, &mut out);
                match cols {
                    Value::Array(c) if !c.is_empty() => {
                        let mut seen: Vec<&str> = Vec::new();
                        for v in c {
                            match v.as_str() {
                                Some(s) if !s.is_empty() && !seen.contains(&s) => seen.push(s),
                                Some(s) => out.push((
                                    "table_outputs",
                                    format!("the table {n:?} has an empty or repeated column name ({s:?})"),
                                )),
                                None => out.push((
                                    "table_outputs",
                                    format!("the table {n:?}: a column name is a text, not {v}"),
                                )),
                            }
                        }
                    }
                    _ => out.push(("table_outputs", format!("the table {n:?} is a list of column names"))),
                }
            }
        }
        Some(v) => out.push(("table_outputs", format!("table_outputs is {{name = [columns]}}, not {v}"))),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problems(v: serde_json::Value) -> Vec<&'static str> {
        plugin_problems(v.as_object().unwrap()).into_iter().map(|(k, _)| k).collect()
    }

    #[test]
    fn the_settings_of_a_plugin_are_checked() {
        assert!(problems(serde_json::json!({"path": "a.wat"})).is_empty());
        assert!(problems(serde_json::json!({"interface": "window", "radius": 2})).is_empty());
        assert!(problems(serde_json::json!({"interface": "map", "outputs": ["a", "b"], "fuel": 50})).is_empty());
        assert!(problems(serde_json::json!({"interface": "$mode", "radius": "$r"})).is_empty());
        assert_eq!(problems(serde_json::json!({"interface": "tiles"})), ["interface"]);
        assert_eq!(problems(serde_json::json!({"interface": "window", "radius": 0})), ["radius"]);
        assert_eq!(problems(serde_json::json!({"interface": "window"})), ["radius"]);
        assert_eq!(problems(serde_json::json!({"interface": "window", "radius": 1.5})), ["radius"]);
        assert_eq!(problems(serde_json::json!({"interface": "map", "outputs": ["a", "a"]})), ["outputs"]);
        assert_eq!(problems(serde_json::json!({"interface": "map", "outputs": []})), ["outputs"]);
        assert_eq!(
            problems(serde_json::json!({"interface": "map", "outputs": ["a"], "table_outputs": {"a": ["x"]}})),
            ["table_outputs"]
        );
        assert_eq!(
            problems(serde_json::json!({"interface": "map", "table_outputs": {"t": ["x", "x"]}})),
            ["table_outputs"]
        );
        assert_eq!(problems(serde_json::json!({"outputs": ["a"]})), ["outputs"]);
        assert_eq!(problems(serde_json::json!({"interface": "map", "fuel": 0})), ["fuel"]);
    }
}
