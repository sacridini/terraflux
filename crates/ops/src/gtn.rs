//! GuidosToolbox's network analysis of an MSPA map (GTN), as landschaft's gtn module: the components of the
//! network of cores (nodes) and bridges (links), its equivalent connected area and coherence
//! (`networks.gtn_components`); the importance of every link and stepping-stone node, the loss of connectivity
//! (dPC) without it (`networks.gtn_importance`); the network as Conefor Sensinode's inputs (`networks.conefor`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;

use crate::landscape::map_of;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Components));
    r.add(Arc::new(Importance));
    r.add(Arc::new(Conefor));
}

/// MSPA's core and bridge classes, and its missing.
const CORE: [u8; 2] = [17, 117];
const BRIDGE: [u8; 6] = [33, 35, 37, 133, 135, 137];
const MISSING: u8 = 129;
/// GTB's colours of the networks (by component), of isolated cores, of missing.
const NW_COLORS: [u8; 9] = [1, 5, 9, 17, 33, 65, 80, 150, 176];

/// An MSPA map's classes as bytes.
fn mspa<'a>(args: &'a Args) -> Result<(&'a Map, Vec<u8>)> {
    let m = args.map("map")?;
    let v = m
        .data
        .iter()
        .map(|&v| match v {
            v if v.is_nan() => Ok(MISSING),
            v if v.fract() == 0.0 && (0.0..=255.0).contains(&v) => Ok(v as u8),
            v => Err(Error::msg(format!("node {:?}: an MSPA map holds its classes, not {v}", args.node))),
        })
        .collect::<Result<_>>()?;
    Ok((m, v))
}

/// The 8-connected objects of a mask, numbered in row order (scipy's label): their count and labels.
fn label(m: &[bool], (ny, nx): (usize, usize)) -> (usize, Vec<usize>) {
    let mut lab = vec![0usize; ny * nx];
    let mut n = 0;
    for s in 0..ny * nx {
        if !m[s] || lab[s] != 0 {
            continue;
        }
        n += 1;
        lab[s] = n;
        let mut stack = vec![s];
        while let Some(c) = stack.pop() {
            let (y, x) = ((c / nx) as i64, (c % nx) as i64);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (yy, xx) = (y + dy, x + dx);
                    if yy >= 0 && xx >= 0 && yy < ny as i64 && xx < nx as i64 {
                        let k = yy as usize * nx + xx as usize;
                        if m[k] && lab[k] == 0 {
                            lab[k] = n;
                            stack.push(k);
                        }
                    }
                }
            }
        }
    }
    (n, lab)
}

/// The network: its cores and bridges, the labels of its components, nodes and links.
struct Network {
    shape: (usize, usize),
    core: Vec<bool>,
    bridge: Vec<bool>,
    comp: Vec<usize>,
    nr_comp: usize,
    nodes: Vec<usize>,
    nr_nodes: usize,
    links: Vec<usize>,
    nr_links: usize,
}

impl Network {
    fn of(v: &[u8], shape: (usize, usize)) -> Network {
        let core: Vec<bool> = v.iter().map(|c| CORE.contains(c)).collect();
        let bridge: Vec<bool> = v.iter().map(|c| BRIDGE.contains(c)).collect();
        let both: Vec<bool> = core.iter().zip(&bridge).map(|(a, b)| *a || *b).collect();
        let (nr_comp, comp) = label(&both, shape);
        let (nr_nodes, nodes) = label(&core, shape);
        let (nr_links, links) = label(&bridge, shape);
        Network { shape, core, bridge, comp, nr_comp, nodes, nr_nodes, links, nr_links }
    }

    /// Every component's core and bridge cells (index 0: none).
    fn comp_counts(&self) -> (Vec<u64>, Vec<u64>) {
        let (mut c, mut b) = (vec![0u64; self.nr_comp + 1], vec![0u64; self.nr_comp + 1]);
        for k in 0..self.comp.len() {
            if self.core[k] {
                c[self.comp[k]] += 1;
            }
            if self.bridge[k] {
                b[self.comp[k]] += 1;
            }
        }
        (c, b)
    }

