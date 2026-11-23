//! The length of a step on the relief (`move.surface_distance`): the 3D length of each step over a DEM, which
//! neither amt nor move2 have (the library's own, after Signer et al. 2019's remark that planar lengths
//! underestimate the distance walked on a slope).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::vector::Column;

use super::amt::numbers;
use super::{fail, give, is_lonlat, tracks_of};
use crate::{Args, Ctx, Operator, Outputs, Registry, Result};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(SurfaceDistance));
}

/// Whether two coordinate systems are the same one.
fn same_crs(a: &str, b: &str) -> bool {
    a == b || terraflux_gdal::same_crs(a, b).unwrap_or(false)
}

/// `move.surface_distance`.
struct SurfaceDistance;

impl Operator for SurfaceDistance {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.surface_distance",
            "The length of each step measured on the relief, not on the plane (the library's own; not in amt or \
             move2): on a steep slope the planar length sl_ of a step underestimates the distance the animal \
             walks, and with it its speed. Points are taken along the segment every spacing (the last stretch, \
             shorter, ends exactly at the end of the step), plus its start and end; the height of each is the \
             bilinear interpolation between the centres of the four nearest cells (terra's extract with \
             method bilinear); the length is the sum of the 3D chords between consecutive points. Never shorter \
             than the planar length, equal to it on a flat DEM, exactly the planar one times sqrt(1 + slope^2) \
             on a plane. A point in the outer half cell takes the height of the nearest edge cell; outside the \
             raster, or with no data in any of its four cells, the step has no value. The steps and the DEM must \
             be in the same projected coordinate system in metres: heights in metres and distances in degrees do \
             not add.",
            Access::Global,
        )
        .input(PortSpec::new("steps", Type::Vector, "the steps (move.steps) or random steps (move.random_steps)"))
        .input(PortSpec::new("dem", Type::Map, "the heights in metres, north up, in the steps' coordinate system"))
        .output(PortSpec::new("steps", Type::Vector, "the steps with a column of their length on the relief"))
        .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
        .param(PortSpec::optional(
            "spacing",
            Type::Number,
            "the distance between points along a step, in the coordinates' units (default: a quarter of the smaller \
             cell side)",
        ))
        .param(PortSpec::optional("name", Type::Text, "the new column (default sl_surface_)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "steps")?;
        let dem = args.map("dem")?;
        let num = |c: &str| -> Result<Vec<f64>> {
            if v.column(c).is_none() {
                return Err(fail(node, "steps: the steps of move.steps or move.random_steps (missing columns)"));
            }
            numbers(v, c).map_err(|e| fail(node, e))
        };
        let (x1, y1, x2, y2) = (num("x1_")?, num("y1_")?, num("x2_")?, num("y2_")?);
        let g = &dem.grid;
        let dcrs = crate::prepare::crs_of(g);
        let geographic = |c: Option<&str>| c.is_some_and(|c| terraflux_gdal::is_geographic(c).unwrap_or(false));
        let steps_ll = is_lonlat(v.crs.as_deref()).map_err(|e| fail(node, e))?;
        if steps_ll || geographic(dcrs.as_deref()) {
            return Err(fail(
                node,
                "surface_distance needs steps and a DEM in a projected CRS in metres: heights in metres and \
                 distances in degrees do not add (project them with reproject first)",
            ));
        }
        if let (Some(a), Some(b)) = (v.crs.as_deref(), dcrs.as_deref())
            && !same_crs(a, b)
        {
            return Err(fail(node, "steps and dem must be in the same CRS (reproject the DEM or the tracks)"));
        }
        let t = g.transform;
        if t[2] != 0.0 || t[4] != 0.0 {
            return Err(fail(node, "rotated rasters are not supported"));
        }
        if !(t[1] > 0.0 && t[5] < 0.0) {
            return Err(fail(node, "dem: rasters with north up (cells growing eastwards and southwards) are expected"));
        }
        let (dx, dy) = (t[1], -t[5]);
        let spacing = match args.opt_param::<f64>("spacing")? {
            None => 0.25 * dx.min(dy),
            Some(s) if s.is_finite() && s > 0.0 => s,
            Some(_) => return Err(fail(node, "spacing: a positive distance")),
        };
        let name: String = args.param("name", "sl_surface_".to_string())?;
        let got = ffi::move_surface_distance(
            &x1,
            &y1,
            &x2,
            &y2,
            &dem.data,
            g.width as i64,
            g.height as i64,
            t[0],
            t[3],
            dx,
            dy,
            spacing,
            super::threads(),
        )?;
        let mut out = (**v).clone();
        super::set_column(&mut out, &name, Column::Numbers(got));
        Ok(give("steps", out))
    }
}
