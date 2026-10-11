//! Seeing a value without writing it (`view.plot`): the application opens a map tab of its own for each plot
//! node that runs, and refreshes it at every run; nothing is written (a result one likes is saved from the
//! tab, or by a `raster.write`).

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
            "Show a map, a stack, the maps of a loop or rasters on disk in a map tab of its own, refreshed at every \
             run, without writing anything; save it from the tab if it is worth keeping.",
            Access::None,
        )
        .input(PortSpec::new("value", Type::Raster, "what to show"))
        .output(PortSpec::new("shown", Type::Raster, "the same value, as shown"))
        .param(PortSpec::optional("title", Type::Text, "the tab's title (default: the node's id)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = args.get("value").ok_or_else(|| Error::msg(format!("node {:?}: connect what to show", args.node)))?;
        // the value itself (a raster on disk stays on disk: the tab reads what it shows)
        Ok(outputs([("shown", v.clone())]))
    }
}
