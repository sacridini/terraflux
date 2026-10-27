//! The long operators tell how far they are and stop when the run is stopped: MSPA by tiles, the metrics by blocks
//! and in a window by strips (between two tiles, blocks or strips).

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
