//! Thematic layers in the segmentation (`bounds`): no object crosses a zone of a map, of vectors or of objects; one
//! zone is no bounds at all, bit for bit; by tiles the same on any number of threads; region growing takes the same
//! layers; the live window cuts the bounds as the whole run does.

use std::sync::Arc;

use terraflux_ops::objects::{Objects, Whole, Window, WindowSettings, segment_window_bounds, segment_window_op};
use terraflux_ops::{Args, Ctx, Key, Map, Outputs, Registry, Stack, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Geometry, Vector};

const W: usize = 60;
const H: usize = 48;

pub(crate) fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*s >> 11) as f64) / (1u64 << 53) as f64
}

/// Two bands of an almost flat image (noise of about 1): with a big scale it is one object, unless bounds cut it.
pub(crate) fn flat(w: usize, h: usize) -> (Arc<Grid>, Vec<Arc<Vec<f64>>>) {
    let mut s = 21u64;
    let data = (0..2).map(|b| Arc::new((0..w * h).map(|_| 100.0 + b as f64 + lcg(&mut s)).collect())).collect();
    (Arc::new(Grid::new(w, h)), data)
}

pub(crate) fn stack((grid, data): &(Arc<Grid>, Vec<Arc<Vec<f64>>>)) -> Value {
    let keys = (0..data.len()).map(|k| Key::Name(format!("b{}", k + 1))).collect();
    Value::Stack(Stack { grid: grid.clone(), keys, layers: data.clone() })
}

pub(crate) fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    Registry::standard().get(op).unwrap().run(&mut Ctx::default(), &args)
}

