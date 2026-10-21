//! Rivers as a network, as hydrographr does on the Hydrography90m: points moved onto the streams, the land upstream
//! of each link or point summarised (the mean of a map, the share of each class: the forest upstream) and the
//! catchment of each point as a polygon. Over the D8 directions of `hydro` (landschaft's), the rest in Rust.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

use crate::hydro::{Dem, cells, down};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Snap));
    r.add(Arc::new(Upstream));
    r.add(Arc::new(Catchment));
}

/// The statistics of a map over the land upstream.
const STATS: [&str; 6] = ["count", "sum", "mean", "min", "max", "std"];

/// The cells of a grid in its coordinates: the extent, the cell sizes and the shape.
struct Cells {
    e: [f64; 4],
    rx: f64,
    ry: f64,
    nx: usize,
    ny: usize,
}

impl Cells {
    fn of(g: &Grid) -> Cells {
        let (e, _) = crate::surface::extent_of(g);
        let (nx, ny) = (g.width, g.height);
        Cells { e, rx: (e[1] - e[0]) / nx as f64, ry: (e[3] - e[2]) / ny as f64, nx, ny }
    }

    fn centre(&self, k: usize) -> Xy {
        [self.e[0] + ((k % self.nx) as f64 + 0.5) * self.rx, self.e[3] - ((k / self.nx) as f64 + 0.5) * self.ry]
    }

    /// The cell a point falls in (none off the grid).
    fn at(&self, p: Xy) -> Option<usize> {
        let (c, r) = (((p[0] - self.e[0]) / self.rx).floor(), ((self.e[3] - p[1]) / self.ry).floor());
        (c >= 0.0 && r >= 0.0 && (c as usize) < self.nx && (r as usize) < self.ny)
            .then(|| r as usize * self.nx + c as usize)
    }

    /// The cell each point moves to on the cells `on`: the nearest within the distance, of equally near ones the
    /// first in raster order (none farther: hydro.stream_links' snapping, searched in a window around the point).
    fn snap(&self, points: &[Option<Xy>], on: &[bool], distance: f64) -> Vec<Option<usize>> {
        let reach = distance * distance * (1.0 + 1e-12);
        let span = |lo: f64, size: f64, n: usize| -> Option<(usize, usize)> {
            let (a, b) = ((lo - distance) / size, (lo + distance) / size);
            if b < 0.0 || a >= n as f64 {
                return None;
            }
            Some((a.floor().max(0.0) as usize, (b.floor() as usize).min(n - 1)))
        };
        points
            .iter()
            .map(|p| {
                let [x, y] = (*p)?;
                let (r0, r1) = span(self.e[3] - y, self.ry, self.ny)?;
                let (c0, c1) = span(x - self.e[0], self.rx, self.nx)?;
                let mut best = (f64::INFINITY, 0);
                for r in r0..=r1 {
                    let row = r * self.nx;
                    for k in (row + c0..=row + c1).filter(|&k| on[k]) {
                        let [cx, cy] = self.centre(k);
                        let d2 = (cx - x).powi(2) + (cy - y).powi(2);
                        if d2 < best.0 {
                            best = (d2, k);
                        }
                    }
                }
                (best.0 <= reach).then_some(best.1)
            })
            .collect()
    }
}

/// The largest move of a point onto the streams: the setting, else 300 map units of a projection or 3 cells.
fn snap_distance(args: &Args, name: &str, g: &Grid) -> Result<f64> {
    if let Some(d) = args.opt_param::<f64>(name)? {
        if d.is_nan() || d < 0.0 {
            return Err(Error::msg(format!("{name} must be 0 or more")));
        }
        return Ok(d);
    }
    let (_, lonlat) = crate::surface::extent_of(g);
    let c = Cells::of(g);
    let projected = crate::prepare::crs_of(g).is_some() && !lonlat;
    Ok(if projected { 300.0 } else { 3.0 * c.rx.max(c.ry) })
}

