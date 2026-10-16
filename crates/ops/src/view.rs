//! Seeing a value without writing it (`view.plot`): the application opens a tab of its own for each plot node
//! that runs, and refreshes it at every run: a map, vector features, or a table drawn as a chart (bars, lines,
//! points, boxes) as its settings ask; nothing is written (a result one likes is saved from the tab, or by a
//! `raster.write`, `vector.write` or `table.write`).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Plot));
}

struct Plot;

impl Operator for Plot {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "view.plot",
            "Show a map, a stack, the maps of a loop, rasters on disk, vector features or a table in a tab of its own, refreshed at every run, without writing anything; save it from the tab if it is worth keeping. A table is drawn as a chart: bars, lines, points or boxes (chart), of the columns y along x, a group by by (a chart suggested when none is given; changed in the tab too). Plots stack: a plot's shown connected to another's under is drawn beneath it (maps over maps, vectors over them), as many as wanted.",
            Access::None,
        )
        .input(PortSpec::optional("value", Type::Raster, "the map (or maps) to show"))
        .input(PortSpec::optional("features", Type::Vector, "vector features to show (over the map, if any)"))
        .input(PortSpec::optional("table", Type::Table, "a table to show, drawn as a chart"))
        .input(PortSpec::optional("under", Type::Raster, "another plot's shown, drawn beneath this one"))
        .output(PortSpec::new("shown", Type::Any, "the same value (the map, else the features, else the table), as shown"))
        .param(PortSpec::optional("title", Type::Text, "the tab's title (default: the node's id)"))
        .param(PortSpec::optional("opacity", Type::Number, "drawn over another plot: how opaque, 0 to 1 (1)"))
        .param(PortSpec::optional("chart", Type::Text, "a table's chart: bars, lines, points or boxes (default: suggested)"))
        .param(PortSpec::optional("x", Type::Text, "a table's chart: the column along the x axis"))
        .param(PortSpec::optional("y", Type::Text, "a table's chart: the columns drawn, by name (comma-separated)"))
        .param(PortSpec::optional("by", Type::Text, "a table's chart: the column whose values make the groups"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = args.get("value").or_else(|| args.get("features")).or_else(|| args.get("table")).ok_or_else(|| {
            Error::msg(format!("node {:?}: connect what to show (a map, features or a table)", args.node))
        })?;
        // the value itself (a raster on disk stays on disk: the tab reads what it shows)
        Ok(outputs([("shown", v.clone())]))
    }
}
