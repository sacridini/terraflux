//! landschaft's HabitatGraph: a graph of habitat nodes with an attribute (an area, a quality) and the distances,
//! probabilities or links between pairs of them; Conefor Sensinode's indices (NC, NL, H, CCP, LCP, IIC, F, AWF,
//! PC, the betweenness BC, BC(IIC) and BC(PC)) overall and per node (dI and its intra, flux and connector
//! fractions), with Makurhini's conventions or Conefor's, the importance of links, of changes to them and of
//! nodes to restore, and the centralities of Makurhini's MK_RMCentrality. The paths and their repairs are
//! landschaft's C++; what landschaft computes with numpy is computed here in the same order.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use terraflux_ls_sys::ffi;

use crate::Table;
use crate::change::numpy_sum;

/// Pairs that change: their nodes (i < j) and new values.
type ChangePairs = (Vec<i32>, Vec<i32>, Vec<f64>);

/// What the pairs' values are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Distance,
    Probability,
    Link,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Kind, String> {
        match s {
            "distance" => Ok(Kind::Distance),
            "probability" => Ok(Kind::Probability),
            "link" => Ok(Kind::Link),
            s => Err(format!("kind must be distance, probability or link, got {s:?}")),
        }
    }

    fn code(self) -> i32 {
        match self {
            Kind::Distance => 0,
            Kind::Probability => 1,
            Kind::Link => 2,
        }
    }
}

pub const METRICS: [&str; 13] = ["A", "NC", "NL", "H", "CCP", "LCP", "IIC", "F", "AWF", "PC", "BC", "BCIIC", "BCPC"];
const LINK_METRICS: [&str; 8] = ["NC", "NL", "H", "CCP", "LCP", "IIC", "BC", "BCIIC"];
const PROBABILITY_METRICS: [&str; 4] = ["F", "AWF", "PC", "BCPC"];
const BETWEENNESS: [&str; 3] = ["BC", "BCIIC", "BCPC"];
/// Makurhini's distance of the pairs without a path (MK_dPCIIC's 1 / (1 + 16443701)).
const MAKURHINI_UNCONNECTED: f64 = 16443701.0;

/// The metrics asked for (all but the betweenness by default; no probabilities on a graph of links).
pub fn metric_list(metrics: Option<&[String]>, kind: Kind) -> Result<Vec<String>, String> {
    let Some(metrics) = metrics else {
        return Ok(METRICS
            .iter()
            .filter(|m| !BETWEENNESS.contains(m) && !(kind == Kind::Link && PROBABILITY_METRICS.contains(m)))
            .map(|m| m.to_string())
            .collect());
    };
    let mut chosen: Vec<String> = Vec::new();
    for m in metrics {
        let key = m.to_uppercase();
        if !METRICS.contains(&key.as_str()) {
            return Err(format!("unknown metric {m:?}; choose from {}", METRICS.join(", ")));
        }
        if kind == Kind::Link && PROBABILITY_METRICS.contains(&key.as_str()) {
            return Err(format!("{key} needs distances or probabilities, not links"));
        }
        if !chosen.contains(&key) {
            chosen.push(key);
        }
    }
    Ok(chosen)
}

fn has(chosen: &[String], m: &str) -> bool {
    chosen.iter().any(|c| c == m)
}

fn any_of(chosen: &[String], set: &[&str]) -> bool {
    chosen.iter().any(|c| set.contains(&c.as_str()))
}

/// Python's min(a, b) (a when b is not smaller, NaN included).
fn py_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// numpy's maximum(x, 0).
fn np_max0(x: f64) -> f64 {
    if x.is_nan() || x >= 0.0 { x } else { 0.0 }
}

/// A dot product (numpy's goes through BLAS: the same to rounding).
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |s, (x, y)| s + x * y)
}

/// The connections of a graph: an n x n matrix (row-major) or (from, to, value) rows.
pub enum Connections {
    Matrix(Vec<f64>),
    Pairs(Vec<[f64; 3]>),
}

/// A scenario: the link distance (None for a graph of probabilities without one), the threshold given to
/// landschaft's C++ (NaN without), the probability at that distance, the smallest probability kept and
/// Conefor's expansion limit.
struct Scenario {
    distance: Option<f64>,
    threshold: f64,
    probability: f64,
    pij_min: f64,
    expand: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HabitatGraph {
    pub ids: Vec<f64>,
    pub attributes: Vec<f64>,
    pub kind: Kind,
    pub landscape_area: Option<f64>,
    /// Conefor's conventions (else Makurhini's).
    pub conefor: bool,
    /// Per node, whether it exists (true) or is to be added (false).
    pub restoration: Option<Vec<bool>>,
    /// The habitat's area, of graphs made from patches.
    pub habitat_area: Option<f64>,
    pub pi: Vec<i32>,
    pub pj: Vec<i32>,
    pub value: Vec<f64>,
    /// The nodes' cells, of graphs made from a raster.
    #[serde(skip)]
    pub patches: Option<std::sync::Arc<super::raster::NodeRaster>>,
    /// The features of graphs made from them.
    #[serde(skip)]
    pub features: Option<std::sync::Arc<terraflux_raster::vector::Vector>>,
}

/// A table from its columns.
pub fn table(cols: Vec<(String, Vec<f64>)>) -> Table {
    let n = cols.first().map_or(0, |c| c.1.len());
    let mut t = Table::new(cols.iter().map(|c| c.0.clone()).collect());
    t.rows = (0..n).map(|r| cols.iter().map(|c| c.1[r]).collect()).collect();
    t
}

/// The key of an id for lookups (0 and -0 the same).
fn id_key(v: f64) -> u64 {
    if v == 0.0 { 0 } else { v.to_bits() }
}

impl HabitatGraph {
    pub fn new(
        attributes: Vec<f64>,
        connections: Connections,
        ids: Option<Vec<f64>>,
        kind: Kind,
        landscape_area: Option<f64>,
        conefor: bool,
        restoration: Option<Vec<f64>>,
    ) -> Result<HabitatGraph, String> {
        let n = attributes.len();
        if n < 1 {
            return Err("a habitat graph needs at least one node".into());
        }
        if attributes.iter().any(|a| !a.is_finite() || *a < 0.0) {
            return Err("node attributes must be finite and non-negative".into());
        }
        let ids = ids.unwrap_or_else(|| (1..=n).map(|i| i as f64).collect());
        if ids.len() != n {
            return Err("ids must have one entry per node".into());
        }
        let mut index: HashMap<u64, usize> = HashMap::new();
        for (k, &id) in ids.iter().enumerate() {
            if index.insert(id_key(id), k).is_some() {
                return Err("node ids must be unique".into());
            }
        }
        let restoration = match restoration {
            None => None,
            Some(flags) => {
                if flags.len() != n || flags.iter().any(|f| *f != 0.0 && *f != 1.0) {
                    return Err("restoration must be 1 (existing node) or 0 (node to add) for every node".into());
                }
                if !flags.contains(&1.0) {
                    return Err("restoration: no existing node".into());
                }
                Some(flags.iter().map(|f| *f == 1.0).collect())
            }
        };
        let mut g = HabitatGraph {
            ids,
            attributes,
            kind,
            landscape_area,
            conefor,
            restoration,
            habitat_area: None,
            pi: Vec::new(),
            pj: Vec::new(),
            value: Vec::new(),
            patches: None,
            features: None,
        };
        match connections {
            Connections::Matrix(m) => {
                if m.len() != n * n {
                    return Err("the connection matrix must be n x n".into());
                }
                for i in 0..n {
                    for j in i + 1..n {
                        g.pi.push(i as i32);
                        g.pj.push(j as i32);
                        g.value.push(m[i * n + j]);
                    }
                }
            }
            Connections::Pairs(rows) => {
                let mut pairs: Vec<(usize, usize, f64)> = Vec::new();
                for r in &rows {
                    let (Some(&fi), Some(&fj)) = (index.get(&id_key(r[0])), index.get(&id_key(r[1]))) else {
                        return Err("the pair table refers to ids that are not nodes".into());
                    };
                    if fi != fj {
                        pairs.push((fi.min(fj), fi.max(fj), r[2]));
                    }
                }
                // drop_duplicates, then no pair twice
                let same = |a: f64, b: f64| a == b || (a.is_nan() && b.is_nan());
                let mut kept: Vec<(usize, usize, f64)> = Vec::new();
                let mut seen: HashMap<(usize, usize), f64> = HashMap::new();
                for (i, j, v) in pairs {
                    match seen.get(&(i, j)) {
                        Some(&w) if same(v, w) => {}
                        Some(_) => return Err("a pair appears twice with different values".into()),
                        None => {
                            seen.insert((i, j), v);
                            kept.push((i, j, v));
                        }
                    }
                }
                kept.sort_by_key(|p| (p.0, p.1));
                for (i, j, v) in kept {
                    g.pi.push(i as i32);
                    g.pj.push(j as i32);
                    g.value.push(v);
                }
            }
        }
        Ok(g)
    }

