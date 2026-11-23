//! Linear referencing, shortest lines and closest points (`vector.points_along`, `vector.substring`,
//! `vector.split_lines`, `vector.locate`, `vector.shortest_line`, `vector.closest_point`). The arithmetic of linear
//! referencing is that of the library's `tf.vector` (`_vector_lines.py`), so the coordinates agree bit for bit: a
//! line has vertices p_0..p_n, the segment lengths are `hypot(dx, dy)`, the cumulative lengths c_0 = 0 and
//! c_{k+1} = c_k + L_k summed in vertex order; the point at the measure m lies on the first segment k with
//! c_k < m <= c_{k+1}, at f = (m - c_k) / L_k along it; m <= 0 gives p_0 and m >= c_n gives p_n. A multi-line is
//! taken part by part, the measures restarting on each. The nearest features and points are GEOS's.

use std::sync::Arc;

use terraflux_gdal::geos::{GeosIndex, nearest_points, project};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::vector::{attribute_table, vector_of};
use crate::vector_tools::like;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(PointsAlong));
    r.add(Arc::new(Substring));
    r.add(Arc::new(SplitLines));
    r.add(Arc::new(Locate));
    r.add(Arc::new(ShortestLine));
    r.add(Arc::new(ClosestPoint));
}

fn give(v: Vector) -> Outputs {
    let t = attribute_table(&v);
    outputs([("vector", Value::Vector(Arc::new(v))), ("table", Value::Table(t))])
}

fn err(node: &str) -> impl Fn(String) -> Error + '_ {
    move |m: String| Error::msg(format!("node {node:?}: {m}"))
}

/// `names` renamed by appending `suffix` while they are among `taken` (or the names already given), as the library.
fn rename(names: &[&str], taken: &[String], suffix: &str) -> Vec<String> {
    let mut seen: Vec<String> = taken.to_vec();
    names
        .iter()
        .map(|c| {
            let mut new = c.to_string();
            while seen.contains(&new) {
                new.push_str(suffix);
            }
            seen.push(new.clone());
            new
        })
        .collect()
}

fn names_of(v: &Vector) -> Vec<String> {
    v.attributes.iter().map(|(n, _)| n.clone()).collect()
}

/// The features `rows` of v (repeated as needed) with `geometries` and the columns appended (a name v has gets `_2`).
fn expanded(v: &Vector, rows: &[usize], geometries: Vec<Geometry>, cols: Vec<(&str, Vec<f64>)>) -> Vector {
    let mut out = v.select(rows);
    out.geometries = geometries;
    let names: Vec<&str> = cols.iter().map(|(n, _)| *n).collect();
    let ren = rename(&names, &names_of(v), "_2");
    for (name, (_, x)) in ren.into_iter().zip(cols) {
        out.attributes.push((name, Column::Numbers(x)));
    }
    out
}

/// Whether a geometry is nothing (empty, or without a vertex).
fn nothing(g: &Geometry) -> bool {
    matches!(g, Geometry::Empty) || g.bounds().is_none()
}

fn check_lines(v: &Vector, what: &str) -> std::result::Result<(), String> {
    if v.geometries.iter().any(|g| !nothing(g) && !matches!(g, Geometry::LineString(_) | Geometry::MultiLineString(_)))
    {
        return Err(format!("{what} must be lines (LineString or MultiLineString)"));
    }
    Ok(())
}

/// The parts of a line as (index of the part, vertices); the parts without a segment are skipped.
fn line_parts(g: &Geometry) -> Vec<(usize, &[[f64; 2]])> {
    match g {
        Geometry::LineString(p) if p.len() >= 2 => vec![(0, p.as_slice())],
        Geometry::MultiLineString(m) => {
            m.iter().enumerate().filter(|(_, p)| p.len() >= 2).map(|(i, p)| (i, p.as_slice())).collect()
        }
        _ => Vec::new(),
    }
}

/// One line part: its vertices, cumulative lengths and the point at a measure.
struct Line<'a> {
    p: &'a [[f64; 2]],
    seg: Vec<f64>,
    /// cumulative lengths, one per vertex
    c: Vec<f64>,
    length: f64,
}

