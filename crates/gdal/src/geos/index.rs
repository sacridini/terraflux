//! Spatial predicates and nearest features between two layers through GEOS's STRtree and prepared geometries
//! (shapely's `STRtree.query(a, predicate=...)` and `nearest`), instead of testing every pair.

use std::ffi::{c_double, c_int, c_void};

use terraflux_raster::vector::Geometry;

use super::{Api, Ctx, G, Geos, Poly, Ring, Session, geos};
use crate::Result;

/// A spatial predicate between a feature of one layer (a) and one of another (b): `p(a, b)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Predicate {
    Intersects,
    Within,
    Contains,
    ContainsProperly,
    Touches,
    Crosses,
    Overlaps,
    Covers,
    CoveredBy,
    Equals,
    Dwithin,
}

impl Predicate {
    /// The predicate of a name (intersects, within, contains, contains_properly, touches, crosses, overlaps,
    /// covers, covered_by, equals, dwithin).
    pub fn from_name(name: &str) -> Option<Predicate> {
        Some(match name.trim().to_lowercase().as_str() {
            "intersects" => Predicate::Intersects,
            "within" => Predicate::Within,
            "contains" => Predicate::Contains,
            "contains_properly" => Predicate::ContainsProperly,
            "touches" => Predicate::Touches,
            "crosses" => Predicate::Crosses,
            "overlaps" => Predicate::Overlaps,
            "covers" => Predicate::Covers,
            "covered_by" => Predicate::CoveredBy,
            "equals" => Predicate::Equals,
            "dwithin" => Predicate::Dwithin,
            _ => return None,
        })
    }
}

/// The features of a layer (b) in GEOS's STRtree, to find which meet those of another (a) by a predicate, or are
/// the nearest to them. One GEOS context: not `Sync`, one thread uses it at a time.
pub struct GeosIndex {
    s: Session<'static>,
    g: &'static Geos,
    /// The GEOS geometry of each feature (null: empty, never found).
    geoms: Vec<G>,
    /// The bounds of each feature (none: empty).
    bounds: Vec<Option<[f64; 4]>>,
    tree: G,
}

unsafe impl Send for GeosIndex {}

unsafe extern "C" fn collect(item: *mut c_void, data: *mut c_void) {
    unsafe { (*(data as *mut Vec<usize>)).push(item as usize - 1) }
}

struct Near<'a> {
    /// The geometry searched from (GEOS may pass it as either item).
    query: G,
    ctx: Ctx,
    api: &'a Api,
    geoms: &'a [G],
}

unsafe extern "C" fn near(x: *const c_void, y: *const c_void, d: *mut c_double, data: *mut c_void) -> c_int {
    unsafe {
        let n = &*(data as *const Near);
        let geom = |p: *const c_void| {
            if std::ptr::eq(p, n.query as *const c_void) { n.query } else { n.geoms[p as usize - 1] }
        };
        (n.api.GEOSDistance_r)(n.ctx, geom(x), geom(y), d)
    }
}

impl Drop for GeosIndex {
    fn drop(&mut self) {
        let a = &self.g.api;
        unsafe {
            if !self.tree.is_null() {
                (a.GEOSSTRtree_destroy_r)(self.s.ctx, self.tree);
            }
            for &h in &self.geoms {
                if !h.is_null() {
                    self.s.destroy(h);
                }
            }
        }
    }
}

impl GeosIndex {
    /// The features' geometries (empty ones never meet anything) in a tree.
    pub fn new(b: &[Geometry]) -> Result<GeosIndex> {
        let g = geos().map_err(String::from)?;
        let s = g.session()?;
        let mut me = GeosIndex {
            s,
            g,
            geoms: Vec::with_capacity(b.len()),
            bounds: b.iter().map(|g| g.bounds()).collect(),
            tree: std::ptr::null_mut(),
        };
        let a = &g.api;
        unsafe {
            me.tree = (a.GEOSSTRtree_create_r)(me.s.ctx, 10);
            if me.tree.is_null() {
                return Err("GEOS could not make the spatial index".into());
            }
            let hs = me.read_all(b)?;
            for (j, &h) in hs.iter().enumerate() {
                me.geoms.push(h);
                if !h.is_null() {
                    (a.GEOSSTRtree_insert_r)(me.s.ctx, me.tree, h, (j + 1) as *mut c_void);
                }
            }
        }
        Ok(me)
    }

