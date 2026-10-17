//! Interpolation against R's gstat 2.1 (tests/data/reference/interpolation.npz,
//! tools/make_interpolation_reference.py): the sample variogram of 120 points (gstat's default cutoff and width,
//! and a cutoff of 50 in classes of 5), the fit of a spherical, an exponential and a Gaussian model (the converged
//! fits to the bit), inverse distance weighting and ordinary kriging on a grid of 25 x 20 cells; to rounding.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::interpolation::{Model, Shape, default_cutoff, fit_variogram, idw_at, krige, sample_variogram};
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;
use terraflux_raster::vector::{Column, Geometry, Vector};

fn reference() -> std::collections::BTreeMap<String, Array> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    terraflux_raster::npy::read_npz(dir.join("interpolation.npz")).unwrap()
}

fn close(what: &str, got: &[f64], want: &[f64], tol: f64) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        assert!((a - b).abs() <= tol * b.abs().max(1.0), "{what}: at {k}: {a} {b}");
    }
}

fn points(z: &std::collections::BTreeMap<String, Array>) -> (Vec<[f64; 2]>, Vec<f64>) {
    z["points"].data.chunks(3).map(|p| ([p[0], p[1]], p[2])).unzip()
}

/// The grid's cell centres, row by row from the top: 25 x 20 cells of 4 from (0, 80).
fn grid() -> Grid {
    let mut g = Grid::new(25, 20);
    g.transform = [0.0, 4.0, 0.0, 80.0, 0.0, -4.0];
    g
}

fn centres() -> Vec<[f64; 2]> {
    (0..20).flat_map(|r| (0..25).map(move |c| [2.0 + 4.0 * c as f64, 78.0 - 4.0 * r as f64])).collect()
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

#[test]
fn the_sample_variogram_is_gstats() {
    let z = reference();
    let (xy, v) = points(&z);
    let cutoff = default_cutoff(&xy);
    for (key, s) in
        [("vario", sample_variogram(&xy, &v, cutoff, cutoff / 15.0)), ("vario50", sample_variogram(&xy, &v, 50.0, 5.0))]
    {
        close(&format!("{key} np"), &s.np, &z[format!("{key}/np").as_str()].data, 0.0);
        close(&format!("{key} dist"), &s.dist, &z[format!("{key}/dist").as_str()].data, 1e-13);
        close(&format!("{key} gamma"), &s.gamma, &z[format!("{key}/gamma").as_str()].data, 1e-13);
    }
}

#[test]
fn the_fits_are_gstats() {
    let z = reference();
    let (xy, v) = points(&z);
    let cutoff = default_cutoff(&xy);
    let s = sample_variogram(&xy, &v, cutoff, cutoff / 15.0);
    // R's var (its long double sums), the fits' start
    let s2 = z["s2"].data[0];
    for (key, shape) in [("Sph", Shape::Spherical), ("Exp", Shape::Exponential), ("Gau", Shape::Gaussian)] {
        let f = fit_variogram(&s, Model { shape, nugget: s2 / 10.0, psill: s2, range: 20.0 }, true, true, true);
        let got = [f.model.nugget, f.model.psill, f.model.range];
        let want = &z[format!("fit{key}/params").as_str()].data;
        // the Gaussian's fit does not converge in 200 iterations, gstat's either: its last oscillations take the
        // last bit of the libm's exp further
        close(&format!("fit {key}"), &got, want, if shape == Shape::Gaussian { 1e-5 } else { 0.0 });
    }
}

#[test]
fn idw_and_kriging_are_gstats() {
    let z = reference();
    let (xy, v) = points(&z);
    let at = centres();
    let idw2: Vec<f64> = at.iter().map(|p| idw_at(&xy, &v, *p, 2.0, None)).collect();
    close("idw 2", &idw2, &z["idw2/pred"].data, 1e-12);
    let idw25: Vec<f64> = at.iter().map(|p| idw_at(&xy, &v, *p, 2.5, Some(8))).collect();
    close("idw 2.5 nmax 8", &idw25, &z["idw25/pred"].data, 1e-12);
    let sph = Model { shape: Shape::Spherical, nugget: 0.2, psill: 1.2, range: 30.0 };
    for (key, m, nmax) in [("ok", sph, None), ("ok10", sph, Some(10))] {
        let k = krige(&xy, &v, &m, &at, nmax).unwrap();
        let (p, var): (Vec<f64>, Vec<f64>) = k.into_iter().unzip();
        close(&format!("{key} pred"), &p, &z[format!("{key}/pred").as_str()].data, 1e-10);
        close(&format!("{key} var"), &var, &z[format!("{key}/var").as_str()].data, 1e-10);
    }
    let fit = &z["fitExp/params"].data;
    let m = Model { shape: Shape::Exponential, nugget: fit[0], psill: fit[1], range: fit[2] };
    let (p, var): (Vec<f64>, Vec<f64>) = krige(&xy, &v, &m, &at, None).unwrap().into_iter().unzip();
    close("okexp pred", &p, &z["okexp/pred"].data, 1e-10);
    close("okexp var", &var, &z["okexp/var"].data, 1e-10);
}

#[test]
fn the_operators_from_points_to_a_map() {
    let z = reference();
    let (xy, v) = points(&z);
    let pts = Value::Vector(Arc::new(Vector {
        geometries: xy.iter().map(|p| Geometry::Point(*p)).collect(),
        attributes: vec![("z".to_string(), Column::Numbers(v.clone()))],
        crs: None,
    }));
    let mut like = vec![0.0; 500];
    like[7] = f64::NAN;
    let like = Value::Map(Map::new(Arc::new(grid()), like));
    let o = run(
        "stats.variogram",
        vec![("points", pts.clone())],
        serde_json::json!({"model": "exponential", "range": 20, "psill": 0.6, "nugget": 0.06}),
    );
    let Value::Table(t) = &o["table"] else { panic!() };
    assert_eq!(t.rows.len(), z["vario/np"].data.len());
    let o = run(
        "stats.kriging",
        vec![("points", pts.clone()), ("like", like.clone()), ("model", o["model"].clone())],
        serde_json::json!({}),
    );
    let Value::Map(p) = &o["prediction"] else { panic!() };
    assert!(p.data[7].is_nan());
    let Value::Map(var) = &o["variance"] else { panic!() };
    assert!(var.data.iter().filter(|x| !x.is_nan()).all(|x| *x > 0.0));
    let o = run("stats.idw", vec![("points", pts), ("like", like)], serde_json::json!({"field": "z"}));
    let Value::Map(m) = &o["map"] else { panic!() };
    let want = &z["idw2/pred"].data;
    close("idw op", &m.data[..7], &want[..7], 1e-12);
}
