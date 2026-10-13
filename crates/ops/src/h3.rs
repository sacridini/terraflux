//! H3 hexagons as planning units (landschaft's h3_units, `prioritize.h3_units`): the hexagons of an H3 resolution
//! that cover a cost map (h3o, a port of Uber's H3), with their cost, for prioritize.optimize as polygons
//! (cost_column "cost").

use std::sync::Arc;

use h3o::Resolution;
use h3o::geom::{ContainmentMode, TilerBuilder};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(H3Units));
}

/// The H3 cells of a resolution that overlap a polygon of longitudes and latitudes (h3's
/// h3shape_to_cells_experimental with contain="overlap"), by index.
pub fn cells_over(ring: &[Xy], resolution: u8) -> Result<Vec<h3o::CellIndex>> {
    let res = Resolution::try_from(resolution).map_err(|_| Error::msg("the resolution is from 0 to 15"))?;
    let line: Vec<geo::Coord<f64>> = ring.iter().map(|p| geo::Coord { x: p[0], y: p[1] }).collect();
    let polygon = geo::Polygon::new(geo::LineString::new(line), Vec::new());
    let mut tiler = TilerBuilder::new(res).containment_mode(ContainmentMode::IntersectsBoundary).build();
    tiler.add(polygon).map_err(|e| Error::msg(format!("H3: {e}")))?;
    let mut cells: Vec<h3o::CellIndex> = tiler.into_coverage().collect();
    cells.sort_unstable();
    cells.dedup();
    Ok(cells)
}

struct H3Units;

impl Operator for H3Units {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.h3_units",
            "H3 hexagons as planning units, as landschaft's h3_units: the hexagons of an H3 resolution that cover \
             a cost map (its footprint in longitude and latitude, every hexagon that overlaps it), in the map's \
             coordinate system, those over cells with a cost, each with the sum of the costs of the cells it covers \
             weighted by the exact share of each cell inside it. For prioritize.optimize as polygons (cost_column \
             cost).",
            Access::Global,
        )
        .input(PortSpec::new("cost", Type::Map, "the cost map (with a coordinate system)"))
        .output(PortSpec::new("units", Type::Vector, "the hexagons: h3 (the cell's index) and cost"))
        .output(PortSpec::new("table", Type::Table, "per hexagon (unit 1..): its cost"))
        .param(PortSpec::new(
            "resolution",
            Type::Integer,
            "H3's resolution, 0 to 15 (7 about 5 km², 8 about 0.7 km², 9 about 0.1 km²)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: String| Error::msg(format!("node {:?}: {e}", args.node));
        let cost = args.map("cost")?;
        let resolution: i64 = args.param("resolution", -1)?;
        if !(0..=15).contains(&resolution) {
            return Err(fail("resolution: from 0 to 15".into()));
        }
        let grid = &cost.grid;
        let crs = crate::prepare::crs_of(grid).ok_or_else(|| fail("the cost map has no coordinate system".into()))?;
        let t = grid.transform;
        let (ny, nx) = (grid.height as f64, grid.width as f64);
        // the map's footprint, its edges cut in 64, in longitude and latitude
        let k: Vec<f64> = (0..=64).map(|i| f64::from(i) / 64.0).collect();
        let cols: Vec<f64> = k
            .iter()
            .map(|k| k * nx)
            .chain(k.iter().map(|_| nx))
            .chain(k.iter().map(|k| (1.0 - k) * nx))
            .chain(k.iter().map(|_| 0.0))
            .collect();
        let rows: Vec<f64> = k
            .iter()
            .map(|_| 0.0)
            .chain(k.iter().map(|k| k * ny))
            .chain(k.iter().map(|_| ny))
            .chain(k.iter().map(|k| (1.0 - k) * ny))
            .collect();
        let edge: Vec<Xy> =
            cols.iter().zip(&rows).map(|(c, r)| [t[0] + c * t[1] + r * t[2], t[3] + c * t[4] + r * t[5]]).collect();
        let footprint = Vector { geometries: vec![Geometry::MultiPoint(edge)], attributes: Vec::new(), crs: Some(crs) };
        let lonlat = terraflux_gdal::reproject_vector(&footprint, "EPSG:4326").map_err(fail)?;
        let Geometry::MultiPoint(ring) = &lonlat.geometries[0] else { return Err(fail("the footprint".into())) };
        let cells = cells_over(ring, resolution as u8)?;
        if cells.is_empty() {
            return Err(fail("no H3 cell covers the map".into()));
        }
        // the hexagons, back in the map's coordinate system
        let hexagons = Vector {
            geometries: cells
                .iter()
                .map(|c| {
                    let mut ring: Vec<Xy> = c.boundary().iter().map(|v| [v.lng(), v.lat()]).collect();
                    ring.push(ring[0]);
                    Geometry::Polygon(vec![ring])
                })
                .collect(),
            attributes: Vec::new(),
            crs: Some("EPSG:4326".into()),
        };
        let hexagons = crate::vector::on_grid(&Arc::new(hexagons), grid)?;
        let valid: Vec<f64> = cost.data.iter().map(|v| if v.is_finite() { 1.0 } else { 0.0 }).collect();
        let geoms: Vec<&Geometry> = hexagons.geometries.iter().collect();
        let sums = crate::vector::exact_sums(grid, &[cost.data.clone(), Arc::new(valid)], &geoms)?;
        let keep: Vec<usize> = (0..cells.len()).filter(|&i| sums[2 * i + 1] > 0.0).collect();
        let costs: Vec<f64> = keep.iter().map(|&i| sums[2 * i]).collect();
        let units = Vector {
            geometries: keep.iter().map(|&i| hexagons.geometries[i].clone()).collect(),
            attributes: vec![
                ("h3".into(), Column::Texts(keep.iter().map(|&i| cells[i].to_string()).collect())),
                ("cost".into(), Column::Numbers(costs.clone())),
            ],
            crs: hexagons.crs.clone(),
        };
        let table = Table {
            columns: vec!["unit".into(), "cost".into()],
            rows: costs.iter().enumerate().map(|(i, c)| vec![(i + 1) as f64, *c]).collect(),
        };
        Ok(outputs([("units", Value::Vector(Arc::new(units))), ("table", Value::Table(table))]))
    }
}
