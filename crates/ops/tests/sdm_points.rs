//! Thinning, a record per cell and background points against landschaft's (tests/data/reference/sdm_points.npz,
//! .json and .gpkg, tools/make_sdm_points_reference.py): spThin's thinning, one record per cell with and without
//! the missing cells, and background points on a lon/lat grid (by the cells' widths, a band of distances, an
//! envelope, a bias, an area) and a projected one, to the bit.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Map, Outputs, Registry, Stack, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::Column;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn check(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    if let Some(k) = got.iter().zip(want).position(|(a, b)| a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan())) {
        panic!("{what}: at {k}: {} {}", got[k], want[k]);
    }
}

fn rows(v: &Value) -> Vec<f64> {
    let Value::Table(t) = v else { panic!("{v:?}") };
    t.rows.iter().flatten().copied().collect()
}

#[test]
fn points_are_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("sdm_points.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sdm_points.json")).unwrap()).unwrap();
    let transform = |k: &str| -> [f64; 6] {
        let v: Vec<f64> = meta[k].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        [v[0], v[1], v[2], v[3], v[4], v[5]]
    };
    let stack = |data: &[f64], t: [f64; 6]| {
        let mut g = Grid::new(48, 40);
        g.transform = t;
        let n = 48 * 40;
        Value::Stack(Stack {
            grid: Arc::new(g),
            keys: vec![Key::Name("a".into()), Key::Name("b".into())],
            layers: vec![Arc::new(data[..n].to_vec()), Arc::new(data[n..].to_vec())],
        })
    };
    let env = stack(&z["in/env"].data, transform("geo"));
    let env_p = stack(&z["in/env_p"].data, transform("proj"));
    let mut occ = Table::new(vec!["lon".into(), "lat".into()]);
    occ.rows = z["in/occ"].data.chunks(2).map(|r| r.to_vec()).collect();
    let occ = Value::Table(occ);

    let out = run(
        "sdm.thin",
        vec![("occurrences", occ.clone())],
        serde_json::json!({"distance": 60000.0, "reps": 5, "seed": 3}),
    );
    check("thin", &rows(&out["occurrences"]), &z["thin"].data);
    let kept: Vec<f64> = serde_json::from_value::<Vec<f64>>(match &out["kept"] {
        Value::Json(j) => j.clone(),
        _ => panic!(),
    })
    .unwrap();
    check("thin kept", &kept, &z["thin_kept"].data);
    for (drop, key) in [(false, "one"), (true, "one_drop")] {
        let out = run(
            "sdm.one_per_cell",
            vec![("occurrences", occ.clone()), ("raster", env.clone())],
            serde_json::json!({"drop_missing": drop}),
        );
        check(key, &rows(&out["occurrences"]), &z[key].data);
    }

    let Value::Stack(s) = &env else { panic!() };
    let bias = Value::Map(Map::new(s.grid.clone(), z["in/bias"].data.clone()));
    let area = Value::Vector(Arc::new(terraflux_gdal::read_vector(dir.join("sdm_points.gpkg"), Some("area")).unwrap()));
    for (name, c) in meta["background"].as_object().unwrap() {
        let mut params = serde_json::json!({"n": c["n"], "seed": c["seed"]});
        for k in ["distance", "envelope", "tryf"] {
            if let Some(v) = c.get(k) {
                params[k] = v.clone();
            }
        }
        let mut inputs = vec![("env", if c.get("projected").is_some() { env_p.clone() } else { env.clone() })];
        if c.get("occ").is_some() {
            inputs.push(("occurrences", occ.clone()));
        }
        if c.get("bias").is_some() {
            inputs.push(("bias", bias.clone()));
        }
        if c.get("area").is_some() {
            inputs.push(("area", area.clone()));
        }
        let out = run("sdm.background", inputs, params);
        let Value::Vector(v) = &out["points"] else { panic!() };
        let Column::Numbers(cells) = &v.attributes[0].1 else { panic!() };
        check(name, cells, &z[name.as_str()].data);
    }
}
