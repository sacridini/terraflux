//! Landscape patterns from landschaft's port of GuidosToolbox: the Morphological Spatial Pattern Analysis
//! (`landscape.mspa`, Soille & Vogt 2009), the same bits as GTB's `mspa` executable.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Mspa));
}

/// The classes of an MSPA map (internal features, inside core-openings, carry 100 more).
pub const MSPA_CLASSES: [(u8, &str); 15] = [
    (0, "Background"),
    (1, "Branch"),
    (3, "Edge"),
    (5, "Perforation"),
    (9, "Islet"),
    (17, "Core"),
    (33, "Bridge"),
    (35, "Bridge in Edge"),
    (37, "Bridge in Perforation"),
    (65, "Loop"),
    (67, "Loop in Edge"),
    (69, "Loop in Perforation"),
    (100, "Core-Opening"),
    (129, "Missing"),
    (220, "Border-Opening"),
];

/// The name of an MSPA class.
pub fn mspa_class(v: f64) -> Option<String> {
    if v.fract() != 0.0 || !(0.0..256.0).contains(&v) {
        return None;
    }
    let v = v as u8;
    if let Some((_, n)) = MSPA_CLASSES.iter().find(|(k, _)| *k == v) {
        return Some(n.to_string());
    }
    let (_, n) =
        MSPA_CLASSES.iter().find(|(k, _)| u16::from(*k) + 100 == u16::from(v) && !matches!(k, 0 | 100 | 129 | 220))?;
    Some(format!("{n} (internal)"))
}

/// GuidosToolbox's colour of an MSPA class (bridges and loops across edges and perforations as transitions).
pub fn mspa_colour(v: u8) -> [u8; 3] {
    let base = |v: u8| -> Option<[u8; 3]> {
        Some(match v {
            0 => [220, 220, 220],
            1 => [255, 140, 0],
            3 => [0, 0, 0],
            5 => [0, 0, 255],
            9 => [160, 60, 0],
            17 => [0, 200, 0],
            33 | 35 | 37 => [255, 0, 0],
            65 | 67 | 69 => [255, 255, 0],
            100 => [136, 136, 136],
            129 => [255, 255, 255],
            220 => [194, 194, 194],
            _ => return None,
        })
    };
    base(v).or_else(|| if v > 100 && !matches!(v, 129 | 220) { base(v - 100) } else { None }).unwrap_or([0, 0, 0])
}

struct Mspa;

impl Operator for Mspa {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "landscape.mspa",
            "Morphological Spatial Pattern Analysis (Soille & Vogt 2009), as GuidosToolbox computes it: the \
             foreground split into core, islet, perforation, edge, loop, bridge and branch (internal features, \
             inside core-openings, 100 more). The foreground is the classes given; without them the map is coded \
             as GTB's: 0 missing, 1 background, 2 foreground. Cells without a value are missing.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the map"))
        .output(PortSpec::new("map", Type::Map, "the MSPA classes"))
        .param(PortSpec::optional("foreground", Type::Any, "the foreground's classes (a class or a list)"))
        .param(PortSpec::optional("connectivity", Type::Integer, "of the foreground: 8 (default) or 4"))
        .param(PortSpec::optional("edge_width", Type::Integer, "the edge's width in cells, 1-255 (default 1)"))
        .param(PortSpec::optional("internal", Type::Bool, "tell internal features apart (default true)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let connectivity: i32 = args.param("connectivity", 8)?;
        let edge_width: i32 = args.param("edge_width", 1)?;
        let internal: bool = args.param("internal", true)?;
        let foreground: Option<Vec<f64>> = match args.params.get("foreground") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::Number(n)) => Some(vec![n.as_f64().unwrap()]),
            Some(v) => Some(
                serde_json::from_value(v.clone())
                    .map_err(|e| Error::msg(format!("node {:?}: foreground: {e}", args.node)))?,
            ),
        };
        let input: Vec<u8> = match &foreground {
            Some(classes) => map
                .data
                .iter()
                .map(|v| {
                    if v.is_nan() {
                        0
                    } else if classes.contains(v) {
                        2
                    } else {
                        1
                    }
                })
                .collect(),
            None => map
                .data
                .iter()
                .map(|v| match v {
                    v if v.is_nan() => Ok(0),
                    v if *v == 0.0 || *v == 1.0 || *v == 2.0 => Ok(*v as u8),
                    v => Err(Error::msg(format!(
                        "node {:?}: the map has a {v}: give the foreground's classes, or code it 0 missing, 1 \
                         background, 2 foreground",
                        args.node
                    ))),
                })
                .collect::<Result<_>>()?,
        };
        let (ny, nx) = map.shape();
        let out = ffi::mspa(&input, ny as i64, nx as i64, connectivity, edge_width, internal)?;
        let data = out.into_iter().map(f64::from).collect();
        Ok(outputs([("map", Value::Map(Map::new(map.grid.clone(), data)))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_colours() {
        assert_eq!(mspa_class(17.0).as_deref(), Some("Core"));
        assert_eq!(mspa_class(117.0).as_deref(), Some("Core (internal)"));
        assert_eq!(mspa_class(229.0), None);
        assert_eq!(mspa_colour(133), [255, 0, 0]);
        assert_eq!(mspa_colour(220), [194, 194, 194]);
    }
}
