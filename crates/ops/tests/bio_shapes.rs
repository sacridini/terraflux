//! Biodiverse's spatial conditions on shapefiles against landschaft's (tests/data/reference/bio_shapes.json, the
//! polygons in bio_shapes.gpkg and .shp, tools/make_bio_shapes_reference.py): sp_point_in_poly_shape (every
//! feature, an attribute's text or number, a position, negated, the processing group's coordinates, other axes)
//! as definition queries and neighbourhoods, and sp_points_in_same_poly_shape alone and with a circle.

use std::path::Path;

use serde_json::Value as Json;
use terraflux_ops::bio::BaseData;
use terraflux_ops::bio::conditions::{Condition, definition_query, neighbours};
use terraflux_ops::{Args, Ctx, Registry, Value};

fn basedata(dir: &Path, groups: &[&str]) -> BaseData {
    let r = Registry::standard();
    let params = serde_json::json!({"path": dir.join("bio_shapes.csv").to_string_lossy(), "group_columns": groups,
                                    "label_columns": ["sp"], "cell_sizes": vec![1.0; groups.len()]});
    let args = Args { node: "n", inputs: Default::default(), params: params.as_object().unwrap() };
    let out = r.get("bio.table").unwrap().run(&mut Ctx::default(), &args).unwrap();
    let Value::BaseData(bd) = &out["basedata"] else { panic!() };
    (**bd).clone()
}

fn ints(v: &Json) -> Vec<i64> {
    v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect()
}

#[test]
fn shape_conditions_are_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let text = std::fs::read_to_string(dir.join("bio_shapes.json")).unwrap();
    let base = dir.canonicalize().unwrap().to_string_lossy().replace('\\', "/");
    let base = base.strip_prefix("//?/").unwrap_or(&base).to_string();
    let meta: Json = serde_json::from_str(&text.replace("$REF/", &format!("{base}/"))).unwrap();
    let bd = basedata(&dir, &["x", "y"]);
    for (case, c) in meta["queries"].as_object().unwrap() {
        let q = Condition::new(c["condition"].as_str().unwrap()).unwrap();
        let mask: Vec<i64> = definition_query(&bd, &q, true).unwrap().iter().map(|&b| i64::from(b)).collect();
        assert_eq!(mask, ints(&c["mask"]), "{case}");
    }
    let bd3 = basedata(&dir, &["z", "x", "y"]);
    for (set, bd) in [("neighbours", &bd), ("axes", &bd3)] {
        for (case, c) in meta[set].as_object().unwrap() {
            let cond = Condition::new(c["condition"].as_str().unwrap()).unwrap();
            let nb = neighbours(bd, &[cond], None, true, true, None).unwrap();
            assert_eq!(nb.groups.iter().map(|&g| g as i64).collect::<Vec<_>>(), ints(&c["groups"]), "{case}: groups");
            assert_eq!(nb.sets[0].0, ints(&c["indptr"]), "{case}: indptr");
            assert_eq!(nb.sets[0].1.iter().map(|&g| g as i64).collect::<Vec<_>>(), ints(&c["indices"]), "{case}");
        }
    }
}
