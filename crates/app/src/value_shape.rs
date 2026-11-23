//! What flows on a wire, in one short line: `Map 300 × 300 · EPSG:32651 · 30 m`, `Table · 12 rows × 5 columns`. The
//! tooltip of a port or a wire shows it. It reads only what a value already holds (sizes, counts, the grid), never
//! its cells: O(1), O(bands) at most, and O(features) for the kinds of a vector's geometries.

use terraflux_ops::Value;
use terraflux_raster::vector::Geometry;
use terraflux_raster::{DataType, Grid};

/// The longest piece of text or of a coordinate system's name shown.
const CUT: usize = 24;

/// A number as a person writes it: whole when it is, else four decimals at most, a power of ten when huge or tiny.
fn num(v: f64) -> String {
    if v.is_nan() {
        return "none".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "∞" } else { "-∞" }.into();
    }
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let a = v.abs();
    if !(1e-4..1e9).contains(&a) {
        return format!("{v:.3e}");
    }
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `1 row`, `12 rows`.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A text cut short, on one line.
fn short(t: &str, max: usize) -> String {
    let line: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > max {
        format!("{}…", line.chars().take(max).collect::<String>().trim_end())
    } else {
        line
    }
}

/// A cell type by numpy's name.
fn dtype_name(t: DataType) -> &'static str {
    match t {
        DataType::U8 => "uint8",
        DataType::I8 => "int8",
        DataType::U16 => "uint16",
        DataType::I16 => "int16",
        DataType::U32 => "uint32",
        DataType::I32 => "int32",
        DataType::U64 => "uint64",
        DataType::I64 => "int64",
        DataType::F32 => "float32",
        DataType::F64 => "float64",
    }
}

/// A coordinate system by its EPSG code, else its name cut short; none when the grid has none.
fn crs_part(crs: Option<&str>) -> Option<String> {
    let name = crate::prepare::crs_name(Some(crs?));
    Some(short(&name, CUT))
}

/// The size of a cell with its unit: metres or feet of a projected grid, degrees of a geographic one; the bare
/// number when the grid has no coordinate system (a default grid's cell of 1 says nothing, left out).
fn cell_part(g: &Grid) -> Option<String> {
    let (w, h) = g.resolution();
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
    let side = if (w - h).abs() <= 1e-9 * w.max(h) { num(w) } else { format!("{} × {}", num(w), num(h)) };
    if g.geo.is_geographic() == Some(true) {
        return Some(format!("{side}°"));
    }
    if g.geo.geokeys.is_none() && g.geo.wkt.is_none() {
        return (w != 1.0 || h != 1.0).then_some(side);
    }
    Some(format!("{side} {}", if g.geo.linear_unit() == 1.0 { "m" } else { "ft" }))
}

/// The pieces after the kind that a grid tells: its size, its coordinate system, its cell.
fn grid_parts(g: &Grid) -> Vec<String> {
    let mut parts = vec![format!("{} × {}", g.width, g.height)];
    parts.extend(crs_part(crate::prepare::crs_of(g).as_deref()));
    parts.extend(cell_part(g));
    parts
}

/// The pieces after the kind: joined with a thin dot.
fn line(kind: &str, parts: Vec<String>) -> String {
    let mut out = kind.to_string();
    for p in parts {
        out.push_str(" · ");
        out.push_str(&p);
    }
    out
}

