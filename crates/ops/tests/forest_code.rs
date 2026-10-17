//! Brazil's Forest Code against the law's numbers worked by hand: the exact distances (against brute force), the
//! margins of rivers by width and of lakes by area, springs, the consolidated areas' narrower margins by fiscal
//! modules, and the Reserva Legal's balance with art. 15's APP and art. 67's small properties.

use std::sync::Arc;

use terraflux_ops::forest_code::{consolidated_lake, consolidated_river, nearest, river_margin};
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Geometry, Vector};

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

fn map(v: &Value) -> &Map {
    let Value::Map(m) = v else { panic!("{v:?}") };
    m
}

#[test]
fn distances_are_exact() {
    let (ny, nx) = (23, 31);
    let mut f = vec![false; ny * nx];
    let mut state = 7u64;
    for c in f.iter_mut() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *c = (state >> 33).is_multiple_of(17);
    }
    for (dx, dy) in [(10.0, 10.0), (30.0, 12.5)] {
        let (d, near) = nearest(&f, ny, nx, dx, dy);
        for i in 0..ny * nx {
            let (r, c) = ((i / nx) as f64, (i % nx) as f64);
            let best = (0..ny * nx)
                .filter(|&j| f[j])
                .map(|j| (((j % nx) as f64 - c) * dx).hypot(((j / nx) as f64 - r) * dy))
                .fold(f64::INFINITY, f64::min);
            assert!((d[i] - best).abs() < 1e-9, "{dx} {dy} at {i}: {} {best}", d[i]);
            let j = near[i].unwrap();
            let dj = (((j % nx) as f64 - c) * dx).hypot(((j / nx) as f64 - r) * dy);
            assert!((dj - best).abs() < 1e-9, "the nearest cell at {i}");
        }
    }
}

#[test]
fn the_laws_widths() {
    assert_eq!(
        [5.0, 10.0, 49.0, 50.0, 51.0, 200.0, 600.0, 601.0].map(river_margin),
        [30.0, 50.0, 50.0, 50.0, 100.0, 100.0, 200.0, 500.0]
    );
    assert_eq!([0.5, 1.0, 1.5, 3.0, 4.0, 9.0].map(|m| consolidated_river(m, 20.0)), [5.0, 5.0, 8.0, 15.0, 15.0, 20.0]);
    assert_eq!([1.0, 2.0, 4.0, 4.5].map(consolidated_lake), [5.0, 8.0, 15.0, 30.0]);
}

/// A grid of cells of 10 m, a river along column 0 and a lake in the corner.
fn grid(nx: usize, ny: usize) -> Arc<Grid> {
    let mut g = Grid::new(nx, ny);
    g.transform = [0.0, 10.0, 0.0, ny as f64 * 10.0, 0.0, -10.0];
    Arc::new(g)
}

#[test]
fn margins_of_rivers_lakes_and_springs() {
    let (nx, ny) = (80, 40);
    let g = grid(nx, ny);
    let rivers: Vec<f64> = (0..nx * ny).map(|i| f64::from(u8::from(i % nx == 0))).collect();
    // a river 12 m wide in the top half, 5 m in the bottom
    let width: Vec<f64> = (0..nx * ny).map(|i| if i / nx < 20 { 12.0 } else { 5.0 }).collect();
    let out = run(
        "forest_code.app",
        vec![
            ("rivers", Value::Map(Map::new(g.clone(), rivers.clone()))),
            ("river_width", Value::Map(Map::new(g.clone(), width))),
        ],
        serde_json::json!({}),
    );
    let app = map(&out["app"]);
    // from the channel's edge (5 m from its centre): 50 m where it is 12 m wide, 30 m where 5 m
    let row = |r: usize| (0..nx).filter(|&c| app.data[r * nx + c] == 1.0).count();
    assert_eq!(row(5), 5); // columns 1-5: centres at 10-50 m, edges 5-45 m from the channel's edge
    assert_eq!(row(30), 3);
    assert_eq!(app.data[0], 0.0, "the channel is not its margin");

    // consolidated, in a property of 2 fiscal modules (80 x 40 cells of 0.01 ha = 32 ha, modules of 16 ha): 8 m
    let props = vec![1.0; nx * ny];
    let out = run(
        "forest_code.app",
        vec![
            ("rivers", Value::Map(Map::new(g.clone(), rivers.clone()))),
            ("properties", Value::Map(Map::new(g.clone(), props))),
            ("consolidated", Value::Map(Map::new(g.clone(), vec![1.0; nx * ny]))),
        ],
        serde_json::json!({"fiscal_module": 16}),
    );
    let app = map(&out["app"]);
    assert_eq!((0..nx).filter(|&c| app.data[c] == 1.0).count(), 1); // the first column: 5 m from the edge

    // a lake of 9 cells (0.09 ha): 50 m; a spring: 50 m around it
    let mut lake = vec![0.0; nx * ny];
    for r in 10..13 {
        for c in 40..43 {
            lake[r * nx + c] = 1.0;
        }
    }
    let spring = Vector { geometries: vec![Geometry::Point([705.0, 205.0])], attributes: vec![], crs: None };
    let out = run(
        "forest_code.app",
        vec![("lakes", Value::Map(Map::new(g.clone(), lake))), ("springs", Value::Vector(Arc::new(spring)))],
        serde_json::json!({}),
    );
    let app = map(&out["app"]);
    let at = |r: usize, c: usize| app.data[r * nx + c];
    assert_eq!(at(11, 47), 2.0); // 50 m from the lake's centre column 42: 45 m from its edge
    assert_eq!(at(11, 48), 0.0);
    assert_eq!(at(19, 70), 3.0); // the spring's cell (column 70, row 19)
    assert_eq!(at(19, 75), 3.0); // 50 m
    assert_eq!(at(19, 76), 0.0);
    let t = table(&out["table"]);
    assert_eq!(t.rows[1][0], 2.0);
}

