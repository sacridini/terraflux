//! Habitat connectivity from landschaft's conn module: habitat graphs (Conefor Sensinode's and Makurhini's
//! indices overall, per node, per link, for links that change and for nodes to restore; the centralities), and
//! their nodes and distances from rasters of patches. The graph's paths are landschaft's C++; the rest follows
//! its numpy in the same order (its BLAS dot products and SIMD exponentials to rounding).

pub mod graph;
pub mod grid;
mod patches;
pub mod protconn;
pub mod raster;
pub mod samc;
pub mod vector;

use std::sync::Arc;

pub use graph::{Connections, HabitatGraph, Kind};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Graph));
    r.add(Arc::new(ReadConefor));
    r.add(Arc::new(WriteConefor));
    r.add(Arc::new(Overall));
    r.add(Arc::new(NodeImportance));
    r.add(Arc::new(LinkImportance));
    r.add(Arc::new(LinkChange));
    r.add(Arc::new(Components));
    r.add(Arc::new(Centrality));
    r.add(Arc::new(EcaChange));
    patches::register(r);
}

pub(super) fn bad(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

/// The settings of the graph every constructor has.
pub(super) fn graph_settings(sig: OpSignature) -> OpSignature {
    sig.param(
        PortSpec::optional(
            "kind",
            Type::Text,
            "what the pairs' values are: distance (default), probability or link (0/1)",
        )
        .choices(["distance", "probability", "link"]),
    )
    .param(
        PortSpec::optional("convention", Type::Text, "makurhini (default) or conefor: the rules of the indices")
            .choices(["makurhini", "conefor"]),
    )
    .param(PortSpec::optional(
        "landscape_area",
        Type::Number,
        "the landscape's area, for LCP, IIC and PC (Conefor's default 1; Makurhini's none)",
    ))
}

pub(super) fn settings(args: &Args) -> Result<(Kind, bool, Option<f64>)> {
    let kind = Kind::parse(&args.param::<String>("kind", "distance".into())?).map_err(|e| bad(args, e))?;
    let convention: String = args.param("convention", "makurhini".into())?;
    let conefor = match convention.as_str() {
        "makurhini" => false,
        "conefor" => true,
        c => return Err(bad(args, format!("the convention is makurhini or conefor, got {c:?}"))),
    };
    Ok((kind, conefor, args.opt_param("landscape_area")?))
}

/// A table's pairs: an n x n matrix, or (from, to, value) in the columns named (else the first three).
fn connections(t: &Table, n: usize, names: [&str; 3]) -> std::result::Result<Connections, String> {
    if t.rows.len() == n && t.columns.len() == n {
        return Ok(Connections::Matrix(t.rows.iter().flatten().copied().collect()));
    }
    let col = |name: &str, k: usize| -> std::result::Result<usize, String> {
        match t.columns.iter().position(|c| c == name) {
            Some(i) => Ok(i),
            None if t.columns.len() == 3 => Ok(k),
            None => Err(format!("the pair table has no column {name:?}")),
        }
    };
    let (f, to, v) = (col(names[0], 0)?, col(names[1], 1)?, col(names[2], 2)?);
    Ok(Connections::Pairs(t.rows.iter().map(|r| [r[f], r[to], r[v]]).collect()))
}

pub(super) fn graph_out(g: HabitatGraph) -> Outputs {
    outputs([("graph", Value::HabitatGraph(Arc::new(g)))])
}

pub(super) fn the_graph<'a>(args: &'a Args) -> Result<&'a HabitatGraph> {
    match args.get("graph") {
        Some(Value::HabitatGraph(g)) => Ok(g),
        _ => Err(bad(args, "connect a habitat graph (conn.graph)")),
    }
}