    /// A graph read from Conefor's node file (id and attribute per line) and connection file ("id id value"
    /// lines or an n x n matrix).
    pub fn from_conefor(
        nodes: &str,
        connections: &str,
        kind: Kind,
        landscape_area: Option<f64>,
        conefor: bool,
    ) -> Result<HabitatGraph, String> {
        let rows = |text: &str| -> Result<Vec<Vec<f64>>, String> {
            text.lines()
                .map(|l| l.split('#').next().unwrap_or("").trim())
                .filter(|l| !l.is_empty())
                .map(|l| {
                    l.split(|c: char| c.is_whitespace() || c == ',')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.parse::<f64>().map_err(|_| format!("not a number: {s:?}")))
                        .collect()
                })
                .collect()
        };
        let nodes = rows(nodes)?;
        if nodes.is_empty() || nodes.iter().any(|r| r.len() < 2) {
            return Err("a Conefor node file has an id and an attribute per line".into());
        }
        let n = nodes.len();
        let values = rows(connections)?;
        let width = values.first().map_or(0, |r| r.len());
        if values.iter().any(|r| r.len() != width) {
            return Err("the rows of the connection file differ in length".into());
        }
        let square = values.len() == n && width == n;
        if !square && width != 3 {
            return Err("a Conefor connection file holds 'id id value' lines or an n x n matrix".into());
        }
        let connections = if square {
            Connections::Matrix(values.into_iter().flatten().collect())
        } else {
            Connections::Pairs(values.into_iter().map(|r| [r[0], r[1], r[2]]).collect())
        };
        HabitatGraph::new(
            nodes.iter().map(|r| r[1]).collect(),
            connections,
            Some(nodes.iter().map(|r| r[0]).collect()),
            kind,
            landscape_area,
            conefor,
            None,
        )
    }

    /// Conefor's node and connection files (the pairs with a value).
    pub fn to_conefor(&self) -> (String, String) {
        let id = |v: f64| if v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", v as i64) } else { py(v) };
        let mut nodes = String::new();
        for (i, a) in self.ids.iter().zip(&self.attributes) {
            nodes += &format!("{}\t{}\n", id(*i), py(*a));
        }
        let mut conns = String::new();
        for k in 0..self.value.len() {
            if !self.value[k].is_nan() {
                let (i, j) = (self.ids[self.pi[k] as usize], self.ids[self.pj[k] as usize]);
                conns += &format!("{}\t{}\t{}\n", id(i), id(j), py(self.value[k]));
            }
        }
        (nodes, conns)
    }

    pub fn n_nodes(&self) -> usize {
        self.attributes.len()
    }

    fn defaults(
        &self,
        distance: Option<f64>,
        probability: Option<f64>,
        pij_min: Option<f64>,
    ) -> Result<(Option<f64>, f64, f64), String> {
        let mut distance = distance;
        if self.kind == Kind::Distance && distance.is_none() {
            if self.conefor {
                return Err("give the distance (threshold) of the links and probabilities".into());
            }
            // the median of the distances twice and a zero per node
            let v: Vec<f64> = self.value.iter().copied().filter(|v| !v.is_nan()).collect();
            let mut all: Vec<f64> =
                v.iter().chain(&v).copied().chain(std::iter::repeat_n(0.0, self.n_nodes())).collect();
            all.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let m = all.len();
            distance = Some(if m % 2 == 1 { all[m / 2] } else { (all[m / 2 - 1] + all[m / 2]) / 2.0 });
        }
        let probability = probability.unwrap_or(0.5);
        let pij_min = pij_min.unwrap_or(if self.conefor { 0.0 } else { 0.01 });
        Ok((distance, probability, pij_min))
    }

    fn scenario(
        &self,
        metrics: &[String],
        distance: Option<f64>,
        probability: Option<f64>,
        pij_min: Option<f64>,
    ) -> Result<Scenario, String> {
        let (distance, probability, pij_min) = self.defaults(distance, probability, pij_min)?;
        if self.kind == Kind::Probability && any_of(metrics, &LINK_METRICS) && distance.is_none() {
            return Err("links between probabilities need a minimum probability (distance)".into());
        }
        let expand = if self.conefor && pij_min > 0.0 { -pij_min.ln() } else { f64::INFINITY };
        let threshold = distance.unwrap_or(f64::NAN);
        if any_of(metrics, &PROBABILITY_METRICS)
            && self.kind == Kind::Distance
            && (threshold.is_nan() || threshold <= 0.0)
        {
            return Err("the distance of the probability must be positive".into());
        }
        Ok(Scenario { distance, threshold, probability, pij_min, expand })
    }

    fn run(
        &self,
        metrics: &[String],
        distance: Option<f64>,
        probability: Option<f64>,
        pij_min: Option<f64>,
        importance: bool,
        edges: bool,
    ) -> Result<(Option<f64>, ffi::ConnAnalysis), String> {
        let s = self.scenario(metrics, distance, probability, pij_min)?;
        let r = ffi::conn_analyse(
            &self.pi,
            &self.pj,
            &self.value,
            &self.attributes,
            self.kind.code(),
            s.threshold,
            s.probability,
            s.pij_min,
            s.expand,
            self.conefor,
            any_of(metrics, &LINK_METRICS),
            any_of(metrics, &PROBABILITY_METRICS),
            importance,
            edges,
            importance && any_of(metrics, &BETWEENNESS),
        )
        .map_err(|e| e.to_string())?;
        Ok((s.distance, r))
    }

    fn area(&self) -> Option<f64> {
        self.landscape_area.or(if self.conefor { Some(1.0) } else { None })
    }