    pub fn len(&self) -> usize {
        self.geoms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.geoms.is_empty()
    }

    /// The geometries as GEOS's (WKB), null for the empty ones; the caller frees them.
    unsafe fn read_all(&self, v: &[Geometry]) -> Result<Vec<G>> {
        let a = &self.g.api;
        let reader = unsafe { (a.GEOSWKBReader_create_r)(self.s.ctx) };
        if reader.is_null() {
            return Err("GEOS could not make the WKB reader".into());
        }
        let mut out: Vec<G> = Vec::with_capacity(v.len());
        let mut failed = None;
        for (i, geom) in v.iter().enumerate() {
            if matches!(geom, Geometry::Empty) || geom.bounds().is_none() {
                out.push(std::ptr::null_mut());
                continue;
            }
            let wkb = geom.to_wkb();
            let h = unsafe { (a.GEOSWKBReader_read_r)(self.s.ctx, reader, wkb.as_ptr(), wkb.len()) };
            if h.is_null() {
                failed = Some(format!("GEOS could not read the geometry of feature {i}"));
                break;
            }
            if unsafe { (a.GEOSisEmpty_r)(self.s.ctx, h) } != 0 {
                unsafe { self.s.destroy(h) };
                out.push(std::ptr::null_mut());
            } else {
                out.push(h);
            }
        }
        unsafe { (a.GEOSWKBReader_destroy_r)(self.s.ctx, reader) };
        match failed {
            None => Ok(out),
            Some(e) => {
                out.into_iter().filter(|h| !h.is_null()).for_each(|h| unsafe { self.s.destroy(h) });
                Err(e)
            }
        }
    }

    /// The features of the tree whose envelopes meet that of `h` (or of its bounds `grow` larger), ascending.
    unsafe fn candidates(&self, h: G, bounds: [f64; 4], grow: f64) -> Result<Vec<usize>> {
        let mut found: Vec<usize> = Vec::new();
        let mut rect: Option<G> = None;
        if grow > 0.0 {
            // a little more than asked: a candidate too many only costs one exact test
            let big = bounds.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            let pad = grow * (1.0 + 1e-9) + big * 1e-12;
            let ring: Ring = vec![
                [bounds[0] - pad, bounds[1] - pad],
                [bounds[2] + pad, bounds[1] - pad],
                [bounds[2] + pad, bounds[3] + pad],
                [bounds[0] - pad, bounds[3] + pad],
                [bounds[0] - pad, bounds[1] - pad],
            ];
            let poly: Poly = vec![ring];
            rect = unsafe { self.s.polygon(&poly) }?;
        }
        let query = rect.unwrap_or(h);
        let a = &self.g.api;
        unsafe {
            (a.GEOSSTRtree_query_r)(
                self.s.ctx,
                self.tree,
                query,
                collect,
                &mut found as *mut Vec<usize> as *mut c_void,
            );
            if let Some(r) = rect {
                self.s.destroy(r);
            }
        }
        found.sort_unstable();
        Ok(found)
    }

    fn distance_between(&self, x: G, y: G) -> Result<f64> {
        let mut d = 0.0;
        if unsafe { (self.g.api.GEOSDistance_r)(self.s.ctx, x, y, &mut d) } == 0 {
            return Err("GEOS could not measure a distance".into());
        }
        Ok(d)
    }