impl<'a> Line<'a> {
    fn new(p: &'a [[f64; 2]]) -> Line<'a> {
        let seg: Vec<f64> = p.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).collect();
        let mut c = Vec::with_capacity(p.len());
        let mut sum = 0.0f64;
        c.push(sum);
        for s in &seg {
            sum += s;
            c.push(sum);
        }
        Line { p, seg, c, length: sum }
    }

    /// The point at the measure m (clamped to the ends).
    fn at(&self, m: f64) -> [f64; 2] {
        if m <= 0.0 {
            return self.p[0];
        }
        if m >= self.length {
            return self.p[self.p.len() - 1];
        }
        // numpy's searchsorted(c[1:], m, side="left")
        let j = self.c[1..].partition_point(|&v| v < m).min(self.seg.len() - 1);
        let f = (m - self.c[j]) / self.seg[j];
        let (a, b) = (self.p[j], self.p[j + 1]);
        [a[0] + f * (b[0] - a[0]), a[1] + f * (b[1] - a[1])]
    }

    /// The line between the measures ms <= me: the point at ms, the vertices strictly between, the point at me.
    fn piece(&self, ms: f64, me: f64) -> Vec<[f64; 2]> {
        let mut out = vec![self.at(ms)];
        out.extend((0..self.p.len()).filter(|&k| self.c[k] > ms && self.c[k] < me).map(|k| self.p[k]));
        out.push(self.at(me));
        out
    }
}

/// `vector.points_along`.
struct PointsAlong;