    fn unconnected(&self, aa: f64) -> f64 {
        if self.conefor { 0.0 * aa } else { aa / (1.0 + MAKURHINI_UNCONNECTED) }
    }

    /// The index (num / area², capped at 1 by Makurhini), its numerator and the equivalent connectivity.
    fn index(&self, name: &str, num: f64, la: Option<f64>, out: &mut Vec<(String, f64)>) {
        let mut ec = num.sqrt();
        let value = match la {
            None => f64::NAN,
            Some(la) => {
                let mut v = num / (la * la);
                if !self.conefor {
                    v = py_min(v, 1.0);
                    ec = py_min(ec, la);
                }
                v
            }
        };
        out.push((name.into(), value));
        out.push((format!("{name}num"), num));
        out.push((format!("EC({name})"), ec));
    }

    fn fractions(name: &str, num: f64, intra: f64, direct: f64, step: f64, out: &mut Vec<(String, f64)>) {
        out.push((format!("{name}intra(%)"), 100.0 * intra / num));
        out.push((format!("{name}direct(%)"), 100.0 * direct / num));
        out.push((format!("{name}step(%)"), 100.0 * step / num));
    }

    /// The graph's indices at each distance (a row each).
    pub fn overall(
        &self,
        metrics: Option<&[String]>,
        distances: &[Option<f64>],
        probability: Option<f64>,
        pij_min: Option<f64>,
    ) -> Result<Table, String> {
        let chosen = metric_list(metrics, self.kind)?;
        let sub;
        let graph = match &self.restoration {
            Some(flags) if self.conefor => {
                sub = self.subgraph(flags);
                &sub
            }
            _ => self,
        };
        let mut rows = Vec::new();
        for &d in distances {
            rows.push(graph.overall_row(&chosen, d, probability, pij_min)?);
        }
        Ok(rows_table(rows))
    }

    fn overall_row(
        &self,
        chosen: &[String],
        distance: Option<f64>,
        probability: Option<f64>,
        pij_min: Option<f64>,
    ) -> Result<Vec<(String, f64)>, String> {
        let (d, r) = self.run(chosen, distance, probability, pij_min, false, false)?;
        let a = &self.attributes;
        let la = self.area();
        let total = numpy_sum(a);
        let intra = dot(a, a);
        let mut out = vec![("distance".to_string(), d.unwrap_or(f64::NAN))];
        if has(chosen, "A") {
            out.push(("A".into(), total));
        }
        if has(chosen, "NC") {
            out.push(("NC".into(), r.nc as f64));
        }
        if has(chosen, "NL") {
            out.push(("NL".into(), r.nl as f64));
        }
        if has(chosen, "H") {
            out.push(("H".into(), r.h));
        }
        if has(chosen, "CCP") {
            out.push(("CCP".into(), r.comp_sq / (total * total)));
        }
        if has(chosen, "LCP") {
            out.push(("LCP".into(), la.map_or(f64::NAN, |la| r.comp_sq / (la * la))));
            out.push(("LCPnum".into(), r.comp_sq));
        }
        if has(chosen, "IIC") {
            let reach = r.iic_reach;
            let num = reach + self.unconnected(r.unreach_aa);
            self.index("IIC", num, la, &mut out);
            let direct = r.iic_direct;
            Self::fractions("IIC", num, intra, direct, reach - intra - direct, &mut out);
        }
        if has(chosen, "F") {
            out.push(("F".into(), numpy_sum(&r.f_row)));
        }
        if has(chosen, "AWF") {
            out.push(("AWF".into(), dot(a, &r.awf_row)));
        }
        if has(chosen, "PC") {
            let num = r.pc_num;
            self.index("PC", num, la, &mut out);
            let direct = r.pc_direct;
            Self::fractions("PC", num, intra, direct, num - intra - direct, &mut out);
        }
        Ok(out)
    }

    /// Every node's importance at each distance (its removal's, or for graphs to restore, its addition's).
    pub fn node_importance(
        &self,
        metrics: Option<&[String]>,
        distances: &[Option<f64>],
        probability: Option<f64>,
        pij_min: Option<f64>,
    ) -> Result<Table, String> {
        let chosen = metric_list(metrics, self.kind)?;
        let mut frames = Vec::new();
        for &d in distances {
            frames.push(match &self.restoration {
                Some(flags) => self.restoration_frame(flags, &chosen, d, probability, pij_min)?,
                None => self.removal_frame(&chosen, d, probability, pij_min)?,
            });
        }
        Ok(concat(frames))
    }