    /// The nodes every link touches (8 neighbours), as landschaft's extract_node_link_contacts.
    fn contacts(&self) -> BTreeMap<usize, BTreeSet<usize>> {
        let (ny, nx) = self.shape;
        let mut out: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        for c in 0..ny * nx {
            let l = self.links[c];
            if l == 0 {
                continue;
            }
            let (y, x) = ((c / nx) as i64, (c % nx) as i64);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (yy, xx) = (y + dy, x + dx);
                    if yy >= 0 && xx >= 0 && yy < ny as i64 && xx < nx as i64 {
                        let n = self.nodes[yy as usize * nx + xx as usize];
                        if n > 0 {
                            out.entry(l).or_default().insert(n);
                        }
                    }
                }
            }
        }
        out
    }
}

/// A pixel's area in hectares: the setting `pixel_size` (metres, or [width, height]), else the grid's when its cells
/// are 0.1 or more (projected metres).
fn pixel_area_ha(args: &Args, grid: &Grid) -> Result<Option<f64>> {
    match args.params.get("pixel_size") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Number(n)) => {
            let s = n.as_f64().unwrap();
            return Ok(Some((s * s).abs() / 10000.0));
        }
        Some(v) => {
            let [w, h]: [f64; 2] = serde_json::from_value(v.clone())
                .map_err(|e| Error::msg(format!("node {:?}: pixel_size: {e}", args.node)))?;
            return Ok(Some((w * h).abs() / 10000.0));
        }
    }
    let (dx, dy) = (grid.transform[1].abs(), grid.transform[5].abs());
    Ok((grid.geo.is_geographic() == Some(false) && dx >= 0.1 && dy >= 0.1).then(|| dx * dy / 10000.0))
}

struct Components;

impl Operator for Components {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "networks.gtn_components",
            "The components of the habitat network of an MSPA map, as GuidosToolbox's nw_components \
             (landschaft's gtn_components): cores (nodes) and bridges (links) grouped into 8-connected components, \
             only the cores habitat; the equivalent connected area ECA = sqrt(sum of the components' core areas \
             squared), ECA_max (all one component) and the coherence ECA / ECA_max.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the MSPA map (landscape.mspa)"))
        .output(PortSpec::new("map", Type::Map, "GTB's colours: the networks by component, isolated cores, missing"))
        .output(PortSpec::new("components", Type::Map, "the component of every core and bridge cell (0 elsewhere)"))
        .output(PortSpec::new(
            "stats",
            Type::Table,
            "per component: its core and bridge cells, all, whether it has a bridge (1), and in hectares",
        ))
        .output(PortSpec::new(
            "summary",
            Type::Table,
            "components, networks, isolated cores, nodes, links, ECA (ha or cells), ECA in cells, ECA_max, \
             ECA_max in cells, coherence (%)",
        ))
        .output(PortSpec::new("report", Type::Text, "GTB's log"))
        .param(PortSpec::optional(
            "pixel_size",
            Type::Any,
            "a cell's size in metres, for hectares (default the grid's)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (m, v) = mspa(args)?;
        let net = Network::of(&v, m.shape());
        let (core_areas, bridges) = net.comp_counts();
        let n = net.nr_comp;
        let has: Vec<bool> = (1..=n).map(|c| bridges[c] > 0).collect();
        let connected = has.iter().filter(|h| **h).count();
        let pcnum: f64 = (1..=n).map(|c| (core_areas[c] as f64).powi(2)).sum();
        let eca_px = pcnum.sqrt();
        let eca_max_px: u64 = core_areas[1..].iter().sum();
        let doc = if eca_max_px > 0 { eca_px / eca_max_px as f64 * 100.0 } else { 0.0 };
        let ha = pixel_area_ha(args, &m.grid)?;
        let (eca, eca_max) = match ha {
            Some(h) => (eca_px * h, eca_max_px as f64 * h),
            None => (eca_px, eca_max_px as f64),
        };
        let mut colored = vec![0u8; v.len()];
        for k in 0..v.len() {
            let c = net.comp[k];
            if c > 0 {
                colored[k] = if has[c - 1] { NW_COLORS[(c - 1) % NW_COLORS.len()] } else { 103 };
            }
            if v[k] == MISSING {
                colored[k] = MISSING;
            }
        }
        let mut cols: Vec<String> = ["component", "core", "bridge", "total", "has_bridge"].map(String::from).to_vec();
        if ha.is_some() {
            cols.extend(["core_ha", "total_ha"].map(String::from));
        }
        let mut stats = Table::new(cols);
        for c in 1..=n {
            let (co, br) = (core_areas[c] as f64, bridges[c] as f64);
            let mut row = vec![c as f64, co, br, co + br, f64::from(u8::from(has[c - 1]))];
            if let Some(h) = ha {
                row.extend([co * h, (co + br) * h]);
            }
            stats.rows.push(row);
        }
        let mut summary = Table::new(
            [
                "components",
                "networks",
                "isolated",
                "nodes",
                "links",
                "eca",
                "eca_cells",
                "eca_max",
                "eca_max_cells",
                "coherence",
            ]
            .map(String::from)
            .to_vec(),
        );
        summary.rows.push(vec![
            n as f64,
            connected as f64,
            (n - connected) as f64,
            net.nr_nodes as f64,
            net.nr_links as f64,
            eca,
            eca_px,
            eca_max,
            eca_max_px as f64,
            doc,
        ]);
        let unit = if ha.is_some() { "ha" } else { "pixels" };
        let report = format!(
            "Network components: {n}\nConnected networks: {connected}\nIsolated core components: {}\nTotal core nodes: \
             {}\nTotal bridge links: {}\nECA: {eca:.2} {unit} ({eca_px:.1} pixels)\nECA_max: {eca_max:.2} {unit} \
             ({eca_max_px} pixels)\nDOC (Degree of Coherence): {doc:.2}%\n",
            n - connected,
            net.nr_nodes,
            net.nr_links,
        );
        Ok(outputs([
            ("map", map_of(m, colored)),
            ("components", Value::Map(Map::new(m.grid.clone(), net.comp.iter().map(|&c| c as f64).collect()))),
            ("stats", Value::Table(stats)),
            ("summary", Value::Table(summary)),
            ("report", Value::Text(report)),
        ]))
    }
}