/// What a value is, in one line for a tooltip: its kind first, then what it holds (a raster's size, cell type,
/// coordinate system and cell; a table's rows and columns; a number itself).
pub fn describe(v: &Value) -> String {
    match v {
        Value::Number(x) => line("Number", vec![num(*x)]),
        Value::Integer(x) => line("Number", vec![format!("{x}")]),
        Value::Bool(b) => line("Yes or no", vec![if *b { "yes" } else { "no" }.into()]),
        Value::Text(t) => line("Text", vec![format!("\"{}\"", short(t, CUT))]),
        Value::Json(j) => match j {
            serde_json::Value::Array(a) => line("List", vec![count(a.len(), "item", "items")]),
            serde_json::Value::Object(o) => line("Record", vec![count(o.len(), "field", "fields")]),
            serde_json::Value::Null => "Empty".into(),
            other => line("Value", vec![short(&other.to_string(), CUT)]),
        },
        Value::Map(m) => line("Map", grid_parts(&m.grid)),
        Value::Stack(s) => line(&format!("Stack of {}", count(s.layers.len(), "map", "maps")), grid_parts(&s.grid)),
        Value::Raster(r) => {
            let layer = r.layers.get(r.chosen);
            let mut parts = grid_parts(r.grid_of(r.chosen));
            parts.insert(1, dtype_name(r.dtype()).into());
            if let Some(nd) = layer.and_then(|l| l.nodata) {
                let t = num(nd);
                if t.chars().count() <= 10 {
                    parts.push(format!("nodata {t}"));
                }
            }
            if r.layers.len() > 1 && !r.picked {
                line(&format!("Stack of {}", count(r.layers.len(), "map", "maps")), parts)
            } else {
                line("Map", parts)
            }
        }
        Value::Series(items) => match items.first() {
            Some(Value::Map(m)) => {
                let mut parts = vec![count(items.len(), "map", "maps")];
                parts.extend(grid_parts(&m.grid));
                line("Series", parts)
            }
            _ => line("Series", vec![count(items.len(), "value", "values")]),
        },
        Value::Table(t) => line(
            "Table",
            vec![format!("{} × {}", count(t.rows.len(), "row", "rows"), count(t.columns.len(), "column", "columns"))],
        ),
        Value::Vector(f) => {
            let (mut polygons, mut lines, mut points) = (0usize, 0usize, 0usize);
            for g in &f.geometries {
                match g {
                    Geometry::Empty => {}
                    g if g.is_polygonal() => polygons += 1,
                    g if g.is_point() => points += 1,
                    _ => lines += 1,
                }
            }
            let kinds = [(polygons, "polygon", "polygons"), (lines, "line", "lines"), (points, "point", "points")];
            let present: Vec<_> = kinds.iter().filter(|k| k.0 > 0).collect();
            let what = match present.as_slice() {
                [k] => count(k.0, k.1, k.2),
                [] => count(f.len(), "feature", "features"),
                _ => format!("{} (mixed)", count(f.len(), "feature", "features")),
            };
            let mut parts = vec![what];
            parts.extend(f.crs.as_deref().and_then(|c| crs_part(Some(c))));
            line("Vector", parts)
        }
        Value::Agents(a) => line(
            "Agents",
            vec![format!(
                "{} × {}",
                count(a.len(), "agent", "agents"),
                count(a.columns.len(), "attribute", "attributes")
            )],
        ),
        Value::Objects(o) => {
            let mut parts = vec![count(o.count, "object", "objects")];
            parts.push(format!("{} × {}", o.grid.width, o.grid.height));
            line("Objects", parts)
        }
        Value::Markov(m) => line("Markov chain", vec![count(m.classes.len(), "class", "classes")]),
        Value::Demand(_) => "Demand · cells per class and step".into(),
        Value::Regional(_) => "Regional demand · one demand per region".into(),
        Value::Glm(_) => "Regressions · one per class".into(),
        Value::Woe(_) => "Weights of evidence".into(),
        Value::Patches(_) => "Patch settings".into(),
        Value::Rng(_) => "Random generator · R".into(),
        Value::NumpyRng(_) => "Random generator · NumPy".into(),
        Value::BaseData(_) => "Base data · groups × labels".into(),
        Value::Tree(t) => line("Tree", vec![count(t.parent.len(), "node", "nodes")]),
        Value::LabelMatrix(_) => "Label matrix".into(),
        Value::HabitatGraph(g) => line("Habitat graph", vec![count(g.ids.len(), "node", "nodes")]),
        Value::SdmModel(_) => "Distribution model · fitted".into(),
        Value::MlModel(_) => "Model · fitted".into(),
        Value::StepDistribution(_) => "Step distribution".into(),
        Value::SwatProject(_) => "SWAT+ project".into(),
        Value::SwatWatershed(_) => "SWAT+ watershed".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use terraflux_ops::{Map, Stack, Table};
    use terraflux_raster::vector::Vector;

    /// A grid in UTM 51N (EPSG:32651) of 30 m cells.
    fn utm(w: usize, h: usize) -> Arc<Grid> {
        let mut g = Grid::new(w, h);
        g.transform = [500000.0, 30.0, 0.0, 4000000.0, 0.0, -30.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32651]);
        Arc::new(g)
    }

    #[test]
    fn numbers_and_text() {
        assert_eq!(describe(&Value::Number(0.376)), "Number · 0.376");
        assert_eq!(describe(&Value::Number(3.0)), "Number · 3");
        assert_eq!(describe(&Value::Number(0.123456789)), "Number · 0.1235");
        assert_eq!(describe(&Value::Number(1.5e12)), "Number · 1500000000000");
        assert_eq!(describe(&Value::Number(2.5e-7)), "Number · 2.500e-7");
        assert_eq!(describe(&Value::Number(f64::NAN)), "Number · none");
        assert_eq!(describe(&Value::Integer(12)), "Number · 12");
        assert_eq!(describe(&Value::Bool(true)), "Yes or no · yes");
        assert_eq!(describe(&Value::Text("forest".into())), "Text · \"forest\"");
        let long = Value::Text("a very long sentence\nthat goes on and on and on".into());
        assert_eq!(describe(&long), "Text · \"a very long sentence tha…\"");
    }

    #[test]
    fn json_kinds() {
        assert_eq!(describe(&Value::Json(serde_json::json!([1, 2, 3]))), "List · 3 items");
        assert_eq!(describe(&Value::Json(serde_json::json!({"a": 1}))), "Record · 1 field");
        assert_eq!(describe(&Value::Json(serde_json::Value::Null)), "Empty");
        assert_eq!(describe(&Value::Json(serde_json::json!(true))), "Value · true");
    }

    #[test]
    fn maps_and_stacks() {
        let g = utm(300, 200);
        let m = Value::Map(Map::new(g.clone(), vec![0.0; 300 * 200]));
        assert_eq!(describe(&m), "Map · 300 × 200 · EPSG:32651 · 30 m");
        let layer = Arc::new(vec![0.0; 300 * 200]);
        let s = Value::Stack(Stack { grid: g.clone(), keys: vec![], layers: vec![layer; 13] });
        assert_eq!(describe(&s), "Stack of 13 maps · 300 × 200 · EPSG:32651 · 30 m");
        let series = Value::Series(vec![m.clone(), m]);
        assert_eq!(describe(&series), "Series · 2 maps · 300 × 200 · EPSG:32651 · 30 m");
    }

    #[test]
    fn grids_without_a_system() {
        let plain = Value::Map(Map::new(Arc::new(Grid::new(3, 2)), vec![0.0; 6]));
        assert_eq!(describe(&plain), "Map · 3 × 2");
        let mut g = Grid::new(3, 2);
        g.transform = [0.0, 25.0, 0.0, 0.0, 0.0, -25.0];
        assert_eq!(describe(&Value::Map(Map::new(Arc::new(g), vec![0.0; 6]))), "Map · 3 × 2 · 25");
        let mut g = Grid::new(3, 2);
        g.transform = [10.0, 0.0003, 0.0, 50.0, 0.0, -0.0003];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 2, 2048, 0, 1, 4326]);
        assert_eq!(describe(&Value::Map(Map::new(Arc::new(g), vec![0.0; 6]))), "Map · 3 × 2 · EPSG:4326 · 0.0003°");
    }

    #[test]
    fn tables_and_series() {
        let mut t = Table::new(vec!["a".into(), "b".into()]);
        t.rows.push(vec![1.0, 2.0]);
        assert_eq!(describe(&Value::Table(t.clone())), "Table · 1 row × 2 columns");
        t.rows.extend(vec![vec![3.0, 4.0]; 11]);
        assert_eq!(describe(&Value::Table(t)), "Table · 12 rows × 2 columns");
        assert_eq!(describe(&Value::Series(vec![Value::Number(1.0)])), "Series · 1 value");
        assert_eq!(describe(&Value::Series(vec![])), "Series · 0 values");
    }

    #[test]
    fn vectors() {
        let poly = Geometry::Polygon(vec![vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]]);
        let v = Vector { geometries: vec![poly.clone(); 340], attributes: vec![], crs: Some("EPSG:4326".into()) };
        assert_eq!(describe(&Value::Vector(Arc::new(v))), "Vector · 340 polygons · EPSG:4326");
        let mixed = Vector { geometries: vec![poly, Geometry::Point([0.0, 0.0])], attributes: vec![], crs: None };
        assert_eq!(describe(&Value::Vector(Arc::new(mixed))), "Vector · 2 features (mixed)");
        let one = Vector { geometries: vec![Geometry::Point([0.0, 0.0])], attributes: vec![], crs: None };
        assert_eq!(describe(&Value::Vector(Arc::new(one))), "Vector · 1 point");
    }

    #[test]
    fn agents_and_models() {
        let a = terraflux_ops::agents::Agents {
            columns: vec!["x".into(), "y".into()],
            values: vec![vec![1.0; 5], vec![2.0; 5]],
        };
        assert_eq!(describe(&Value::Agents(Arc::new(a))), "Agents · 5 agents × 2 attributes");
        let m = terraflux_ops::Markov { classes: vec![1.0, 2.0, 3.0], single: vec![], multi: vec![], steps: 1 };
        assert_eq!(describe(&Value::Markov(Arc::new(m))), "Markov chain · 3 classes");
    }
}