    fn removal_frame(
        &self,
        chosen: &[String],
        distance: Option<f64>,
        probability: Option<f64>,
        pij_min: Option<f64>,
    ) -> Result<Vec<(String, Vec<f64>)>, String> {
        let (d, r) = self.run(chosen, distance, probability, pij_min, true, false)?;
        let a = &self.attributes;
        let n = a.len();
        let la = self.area();
        let total = numpy_sum(a);
        let a2: Vec<f64> = a.iter().map(|x| x * x).collect();
        let map = |f: &dyn Fn(usize) -> f64| -> Vec<f64> { (0..n).map(f).collect() };
        let mut cols: Vec<(String, Vec<f64>)> =
            vec![("id".into(), self.ids.clone()), ("distance".into(), vec![d.unwrap_or(f64::NAN); n])];
        if has(chosen, "A") {
            cols.push(("dA".into(), map(&|k| 100.0 * a[k] / total)));
        }
        let mut rest_sq = Vec::new();
        if any_of(chosen, &LINK_METRICS) {
            let comp = &r.component;
            let mut sums = vec![0.0; comp.iter().map(|&c| c as usize + 1).max().unwrap_or(0)];
            for (k, &c) in comp.iter().enumerate() {
                sums[c as usize] += a[k];
            }
            rest_sq = map(&|k| {
                let ca = sums[comp[k] as usize];
                r.comp_sq - ca * ca + r.pieces_sq[k]
            });
        }
        if has(chosen, "NC") {
            cols.push(("dNC".into(), map(&|k| 100.0 * (1 - r.pieces[k]) as f64 / r.nc as f64)));
        }
        if has(chosen, "NL") {
            let nl = r.nl;
            cols.push(("dNL".into(), map(&|k| if nl != 0 { 100.0 * r.degree[k] as f64 / nl as f64 } else { 0.0 })));
        }
        if has(chosen, "H") {
            let h = r.h;
            cols.push(("dH".into(), map(&|k| if h != 0.0 { 100.0 * (r.h_row[k] + r.h_conn[k]) / h } else { 0.0 })));
        }
        if has(chosen, "CCP") {
            let ccp = r.comp_sq / (total * total);
            cols.push((
                "dCCP".into(),
                map(&|k| {
                    let del = rest_sq[k] / ((total - a[k]) * (total - a[k]));
                    100.0 * (ccp - del) / ccp
                }),
            ));
        }
        if has(chosen, "LCP") {
            cols.push(("dLCP".into(), map(&|k| 100.0 * (r.comp_sq - rest_sq[k]) / r.comp_sq)));
        }
        if has(chosen, "IIC") {
            let num = r.iic_reach + self.unconnected(r.unreach_aa);
            let flux_reach = map(&|k| 2.0 * (a[k] * r.iic_row[k] - a2[k]));
            let conn = &r.iic_conn;
            let (delta, flux, connector): (Vec<f64>, Vec<f64>, Vec<f64>) = if !self.conefor {
                let whole = self.unconnected(r.unreach_aa);
                let own = map(&|k| self.unconnected(a[k] * r.unreach_a[k]));
                (
                    map(&|k| a2[k] + flux_reach[k] + conn[k] + whole),
                    map(&|k| flux_reach[k] + 2.0 * own[k]),
                    map(&|k| np_max0(conn[k] + whole - 2.0 * own[k])),
                )
            } else {
                (map(&|k| a2[k] + flux_reach[k] + conn[k]), flux_reach.clone(), conn.to_vec())
            };
            cols.push(("dIIC".into(), map(&|k| 100.0 * delta[k] / num)));
            cols.push(("dIICintra".into(), map(&|k| 100.0 * a2[k] / num)));
            cols.push(("dIICflux".into(), map(&|k| 100.0 * flux[k] / num)));
            cols.push(("dIICconnector".into(), map(&|k| 100.0 * connector[k] / num)));
        }
        if has(chosen, "F") {
            let f = &r.f_row;
            let s = numpy_sum(f);
            cols.push(("dF".into(), map(&|k| 200.0 * f[k] / s)));
        }
        if has(chosen, "AWF") {
            let w = map(&|k| a[k] * r.awf_row[k]);
            let s = numpy_sum(&w);
            cols.push(("dAWF".into(), map(&|k| 200.0 * w[k] / s)));
        }
        if has(chosen, "PC") {
            let num = r.pc_num;
            let flux = map(&|k| 2.0 * (a[k] * r.pc_row[k] - a2[k]));
            let conn = &r.pc_conn;
            let delta = map(&|k| a2[k] + flux[k] + conn[k]);
            cols.push(("dPC".into(), map(&|k| 100.0 * delta[k] / num)));
            cols.push(("dPCintra".into(), map(&|k| 100.0 * a2[k] / num)));
            cols.push(("dPCflux".into(), map(&|k| 100.0 * flux[k] / num)));
            cols.push(("dPCconnector".into(), map(&|k| 100.0 * conn[k] / num)));
            cols.push(("varPC".into(), map(&|k| la.map_or(f64::NAN, |la| delta[k] / (la * la)))));
        }
        if has(chosen, "BC") {
            let pairs = ((n as f64 - 1.0) * (n as f64 - 2.0)) / 2.0;
            cols.push(("BC".into(), map(&|k| if n > 2 { r.bc[k] / 2.0 / pairs } else { 0.0 })));
        }
        if has(chosen, "BCIIC") {
            cols.push(("dBC_IIC".into(), map(&|k| 100.0 * r.bc_iic[k] / r.iic_reach)));
            cols.push(("varBC_IIC".into(), r.bc_iic.to_vec()));
        }
        if has(chosen, "BCPC") {
            cols.push(("dBC_PC".into(), map(&|k| 100.0 * r.bc_pc[k] / r.pc_num)));
            cols.push(("varBC_PC".into(), r.bc_pc.to_vec()));
        }
        Ok(cols)
    }

    /// The component (1, 2...) of every node at a distance.
    pub fn components(&self, distance: Option<f64>) -> Result<Table, String> {
        let (_, r) = self.run(&["NC".to_string()], distance, None, None, false, false)?;
        Ok(table(vec![
            ("id".into(), self.ids.clone()),
            ("component".into(), r.component.iter().map(|&c| c as f64 + 1.0).collect()),
        ]))
    }

    fn subgraph(&self, keep: &[bool]) -> HabitatGraph {
        let mut index = vec![-1i32; self.n_nodes()];
        let mut next = 0;
        for (k, &kp) in keep.iter().enumerate() {
            if kp {
                index[k] = next;
                next += 1;
            }
        }
        let both: Vec<usize> =
            (0..self.pi.len()).filter(|&k| keep[self.pi[k] as usize] && keep[self.pj[k] as usize]).collect();
        HabitatGraph {
            ids: (0..self.n_nodes()).filter(|&k| keep[k]).map(|k| self.ids[k]).collect(),
            attributes: (0..self.n_nodes()).filter(|&k| keep[k]).map(|k| self.attributes[k]).collect(),
            kind: self.kind,
            landscape_area: self.landscape_area,
            conefor: self.conefor,
            restoration: None,
            habitat_area: None,
            pi: both.iter().map(|&k| index[self.pi[k] as usize]).collect(),
            pj: both.iter().map(|&k| index[self.pj[k] as usize]).collect(),
            value: both.iter().map(|&k| self.value[k]).collect(),
            patches: None,
            features: None,
        }
    }

    fn lengths(&self, values: &[f64], s: &Scenario, pij_min: f64, links: bool, cut: bool) -> Result<Vec<f64>, String> {
        ffi::conn_pair_lengths(values, self.kind.code(), s.threshold, s.probability, pij_min, self.conefor, links, cut)
            .map_err(|e| e.to_string())
    }

    fn all_pairs(&self, s: &Scenario, links: bool) -> Result<Vec<f64>, String> {
        let n = self.n_nodes();
        if (n * n) as f64 > 4e8 {
            return Err(format!("all-pairs lengths of {n} nodes do not fit in memory"));
        }
        ffi::conn_all_pairs(
            &self.pi,
            &self.pj,
            &self.value,
            n as i32,
            self.kind.code(),
            s.threshold,
            s.probability,
            s.pij_min,
            s.expand,
            self.conefor,
            links,
        )
        .map_err(|e| e.to_string())
    }

    /// a f(D) a, f = 1 / (1 + hops) or exp(-cost).
    fn numerator(d: &[f64], a: &[f64], links: bool) -> f64 {
        let n = a.len();
        let mut af = vec![0.0; n];
        for (i, ai) in a.iter().enumerate() {
            for j in 0..n {
                let x = d[i * n + j];
                let f = if links { if x.is_finite() { 1.0 / (1.0 + x) } else { 0.0 } } else { (-x).exp() };
                af[j] += ai * f;
            }
        }
        dot(&af, a)
    }

    fn restoration_frame(
        &self,
        flags: &[bool],
        chosen: &[String],
        distance: Option<f64>,
        probability: Option<f64>,
        pij_min: Option<f64>,
    ) -> Result<Vec<(String, Vec<f64>)>, String> {
        let base = flags;
        let n = self.n_nodes();
        let existing = self.subgraph(base);
        if !self.conefor {
            let mut frame = self.removal_frame(chosen, distance, probability, pij_min)?;
            for metric in ["IIC", "PC"] {
                if !has(chosen, metric) {
                    continue;
                }
                let (gain, num_base, _) =
                    self.addition_gains(&existing, base, metric, distance, probability, pij_min)?;
                let mut col = vec![0.0; n];
                let mut c = 0;
                for k in 0..n {
                    if !base[k] {
                        col[k] = 100.0 * gain[c] / (num_base + gain[c]);
                        c += 1;
                    }
                }
                frame.push((format!("d{metric}res"), col));
            }
            return Ok(frame);
        }
        let removal = existing.removal_frame(chosen, distance, probability, pij_min)?;
        let d0 = removal[1].1.first().copied().unwrap_or(f64::NAN);
        let mut frame: Vec<(String, Vec<f64>)> =
            vec![("id".into(), self.ids.clone()), ("distance".into(), vec![d0; n])];
        let spread = |values: &[f64], on: bool| -> Vec<f64> {
            let mut out = vec![f64::NAN; n];
            let mut c = 0;
            for k in 0..n {
                if base[k] == on {
                    out[k] = values[c];
                    c += 1;
                }
            }
            out
        };
        for (name, values) in &removal[2..] {
            frame.push((name.clone(), spread(values, true)));
        }
        for (name, values) in self.conefor_additions(&existing, base, chosen, distance, probability, pij_min)? {
            frame.push((name, spread(&values, false)));
        }
        Ok(frame)
    }

