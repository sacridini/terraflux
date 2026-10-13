//! Legends: the names and colours of a map's classes, given in the model (`raster.legend`) and carried with the
//! map in its grid's metadata (`terraflux_legend`), so that the maps made from it with the same classes (a
//! simulation's) and the GeoTIFF it is written to keep them; what changes the classes (a reclassification)
//! leaves them behind.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Value, outputs};

/// The metadata item a legend is kept in.
pub const ITEM: &str = "terraflux_legend";

pub fn register(r: &mut Registry) {
    r.add(Arc::new(LegendOp));
}

/// A class of a legend: its value, name and colour (#rrggbb).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Class {
    pub value: f64,
    pub name: String,
    pub colour: String,
}

/// The classes of a legend from a setting: [[value, "name", "#rrggbb"], ...] (the name and colour may be left
/// out or empty).
pub fn parse(v: &serde_json::Value) -> std::result::Result<Vec<Class>, String> {
    let rows = v.as_array().ok_or("a legend is a list of [value, name, colour]")?;
    rows.iter()
        .map(|r| {
            let r = r.as_array().ok_or("each class is [value, name, colour]")?;
            let value = r.first().and_then(|x| x.as_f64()).ok_or("a class's value is a number")?;
            let text = |k: usize| r.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let colour = text(2);
            if !colour.is_empty() && rgb(&colour).is_none() {
                return Err(format!("the colour of class {value}: #rrggbb, not {colour:?}"));
            }
            Ok(Class { value, name: text(1), colour })
        })
        .collect()
}

/// A colour #rrggbb as its red, green and blue.
pub fn rgb(text: &str) -> Option<[u8; 3]> {
    let h = text.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let c = |k: usize| u8::from_str_radix(&h[k..k + 2], 16).ok();
    Some([c(0)?, c(2)?, c(4)?])
}

/// The legend a grid carries, if any.
pub fn of_grid(g: &Grid) -> Option<Vec<Class>> {
    let text = g.geo.item(ITEM)?;
    serde_json::from_str(text).ok()
}

/// A grid carrying a legend (none: without).
pub fn with_legend(g: &Grid, classes: Option<&[Class]>) -> Grid {
    let mut g = g.clone();
    let text = classes.map(|c| serde_json::to_string(c).unwrap_or_default());
    g.geo.set_item(ITEM, text.as_deref());
    g
}

struct LegendOp;

impl Operator for LegendOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.legend",
            "The names and colours of a map's classes: the map shown with them, and carried with it (the maps made \
             from it that keep its classes, the GeoTIFF it is written to); the viewer's legend editor writes them \
             here. Rasters on disk are shown so, not read.",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Raster, "the map of classes"))
        .output(PortSpec::new("map", Type::Raster, "the same map, with its legend"))
        .param(PortSpec::new("classes", Type::Any, "[[value, \"name\", \"#rrggbb\"], ...]"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let classes = parse(args.params.get("classes").unwrap_or(&serde_json::Value::Null))
            .map_err(|e| Error::msg(format!("node {:?}: classes: {e}", args.node)))?;
        let out = match args.get("map") {
            Some(Value::Map(m)) => {
                Value::Map(Map { grid: Arc::new(with_legend(&m.grid, Some(&classes))), data: m.data.clone() })
            }
            // rasters on disk as they are: the viewer takes the legend from the node
            Some(Value::Raster(r)) => Value::Raster(r.clone()),
            Some(Value::Stack(s)) => {
                let mut s = s.clone();
                s.grid = Arc::new(with_legend(&s.grid, Some(&classes)));
                Value::Stack(s)
            }
            Some(v) => return Err(Error::msg(format!("node {:?}: map: a map, not a {:?}", args.node, v.ty()))),
            None => return Err(Error::msg(format!("node {:?}: connect a map", args.node))),
        };
        Ok(outputs([("map", out)]))
    }
}