pub(crate) fn objects_of(out: &Outputs) -> Arc<Objects> {
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

fn multires(image: Value, extra: Vec<(&str, Value)>, params: serde_json::Value) -> Arc<Objects> {
    let mut inputs = vec![("image", image)];
    inputs.extend(extra);
    objects_of(&run("segment.multiresolution", inputs, params).unwrap())
}

pub(crate) fn map(w: usize, h: usize, data: Vec<f64>) -> Value {
    Value::Map(Map::new(Arc::new(Grid::new(w, h)), data))
}

/// A map of zones: six blocks of 20 x 24 cells, a few cells without a value.
pub(crate) fn blocks_map() -> (Value, Vec<i64>) {
    let zone: Vec<i64> = (0..W * H).map(|i| ((i % W) / 20 + 3 * ((i / W) / 24)) as i64).collect();
    let data = zone.iter().enumerate().map(|(i, &z)| if i % 97 == 5 { f64::NAN } else { z as f64 }).collect();
    let zone = zone.iter().enumerate().map(|(i, &z)| if i % 97 == 5 { -1 } else { z }).collect();
    (map(W, H, data), zone)
}

/// Vectors: the left half, the top right quarter, and (in no feature) the bottom right quarter.
pub(crate) fn features() -> (Value, Vec<i64>) {
    let square = |c0: f64, r0: f64, c1: f64, r1: f64| {
        Geometry::Polygon(vec![vec![[c0, -r0], [c1, -r0], [c1, -r1], [c0, -r1], [c0, -r0]]])
    };
    let v =
        Vector { geometries: vec![square(0.0, 0.0, 30.0, 48.0), square(30.0, 0.0, 60.0, 24.0)], ..Default::default() };
    let zone = (0..W * H)
        .map(|i| {
            if i % W < 30 {
                0
            } else if i / W < 24 {
                1
            } else {
                2
            }
        })
        .collect();
    (Value::Vector(Arc::new(v)), zone)
}

/// Objects: nine blocks of 20 x 16 cells (from a map), the cells of the first without an object in a stripe.
pub(crate) fn blocks_objects() -> (Value, Vec<i64>) {
    let data: Vec<f64> = (0..W * H)
        .map(|i| {
            let (r, c) = (i / W, i % W);
            if r == 3 && c < 8 { f64::NAN } else { (c / 20 + 3 * (r / 16)) as f64 }
        })
        .collect();
    let out = run("segment.from_map", vec![("map", map(W, H, data))], serde_json::json!({})).unwrap();
    let o = objects_of(&out);
    let zone = o.labels.iter().map(|&l| l as i64).collect();
    (Value::Objects(o), zone)
}

/// No object has cells of two zones, and every zone has an object.
pub(crate) fn assert_no_crossing(o: &Objects, zone: &[i64], what: &str) {
    let mut of = vec![i64::MIN; o.count];
    for (i, &l) in o.labels.iter().enumerate() {
        if l == 0 {
            continue;
        }
        let k = l as usize - 1;
        if of[k] == i64::MIN {
            of[k] = zone[i];
        }
        assert_eq!(of[k], zone[i], "{what}: object {l} crosses a zone at cell {i}");
    }
    let mut zones: Vec<i64> = zone.to_vec();
    zones.sort_unstable();
    zones.dedup();
    assert!(o.count >= zones.len(), "{what}: {} objects for {} zones", o.count, zones.len());
}

pub(crate) fn crosses(o: &Objects, zone: &[i64]) -> bool {
    let mut of = vec![i64::MIN; o.count];
    o.labels.iter().enumerate().any(|(i, &l)| {
        l != 0 && {
            let k = l as usize - 1;
            if of[k] == i64::MIN {
                of[k] = zone[i];
            }
            of[k] != zone[i]
        }
    })
}

fn params() -> serde_json::Value {
    serde_json::json!({ "scale": 100.0, "shape": 0.1 })
}

#[test]
fn no_object_crosses_a_zone_of_a_map_vectors_or_objects() {
    let img = flat(W, H);
    let free = multires(stack(&img), vec![], params());
    for (name, (bounds, zone)) in [("a map", blocks_map()), ("vectors", features()), ("objects", blocks_objects())] {
        assert!(crosses(&free, &zone), "{name}: the image alone must cross the zones ({} objects)", free.count);
        let o = multires(stack(&img), vec![("bounds", bounds)], params());
        assert_no_crossing(&o, &zone, name);
    }
}

#[test]
fn one_zone_is_no_bounds_bit_for_bit() {
    let img = flat(W, H);
    let mut s = 3u64;
    // a busier image, so that the objects are many and the merge order matters
    let busy = (
        img.0.clone(),
        (0..3).map(|_| Arc::new((0..W * H).map(|_| 40.0 * lcg(&mut s)).collect::<Vec<f64>>())).collect::<Vec<_>>(),
    );
    let p = serde_json::json!({ "scale": 6.0, "shape": 0.3, "seed": 5 });
    let want = multires(stack(&busy), vec![], p.clone());
    assert!(want.count > 20, "{} objects", want.count);
    let one_map = map(W, H, vec![7.0; W * H]);
    let got = multires(stack(&busy), vec![("bounds", one_map.clone())], p.clone());
    assert_eq!(got.labels[..], want.labels[..], "one zone, a map");
    // (-0 and 0, and any NaN, are one value)
    let signs = map(W, H, (0..W * H).map(|i| if i % 2 == 0 { 0.0 } else { -0.0 }).collect());
    assert_eq!(multires(stack(&busy), vec![("bounds", signs)], p.clone()).labels[..], want.labels[..]);
    let nans = map(W, H, vec![f64::NAN; W * H]);
    assert_eq!(multires(stack(&busy), vec![("bounds", nans)], p.clone()).labels[..], want.labels[..]);
    // with `from`
    let below = multires(stack(&busy), vec![], serde_json::json!({ "scale": 3.0, "seed": 5 }));
    let from = Value::Objects(below);
    let want = multires(stack(&busy), vec![("from", from.clone())], p.clone());
    let got = multires(stack(&busy), vec![("from", from), ("bounds", one_map)], p);
    assert_eq!(got.labels[..], want.labels[..], "one zone, with from");
}

#[test]
fn from_objects_that_cross_a_zone_are_cut() {
    let img = flat(W, H);
    // one object for the whole image, and the squares of 30 cells: both cross the zones
    let whole = objects_of(
        &run("segment.from_map", vec![("map", map(W, H, vec![1.0; W * H]))], serde_json::json!({})).unwrap(),
    );
    assert_eq!(whole.count, 1);
    let squares = objects_of(
        &run("segment.chessboard", vec![("image", map(W, H, vec![1.0; W * H]))], serde_json::json!({ "size": 30 }))
            .unwrap(),
    );
    for (name, from) in [("the whole image", whole), ("squares", squares)] {
        for (what, (bounds, zone)) in [("a map", blocks_map()), ("vectors", features())] {
            let o = multires(stack(&img), vec![("from", Value::Objects(from.clone())), ("bounds", bounds)], params());
            assert_no_crossing(&o, &zone, &format!("from {name}, {what}"));
        }
    }
}

#[test]
fn a_zone_edge_still_counts_in_the_shape_cost() {
    // a high weight of shape (compactness only): the zones' edges count in the perimeters, so the run is the same
    // twice and no object crosses
    let img = flat(W, H);
    let (bounds, zone) = blocks_map();
    let p = serde_json::json!({ "scale": 100.0, "shape": 0.9, "compactness": 1.0 });
    let a = multires(stack(&img), vec![("bounds", bounds.clone())], p.clone());
    let b = multires(stack(&img), vec![("bounds", bounds)], p);
    assert_eq!(a.labels[..], b.labels[..]);
    assert_no_crossing(&a, &zone, "shape 0.9");
}

#[test]
fn tiled_with_bounds_has_no_crossing_and_the_same_objects_on_any_threads() {
    let (w, h) = (96, 80);
    let img = flat(w, h);
    // zones of 40 x 32 cells: their borders cross the tiles of 32
    let zone: Vec<i64> = (0..w * h).map(|i| ((i % w) / 40 + 3 * ((i / w) / 32)) as i64).collect();
    let bounds = map(w, h, zone.iter().map(|&z| z as f64).collect());
    let p = serde_json::json!({ "scale": 100.0, "tile": 32, "margin": 8, "seed": 2 });
    let run_on = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| multires(stack(&img), vec![("bounds", bounds.clone())], p.clone()))
    };
    let one = run_on(1);
    assert_no_crossing(&one, &zone, "tiled");
    assert!(one.count >= 6);
    let eight = run_on(8);
    assert_eq!(one.labels[..], eight.labels[..], "1 and 8 threads");
    let hash = |o: &Objects| {
        let mut h = std::hash::DefaultHasher::new();
        o.content_hash(&mut h);
        std::hash::Hasher::finish(&h)
    };
    assert_eq!(hash(&one), hash(&eight));
}