/// The points of a vector in the grid's coordinate system, one per feature (none for an empty one).
fn points_of(args: &Args, port: &str, g: &Grid) -> Result<(Arc<Vector>, Vec<Option<Xy>>)> {
    let v = crate::vector::on_grid(crate::vector::vector_of(args, port)?, g)?;
    let mut pts = Vec::with_capacity(v.len());
    for (i, geom) in v.geometries.iter().enumerate() {
        pts.push(match geom {
            Geometry::Point(p) => Some(*p),
            Geometry::MultiPoint(ps) if ps.len() <= 1 => ps.first().copied(),
            Geometry::Empty => None,
            _ => return Err(Error::msg(format!("{port}: feature {} is not a point", i + 1))),
        });
    }
    Ok((v, pts))
}

/// The cells of a map that are streams: those with a value other than 0 (a link's number, 1).
fn stream_cells(m: &Map) -> Vec<bool> {
    m.data.iter().map(|v| !v.is_nan() && *v != 0.0).collect()
}

/// A raster on the DEM's grid, or why not.
fn on_dem<'a>(dem: &Map, m: &'a Map, what: &str) -> Result<&'a [f64]> {
    if m.shape() != dem.shape() {
        return Err(Error::msg(format!("the {what} differ in shape from the DEM: raster.match makes them one grid")));
    }
    Ok(&m.data)
}

/// The network a summary runs on: the places summarised as groups of cells (a
/// link's last cells, a point's cell), numbered 1..; every cell labelled with the first group at or below it.
struct Network {
    /// the group of each cell's own (0: none)
    seed: Vec<u32>,
    groups: usize,
    /// the first group at or below each cell (0: none)
    label: Vec<u32>,
    /// the group each group drains to (0: none), by group (index 0 unused)
    below: Vec<u32>,
}

impl Network {
    fn new(down: &[i64], seed: Vec<u32>, groups: usize) -> Result<Network> {
        let label = first_below(down, &seed)?;
        let mut below = vec![0u32; groups + 1];
        for (k, &g) in seed.iter().enumerate() {
            if g > 0 && below[g as usize] == 0 && down[k] >= 0 {
                let b = label[down[k] as usize];
                if b != g {
                    below[g as usize] = b;
                }
            }
        }
        Ok(Network { seed, groups, label, below })
    }

    /// The groups in an order where each comes before the one it drains to.
    fn upward_first(&self) -> Vec<usize> {
        let mut ups = vec![0usize; self.groups + 1];
        for g in 1..=self.groups {
            ups[self.below[g] as usize] += 1;
        }
        let mut order: Vec<usize> = (1..=self.groups).filter(|&g| ups[g] == 0).collect();
        let mut i = 0;
        while i < order.len() {
            let b = self.below[order[i]] as usize;
            if b > 0 {
                ups[b] -= 1;
                if ups[b] == 0 {
                    order.push(b);
                }
            }
            i += 1;
        }
        order
    }

    /// The groups upstream of a group, itself included.
    fn upstream_of(&self, g: usize, above: &[Vec<usize>]) -> Vec<usize> {
        let mut out = vec![g];
        let mut i = 0;
        while i < out.len() {
            out.extend(above[out[i]].iter().copied());
            i += 1;
        }
        out
    }
}

/// Every cell labelled with the first seed at or below it along the directions (0: none), each path walked once.
fn first_below(down: &[i64], seed: &[u32]) -> Result<Vec<u32>> {
    const UNSEEN: u32 = u32::MAX;
    let mut label = vec![UNSEEN; down.len()];
    let mut path = Vec::new();
    for start in 0..down.len() {
        let mut i = start as i64;
        let found = loop {
            if i < 0 {
                break 0;
            }
            let k = i as usize;
            if label[k] != UNSEEN {
                break label[k];
            }
            if seed[k] > 0 {
                label[k] = seed[k];
                break seed[k];
            }
            if path.len() > down.len() {
                return Err(Error::msg("the flow directions go round in a circle"));
            }
            path.push(k);
            i = down[k];
        };
        for k in path.drain(..) {
            label[k] = found;
        }
    }
    Ok(label)
}

/// Values summarised as they come and joined (Chan et al.'s pairwise update of the mean and the squared deviations).
#[derive(Clone, Copy)]
struct Acc {
    n: f64,
    sum: f64,
    mean: f64,
    m2: f64,
    min: f64,
    max: f64,
}

