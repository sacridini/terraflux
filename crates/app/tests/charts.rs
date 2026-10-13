//! The table viewer's charts drawn off screen (wgpu) from the tables of the signature analysis, a picture of each
//! in the temporary folder (terraflux-chart-*.png): several metrics at once, a clustergram, a correlation matrix.

use std::path::Path;

use egui_kittest::Harness;
use terraflux_app::charts::{Chart, Kind};
use terraflux_app::table_view::{TableState, show};
use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};

/// landschaft's signature table of augusta.tif (tests/data/reference/lsm_signatures.json), its zones as keys.
fn signatures() -> Table {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/lsm_signatures.json");
    let r: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut columns = vec!["zone".to_string()];
    columns.extend(r["columns"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()));
    let rows = r["zones"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["table"].as_array().unwrap())
        .map(|(z, row)| {
            std::iter::once(z.as_f64().unwrap())
                .chain(row.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()))
                .collect()
        })
        .collect();
    Table { columns, rows }
}

fn run(op: &str, t: &Table, params: serde_json::Value) -> Outputs {
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args {
        node: "n",
        inputs: [("table".to_string(), Value::Table(t.clone()))].into_iter().collect(),
        params: &params,
    };
    Registry::standard().get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
}

fn table(out: &Outputs, port: &str) -> Table {
    let Value::Table(t) = &out[port] else { panic!() };
    t.clone()
}

/// A table shown with a chart, drawn, and its picture saved.
fn draw(t: Table, chart: Chart, name: &str) {
    let id = egui::Id::new(("chart test", name));
    let st = TableState { chart, ..Default::default() };
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        eframe::egui_wgpu::RendererOptions::PREDICTABLE,
    );
    let mut h = Harness::builder()
        .with_size((1100.0, 760.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui(move |ui| {
            terraflux_app::theme::apply(ui.ctx());
            show(ui, &t, id);
        });
    h.ctx.data_mut(|d| d.insert_temp(id, st.clone()));
    h.run_steps(4);
    assert_eq!(h.ctx.data(|d| d.get_temp::<TableState>(id)).map(|s| s.chart), Some(st.chart), "{name}");
    match h.render() {
        Ok(img) => img.save(std::env::temp_dir().join(format!("terraflux-chart-{name}.png"))).unwrap(),
        Err(e) => eprintln!("no renderer ({e}): {name} not drawn"),
    }
}

#[test]
fn charts_of_signatures() {
    let s = signatures();
    let col = |t: &Table, c: &str| t.columns.iter().position(|x| x == c).unwrap();
    // several metrics at once, as bars by zone
    let bars =
        Chart { kind: Kind::Bars, columns: vec![col(&s, "shdi"), col(&s, "ed"), col(&s, "np")], x: Some(0), by: None };
    draw(s.clone(), bars, "bars");
    // the zones' metrics against each other, coloured by their cluster
    let clusters = table(&run("metrics.cluster", &s, serde_json::json!({"k": 4})), "table");
    let points = Chart {
        kind: Kind::Points,
        columns: vec![col(&clusters, "ed"), col(&clusters, "pland_41")],
        x: Some(col(&clusters, "shdi")),
        by: Some(col(&clusters, "cluster")),
    };
    draw(clusters, points, "points");
    // the clustergram of the k-means' means
    let means = table(&run("metrics.clustergram", &s, serde_json::json!({"k_max": 6})), "means");
    assert!(Kind::of(&means).contains(&Kind::Clustergram));
    draw(means, Chart { kind: Kind::Clustergram, ..Default::default() }, "clustergram");
    // the correlation matrix
    let cor = table(&run("metrics.correlation", &s, serde_json::json!({})), "table");
    assert!(Kind::of(&cor).contains(&Kind::Matrix));
    draw(cor, Chart { kind: Kind::Matrix, ..Default::default() }, "matrix");
    // a series: a metric per class along the dates
    let series = Table {
        columns: vec!["class".into(), "date".into(), "pland".into(), "np".into()],
        rows: (0..3)
            .flat_map(|c| {
                (0..5).map(move |d| {
                    vec![
                        f64::from(c + 1),
                        2000.0 + 5.0 * f64::from(d),
                        10.0 + f64::from(c * 20 + d * (c + 1)),
                        f64::from(40 - d * c),
                    ]
                })
            })
            .collect(),
    };
    let lines = Chart::suggested(&series, Kind::Lines);
    assert_eq!((lines.x, lines.by), (Some(1), Some(0)));
    let lines = Chart { columns: vec![2, 3], ..lines };
    draw(series, lines, "lines");
}
