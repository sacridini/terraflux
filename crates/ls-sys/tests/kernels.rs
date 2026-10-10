//! The kernels through the bridge: known values of R and small cases worked by hand.

use terraflux_ls_sys::{RRng, ffi};

#[test]
fn r_generator() {
    // R: set.seed(1); runif(3)
    let mut rng = RRng::new(1);
    let u = rng.runif(3);
    let r = [0.2655086631421, 0.3721238996368, 0.5728533633519];
    for (a, b) in u.iter().zip(r) {
        assert!((a - b).abs() < 1e-12, "{a} != {b}");
    }
}

#[test]
fn classes_and_crosstab() {
    let a = [1.0, 2.0, 2.0, f64::NAN, 3.0];
    let b = [1.0, 1.0, 2.0, 2.0, 3.0];
    let c = ffi::map_classes(&a);
    assert_eq!(c, vec![1.0, 2.0, 3.0]);
    let t = ffi::crosstab(&a, &b, &c, &c).unwrap();
    assert_eq!(t, vec![1, 0, 0, 1, 1, 0, 0, 0, 1]);
}

#[test]
fn errors_reach_rust() {
    let e = ffi::allow_matrix(&[1.0, 9.0], &[1, 1], &[1.0, 2.0], &[0.0, 0.0], &[1, 1, 1, 1]).unwrap_err();
    assert!(e.what().contains("does not know"), "{}", e.what());
    assert!(ffi::markov_root(&[0.3, 0.7, 0.65, 0.35], 2, 3).is_err());
}

#[test]
fn markov_and_demand() {
    let mut kept = Vec::new();
    let p = ffi::markov_from_counts(&[90.0, 10.0, 0.0, 100.0], 2, &mut kept).unwrap();
    assert_eq!(kept, vec![0, 1]);
    assert_eq!(p, vec![0.9, 0.1, 0.0, 1.0]);
    let root = ffi::markov_root(&p, 2, 2).unwrap();
    let sq = [root[0] * root[0] + root[1] * root[2], root[0] * root[1] + root[1] * root[3]];
    assert!((sq[0] - 0.9).abs() < 1e-14 && (sq[1] - 0.1).abs() < 1e-14);
    let r = ffi::round_sum(&[1.0, 1.0, 1.0], 1, 3, 10).unwrap();
    assert_eq!(r.iter().sum::<f64>(), 10.0);
    let y = ffi::approx_extrap(&[0.0, 10.0], &[0.0, 100.0], &[5.0, 20.0]).unwrap();
    assert_eq!(y, vec![50.0, 200.0]);
}

#[test]
fn woe_pieces() {
    let w = ffi::woe_weights(5.0, 10.0, 50.0, 1000.0, false);
    assert!(w.plus > 0.0 && w.contrast > 0.0);
    let mut sums = vec![0.0, f64::NAN, 1.0, f64::NAN];
    ffi::woe_normalise(&mut sums, 2, 2).unwrap();
    assert!((sums[0] - 1.0 / (1.0 + 1.0 + std::f64::consts::E)).abs() < 1e-15);
    assert!(sums[1].is_nan() && sums[3].is_nan());
}

#[test]
fn observed_patches() {
    // one new patch of 2 cells, one expansion of 1 cell next to class 2
    let a = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0];
    let b = [2.0, 2.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 1.0];
    let o = ffi::observed_patches(&a, &b, 3, 3, 1.0, 2.0).unwrap();
    assert_eq!(o.cells, 3);
    assert_eq!((o.new_patches, o.expansion_patches), (1, 1));
    assert_eq!((o.new_mean, o.expansion_mean), (2.0, 1.0));
}