impl Default for Acc {
    fn default() -> Self {
        Acc { n: 0.0, sum: 0.0, mean: 0.0, m2: 0.0, min: f64::INFINITY, max: f64::NEG_INFINITY }
    }
}

impl Acc {
    fn push(&mut self, x: f64) {
        self.n += 1.0;
        self.sum += x;
        let d = x - self.mean;
        self.mean += d / self.n;
        self.m2 += d * (x - self.mean);
        self.min = self.min.min(x);
        self.max = self.max.max(x);
    }

    fn join(&mut self, o: &Acc) {
        if o.n == 0.0 {
            return;
        }
        let n = self.n + o.n;
        let d = o.mean - self.mean;
        self.mean += d * o.n / n;
        self.m2 += o.m2 + d * d * self.n * o.n / n;
        self.n = n;
        self.sum += o.sum;
        self.min = self.min.min(o.min);
        self.max = self.max.max(o.max);
    }

    fn stat(&self, s: &str) -> f64 {
        match s {
            "count" => self.n,
            "sum" => self.sum,
            _ if self.n == 0.0 => f64::NAN,
            "mean" => self.mean,
            "min" => self.min,
            "max" => self.max,
            _ => (self.m2 / self.n).sqrt(),
        }
    }
}

/// The land upstream of every group: its cells (those with an elevation), the values of each layer, the cells of
/// each class.
struct Upland {
    cells: Vec<f64>,
    layers: Vec<Vec<Acc>>,
    classes: Vec<Vec<f64>>,
    classified: Vec<f64>,
}

fn upland(net: &Network, dem: &[f64], layers: &[&[f64]], classes: Option<(&[f64], &[f64])>) -> Upland {
    let n = net.groups + 1;
    let mut u = Upland {
        cells: vec![0.0; n],
        layers: vec![vec![Acc::default(); n]; layers.len()],
        classes: vec![vec![0.0; n]; classes.map_or(0, |c| c.1.len())],
        classified: vec![0.0; n],
    };
    let index: HashMap<u64, usize> =
        classes.map_or_else(HashMap::new, |(_, ids)| ids.iter().enumerate().map(|(i, c)| (c.to_bits(), i)).collect());
    for (k, &g) in net.label.iter().enumerate() {
        if g == 0 || dem[k].is_nan() {
            continue;
        }
        let g = g as usize;
        u.cells[g] += 1.0;
        for (acc, v) in u.layers.iter_mut().zip(layers) {
            if !v[k].is_nan() {
                acc[g].push(v[k]);
            }
        }
        if let Some((c, _)) = classes
            && let Some(&i) = index.get(&c[k].to_bits())
        {
            u.classes[i][g] += 1.0;
            u.classified[g] += 1.0;
        }
    }
    for g in net.upward_first() {
        let b = net.below[g] as usize;
        if b == 0 {
            continue;
        }
        u.cells[b] += u.cells[g];
        u.classified[b] += u.classified[g];
        for acc in &mut u.layers {
            let a = acc[g];
            acc[b].join(&a);
        }
        for c in &mut u.classes {
            c[b] += c[g];
        }
    }
    u
}

/// The statistics asked for (a list, or names joined by commas), of [`STATS`].
fn stats_of(args: &Args) -> Result<Vec<String>> {
    let names: Vec<String> = match args.params.get("stats") {
        None | Some(serde_json::Value::Null) => vec!["mean".into()],
        Some(serde_json::Value::String(s)) => {
            s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
        }
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| Error::msg(format!("the setting \"stats\": {e}")))?,
    };
    if let Some(s) = names.iter().find(|s| !STATS.contains(&s.as_str())) {
        return Err(Error::msg(format!("unknown statistic {s:?} (count, sum, mean, min, max, std)")));
    }
    Ok(names)
}

/// The places on the network: the groups of the links (their last cells) or of the points (their cells), and a
/// row of the table per link or point with its group.
struct Places {
    seed: Vec<u32>,
    groups: usize,
    /// per row: its id (the link's number, the point's 1..), its group (none: a point off the network) and cell
    rows: Vec<(f64, Option<usize>, Option<usize>)>,
}