impl Operator for PointsAlong {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.points_along",
            "Points along each line, at a fixed `spacing` or in a fixed number `n` (PostGIS's \
             ST_LineInterpolatePoints, QGIS's Points along geometry). With `spacing` they lie at the measures \
             start + k * spacing while those are within the length; with `n` (1 or more) at length * k / (n - 1), \
             the first at the start and the last at the end (n = 1: the midpoint). A multi-line is taken part by \
             part, its measures restarting at 0 on each. One feature per point with the attributes of its line \
             repeated and `part` (the part of the line) and `measure` (along it).",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the lines"))
        .output(PortSpec::new("vector", Type::Vector, "the points"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional("spacing", Type::Number, "the distance between points, in the layer's units"))
        .param(PortSpec::optional("n", Type::Number, "the number of points on each line (instead of spacing)"))
        .param(PortSpec::optional("start", Type::Number, "spacing: the measure of the first point (0)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let spacing: Option<f64> = args.opt_param("spacing")?;
        let n: Option<f64> = args.opt_param("n")?;
        let start: f64 = args.param("start", 0.0)?;
        if spacing.is_some() == n.is_some() {
            return Err(e("give exactly one of spacing and n".into()));
        }
        if let Some(s) = spacing
            && !(s.is_finite() && s > 0.0)
        {
            return Err(e(format!("spacing must be a finite number > 0, not {s}")));
        }
        if let Some(n) = n
            && !(n.fract() == 0.0 && (1.0..=1e9).contains(&n))
        {
            return Err(e(format!("n must be an integer >= 1, not {n}")));
        }
        if !(start.is_finite() && start >= 0.0) {
            return Err(e(format!("start must be a finite number >= 0, not {start}")));
        }
        let v = vector_of(args, "vector")?;
        check_lines(v, "the features").map_err(&e)?;
        let (mut rows, mut parts, mut measures, mut geoms) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for (i, g) in v.geometries.iter().enumerate() {
            for (pi, p) in line_parts(g) {
                let ln = Line::new(p);
                let ms: Vec<f64> = if let Some(sp) = spacing {
                    let kmax = if ln.length >= start { ((ln.length - start) / sp).floor() as i64 + 2 } else { 0 };
                    (0..=kmax.max(0)).map(|k| start + k as f64 * sp).filter(|&m| m <= ln.length).collect()
                } else {
                    let n = n.expect("one of them") as i64;
                    if n == 1 {
                        vec![ln.length / 2.0]
                    } else {
                        (0..n).map(|k| (ln.length * k as f64 / (n - 1) as f64).min(ln.length)).collect()
                    }
                };
                for m in ms {
                    geoms.push(Geometry::Point(ln.at(m)));
                    rows.push(i);
                    parts.push(pi as f64);
                    measures.push(m);
                }
            }
        }
        Ok(give(expanded(v, &rows, geoms, vec![("part", parts), ("measure", measures)])))
    }
}

/// The geometry of a line cut part by part (the line itself when it has no part).
fn cut(g: &Geometry, bounds: &dyn Fn(f64) -> (f64, f64)) -> Geometry {
    let mut pieces: Vec<Vec<[f64; 2]>> = line_parts(g)
        .into_iter()
        .map(|(_, p)| {
            let ln = Line::new(p);
            let (ms, me) = bounds(ln.length);
            ln.piece(ms, me)
        })
        .collect();
    match pieces.len() {
        0 => g.clone(),
        1 => Geometry::LineString(pieces.remove(0)),
        _ => Geometry::MultiLineString(pieces),
    }
}

/// `vector.substring`.
struct Substring;

impl Operator for Substring {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.substring",
            "Each line (each part of a multi-line) cut between the measures `start` and `end` (PostGIS's \
             ST_LineSubstring, QGIS's Line substring); with `normalized` they are fractions of the length of the \
             part. Both are clamped to the part, and start must not exceed end. The piece is the point at start, \
             every vertex strictly between and the point at end; a piece of no length stays a line of two equal \
             points. A multi-line of several parts gives a multi-line. Empty features stay as they are.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the lines"))
        .output(PortSpec::new("vector", Type::Vector, "the pieces"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::new("start", Type::Number, "the measure where the piece starts"))
        .param(PortSpec::new("end", Type::Number, "the measure where the piece ends"))
        .param(PortSpec::optional(
            "normalized",
            Type::Bool,
            "start and end are fractions of the length (false)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let start: f64 = args.opt_param("start")?.ok_or_else(|| e("give the start".into()))?;
        let end: f64 = args.opt_param("end")?.ok_or_else(|| e("give the end".into()))?;
        let normalized: bool = args.param("normalized", false)?;
        if !(start.is_finite() && end.is_finite()) {
            return Err(e("start and end must be finite numbers".into()));
        }
        if start > end {
            return Err(e(format!("start ({start}) must not be greater than end ({end})")));
        }
        let v = vector_of(args, "vector")?;
        check_lines(v, "the features").map_err(&e)?;
        let bound = |x: f64, length: f64| (if normalized { x * length } else { x }).max(0.0).min(length);
        let geoms: Vec<Geometry> =
            v.geometries.iter().map(|g| cut(g, &|length| (bound(start, length), bound(end, length)))).collect();
        let mut out = Vector::clone(v);
        out.geometries = geoms;
        Ok(give(out))
    }
}

/// `vector.split_lines`.
struct SplitLines;

impl Operator for SplitLines {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.split_lines",
            "Each line cut into consecutive pieces of at most `length` (QGIS's Split lines by maximum length; in \
             PostGIS ST_LineSubstring over a generate_series): each part is cut at the measures k * length, the \
             last piece being the shorter, and no piece has zero length. One feature per piece with the \
             attributes repeated and `part` (the number of the piece within its line, over all its parts); a \
             line without length gives none.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the lines"))
        .output(PortSpec::new("vector", Type::Vector, "the pieces"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::new("length", Type::Number, "the longest piece, in the layer's units"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let length: f64 = args.opt_param("length")?.ok_or_else(|| e("give the length".into()))?;
        if !(length.is_finite() && length > 0.0) {
            return Err(e(format!("length must be a finite number > 0, not {length}")));
        }
        let v = vector_of(args, "vector")?;
        check_lines(v, "the features").map_err(&e)?;
        let (mut rows, mut seq, mut geoms) = (Vec::new(), Vec::new(), Vec::new());
        for (i, g) in v.geometries.iter().enumerate() {
            let mut k = 0usize;
            for (_, p) in line_parts(g) {
                let ln = Line::new(p);
                if ln.length <= 0.0 {
                    continue;
                }
                let count = (ln.length / length).ceil() as i64 + 2;
                for kk in (0..count).map(|k| k as f64) {
                    if kk * length >= ln.length {
                        break;
                    }
                    let (s, en) = (kk * length, ((kk + 1.0) * length).min(ln.length));
                    geoms.push(Geometry::LineString(ln.piece(s, en)));
                    rows.push(i);
                    seq.push(k as f64);
                    k += 1;
                }
            }
        }
        Ok(give(expanded(v, &rows, geoms, vec![("part", seq)])))
    }
}

/// The nearest feature of b to each feature of a (lowest index on a tie) and GEOS's distance to it.
fn nearest_of(a: &Vector, b: &Vector) -> std::result::Result<Vec<Option<(usize, f64)>>, String> {
    GeosIndex::new(&b.geometries).and_then(|t| t.nearest(&a.geometries, None))
}

/// `vector.locate`.
struct Locate;

impl Operator for Locate {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.locate",
            "For each feature, the nearest line of another layer and the measure along it of the closest point \
             (PostGIS's ST_LineLocatePoint, QGIS's Join by nearest): `line` (the feature of the lines, -1 for \
             none; the first on a tie), `measure` (GEOS's, of the point of the line closest to the feature; along \
             a multi-line its parts in sequence) and `distance` (the layer's units). An empty feature, or no \
             lines, gives -1 and NaN. The lines are reprojected to the first layer's coordinate system.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features to locate"))
        .input(PortSpec::new("lines", Type::Vector, "the lines to measure along"))
        .output(PortSpec::new("vector", Type::Vector, "the features with line, measure and distance"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let a = vector_of(args, "vector")?;
        let b = like(vector_of(args, "lines")?, &a.crs)?;
        check_lines(&b, "lines").map_err(&e)?;
        let found = nearest_of(a, &b).map_err(&e)?;
        let hits: Vec<(usize, usize)> = found.iter().enumerate().filter_map(|(i, f)| f.map(|(j, _)| (i, j))).collect();
        let near = nearest_points(&a.geometries, &b.geometries, &hits).map_err(&e)?;
        let at: Vec<(usize, [f64; 2])> = hits.iter().zip(&near).map(|(&(_, j), p)| (j, p[1])).collect();
        let m = project(&b.geometries, &at).map_err(&e)?;
        let mut measure = vec![f64::NAN; a.len()];
        for (&(i, _), m) in hits.iter().zip(m) {
            measure[i] = m;
        }
        let line = found.iter().map(|f| f.map_or(-1.0, |(j, _)| j as f64)).collect();
        let dist = found.iter().map(|f| f.map_or(f64::NAN, |(_, d)| d)).collect();
        let rows: Vec<usize> = (0..a.len()).collect();
        let cols = vec![("line", line), ("measure", measure), ("distance", dist)];
        Ok(give(expanded(a, &rows, a.geometries.clone(), cols)))
    }
}

/// `vector.shortest_line`.
struct ShortestLine;

impl Operator for ShortestLine {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.shortest_line",
            "The line from each feature to the nearest feature of another layer (PostGIS's ST_ShortestLine, \
             QGIS's Shortest line between features): GEOS's nearest points of the two, the first of them on the \
             feature. Each keeps its attributes and takes those of the other feature (a name the first has gets \
             `suffix`) and `distance`; the first feature on a tie. An empty feature, or an empty other layer, \
             gives an empty line and no values. The other layer is reprojected to the first's.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features the lines start from"))
        .input(PortSpec::new("other", Type::Vector, "the features they end on"))
        .output(PortSpec::new("vector", Type::Vector, "the lines, with both layers' attributes and distance"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional("suffix", Type::Text, "after a name of the other layer the first has (_2)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let a = vector_of(args, "vector")?;
        let b = like(vector_of(args, "other")?, &a.crs)?;
        let suffix: String = args.param("suffix", "_2".to_string())?;
        let found = nearest_of(a, &b).map_err(&e)?;
        let hits: Vec<(usize, usize)> = found.iter().enumerate().filter_map(|(i, f)| f.map(|(j, _)| (i, j))).collect();
        let near = nearest_points(&a.geometries, &b.geometries, &hits).map_err(&e)?;
        let mut geometries = vec![Geometry::Empty; a.len()];
        for (&(i, _), p) in hits.iter().zip(&near) {
            geometries[i] = Geometry::LineString(vec![p[0], p[1]]);
        }
        // the other layer's attributes, renamed against the first's, then distance
        let mut attributes = a.attributes.clone();
        let bnames: Vec<&str> = b.attributes.iter().map(|(n, _)| n.as_str()).collect();
        let renamed = rename(&bnames, &names_of(a), &suffix);
        for (name, (_, c)) in renamed.iter().zip(&b.attributes) {
            let col = match c {
                Column::Numbers(x) => {
                    Column::Numbers(found.iter().map(|f| f.map_or(f64::NAN, |(j, _)| x[j])).collect())
                }
                Column::Texts(x) => {
                    Column::Texts(found.iter().map(|f| f.map_or(String::new(), |(j, _)| x[j].clone())).collect())
                }
            };
            attributes.push((name.clone(), col));
        }
        let taken: Vec<String> = attributes.iter().map(|(n, _)| n.clone()).collect();
        let name = rename(&["distance"], &taken, &suffix).remove(0);
        attributes.push((name, Column::Numbers(found.iter().map(|f| f.map_or(f64::NAN, |(_, d)| d)).collect())));
        Ok(give(Vector { geometries, attributes, crs: a.crs.clone() }))
    }
}

/// `vector.closest_point`.
struct ClosestPoint;

impl Operator for ClosestPoint {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.closest_point",
            "Each point moved to the nearest point of the nearest feature of another layer (PostGIS's \
             ST_ClosestPoint, QGIS's Snap geometries to layer, point to segment): the target is the point of a \
             segment, not only a vertex. With `max_distance` a point farther from its target stays where it is. \
             Adds `moved` (the distance moved, 0 for a point left as it is) and `snapped` (1 when the point was \
             set on a target, even if it already lay on it). The other layer is reprojected to the first's.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the points to move"))
        .input(PortSpec::new("other", Type::Vector, "the features to move them onto"))
        .output(PortSpec::new("vector", Type::Vector, "the points, with moved and snapped"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional("max_distance", Type::Number, "the farthest a point is moved (none: any)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let a = vector_of(args, "vector")?;
        let b = like(vector_of(args, "other")?, &a.crs)?;
        if a.geometries.iter().any(|g| !nothing(g) && !matches!(g, Geometry::Point(_))) {
            return Err(e("the features must be points".into()));
        }
        let max: Option<f64> = args.opt_param("max_distance")?;
        if let Some(m) = max
            && (m.is_nan() || m < 0.0)
        {
            return Err(e(format!("max_distance must be a number >= 0, not {m}")));
        }
        let found = nearest_of(a, &b).map_err(&e)?;
        let hits: Vec<(usize, usize)> = found.iter().enumerate().filter_map(|(i, f)| f.map(|(j, _)| (i, j))).collect();
        let near = nearest_points(&a.geometries, &b.geometries, &hits).map_err(&e)?;
        let mut geometries = a.geometries.clone();
        let (mut moved, mut snapped) = (vec![0.0; a.len()], vec![0.0; a.len()]);
        for (&(i, _), p) in hits.iter().zip(&near) {
            let d = (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]);
            if max.is_none_or(|m| d <= m) {
                geometries[i] = Geometry::Point(p[1]);
                moved[i] = d;
                snapped[i] = 1.0;
            }
        }
        let rows: Vec<usize> = (0..a.len()).collect();
        Ok(give(expanded(a, &rows, geometries, vec![("moved", moved), ("snapped", snapped)])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(op: &dyn Operator, ins: Vec<(&str, Vector)>, params: serde_json::Value) -> Result<Outputs> {
        let args = Args {
            node: "n",
            inputs: ins.into_iter().map(|(k, v)| (k.to_string(), Value::Vector(Arc::new(v)))).collect(),
            params: params.as_object().unwrap(),
        };
        op.run(&mut Ctx::default(), &args)
    }

    fn vector(o: &Outputs) -> Arc<Vector> {
        let Value::Vector(v) = &o["vector"] else { panic!() };
        v.clone()
    }

    fn ready() -> bool {
        terraflux_gdal::geos::geos().is_ok()
    }

    fn lines() -> Vector {
        Vector {
            geometries: vec![
                Geometry::LineString(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]),
                Geometry::MultiLineString(vec![vec![[0.0, 5.0], [4.0, 5.0]], vec![[0.0, 8.0], [0.0, 8.0]]]),
                Geometry::Empty,
            ],
            attributes: vec![
                ("id".into(), Column::Numbers(vec![1.0, 2.0, 3.0])),
                ("part".into(), Column::Numbers(vec![9.0, 9.0, 9.0])),
            ],
            crs: None,
        }
    }

    #[test]
    fn points_along_by_spacing_and_by_number() {
        let v = vector(&run(&PointsAlong, vec![("vector", lines())], serde_json::json!({"spacing": 8.0})).unwrap());
        // line 1 (length 20): 0, 8, 16; line 2: part 0 (length 4): 0; part 1 has no length: 0
        assert_eq!(v.column("measure").unwrap().numbers(), vec![0.0, 8.0, 16.0, 0.0, 0.0]);
        assert_eq!(v.geometries[2], Geometry::Point([10.0, 6.0]));
        assert_eq!(v.column("id").unwrap().numbers(), vec![1.0, 1.0, 1.0, 2.0, 2.0]);
        assert_eq!(v.column("part").unwrap().numbers(), vec![9.0; 5]);
        assert_eq!(v.column("part_2").unwrap().numbers(), vec![0.0, 0.0, 0.0, 0.0, 1.0]);
        let n = vector(&run(&PointsAlong, vec![("vector", lines())], serde_json::json!({"n": 3})).unwrap());
        assert_eq!(n.geometries[1], Geometry::Point([10.0, 0.0]));
        assert_eq!(n.geometries[2], Geometry::Point([10.0, 10.0]));
        let one = vector(&run(&PointsAlong, vec![("vector", lines())], serde_json::json!({"n": 1})).unwrap());
        assert_eq!(one.geometries[0], Geometry::Point([10.0, 0.0]));
        let late = vector(
            &run(&PointsAlong, vec![("vector", lines())], serde_json::json!({"spacing": 8.0, "start": 5.0})).unwrap(),
        );
        assert_eq!(late.column("measure").unwrap().numbers(), vec![5.0, 13.0]);
        for bad in [serde_json::json!({}), serde_json::json!({"spacing": 1, "n": 2}), serde_json::json!({"n": 0})] {
            assert!(run(&PointsAlong, vec![("vector", lines())], bad).is_err());
        }
        let pts = Vector { geometries: vec![Geometry::Point([0.0, 0.0])], ..Vector::default() };
        assert!(run(&PointsAlong, vec![("vector", pts)], serde_json::json!({"n": 2})).is_err());
    }

    #[test]
    fn substring_clamps_and_cuts_parts() {
        let p = serde_json::json!({"start": 5.0, "end": 14.0});
        let v = vector(&run(&Substring, vec![("vector", lines())], p).unwrap());
        assert_eq!(v.geometries[0], Geometry::LineString(vec![[5.0, 0.0], [10.0, 0.0], [10.0, 4.0]]));
        // two parts: a multi-line, the second of no length stays two equal points
        let Geometry::MultiLineString(m) = &v.geometries[1] else { panic!() };
        assert_eq!(m[0], vec![[4.0, 5.0], [4.0, 5.0]]);
        assert_eq!(m[1], vec![[0.0, 8.0], [0.0, 8.0]]);
        assert_eq!(v.geometries[2], Geometry::Empty);
        let q = serde_json::json!({"start": 0.25, "end": 1.5, "normalized": true});
        let w = vector(&run(&Substring, vec![("vector", lines())], q).unwrap());
        assert_eq!(w.geometries[0], Geometry::LineString(vec![[5.0, 0.0], [10.0, 0.0], [10.0, 10.0]]));
        assert!(run(&Substring, vec![("vector", lines())], serde_json::json!({"start": 2.0, "end": 1.0})).is_err());
        assert!(run(&Substring, vec![("vector", lines())], serde_json::json!({"start": 2.0})).is_err());
    }

    #[test]
    fn split_lines_numbers_the_pieces_and_drops_zero_length() {
        let v = vector(&run(&SplitLines, vec![("vector", lines())], serde_json::json!({"length": 8.0})).unwrap());
        // 20 -> 8, 8, 4; 4 -> one piece; the part of no length gives none
        assert_eq!(v.len(), 4);
        assert_eq!(v.column("id").unwrap().numbers(), vec![1.0, 1.0, 1.0, 2.0]);
        assert_eq!(v.column("part_2").unwrap().numbers(), vec![0.0, 1.0, 2.0, 0.0]);
        assert_eq!(v.geometries[1], Geometry::LineString(vec![[8.0, 0.0], [10.0, 0.0], [10.0, 6.0]]));
        assert_eq!(v.geometries[2], Geometry::LineString(vec![[10.0, 6.0], [10.0, 10.0]]));
        // a length that divides the line leaves no zero-length piece at the end
        let w = vector(&run(&SplitLines, vec![("vector", lines())], serde_json::json!({"length": 10.0})).unwrap());
        assert_eq!(w.len(), 3);
        assert!(run(&SplitLines, vec![("vector", lines())], serde_json::json!({"length": 0.0})).is_err());
    }

    fn stops() -> Vector {
        Vector {
            geometries: vec![
                Geometry::Point([4.0, 3.0]),
                Geometry::Point([12.0, 6.0]),
                Geometry::Empty,
                Geometry::Point([100.0, 100.0]),
            ],
            attributes: vec![("id".into(), Column::Numbers(vec![1.0, 2.0, 3.0, 4.0]))],
            crs: None,
        }
    }

    fn roads() -> Vector {
        Vector {
            geometries: vec![
                Geometry::LineString(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]),
                Geometry::LineString(vec![[0.0, 50.0], [20.0, 50.0]]),
            ],
            attributes: vec![
                ("id".into(), Column::Numbers(vec![7.0, 8.0])),
                ("name".into(), Column::Texts(vec!["a".into(), "b".into()])),
            ],
            crs: None,
        }
    }

    #[test]
    fn locate_gives_line_measure_and_distance() {
        if !ready() {
            return;
        }
        let o = run(&Locate, vec![("vector", stops()), ("lines", roads())], serde_json::json!({})).unwrap();
        let v = vector(&o);
        assert_eq!(v.column("line").unwrap().numbers(), vec![0.0, 0.0, -1.0, 1.0]);
        let m = v.column("measure").unwrap().numbers();
        assert_eq!((m[0], m[1]), (4.0, 16.0));
        assert!(m[2].is_nan());
        let d = v.column("distance").unwrap().numbers();
        assert_eq!((d[0], d[1]), (3.0, 2.0));
        assert!(d[2].is_nan());
        assert!(run(&Locate, vec![("vector", roads()), ("lines", stops())], serde_json::json!({})).is_err());
    }

    #[test]
    fn shortest_line_joins_attributes_and_renames() {
        if !ready() {
            return;
        }
        let v =
            vector(&run(&ShortestLine, vec![("vector", stops()), ("other", roads())], serde_json::json!({})).unwrap());
        assert_eq!(v.geometries[0], Geometry::LineString(vec![[4.0, 3.0], [4.0, 0.0]]));
        assert_eq!(v.geometries[2], Geometry::Empty);
        let names: Vec<&str> = v.attributes.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["id", "id_2", "name", "distance"]);
        assert_eq!(v.column("id_2").unwrap().numbers()[..2], [7.0, 7.0]);
        assert!(v.column("id_2").unwrap().numbers()[2].is_nan());
        assert!(matches!(v.column("name"), Some(Column::Texts(t)) if t[3] == "b" && t[2].is_empty()));
        assert_eq!(v.column("distance").unwrap().numbers()[0], 3.0);
    }

    #[test]
    fn closest_point_snaps_within_max_distance() {
        if !ready() {
            return;
        }
        let o = run(
            &ClosestPoint,
            vec![("vector", stops()), ("other", roads())],
            serde_json::json!({"max_distance": 10.0}),
        )
        .unwrap();
        let v = vector(&o);
        assert_eq!(v.geometries[0], Geometry::Point([4.0, 0.0]));
        assert_eq!(v.geometries[1], Geometry::Point([10.0, 6.0]));
        assert_eq!(v.geometries[2], Geometry::Empty);
        // the fourth is 50 from the second road
        assert_eq!(v.geometries[3], Geometry::Point([100.0, 100.0]));
        assert_eq!(v.column("moved").unwrap().numbers(), vec![3.0, 2.0, 0.0, 0.0]);
        assert_eq!(v.column("snapped").unwrap().numbers(), vec![1.0, 1.0, 0.0, 0.0]);
        let all =
            vector(&run(&ClosestPoint, vec![("vector", stops()), ("other", roads())], serde_json::json!({})).unwrap());
        assert_eq!(all.geometries[3], Geometry::Point([20.0, 50.0]));
        assert!(run(&ClosestPoint, vec![("vector", roads()), ("other", stops())], serde_json::json!({})).is_err());
        assert!(
            run(&ClosestPoint, vec![("vector", stops()), ("other", roads())], serde_json::json!({"max_distance": -1}))
                .is_err()
        );
    }
}
