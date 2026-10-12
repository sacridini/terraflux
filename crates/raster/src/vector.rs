//! Vector features: their geometries (points, lines, polygons and their multi forms, in 2-D), their attributes
//! (numbers or texts) and the coordinate system (WKT); geometries to and from WKB (OGR's ISO form).

/// A point.
pub type Xy = [f64; 2];

/// A geometry in 2-D (Z and M dropped). A polygon is its rings, the outer first.
#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    Empty,
    Point(Xy),
    LineString(Vec<Xy>),
    Polygon(Vec<Vec<Xy>>),
    MultiPoint(Vec<Xy>),
    MultiLineString(Vec<Vec<Xy>>),
    MultiPolygon(Vec<Vec<Vec<Xy>>>),
}

/// An attribute's values, one per feature (NaN or "" without a value).
#[derive(Debug, Clone, PartialEq)]
pub enum Column {
    Numbers(Vec<f64>),
    Texts(Vec<String>),
}

impl Column {
    pub fn len(&self) -> usize {
        match self {
            Column::Numbers(v) => v.len(),
            Column::Texts(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The values as numbers (texts read as numbers where they are, else NaN).
    pub fn numbers(&self) -> Vec<f64> {
        match self {
            Column::Numbers(v) => v.clone(),
            Column::Texts(v) => v.iter().map(|s| s.trim().parse().unwrap_or(f64::NAN)).collect(),
        }
    }
}

/// Features: a geometry and attributes each, in a coordinate system.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Vector {
    pub geometries: Vec<Geometry>,
    /// the attributes in order, a value per feature
    pub attributes: Vec<(String, Column)>,
    /// the coordinate system as WKT (none: unknown)
    pub crs: Option<String>,
}

impl Vector {
    pub fn len(&self) -> usize {
        self.geometries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.geometries.is_empty()
    }

    pub fn column(&self, name: &str) -> Option<&Column> {
        self.attributes.iter().find(|(n, _)| n == name).map(|(_, c)| c)
    }

    /// The features kept (in their order).
    pub fn select(&self, keep: &[usize]) -> Vector {
        Vector {
            geometries: keep.iter().map(|&i| self.geometries[i].clone()).collect(),
            attributes: self
                .attributes
                .iter()
                .map(|(n, c)| {
                    let c = match c {
                        Column::Numbers(v) => Column::Numbers(keep.iter().map(|&i| v[i]).collect()),
                        Column::Texts(v) => Column::Texts(keep.iter().map(|&i| v[i].clone()).collect()),
                    };
                    (n.clone(), c)
                })
                .collect(),
            crs: self.crs.clone(),
        }
    }

    /// The bounds of every geometry: xmin, ymin, xmax, ymax (none for the empty ones).
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for g in &self.geometries {
            g.points(&mut |p| {
                b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
            });
        }
        b[0].is_finite().then_some(b)
    }
}

impl Geometry {
    /// Every point of the geometry.
    pub fn points(&self, f: &mut dyn FnMut(Xy)) {
        match self {
            Geometry::Empty => {}
            Geometry::Point(p) => f(*p),
            Geometry::LineString(l) | Geometry::MultiPoint(l) => l.iter().for_each(|p| f(*p)),
            Geometry::Polygon(r) | Geometry::MultiLineString(r) => r.iter().flatten().for_each(|p| f(*p)),
            Geometry::MultiPolygon(m) => m.iter().flatten().flatten().for_each(|p| f(*p)),
        }
    }

    /// The polygons of a polygon or multipolygon (none of other kinds).
    pub fn polygons(&self) -> Vec<&Vec<Vec<Xy>>> {
        match self {
            Geometry::Polygon(r) => vec![r],
            Geometry::MultiPolygon(m) => m.iter().collect(),
            _ => Vec::new(),
        }
    }

    pub fn is_polygonal(&self) -> bool {
        matches!(self, Geometry::Polygon(_) | Geometry::MultiPolygon(_))
    }

    pub fn is_point(&self) -> bool {
        matches!(self, Geometry::Point(_) | Geometry::MultiPoint(_))
    }

    /// The bounds: xmin, ymin, xmax, ymax.
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        self.points(&mut |p| b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
        b[0].is_finite().then_some(b)
    }

    /// The geometry from WKB (ISO or OGR's extended, either byte order); Z and M dropped.
    pub fn from_wkb(b: &[u8]) -> Result<Geometry, String> {
        let mut r = Wkb { b, at: 0 };
        let g = r.geometry()?;
        Ok(g)
    }

    /// The geometry as ISO WKB, little-endian, 2-D.
    pub fn to_wkb(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let head = |out: &mut Vec<u8>, t: u32| {
            out.push(1);
            out.extend_from_slice(&t.to_le_bytes());
        };
        let n = |out: &mut Vec<u8>, k: usize| out.extend_from_slice(&(k as u32).to_le_bytes());
        let pt = |out: &mut Vec<u8>, p: &Xy| {
            out.extend_from_slice(&p[0].to_le_bytes());
            out.extend_from_slice(&p[1].to_le_bytes());
        };
        let line = |out: &mut Vec<u8>, l: &[Xy]| {
            n(out, l.len());
            l.iter().for_each(|p| pt(out, p));
        };
        let poly = |out: &mut Vec<u8>, r: &[Vec<Xy>]| {
            n(out, r.len());
            r.iter().for_each(|l| line(out, l));
        };
        match self {
            Geometry::Empty => {
                head(&mut out, 7);
                n(&mut out, 0);
            }
            Geometry::Point(p) => {
                head(&mut out, 1);
                pt(&mut out, p);
            }
            Geometry::LineString(l) => {
                head(&mut out, 2);
                line(&mut out, l);
            }
            Geometry::Polygon(r) => {
                head(&mut out, 3);
                poly(&mut out, r);
            }
            Geometry::MultiPoint(ps) => {
                head(&mut out, 4);
                n(&mut out, ps.len());
                for p in ps {
                    head(&mut out, 1);
                    pt(&mut out, p);
                }
            }
            Geometry::MultiLineString(ls) => {
                head(&mut out, 5);
                n(&mut out, ls.len());
                for l in ls {
                    head(&mut out, 2);
                    line(&mut out, l);
                }
            }
            Geometry::MultiPolygon(ps) => {
                head(&mut out, 6);
                n(&mut out, ps.len());
                for p in ps {
                    head(&mut out, 3);
                    poly(&mut out, p);
                }
            }
        }
        out
    }
}

struct Wkb<'a> {
    b: &'a [u8],
    at: usize,
}

