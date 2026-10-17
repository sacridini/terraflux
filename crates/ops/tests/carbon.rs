//! The carbon of land change over a series of maps, against sums done by hand: a forest kept, a forest cleared, a
//! pasture left to regrow (Chapman-Richards and linear) and a secondary forest of unknown age turned to forest.

use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Outputs, Registry, Stack, Table, Value};
use terraflux_raster::Grid;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn table(v: &Value) -> &Table {
    let Value::Table(t) = v else { panic!("{v:?}") };
    t
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} {b}");
}

/// Four cells of a hectare at 2000, 2010 and 2020: 1 forest (100 tC/ha), 2 pasture (10), 3 secondary forest.
fn maps() -> Value {
    let mut g = Grid::new(2, 2);
    g.transform = [0.0, 100.0, 0.0, 200.0, 0.0, -100.0];
    let layers = [[1.0, 1.0, 2.0, 3.0], [1.0, 2.0, 3.0, 3.0], [1.0, 2.0, 3.0, 1.0]];
    Value::Stack(Stack {
        grid: Arc::new(g),
        keys: ["2000", "2010", "2020"].iter().map(|k| Key::Name(k.to_string())).collect(),
        layers: layers.iter().map(|l| Arc::new(l.to_vec())).collect(),
    })
}

#[test]
fn emissions_and_removals_by_hand() {
    let regrowth = 80.0 * (1.0 - (-0.1f64 * 10.0).exp());
    let out = run(
        "carbon.emissions",
        vec![("maps", maps())],
        serde_json::json!({"stock": {"1": 100, "2": 10, "3": 40}, "regrowth": {"3": {"max": 80, "rate": 0.1}}, "co2": false}),
    );
    let p = table(&out["periods"]);
    // 2000-2010: the forest cleared (90) and the pasture's 10 lost when it turned to young regrowth
    assert_eq!(p.rows[0][..2], [2000.0, 2010.0]);
    close(p.rows[0][2], 100.0);
    close(p.rows[0][3], 0.0);
    // 2010-2020: ten years of regrowth, and the secondary forest of unknown age (40) turned to forest (100)
    close(p.rows[1][2], 0.0);
    close(p.rows[1][3], regrowth + 60.0);
    close(p.rows[1][4], -(regrowth + 60.0));
    let t = table(&out["transitions"]);
    let row = |y: f64, from: f64, to: f64| t.rows.iter().find(|r| r[0] == y && r[2] == from && r[3] == to).unwrap();
    close(row(2000.0, 1.0, 2.0)[5], 90.0);
    close(row(2000.0, 2.0, 3.0)[5], 10.0);
    close(row(2010.0, 3.0, 3.0)[6], regrowth);
    close(row(2010.0, 3.0, 1.0)[6], 60.0);
    close(row(2000.0, 1.0, 1.0)[4], 1.0);
    let Value::Stack(s) = &out["stocks"] else { panic!() };
    assert_eq!(*s.layers[1], vec![100.0, 10.0, 0.0, 40.0]);
    close(s.layers[2][2], regrowth);
    let Value::Map(net) = &out["net"] else { panic!() };
    close(net.data[1], 90.0);
    close(net.data[2], 10.0 - regrowth);

    // in CO2, and a linear regrowth of 3 tC/ha a year
    let out = run(
        "carbon.emissions",
        vec![("maps", maps())],
        serde_json::json!({"stock": {"1": 100, "2": 10, "3": 40}, "regrowth": {"3": {"max": 80, "rate": 3, "form": "linear"}}}),
    );
    let p = table(&out["periods"]);
    close(p.rows[0][2], 100.0 * 44.0 / 12.0);
    close(p.rows[1][3], (30.0 + 60.0) * 44.0 / 12.0);
}
