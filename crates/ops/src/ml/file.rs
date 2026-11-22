//! A fitted model in a file, to carry to another model: `ml.write` saves it as JSON (its parameters, its trees and,
//! for a model of `ml.fit`, what the training saw), `ml.read` reads it back. A file saved by an earlier version, with no
//! training summary, reads as it is and gives no novelty.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::Model;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Write));
    r.add(Arc::new(Read));
}

fn path_of(args: &Args) -> Result<std::path::PathBuf> {
    let path: String = args.param("path", String::new())?;
    if path.trim().is_empty() {
        return Err(Error::msg(format!("node {:?}: choose the file", args.node)));
    }
    Ok(std::path::PathBuf::from(path.trim()))
}

/// The model as the text of the file.
pub fn to_text(m: &Model) -> Result<String> {
    serde_json::to_string(m).map_err(|e| Error::msg(format!("the model cannot be written: {e}")))
}

/// A model from the text of a file.
pub fn from_text(text: &str) -> std::result::Result<Model, String> {
    serde_json::from_str(text).map_err(|e| format!("not a fitted model: {e}"))
}

struct Write;

impl Operator for Write {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "ml.write",
            "A fitted model written to a JSON file (ml.fit, ml.pca, ml.kmeans, ml.som, ml.potential): its \
             parameters and trees and, for a model of ml.fit, the summary of what it was trained on (each \
             predictor's range, quantiles, mean and deviation) and the training rows (a sample of 10 000 when there \
             are more), which tell a second scene what lies outside. Read it in another model with ml.read.",
            Access::None,
        )
        .input(PortSpec::new("model", Type::MlModel, "the fitted model"))
        .output(PortSpec::new("path", Type::Text, "the file written"))
        .param(PortSpec::new("path", Type::Text, "the file (.json), relative to the model's folder"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let Some(Value::MlModel(m)) = args.get("model") else {
            return Err(Error::msg(format!("node {:?}: connect the fitted model to write", args.node)));
        };
        let mut path = path_of(args)?;
        if path.extension().is_none() {
            path.set_extension("json");
        }
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| Error::msg(format!("{}: {e}", dir.display())))?;
        }
        std::fs::write(&path, to_text(m)?).map_err(|e| Error::msg(format!("{}: {e}", path.display())))?;
        Ok(outputs([("path", Value::Text(path.to_string_lossy().into_owned()))]))
    }
}

struct Read;

impl Operator for Read {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "ml.read",
            "A fitted model read from the JSON file ml.write made (or a model's JSON written by an output of a \
             model): it classifies another scene's objects (objects.classify) or maps (ml.predict) as it is, \
             without fitting again. A file written before the training summary was kept reads as well, and gives no \
             novelty.",
            Access::None,
        )
        .input(PortSpec::optional(
            "after",
            Type::Any,
            "anything that has to be done first, ml.write's path when the model of this graph writes the file it reads",
        ))
        .output(PortSpec::new("model", Type::MlModel, "the fitted model"))
        .param(PortSpec::new("path", Type::Text, "the file, relative to the model's folder"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let path = path_of(args)?;
        let text = std::fs::read_to_string(&path).map_err(|e| Error::msg(format!("{}: {e}", path.display())))?;
        let m = from_text(&text).map_err(|e| Error::msg(format!("node {:?}: {}: {e}", args.node, path.display())))?;
        Ok(outputs([("model", Value::MlModel(Arc::new(m)))]))
    }
}