#[test]
fn region_growing_takes_vectors_and_objects_as_bounds() {
    let img = flat(W, H);
    let p = serde_json::json!({ "threshold": 1.0 });
    let free = objects_of(&run("segment.region_growing", vec![("image", stack(&img))], p.clone()).unwrap());
    for (name, (bounds, zone)) in [("vectors", features()), ("objects", blocks_objects()), ("a map", blocks_map())] {
        assert!(crosses(&free, &zone), "{name}: the image alone must cross the zones");
        let out = run("segment.region_growing", vec![("image", stack(&img)), ("bounds", bounds)], p.clone()).unwrap();
        assert_no_crossing(&objects_of(&out), &zone, name);
    }
    // a thing that is not a layer of zones, and a layer of another grid
    let e = run("segment.region_growing", vec![("image", stack(&img)), ("bounds", Value::Number(1.0))], p.clone());
    assert!(e.err().unwrap().to_string().contains("bounds"));
    let other = map(W + 1, H, vec![0.0; (W + 1) * H]);
    let e = run("segment.multiresolution", vec![("image", stack(&img)), ("bounds", other)], params());
    let msg = e.err().unwrap().to_string();
    assert!(msg.contains("bounds") && msg.contains("another grid"), "{msg}");
}

#[test]
fn the_live_window_cuts_the_bounds_and_on_the_whole_image_is_the_runs() {
    let img = flat(W, H);
    let names = ["a".to_string(), "b".to_string()];
    let whole = Whole::of(&img.1);
    let full = Window { x0: 0, y0: 0, width: W, height: H };
    let p = serde_json::json!({ "scale": 100.0, "shape": 0.1 });
    let set = WindowSettings { scale: 100.0, shape: 0.1, ..Default::default() };
    for (name, (bounds, zone)) in [("a map", blocks_map()), ("vectors", features()), ("objects", blocks_objects())] {
        let want = multires(stack(&img), vec![("bounds", bounds.clone())], p.clone());
        assert_no_crossing(&want, &zone, name);
        let params = p.as_object().cloned().unwrap();
        let extra = [("bounds".to_string(), bounds.clone())];
        let got = segment_window_op(
            &Registry::standard(),
            "segment.multiresolution",
            &params,
            &img.0,
            &img.1,
            &names,
            &whole,
            full,
            1,
            &extra,
        )
        .unwrap();
        assert_eq!(got.labels[..], want.labels[..], "{name}: segment_window_op on the whole window");
        let got = segment_window_bounds(&img.0, &img.1, full, 1, &set, Some(&bounds)).unwrap();
        assert_eq!(got.labels[..], want.labels[..], "{name}: segment_window_bounds on the whole window");
        // a part read every second cell: the zones of the cells it reads
        let win = Window { x0: 7, y0: 5, width: 50, height: 40 };
        let part = segment_window_bounds(&img.0, &img.1, win, 2, &set, Some(&bounds)).unwrap();
        let cut: Vec<i64> = (0..part.grid.cells())
            .map(|i| zone[(win.y0 + (i / part.grid.width) * 2) * W + win.x0 + (i % part.grid.width) * 2])
            .collect();
        assert_no_crossing(&part, &cut, &format!("{name}, a window of every second cell"));
    }
}