/// The distances asked for: one, several or none (Makurhini's median).
fn distances(args: &Args) -> Result<Vec<Option<f64>>> {
    Ok(match args.params.get("distance") {
        None | Some(serde_json::Value::Null) => vec![None],
        Some(serde_json::Value::Array(_)) => args.param::<Vec<Option<f64>>>("distance", vec![])?,
        Some(_) => vec![Some(args.param("distance", 0.0)?)],
    })
}

fn metrics(args: &Args) -> Result<Option<Vec<String>>> {
    Ok(match args.params.get("metrics") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(vec![s.clone()]),
        Some(_) => Some(args.param("metrics", vec![])?),
    })
}

/// The settings of a scenario.
fn scenario(sig: OpSignature, several: bool) -> OpSignature {
    sig.input(PortSpec::new("graph", Type::HabitatGraph, "a habitat graph"))
        .param(PortSpec::optional(
            "distance",
            Type::Any,
            if several {
                "the link distance (the distance of `probability`), or several: a row each (default: Makurhini's \
                 median of the distances; Conefor needs one)"
            } else {
                "the link distance (the distance of `probability`; default: Makurhini's median of the distances)"
            },
        ))
        .param(PortSpec::optional("probability", Type::Number, "the probability at that distance (default 0.5)"))
        .param(PortSpec::optional(
            "pij_min",
            Type::Number,
            "probabilities below it make no edge (default 0.01 Makurhini, 0 Conefor)",
        ))
}

fn table_out(port: &str, t: Table) -> Outputs {
    outputs([(port, Value::Table(t))])
}

struct Graph;

