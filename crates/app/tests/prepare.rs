//! The preparation in a click: data next to the reference grid, the steps suggested for it, and the nodes they
//! add (with their notes), run: a raster of another coordinate system and cells put on the grid, the distance to
//! features and to a class on it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use terraflux_app::prepare::{self, Data, Kind, Step};
use terraflux_exec::Engine;
use terraflux_graph::{Model, Node};
use terraflux_ops::{Registry, Value};
use terraflux_raster::vector::{Column, Geometry, Vector};

struct Quiet;
impl terraflux_exec::Observer for Quiet {}

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name)
}

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("terraflux-prepare-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The model run: its results.
fn run(m: &Model) -> terraflux_exec::Results {
    Engine::default().run(m, &BTreeMap::new(), &mut Quiet).unwrap()
}

fn read(id: &str, path: &Path) -> Node {
    Node::new(id, "raster.read").param("path", path.to_string_lossy())
}

/// Sibuyan's land use in UTM 51N (the reference), its drivers in geographic coordinates; a road and two towns.
fn inputs(d: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let land = d.join("land.tif");
    terraflux_gdal::translate(&data("sibuyan_landuse.tif"), &land, &["-a_srs".into(), "EPSG:32651".into()]).unwrap();
    let utm = d.join("drivers_utm.tif");
    terraflux_gdal::translate(&data("sibuyan_drivers.tif"), &utm, &["-a_srs".into(), "EPSG:32651".into()]).unwrap();
    let geo = d.join("drivers_geo.tif");
    terraflux_gdal::warp(
        &[&utm],
        &geo,
        &["-t_srs".into(), "EPSG:4326".into(), "-tr".into(), "0.004".into(), "0.004".into()],
    )
    .unwrap();
    let roads = d.join("roads.geojson");
    let v = Vector {
        geometries: vec![
            Geometry::LineString(vec![[437000.0, 1370000.0], [452000.0, 1375000.0], [467000.0, 1360000.0]]),
            Geometry::Point([440000.0, 1380000.0]),
            Geometry::Point([460000.0, 1358000.0]),
        ],
        attributes: vec![("kind".into(), Column::Numbers(vec![1.0, 2.0, 2.0]))],
        crs: Some("EPSG:32651".into()),
    };
    terraflux_gdal::write_vector(&roads, "roads", &v).unwrap();
    (land, geo, roads)
}

#[test]
fn kinds_resample_as_their_values_need() {
    assert_eq!(Kind::Classes.resampling(false), "near");
    assert_eq!(Kind::Classes.resampling(true), "mode");
    assert_eq!(Kind::Values.resampling(false), "bilinear");
    assert_eq!(Kind::Values.resampling(true), "average");
    assert_eq!(Kind::Counts.resampling(false), "sum");
    assert_eq!(prepare::crs_name(Some("EPSG:32651")), "EPSG:32651");
    assert!(
        prepare::crs_name(Some(r#"PROJCRS["WGS 84 / UTM zone 51N",BASEGEOGCRS[...],ID["EPSG",32651]]"#))
            .starts_with("WGS 84 / UTM zone 51N (EPSG:32651)")
    );
}

/// A raster of another coordinate system put on the reference grid by the step suggested; a distance to roads
/// and to a class, each a node with its note; the model run gives maps on the grid.
#[test]
fn steps_put_data_on_the_reference_grid() {
    let d = dir();
    let (land, geo, roads) = inputs(&d);
    let r = Registry::standard();
    let mut m = Model::new("prepared");
    m.nodes.push(read("land", &land));
    m.nodes.push(read("drivers", &geo));
    m.nodes.push(Node::new("roads", "vector.read").param("path", roads.to_string_lossy()));
    let res = run(&m);
    let grid = |v: &Value| Data::of(v).unwrap().grid().unwrap().clone();
    let land_v = res.get("land.raster").unwrap().clone();
    let reference = grid(&land_v);
    // the land use: classes, on its own grid
    let land_d = Data::of(&land_v).unwrap();
    assert_eq!(land_d.kind(), Kind::Classes);
    assert!(!prepare::compare(&reference, &reference).any());
    // the drivers: values in another coordinate system, on smaller cells
    let drivers = Data::of(res.get("drivers.raster").unwrap()).unwrap();
    assert_eq!(drivers.kind(), Kind::Values);
    let diff = prepare::compare(drivers.grid().unwrap(), &reference);
    assert!(diff.crs && diff.extent, "{diff:?}");
    let steps = prepare::suggestions(&drivers, Kind::Values, Some(&reference));
    let Some(Step::Match { kind: Kind::Values, coarser }) = steps.first().cloned() else { panic!("{steps:?}") };
    assert!(steps.contains(&Step::Slope));
    let step = Step::Match { kind: Kind::Values, coarser };
    let id = prepare::apply(
        &mut m,
        &r,
        &step,
        ("drivers", "raster"),
        Some(("land", "raster")),
        &diff.describe(),
        [0.0, 0.0],
    )
    .unwrap();
    let n = m.nodes.iter().find(|n| n.id == id).unwrap();
    assert_eq!(n.op, "raster.match");
    assert_eq!(n.params["resampling"], serde_json::json!(Kind::Values.resampling(coarser)));
    assert!(n.note.as_ref().unwrap().contains("another coordinate system"), "{:?}", n.note);
    assert_eq!(prepare::reference_of(&m), Some(("land".to_string(), "raster".to_string())));
    // vectors: the distance to them on the grid
    let roads_d = Data::of(res.get("roads.vector").unwrap()).unwrap();
    let steps = prepare::suggestions(&roads_d, Kind::Values, Some(&reference));
    assert_eq!(steps.first(), Some(&Step::DistanceToFeatures));
    assert!(prepare::suggestions(&roads_d, Kind::Values, None).is_empty());
    let dist = prepare::apply(
        &mut m,
        &r,
        &Step::DistanceToFeatures,
        ("roads", "vector"),
        Some(("land", "raster")),
        &[],
        [0.0, 0.0],
    )
    .unwrap();
    let burnt = prepare::apply(
        &mut m,
        &r,
        &Step::Rasterize { field: Some("kind".into()) },
        ("roads", "vector"),
        Some(("land", "raster")),
        &[],
        [0.0, 0.0],
    )
    .unwrap();
    // a class: the distance to the cells of class 1 of the land use
    let to_class = prepare::apply(
        &mut m,
        &r,
        &Step::DistanceToClass { classes: vec![1.0] },
        ("land", "map"),
        None,
        &[],
        [0.0, 0.0],
    )
    .unwrap();
    let slope = prepare::apply(&mut m, &r, &Step::Slope, (&id, "raster"), None, &[], [0.0, 0.0]).unwrap();
    // the steps that need a grid refuse without one
    assert!(prepare::apply(&mut m, &r, &Step::Zonal, ("roads", "vector"), None, &[], [0.0, 0.0]).is_err());
    let issues = terraflux_graph::check(&m, &r);
    assert!(issues.is_empty(), "{issues:?}");
    let res = run(&m);
    let matched = grid(res.get(&format!("{id}.raster")).unwrap());
    assert!(matched.same_as(&reference), "{:?} {:?}", matched.transform, reference.transform);
    let Value::Map(dist) = res.get(&format!("{dist}.map")).unwrap() else { panic!() };
    assert!(dist.grid.same_as(&reference));
    let min = dist.data.iter().copied().fold(f64::INFINITY, f64::min);
    assert_eq!(min, 0.0);
    assert!(dist.data.iter().all(|v| *v >= 0.0 && v.is_finite()));
    let Value::Map(burnt) = res.get(&format!("{burnt}.map")).unwrap() else { panic!() };
    assert!(burnt.data.contains(&1.0) && burnt.data.contains(&2.0));
    let Value::Map(to_class) = res.get(&format!("{to_class}.distance")).unwrap() else { panic!() };
    // raster.read's map stays rasters on disk until a node reads it
    let classes = match res.get("land.map").unwrap() {
        Value::Raster(r) => r.map().unwrap(),
        Value::Map(m) => m.clone(),
        v => panic!("{:?}", v.ty()),
    };
    for (c, v) in classes.data.iter().zip(to_class.data.iter()) {
        if *c == 1.0 {
            assert_eq!(*v, 0.0);
        }
    }
    assert!(matches!(res.get(&format!("{slope}.raster")), Some(Value::Raster(_))));
    std::fs::remove_dir_all(&d).ok();
}
