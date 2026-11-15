//! The Python counterparts of the operators of terrain, water and ecosystem services (surface, hydro, services, carbon).
//!
//! Left without a counterpart: `raster.terrain` (gdaldem's hillshade, TRI, TPI and roughness are not the library's
//! `tf.surface.terrain`, which does slope and aspect only, with other flat-cell and longitude / latitude rules).

#![allow(unused_imports)]

use super::*;

/// The operators of this family called with their arguments mapped.
pub(super) const MAPPED: &[Py] = &[
    Py {
        op: "surface.aacf",
        call: "tf.surface.aacf",
        args: &[("map", "")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.bearing",
        call: "tf.surface.bearing_area",
        args: &[("map", "")],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.dissection",
        call: "tf.surface.dissection",
        args: &[("map", ""), ("s", "s")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.hsp",
        call: "tf.surface.hsp",
        args: &[
            ("map", ""),
            ("min_scale", "min_scale"),
            ("max_scale", "max_scale"),
            ("inc", "inc"),
            ("win", "win"),
            ("normalize", "normalize"),
        ],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.spherical_sd",
        call: "tf.surface.spherical_sd",
        args: &[("map", ""), ("d", "d"), ("variance", "variance")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.srr",
        call: "tf.surface.srr",
        args: &[("map", ""), ("s", "s")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.trasp",
        call: "tf.surface.trasp",
        args: &[("map", "")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.tri",
        call: "tf.surface.tri",
        args: &[("map", ""), ("s", "s")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.geomorphons",
        call: "tf.surface.geomorphons",
        args: &[
            ("dem", ""),
            ("search", "search"),
            ("skip", "skip"),
            ("flat", "flat"),
            ("dist", "dist"),
            ("units", "units"),
            ("comparison", "comparison"),
            ("extended", "extended"),
            ("measures", "measures"),
        ],
        defaults: &[],
        outs: &[
            ("forms", ".forms"),
            ("ternary", ".ternary"),
            ("positive", ".positive"),
            ("negative", ".negative"),
            ("share", ".share().reset_index()"),
            ("intensity", ".intensity"),
            ("exposition", ".exposition"),
            ("range", ".range"),
            ("variance", ".variance"),
            ("elongation", ".elongation"),
            ("azimuth", ".azimuth"),
            ("extend", ".extend"),
            ("width", ".width"),
        ],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.hilltop",
        call: "tf.surface.hilltop",
        args: &[
            ("dem", ""),
            ("plain", "plain"),
            ("min_height", "min_height"),
            ("min_slope", "min_slope"),
            ("slope", "slope"),
            ("fraction", "fraction"),
        ],
        defaults: &[],
        // the table's base_kind and qualifies as Terraflux's codes (0 saddle, 1 plain, 2 lowest; 0 or 1), the hill a column
        outs: &[
            ("app", ".app"),
            ("hill", ".hill"),
            (
                "hills",
                ".hills.reset_index().assign(base_kind=lambda d: d[\"base_kind\"].map({\"saddle\": 0, \"plain\": 1, \"lowest\": 2}), qualifies=lambda d: d[\"qualifies\"].astype(int))",
            ),
        ],
        kind: Kind::Plain,
    },
    Py {
        op: "hydro.along_network",
        call: "tf.hydro.along_network",
        args: &[
            ("dem", ""),
            ("points", ""),
            ("streams", ""),
            ("upstream", "upstream"),
            ("downstream", "downstream"),
            ("snap", "snap"),
            ("flats", "flats"),
        ],
        defaults: &[],
        outs: &[("map", ".map"), ("distance", ".distance")],
        kind: Kind::Plain,
    },
    Py {
        op: "hydro.dci",
        call: "tf.hydro.dci",
        args: &[
            ("dem", ""),
            ("streams", ""),
            ("barriers", ""),
            ("weights", "weights"),
            ("passability", "passability"),
            ("direction", "direction"),
            ("snap", "snap"),
            ("flats", "flats"),
        ],
        defaults: &[],
        outs: &[("table", ".table"), ("barriers", ".barriers"), ("segments", ".segments"), ("reach", ".reach")],
        kind: Kind::Plain,
    },
    Py {
        op: "hydro.snap",
        call: "tf.hydro.snap_points",
        args: &[
            ("points", ""),
            ("streams", ""),
            ("distance", "distance"),
            ("accumulation", "accumulation"),
            ("min_accumulation", "min_accumulation"),
        ],
        defaults: &[],
        outs: &[("vector", ""), ("table", ".drop(columns=\"geometry\", errors=\"ignore\")")],
        kind: Kind::Plain,
    },
    Py {
        op: "hydro.stream_links",
        call: "tf.hydro.stream_links",
        args: &[("dem", ""), ("streams", ""), ("outlets", "outlets"), ("snap", "snap"), ("flats", "flats")],
        defaults: &[],
        outs: &[("map", "[0]"), ("table", "[1]")],
        kind: Kind::Plain,
    },
    Py {
        op: "hydro.upstream",
        call: "tf.hydro.upstream",
        args: &[
            ("dem", ""),
            ("streams", "streams"),
            ("links", "links"),
            ("points", "points"),
            ("values", "values"),
            ("classes", "classes"),
            ("stats", "stats"),
            ("to_map", "to_map"),
            ("snap", "snap"),
            ("flats", "flats"),
        ],
        defaults: &[],
        outs: &[("table", ".table"), ("map", ".map")],
        kind: Kind::Plain,
    },
    Py {
        op: "hydro.upstream_catchment",
        call: "tf.hydro.upstream_catchment",
        args: &[("dem", ""), ("points", ""), ("streams", "streams"), ("snap", "snap"), ("flats", "flats")],
        defaults: &[],
        outs: &[("vector", ".polygons"), ("table", ".table"), ("map", ".map")],
        kind: Kind::Plain,
    },
    Py {
        op: "carbon.emissions",
        call: "tf.services.emissions",
        args: &[
            ("maps", ""),
            ("stock", "stock"),
            ("regrowth", "regrowth"),
            ("co2", "co2"),
            ("times", "times"),
            ("cell_area", "cell_area"),
        ],
        defaults: &[],
        outs: &[("periods", ".periods"), ("transitions", ".transitions"), ("stocks", ".stocks"), ("net", ".net")],
        kind: Kind::Plain,
    },
    Py {
        op: "services.ndr",
        call: "tf.services.ndr",
        args: &[
            ("dem", ""),
            ("lulc", ""),
            ("runoff_proxy", ""),
            ("biophysical", ""),
            ("watersheds", ""),
            ("threshold", "threshold"),
            ("k", "k"),
            ("nitrogen", "nitrogen"),
            ("phosphorus", "phosphorus"),
            ("subsurface_critical_length", "subsurface_critical_length"),
            ("subsurface_efficiency", "subsurface_efficiency"),
            ("runoff_proxy_mean", "runoff_proxy_mean"),
            ("method", "method"),
        ],
        defaults: &[],
        outs: &[
            ("n_surface_export", ".n_surface_export"),
            ("n_subsurface_export", ".n_subsurface_export"),
            ("n_total_export", ".n_total_export"),
            ("p_surface_export", ".p_surface_export"),
            ("ndr_n", ".ndr_n"),
            ("ndr_p", ".ndr_p"),
            ("effective_retention_n", ".effective_retention_n"),
            ("effective_retention_p", ".effective_retention_p"),
            ("ic", ".ic"),
            ("stream", ".stream"),
            ("drains_to_stream", ".drains_to_stream"),
            ("watershed_table", ".watersheds.drop(columns=\"geometry\", errors=\"ignore\")"),
            ("ic_0", ".ic_0"),
            ("runoff_proxy_mean", ".runoff_proxy_mean"),
        ],
        kind: Kind::Plain,
    },
    Py {
        op: "services.sdr",
        call: "tf.services.sdr",
        args: &[
            ("dem", ""),
            ("erosivity", ""),
            ("erodibility", ""),
            ("lulc", ""),
            ("biophysical", ""),
            ("watersheds", ""),
            ("drainage", "drainage"),
            ("threshold", "threshold"),
            ("k", "k"),
            ("ic_0", "ic_0"),
            ("sdr_max", "sdr_max"),
            ("l_max", "l_max"),
            ("method", "method"),
        ],
        defaults: &[],
        outs: &[
            ("usle", ".usle"),
            ("sed_export", ".sed_export"),
            ("sed_deposition", ".sed_deposition"),
            ("avoided_export", ".avoided_export"),
            ("avoided_erosion", ".avoided_erosion"),
            ("rkls", ".rkls"),
            ("sdr", ".sdr"),
            ("ic", ".ic"),
            ("stream", ".stream"),
            ("drains_to_stream", ".drains_to_stream"),
            ("watershed_table", ".watersheds.drop(columns=\"geometry\", errors=\"ignore\")"),
        ],
        kind: Kind::Plain,
    },
    Py {
        op: "services.seasonal_water_yield",
        call: "tf.services.seasonal_water_yield",
        args: &[
            ("dem", ""),
            ("lulc", ""),
            ("biophysical", ""),
            ("watersheds", ""),
            ("precipitation", "precipitation"),
            ("et0", "et0"),
            ("soil_group", "soil_group"),
            ("climate_zones", "climate_zones"),
            ("climate_zone_table", "climate_zone_table"),
            ("recharge", "recharge"),
            ("threshold", "threshold"),
            ("rain_events", "rain_events"),
            ("alpha", "alpha"),
            ("beta", "beta"),
            ("gamma", "gamma"),
            ("method", "method"),
        ],
        defaults: &[],
        outs: &[
            ("quickflow", ".quickflow"),
            ("local_recharge", ".local_recharge"),
            ("l_avail", ".l_avail"),
            ("l_sum_avail", ".l_sum_avail"),
            ("l_sum", ".l_sum"),
            ("baseflow", ".baseflow"),
            ("b_sum", ".b_sum"),
            ("vri", ".vri"),
            ("aet", ".aet"),
            ("annual_precipitation", ".precipitation"),
            ("stream", ".stream"),
            ("quickflow_monthly", ".intermediate[\"quickflow_monthly\"]"),
            ("watershed_table", ".watersheds.drop(columns=\"geometry\", errors=\"ignore\")"),
            ("monthly_table", ".monthly"),
        ],
        kind: Kind::Plain,
    },
    Py {
        op: "services.water_yield",
        call: "tf.services.water_yield",
        args: &[
            ("lulc", ""),
            ("biophysical", ""),
            ("precipitation", ""),
            ("eto", ""),
            ("depth", ""),
            ("pawc", ""),
            ("seasonality", "seasonality"),
            ("watersheds", "watersheds"),
            ("subwatersheds", "subwatersheds"),
            ("demand", "demand"),
            ("valuation", "valuation"),
        ],
        defaults: &[],
        outs: &[
            ("wyield", ".wyield"),
            ("aet", ".aet"),
            ("pet", ".pet"),
            ("fractp", ".fractp"),
            ("demand_map", ".demand"),
            ("watershed_table", ".watersheds.drop(columns=\"geometry\", errors=\"ignore\")"),
            ("subwatershed_table", ".subwatersheds.drop(columns=\"geometry\", errors=\"ignore\")"),
        ],
        kind: Kind::Plain,
    },
];

/// The call's arguments: `pos` (inputs or settings) by position, then `named` as `theirs=value`; and a comment for
/// each input or setting of the node left out (neither in these nor in `handled`).
fn arguments(
    n: &Node,
    sc: &Scope,
    pos: &[&str],
    named: &[(&str, &str)],
    handled: &[&str],
) -> Result<(String, Vec<String>), String> {
    let mut args = Vec::new();
    for p in pos {
        args.push(value_of(n, p, sc)?.ok_or_else(|| format!("its {p} is not connected"))?);
    }
    for (ours, theirs) in named {
        if let Some(v) = value_of(n, ours, sc)? {
            args.push(format!("{theirs}={v}"));
        }
    }
    let known = |k: &str| pos.contains(&k) || handled.contains(&k) || named.iter().any(|(o, _)| *o == k);
    let mut notes = String::new();
    for (k, v) in &n.params {
        if !known(k) {
            notes.push_str(&format!("# (not passed: {k} = {})\n", literal(v)));
        }
    }
    for k in n.inputs.keys().filter(|k| !known(k)) {
        notes.push_str(&format!("# (not passed: the input {k})\n"));
    }
    Ok((notes, args))
}

/// A table setting as a DataFrame: a reference, a file (`tf.load_table`) or a list of objects (a row each, the
/// names of their keys lower case as columns).
fn table_of(n: &Node, name: &str, sc: &Scope) -> Result<Option<String>, String> {
    let Some(v) = n.params.get(name) else { return Ok(None) };
    if terraflux_graph::setting_ref(v).is_some() {
        return setting(n, name, sc);
    }
    match v {
        Value::String(s) => Ok(Some(format!("tf.load_table({})", path(s)))),
        Value::Array(rows) => {
            let mut columns: Vec<String> = Vec::new();
            let mut objects = Vec::new();
            for r in rows {
                let o = r.as_object().ok_or_else(|| format!("its {name}: each row an object"))?;
                let o: Vec<(String, &Value)> = o.iter().map(|(k, v)| (k.trim().to_lowercase(), v)).collect();
                for (k, _) in &o {
                    if !columns.contains(k) {
                        columns.push(k.clone());
                    }
                }
                objects.push(o);
            }
            let rows: Vec<String> = objects
                .iter()
                .map(|o| {
                    let cells: Vec<String> = columns
                        .iter()
                        .map(|c| o.iter().find(|(k, _)| k == c).map_or("None".into(), |(_, v)| literal(v)))
                        .collect();
                    format!("[{}]", cells.join(", "))
                })
                .collect();
            let names: Vec<String> = columns.iter().map(|c| text(c)).collect();
            Ok(Some(format!("_table([{}], [{}])", names.join(", "), rows.join(", "))))
        }
        _ => Err(format!("its {name}: a file or a list of rows")),
    }
}

fn outs(pairs: &[(&str, String)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

/// The operators of this family translated by code of their own (as `special` in python.rs); `None`: not one of them.
pub(super) fn special(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    let one = |code: String, port: &str, expr: String, need: Need| -> Result<Option<Code>, String> {
        Ok(Some((code, outs(&[(port, expr)]), need)))
    };
    match n.op.as_str() {
        "surface.window" => {
            // the library takes the metrics always: every one by default
            let (notes, mut args) =
                arguments(n, sc, &["map"], &[("window", "window"), ("shape", "shape")], &["metrics"])?;
            let metrics = value_of(n, "metrics", sc)?
                .unwrap_or_else(|| "tf.surface.list_metrics()[\"metric\"].tolist()".into());
            args.insert(1, metrics);
            let code = format!("{notes}{var} = tf.surface.window_metrics({})\n", args.join(", "));
            one(code, "maps", format!("_stack({{k: {var}[k] for k in {var}.data_vars}})"), Need::Flow)
        }
        "surface.metrics" => {
            // a Series by metric: the table of one row
            let (notes, mut args) = arguments(n, sc, &["map"], &[], &["metrics"])?;
            if let Some(m) = value_of(n, "metrics", sc)? {
                args.push(m);
            }
            let code = format!(
                "{notes}{var} = tf.surface.metrics({}).to_frame().T.reset_index(drop=True)\n",
                args.join(", ")
            );
            one(code, "table", var.to_string(), Need::Nothing)
        }
        "surface.peaks" => {
            let (notes, args) = arguments(n, sc, &["map"], &[], &["valleys"])?;
            let call = match setting(n, "valleys", sc)?.as_deref() {
                None | Some("False") => "tf.surface.find_peaks".to_string(),
                Some("True") => "tf.surface.find_valleys".to_string(),
                Some(e) => format!("(tf.surface.find_valleys if {e} else tf.surface.find_peaks)"),
            };
            one(format!("{notes}{var} = {call}({})\n", args.join(", ")), "table", var.to_string(), Need::Nothing)
        }
        "surface.fit_plane" => {
            let (notes, args) = arguments(n, sc, &["map"], &[], &["order"])?;
            let m = args[0].clone();
            let call = match setting(n, "order", sc)?.as_deref() {
                None | Some("\"best\"") => format!("tf.surface.best_fit_plane({m})"),
                Some(o) if o.parse::<u8>().is_ok() => format!("tf.surface.fit_plane({m}, {o})"),
                Some(o) => {
                    format!("(tf.surface.best_fit_plane({m}) if {o} == \"best\" else tf.surface.fit_plane({m}, {o}))")
                }
            };
            let code = format!("{notes}{var} = {call}\n");
            let o = outs(&[("plane", var.to_string()), ("residuals", format!("({m} - {var})"))]);
            Ok(Some((code, o, Need::Nothing)))
        }
        "raster.viewshed" => {
            let (notes, mut args) = arguments(
                n,
                sc,
                &["map"],
                &[
                    ("observer_height", "observer_height"),
                    ("target_height", "target_height"),
                    ("radius", "radius"),
                    ("curvature", "curvature"),
                ],
                &["points", "x", "y"],
            )?;
            let observers = match (value_of(n, "points", sc)?, value_of(n, "x", sc)?, value_of(n, "y", sc)?) {
                (Some(p), _, _) => p,
                (None, Some(x), Some(y)) => format!("({x}, {y})"),
                _ => return Err("give the observers: points, or x and y".into()),
            };
            args.insert(1, observers);
            let code = format!(
                "{notes}# (on a DEM in longitude / latitude the library measures the radius in metres)\n{var} = tf.surface.viewshed({})\n",
                args.join(", ")
            );
            one(code, "map", var.to_string(), Need::Nothing)
        }
        "services.carbon" => {
            let (notes, args) = arguments(
                n,
                sc,
                &["lulc", "pools"],
                &[
                    ("future", "future"),
                    ("price", "price"),
                    ("discount_rate", "discount_rate"),
                    ("rate_change", "rate_change"),
                    ("year", "year"),
                    ("future_year", "future_year"),
                ],
                &[],
            )?;
            let code = format!("{notes}{var} = tf.services.carbon({})\n", args.join(", "));
            let o = outs(&[
                ("storage", format!("{var}.storage")),
                ("pools", format!("_stack({var}.pools)")),
                ("storage_future", format!("{var}.storage_future")),
                ("pools_future", format!("_stack({var}.pools_future)")),
                ("change", format!("{var}.change")),
                ("npv", format!("{var}.npv")),
                ("totals", format!("{var}.totals")),
            ]);
            Ok(Some((code, o, Need::Flow)))
        }
        "services.habitat_quality" => {
            let (notes, mut args) = arguments(
                n,
                sc,
                &["lulc"],
                &[
                    ("future", "future"),
                    ("future_threat_maps", "future_threat_maps"),
                    ("base", "base"),
                    ("base_threat_maps", "base_threat_maps"),
                    ("access", "access"),
                    ("half_saturation", "half_saturation"),
                ],
                &["threats", "sensitivity", "threat_maps"],
            )?;
            let threats = table_of(n, "threats", sc)?.ok_or("give the threats")?;
            let sensitivity = value_of(n, "sensitivity", sc)?.ok_or("its sensitivity table is not connected")?;
            let maps = value_of(n, "threat_maps", sc)?.ok_or("its threat_maps are not connected")?;
            args.splice(1..1, [threats, sensitivity, maps]);
            let code = format!("{notes}{var} = tf.services.habitat_quality({})\n", args.join(", "));
            let by = |field: &str, scenario: &str| format!("{var}.{field}[\"{scenario}\"]");
            let o = outs(&[
                ("quality", by("quality", "current")),
                ("degradation", by("degradation", "current")),
                ("threats", format!("_stack({})", by("threats", "current"))),
                ("quality_future", by("quality", "future")),
                ("degradation_future", by("degradation", "future")),
                ("quality_base", by("quality", "base")),
                ("degradation_base", by("degradation", "base")),
                ("rarity", by("rarity", "current")),
                ("rarity_future", by("rarity", "future")),
                ("rarity_table", format!("{var}.rarity_table")),
            ]);
            Ok(Some((code, o, Need::Flow)))
        }
        "services.pollination" => {
            let (notes, mut args) = arguments(n, sc, &["lulc"], &[("farms", "farms")], &["guilds", "biophysical"])?;
            let guilds = table_of(n, "guilds", sc)?.ok_or("give the guilds")?;
            let bio = value_of(n, "biophysical", sc)?.ok_or("its biophysical table is not connected")?;
            args.splice(1..1, [guilds, bio]);
            let code = format!("{notes}{var} = tf.services.pollination({})\n", args.join(", "));
            let o = outs(&[
                ("supply", format!("_stack({var}.supply)")),
                (
                    "abundance",
                    format!(
                        "_stack({{f\"{{sp}}_{{se}}\": a for sp, d in {var}.abundance.items() for se, a in d.items()}})"
                    ),
                ),
                ("total_abundance", format!("_stack({var}.total_abundance)")),
                ("farm_pollinators", format!("{var}.farm_pollinators")),
                ("total_yield", format!("{var}.total_yield")),
                ("wild_yield", format!("{var}.wild_yield")),
                (
                    "farm_table",
                    format!("{var}.farms[[\"p_abund\", \"y_tot\", \"pdep_y_w\", \"y_wild\"]].reset_index(drop=True)"),
                ),
            ]);
            Ok(Some((code, o, Need::Flow)))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The script parses as Python (when there is a Python to ask).
    fn parses(t: &str) {
        if let Ok(o) = std::process::Command::new("python")
            .args(["-I", "-c", "import ast, sys; ast.parse(sys.stdin.read())"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.take().unwrap().write_all(t.as_bytes())?;
                c.wait_with_output()
            })
        {
            assert!(o.status.success(), "{}\n{t}", String::from_utf8_lossy(&o.stderr));
        }
    }

    #[test]
    fn a_river_network_and_the_landforms() {
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "dem"
op = "nlm.mpd"

[[nodes]]
id = "fill"
op = "hydro.fill"
inputs = { dem = "dem.map" }

[[nodes]]
id = "acc"
op = "hydro.flow_accumulation"
inputs = { dem = "fill.map" }

[[nodes]]
id = "rivers"
op = "hydro.streams"
inputs = { accumulation = "acc.map" }
params = { threshold = 50 }

[[nodes]]
id = "links"
op = "hydro.stream_links"
inputs = { dem = "fill.map", streams = "rivers.map" }

[[nodes]]
id = "up"
op = "hydro.upstream"
inputs = { dem = "fill.map", links = "links.map", values = "dem.map" }
params = { stats = ["mean", "max"] }

[[nodes]]
id = "forms"
op = "surface.geomorphons"
inputs = { dem = "dem.map" }
params = { search = 10 }

[[nodes]]
id = "rough"
op = "surface.tri"
inputs = { map = "dem.map" }
params = { s = 5 }

[[nodes]]
id = "peaks"
op = "surface.peaks"
inputs = { map = "dem.map" }
params = { valleys = true }

[[nodes]]
id = "plane"
op = "surface.fit_plane"
inputs = { map = "dem.map" }

[[nodes]]
id = "window"
op = "surface.window"
inputs = { map = "plane.residuals" }
params = { metrics = ["sa", "sq"], window = 7 }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &Registry::standard(), None);
        assert!(s.left.is_empty(), "left: {:?}", s.left);
        let t = &s.text;
        assert!(t.contains("tf.hydro.stream_links("), "{t}");
        assert!(t.contains("tf.hydro.upstream(") && t.contains("stats=[\"mean\", \"max\"]"), "{t}");
        assert!(t.contains("tf.surface.geomorphons(") && t.contains("search=10"), "{t}");
        assert!(t.contains("tf.surface.tri(") && t.contains("s=5"), "{t}");
        assert!(t.contains("tf.surface.find_valleys("), "{t}");
        assert!(t.contains("tf.surface.best_fit_plane("), "{t}");
        assert!(t.contains("window_metrics(") && t.contains("window=7"), "{t}");
        parses(t);
    }

    #[test]
    fn an_invest_model() {
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "nlm.random"

[[nodes]]
id = "pools"
op = "table.read"
params = { path = "pools.csv" }

[[nodes]]
id = "carbon"
op = "services.carbon"
inputs = { lulc = "lulc.map", pools = "pools.table" }

[[nodes]]
id = "sens"
op = "table.read"
params = { path = "sens.csv" }

[[nodes]]
id = "road"
op = "nlm.random"

[[nodes]]
id = "threat_maps"
op = "stack.of"
inputs = { road = "road.map" }

[[nodes]]
id = "hq"
op = "services.habitat_quality"
inputs = { lulc = "lulc.map", sensitivity = "sens.table", threat_maps = "threat_maps.stack" }
params = { half_saturation = 0.1, threats = [{ threat = "road", max_dist = 600, weight = 1.0, decay = "linear" }] }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &Registry::standard(), None);
        assert!(s.left.is_empty(), "left: {:?}", s.left);
        let t = &s.text;
        assert!(t.contains("tf.services.carbon("), "{t}");
        assert!(t.contains("tf.services.habitat_quality("), "{t}");
        // the keys of a row in the model file's order (sorted), the library takes the columns by name
        assert!(t.contains("_table([\"decay\", \"max_dist\", \"threat\", \"weight\"], [[\"linear\", 600, \"road\", 1.0]])"), "{t}");
        parses(t);
    }
}