    /// The pairs (i, j), ascending, with `predicate(a[i], b[j])` for the features of this index as b (shapely's
    /// `STRtree(b).query(a, predicate)`); `Dwithin` needs the distance (a_i within it of b_j, itself included).
    /// Empty geometries never match.
    pub fn pairs(&self, a: &[Geometry], predicate: Predicate, distance: Option<f64>) -> Result<Vec<(usize, usize)>> {
        let api = &self.g.api;
        let dist = match (predicate, distance) {
            (Predicate::Dwithin, Some(d)) if d >= 0.0 && d.is_finite() => d,
            (Predicate::Dwithin, _) => return Err("dwithin needs a distance of 0 or more".into()),
            _ => 0.0,
        };
        let ha = unsafe { self.read_all(a) }?;
        let mut out = Vec::new();
        let result = (|| -> Result<()> {
            for (i, &h) in ha.iter().enumerate() {
                if h.is_null() {
                    continue;
                }
                let bounds = a[i].bounds().ok_or("an empty geometry")?;
                let found =
                    unsafe { self.candidates(h, bounds, if predicate == Predicate::Dwithin { dist } else { 0.0 }) }?;
                if found.is_empty() {
                    continue;
                }
                let prep = unsafe { (api.GEOSPrepare_r)(self.s.ctx, h) };
                if prep.is_null() {
                    return Err("GEOS could not prepare a geometry".into());
                }
                let r = (|| -> Result<()> {
                    for j in found {
                        let hb = self.geoms[j];
                        let ctx = self.s.ctx;
                        let v = unsafe {
                            match predicate {
                                Predicate::Intersects => (api.GEOSPreparedIntersects_r)(ctx, prep, hb),
                                Predicate::Within => (api.GEOSPreparedWithin_r)(ctx, prep, hb),
                                Predicate::Contains => (api.GEOSPreparedContains_r)(ctx, prep, hb),
                                Predicate::ContainsProperly => (api.GEOSPreparedContainsProperly_r)(ctx, prep, hb),
                                Predicate::Touches => (api.GEOSPreparedTouches_r)(ctx, prep, hb),
                                Predicate::Crosses => (api.GEOSPreparedCrosses_r)(ctx, prep, hb),
                                Predicate::Overlaps => (api.GEOSPreparedOverlaps_r)(ctx, prep, hb),
                                Predicate::Covers => (api.GEOSPreparedCovers_r)(ctx, prep, hb),
                                Predicate::CoveredBy => (api.GEOSPreparedCoveredBy_r)(ctx, prep, hb),
                                Predicate::Equals => (api.GEOSEquals_r)(ctx, h, hb),
                                Predicate::Dwithin => match api.GEOSPreparedDistanceWithin_r {
                                    Some(f) => f(ctx, prep, hb, dist),
                                    None => {
                                        let mut d = 0.0;
                                        if (api.GEOSDistance_r)(ctx, h, hb, &mut d) == 0 {
                                            2
                                        } else {
                                            (d <= dist) as i8
                                        }
                                    }
                                },
                            }
                        };
                        match v {
                            0 => {}
                            1 => out.push((i, j)),
                            _ => return Err(format!("GEOS could not test {predicate:?} (features {i} and {j})")),
                        }
                    }
                    Ok(())
                })();
                unsafe { (api.GEOSPreparedGeom_destroy_r)(self.s.ctx, prep) };
                r?;
            }
            Ok(())
        })();
        ha.into_iter().filter(|h| !h.is_null()).for_each(|h| unsafe { self.s.destroy(h) });
        result.map(|()| out)
    }

    /// For each feature of a, the nearest feature of this index (b) and GEOS's distance to it: the lowest index
    /// on ties; none when a is empty, b has nothing, or the nearest is farther than `max_distance`.
    pub fn nearest(&self, a: &[Geometry], max_distance: Option<f64>) -> Result<Vec<Option<(usize, f64)>>> {
        let api = &self.g.api;
        let ha = unsafe { self.read_all(a) }?;
        let mut out = Vec::with_capacity(a.len());
        let result = (|| -> Result<()> {
            for (i, &h) in ha.iter().enumerate() {
                if h.is_null() || self.geoms.iter().all(|g| g.is_null()) {
                    out.push(None);
                    continue;
                }
                let near_data = Near { query: h, ctx: self.s.ctx, api, geoms: &self.geoms };
                let first = unsafe {
                    (api.GEOSSTRtree_nearest_generic_r)(
                        self.s.ctx,
                        self.tree,
                        h as *const c_void,
                        h,
                        near,
                        &near_data as *const Near as *mut c_void,
                    )
                };
                if first.is_null() {
                    out.push(None);
                    continue;
                }
                let d0 = self.distance_between(h, self.geoms[first as usize - 1])?;
                // every feature at that distance has an envelope within it of this one's
                let bounds = a[i].bounds().ok_or("an empty geometry")?;
                let found = unsafe { self.candidates(h, bounds, d0) }?;
                let mut best: Option<(usize, f64)> = None;
                for j in found {
                    let d = self.distance_between(h, self.geoms[j])?;
                    if best.is_none_or(|(_, bd)| d < bd) {
                        best = Some((j, d));
                    }
                }
                out.push(best.filter(|&(_, d)| max_distance.is_none_or(|m| d <= m)));
            }
            Ok(())
        })();
        ha.into_iter().filter(|h| !h.is_null()).for_each(|h| unsafe { self.s.destroy(h) });
        result.map(|()| out)
    }