impl Operator for Graph {
    fn signature(&self) -> OpSignature {
        graph_settings(
            OpSignature::new(
                "conn.graph",
                "A habitat graph (landschaft's HabitatGraph): nodes with an attribute (an area, a quality) and the \
                 distances, probabilities or links between pairs of them, from a table of nodes and a table of \
                 pairs (from, to, value) or an n x n matrix.",
                Access::Global,
            )
            .input(PortSpec::new("nodes", Type::Table, "a row per node: its id and attribute"))
            .input(PortSpec::new("pairs", Type::Table, "(from, to, value) rows, or an n x n matrix"))
            .output(PortSpec::new("graph", Type::HabitatGraph, "the habitat graph")),
        )
        .param(PortSpec::optional("id", Type::Text, "the nodes' id column (default id; else 1, 2...)"))
        .param(PortSpec::optional("attribute", Type::Text, "the nodes' attribute column (default attribute)"))
        .param(PortSpec::optional(
            "restoration",
            Type::Text,
            "a column of 1 (existing node) or 0 (node to add): the importances of nodes to restore",
        ))
        .param(PortSpec::optional("from", Type::Text, "the pairs' columns (default from, to, value)"))
        .param(PortSpec::optional("to", Type::Text, "the pairs' second node column"))
        .param(PortSpec::optional("value", Type::Text, "the pairs' value column"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let Some(Value::Table(nodes)) = args.get("nodes") else { return Err(bad(args, "connect the nodes' table")) };
        let Some(Value::Table(pairs)) = args.get("pairs") else { return Err(bad(args, "connect the pairs' table")) };
        let (kind, conefor, area) = settings(args)?;
        let column = |name: &str| nodes.column(name);
        let id: String = args.param("id", "id".into())?;
        let attribute: String = args.param("attribute", "attribute".into())?;
        let attributes =
            column(&attribute).ok_or_else(|| bad(args, format!("the nodes have no column {attribute:?}")))?;
        let restoration = match args.opt_param::<String>("restoration")? {
            Some(c) => Some(column(&c).ok_or_else(|| bad(args, format!("the nodes have no column {c:?}")))?),
            None => None,
        };
        let names: [String; 3] =
            [args.param("from", "from".into())?, args.param("to", "to".into())?, args.param("value", "value".into())?];
        let conns =
            connections(pairs, attributes.len(), [&names[0], &names[1], &names[2]]).map_err(|e| bad(args, e))?;
        let g = HabitatGraph::new(attributes, conns, column(&id), kind, area, conefor, restoration)
            .map_err(|e| bad(args, e))?;
        Ok(graph_out(g))
    }
}

struct ReadConefor;

impl Operator for ReadConefor {
    fn signature(&self) -> OpSignature {
        graph_settings(
            OpSignature::new(
                "conn.read_conefor",
                "A habitat graph from Conefor Sensinode's files: the node file (an id and an attribute per line) \
                 and the connection file (id id value lines, or an n x n matrix).",
                Access::Global,
            )
            .output(PortSpec::new("graph", Type::HabitatGraph, "the habitat graph")),
        )
        .param(PortSpec::new("nodes", Type::Text, "the node file"))
        .param(PortSpec::new("connections", Type::Text, "the connection file"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (kind, conefor, area) = settings(args)?;
        let read = |p: &str| -> Result<String> {
            let path: String = args.opt_param(p)?.ok_or_else(|| bad(args, format!("set {p}")))?;
            std::fs::read_to_string(&path).map_err(|e| bad(args, format!("{path}: {e}")))
        };
        let g = HabitatGraph::from_conefor(&read("nodes")?, &read("connections")?, kind, area, conefor)
            .map_err(|e| bad(args, e))?;
        Ok(graph_out(g))
    }
}

struct WriteConefor;

impl Operator for WriteConefor {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "conn.write_conefor",
            "A habitat graph written as Conefor Sensinode's node and connection files (the pairs with a value).",
            Access::Global,
        )
        .input(PortSpec::new("graph", Type::HabitatGraph, "a habitat graph"))
        .output(PortSpec::new("nodes", Type::Text, "the node file written"))
        .output(PortSpec::new("connections", Type::Text, "the connection file written"))
        .param(PortSpec::new("nodes", Type::Text, "the node file"))
        .param(PortSpec::new("connections", Type::Text, "the connection file"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let g = the_graph(args)?;
        let (nodes, conns) = g.to_conefor();
        let mut out = Outputs::new();
        for (p, text) in [("nodes", nodes), ("connections", conns)] {
            let path: String = args.opt_param(p)?.ok_or_else(|| bad(args, format!("set {p}")))?;
            std::fs::write(&path, text).map_err(|e| bad(args, format!("{path}: {e}")))?;
            out.insert(p.into(), Value::Text(path));
        }
        Ok(out)
    }
}

struct Overall;

impl Operator for Overall {
    fn signature(&self) -> OpSignature {
        scenario(
            OpSignature::new(
                "conn.overall",
                "A habitat graph's connectivity (Conefor's indices, Makurhini's MK_dPCIIC overall): A, NC \
                 (components), NL (links), H (Harary), CCP, LCP, IIC and its equivalent connectivity and \
                 fractions, F, AWF, PC and its equivalent connectivity and fractions; a row per distance.",
                Access::Global,
            )
            .output(PortSpec::new("table", Type::Table, "the indices")),
            true,
        )
        .param(PortSpec::optional(
            "metrics",
            Type::Any,
            "of A, NC, NL, H, CCP, LCP, IIC, F, AWF, PC (default all; no probabilities on links)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let g = the_graph(args)?;
        let t = g
            .overall(
                metrics(args)?.as_deref(),
                &distances(args)?,
                args.opt_param("probability")?,
                args.opt_param("pij_min")?,
            )
            .map_err(|e| bad(args, e))?;
        Ok(table_out("table", t))
    }
}

struct NodeImportance;

impl Operator for NodeImportance {
    fn signature(&self) -> OpSignature {
        scenario(
            OpSignature::new(
                "conn.node_importance",
                "Every node's importance (Conefor's dI, Makurhini's MK_dPCIIC): the share of each index lost \
                 without it and the intra, flux and connector fractions of dIIC and dPC; the betweenness BC, \
                 BC(IIC) and BC(PC) when asked for. On a graph with nodes to restore, the gain of adding each \
                 (Makurhini's dIICres and dPCres, or Conefor's d*add). A row per node and distance. As in \
                 Conefor and Makurhini, a node is taken out of the graph as it is: the distances between the \
                 others are not measured again, so with least-cost distances the detours it forces are missed.",
                Access::Global,
            )
            .output(PortSpec::new("table", Type::Table, "the importances")),
            true,
        )
        .param(PortSpec::optional(
            "metrics",
            Type::Any,
            "of A, NC, NL, H, CCP, LCP, IIC, F, AWF, PC, BC, BCIIC, BCPC (default all but the betweenness)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let g = the_graph(args)?;
        let t = g
            .node_importance(
                metrics(args)?.as_deref(),
                &distances(args)?,
                args.opt_param("probability")?,
                args.opt_param("pij_min")?,
            )
            .map_err(|e| bad(args, e))?;
        Ok(table_out("table", t))
    }
}

fn one_distance(args: &Args) -> Result<Option<f64>> {
    match distances(args)?.as_slice() {
        [d] => Ok(*d),
        _ => Err(bad(args, "give one distance")),
    }
}

struct LinkImportance;

impl Operator for LinkImportance {
    fn signature(&self) -> OpSignature {
        scenario(
            OpSignature::new(
                "conn.link_importance",
                "Every link's importance (Conefor's link removal): the share of IIC or PC lost without it.",
                Access::Global,
            )
            .output(PortSpec::new("table", Type::Table, "from, to, distance and the share lost")),
            false,
        )
        .param(PortSpec::optional("metric", Type::Text, "IIC or PC (default)").choices(["IIC", "PC"]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let g = the_graph(args)?;
        let metric: String = args.param("metric", "PC".into())?;
        let t = g
            .link_importance(&metric, one_distance(args)?, args.opt_param("probability")?, args.opt_param("pij_min")?)
            .map_err(|e| bad(args, e))?;
        Ok(table_out("table", t))
    }
}

struct LinkChange;

impl Operator for LinkChange {
    fn signature(&self) -> OpSignature {
        scenario(
            OpSignature::new(
                "conn.link_change",
                "The change of IIC or PC when pairs change (Conefor's link change): new links or lost ones, \
                 distances or probabilities raised or lowered; negative for a gain.",
                Access::Global,
            )
            .input(PortSpec::new("changes", Type::Table, "(from, to, new value) rows, or the new n x n matrix"))
            .output(PortSpec::new("table", Type::Table, "from, to, distance and the change (%)")),
            false,
        )
        .param(PortSpec::optional("metric", Type::Text, "IIC or PC (default)").choices(["IIC", "PC"]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let g = the_graph(args)?;
        let Some(Value::Table(c)) = args.get("changes") else { return Err(bad(args, "connect the changes")) };
        let changes = if c.rows.len() == g.n_nodes() && c.columns.len() == g.n_nodes() {
            Connections::Matrix(c.rows.iter().flatten().copied().collect())
        } else if c.columns.len() == 3 {
            Connections::Pairs(c.rows.iter().map(|r| [r[0], r[1], r[2]]).collect())
        } else {
            return Err(bad(args, "the changes are (from, to, value) rows or an n x n matrix"));
        };
        let metric: String = args.param("metric", "PC".into())?;
        let t = g
            .link_change(
                &changes,
                &metric,
                one_distance(args)?,
                args.opt_param("probability")?,
                args.opt_param("pij_min")?,
            )
            .map_err(|e| bad(args, e))?;
        Ok(table_out("table", t))
    }
}

struct Components;

impl Operator for Components {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "conn.components",
            "The component (1, 2...) of every node: the groups of nodes joined by links at the distance.",
            Access::Global,
        )
        .input(PortSpec::new("graph", Type::HabitatGraph, "a habitat graph"))
        .output(PortSpec::new("table", Type::Table, "id and component"))
        .param(PortSpec::optional(
            "distance",
            Type::Number,
            "the link distance (default: Makurhini's median of the distances)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let g = the_graph(args)?;
        Ok(table_out("table", g.components(args.opt_param("distance")?).map_err(|e| bad(args, e))?))
    }
}

struct Centrality;

impl Operator for Centrality {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "conn.centrality",
            "The centralities of every node (Makurhini's MK_RMCentrality): degree (or strength), eigenvector, \
             closeness, betweenness (igraph's) and cluster, on the links shorter than the distance, or on \
             probabilities. A row per node and distance.",
            Access::Global,
        )
        .input(PortSpec::new("graph", Type::HabitatGraph, "a habitat graph of distances"))
        .output(PortSpec::new("table", Type::Table, "the centralities"))
        .param(PortSpec::optional(
            "distance",
            Type::Any,
            "the link distance, or several (default: Makurhini's median of the distances)",
        ))
        .param(PortSpec::optional(
            "probability",
            Type::Number,
            "the probability at that distance: centralities on probabilities",
        ))
        .param(PortSpec::optional(
            "binary",
            Type::Bool,
            "links (default true), else probabilities exp(-d / distance)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let g = the_graph(args)?;
        let t = g
            .centrality(&distances(args)?, args.opt_param("probability")?, args.param("binary", true)?)
            .map_err(|e| bad(args, e))?;
        Ok(table_out("table", t))
    }
}

struct EcaChange;

impl Operator for EcaChange {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "conn.eca_change",
            "The change of the equivalent connected area (ECA) from graph to graph, e.g. the habitat of several              years (Makurhini's MK_dECA): per distance and graph the habitat's area, ECA (of PC or IIC), ECA              against the landscape's area and the habitat's, the changes of area and ECA (%; the first against              the landscape's area) and their ratio, and how they compare (dECA < dA < 0: connectivity loss...).",
            Access::Global,
        )
        .input(PortSpec::new("graphs", Type::Any, "habitat graphs in time order (a series), or one"))
        .output(PortSpec::new("table", Type::Table, "time (1, 2...), LA, area, distance, ECA, ECA_LA, ECA_area, dA, dECA, rECA"))
        .output(PortSpec::new("readings", Type::Any, "per row, the comparison and its reading"))
        .param(PortSpec::new("landscape_area", Type::Number, "the landscape's area"))
        .param(PortSpec::optional("metric", Type::Text, "PC (default) or IIC").choices(["PC", "IIC"]))
        .param(PortSpec::optional(
            "distance",
            Type::Any,
            "the link distance, or several (default: Makurhini's median of the distances)",
        ))
        .param(PortSpec::optional("probability", Type::Number, "the probability at that distance (default 0.5)"))
        .param(PortSpec::optional("pij_min", Type::Number, "probabilities below it make no edge"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let graphs: Vec<&HabitatGraph> = match args.get("graphs") {
            Some(Value::HabitatGraph(g)) => vec![&**g],
            Some(Value::Series(items)) => items
                .iter()
                .map(|v| match v {
                    Value::HabitatGraph(g) => Ok(&**g),
                    _ => Err(bad(args, "the series must hold habitat graphs")),
                })
                .collect::<Result<_>>()?,
            _ => return Err(bad(args, "connect habitat graphs (a series of them)")),
        };
        let la: f64 = args.opt_param("landscape_area")?.ok_or_else(|| bad(args, "set the landscape's area"))?;
        let metric: String = args.param("metric", "PC".into())?;
        let (t, readings) = graph::eca_change(
            &graphs,
            &metric,
            &distances(args)?,
            la,
            args.opt_param("probability")?,
            args.opt_param("pij_min")?,
        )
        .map_err(|e| bad(args, e))?;
        let readings = serde_json::Value::Array(
            readings.iter().map(|(c, r)| serde_json::json!({"comparison": c, "change": r})).collect(),
        );
        Ok(outputs([("table", Value::Table(t)), ("readings", Value::Json(readings))]))
    }
}