/// The links of a map of link numbers, a group each, its last cells (those draining out of it) seeding it.
fn links_places(links: &[f64], down: &[i64]) -> Result<Places> {
    let mut ids: BTreeMap<u64, f64> = BTreeMap::new();
    for &v in links {
        if !v.is_nan() && v != 0.0 {
            if v < 0.0 || v.fract() != 0.0 {
                return Err(Error::msg("the links are positive whole numbers (hydro.stream_links' map)"));
            }
            ids.insert(v as u64, v);
        }
    }
    let group: HashMap<u64, usize> = ids.keys().enumerate().map(|(i, &l)| (l, i + 1)).collect();
    let mut seed = vec![0u32; links.len()];
    for (k, &v) in links.iter().enumerate() {
        if v.is_nan() || v == 0.0 {
            continue;
        }
        let next = down[k];
        if next < 0 || links[next as usize] != v {
            seed[k] = group[&(v as u64)] as u32;
        }
    }
    let rows = ids.values().enumerate().map(|(i, &l)| (l, Some(i + 1), None)).collect();
    Ok(Places { seed, groups: ids.len(), rows })
}

/// The points, a group per cell they fall in (moved onto the streams when given; points on one cell share it).
fn points_places(cells: &[Option<usize>], n: usize) -> Places {
    let mut seed = vec![0u32; n];
    let mut groups = 0;
    let rows = cells
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let g = c.map(|k| {
                if seed[k] == 0 {
                    groups += 1;
                    seed[k] = groups as u32;
                }
                seed[k] as usize
            });
            ((i + 1) as f64, g, *c)
        })
        .collect();
    Places { seed, groups, rows }
}

/// The cell of each point: moved onto the streams when given (none when it is farther than the distance), else
/// the cell it falls in.
fn cells_of_points(
    ctx: &mut Ctx,
    pts: &[Option<Xy>],
    g: &Grid,
    streams: Option<&[bool]>,
    distance: f64,
) -> Vec<Option<usize>> {
    let c = Cells::of(g);
    let out = match streams {
        Some(on) => c.snap(pts, on, distance),
        None => pts.iter().map(|p| p.and_then(|p| c.at(p))).collect(),
    };
    let lost: Vec<String> =
        out.iter().enumerate().filter(|(_, k)| k.is_none()).map(|(i, _)| (i + 1).to_string()).collect();
    if !lost.is_empty() {
        let why =
            if streams.is_some() { format!("farther than {distance} from the streams") } else { "off the grid".into() };
        ctx.warn(format!("point(s) {} {why}: left without values", lost.join(", ")));
    }
    out
}

/// The row of the group below a group among the rows (the first row of that group).
fn row_below(places: &Places, net: &Network, g: Option<usize>) -> f64 {
    let Some(g) = g else { return f64::NAN };
    let b = net.below[g] as usize;
    if b == 0 {
        return 0.0;
    }
    places.rows.iter().find(|r| r.1 == Some(b)).map_or(0.0, |r| r.0)
}

struct Snap;