#[test]
fn the_reserva_legal_balance() {
    // two properties of 100 cells of 1 ha (cells of 100 m): 1 with 30 ha of vegetation (10 of them in APP), 2 with 10
    let (nx, ny) = (20, 10);
    let mut g = Grid::new(nx, ny);
    g.transform = [0.0, 100.0, 0.0, 1000.0, 0.0, -100.0];
    let g = Arc::new(g);
    let props: Vec<f64> = (0..200).map(|i| if i % nx < 10 { 1.0 } else { 2.0 }).collect();
    let veg: Vec<f64> = (0..200)
        .map(|i| {
            let (r, c) = (i / nx, i % nx);
            f64::from(u8::from((c < 10 && r < 3) || (c >= 10 && r < 1)))
        })
        .collect();
    let app: Vec<f64> = (0..200).map(|i| f64::from(u8::from(i / nx == 0))).collect();
    let m = |v: &Vec<f64>| Value::Map(Map::new(g.clone(), v.clone()));
    let out = run(
        "forest_code.reserve",
        vec![("properties", m(&props)), ("vegetation", m(&veg)), ("app", m(&app))],
        serde_json::json!({"share": 20}),
    );
    let t = table(&out["table"]);
    let col = |n: &str, r: usize| t.rows[r][t.columns.iter().position(|c| c == n).unwrap()];
    assert_eq!((col("area_ha", 0), col("native_ha", 0), col("app_native_ha", 0)), (100.0, 30.0, 10.0));
    assert_eq!((col("required_ha", 0), col("surplus_ha", 0), col("deficit_ha", 0)), (20.0, 10.0, 0.0));
    assert_eq!((col("counted_ha", 1), col("deficit_ha", 1)), (10.0, 10.0));
    // without counting the APP (art. 15 not used): property 1 counts 20, property 2 none
    let out = run(
        "forest_code.reserve",
        vec![("properties", m(&props)), ("vegetation", m(&veg)), ("app", m(&app))],
        serde_json::json!({"share": 20, "count_app": false}),
    );
    let t2 = table(&out["table"]);
    assert_eq!((t2.rows[0][8], t2.rows[1][9]), (20.0, 20.0));
    // art. 67: modules of 30 ha (property 2 has 3.3), the vegetation of 2008 the same as now: no deficit
    let out = run(
        "forest_code.reserve",
        vec![("properties", m(&props)), ("vegetation", m(&veg)), ("app", m(&app)), ("vegetation_2008", m(&veg))],
        serde_json::json!({"share": 20, "fiscal_module": 30}),
    );
    let t3 = table(&out["table"]);
    assert_eq!((t3.rows[1][7], t3.rows[1][9]), (10.0, 0.0));
    let bal = map(&out["balance"]);
    assert_eq!((bal.data[0], bal.data[15]), (10.0, 0.0));
}