    /// The edges from each candidate (a node to add) to the existing nodes: (start per candidate, node, length).
    fn candidate_edges(&self, base: &[bool], lengths: &[f64]) -> (Vec<i64>, Vec<i32>, Vec<f64>) {
        let n = self.n_nodes();
        let (mut cand_index, mut base_index) = (vec![-1i64; n], vec![-1i64; n]);
        let (mut nc, mut nb) = (0, 0);
        for k in 0..n {
            if base[k] {
                base_index[k] = nb;
                nb += 1;
            } else {
                cand_index[k] = nc;
                nc += 1;
            }
        }
        let mut rows: Vec<(i64, i64, f64)> = Vec::new();
        for (c_end, b_end) in [(&self.pi, &self.pj), (&self.pj, &self.pi)] {
            for k in 0..lengths.len() {
                let (c, b) = (c_end[k] as usize, b_end[k] as usize);
                if !lengths[k].is_nan() && !base[c] && base[b] {
                    rows.push((cand_index[c], base_index[b], lengths[k]));
                }
            }
        }
        rows.sort_by_key(|r| r.0); // stable
        let mut start = vec![0i64; nc as usize + 1];
        for (k, s) in start.iter_mut().enumerate() {
            *s = rows.partition_point(|r| r.0 < k as i64) as i64;
        }
        (start, rows.iter().map(|r| r.1 as i32).collect(), rows.iter().map(|r| r.2).collect())
    }

    fn addition_gains(
        &self,
        existing: &HabitatGraph,
        base: &[bool],
        metric: &str,
        distance: Option<f64>,
        probability: Option<f64>,
        pij_min: Option<f64>,
    ) -> Result<(Vec<f64>, f64, ffi::ConnGains), String> {
        let links = metric == "IIC";
        let s = self.scenario(&[metric.to_string()], distance, probability, pij_min)?;
        let d = existing.all_pairs(&s, links)?;
        let lengths = self.lengths(&self.value, &s, s.pij_min, links, true)?;
        let (start, node, length) = self.candidate_edges(base, &lengths);
        let cand_area: Vec<f64> = (0..self.n_nodes()).filter(|&k| !base[k]).map(|k| self.attributes[k]).collect();
        let g = ffi::conn_addition_gains(&d, &existing.attributes, &start, &node, &length, &cand_area, links)
            .map_err(|e| e.to_string())?;
        Ok((g.num.to_vec(), Self::numerator(&d, &existing.attributes, links), g))
    }

    fn conefor_additions(
        &self,
        existing: &HabitatGraph,
        base: &[bool],
        chosen: &[String],
        distance: Option<f64>,
        probability: Option<f64>,
        pij_min: Option<f64>,
    ) -> Result<Vec<(String, Vec<f64>)>, String> {
        let a_base = &existing.attributes;
        let a_cand: Vec<f64> = (0..self.n_nodes()).filter(|&k| !base[k]).map(|k| self.attributes[k]).collect();
        let m = a_cand.len();
        let total = numpy_sum(a_base);
        let mut out: Vec<(String, Vec<f64>)> = Vec::new();
        let s = self.scenario(chosen, distance, probability, pij_min)?;
        if has(chosen, "A") {
            out.push(("dAadd".into(), a_cand.iter().map(|x| 100.0 * x / total).collect()));
        }
        if any_of(chosen, &LINK_METRICS) {
            let (_, r) = existing.run(&["NC".to_string()], distance, probability, pij_min, false, false)?;
            let comp = &r.component;
            let mut comp_area = vec![0.0; r.nc as usize];
            for (k, &c) in comp.iter().enumerate() {
                comp_area[c as usize] += a_base[k];
            }
            let lengths = self.lengths(&self.value, &s, s.pij_min, true, true)?;
            let (start, node, _) = self.candidate_edges(base, &lengths);
            let degree: Vec<f64> = start.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
            let touched: Vec<Vec<usize>> = (0..m)
                .map(|k| {
                    let mut t: Vec<usize> = node[start[k] as usize..start[k + 1] as usize]
                        .iter()
                        .map(|&b| comp[b as usize] as usize)
                        .collect();
                    t.sort_unstable();
                    t.dedup();
                    t
                })
                .collect();
            let nc = r.nc as f64;
            let nc_add: Vec<f64> = touched.iter().map(|t| (r.nc as i64 + 1 - t.len() as i64) as f64).collect();
            let merged: Vec<f64> = touched
                .iter()
                .enumerate()
                .map(|(k, t)| a_cand[k] + numpy_sum(&t.iter().map(|&c| comp_area[c]).collect::<Vec<_>>()))
                .collect();
            let sq_add: Vec<f64> = touched
                .iter()
                .enumerate()
                .map(|(k, t)| {
                    r.comp_sq - numpy_sum(&t.iter().map(|&c| comp_area[c] * comp_area[c]).collect::<Vec<_>>())
                        + merged[k] * merged[k]
                })
                .collect();
            if has(chosen, "NC") {
                out.push(("dNCadd".into(), nc_add.iter().map(|x| 100.0 * (x - nc) / nc).collect()));
            }
            if has(chosen, "NL") {
                let nl = r.nl;
                out.push((
                    "dNLadd".into(),
                    degree.iter().map(|x| if nl != 0 { 100.0 * x / nl as f64 } else { 0.0 }).collect(),
                ));
            }
            if has(chosen, "CCP") {
                let ccp = r.comp_sq / (total * total);
                out.push((
                    "dCCPadd".into(),
                    (0..m)
                        .map(|k| {
                            let t = total + a_cand[k];
                            100.0 * (sq_add[k] / (t * t) - ccp) / ccp
                        })
                        .collect(),
                ));
            }
            if has(chosen, "LCP") {
                out.push(("dLCPadd".into(), sq_add.iter().map(|x| 100.0 * (x - r.comp_sq) / r.comp_sq).collect()));
            }
            if has(chosen, "IIC") || has(chosen, "H") {
                let (gain, num_base, g) = self.addition_gains(existing, base, "IIC", distance, probability, pij_min)?;
                if has(chosen, "H") {
                    let (_, rh) = existing.run(&["H".to_string()], distance, probability, pij_min, false, false)?;
                    out.push((
                        "dHadd".into(),
                        g.h.iter().map(|x| if rh.h != 0.0 { 100.0 * x / rh.h } else { 0.0 }).collect(),
                    ));
                }
                if has(chosen, "IIC") {
                    out.push(("dIICadd".into(), gain.iter().map(|x| 100.0 * x / num_base).collect()));
                }
            }
        }
        if any_of(chosen, &PROBABILITY_METRICS) {
            let uncut = self.lengths(&self.value, &s, 0.0, false, false)?;
            let p: Vec<f64> = uncut.iter().map(|x| (-x).exp()).map(|p| if p.is_nan() { 0.0 } else { p }).collect();
            let mut cand_pos = vec![-1i64; self.n_nodes()];
            let mut c = 0;
            for k in 0..self.n_nodes() {
                if !base[k] {
                    cand_pos[k] = c;
                    c += 1;
                }
            }
            let (mut f_add, mut awf_add) = (vec![0.0; m], vec![0.0; m]);
            for (c_end, b_end) in [(&self.pi, &self.pj), (&self.pj, &self.pi)] {
                for k in 0..p.len() {
                    let (ce, be) = (c_end[k] as usize, b_end[k] as usize);
                    if !base[ce] && base[be] {
                        f_add[cand_pos[ce] as usize] += p[k];
                    }
                }
                for k in 0..p.len() {
                    let (ce, be) = (c_end[k] as usize, b_end[k] as usize);
                    if !base[ce] && base[be] {
                        awf_add[cand_pos[ce] as usize] += self.attributes[be] * p[k];
                    }
                }
            }
            if has(chosen, "F") || has(chosen, "AWF") {
                let (_, rf) = existing.run(&["F".to_string()], distance, probability, pij_min, false, false)?;
                if has(chosen, "F") {
                    let s = numpy_sum(&rf.f_row);
                    out.push(("dFadd".into(), f_add.iter().map(|x| 200.0 * x / s).collect()));
                }
                if has(chosen, "AWF") {
                    let s = dot(a_base, &rf.awf_row);
                    out.push(("dAWFadd".into(), (0..m).map(|k| 200.0 * a_cand[k] * awf_add[k] / s).collect()));
                }
            }
            if has(chosen, "PC") {
                let (gain, num_base, _) = self.addition_gains(existing, base, "PC", distance, probability, pij_min)?;
                out.push(("dPCadd".into(), gain.iter().map(|x| 100.0 * x / num_base).collect()));
            }
        }
        Ok(out)
    }

