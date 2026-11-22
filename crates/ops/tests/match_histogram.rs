//! `raster.match_histogram` against scikit-image's `exposure.match_histograms`
//! (tests/data/reference/match_histogram.json, tools/make_match_histogram_reference.py).

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Map, Outputs, Registry, Stack, Value};
use terraflux_raster::Grid;

fn run(inputs: Vec<(&str, Value)>) -> terraflux_ops::Result<Outputs> {
    let r = Registry::standard();
    let params = serde_json::Map::new();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get("raster.match_histogram").unwrap().run(&mut Ctx::default(), &args)
}

fn reference() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/match_histogram.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The bands of a nested list (rows, columns, bands) as a stack named b0, b1, b2.
fn stack(v: &serde_json::Value) -> Value {
    let rows = v.as_array().unwrap();
    let (h, w, k) = (rows.len(), rows[0].as_array().unwrap().len(), 3);
    let mut layers = vec![Vec::with_capacity(h * w); k];
    for row in rows {
        for cell in row.as_array().unwrap() {
            for (b, x) in cell.as_array().unwrap().iter().enumerate() {
                layers[b].push(x.as_f64().unwrap());
            }
        }
    }
    Value::Stack(Stack {
        grid: Arc::new(Grid::new(w, h)),
        keys: (0..k).map(|b| Key::Name(format!("b{b}"))).collect(),
        layers: layers.into_iter().map(Arc::new).collect(),
    })
}

fn bands(v: &serde_json::Value) -> Vec<Vec<f64>> {
    let Value::Stack(s) = stack(v) else { unreachable!() };
    s.layers.iter().map(|l| l.to_vec()).collect()
}

fn close(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}");
    for (i, (a, b)) in got.iter().zip(want).enumerate() {
        assert!((a - b).abs() <= 1e-12 * b.abs().max(1.0), "{what}: cell {i}: {a} against {b}");
    }
}

#[test]
fn match_histogram_is_scikit_images() {
    let z = reference();
    let out = run(vec![("raster", stack(&z["source"])), ("reference", stack(&z["reference"]))]).unwrap();
    let Value::Stack(got) = &out["raster"] else { panic!("a stack") };
    assert_eq!(got.keys.iter().map(|k| k.to_string()).collect::<Vec<_>>(), ["b0", "b1", "b2"]);
    for (b, want) in bands(&z["matched"]).iter().enumerate() {
        close(&format!("band {b}"), &got.layers[b], want);
    }
}

#[test]
fn a_mask_makes_the_distributions_of_the_cells_inside() {
    let z = reference();
    let mask: Vec<f64> = z["mask"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()))
        .collect();
    let grid = Arc::new(Grid::new(50, 40));
    let out = run(vec![
        ("raster", stack(&z["source"])),
        ("reference", stack(&z["reference"])),
        ("mask", Value::Map(Map::new(grid, mask))),
    ])
    .unwrap();
    let Value::Stack(got) = &out["raster"] else { panic!("a stack") };
    for (b, want) in bands(&z["masked"]).iter().enumerate() {
        close(&format!("masked band {b}"), &got.layers[b], want);
    }
}

#[test]
fn a_map_with_missing_cells_and_the_refusals() {
    let grid = Arc::new(Grid::new(2, 2));
    let source = Value::Map(Map::new(grid.clone(), vec![1.0, f64::NAN, 3.0, 2.0]));
    let reference = Value::Map(Map::new(grid.clone(), vec![10.0, 20.0, 30.0, 40.0]));
    let out = run(vec![("raster", source.clone()), ("reference", reference)]).unwrap();
    let Value::Map(m) = &out["raster"] else { panic!("a map") };
    assert!(m.data[1].is_nan(), "a missing cell stays missing");
    // 1, 2 and 3 have the shares 1/3, 2/3 and 1; the reference's 10, 20, 30 and 40 have 1/4, 1/2, 3/4 and 1
    close(
        "map",
        &[m.data[0], m.data[3], m.data[2]],
        &[10.0 + (1.0 / 3.0 - 0.25) / 0.25 * 10.0, 20.0 + (2.0 / 3.0 - 0.5) / 0.25 * 10.0, 40.0],
    );
    // the mask is on the raster's grid
    let other = Value::Map(Map::new(Arc::new(Grid::new(3, 1)), vec![1.0, 1.0, 1.0]));
    let e =
        run(vec![("raster", source.clone()), ("reference", source.clone()), ("mask", other)]).unwrap_err().to_string();
    assert!(e.contains("mask"), "{e}");
    // a stack of three bands and a reference of two
    let z = reference_two();
    let e = run(vec![("raster", stack(&z)), ("reference", two_bands())]).unwrap_err().to_string();
    assert!(e.contains("layers"), "{e}");
}

fn reference_two() -> serde_json::Value {
    reference()["source"].clone()
}

fn two_bands() -> Value {
    let grid = Arc::new(Grid::new(2, 1));
    Value::Stack(Stack {
        grid,
        keys: vec![Key::Name("x".into()), Key::Name("y".into())],
        layers: vec![Arc::new(vec![1.0, 2.0]), Arc::new(vec![3.0, 4.0])],
    })
}