impl Wkb<'_> {
    fn take(&mut self, k: usize) -> Result<&[u8], String> {
        if self.at + k > self.b.len() {
            return Err("WKB: truncated".into());
        }
        let s = &self.b[self.at..self.at + k];
        self.at += k;
        Ok(s)
    }

    fn u32(&mut self, le: bool) -> Result<u32, String> {
        let s: [u8; 4] = self.take(4)?.try_into().unwrap();
        Ok(if le { u32::from_le_bytes(s) } else { u32::from_be_bytes(s) })
    }

    fn f64(&mut self, le: bool) -> Result<f64, String> {
        let s: [u8; 8] = self.take(8)?.try_into().unwrap();
        Ok(if le { f64::from_le_bytes(s) } else { f64::from_be_bytes(s) })
    }

    fn point(&mut self, le: bool, dims: usize) -> Result<Xy, String> {
        let x = self.f64(le)?;
        let y = self.f64(le)?;
        for _ in 2..dims {
            self.f64(le)?;
        }
        Ok([x, y])
    }

    fn line(&mut self, le: bool, dims: usize) -> Result<Vec<Xy>, String> {
        let n = self.u32(le)? as usize;
        (0..n).map(|_| self.point(le, dims)).collect()
    }

    fn rings(&mut self, le: bool, dims: usize) -> Result<Vec<Vec<Xy>>, String> {
        let n = self.u32(le)? as usize;
        (0..n).map(|_| self.line(le, dims)).collect()
    }

    fn geometry(&mut self) -> Result<Geometry, String> {
        let le = self.take(1)?[0] == 1;
        let t = self.u32(le)?;
        // ISO: +1000 Z, +2000 M, +3000 ZM; OGR's extended: the high bits
        let (z, m) = (t & 0x8000_0000 != 0, t & 0x4000_0000 != 0);
        let t = t & 0x0FFF_FFFF;
        let (base, iso) = (t % 1000, t / 1000);
        let dims = 2 + usize::from(z || iso == 1 || iso == 3) + usize::from(m || iso == 2 || iso == 3);
        Ok(match base {
            1 => {
                let p = self.point(le, dims)?;
                if p[0].is_nan() && p[1].is_nan() { Geometry::Empty } else { Geometry::Point(p) }
            }
            2 => Geometry::LineString(self.line(le, dims)?),
            3 => Geometry::Polygon(self.rings(le, dims)?),
            4..=7 => {
                let n = self.u32(le)? as usize;
                let parts: Vec<Geometry> = (0..n).map(|_| self.geometry()).collect::<Result<_, _>>()?;
                collect(base, parts)
            }
            _ => return Err(format!("WKB: geometries of type {t} are not read")),
        })
    }
}

/// The parts of a multi geometry (or of a collection: its points, lines or polygons, the kind of its first).
fn collect(base: u32, parts: Vec<Geometry>) -> Geometry {
    let (mut pts, mut lines, mut polys) = (Vec::new(), Vec::new(), Vec::new());
    for p in parts {
        match p {
            Geometry::Point(p) => pts.push(p),
            Geometry::MultiPoint(v) => pts.extend(v),
            Geometry::LineString(l) => lines.push(l),
            Geometry::MultiLineString(v) => lines.extend(v),
            Geometry::Polygon(r) => polys.push(r),
            Geometry::MultiPolygon(v) => polys.extend(v),
            Geometry::Empty => {}
        }
    }
    match base {
        4 => Geometry::MultiPoint(pts),
        5 => Geometry::MultiLineString(lines),
        6 => Geometry::MultiPolygon(polys),
        _ if !polys.is_empty() => Geometry::MultiPolygon(polys),
        _ if !lines.is_empty() => Geometry::MultiLineString(lines),
        _ if !pts.is_empty() => Geometry::MultiPoint(pts),
        _ => Geometry::Empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wkb_round_trip() {
        let gs = [
            Geometry::Point([1.5, -2.0]),
            Geometry::LineString(vec![[0.0, 0.0], [1.0, 2.0]]),
            Geometry::Polygon(vec![vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]]),
            Geometry::MultiPolygon(vec![
                vec![vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]],
                vec![vec![[5.0, 5.0], [6.0, 5.0], [6.0, 6.0], [5.0, 5.0]]],
            ]),
            Geometry::MultiPoint(vec![[1.0, 1.0], [2.0, 2.0]]),
        ];
        for g in gs {
            assert_eq!(Geometry::from_wkb(&g.to_wkb()).unwrap(), g);
        }
        // a point with Z (ISO 1001), big-endian
        let mut b = vec![0u8];
        b.extend_from_slice(&1001u32.to_be_bytes());
        for v in [1.0f64, 2.0, 3.0] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        assert_eq!(Geometry::from_wkb(&b).unwrap(), Geometry::Point([1.0, 2.0]));
    }
}