    /// The share of IIC or PC lost when each link (edge) is removed.
    pub fn link_importance(
        &self,
        metric: &str,
        distance: Option<f64>,
        probability: Option<f64>,
        pij_min: Option<f64>,
    ) -> Result<Table, String> {
        let metric = metric.to_uppercase();
        if metric != "IIC" && metric != "PC" {
            return Err("the metric is IIC or PC".into());
        }
        let (d, r) = self.run(std::slice::from_ref(&metric), distance, probability, pij_min, false, true)?;
        let (num, delta, i, j): (f64, Vec<f64>, &[i32], &[i32]) = if metric == "IIC" {
            let whole = self.unconnected(r.unreach_aa);
            (r.iic_reach + whole, r.edge_iic.iter().map(|x| x + whole).collect(), &r.link_i, &r.link_j)
        } else {
            (r.pc_num, r.edge_pc.to_vec(), &r.edge_i, &r.edge_j)
        };
        Ok(table(vec![
            ("from".into(), i.iter().map(|&k| self.ids[k as usize]).collect()),
            ("to".into(), j.iter().map(|&k| self.ids[k as usize]).collect()),
            ("distance".into(), vec![d.unwrap_or(f64::NAN); i.len()]),
            (format!("d{metric}_removal"), delta.iter().map(|x| 100.0 * x / num).collect()),
        ]))
    }

    /// The change of IIC or PC when each pair's value changes (a link made or lost, a probability raised or
    /// lowered).
    pub fn link_change(
        &self,
        changes: &Connections,
        metric: &str,
        distance: Option<f64>,
        probability: Option<f64>,
        pij_min: Option<f64>,
    ) -> Result<Table, String> {
        let metric = metric.to_uppercase();
        if metric != "IIC" && metric != "PC" {
            return Err("the metric is IIC or PC".into());
        }
        let links = metric == "IIC";
        let n = self.n_nodes();
        let (ci, cj, cv) = self.change_pairs(changes)?;
        let s = self.scenario(std::slice::from_ref(&metric), distance, probability, pij_min)?;
        let key: Vec<i64> = (0..self.pi.len()).map(|k| self.pi[k] as i64 * n as i64 + self.pj[k] as i64).collect();
        let mut pos = Vec::with_capacity(ci.len());
        let mut old_value = Vec::with_capacity(ci.len());
        for k in 0..ci.len() {
            let wanted = ci[k] as i64 * n as i64 + cj[k] as i64;
            let p = key.partition_point(|&x| x < wanted).min(key.len().saturating_sub(1));
            pos.push(p);
            old_value.push(if !key.is_empty() && key[p] == wanted { self.value[p] } else { f64::NAN });
        }
        let old_len = self.lengths(&old_value, &s, s.pij_min, links, true)?;
        let new_len = self.lengths(&cv, &s, s.pij_min, links, links)?;
        let d = self.all_pairs(&s, links)?;
        let a = &self.attributes;
        let num_graph = Self::numerator(&d, a, links);
        let num = if links {
            let (_, r) = self.run(&["IIC".to_string()], distance, probability, pij_min, false, false)?;
            num_graph + self.unconnected(r.unreach_aa)
        } else {
            num_graph
        };
        let mut out = vec![0.0; ci.len()];
        let differs: Vec<bool> = (0..ci.len()).map(|k| cv[k] != old_value[k]).collect();
        let gains = |sel: &[usize], len: Vec<f64>, d: &[f64]| -> Result<Vec<f64>, String> {
            let ea: Vec<i32> = sel.iter().map(|&k| ci[k]).collect();
            let eb: Vec<i32> = sel.iter().map(|&k| cj[k]).collect();
            Ok(ffi::conn_edge_gains(d, a, &ea, &eb, &len, links).map_err(|e| e.to_string())?.num.to_vec())
        };
        if links {
            let apply: Vec<usize> = (0..ci.len()).filter(|&k| differs[k] && !new_len[k].is_nan()).collect();
            let new_link: Vec<usize> = apply.iter().copied().filter(|&k| old_len[k].is_nan()).collect();
            if !new_link.is_empty() {
                for (k, g) in new_link.iter().zip(gains(&new_link, vec![1.0; new_link.len()], &d)?) {
                    out[*k] = g;
                }
            }
            for k in apply {
                out[k] = (num - (num_graph + out[k])) / num * 100.0;
            }
        } else {
            let better: Vec<bool> = (0..ci.len())
                .map(|k| differs[k] && !new_len[k].is_nan() && (old_len[k].is_nan() || new_len[k] < old_len[k]))
                .collect();
            let sel: Vec<usize> = (0..ci.len()).filter(|&k| better[k]).collect();
            if !sel.is_empty() {
                let len = sel.iter().map(|&k| new_len[k]).collect();
                for (k, g) in sel.iter().zip(gains(&sel, len, &d)?) {
                    out[*k] = -100.0 * g / num;
                }
            }
            for k in 0..ci.len() {
                if differs[k] && !better[k] && !old_len[k].is_nan() {
                    out[k] = 100.0 * (num - self.replaced(pos[k], new_len[k], &s)?) / num;
                }
            }
        }
        Ok(table(vec![
            ("from".into(), ci.iter().map(|&k| self.ids[k as usize]).collect()),
            ("to".into(), cj.iter().map(|&k| self.ids[k as usize]).collect()),
            ("distance".into(), vec![s.distance.unwrap_or(f64::NAN); ci.len()]),
            (format!("d{metric}_change"), out),
        ]))
    }