impl Operator for Snap {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "hydro.snap",
            "Points moved onto the stream network, as hydrographr's snap_to_network: gauges, dams, records of fish \
             each to the centre of the nearest stream cell within a distance (of equally near ones the first in \
             raster order), or of the nearest whose flow accumulation is at least a minimum (on the river, not a \
             tributary beside it). A point farther than the distance stays where it was, snapped 0.",
            Access::Global,
        )
        .input(PortSpec::new("points", Type::Vector, "the points"))
        .input(PortSpec::new("streams", Type::Map, "the streams: non-zero on their cells (hydro.streams, the links)"))
        .input(PortSpec::optional("accumulation", Type::Map, "a flow accumulation, with min_accumulation"))
        .output(PortSpec::new(
            "vector",
            Type::Vector,
            "the points moved, their attributes and row, col, stream (the streams' value there), moved (the \
             distance), snapped (1 or 0)",
        ))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional(
            "distance",
            Type::Number,
            "the largest move, map units (300 m; 3 cells without a projection)",
        ))
        .param(PortSpec::optional(
            "min_accumulation",
            Type::Number,
            "only the stream cells of at least this accumulation (in its units)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let r = |ctx: &mut Ctx| -> Result<Outputs> {
            let streams = args.map("streams")?;
            let g = &streams.grid;
            let mut on = stream_cells(streams);
            match (args.opt_map("accumulation")?, args.opt_param::<f64>("min_accumulation")?) {
                (Some(acc), Some(min)) => {
                    let a = on_dem(streams, acc, "accumulation").map_err(|_| {
                        Error::msg(
                            "the accumulation differs in shape from the streams: raster.match makes them one grid",
                        )
                    })?;
                    for (o, a) in on.iter_mut().zip(a) {
                        *o &= *a >= min;
                    }
                }
                (None, Some(_)) => return Err(Error::msg("min_accumulation needs the accumulation")),
                _ => {}
            }
            let (v, pts) = points_of(args, "points", g)?;
            let distance = snap_distance(args, "distance", g)?;
            let c = Cells::of(g);
            let to = c.snap(&pts, &on, distance);
            let lost: Vec<String> =
                to.iter().enumerate().filter(|(_, k)| k.is_none()).map(|(i, _)| (i + 1).to_string()).collect();
            if !lost.is_empty() {
                ctx.warn(format!("point(s) {} farther than {distance} from the streams: not moved", lost.join(", ")));
            }
            let mut out = (*v).clone();
            let (mut row, mut col, mut stream, mut moved, mut snapped) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for (i, k) in to.iter().enumerate() {
                match (k, pts[i]) {
                    (Some(k), Some(p)) => {
                        let q = c.centre(*k);
                        out.geometries[i] = Geometry::Point(q);
                        row.push((k / c.nx) as f64);
                        col.push((k % c.nx) as f64);
                        stream.push(streams.data[*k]);
                        moved.push((q[0] - p[0]).hypot(q[1] - p[1]));
                        snapped.push(1.0);
                    }
                    _ => {
                        for x in [&mut row, &mut col, &mut stream, &mut moved] {
                            x.push(f64::NAN);
                        }
                        snapped.push(0.0);
                    }
                }
            }
            let (out, table) = crate::vector::with_columns(
                &out,
                vec![
                    ("row".into(), row),
                    ("col".into(), col),
                    ("stream".into(), stream),
                    ("moved".into(), moved),
                    ("snapped".into(), snapped),
                ],
            );
            Ok(outputs([("vector", Value::Vector(Arc::new(out))), ("table", Value::Table(table))]))
        };
        r(ctx).map_err(fail)
    }
}

fn dem_and_flats(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::new("dem", Type::Map, "the elevations (no value: missing), best with depressions filled"))
        .param(PortSpec::optional(
            "flats",
            Type::Bool,
            "drain flat areas towards lower terrain and away from higher terrain (Barnes et al. 2014; default true)",
        ))
}

fn snap_inputs(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::optional(
        "streams",
        Type::Map,
        "the streams the points are moved onto (non-zero on their cells); without them, the cell each falls in",
    ))
    .param(PortSpec::optional(
        "snap",
        Type::Number,
        "the largest move of a point onto the streams, map units (300 m; 3 cells without a projection)",
    ))
}

/// The DEM's network: the cell each cell drains to, and the cells of the DEM with no value.
fn drainage(args: &Args, dem: &Map) -> Result<Vec<i64>> {
    let d = Dem::of(dem);
    let dir = d.d8(args.param("flats", true)?)?;
    Ok(down(&dir, i64::from(d.ny), i64::from(d.nx)))
}

/// The places of the points of a node: moved onto its streams (when given).
fn point_places(ctx: &mut Ctx, args: &Args, dem: &Map) -> Result<(Arc<Vector>, Places)> {
    let (v, pts) = points_of(args, "points", &dem.grid)?;
    let streams = args.opt_map("streams")?.map(|s| on_dem(dem, s, "streams").map(|_| stream_cells(s))).transpose()?;
    let distance = snap_distance(args, "snap", &dem.grid)?;
    let cells = cells_of_points(ctx, &pts, &dem.grid, streams.as_deref(), distance);
    Ok((v, points_places(&cells, dem.data.len())))
}

struct Upstream;