/// Connected components of nodes `0..n` joined by the pairs of every link's nodes: a component per node.
fn components(n: usize, pairs: &[(usize, usize)]) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for &(a, b) in pairs {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent[ra] = rb;
        }
    }
    (0..n).map(|x| find(&mut parent, x)).collect()
}

/// The sum of the squared areas of the components (integers: exact).
fn pc(areas: &[u64], comp: &[usize]) -> f64 {
    let mut by: BTreeMap<usize, u64> = BTreeMap::new();
    for (a, c) in areas.iter().zip(comp) {
        *by.entry(*c).or_default() += a;
    }
    by.values().map(|&a| (a as f64).powi(2)).sum()
}

struct LinkRow {
    id: usize,
    comp: usize,
    pixels: u64,
    dpc: f64,
    nodes: usize,
}

struct NodeRow {
    id: usize,
    comp: usize,
    pixels: u64,
    dpc: f64,
    degree: usize,
}

struct Importance;

impl Operator for Importance {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "networks.gtn_importance",
            "The importance of every link (bridge) and stepping-stone node (core with two links or more) of the \
             habitat network of an MSPA map, as GuidosToolbox's nw_importance (landschaft's gtn_importance): the \
             loss of connectivity (dPC, the drop of PC's numerator, %) without it; links positive and nodes \
             negative on the maps, the top 1, 5 and 10 % in GTB's colours.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the MSPA map (landscape.mspa)"))
        .output(PortSpec::new("map", Type::Map, "GTB's colours: cores, bridges, the top 1, 5, 10 % of links and nodes"))
        .output(PortSpec::new("importance", Type::Map, "dPC of every link (positive) and node (negative) on its cells"))
        .output(PortSpec::new("ids", Type::Map, "the link (positive) or node (negative) of every cell"))
        .output(PortSpec::new(
            "links",
            Type::Table,
            "per link: its component, cells, dPC, the nodes it joins, its importance normalized to the largest (%)",
        ))
        .output(PortSpec::new(
            "nodes",
            Type::Table,
            "per stepping-stone node: its component, cells, dPC, links (degree), normalized importance (%)",
        ))
        .output(PortSpec::new("report", Type::Text, "GTB's report (_cs.txt)"))
        .param(PortSpec::optional("title", Type::Text, "the report's first line"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (m, v) = mspa(args)?;
        let net = Network::of(&v, m.shape());
        let cells = v.len();
        let (comp_core, _) = net.comp_counts();
        let pcnum: f64 = comp_core[1..].iter().map(|&a| (a as f64).powi(2)).sum();
        let mut node_area = vec![0u64; net.nr_nodes + 1];
        let mut link_area = vec![0u64; net.nr_links + 1];
        for k in 0..cells {
            node_area[net.nodes[k]] += 1;
            link_area[net.links[k]] += 1;
        }
        let mut importance = vec![0f32; cells];
        let mut ids = vec![0i64; cells];
        let mut colored = vec![0u8; cells];
        let (mut link_rows, mut node_rows) = (Vec::new(), Vec::new());
        let has_any = net.nr_nodes > 0 && net.nr_links > 0 && pcnum > 0.0;
        if has_any {
            for k in 0..cells {
                if net.core[k] {
                    colored[k] = 176;
                }
                if net.bridge[k] {
                    colored[k] = 103;
                }
                if v[k] == MISSING {
                    colored[k] = MISSING;
                }
                if net.links[k] > 0 {
                    ids[k] = net.links[k] as i64;
                }
                if net.nodes[k] > 0 {
                    ids[k] = -(net.nodes[k] as i64);
                }
            }
            let contacts = net.contacts();
            let empty = BTreeSet::new();
            let touches = |l: usize| contacts.get(&l).unwrap_or(&empty);
            // every component's links and nodes
            let (mut comp_links, mut comp_nodes) =
                (vec![BTreeSet::new(); net.nr_comp + 1], vec![BTreeSet::new(); net.nr_comp + 1]);
            for k in 0..cells {
                if net.bridge[k] {
                    comp_links[net.comp[k]].insert(net.links[k]);
                }
                if net.core[k] {
                    comp_nodes[net.comp[k]].insert(net.nodes[k]);
                }
            }
            let mut link_dpc = vec![f64::NAN; net.nr_links + 1];
            let mut node_dpc = vec![f64::NAN; net.nr_nodes + 1];
            for idc in 1..=net.nr_comp {
                let lids: Vec<usize> = comp_links[idc].iter().copied().collect();
                if lids.is_empty() {
                    continue;
                }
                let nids: Vec<usize> = comp_nodes[idc].iter().copied().collect();
                let at: BTreeMap<usize, usize> = nids.iter().enumerate().map(|(i, &n)| (n, i)).collect();
                let hh = pcnum - (comp_core[idc] as f64).powi(2);
                let areas: Vec<u64> = nids.iter().map(|&n| node_area[n]).collect();
                let pairs_of = |skip_link: Option<usize>, skip_node: Option<usize>, at: &BTreeMap<usize, usize>| {
                    let mut pairs = Vec::new();
                    for &l in &lids {
                        if Some(l) == skip_link {
                            continue;
                        }
                        let nds: Vec<usize> = touches(l).iter().copied().filter(|&n| Some(n) != skip_node).collect();
                        for i in 0..nds.len() {
                            for j in i + 1..nds.len() {
                                pairs.push((at[&nds[i]], at[&nds[j]]));
                            }
                        }
                    }
                    pairs
                };
                // the links
                for &l in &lids {
                    let comp = components(nids.len(), &pairs_of(Some(l), None, &at));
                    let pcnew = pc(&areas, &comp) + hh;
                    let dpc = (pcnum - pcnew) / pcnum * 100.0;
                    link_dpc[l] = dpc;
                    link_rows.push(LinkRow { id: l, comp: idc, pixels: link_area[l], dpc, nodes: touches(l).len() });
                }
                // the stepping-stone nodes (two links or more)
                let mut adj: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
                for &l in &lids {
                    let nds: Vec<usize> = touches(l).iter().copied().collect();
                    for &a in &nds {
                        for &b in &nds {
                            if a != b {
                                adj.entry(a).or_default().insert(b);
                            }
                        }
                    }
                }
                for &n in nids.iter().filter(|n| adj.get(n).is_some_and(|s| s.len() >= 2)) {
                    let rem: Vec<usize> = nids.iter().copied().filter(|&x| x != n).collect();
                    let at_rem: BTreeMap<usize, usize> = rem.iter().enumerate().map(|(i, &x)| (x, i)).collect();
                    let rem_areas: Vec<u64> = rem.iter().map(|&x| node_area[x]).collect();
                    let comp = components(rem.len(), &pairs_of(None, Some(n), &at_rem));
                    let pcnew1 = pc(&rem_areas, &comp);
                    let pcnew2 = (rem_areas.iter().sum::<u64>() as f64).powi(2);
                    let dpc = (pcnew2 - pcnew1) / pcnum * 100.0;
                    node_dpc[n] = dpc;
                    node_rows.push(NodeRow { id: n, comp: idc, pixels: node_area[n], dpc, degree: adj[&n].len() });
                }
            }
            for k in 0..cells {
                if net.links[k] > 0 && link_dpc[net.links[k]].is_finite() {
                    importance[k] = link_dpc[net.links[k]] as f32;
                }
                if net.nodes[k] > 0 && node_dpc[net.nodes[k]].is_finite() {
                    importance[k] = -node_dpc[net.nodes[k]] as f32;
                }
            }
        }
        let max_link = link_rows.iter().map(|r| r.dpc).fold(f64::NAN, f64::max);
        let max_node = node_rows.iter().map(|r| r.dpc).fold(f64::NAN, f64::max);
        let (max_link, max_node) =
            (if max_link.is_nan() { 0.0 } else { max_link }, if max_node.is_nan() { 0.0 } else { max_node });
        // the top 1, 5 and 10 % (the thresholds in single precision, as numpy compares them)
        if max_link > 0.0 {
            for (share, code) in [(0.90, 195u8), (0.95, 194), (0.99, 192)] {
                let t = (max_link * share) as f32;
                for k in 0..cells {
                    if net.bridge[k] && importance[k] >= t {
                        colored[k] = code;
                    }
                }
            }
        }
        if max_node > 0.0 {
            for (share, code) in [(0.90, 211u8), (0.95, 210), (0.99, 208)] {
                let t = (-max_node * share) as f32;
                for k in 0..cells {
                    if net.core[k] && importance[k] <= t {
                        colored[k] = code;
                    }
                }
            }
        }
        let norm = |d: f64, max: f64| if max > 0.0 { d / max * 100.0 } else { 0.0 };
        let mut links = Table::new(
            ["link", "component", "pixels", "dpc", "nodes", "normalized_importance"].map(String::from).to_vec(),
        );
        links.rows = link_rows
            .iter()
            .map(|r| vec![r.id as f64, r.comp as f64, r.pixels as f64, r.dpc, r.nodes as f64, norm(r.dpc, max_link)])
            .collect();
        let mut nodes = Table::new(
            ["node", "component", "pixels", "dpc", "degree", "normalized_importance"].map(String::from).to_vec(),
        );
        nodes.rows = node_rows
            .iter()
            .map(|r| vec![r.id as f64, r.comp as f64, r.pixels as f64, r.dpc, r.degree as f64, norm(r.dpc, max_node)])
            .collect();
        // GTB's report
        let title: String = args.param("title", "landschaft Connectivity Importance".to_string())?;
        let mut rep = format!(
            "{title}: Connectivity Importance(Node_max={max_node:.2}, Link_max={max_link:.2})\n(NOTE: node ID & \
             importance in the actual image data are\nNEGATIVE to distinguish them from link ID & importance)\n{}\n\
             Total number of components/links/nodes: {} / {} / {}\n{}\n",
            "=".repeat(68),
            net.nr_comp,
            net.nr_links,
            net.nr_nodes,
            "=".repeat(68)
        );
        let comps: BTreeSet<usize> = link_rows.iter().map(|r| r.comp).collect();
        for c in comps {
            let cl: Vec<&LinkRow> = link_rows.iter().filter(|r| r.comp == c).collect();
            let cn: Vec<&NodeRow> = node_rows.iter().filter(|r| r.comp == c).collect();
            rep += &format!(
                "                   Component: {c}\n{}\n# of links/nodes: {} / {}\n",
                "=".repeat(64),
                cl.len(),
                cn.len()
            );
            if !cl.is_empty() {
                rep += &format!(
                    "{}\n     Link ID        pixels          dPC      normalized.importance\n{}\n",
                    "=".repeat(64),
                    "-".repeat(64)
                );
                for r in &cl {
                    rep += &format!(
                        "{:>12}  {:>12}    {:>10.6}      {:>10.4}\n",
                        r.id,
                        r.pixels,
                        r.dpc,
                        norm(r.dpc, max_link)
                    );
                }
            }
            if !cn.is_empty() {
                rep += &format!(
                    "{}\n     Node ID        pixels          dPC      normalized.importance\n{}\n",
                    "=".repeat(64),
                    "-".repeat(64)
                );
                for r in &cn {
                    rep += &format!(
                        "{:>12}  {:>12}    {:>10.6}      {:>10.4}\n",
                        -(r.id as i64),
                        r.pixels,
                        r.dpc,
                        norm(r.dpc, max_node)
                    );
                }
            }
            rep += &format!("{}\n", "=".repeat(64));
        }
        Ok(outputs([
            ("map", map_of(m, colored)),
            ("importance", Value::Map(Map::new(m.grid.clone(), importance.iter().map(|&x| f64::from(x)).collect()))),
            ("ids", Value::Map(Map::new(m.grid.clone(), ids.iter().map(|&x| x as f64).collect()))),
            ("links", Value::Table(links)),
            ("nodes", Value::Table(nodes)),
            ("report", Value::Text(rep)),
        ]))
    }
}