    /// PC's numerator once pair `pos` is replaced by an edge of length `new_len` (NaN: removed).
    fn replaced(&self, pos: usize, new_len: f64, s: &Scenario) -> Result<f64, String> {
        let mut sub = self.clone();
        sub.value[pos] = f64::NAN;
        let d = sub.all_pairs(s, false)?;
        let num = Self::numerator(&d, &self.attributes, false);
        if new_len.is_nan() {
            return Ok(num);
        }
        let g = ffi::conn_edge_gains(
            &d,
            &self.attributes,
            &self.pi[pos..pos + 1],
            &self.pj[pos..pos + 1],
            &[new_len],
            false,
        )
        .map_err(|e| e.to_string())?;
        Ok(num + g.num[0])
    }

    /// The pairs that change: (i < j, j, new value).
    fn change_pairs(&self, changes: &Connections) -> Result<ChangePairs, String> {
        let n = self.n_nodes();
        match changes {
            Connections::Pairs(rows) => {
                let index: HashMap<u64, usize> = self.ids.iter().enumerate().map(|(k, &id)| (id_key(id), k)).collect();
                let (mut ci, mut cj, mut cv) = (Vec::new(), Vec::new(), Vec::new());
                for r in rows {
                    let (Some(&fi), Some(&fj)) = (index.get(&id_key(r[0])), index.get(&id_key(r[1]))) else {
                        return Err("changes must join two different nodes of the graph".into());
                    };
                    if fi == fj {
                        return Err("changes must join two different nodes of the graph".into());
                    }
                    ci.push(fi.min(fj) as i32);
                    cj.push(fi.max(fj) as i32);
                    cv.push(r[2]);
                }
                Ok((ci, cj, cv))
            }
            Connections::Matrix(m) => {
                if m.len() != n * n {
                    return Err("changes must be a (from, to, value) table or an n x n matrix".into());
                }
                let mut current: HashMap<(i32, i32), f64> = HashMap::new();
                for k in 0..self.pi.len() {
                    current.insert((self.pi[k], self.pj[k]), self.value[k]);
                }
                let (mut ci, mut cj, mut cv) = (Vec::new(), Vec::new(), Vec::new());
                for i in 0..n {
                    for j in i + 1..n {
                        let v = m[i * n + j];
                        let c = current.get(&(i as i32, j as i32)).copied().unwrap_or(f64::NAN);
                        if !(v == c || (v.is_nan() && c.is_nan())) {
                            ci.push(i as i32);
                            cj.push(j as i32);
                            cv.push(v);
                        }
                    }
                }
                Ok((ci, cj, cv))
            }
        }
    }

    /// Makurhini's MK_RMCentrality at each distance: degree (or strength), eigenvector, closeness, betweenness
    /// and cluster of every node, on the links shorter than the distance (binary) or on probabilities.
    pub fn centrality(
        &self,
        distances: &[Option<f64>],
        probability: Option<f64>,
        binary: bool,
    ) -> Result<Table, String> {
        if self.kind != Kind::Distance {
            return Err("centrality needs distances between the nodes (kind distance)".into());
        }
        let binary = binary && probability.is_none();
        let mut frames = Vec::new();
        for &d in distances {
            frames.push(self.centrality_frame(d, probability, binary)?);
        }
        Ok(concat(frames))
    }

    fn centrality_frame(
        &self,
        distance: Option<f64>,
        probability: Option<f64>,
        binary: bool,
    ) -> Result<Vec<(String, Vec<f64>)>, String> {
        let n = self.n_nodes();
        let t = self.defaults(distance, None, None)?.0.unwrap_or(f64::NAN);
        let (mut i, mut j, mut weight) = (Vec::new(), Vec::new(), Vec::new());
        for k in 0..self.value.len() {
            let v = self.value[k];
            if v.is_nan() {
                continue;
            }
            let w = if binary {
                let keep = if self.conefor { v <= t } else { v < t };
                if !keep {
                    continue;
                }
                1.0
            } else {
                let p = match probability {
                    None => (-(1.0 / t) * v).exp(),
                    Some(pr) => (v * pr.ln() / t).exp(),
                };
                if p.is_nan() || p <= 0.0 {
                    continue;
                }
                p
            };
            i.push(self.pi[k]);
            j.push(self.pj[k]);
            weight.push(w);
        }
        let length: Vec<f64> = if binary { weight.clone() } else { weight.iter().map(|w| 1.0 / w).collect() };
        let c = ffi::conn_centrality(&i, &j, &length, n as i32, binary).map_err(|e| e.to_string())?;
        // the symmetric matrix of the lengths, its rows by column
        let mut rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        for k in 0..i.len() {
            rows[i[k] as usize].push((j[k] as usize, length[k]));
            rows[j[k] as usize].push((i[k] as usize, length[k]));
        }
        for r in rows.iter_mut() {
            r.sort_by_key(|e| e.0);
        }
        let strength: Vec<f64> = rows.iter().map(|r| numpy_sum(&r.iter().map(|e| e.1).collect::<Vec<_>>())).collect();
        let close: Vec<f64> = (0..n)
            .map(|k| {
                if c.reached[k] > 1 {
                    if binary { 1.0 / c.closeness_sum[k] } else { (c.reached[k] - 1) as f64 / c.closeness_sum[k] }
                } else {
                    f64::NAN
                }
            })
            .collect();
        // scipy's connected_components: labels in the order of each component's first node
        let mut cluster = vec![0.0; n];
        let mut label = 0.0;
        for s in 0..n {
            if cluster[s] != 0.0 {
                continue;
            }
            label += 1.0;
            cluster[s] = label;
            let mut stack = vec![s];
            while let Some(u) = stack.pop() {
                for &(v, _) in &rows[u] {
                    if cluster[v] == 0.0 {
                        cluster[v] = label;
                        stack.push(v);
                    }
                }
            }
        }
        Ok(vec![
            ("id".into(), self.ids.clone()),
            ("distance".into(), vec![t; n]),
            ((if binary { "degree" } else { "strength" }).into(), strength),
            ("eigen".into(), eigenvector(&rows)),
            ("close".into(), close),
            ("BWC".into(), c.betweenness.to_vec()),
            ("cluster".into(), cluster),
        ])
    }
}

/// The leading eigenvector of a symmetric matrix (rows of (column, value)), scaled so that its largest entry
/// in magnitude is 1 (numpy's eigh: the same to rounding when the leading eigenvalue is simple).
fn eigenvector(rows: &[Vec<(usize, f64)>]) -> Vec<f64> {
    let n = rows.len();
    if rows.iter().all(|r| r.is_empty()) {
        return vec![1.0; n];
    }
    let mut vec = if n <= 2000 {
        let mut a = vec![0.0; n * n];
        for (i, r) in rows.iter().enumerate() {
            for &(j, v) in r {
                a[i * n + j] = v;
            }
        }
        jacobi_top(a, n)
    } else {
        power_top(rows)
    };
    let big =
        vec.iter().copied().enumerate().fold((0, 0.0f64), |b, (k, x)| if x.abs() > b.1 { (k, x.abs()) } else { b }).0;
    let s = vec[big];
    for x in vec.iter_mut() {
        *x /= s;
        if x.abs() < 1e-15 {
            *x = 0.0;
        }
    }
    vec
}