impl Operator for Upstream {
    fn signature(&self) -> OpSignature {
        snap_inputs(dem_and_flats(OpSignature::new(
            "hydro.upstream",
            "The land upstream of every link of a stream network, or of every point on it, summarised, as \
             hydrographr's get_upstream_variable: the statistics of maps over all the cells that drain through it \
             (the mean slope, the rain summed) and the share of each class of a map (the forest, the cities \
             upstream), from each link's last cell or the point's along the D8 directions; its cells and area. \
             The cells without a value of a map left out of its statistics.",
            Access::Global,
        )))
        .input(PortSpec::optional("links", Type::Map, "the links of hydro.stream_links: a row per link"))
        .input(PortSpec::optional("points", Type::Vector, "or points (gauges, dams, sites): a row per point"))
        .input(PortSpec::optional("values", Type::Any, "a map or a stack of maps whose statistics are given"))
        .input(PortSpec::optional("classes", Type::Map, "a map of classes whose shares are given (share_<class>)"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per link (link, downstream) or point (point, downstream, row, col), then cells, area (map units \
             squared; m² for longitude / latitude), the statistics (<layer>_<statistic> of a stack) and the shares",
        ))
        .output(PortSpec::new(
            "map",
            Type::Map,
            "the column to_map painted on each link's cells, or on each point's cell",
        ))
        .param(PortSpec::optional("stats", Type::Any, "of the values: count, sum, mean, min, max, std (default mean)"))
        .param(PortSpec::optional("to_map", Type::Text, "the column of the map (the first after the area)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let r = |ctx: &mut Ctx| -> Result<Outputs> {
            let dem = args.map("dem")?;
            let down = drainage(args, dem)?;
            let links = args.opt_map("links")?;
            let (places, by_link) = match (links, args.get("points").is_some()) {
                (Some(l), false) => (links_places(on_dem(dem, l, "links")?, &down)?, true),
                (None, true) => (point_places(ctx, args, dem)?.1, false),
                _ => return Err(Error::msg("give the links or the points (one of them)")),
            };
            let values = match args.get("values") {
                Some(_) => Some(crate::raster_stats::layers(args, "values")?),
                None => None,
            };
            let mut layers: Vec<&[f64]> = Vec::new();
            if let Some(v) = &values {
                if (v.grid.height, v.grid.width) != dem.shape() {
                    return Err(Error::msg(
                        "the values differ in shape from the DEM: raster.match makes them one grid",
                    ));
                }
                layers = v.data.iter().map(|d| d.as_slice()).collect();
            }
            let classes = args.opt_map("classes")?.map(|c| on_dem(dem, c, "classes").map(|d| (d, c.classes())));
            let classes = classes.transpose()?;
            let stats = stats_of(args)?;
            let net = Network::new(&down, places.seed.clone(), places.groups)?;
            let u = upland(&net, &dem.data, &layers, classes.as_ref().map(|(d, ids)| (*d, ids.as_slice())));
            let (dx, dy) = cells(&dem.grid);
            let nx = dem.grid.width;
            let mut columns: Vec<String> = if by_link {
                vec!["link".into(), "downstream".into()]
            } else {
                vec!["point".into(), "downstream".into(), "row".into(), "col".into()]
            };
            columns.extend(["cells".into(), "area".into()]);
            let first = columns.len();
            let names = values.as_ref().map(|v| v.names.clone()).unwrap_or_default();
            let multi = names.len() > 1;
            for name in &names {
                for s in &stats {
                    columns.push(if multi { format!("{name}_{s}") } else { s.clone() });
                }
            }
            if let Some((_, ids)) = &classes {
                columns.extend(ids.iter().map(|c| format!("share_{c}")));
            }
            let mut table = Table::new(columns);
            for &(id, g, cell) in &places.rows {
                let mut row = vec![id, row_below(&places, &net, g)];
                if !by_link {
                    row.push(cell.map_or(f64::NAN, |k| (k / nx) as f64));
                    row.push(cell.map_or(f64::NAN, |k| (k % nx) as f64));
                }
                let Some(g) = g else {
                    row.resize(table.columns.len(), f64::NAN);
                    table.rows.push(row);
                    continue;
                };
                row.extend([u.cells[g], u.cells[g] * dx * dy]);
                for acc in &u.layers {
                    row.extend(stats.iter().map(|s| acc[g].stat(s)));
                }
                for c in &u.classes {
                    row.push(if u.classified[g] > 0.0 { c[g] / u.classified[g] } else { f64::NAN });
                }
                table.rows.push(row);
            }
            let to_map: String = args.param("to_map", table.columns.get(first).cloned().unwrap_or("cells".into()))?;
            let j =
                table.columns.iter().position(|c| *c == to_map).ok_or_else(|| {
                    Error::msg(format!("to_map: no column {to_map:?} (of {})", table.columns.join(", ")))
                })?;
            // the value of each group (of its first row), painted on the links' cells or the points'
            let mut of_group = vec![f64::NAN; places.groups + 1];
            for (r, &(_, g, _)) in table.rows.iter().zip(&places.rows) {
                if let Some(g) = g
                    && of_group[g].is_nan()
                {
                    of_group[g] = r[j];
                }
            }
            let map: Vec<f64> = match links {
                Some(l) if by_link => l
                    .data
                    .iter()
                    .zip(&net.label)
                    .map(|(v, &g)| if v.is_nan() || *v == 0.0 { f64::NAN } else { of_group[g as usize] })
                    .collect(),
                _ => net.seed.iter().map(|&g| if g == 0 { f64::NAN } else { of_group[g as usize] }).collect(),
            };
            Ok(outputs([("table", Value::Table(table)), ("map", Value::Map(Map::new(dem.grid.clone(), map)))]))
        };
        r(ctx).map_err(fail)
    }
}

struct Catchment;

impl Operator for Catchment {
    fn signature(&self) -> OpSignature {
        snap_inputs(dem_and_flats(OpSignature::new(
            "hydro.upstream_catchment",
            "The catchment of every point (a gauge, a dam, a site), as hydrographr's get_upstream_catchment: all \
             the land that drains to it along the D8 directions, as a polygon per point (with the point's \
             attributes, its cells and area; the catchments of points one above another overlap), and a map of \
             each cell's first point downstream (the catchments nested, between one point and the next).",
            Access::Global,
        )))
        .input(PortSpec::new("points", Type::Vector, "the points (moved onto the streams when they are given)"))
        .output(PortSpec::new(
            "vector",
            Type::Vector,
            "a polygon per point: its attributes and point (1..), downstream (the point below, 0 none), cells, area",
        ))
        .output(PortSpec::new("table", Type::Table, "point, downstream, row, col, cells, area"))
        .output(PortSpec::new(
            "map",
            Type::Map,
            "the first point each cell drains to (1..; 0 none): the catchments between one point and the next",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let r = |ctx: &mut Ctx| -> Result<Outputs> {
            let dem = args.map("dem")?;
            let down = drainage(args, dem)?;
            let (v, places) = point_places(ctx, args, dem)?;
            let net = Network::new(&down, places.seed.clone(), places.groups)?;
            let u = upland(&net, &dem.data, &[], None);
            let g = &dem.grid;
            let nx = g.width;
            let (dx, dy) = cells(g);
            // each group's own cells: their bounds
            let mut bounds = vec![[usize::MAX, usize::MAX, 0, 0]; places.groups + 1];
            for (k, &l) in net.label.iter().enumerate() {
                if l > 0 && !dem.data[k].is_nan() {
                    let b = &mut bounds[l as usize];
                    let (r, c) = (k / nx, k % nx);
                    *b = [b[0].min(r), b[1].min(c), b[2].max(r), b[3].max(c)];
                }
            }
            let mut above = vec![Vec::new(); places.groups + 1];
            for gr in 1..=places.groups {
                above[net.below[gr] as usize].push(gr);
            }
            let t = g.transform;
            let at = |p: Xy| [t[0] + p[0] * t[1] + p[1] * t[2], t[3] + p[0] * t[4] + p[1] * t[5]];
            let mut inside = vec![false; places.groups + 1];
            let mut geoms = Vec::with_capacity(places.rows.len());
            let mut table =
                Table::new(["point", "downstream", "row", "col", "cells", "area"].map(String::from).to_vec());
            for &(id, gr, cell) in &places.rows {
                let below = row_below(&places, &net, gr);
                let (row, col) = cell.map_or((f64::NAN, f64::NAN), |k| ((k / nx) as f64, (k % nx) as f64));
                let Some(gr) = gr else {
                    geoms.push(Geometry::Empty);
                    table.rows.push(vec![id, below, row, col, f64::NAN, f64::NAN]);
                    continue;
                };
                let set = net.upstream_of(gr, &above);
                let mut b = [usize::MAX, usize::MAX, 0, 0];
                for &s in &set {
                    inside[s] = true;
                    let o = bounds[s];
                    if o[0] != usize::MAX {
                        b = [b[0].min(o[0]), b[1].min(o[1]), b[2].max(o[2]), b[3].max(o[3])];
                    }
                }
                let mut polys: Vec<Vec<Vec<Xy>>> = Vec::new();
                if b[0] != usize::MAX {
                    let (w, h) = (b[3] - b[1] + 1, b[2] - b[0] + 1);
                    let mut mask = vec![f64::NAN; w * h];
                    for r in 0..h {
                        for c in 0..w {
                            let k = (b[0] + r) * nx + b[1] + c;
                            if inside[net.label[k] as usize] && net.label[k] > 0 && !dem.data[k].is_nan() {
                                mask[r * w + c] = 1.0;
                            }
                        }
                    }
                    for (_, _, ps) in crate::vector_tools::polygons_of(&mask, w, h, false) {
                        for p in ps {
                            polys.push(
                                p.into_iter()
                                    .map(|ring| {
                                        let mut ring: Vec<Xy> = ring
                                            .into_iter()
                                            .map(|q| at([q[0] + b[1] as f64, q[1] + b[0] as f64]))
                                            .collect();
                                        ring.push(ring[0]);
                                        ring
                                    })
                                    .collect(),
                            );
                        }
                    }
                }
                for &s in &set {
                    inside[s] = false;
                }
                geoms.push(match polys.len() {
                    0 => Geometry::Empty,
                    1 => Geometry::Polygon(polys.remove(0)),
                    _ => Geometry::MultiPolygon(polys),
                });
                table.rows.push(vec![id, below, row, col, u.cells[gr], u.cells[gr] * dx * dy]);
            }
            let mut out =
                Vector { geometries: geoms, attributes: v.attributes.clone(), crs: crate::prepare::crs_of(g) };
            let column = |j: usize| table.rows.iter().map(|r| r[j]).collect::<Vec<f64>>();
            for (name, j) in [("point", 0), ("downstream", 1), ("cells", 4), ("area", 5)] {
                out.attributes.retain(|(n, _)| n != name);
                out.attributes.push((name.to_string(), Column::Numbers(column(j))));
            }
            // the first point of each group, on the cells draining to it
            let mut first = vec![0.0; places.groups + 1];
            for &(id, gr, _) in places.rows.iter().rev() {
                if let Some(gr) = gr {
                    first[gr] = id;
                }
            }
            let map: Vec<f64> = net
                .label
                .iter()
                .zip(dem.data.iter())
                .map(|(&l, z)| if z.is_nan() { f64::NAN } else { first[l as usize] })
                .collect();
            Ok(outputs([
                ("vector", Value::Vector(Arc::new(out))),
                ("table", Value::Table(table)),
                ("map", Value::Map(Map::new(g.clone(), map))),
            ]))
        };
        r(ctx).map_err(fail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_seed_below_labels_every_cell() {
        // a line of five cells draining right, seeds on the 2nd and the 5th
        let down = vec![1, 2, 3, 4, -1];
        let seed = vec![0, 1, 0, 0, 2];
        assert_eq!(first_below(&down, &seed).unwrap(), [1, 1, 2, 2, 2]);
        assert!(first_below(&[1, 0], &[0, 0]).is_err(), "a circle");
    }

    #[test]
    fn values_joined_are_the_values_together() {
        let (mut a, mut b, mut all) = (Acc::default(), Acc::default(), Acc::default());
        for x in [1.0, 4.0, 2.0] {
            a.push(x);
            all.push(x);
        }
        for x in [8.0, 5.0] {
            b.push(x);
            all.push(x);
        }
        a.join(&b);
        for s in STATS {
            assert!((a.stat(s) - all.stat(s)).abs() < 1e-12, "{s}");
        }
        assert_eq!(a.stat("mean"), 4.0);
        assert!((a.stat("std") - 2.4494897427831783).abs() < 1e-12);
        assert!(Acc::default().stat("mean").is_nan() && Acc::default().stat("sum") == 0.0);
    }
}
