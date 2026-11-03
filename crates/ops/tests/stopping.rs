//! The long operators tell how far they are and stop when the run is stopped: MSPA by tiles, the metrics by blocks
//! and in a window by strips (between two tiles, blocks or strips); Monte Carlo by draws, a metapopulation and
//! MigClim by steps, the importance of the nodes by distances.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Map, Progress, Registry, Value};
use terraflux_raster::Raster;

fn input() -> Map {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extern/landschaft/src/landschaft/data/input.tif");
    let r = Raster::read(&path).unwrap();
    Map::new(Arc::new(r.grid.clone()), r.bands[0].data.clone())
}

/// The node run with `watch` told how it goes: its error (none: it ran to its end), whether it was stopped.
fn run(
    op: &str,
    params: serde_json::Value,
    watch: &mut dyn FnMut(Option<&Progress>) -> bool,
) -> (Option<String>, bool) {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: [("map".to_string(), Value::Map(input()))].into_iter().collect(), params: &params };
    let mut ctx = Ctx::watched(watch);
    let out = r.get(op).unwrap().run(&mut ctx, &args);
    (out.err().map(|e| e.to_string()), ctx.was_stopped())
}

#[test]
fn long_operators_report_and_stop() {
    let cases = [
        ("landscape.mspa_tiled", serde_json::json!({"tile_size": 600, "overlap": 200})),
        ("metrics.landscape", serde_json::json!({"metrics": ["ta", "np"], "block_rows": 200})),
        ("metrics.window", serde_json::json!({"metrics": ["ta"], "window": 3, "block_rows": 100})),
    ];
    for (op, params) in cases {
        // told how it goes
        let mut told = Vec::new();
        let (e, stopped) = run(op, params.clone(), &mut |p| {
            told.extend(p.map(|p| p.text.clone()));
            true
        });
        assert!(e.is_none() && !stopped, "{op}: {e:?}");
        assert!(told.len() > 1, "{op}: told {told:?}");
        // stopped after the first: the node fails, stopped
        let mut seen = 0;
        let (e, stopped) = run(op, params, &mut |p| {
            seen += usize::from(p.is_some());
            seen <= 1
        });
        let e = e.unwrap_or_default();
        assert!(stopped && e.contains("stopped"), "{op}: {e}");
    }
}

/// The node of no map run with `inputs`, `watch` told how it goes: as [`run`].
fn run_with(
    op: &str,
    inputs: Vec<(&str, Value)>,
    params: serde_json::Value,
    watch: &mut dyn FnMut(Option<&Progress>) -> bool,
) -> (Option<String>, bool) {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    let mut ctx = Ctx::watched(watch);
    let out = r.get(op).unwrap().run(&mut ctx, &args);
    (out.err().map(|e| e.to_string()), ctx.was_stopped())
}

/// Monte Carlo's draws and a metapopulation's steps: told, and stopped between two.
#[test]
fn draws_and_steps_report_and_stop() {
    let patches = terraflux_ops::Table {
        columns: ["x", "y", "area", "occupied"].map(String::from).to_vec(),
        rows: (0..20)
            .map(|i| vec![f64::from(i % 5), f64::from(i / 5), 1.0 + f64::from(i % 3), f64::from(i % 2)])
            .collect(),
        ..Default::default()
    };
    let cases = [
        (
            "uncertainty.monte_carlo",
            vec![],
            serde_json::json!({"expression": "a * b", "parameters": [["a", 2.0, 0.1], ["b", 3.0, 0.2]], "n": 500}),
        ),
        (
            "conn.metapopulation",
            vec![("patches", Value::Table(patches))],
            serde_json::json!({"steps": 30, "alpha": 0.5, "x": 0.5, "y": 1.0, "e": 0.2}),
        ),
    ];
    for (op, inputs, params) in cases {
        let mut told = Vec::new();
        let (e, stopped) = run_with(op, inputs.clone(), params.clone(), &mut |p| {
            told.extend(p.map(|p| p.text.clone()));
            true
        });
        assert!(e.is_none() && !stopped, "{op}: {e:?}");
        assert!(told.len() > 10, "{op}: told {told:?}");
        let mut seen = 0;
        let (e, stopped) = run_with(op, inputs, params, &mut |p| {
            seen += usize::from(p.is_some());
            seen <= 3
        });
        let e = e.unwrap_or_default();
        assert!(stopped && e.contains("stopped"), "{op}: {e}");
    }
}