/// The eigenvector of the largest eigenvalue of a dense symmetric matrix (cyclic Jacobi).
fn jacobi_top(mut a: Vec<f64>, n: usize) -> Vec<f64> {
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }
    for _sweep in 0..100 {
        let off: f64 = (0..n)
            .flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| a[i * n + j].powi(2))
            .sum();
        if off < 1e-30 {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = a[p * n + q];
                if apq.abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let (akp, akq) = (a[k * n + p], a[k * n + q]);
                    a[k * n + p] = c * akp - s * akq;
                    a[k * n + q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let (apk, aqk) = (a[p * n + k], a[q * n + k]);
                    a[p * n + k] = c * apk - s * aqk;
                    a[q * n + k] = s * apk + c * aqk;
                }
                for k in 0..n {
                    let (vkp, vkq) = (v[k * n + p], v[k * n + q]);
                    v[k * n + p] = c * vkp - s * vkq;
                    v[k * n + q] = s * vkp + c * vkq;
                }
            }
        }
    }
    let top = (0..n).max_by(|&x, &y| a[x * n + x].partial_cmp(&a[y * n + y]).unwrap()).unwrap();
    (0..n).map(|k| v[k * n + top]).collect()
}

/// The leading eigenvector of a large sparse symmetric matrix (power iteration, shifted to make the largest
/// eigenvalue the dominant one).
fn power_top(rows: &[Vec<(usize, f64)>]) -> Vec<f64> {
    let n = rows.len();
    let shift = rows.iter().map(|r| r.iter().map(|e| e.1.abs()).sum::<f64>()).fold(0.0, f64::max);
    let mut x = vec![1.0 / (n as f64).sqrt(); n];
    for _ in 0..10_000 {
        let mut y: Vec<f64> =
            (0..n).map(|i| shift * x[i] + rows[i].iter().map(|&(j, v)| v * x[j]).sum::<f64>()).collect();
        let norm = y.iter().map(|v| v * v).sum::<f64>().sqrt();
        y.iter_mut().for_each(|v| *v /= norm);
        let diff = y.iter().zip(&x).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        x = y;
        if diff < 1e-14 {
            break;
        }
    }
    x
}

/// Python's repr of a float.
fn py(v: f64) -> String {
    crate::bio::names::py_float(v)
}

/// A table of rows of named values (the columns in the order they first appear).
fn rows_table(rows: Vec<Vec<(String, f64)>>) -> Table {
    let mut columns: Vec<String> = Vec::new();
    for r in &rows {
        for (k, _) in r {
            if !columns.contains(k) {
                columns.push(k.clone());
            }
        }
    }
    let mut t = Table::new(columns.clone());
    t.rows = rows
        .iter()
        .map(|r| columns.iter().map(|c| r.iter().find(|(k, _)| k == c).map_or(f64::NAN, |e| e.1)).collect())
        .collect();
    t
}

/// Frames one under the other.
fn concat(frames: Vec<Vec<(String, Vec<f64>)>>) -> Table {
    let mut columns: Vec<String> = Vec::new();
    for f in &frames {
        for (k, _) in f {
            if !columns.contains(k) {
                columns.push(k.clone());
            }
        }
    }
    let mut t = Table::new(columns.clone());
    for f in &frames {
        let n = f.first().map_or(0, |c| c.1.len());
        for r in 0..n {
            t.rows.push(columns.iter().map(|c| f.iter().find(|(k, _)| k == c).map_or(f64::NAN, |e| e.1[r])).collect());
        }
    }
    t
}

/// Per row of eca_change, the comparison and its reading.
pub type Readings = Vec<(&'static str, &'static str)>;

/// How a change of ECA reads against the change of habitat (Makurhini's MK_dECA): the comparison and its
/// meaning.
pub fn comparison(deca: f64, da: f64) -> (&'static str, &'static str) {
    if deca < da && da < 0.0 {
        ("dECA < dA < 0", "+ Connectivity loss")
    } else if da < deca && deca < 0.0 {
        ("dA < dECA < 0", "+ Habitat loss")
    } else if deca == da && da < 0.0 {
        ("dECA = dA < 0", "Equal loss")
    } else if deca == da && da == 0.0 {
        ("dECA = dA = 0", "Habitat or connectivity maintained")
    } else {
        ("dECA or dA gain", "Habitat or connectivity gain")
    }
}

/// The change of the equivalent connected area from graph to graph (Makurhini's MK_dECA, landschaft's
/// eca_change): per distance and graph, the habitat area, ECA and their changes (%, the first against the
/// landscape's area), ECA / dA and how they compare.
pub fn eca_change(
    graphs: &[&HabitatGraph],
    metric: &str,
    distances: &[Option<f64>],
    landscape_area: f64,
    probability: Option<f64>,
    pij_min: Option<f64>,
) -> Result<(Table, Readings), String> {
    let metric = metric.to_uppercase();
    if metric != "PC" && metric != "IIC" {
        return Err("the metric is PC or IIC".into());
    }
    if graphs.is_empty() {
        return Err("give at least one graph".into());
    }
    let la = landscape_area;
    let n = graphs.len();
    let areas: Vec<f64> = graphs.iter().map(|g| g.habitat_area.unwrap_or_else(|| ffi::r_sum(&g.attributes))).collect();
    let mut cols: Vec<(String, Vec<f64>)> =
        ["time", "LA", "area", "distance", "ECA", "ECA_LA", "ECA_area", "dA", "dECA", "rECA"]
            .iter()
            .map(|c| (c.to_string(), Vec::new()))
            .collect();
    let mut readings = Vec::new();
    for &d in distances {
        let mut eca = Vec::with_capacity(n);
        for g in graphs {
            let mut capped = (*g).clone();
            capped.landscape_area = Some(la);
            capped.restoration = None;
            let t = capped.overall(Some(std::slice::from_ref(&metric)), &[d], probability, pij_min)?;
            let k = t.columns.iter().position(|c| *c == format!("EC({metric})")).unwrap();
            eca.push(t.rows[0][k]);
        }
        let mut da = vec![0.0; n];
        let mut deca = vec![0.0; n];
        da[0] = (areas[0] - la) / la * 100.0;
        deca[0] = (eca[0] - la) / la * 100.0;
        for k in 1..n {
            da[k] = (areas[k] - areas[k - 1]) / areas[k - 1] * 100.0;
            deca[k] = (eca[k] - eca[k - 1]) / eca[k - 1] * 100.0;
        }
        for k in 0..n {
            let area_pct = areas[k] * 100.0 / la;
            let row = [
                (k + 1) as f64,
                la,
                areas[k],
                d.unwrap_or(f64::NAN),
                eca[k],
                eca[k] * area_pct / areas[k],
                eca[k] * 100.0 / areas[k],
                da[k],
                deca[k],
                deca[k] / da[k],
            ];
            for (c, v) in cols.iter_mut().zip(row) {
                c.1.push(v);
            }
            readings.push(comparison(deca[k], da[k]));
        }
    }
    Ok((table(cols), readings))
}