struct Conefor;

impl Operator for Conefor {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "networks.conefor",
            "The habitat network of an MSPA map as Conefor Sensinode's inputs, as GuidosToolbox's nw_cs22 \
             (landschaft's conefor_inputs): the cores as nodes (negative ids, their area) and the bridges (positive \
             ids, area 0), a link from every bridge to every core it touches at distance 1.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the MSPA map (landscape.mspa)"))
        .output(PortSpec::new("nodes", Type::Table, "id and pixels per node"))
        .output(PortSpec::new("links", Type::Table, "link, node and distance per link"))
        .output(PortSpec::new("nodes_txt", Type::Text, "Conefor's nodes file"))
        .output(PortSpec::new("links_txt", Type::Text, "Conefor's links file"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (m, v) = mspa(args)?;
        let net = Network::of(&v, m.shape());
        let mut area = vec![0u64; net.nr_nodes + 1];
        net.nodes.iter().for_each(|&n| area[n] += 1);
        let contacts = net.contacts();
        let mut nodes = Table::new(["id", "pixels"].map(String::from).to_vec());
        nodes.rows = (1..=net.nr_nodes)
            .map(|i| vec![-(i as f64), area[i] as f64])
            .chain((1..=net.nr_links).map(|j| vec![j as f64, 0.0]))
            .collect();
        let mut links = Table::new(["link", "node", "distance"].map(String::from).to_vec());
        for b in 1..=net.nr_links {
            for &c in contacts.get(&b).into_iter().flatten() {
                links.rows.push(vec![b as f64, -(c as f64), 1.0]);
            }
        }
        let nodes_txt: String =
            nodes.rows.iter().map(|r| format!("{:>10} {:>12}\n", r[0] as i64, r[1] as i64)).collect();
        let links_txt: String =
            links.rows.iter().map(|r| format!("{:>8} {:>8} {:>6}\n", r[0] as i64, r[1] as i64, r[2] as i64)).collect();
        Ok(outputs([
            ("nodes", Value::Table(nodes)),
            ("links", Value::Table(links)),
            ("nodes_txt", Value::Text(nodes_txt)),
            ("links_txt", Value::Text(links_txt)),
        ]))
    }
}
