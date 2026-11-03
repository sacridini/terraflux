//! Seeing a value without writing it (`view.plot`): the application shows a plot node in the Viewer when it is
//! clicked, refreshed at every run: a map, vector features, or a table drawn as a chart (bars, lines,
//! points, boxes) as its settings ask; nothing is written (a result one likes is saved from the Viewer, or by a
//! `raster.write`, `vector.write` or `table.write`).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Plot));
}

struct Plot;

/// How a plot compares its map with another (`compare`).
pub const COMPARE: [&str; 3] = ["curtain", "side by side", "difference"];

impl Operator for Plot {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "view.plot",
            "Retired: a figure is a Plot now, kept with the model outside the graph (the Viewer's Plots; set_plot for an agent), and a model's view.plot nodes are made Plots when it is read; it stays for the plots inside groups until 2.0. Show a map, a stack, the maps of a loop, rasters on disk, vector features or a table in the Viewer when the node is clicked, refreshed at every run, without writing anything; save it from the Viewer if it is worth keeping. A table is drawn as a chart: bars, lines, points or boxes (chart), of the columns y along x, a group by by (a chart suggested when none is given; changed in the Viewer too). Plots stack: a plot's shown connected to another's under is drawn beneath it (maps over maps, vectors over them), as many as wanted.",
            Access::None,
        )
        .input(PortSpec::optional("value", Type::Raster, "the map (or maps) to show"))
        .input(PortSpec::optional("features", Type::Vector, "vector features to show (over the map, if any)"))
        .input(PortSpec::optional("table", Type::Table, "a table to show, drawn as a chart"))
        .input(PortSpec::optional("under", Type::Raster, "another plot's shown, drawn beneath this one"))
        .input(PortSpec::optional("compared", Type::Raster, "another map compared with the one shown (another date, a scenario, the observed), as compare says"))
        .output(PortSpec::new("shown", Type::Any, "the same value (the map, else the features, else the table), as shown"))
        .param(PortSpec::optional("title", Type::Text, "the tab's title (default: the node's id)"))
        .param(PortSpec::optional("opacity", Type::Number, "drawn over another plot: how opaque, 0 to 1 (1)"))
        .param(PortSpec::optional("chart", Type::Text, "a table's chart: bars, lines, points or boxes (default: suggested)"))
        .param(PortSpec::optional("x", Type::Text, "a table's chart: the column along the x axis"))
        .param(PortSpec::optional("y", Type::Text, "a table's chart: the columns drawn, by name (comma-separated)"))
        .param(PortSpec::optional("by", Type::Text, "a table's chart: the column whose values make the groups"))
        .param(PortSpec::optional("compare", Type::Text, "with a map compared: curtain (the other right of a line dragged across the map), side by side (default), or difference (the other minus this one; between maps of classes, the transitions); the report shows both maps, and their difference when asked"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        if let Some(how) = args.opt_param::<String>("compare")?
            && !COMPARE.contains(&how.as_str())
        {
            return Err(Error::msg(format!(
                "node {:?}: compare is curtain, side by side or difference, not {how:?}",
                args.node
            )));
        }
        let v = args.get("value").or_else(|| args.get("features")).or_else(|| args.get("table")).ok_or_else(|| {
            Error::msg(format!("node {:?}: connect what to show (a map, features or a table)", args.node))
        })?;
        // the value itself (a raster on disk stays on disk: the tab reads what it shows)
        Ok(outputs([("shown", v.clone())]))
    }
}