    /// For each feature of a, its `k` nearest features of this index (b) with GEOS's distances, by (distance, index
    /// of b): the lower index first on ties. The search distance doubles until the k-th distance lies within it
    /// (or it reaches `max_distance`, or covers all of b), so the result is exact. `own` leaves feature i of a out
    /// of its own neighbours (a is the layer of the index; by index, not by geometry). Neighbours farther than
    /// `max_distance` are cut (`<=` is kept); an empty feature, or none near, gets an empty list.
    pub fn nearest_k(
        &self,
        a: &[Geometry],
        k: usize,
        max_distance: Option<f64>,
        own: bool,
    ) -> Result<Vec<Vec<(usize, f64)>>> {
        let live = self.geoms.iter().filter(|g| !g.is_null()).count();
        let mut bb = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for (g, b) in self.geoms.iter().zip(&self.bounds) {
            if let (false, Some(b)) = (g.is_null(), b) {
                bb = [bb[0].min(b[0]), bb[1].min(b[1]), bb[2].max(b[2]), bb[3].max(b[3])];
            }
        }
        let ha = unsafe { self.read_all(a) }?;
        let mut out: Vec<Vec<(usize, f64)>> = Vec::with_capacity(a.len());
        let result = (|| -> Result<()> {
            for (i, &h) in ha.iter().enumerate() {
                if h.is_null() || live == 0 || k == 0 {
                    out.push(Vec::new());
                    continue;
                }
                let bounds = a[i].bounds().ok_or("an empty geometry")?;
                let itself = own && self.geoms.get(i).is_some_and(|g| !g.is_null());
                let others = live - itself as usize;
                if others == 0 {
                    out.push(Vec::new());
                    continue;
                }
                let span = (bb[2] - bb[0]).hypot(bb[3] - bb[1]);
                let mut d = if span > 0.0 { (span / (live as f64).sqrt()).max(1e-9) } else { 1.0 };
                if let Some(m) = max_distance {
                    d = d.min(m).max(f64::MIN_POSITIVE);
                }
                loop {
                    let found = unsafe { self.candidates(h, bounds, d) }?;
                    let mut ds: Vec<(f64, usize)> = Vec::with_capacity(found.len());
                    for &j in &found {
                        if own && j == i {
                            continue;
                        }
                        ds.push((self.distance_between(h, self.geoms[j])?, j));
                    }
                    ds.sort_unstable_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
                    let all = ds.len() >= others;
                    let capped = max_distance.is_some_and(|m| d >= m);
                    if all || capped || (ds.len() >= k && ds[k - 1].0 <= d) {
                        ds.truncate(k);
                        if let Some(m) = max_distance {
                            ds.retain(|&(dist, _)| dist <= m);
                        }
                        out.push(ds.into_iter().map(|(dist, j)| (j, dist)).collect());
                        break;
                    }
                    d = match max_distance {
                        Some(m) => (d * 2.0).min(m),
                        None => d * 2.0,
                    };
                }
            }
            Ok(())
        })();
        ha.into_iter().filter(|h| !h.is_null()).for_each(|h| unsafe { self.s.destroy(h) });
        result.map(|()| out)
    }
}
