//! The Python counterparts of the operators of connectivity (conn, circuit, networks).

#![allow(unused_imports)]

use super::*;

/// The operators of this family called with their arguments mapped.
pub(super) const MAPPED: &[Py] = &[
    // --- the habitat graph and its analyses (methods of tf.conn.HabitatGraph) ---
    Py {
        op: "conn.centrality",
        call: "tf.conn.HabitatGraph.centrality",
        args: &[("graph", ""), ("distance", "distance"), ("probability", "probability"), ("binary", "binary")],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Method,
    },
    Py {
        op: "conn.overall",
        call: "tf.conn.HabitatGraph.overall",
        args: &[
            ("graph", ""),
            ("metrics", "metrics"),
            ("distance", "distance"),
            ("probability", "probability"),
            ("pij_min", "pij_min"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Method,
    },
    Py {
        op: "conn.node_importance",
        call: "tf.conn.HabitatGraph.node_importance",
        args: &[
            ("graph", ""),
            ("metrics", "metrics"),
            ("distance", "distance"),
            ("probability", "probability"),
            ("pij_min", "pij_min"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Method,
    },
    Py {
        op: "conn.link_importance",
        call: "tf.conn.HabitatGraph.link_importance",
        args: &[
            ("graph", ""),
            ("metric", "metric"),
            ("distance", "distance"),
            ("probability", "probability"),
            ("pij_min", "pij_min"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Method,
    },
    Py {
        op: "conn.link_change",
        call: "tf.conn.HabitatGraph.link_change",
        args: &[
            ("graph", ""),
            ("changes", "changes"),
            ("metric", "metric"),
            ("distance", "distance"),
            ("probability", "probability"),
            ("pij_min", "pij_min"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Method,
    },
    // --- graphs from rasters, features and settlement probabilities ---
    Py {
        op: "conn.graph_from_raster",
        call: "tf.conn.HabitatGraph.from_raster",
        args: &[
            ("map", ""),
            ("habitat", "habitat"),
            ("directions", "directions"),
            ("attribute", "attribute"),
            ("weighted", "weighted"),
            ("area_unit", "area_unit"),
            ("resistance", "resistance"),
            ("distance", "distance"),
            ("max_distance", "max_distance"),
            ("distance_unit", "distance_unit"),
            ("cost_directions", "cost_directions"),
            ("cost_function", "cost_function"),
            ("unreachable", "unreachable"),
            ("kind", "kind"),
            ("convention", "convention"),
            ("landscape_area", "landscape_area"),
            ("restoration", "restoration"),
        ],
        defaults: &[],
        outs: &[("graph", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "conn.graph_from_vector",
        call: "tf.conn.HabitatGraph.from_vector",
        args: &[
            ("vector", ""),
            ("id", "id"),
            ("attribute", "attribute"),
            ("weighted", "weighted"),
            ("area_unit", "area_unit"),
            ("resistance", "resistance"),
            ("distance", "distance"),
            ("max_distance", "max_distance"),
            ("distance_unit", "distance_unit"),
            ("cost_directions", "cost_directions"),
            ("cost_function", "cost_function"),
            ("unreachable", "unreachable"),
            ("kind", "kind"),
            ("convention", "convention"),
            ("landscape_area", "landscape_area"),
            ("restoration", "restoration"),
        ],
        defaults: &[],
        outs: &[("graph", "")],
        kind: Kind::Plain,
    },
    // (its `kind` is always probability, and from_samc has no `weighted`: both are left out)
    Py {
        op: "conn.graph_from_samc",
        call: "tf.conn.HabitatGraph.from_samc",
        args: &[
            ("vector", ""),
            ("resistance", "resistance"),
            ("absorption", "absorption"),
            ("dispersal_distance", "dispersal_distance"),
            ("directions", "directions"),
            ("cost_function", "cost_function"),
            ("vertices", "vertices"),
            ("vertex_fraction", "vertex_fraction"),
            ("settle", "settle"),
            ("id", "id"),
            ("attribute", "attribute"),
            ("area_unit", "area_unit"),
            ("convention", "convention"),
            ("landscape_area", "landscape_area"),
        ],
        defaults: &[],
        outs: &[("graph", "")],
        kind: Kind::Plain,
    },
    // --- nodes, distances, settlement ---
    Py {
        op: "conn.nodes",
        call: "tf.conn.node_table",
        args: &[
            ("map", ""),
            ("vector", ""),
            ("habitat", "habitat"),
            ("directions", "directions"),
            ("id", "id"),
            ("attribute", "attribute"),
            ("weighted", "weighted"),
            ("area_unit", "area_unit"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "conn.distances",
        call: "tf.conn.distance_table",
        args: &[
            ("map", ""),
            ("vector", ""),
            ("resistance", "resistance"),
            ("habitat", "habitat"),
            ("directions", "directions"),
            ("id", "id"),
            ("distance", "distance"),
            ("max_distance", "max_distance"),
            ("distance_unit", "distance_unit"),
            ("cost_directions", "cost_directions"),
            ("cost_function", "cost_function"),
            ("unreachable", "unreachable"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "conn.settlement_probabilities",
        call: "tf.conn.settlement_probabilities",
        args: &[
            ("vector", ""),
            ("resistance", "resistance"),
            ("absorption", "absorption"),
            ("dispersal_distance", "dispersal_distance"),
            ("directions", "directions"),
            ("cost_function", "cost_function"),
            ("vertices", "vertices"),
            ("vertex_fraction", "vertex_fraction"),
            ("settle", "settle"),
            ("id", "id"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    // --- ECA over time, focal nodes, metapopulations ---
    // (the library asks for the distance: None is Makurhini's median when the node gives none)
    Py {
        op: "conn.eca_change",
        call: "tf.conn.eca_change",
        args: &[
            ("graphs", ""),
            ("metric", "metric"),
            ("distance", "distance"),
            ("landscape_area", "landscape_area"),
            ("probability", "probability"),
            ("pij_min", "pij_min"),
        ],
        defaults: &[("distance", "None")],
        outs: &[("table", ""), ("readings", "[[\"comparison\", \"change\"]]")],
        kind: Kind::Plain,
    },
    Py {
        op: "conn.focal_nodes",
        call: "tf.conn.focal_nodes",
        args: &[
            ("vector", ""),
            ("distance", "distance"),
            ("search_buffer", "search_buffer"),
            ("metric", "metric"),
            ("probability", "probability"),
            ("pij_min", "pij_min"),
            ("id", "id"),
            ("attribute", "attribute"),
            ("area_unit", "area_unit"),
            ("threshold", "threshold"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "conn.metapopulation",
        call: "tf.conn.metapopulation",
        args: &[
            ("patches", ""),
            ("steps", "steps"),
            ("alpha", "alpha"),
            ("x", "x"),
            ("y", "y"),
            ("e", "e"),
            ("b", "b"),
            ("kernel", "kernel"),
            ("beta", "beta"),
            ("connectivity", "connectivity"),
            ("c1", "c1"),
            ("colonisation", "colonisation"),
            ("c2", "c2"),
            ("z", "z"),
            ("extinction", "extinction"),
            ("r", "r"),
            ("area", "area"),
            ("occupied", "occupied"),
            ("seed", "seed"),
        ],
        defaults: &[],
        outs: &[("patches", ".patches"), ("table", ".table")],
        kind: Kind::Plain,
    },
    // --- ProtConn (conn.protconn has its own code: its `delta` makes the call give two values) ---
    Py {
        op: "conn.protconn_regions",
        call: "tf.conn.protconn_regions",
        args: &[
            ("protected", "protected_areas"),
            ("regions", "regions"),
            ("resistance", "resistance"),
            ("id", "id"),
            ("distance", "distance"),
            ("probability", "probability"),
            ("transboundary", "transboundary"),
            ("transboundary_type", "transboundary_type"),
            ("bound", "bound"),
            ("area_unit", "area_unit"),
            ("distance_type", "distance_type"),
            ("transboundary_weight", "transboundary_weight"),
            ("cost_directions", "cost_directions"),
            ("cost_function", "cost_function"),
            ("preselect", "preselect"),
            ("pij_min", "pij_min"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "conn.protconn_raster",
        call: "tf.conn.protconn_raster",
        args: &[
            ("protected", "protected_areas"),
            ("region", "region"),
            ("region_map", "region"),
            ("distance", "distance"),
            ("probability", "probability"),
            ("transboundary", "transboundary"),
            ("transboundary_type", "transboundary_type"),
            ("bound", "bound"),
            ("area_unit", "area_unit"),
            ("pij_min", "pij_min"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "conn.connectivity_grid",
        call: "tf.conn.connectivity_grid",
        args: &[
            ("protected", "protected_areas"),
            ("region", "region"),
            ("grid", "grid"),
            ("cell_area", "cell_area"),
            ("hexagonal", "hexagonal"),
            ("distance", "distance"),
            ("protconn_index", "protconn_index"),
            ("probability", "probability"),
            ("transboundary", "transboundary"),
            ("area_unit", "area_unit"),
            ("pij_min", "pij_min"),
        ],
        defaults: &[],
        outs: &[("grid", "")],
        kind: Kind::Plain,
    },
    // --- circuit theory ---
    Py {
        op: "circuit.advanced",
        call: "tf.circuit.advanced",
        args: &[("conductance", ""), ("source", "source"), ("ground", "ground"), ("four_neighbors", "four_neighbors")],
        defaults: &[],
        outs: &[("current", ".current"), ("voltage", ".voltage")],
        kind: Kind::Plain,
    },
    Py {
        op: "circuit.omniscape",
        call: "tf.circuit.omniscape",
        args: &[
            ("resistance", ""),
            ("radius", "radius"),
            ("source", "source"),
            ("block_size", "block_size"),
            ("buffer", "buffer"),
            ("source_threshold", "source_threshold"),
            ("r_cutoff", "r_cutoff"),
            ("resistance_is_conductance", "resistance_is_conductance"),
            ("four_neighbors", "four_neighbors"),
            ("flow_potential", "flow_potential"),
            ("normalized_current", "normalized_current"),
            ("correct_artifacts", "correct_artifacts"),
            ("mask_nodata", "mask_nodata"),
            ("reclass_table", "reclass_table"),
            ("condition1", "condition1"),
            ("condition2", "condition2"),
            ("condition1_future", "condition1_future"),
            ("condition2_future", "condition2_future"),
            ("compare_to_future", "compare_to_future"),
            ("comparison1", "comparison1"),
            ("comparison2", "comparison2"),
            ("condition1_lower", "condition1_lower"),
            ("condition1_upper", "condition1_upper"),
            ("condition2_lower", "condition2_lower"),
            ("condition2_upper", "condition2_upper"),
            ("precision", "precision"),
        ],
        defaults: &[],
        outs: &[
            ("current", "[\"cum_currmap\"]"),
            ("flow_potential", "[\"flow_potential\"]"),
            ("normalized", "[\"normalized_cum_currmap\"]"),
        ],
        kind: Kind::Plain,
    },
    // (the INI's path is the model's folder's, as every `path` setting)
    Py {
        op: "circuit.omniscape_ini",
        call: "tf.circuit.omniscape_ini",
        args: &[("path", ""), ("write_outputs", "write_outputs")],
        defaults: &[],
        outs: &[
            ("current", "[\"cum_currmap\"]"),
            ("flow_potential", "[\"flow_potential\"]"),
            ("normalized", "[\"normalized_cum_currmap\"]"),
            ("classified_resistance", "[\"classified_resistance\"]"),
        ],
        kind: Kind::Plain,
    },
    // (the INI's raster maps are arrays, not georeferenced; its resistances and maps (a matrix, a dict) are left out)
    Py {
        op: "circuit.circuitscape_ini",
        call: "tf.circuit.circuitscape_ini",
        args: &[("path", ""), ("write_outputs", "write_outputs")],
        defaults: &[],
        outs: &[("voltage", ".voltage"), ("current", ".current"), ("cum", ".cum_current"), ("max", ".max_current")],
        kind: Kind::Plain,
    },
    // --- the habitat networks of MSPA (GuidosToolbox) ---
    Py {
        op: "networks.conefor",
        call: "tf.conefor_inputs",
        args: &[("map", "")],
        defaults: &[],
        outs: &[("nodes", "[0]"), ("links", "[1].rename(columns={\"link_id\": \"link\", \"node_id\": \"node\"})")],
        kind: Kind::Plain,
    },
    Py {
        op: "networks.gtn_components",
        call: "tf.gtn_components",
        args: &[("map", ""), ("pixel_size", "pixel_size")],
        defaults: &[],
        outs: &[
            ("map", ".colored_map"),
            ("components", ".components"),
            (
                "stats",
                ".stats.rename(columns={\"component_id\": \"component\", \"core_pixels\": \"core\", \"bridge_pixels\": \"bridge\", \"total_pixels\": \"total\"})",
            ),
        ],
        kind: Kind::Plain,
    },
    Py {
        op: "networks.gtn_importance",
        call: "tf.gtn_importance",
        args: &[("map", "")],
        defaults: &[],
        outs: &[
            ("map", ".colored_map"),
            ("importance", ".importance_map"),
            ("ids", ".node_link_ids"),
            (
                "links",
                ".links.rename(columns={\"link_id\": \"link\", \"component_id\": \"component\", \"connected_nodes\": \"nodes\"})",
            ),
            ("nodes", ".nodes.rename(columns={\"node_id\": \"node\", \"component_id\": \"component\"})"),
        ],
        kind: Kind::Plain,
    },
];

/// The operators of this family translated by code of their own (as `special` in python.rs); `None`: not one of them.
pub(super) fn special(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    let code = match n.op.as_str() {
        "conn.components" => components(n, var, sc)?,
        "conn.graph" => graph(n, var, sc)?,
        "conn.read_conefor" => read_conefor(n, var, sc)?,
        "conn.write_conefor" => write_conefor(n, var, sc)?,
        "conn.to_map" => to_nodes(n, var, sc, "to_raster", "map")?,
        "conn.to_vector" => to_nodes(n, var, sc, "to_geodataframe", "vector")?,
        "conn.protconn" => protconn(n, var, sc)?,
        "conn.eca_change" => eca_change(n, var, sc)?,
        "circuit.pairwise" => circuit_raster(n, var, sc, false)?,
        "circuit.one_to_all" => circuit_raster(n, var, sc, true)?,
        "circuit.network_advanced" => network_advanced(n, var, sc)?,
        "circuit.network_pairwise" => network_pairwise(n, var, sc)?,
        _ => return Ok(None),
    };
    Ok(Some(code))
}

/// conn.eca_change: the library wants a list of graphs, Terraflux's node takes one graph or a series. (The MAPPED
/// entry stays for the tests that check the names; this runs first.)
fn eca_change(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let g = need(n, sc, "graphs", "its graphs")?;
    let names = [
        ("metric", "metric"),
        ("distance", "distance"),
        ("landscape_area", "landscape_area"),
        ("probability", "probability"),
        ("pij_min", "pij_min"),
    ];
    let mut args = vec![format!("{g} if isinstance({g}, (list, tuple)) else [{g}]")];
    args.extend(kw(n, sc, &names)?);
    if !n.params.contains_key("distance") {
        args.push("distance=None".into());
    }
    let mut used: Vec<&str> = names.iter().map(|(k, _)| *k).collect();
    used.push("graphs");
    let code = format!(
        "{}{var} = tf.conn.eca_change({})
",
        unpassed(n, &used),
        args.join(", ")
    );
    let o = outs([("table", var.to_string()), ("readings", format!("{var}[[\"comparison\", \"change\"]]"))]);
    Ok((code, o, Need::Nothing))
}

/// A node's value (input or setting) that must be there.
fn need(n: &Node, sc: &Scope, name: &str, what: &str) -> Result<String, String> {
    value_of(n, name, sc)?.ok_or_else(|| format!("{what} is not connected"))
}

/// The node's inputs and settings given among `names`, as `theirs=value`.
fn kw(n: &Node, sc: &Scope, names: &[(&str, &str)]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for (ours, theirs) in names {
        if let Some(v) = value_of(n, ours, sc)? {
            out.push(format!("{theirs}={v}"));
        }
    }
    Ok(out)
}

/// The settings and inputs outside `used`, as the comments that say they were not passed.
fn unpassed(n: &Node, used: &[&str]) -> String {
    let mut s = String::new();
    for k in n.inputs.keys().filter(|k| !used.contains(&k.as_str())) {
        s.push_str(&format!("# (not passed: the input {k})\n"));
    }
    for (k, v) in n.params.iter().filter(|(k, _)| !used.contains(&k.as_str())) {
        s.push_str(&format!("# (not passed: {k} = {})\n", literal(v)));
    }
    s
}

/// A setting that is the path of a file: relative to the model's folder (None: not given, or empty).
fn file(n: &Node, name: &str, sc: &Scope) -> Result<Option<String>, String> {
    match n.params.get(name) {
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(v @ Value::String(s)) if terraflux_graph::setting_ref(v).is_none() => Ok(Some(path(s))),
        _ => setting(n, name, sc),
    }
}

/// The outputs of a node: each port and its expression.
fn outs<const N: usize>(ports: [(&str, String); N]) -> HashMap<String, String> {
    ports.into_iter().map(|(p, e)| (p.to_string(), e)).collect()
}

/// A literal `true` / `false` setting; an error when it is a value known only on running.
fn flag(n: &Node, name: &str) -> Result<bool, String> {
    match n.params.get(name) {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(format!("{name}: a value known only on running, the code is not the same")),
    }
}

fn components(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let g = need(n, sc, "graph", "its graph")?;
    let args = kw(n, sc, &[("distance", "distance")])?.join(", ");
    // the library gives a Series indexed by the nodes' ids: the table of id and component
    let code = format!(
        "{}{var} = {g}.components({args}).rename(\"component\").rename_axis(\"id\").reset_index()\n",
        unpassed(n, &["graph", "distance"])
    );
    Ok((code, outs([("table", var.to_string())]), Need::Nothing))
}

/// conn.graph: the nodes' table and the pairs' table as HabitatGraph(attributes, connections, ids=...).
fn graph(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let nodes = need(n, sc, "nodes", "its nodes")?;
    let pairs = need(n, sc, "pairs", "its pairs")?;
    let column = |name: &str, default: &str| -> Result<String, String> {
        Ok(setting(n, name, sc)?.unwrap_or_else(|| text(default)))
    };
    let ids = match setting(n, "id", sc)? {
        Some(c) => format!("{nodes}[{c}]"),
        None => format!("({nodes}[\"id\"] if \"id\" in {nodes}.columns else None)"),
    };
    let named = ["from", "to", "value"].iter().any(|k| n.params.contains_key(*k));
    let mut code = unpassed(
        n,
        &[
            "nodes",
            "pairs",
            "id",
            "attribute",
            "restoration",
            "from",
            "to",
            "value",
            "kind",
            "convention",
            "landscape_area",
        ],
    );
    // the pairs: the columns named, else from, to and value when the table has them, else the table as it is
    let connections = if named {
        let cols = [column("from", "from")?, column("to", "to")?, column("value", "value")?];
        format!("{pairs}[[{}]].to_numpy()", cols.join(", "))
    } else {
        code.push_str(&format!(
            "{var}_pairs = ({pairs}[[\"from\", \"to\", \"value\"]] if {{\"from\", \"to\", \"value\"}} <= set({pairs}.columns) else {pairs}).to_numpy()\n"
        ));
        format!("{var}_pairs")
    };
    let mut args = vec![format!("{nodes}[{}]", column("attribute", "attribute")?), connections, format!("ids={ids}")];
    args.extend(kw(n, sc, &[("kind", "kind"), ("landscape_area", "landscape_area"), ("convention", "convention")])?);
    if let Some(c) = setting(n, "restoration", sc)? {
        args.push(format!("restoration={nodes}[{c}]"));
    }
    code.push_str(&format!("{var} = tf.conn.HabitatGraph({})\n", args.join(", ")));
    Ok((code, outs([("graph", var.to_string())]), Need::Nothing))
}

fn read_conefor(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let nodes = file(n, "nodes", sc)?.ok_or("its node file is not set")?;
    let connections = file(n, "connections", sc)?.ok_or("its connection file is not set")?;
    let mut args = vec![nodes, connections];
    args.extend(kw(n, sc, &[("kind", "kind"), ("convention", "convention"), ("landscape_area", "landscape_area")])?);
    let code = format!(
        "{}{var} = tf.conn.HabitatGraph.from_conefor({})\n",
        unpassed(n, &["nodes", "connections", "kind", "convention", "landscape_area"]),
        args.join(", ")
    );
    Ok((code, outs([("graph", var.to_string())]), Need::Nothing))
}

fn write_conefor(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let g = need(n, sc, "graph", "its graph")?;
    let nodes = file(n, "nodes", sc)?.ok_or("its node file is not set")?;
    let connections = file(n, "connections", sc)?.ok_or("its connection file is not set")?;
    // the library writes the files and gives nothing: the outputs are the paths
    let code = format!(
        "{var}_nodes = {nodes}\n{var}_connections = {connections}\n{g}.to_conefor({var}_nodes, {var}_connections)\n"
    );
    let o = outs([("nodes", format!("{var}_nodes")), ("connections", format!("{var}_connections"))]);
    Ok((code, o, Need::Nothing))
}

/// conn.to_map and conn.to_vector: the table's values on the graph's patches or features.
fn to_nodes(n: &Node, var: &str, sc: &Scope, method: &str, port: &str) -> Result<Code, String> {
    let g = need(n, sc, "graph", "its graph")?;
    let mut table = need(n, sc, "table", "its table")?;
    // the library reads the nodes' ids from the column `id`
    if let Some(id) = setting(n, "id", sc)?.filter(|i| i != "\"id\"") {
        table = format!("{table}.rename(columns={{{id}: \"id\"}})");
    }
    let mut args = vec![table];
    if port == "map" {
        let column = setting(n, "column", sc)?.ok_or("its column is not set")?;
        args.push(format!("name={column}"));
    }
    let code =
        format!("{}{var} = {g}.{method}({})\n", unpassed(n, &["graph", "table", "id", "column"]), args.join(", "));
    Ok((code, outs([(port, var.to_string())]), Need::Nothing))
}

/// conn.protconn: protconn(protected, region, ...), which gives the table and, with `delta`, the protected areas'.
fn protconn(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let protected = need(n, sc, "protected", "its protected areas")?;
    let region = need(n, sc, "region", "its region")?;
    let delta = flag(n, "delta")?;
    if value_of(n, "distance", sc)?.is_none() {
        return Err("its distance is not set".into());
    }
    let names = [
        ("distance", "distance"),
        ("probability", "probability"),
        ("transboundary", "transboundary"),
        ("transboundary_type", "transboundary_type"),
        ("bound", "bound"),
        ("area_unit", "area_unit"),
        ("distance_type", "distance_type"),
        ("resistance", "resistance"),
        ("transboundary_weight", "transboundary_weight"),
        ("cost_directions", "cost_directions"),
        ("cost_function", "cost_function"),
        ("preselect", "preselect"),
        ("pij_min", "pij_min"),
    ];
    let mut args = vec![protected, region];
    args.extend(kw(n, sc, &names)?);
    if delta {
        args.push("delta=True".into());
    }
    let mut used: Vec<&str> = names.iter().map(|(k, _)| *k).collect();
    used.extend(["protected", "region", "delta"]);
    let comments = unpassed(n, &used);
    let call = format!("tf.conn.protconn({})", args.join(", "));
    Ok(if delta {
        let code = format!("{comments}{var}, {var}_delta = {call}\n");
        (code, outs([("table", var.to_string()), ("delta", format!("{var}_delta"))]), Need::Nothing)
    } else {
        (format!("{comments}{var} = {call}\n"), outs([("table", var.to_string())]), Need::Nothing)
    })
}

/// A matrix with the ids in its first row and column (Circuitscape's), as the table of id and a column per id.
fn matrix_table(name: &str, matrix: &str) -> String {
    format!(
        "{name} = pd.DataFrame({matrix}[1:, 1:], columns=[str(int(i)) for i in {matrix}[0, 1:]])\n{name}.insert(0, \"id\", {matrix}[1:, 0])\n"
    )
}

/// circuit.pairwise and circuit.one_to_all: circuitscape_pairwise / circuitscape_one_to_all of the rasters.
fn circuit_raster(n: &Node, var: &str, sc: &Scope, one_to_all: bool) -> Result<Code, String> {
    let habitat = need(n, sc, "habitat", "its habitat")?;
    let points = need(n, sc, "points", "its points")?;
    let mut names = vec![
        ("polygons", "polygons"),
        ("mask", "mask"),
        ("habitat_is_resistance", "habitat_is_resistance"),
        ("four_neighbors", "four_neighbors"),
        ("avg_resistances", "avg_resistances"),
        ("cum_map", "cum_map"),
        ("max_map", "max_map"),
        ("focal_zero", "set_focal_node_currents_to_zero"),
    ];
    let (call, extra): (&str, &[(&str, &str)]) = if one_to_all {
        ("circuitscape_one_to_all", &[("all_to_one", "all_to_one"), ("strengths", "strengths")])
    } else {
        (
            "circuitscape_pairwise",
            &[("null_currents", "set_null_currents_to_nodata"), ("log_transform", "log_transform_maps")],
        )
    };
    names.extend_from_slice(extra);
    let mut args = vec![habitat.clone(), points];
    args.extend(kw(n, sc, &names)?);
    // [[value, new value], ...] as the library's {value: new value}
    if let Some(t) = value_of(n, "reclass_table", sc)?.filter(|t| t != "None") {
        args.push(format!("reclass=dict({t})"));
    }
    let mut used: Vec<&str> = names.iter().map(|(k, _)| *k).collect();
    used.extend(["habitat", "points", "reclass_table"]);
    let mut code = unpassed(n, &used);
    code.push_str("import pandas as pd\n");
    code.push_str(&format!("{var} = tf.circuit.{call}({})\n", args.join(", ")));
    // the library gives arrays: the table and the maps on the habitat's grid, as Terraflux's
    if one_to_all {
        let second = match value_of(n, "all_to_one", sc)?.as_deref() {
            None | Some("False") => "\"resistance\"".to_string(),
            Some("True") => "\"value\"".to_string(),
            Some(e) => format!("(\"value\" if {e} else \"resistance\")"),
        };
        code.push_str(&format!("{var}_resistances = pd.DataFrame({var}.resistances, columns=[\"id\", {second}])\n"));
    } else {
        code.push_str(&matrix_table(&format!("{var}_resistances"), &format!("{var}.resistances")));
    }
    code.push_str(&format!(
        "{var}_like = {habitat}.squeeze(\"band\", drop=True) if \"band\" in {habitat}.dims else {habitat}\n"
    ));
    for (port, field) in [("cum", "cum_current"), ("max", "max_current")] {
        code.push_str(&format!(
            "{var}_{port} = {var}_like.copy(data={var}.{field}) if {var}.{field} is not None else None\n"
        ));
    }
    let o = outs([
        ("resistances", format!("{var}_resistances")),
        ("cum", format!("{var}_cum")),
        ("max", format!("{var}_max")),
    ]);
    Ok((code, o, Need::Nothing))
}

/// A network's table: the input (as an array), else the file setting.
fn network_table(n: &Node, sc: &Scope, port: &str, file_setting: &str, what: &str) -> Result<String, String> {
    match value_of(n, port, sc)? {
        Some(t) if n.inputs.contains_key(port) => Ok(format!("{t}.to_numpy()")),
        _ => file(n, file_setting, sc)?.ok_or_else(|| format!("{what} is not connected, nor its file set")),
    }
}

fn network_advanced(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let graph = network_table(n, sc, "graph", "graph_file", "its graph")?;
    let sources = network_table(n, sc, "sources", "sources_file", "its sources")?;
    let grounds = network_table(n, sc, "grounds", "grounds_file", "its grounds")?;
    let mut args = vec![graph, sources, grounds];
    let names = [
        ("graph_is_resistance", "graph_is_resistance"),
        ("ground_is_resistance", "ground_is_resistance"),
        ("remove_src_or_gnd", "remove_src_or_gnd"),
    ];
    args.extend(kw(n, sc, &names)?);
    let mut used: Vec<&str> = names.iter().map(|(k, _)| *k).collect();
    used.extend(["graph", "sources", "grounds", "graph_file", "sources_file", "grounds_file"]);
    let mut code = unpassed(n, &used);
    code.push_str("import pandas as pd\n");
    code.push_str(&format!("{var} = tf.circuit.circuitscape_network_advanced({})\n", args.join(", ")));
    for (port, columns) in [
        ("voltages", "\"node\", \"voltage\""),
        ("node_currents", "\"node\", \"current\""),
        ("branch_currents", "\"from\", \"to\", \"current\""),
    ] {
        code.push_str(&format!("{var}_{port} = pd.DataFrame({var}.{port}, columns=[{columns}])\n"));
    }
    let o = outs([
        ("voltages", format!("{var}_voltages")),
        ("node_currents", format!("{var}_node_currents")),
        ("branch_currents", format!("{var}_branch_currents")),
    ]);
    Ok((code, o, Need::Nothing))
}

fn network_pairwise(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let graph = network_table(n, sc, "graph", "graph_file", "its graph")?;
    let points = file(n, "focal_nodes", sc)?.ok_or("its focal nodes are not set")?;
    let mut args = vec![graph, points];
    let names = [
        ("graph_is_resistance", "graph_is_resistance"),
        ("voltage_maps", "voltage_maps"),
        ("current_maps", "current_maps"),
    ];
    args.extend(kw(n, sc, &names)?);
    if let Some(p) = file(n, "included_pairs", sc)? {
        args.push(format!("included_pairs={p}"));
    }
    let mut used: Vec<&str> = names.iter().map(|(k, _)| *k).collect();
    used.extend(["graph", "graph_file", "focal_nodes", "included_pairs"]);
    let mut code = unpassed(n, &used);
    code.push_str("import pandas as pd\n");
    code.push_str(&format!("{var} = tf.circuit.circuitscape_network_pairwise({})\n", args.join(", ")));
    code.push_str(&matrix_table(&format!("{var}_resistances"), &format!("{var}.resistances")));
    // summed over the pairs, when the maps were asked for
    for (port, columns) in
        [("cum_node_currents", "\"node\", \"current\""), ("cum_branch_currents", "\"from\", \"to\", \"current\"")]
    {
        code.push_str(&format!(
            "{var}_{port} = pd.DataFrame({var}.{port}, columns=[{columns}]) if {var}.{port} is not None else None\n"
        ));
    }
    let o = outs([
        ("resistances", format!("{var}_resistances")),
        ("cum_node_currents", format!("{var}_cum_node_currents")),
        ("cum_branch_currents", format!("{var}_cum_branch_currents")),
    ]);
    Ok((code, o, Need::Nothing))
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

    fn translate(toml: &str) -> super::super::Script {
        let r = Registry::standard();
        let m = Model::from_toml(toml).unwrap();
        super::super::script(&m, &r, None)
    }

    #[test]
    fn a_graph_of_a_habitat_map_and_its_analyses() {
        let s = translate(
            r#"
format = "terraflux/1"

[[nodes]]
id = "land"
op = "nlm.random_cluster"
params = { rows = 40, columns = 40, p = 0.4 }

[[nodes]]
id = "habitat"
op = "raster.reclassify"
inputs = { map = "land.map" }
params = { rules = "0..2 -> 1\n* -> 0" }

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = { map = "habitat.map" }
params = { habitat = 1, distance = "edge", max_distance = 500, area_unit = "ha", landscape_area = 160 }

[[nodes]]
id = "cent"
op = "conn.centrality"
inputs = { graph = "graph.graph" }
params = { distance = 100 }

[[nodes]]
id = "overall"
op = "conn.overall"
inputs = { graph = "graph.graph" }
params = { metrics = ["NC", "PC"], distance = [100, 200], probability = 0.5 }

[[nodes]]
id = "imp"
op = "conn.node_importance"
inputs = { graph = "graph.graph" }
params = { metrics = "PC", distance = 100 }

[[nodes]]
id = "links"
op = "conn.link_importance"
inputs = { graph = "graph.graph" }
params = { distance = 100, metric = "IIC" }

[[nodes]]
id = "comp"
op = "conn.components"
inputs = { graph = "graph.graph" }
params = { distance = 100 }

[[nodes]]
id = "dpc"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "imp.table" }
params = { column = "dPC" }

[[nodes]]
id = "compmap"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "comp.table" }
params = { column = "component" }

[outputs.dpc]
from = "dpc.map"
"#,
        );
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(
            t.contains("graph = tf.conn.HabitatGraph.from_raster(habitat, habitat=1, area_unit=\"ha\", distance=\"edge\", max_distance=500, landscape_area=160)\n"),
            "{t}"
        );
        assert!(t.contains("cent = graph.centrality(distance=100)\n"), "{t}");
        assert!(
            t.contains("overall = graph.overall(metrics=[\"NC\", \"PC\"], distance=[100, 200], probability=0.5)\n"),
            "{t}"
        );
        assert!(t.contains("imp = graph.node_importance(metrics=\"PC\", distance=100)\n"), "{t}");
        assert!(t.contains("links = graph.link_importance(metric=\"IIC\", distance=100)\n"), "{t}");
        assert!(
            t.contains(
                "comp = graph.components(distance=100).rename(\"component\").rename_axis(\"id\").reset_index()\n"
            ),
            "{t}"
        );
        assert!(t.contains("dpc = graph.to_raster(imp, name=\"dPC\")\n"), "{t}");
        assert!(t.contains("compmap = graph.to_raster(comp, name=\"component\")\n"), "{t}");
        parses(t);
    }

    #[test]
    fn a_graph_from_tables_and_conefor_files() {
        let s = translate(
            r#"
format = "terraflux/1"

[[nodes]]
id = "nodes"
op = "table.of"
params = { columns = ["id", "attribute"], rows = [[1, 10], [2, 20], [3, 5]] }

[[nodes]]
id = "pairs"
op = "table.of"
params = { columns = ["from", "to", "value"], rows = [[1, 2, 100], [2, 3, 300]] }

[[nodes]]
id = "graph"
op = "conn.graph"
inputs = { nodes = "nodes.table", pairs = "pairs.table" }
params = { landscape_area = 100 }

[[nodes]]
id = "change"
op = "conn.link_change"
inputs = { graph = "graph.graph", changes = "pairs.table" }
params = { distance = 150, metric = "PC" }

[[nodes]]
id = "wr"
op = "conn.write_conefor"
inputs = { graph = "graph.graph" }
params = { nodes = "files/nodes.txt", connections = "files/connections.txt" }

[[nodes]]
id = "back"
op = "conn.read_conefor"
params = { nodes = "files/nodes.txt", connections = "files/connections.txt", landscape_area = 100 }

[[nodes]]
id = "overall"
op = "conn.overall"
inputs = { graph = "back.graph" }
params = { distance = 150 }
"#,
        );
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(
            t.contains("graph_pairs = (pairs[[\"from\", \"to\", \"value\"]] if {\"from\", \"to\", \"value\"} <= set(pairs.columns) else pairs).to_numpy()\n"),
            "{t}"
        );
        assert!(
            t.contains("graph = tf.conn.HabitatGraph(nodes[\"attribute\"], graph_pairs, ids=(nodes[\"id\"] if \"id\" in nodes.columns else None), landscape_area=100)\n"),
            "{t}"
        );
        assert!(t.contains("change = graph.link_change(changes=pairs, metric=\"PC\", distance=150)\n"), "{t}");
        assert!(t.contains("graph.to_conefor(wr_nodes, wr_connections)\n"), "{t}");
        assert!(t.contains("wr_nodes = MODEL_DIR / \"files/nodes.txt\"\n"), "{t}");
        assert!(
            t.contains("back = tf.conn.HabitatGraph.from_conefor(MODEL_DIR / \"files/nodes.txt\", MODEL_DIR / \"files/connections.txt\", landscape_area=100)\n"),
            "{t}"
        );
        parses(t);
    }

    #[test]
    fn protconn_with_and_without_delta() {
        let s = translate(
            r#"
format = "terraflux/1"

[inputs.reserves]
kind = "value"
value = 1

[inputs.region]
kind = "value"
value = 2

[[nodes]]
id = "pc"
op = "conn.protconn"
inputs = { protected = "$reserves", region = "$region" }
params = { distance = [1000, 5000], area_unit = "km2" }

[[nodes]]
id = "pcd"
op = "conn.protconn"
inputs = { protected = "$reserves", region = "$region" }
params = { distance = 2000, delta = true }

[[nodes]]
id = "grid"
op = "conn.connectivity_grid"
inputs = { protected = "$reserves", region = "$region" }
params = { distance = 2000, cell_area = 500 }

[outputs.table]
from = "pcd.table"

[outputs.delta]
from = "pcd.delta"
"#,
        );
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(
            t.contains("pc = tf.conn.protconn(reserves, region, distance=[1000, 5000], area_unit=\"km2\")\n"),
            "{t}"
        );
        assert!(t.contains("pcd, pcd_delta = tf.conn.protconn(reserves, region, distance=2000, delta=True)\n"), "{t}");
        assert!(
            t.contains("grid = tf.conn.connectivity_grid(protected_areas=reserves, region=region, cell_area=500, distance=2000)\n"),
            "{t}"
        );
        parses(t);
    }

    #[test]
    fn circuits_on_a_resistance_map() {
        let s = translate(
            r#"
format = "terraflux/1"

[inputs.points]
kind = "map"
path = "data/points.tif"

[[nodes]]
id = "noise"
op = "nlm.random_cluster"
params = { rows = 30, columns = 30, p = 0.4 }

[[nodes]]
id = "resistance"
op = "raster.reclassify"
inputs = { map = "noise.map" }
params = { rules = "0..2 -> 1\n* -> 10" }

[[nodes]]
id = "flow"
op = "circuit.omniscape"
inputs = { resistance = "resistance.map" }
params = { radius = 6, block_size = 3, flow_potential = true, normalized_current = true }

[[nodes]]
id = "pair"
op = "circuit.pairwise"
inputs = { habitat = "resistance.map", points = "$points" }
params = { cum_map = true, reclass_table = [[1, 2], [3, 4]], focal_zero = true }

[[nodes]]
id = "one"
op = "circuit.one_to_all"
inputs = { habitat = "resistance.map", points = "$points" }
params = { all_to_one = true, cum_map = true }

[[nodes]]
id = "edges"
op = "table.of"
params = { columns = ["a", "b", "r"], rows = [[1, 2, 1.0], [2, 3, 2.0]] }

[[nodes]]
id = "net"
op = "circuit.network_pairwise"
inputs = { graph = "edges.table" }
params = { focal_nodes = [1, 3] }

[outputs.current]
from = "flow.current"

[outputs.cum]
from = "pair.cum"

[outputs.resistances]
from = "pair.resistances"
"#,
        );
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(
            t.contains("flow = tf.circuit.omniscape(resistance, radius=6, block_size=3, flow_potential=True, normalized_current=True)\n"),
            "{t}"
        );
        assert!(
            t.contains("pair = tf.circuit.circuitscape_pairwise(resistance, points, cum_map=True, set_focal_node_currents_to_zero=True, reclass=dict([[1, 2], [3, 4]]))\n"),
            "{t}"
        );
        assert!(
            t.contains("pair_resistances = pd.DataFrame(pair.resistances[1:, 1:], columns=[str(int(i)) for i in pair.resistances[0, 1:]])\n"),
            "{t}"
        );
        assert!(
            t.contains("pair_cum = pair_like.copy(data=pair.cum_current) if pair.cum_current is not None else None\n"),
            "{t}"
        );
        assert!(
            t.contains("one = tf.circuit.circuitscape_one_to_all(resistance, points, cum_map=True, all_to_one=True)\n"),
            "{t}"
        );
        assert!(t.contains("one_resistances = pd.DataFrame(one.resistances, columns=[\"id\", \"value\"])\n"), "{t}");
        assert!(t.contains("net = tf.circuit.circuitscape_network_pairwise(edges.to_numpy(), [1, 3])\n"), "{t}");
        parses(t);
    }

    #[test]
    fn the_networks_of_an_mspa_map() {
        let s = translate(
            r#"
format = "terraflux/1"

[inputs.land]
kind = "map"
path = "data/land.tif"

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "$land" }
params = { foreground = 1 }

[[nodes]]
id = "comp"
op = "networks.gtn_components"
inputs = { map = "mspa.map" }
params = { pixel_size = 25 }

[[nodes]]
id = "imp"
op = "networks.gtn_importance"
inputs = { map = "mspa.map" }

[[nodes]]
id = "cs"
op = "networks.conefor"
inputs = { map = "mspa.map" }

[outputs.stats]
from = "comp.stats"

[outputs.links]
from = "cs.links"
"#,
        );
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(t.contains("comp = tf.gtn_components(mspa, pixel_size=25)\n"), "{t}");
        assert!(t.contains("imp = tf.gtn_importance(mspa)\n"), "{t}");
        assert!(t.contains("cs = tf.conefor_inputs(mspa)\n"), "{t}");
        parses(t);
    }
}
