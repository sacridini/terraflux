//! A hash of what a value holds (not where it lives): the values a loop's body reads from outside, carried or
//! `@item`, are new Arcs at every run (`elements`, a carried map made again), so a node of a body is told apart by
//! its inputs' content.

use std::hash::{Hash, Hasher};

use terraflux_ops::Value;

const K: u64 = 0x9E37_79B9_7F4A_7C15;

/// The bits of the numbers of a slice, four lanes at a time (about the speed of memory; not for anything but telling
/// two values apart).
fn words(data: &[f64]) -> u64 {
    let mut lanes = [K, K.rotate_left(16), K.rotate_left(32), K.rotate_left(48)];
    let (chunks, rest) = data.as_chunks::<4>();
    for c in chunks {
        for (l, x) in lanes.iter_mut().zip(c) {
            *l = (l.rotate_left(23) ^ x.to_bits()).wrapping_mul(K);
        }
    }
    for x in rest {
        lanes[0] = (lanes[0].rotate_left(23) ^ x.to_bits()).wrapping_mul(K);
    }
    let mut h = data.len() as u64;
    for l in lanes {
        h = (h.rotate_left(29) ^ l).wrapping_mul(K);
    }
    h ^ (h >> 32)
}

/// Hash a value by its content. False when it cannot be told apart by it: it holds a random generator, whose state
/// moves as it is drawn from (what reads it is not kept).
pub(crate) fn hash_value(v: &Value, h: &mut impl Hasher) -> bool {
    match v {
        Value::Rng(_) | Value::NumpyRng(_) => return false,
        Value::Map(m) => {
            1u8.hash(h);
            format!("{:?}", m.grid).hash(h);
            words(&m.data).hash(h);
        }
        Value::Stack(s) => {
            2u8.hash(h);
            format!("{:?}", s.grid).hash(h);
            format!("{:?}", s.keys).hash(h);
            s.layers.len().hash(h);
            for l in &s.layers {
                words(l).hash(h);
            }
        }
        Value::Series(items) => {
            3u8.hash(h);
            items.len().hash(h);
            return items.iter().all(|x| hash_value(x, h));
        }
        Value::Table(t) => {
            4u8.hash(h);
            t.columns.hash(h);
            t.rows.len().hash(h);
            for r in &t.rows {
                words(r).hash(h);
            }
            t.labels.hash(h);
        }
        Value::Number(x) => (5u8, x.to_bits()).hash(h),
        Value::Integer(i) => (6u8, i).hash(h),
        Value::Bool(b) => (7u8, b).hash(h),
        Value::Text(t) => (8u8, t).hash(h),
        Value::Objects(o) => {
            9u8.hash(h);
            o.content_hash(h);
        }
        // (the rest as the top level reads them: what they print)
        v => format!("{v:?}").hash(h),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::sync::Arc;
    use terraflux_raster::Grid;

    fn print(v: &Value) -> Option<u64> {
        let mut h = DefaultHasher::new();
        hash_value(v, &mut h).then(|| h.finish())
    }

    fn map(w: usize, h: usize, f: impl Fn(usize) -> f64) -> Value {
        Value::Map(terraflux_ops::Map::new(Arc::new(Grid::new(w, h)), (0..w * h).map(f).collect()))
    }

    #[test]
    fn equal_content_equal_print_whatever_the_arc() {
        let (a, b) = (map(4, 3, |i| i as f64), map(4, 3, |i| i as f64));
        assert_eq!(print(&a), print(&b));
        assert_ne!(print(&a), print(&map(4, 3, |i| if i == 7 { 8.0 } else { i as f64 })));
        assert_ne!(print(&a), print(&map(3, 4, |i| i as f64)), "the shape too");
        assert_ne!(print(&Value::Number(1.0)), print(&Value::Integer(1)));
        let s = Value::Series(vec![a.clone(), Value::Text("x".into())]);
        assert_eq!(print(&s), print(&Value::Series(vec![b, Value::Text("x".into())])));
    }

    /// What hashing a 1000 x 1000 map costs (run with --nocapture).
    #[test]
    fn the_cost_of_a_big_map() {
        let v = map(1000, 1000, |i| (i as f64).sin());
        let t = std::time::Instant::now();
        let n = 20;
        let mut last = 0;
        for _ in 0..n {
            last = print(&v).unwrap();
        }
        let per = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        eprintln!("hash of a 1000 x 1000 map: {per:.2} ms ({last})");
        assert!(per < 50.0, "{per} ms");
    }
}
