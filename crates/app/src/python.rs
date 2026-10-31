//! The model as a Python script of landschaft (the library whose kernels Terraflux runs): for the paper, the
//! supervisor or the reviewer who asks for the code. Each node becomes a call of landschaft's function of the same
//! method, in the order the model runs them: its inputs as the values of the nodes before, its settings as the
//! function's arguments (by their names in landschaft), its outputs as the result or its fields. A calculator's
//! script is kept as it is, run in numpy by a helper written into the script; reclassification rules become
//! `ls.reclassify`'s table; the foreground of the habitat patterns (MSPA, FAD...) `ls.reclassify(foreground=...)`.
//! A node with no counterpart known stays as a comment with its settings, the script saying which.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde_json::Value;
use terraflux_graph::{Input, Model, Node, Ref, Source};
use terraflux_ops::Registry;

/// How a node's call is made.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// The arguments as they are.
    Plain,
    /// The map through `ls.reclassify(map, foreground=...)` when the node gives the foreground's classes.
    Habitat,
    /// A neutral landscape: its size from `like` (columns, rows), else the settings or 256.
    Grid,
}

/// An operator's counterpart in landschaft.
struct Py {
    op: &'static str,
    call: &'static str,
    /// The node's inputs and settings, by their names here, and the function's argument (`""`: the first, by
    /// position).
    args: &'static [(&'static str, &'static str)],
    /// Arguments given when the node does not give them (Terraflux's defaults where landschaft's differ).
    defaults: &'static [(&'static str, &'static str)],
    /// The outputs: the port and what of the result it is (`""`: the result, `.field`).
    outs: &'static [(&'static str, &'static str)],
    kind: Kind,
}

const GRID: &[(&str, &str)] = &[("rows", "nrow"), ("columns", "ncol"), ("resolution", "resolution")];

macro_rules! nlm {
    ($op:literal, $call:literal, [$($a:literal => $b:literal),*], defaults: [$($k:literal = $v:literal),*]) => {
        Py {
            op: $op,
            call: $call,
            args: &[("rows", "nrow"), ("columns", "ncol"), ("resolution", "resolution"), $(($a, $b)),*],
            defaults: &[$(($k, $v)),*],
            outs: &[("map", "")],
            kind: Kind::Grid,
        }
    };
}

const MAPPED: &[Py] = &[
    Py {
        op: "raster.read",
        call: "ls.load_raster",
        args: &[("path", "")],
        defaults: &[],
        outs: &[("raster", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "raster.write",
        call: "ls.write_raster",
        args: &[("value", ""), ("path", "path"), ("type", "dtype")],
        defaults: &[],
        outs: &[("path", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "vector.read",
        call: "ls.load_vector",
        args: &[("path", ""), ("layer", "layer")],
        defaults: &[],
        outs: &[("vector", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "vector.write",
        call: "ls.write_vector",
        args: &[("vector", ""), ("path", "path"), ("layer", "layer")],
        defaults: &[],
        outs: &[("path", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "table.read",
        call: "ls.load_table",
        args: &[("path", "")],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "table.write",
        call: "ls.write_table",
        args: &[("table", ""), ("path", "path")],
        defaults: &[],
        outs: &[("path", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "landscape.mspa",
        call: "ls.mspa",
        args: &[("map", ""), ("connectivity", "connectivity"), ("edge_width", "edge_width"), ("internal", "internal")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Habitat,
    },
    Py {
        op: "landscape.spa",
        call: "ls.spa",
        args: &[("map", ""), ("variant", "variant")],
        defaults: &[],
        outs: &[("map", ".map"), ("stats", ".stats"), ("porosity", ".porosity")],
        kind: Kind::Habitat,
    },
    Py {
        op: "landscape.accounting",
        call: "ls.accounting",
        args: &[
            ("map", ""),
            ("thresholds", "thresholds"),
            ("connectivity", "connectivity"),
            ("highlight_largest", "highlight_largest"),
            ("pixel_resolution", "pixel_resolution"),
        ],
        defaults: &[],
        outs: &[("map", ".classes"), ("ids", ".ids"), ("area", ".area"), ("stats", ".stats")],
        kind: Kind::Habitat,
    },
    Py {
        op: "fragmentation.fad",
        call: "ls.fad",
        args: &[("map", ""), ("window", "window_size")],
        defaults: &[],
        outs: &[("map", ".density"), ("classes", ".classes"), ("stats", ".stats")],
        kind: Kind::Habitat,
    },
    Py {
        op: "fragmentation.fad_multiscale",
        call: "ls.fad_multiscale",
        args: &[("map", ""), ("windows", "window_sizes")],
        defaults: &[],
        outs: &[("map", ".multiscale_map"), ("classes", ".classes"), ("stats", ".stats")],
        kind: Kind::Habitat,
    },
    Py {
        op: "fragmentation.fos",
        call: "ls.fos",
        args: &[
            ("map", ""),
            ("window", "window_size"),
            ("method", "method"),
            ("classes", "classes"),
            ("connectivity", "connectivity"),
            ("grayscale", "grayscale"),
            ("gray_threshold", "gray_threshold"),
            ("pixel_resolution", "pixel_resolution"),
        ],
        defaults: &[],
        outs: &[("map", ".map"), ("stats", ".stats")],
        kind: Kind::Habitat,
    },
    nlm!("nlm.random", "ls.nlm.random", ["seed" => "seed", "rescale" => "rescale"], defaults: ["seed" = "1"]),
    nlm!("nlm.gaussian_field", "ls.nlm.gaussian_field",
         ["range" => "autocorr_range", "variance" => "mag_var", "nugget" => "nug", "seed" => "seed"], defaults: ["seed" = "1"]),
    nlm!("nlm.fbm", "ls.nlm.fbm", ["dimension" => "fract_dim", "seed" => "seed"], defaults: ["seed" = "1"]),
    nlm!("nlm.random_cluster", "ls.nlm.random_cluster",
         ["p" => "p", "weights" => "ai", "neighbourhood" => "neighbourhood", "seed" => "seed"],
         defaults: ["seed" = "1", "rescale" = "False"]),
    nlm!("nlm.distance_gradient", "ls.nlm.distance_gradient", ["origin" => "origin", "nlmr_compat" => "nlmr_compat", "rescale" => "rescale"],
         defaults: []),
    nlm!("nlm.planar_gradient", "ls.nlm.planar_gradient", ["direction" => "direction", "seed" => "seed", "rescale" => "rescale"], defaults: ["seed" = "1"]),
    nlm!("nlm.edge_gradient", "ls.nlm.edge_gradient", ["direction" => "direction", "seed" => "seed", "rescale" => "rescale"], defaults: ["seed" = "1"]),
    nlm!("nlm.mpd", "ls.nlm.mpd",
         ["roughness" => "roughness", "deviation" => "rand_dev", "torus" => "torus", "nlmr_compat" => "nlmr_compat", "seed" => "seed", "rescale" => "rescale"],
         defaults: ["seed" = "1"]),
    nlm!("nlm.mosaic_tess", "ls.nlm.mosaic_tess", ["germs" => "germs", "nlmr_compat" => "nlmr_compat", "seed" => "seed", "rescale" => "rescale"], defaults: ["seed" = "1"]),
    nlm!("nlm.mosaic_gibbs", "ls.nlm.mosaic_gibbs",
         ["germs" => "germs", "r" => "R", "patch_classes" => "patch_classes", "nlmr_compat" => "nlmr_compat", "seed" => "seed", "rescale" => "rescale"],
         defaults: ["seed" = "1"]),
    nlm!("nlm.percolation", "ls.nlm.percolation", ["prob" => "prob", "seed" => "seed"], defaults: ["seed" = "1"]),
    nlm!("nlm.random_rectangular_cluster", "ls.nlm.random_rectangular_cluster", ["minl" => "minl", "maxl" => "maxl", "seed" => "seed", "rescale" => "rescale"],
         defaults: ["seed" = "1"]),
    nlm!("nlm.neigh", "ls.nlm.neigh",
         ["p_neigh" => "p_neigh", "p_empty" => "p_empty", "categories" => "categories",
          "neighbourhood" => "neighbourhood", "proportions" => "proportions", "seed" => "seed", "rescale" => "rescale"], defaults: ["seed" = "1"]),
    Py {
        op: "nlm.curds",
        call: "ls.nlm.curds",
        args: &[
            ("curds", "curds"),
            ("steps", "recursion_steps"),
            ("wheyes", "wheyes"),
            ("resolution", "resolution"),
            ("seed", "seed"),
        ],
        defaults: &[("seed", "1")],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "nlm.classify",
        call: "ls.nlm.classify",
        args: &[("map", ""), ("n", "n"), ("weights", "weighting"), ("real", "real_land"), ("mask_value", "mask_val")],
        defaults: &[],
        outs: &[("classes", "")],
        kind: Kind::Plain,
    },
];

/// Operators whose inputs and settings are the arguments of landschaft's function of the same name, by the same
/// names, and that give one output: called with them as they are.
const BY_NAME: &[(&str, &str)] = &[
    ("conn.eca_change_grid", "ls.conn.eca_change_grid"),
    ("conn.make_grid", "ls.conn.make_grid"),
    ("hydro.breach", "ls.hydro.breach"),
    ("hydro.centrality", "ls.hydro.centrality"),
    ("hydro.connectivity_index", "ls.hydro.connectivity_index"),
    ("hydro.distance_to_channel", "ls.hydro.distance_to_channel"),
    ("hydro.fill", "ls.hydro.fill"),
    ("hydro.flow_accumulation", "ls.hydro.flow_accumulation"),
    ("hydro.flow_direction", "ls.hydro.flow_direction"),
    ("hydro.hand", "ls.hydro.hand"),
    ("hydro.network_distance", "ls.hydro.network_distance"),
    ("hydro.spi", "ls.hydro.spi"),
    ("hydro.sti", "ls.hydro.sti"),
    ("hydro.streams", "ls.hydro.streams"),
    ("hydro.twi", "ls.hydro.twi"),
    ("hydro.watersheds", "ls.hydro.watersheds"),
    ("nlm.merge", "ls.nlm.merge"),
    ("prioritize.decay_connectivity", "ls.prioritize.decay_connectivity"),
    ("prioritize.protected_cells", "ls.prioritize.protected_cells"),
    ("prioritize.refine", "ls.prioritize.refine"),
    ("redd.nrt", "ls.redd.nrt"),
    ("sdm.exdet", "ls.sdm.exdet"),
    ("sdm.mess", "ls.sdm.mess"),
    ("sdm.mop", "ls.sdm.mop"),
    ("sdm.reference_data", "ls.sdm.reference_data"),
    ("sdm.vif", "ls.sdm.vif"),
    ("sensitivity.design", "ls.sensitivity.design"),
    ("stats.idw", "ls.stats.idw"),
    ("stats.moran", "ls.stats.moran"),
    ("surface.gravity", "ls.surface.gravity"),
    ("surface.hli", "ls.surface.hli"),
    ("uncertainty.product_sd", "ls.uncertainty.product_sd"),
    ("uncertainty.sum_sd", "ls.uncertainty.sum_sd"),
];

/// The helper that runs a calculator's script in numpy (written into the script when a calculator is in it).
pub const CALC_HELPER: &str = include_str!("python_calc.py");

/// A model as a Python script.
#[derive(Clone, Debug, Default)]
pub struct Script {
    pub text: String,
    /// The nodes translated, and those left as comments (their id, operator and why).
    pub translated: usize,
    pub left: Vec<(String, String, String)>,
}

/// A value of JSON as Python writes it.
pub fn literal(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => text(s),
        Value::Array(a) => format!("[{}]", a.iter().map(literal).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => {
            format!(
                "{{{}}}",
                o.iter().map(|(k, v)| format!("{}: {}", text(k), literal(v))).collect::<Vec<_>>().join(", ")
            )
        }
    }
}

/// A text as a Python string.
fn text(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

const RESERVED: &[&str] = &[
    "False",
    "None",
    "True",
    "and",
    "as",
    "assert",
    "async",
    "await",
    "break",
    "class",
    "continue",
    "def",
    "del",
    "elif",
    "else",
    "except",
    "finally",
    "for",
    "from",
    "global",
    "if",
    "import",
    "in",
    "is",
    "lambda",
    "nonlocal",
    "not",
    "or",
    "pass",
    "raise",
    "return",
    "try",
    "while",
    "with",
    "yield",
    "ls",
    "np",
    "Path",
    "MODEL_DIR",
    "OUT",
    "map",
    "print",
    "type",
    "input",
    "len",
    "list",
    "dict",
    "set",
    "min",
    "max",
    "sum",
    "abs",
    "round",
    "range",
];

/// A name of the model (a node's id, an input's) as a Python variable.
pub fn variable(id: &str) -> String {
    let mut v: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if v.is_empty() || v.starts_with(|c: char| c.is_ascii_digit()) {
        v.insert_str(0, "n_");
    }
    if RESERVED.contains(&v.as_str()) || v.starts_with("_") {
        v.push('_');
    }
    v
}

/// A path of the model (relative to its folder) as Python writes it.
fn path(p: &str) -> String {
    let unix = p.replace('\\', "/");
    if Path::new(p).is_absolute() || unix.starts_with('/') || unix.contains("://") || unix.starts_with("/vsi") {
        format!("Path({})", text(&unix))
    } else {
        format!("MODEL_DIR / {}", text(&unix))
    }
}

/// Reclassification rules (`1 2 -> 1`, `0..10 -> 2`, `0 10 2`, `* -> 0`, `5 -> nan`) as `ls.reclassify`'s table
/// (ranges `[low, high)`: both ends in, the high end just past its value) and `others`.
pub fn reclassify_table(rules: &str) -> Result<(String, Option<String>), String> {
    let mut entries = Vec::new();
    let mut others = None;
    let num = |s: &str| -> Result<String, String> {
        let s = s.trim();
        match s.to_lowercase().as_str() {
            "nan" | "none" | "nodata" => Ok("np.nan".into()),
            _ => s.parse::<f64>().map(|_| s.to_string()).map_err(|_| format!("{s:?} is not a number")),
        }
    };
    for line in rules.split(['\n', ';']).map(str::trim).filter(|l| !l.is_empty()) {
        let fields: Vec<&str> = line.split([' ', ',', '\t']).map(str::trim).filter(|f| !f.is_empty()).collect();
        let (from, to) = match line.split_once("->") {
            Some((f, t)) => (f.to_string(), t.to_string()),
            None if fields.len() == 3 => (format!("{}..{}", fields[0], fields[1]), fields[2].to_string()),
            None => return Err(format!("{line:?}: not a rule")),
        };
        let to = num(&to)?;
        for f in from.split([' ', ',']).map(str::trim).filter(|f| !f.is_empty()) {
            if f == "*" {
                others.get_or_insert(to.clone());
            } else if let Some((a, b)) = f.split_once("..") {
                entries.push(format!("({}, np.nextafter({}, np.inf)): {to}", num(a)?, num(b)?));
            } else {
                entries.push(format!("{}: {to}", num(f)?));
            }
        }
    }
    Ok((format!("{{{}}}", entries.join(", ")), others))
}

/// What a node is in the script: the expression of each of its outputs.
struct Made {
    outs: HashMap<String, String>,
}

/// The model as a Python script of landschaft (its top level; containers stay as comments).
pub fn script(m: &Model, r: &Registry, model_path: Option<&Path>) -> Script {
    let mut out = Script::default();
    let mut body = String::new();
    let mut made: HashMap<String, Made> = HashMap::new();
    let mut inputs: BTreeMap<String, String> = BTreeMap::new();
    let mut calc = false;
    let ids = terraflux_graph::ids(&m.nodes);
    let order = terraflux_graph::order(&m.nodes, |n| terraflux_graph::dependencies_among(n, &ids))
        .unwrap_or_else(|_| m.nodes.iter().collect());
    // the model's inputs
    for (name, input) in &m.inputs {
        let v = variable(name);
        let line = match input {
            Input::Map { path: p, band, nodata, .. } => {
                let mut a = vec![path(p)];
                match band.as_deref().map(|b| b.parse::<usize>()) {
                    Some(Ok(b)) => a.push(format!("band={}", b + 1)),
                    Some(Err(_)) => {
                        a.push(format!("band=1  # the band named {:?}: its number here", band.as_deref().unwrap_or("")))
                    }
                    None => {}
                }
                let n = nodata.map_or(String::new(), |d| format!("  # no data: {d}"));
                format!("{v} = ls.load_raster({}){n}", a.join(", "))
            }
            Input::Stack { path: p, .. } => format!("{v} = ls.load_raster({})", path(p)),
            Input::Value { value, .. } => format!("{v} = {}", literal(value)),
        };
        if let Some(n) = input.note() {
            body.push_str(&format!("# {}\n", one_line(n)));
        }
        body.push_str(&line);
        body.push('\n');
        inputs.insert(name.clone(), v);
    }
    if !m.inputs.is_empty() {
        body.push('\n');
    }
    for n in order {
        let mut block = String::new();
        block.push_str(&format!("# {} ({})", n.id, n.op));
        if let Some(note) = n.note.as_deref().filter(|t| !t.trim().is_empty()) {
            block.push_str(&format!(": {}", one_line(note)));
        }
        block.push('\n');
        if let Some(why) = n.why.as_deref().filter(|t| !t.trim().is_empty()) {
            block.push_str(&format!("# why: {}\n", one_line(why)));
        }
        let var = variable(&n.id);
        match node(n, r, &var, &made, &inputs) {
            Ok((code, outs, used_calc)) => {
                calc |= used_calc;
                out.translated += 1;
                block.push_str(&code);
                made.insert(n.id.clone(), Made { outs });
            }
            Err(why) => {
                out.left.push((n.id.clone(), n.op.clone(), why.clone()));
                block.push_str(&format!("# not translated: {why}\n"));
                for (k, v) in &n.params {
                    block.push_str(&format!("#   {k} = {}\n", literal(v)));
                }
                for (k, s) in &n.inputs {
                    let s = match s {
                        Source::Ref(x) => x.clone(),
                        Source::Literal(v) => literal(v),
                    };
                    block.push_str(&format!("#   input {k} <- {s}\n"));
                }
                block.push_str(&format!("{var} = None\n"));
                made.insert(n.id.clone(), Made { outs: HashMap::new() });
            }
        }
        body.push_str(&block);
        body.push('\n');
    }
    // the model's outputs
    if !m.outputs.is_empty() {
        body.push_str("# the model's outputs\nOUT.mkdir(parents=True, exist_ok=True)\n");
        for (name, o) in &m.outputs {
            let file = text(&o.path.clone().unwrap_or_else(|| name.clone()));
            body.push_str(&match reference(&o.from, &made, &inputs) {
                Ok(value) => format!("ls.save({value}, OUT / {file})\n"),
                Err(e) => format!("# ls.save(..., OUT / {file})  # {e}\n"),
            });
        }
    }
    // the header
    let title = m
        .name
        .clone()
        .filter(|t| !t.trim().is_empty())
        .or_else(|| model_path.and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "a model".into());
    // (a raw docstring: the texts' backslashes as they are)
    let clean = |t: &str| t.replace("\"\"\"", "'''").trim_end_matches('\\').to_string();
    let mut head = format!("r\"\"\"{} as a Python script of landschaft.\n\n", clean(&one_line(&title)));
    for t in [&m.description, &m.request].into_iter().flatten().filter(|t| !t.trim().is_empty()) {
        head.push_str(&wrap(&clean(t), 100));
        head.push_str("\n\n");
    }
    head.push_str(&wrap(
        &format!(
            "Written by Terraflux {} from the model{}: each node a call of landschaft's function of the same \
             method, in the order the model runs them ({} of {} nodes translated).",
            env!("CARGO_PKG_VERSION"),
            model_path.map_or(String::new(), |p| format!(" {}", p.to_string_lossy().replace('\\', "/"))),
            out.translated,
            out.translated + out.left.len()
        ),
        100,
    ));
    head.push('\n');
    if !out.left.is_empty() {
        head.push_str("Left as comments (to write by hand):\n");
        for (id, op, why) in &out.left {
            head.push_str(&format!("  - {id} ({op}): {why}\n"));
        }
    }
    head.push_str("\"\"\"\n\n");
    head.push_str("from pathlib import Path\n\nimport numpy as np\n\nimport landschaft as ls\n\n");
    let dir = model_path.and_then(|p| p.parent()).filter(|d| !d.as_os_str().is_empty());
    head.push_str(&match dir {
        Some(d) => format!(
            "MODEL_DIR = Path({})  # the model's folder: its paths are relative to it\n",
            text(&d.to_string_lossy().replace('\\', "/"))
        ),
        None => "MODEL_DIR = Path(__file__).parent  # the model's paths are relative to its folder\n".into(),
    });
    head.push_str("OUT = MODEL_DIR / \"output\"\n\n");
    if calc {
        head.push_str("\n# --- Terraflux's calculator (raster.calc) in numpy ---\n\n");
        head.push_str(CALC_HELPER.trim_end());
        head.push_str("\n\n# --- the model ---\n\n");
    }
    out.text = format!("{head}\n{}", body.trim_end()) + "\n";
    out
}

fn one_line(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A text in lines of at most `width` characters.
fn wrap(t: &str, width: usize) -> String {
    let mut lines = Vec::new();
    let mut line = String::new();
    for w in t.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + w.len() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(w);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines.join("\n")
}

/// The expression of a reference: `$input`, `node.port`.
fn reference(r: &str, made: &HashMap<String, Made>, inputs: &BTreeMap<String, String>) -> Result<String, String> {
    match Ref::parse(r).map_err(|e| e.to_string())? {
        Ref::Input(name) => inputs.get(&name).cloned().ok_or_else(|| format!("${name}: not an input of the model")),
        Ref::Carried(name) => Err(format!("@{name}: a value of a container")),
        Ref::Port { node, port } => {
            let m = made.get(&node).ok_or_else(|| format!("{node}: not a node before"))?;
            m.outs.get(&port).cloned().ok_or_else(|| format!("{node}.{port}: not translated"))
        }
    }
}

/// The value of a node's input or setting.
fn value_of(
    n: &Node,
    name: &str,
    made: &HashMap<String, Made>,
    inputs: &BTreeMap<String, String>,
) -> Result<Option<String>, String> {
    if let Some(s) = n.inputs.get(name) {
        return match s {
            Source::Ref(x) => reference(x, made, inputs).map(Some),
            Source::Literal(v) => Ok(Some(literal(v))),
        };
    }
    match n.params.get(name) {
        None => Ok(None),
        Some(v) => match terraflux_graph::setting_ref(v) {
            Some(x) => reference(x, made, inputs).map(Some),
            None if name == "path" => Ok(Some(v.as_str().map_or_else(|| literal(v), path))),
            None => Ok(Some(literal(v))),
        },
    }
}

/// Terraflux's cell types as numpy's.
fn dtype(t: &str) -> &str {
    match t {
        "u8" => "uint8",
        "i8" => "int8",
        "u16" => "uint16",
        "i16" => "int16",
        "u32" => "uint32",
        "i32" => "int32",
        "u64" => "uint64",
        "i64" => "int64",
        "f32" => "float32",
        "f64" => "float64",
        t => t,
    }
}

type Code = (String, HashMap<String, String>, bool);

/// A node's code, and the expression of each output; or why it is not translated.
fn node(
    n: &Node,
    r: &Registry,
    var: &str,
    made: &HashMap<String, Made>,
    inputs: &BTreeMap<String, String>,
) -> Result<Code, String> {
    if n.body.is_some() {
        return Err("a container (its body is not translated)".into());
    }
    if n.op.starts_with("view.") {
        // a chart: what it draws, by matplotlib (left as a comment: a window would stop the script)
        let shown = ["value", "table", "features"].iter().find_map(|k| value_of(n, k, made, inputs).ok().flatten());
        let code = match shown {
            Some(v) => format!("# ls.plot({v})  # the chart (matplotlib)\n"),
            None => "# (a chart of a value not translated)\n".to_string(),
        };
        return Ok((code, HashMap::new(), false));
    }
    let ports: Vec<String> = crate::editor::output_ports(n, r).into_iter().map(|(p, _)| p).collect();
    match n.op.as_str() {
        "raster.calc" => {
            let script = n.params.get("expression").and_then(Value::as_str).ok_or("no script")?;
            let mut args = Vec::new();
            for k in n.inputs.keys() {
                let v = value_of(n, k, made, inputs)?.unwrap_or_else(|| "None".into());
                args.push(format!("{}={v}", k));
            }
            let quoted = if script.contains("\"\"\"") || script.contains('\\') {
                text(script)
            } else {
                format!("\"\"\"\n{}\n\"\"\"", script.trim_end())
            };
            let code = format!("{var} = _calc(\n    {quoted},\n    {},\n)\n", args.join(", "));
            // its outputs: the ports named by `out`, else `map`
            let outs = ports.iter().map(|p| (p.clone(), format!("{var}[{}]", text(p)))).collect();
            return Ok((code, outs, true));
        }
        "raster.reclassify" => {
            let rules = n.params.get("rules").and_then(Value::as_str).ok_or("no rules")?;
            let (table, others) = reclassify_table(rules)?;
            let map = value_of(n, "map", made, inputs)?.ok_or("its map is not connected")?;
            let others = others.map_or(String::new(), |o| format!(", others={o}"));
            let code = format!("{var} = ls.reclassify({map}, {table}{others})\n");
            return Ok((code, HashMap::from([("map".to_string(), var.to_string())]), false));
        }
        _ => {}
    }
    if let Some(py) = MAPPED.iter().find(|p| p.op == n.op) {
        let mut args: Vec<String> = Vec::new();
        let mut given: Vec<&str> = Vec::new();
        let mut skipped: Vec<String> = Vec::new();
        // a neutral landscape's size first: its `like`'s, else the settings', else 256
        if py.kind == Kind::Grid {
            let like = value_of(n, "like", made, inputs)?;
            for (ours, theirs, dim) in [("columns", "ncol", "x"), ("rows", "nrow", "y")] {
                let v = match &like {
                    Some(l) => format!("{l}.sizes[\"{dim}\"]"),
                    None => value_of(n, ours, made, inputs)?.unwrap_or_else(|| "256".into()),
                };
                args.push(format!("{theirs}={v}"));
                given.push(theirs);
            }
        }
        for (ours, theirs) in py.args {
            if given.contains(theirs) {
                continue;
            }
            let Some(mut v) = value_of(n, ours, made, inputs)? else { continue };
            if py.kind == Kind::Habitat && *ours == "map" {
                v = match value_of(n, "foreground", made, inputs)? {
                    Some(fg) => format!("ls.reclassify({v}, foreground={fg})"),
                    // coded as GuidosToolbox's (0 missing, 1 background, 2 foreground): as bytes
                    None => format!("{v}.fillna(0).astype(\"uint8\")"),
                };
            }
            if n.op == "raster.write" && *ours == "type" {
                v = text(dtype(n.params.get("type").and_then(Value::as_str).unwrap_or("")));
            }
            if n.op == "raster.read"
                && *ours == "path"
                && let Some(b) = n.params.get("band")
            {
                match b.as_u64().or_else(|| b.as_str().and_then(|s| s.parse().ok())) {
                    Some(k) => v = format!("{v}, band={}", k + 1),
                    None => skipped.push(format!("band = {} (by name)", literal(b))),
                }
            }
            args.push(if theirs.is_empty() { v } else { format!("{theirs}={v}") });
            given.push(theirs);
        }
        for (k, v) in py.defaults {
            if !given.contains(k) {
                args.push(format!("{k}={v}"));
            }
        }
        // the settings not passed
        let known: Vec<&str> = py.args.iter().map(|(a, _)| *a).chain(["foreground", "like", "band"]).collect();
        for (k, v) in &n.params {
            if !known.contains(&k.as_str()) && !(py.kind == Kind::Grid && GRID.iter().any(|(g, _)| g == k)) {
                skipped.push(format!("{k} = {}", literal(v)));
            }
        }
        let mut code = String::new();
        for s in &skipped {
            code.push_str(&format!("# (not passed: {s})\n"));
        }
        code.push_str(&format!("{var} = {}({})\n", py.call, args.join(", ")));
        let mut outs: HashMap<String, String> = HashMap::new();
        for (port, field) in py.outs {
            outs.insert(port.to_string(), format!("{var}{field}"));
        }
        return Ok((code, outs, false));
    }
    if let Some((_, call)) = BY_NAME.iter().find(|(op, _)| *op == n.op) {
        let mut args = Vec::new();
        for k in n.inputs.keys().chain(n.params.keys()) {
            if let Some(v) = value_of(n, k, made, inputs)? {
                args.push(format!("{k}={v}"));
            }
        }
        let code = format!("{var} = {call}({})  # its arguments by their names\n", args.join(", "));
        let outs = ports.iter().map(|p| (p.clone(), var.to_string())).collect();
        return Ok((code, outs, false));
    }
    Err(if r.sig(&n.op).is_some() {
        "no landschaft function known for it yet".into()
    } else {
        "an operator unknown".into()
    })
}

/// The window of the script.
#[derive(Default)]
pub struct View {
    pub script: Script,
    pub said: Option<String>,
}

/// The script of the model open, shown to copy or save.
pub fn window(ctx: &egui::Context, s: &mut crate::app::Session) {
    let Some(v) = &mut s.python else { return };
    let mut open = true;
    egui::Window::new("Python script (landschaft)")
        .open(&mut open)
        .default_size([760.0, 560.0])
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            let total = v.script.translated + v.script.left.len();
            ui.label(
                egui::RichText::new(format!(
                    "The model as a script of landschaft, the Python library whose methods Terraflux runs: {} of \
                     {total} nodes translated. A calculator keeps its script, run in numpy by a helper at the top.",
                    v.script.translated
                ))
                .size(11.5)
                .weak(),
            );
            if !v.script.left.is_empty() {
                ui.label(
                    egui::RichText::new(format!(
                        "Left as comments: {}",
                        v.script.left.iter().map(|(id, op, _)| format!("{id} ({op})")).collect::<Vec<_>>().join(", ")
                    ))
                    .size(11.5)
                    .color(ui.visuals().warn_fg_color),
                );
            }
            ui.horizontal(|ui| {
                if ui.button("Copy").clicked() {
                    ui.ctx().copy_text(v.script.text.clone());
                    v.said = Some("copied".into());
                }
                if ui.button("Save as...").clicked() {
                    let name = s
                        .path
                        .as_ref()
                        .and_then(|p| p.file_stem())
                        .map_or("model".to_string(), |f| f.to_string_lossy().into_owned());
                    let mut d =
                        rfd::FileDialog::new().add_filter("Python", &["py"]).set_file_name(format!("{name}.py"));
                    if let Some(dir) = s.path.as_ref().and_then(|p| p.parent()) {
                        d = d.set_directory(dir);
                    }
                    if let Some(p) = d.save_file() {
                        v.said = Some(match std::fs::write(&p, &v.script.text) {
                            Ok(()) => format!("saved: {}", p.display()),
                            Err(e) => format!("not saved: {e}"),
                        });
                    }
                }
                if let Some(t) = &v.said {
                    ui.label(egui::RichText::new(t).size(11.5));
                }
            });
            ui.separator();
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                let mut t = v.script.text.as_str();
                ui.add(egui::TextEdit::multiline(&mut t).code_editor().desired_width(f32::INFINITY));
            });
        });
    if !open {
        s.python = None;
    }
}

/// The script of the session's model, its window opened.
pub fn open(s: &mut crate::app::Session, r: &Registry) {
    let script = script(&s.source, r, s.path.as_deref());
    s.python = Some(View { script, said: None });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_names_and_rules() {
        assert_eq!(
            literal(&serde_json::json!({"a": [1, 2.5, null, true], "b": "x\"y"})),
            "{\"a\": [1, 2.5, None, True], \"b\": \"x\\\"y\"}"
        );
        assert_eq!(variable("raster_read"), "raster_read");
        assert_eq!(variable("class"), "class_");
        assert_eq!(variable("map"), "map_");
        assert_eq!(variable("2nd-step"), "n_2nd_step");
        let (t, o) = reclassify_table("1 2 -> 10\n3..5 -> 20; 6 9 30\n* -> nan").unwrap();
        assert_eq!(t, "{1: 10, 2: 10, (3, np.nextafter(5, np.inf)): 20, (6, np.nextafter(9, np.inf)): 30}");
        assert_eq!(o.as_deref(), Some("np.nan"));
        assert!(reclassify_table("1 => 2").is_err());
    }

    #[test]
    fn every_counterpart_names_ports_of_its_operator() {
        let r = Registry::standard();
        for p in MAPPED {
            let sig = r.sig(p.op).unwrap_or_else(|| panic!("{}", p.op));
            let names: Vec<&str> = sig.inputs.iter().chain(&sig.params).map(|x| x.name.as_str()).collect();
            for (ours, _) in p.args {
                assert!(names.contains(ours), "{}: {ours}", p.op);
            }
            for (port, _) in p.outs {
                assert!(sig.outputs.iter().any(|o| o.name == *port), "{}: {port}", p.op);
            }
        }
        for (op, _) in BY_NAME {
            assert!(r.sig(op).is_some(), "{op}");
        }
    }

    /// The counterparts as JSON (to check them against landschaft's functions: TERRAFLUX_PY_DUMP=file).
    #[test]
    #[ignore]
    fn dump_counterparts() {
        let all: Vec<serde_json::Value> = MAPPED
            .iter()
            .map(|p| {
                let mut args: Vec<&str> = p.args.iter().map(|(_, t)| *t).filter(|t| !t.is_empty()).collect();
                args.extend(p.defaults.iter().map(|(k, _)| *k));
                if p.kind == Kind::Grid {
                    args.extend(["ncol", "nrow"]);
                }
                let fields: Vec<&str> = p.outs.iter().filter_map(|(_, f)| f.strip_prefix('.')).collect();
                serde_json::json!({"op": p.op, "call": p.call, "args": args, "fields": fields})
            })
            .chain(
                BY_NAME.iter().map(|(op, call)| serde_json::json!({"op": op, "call": call, "args": [], "fields": []})),
            )
            .collect();
        let Ok(file) = std::env::var("TERRAFLUX_PY_DUMP") else { return };
        std::fs::write(file, serde_json::to_string(&all).unwrap()).unwrap();
    }

    /// Every template as a model and its script (TERRAFLUX_PY_TEMPLATES=folder), to run both and compare.
    #[test]
    #[ignore]
    fn dump_templates() {
        let Ok(dir) = std::env::var("TERRAFLUX_PY_TEMPLATES") else { return };
        let dir = std::path::PathBuf::from(dir);
        let r = Registry::standard();
        let mut lines = Vec::new();
        // other models too (TERRAFLUX_PY_MODELS: paths separated by ;), their script beside them
        for p in std::env::var("TERRAFLUX_PY_MODELS").unwrap_or_default().split(';').filter(|p| !p.is_empty()) {
            let m = Model::load(p).unwrap();
            std::fs::write(Path::new(p).with_extension("py"), script(&m, &r, Some(Path::new(p))).text).unwrap();
        }
        for (k, t) in crate::templates::TEMPLATES.iter().enumerate() {
            let m = t.model();
            let model = dir.join(format!("t{k:02}.toml"));
            std::fs::create_dir_all(&dir).unwrap();
            m.save(&model).unwrap();
            let s = script(&m, &r, Some(&model));
            std::fs::write(dir.join(format!("t{k:02}.py")), &s.text).unwrap();
            lines.push(format!("t{k:02}\t{}\t{}\t{}", t.name, s.translated, s.left.len()));
        }
        std::fs::write(dir.join("index.tsv"), lines.join("\n")).unwrap();
    }

    #[test]
    fn a_model_as_a_script() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"
name = "Forest patterns"
request = "the forest's MSPA"

[inputs.land]
kind = "map"
path = "data/land.tif"

[[nodes]]
id = "forest"
op = "raster.calc"
inputs = { land = "$land" }
params = { expression = "out forest = land == 3" }
note = "the forest"

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.forest" }
params = { foreground = 1, edge_width = 2 }

[[nodes]]
id = "noise"
op = "nlm.gaussian_field"
inputs = { like = "$land" }
params = { range = 20 }

[[nodes]]
id = "classes"
op = "raster.reclassify"
inputs = { map = "noise.map" }
params = { rules = "0..0.5 -> 1\n* -> 2" }

[[nodes]]
id = "flows"
op = "markov.fit"
inputs = {}

[outputs.mspa]
from = "mspa.map"
"#,
        )
        .unwrap();
        let s = script(&m, &r, Some(Path::new("C:/models/forest.toml")));
        let t = &s.text;
        assert!(t.contains("land = ls.load_raster(MODEL_DIR / \"data/land.tif\")"), "{t}");
        assert!(t.contains("forest = _calc("), "{t}");
        assert!(t.contains("land=land"), "{t}");
        assert!(t.contains("mspa = ls.mspa(ls.reclassify(forest[\"forest\"], foreground=1), edge_width=2)"), "{t}");
        assert!(t.contains("noise = ls.nlm.gaussian_field(ncol=land.sizes[\"x\"], nrow=land.sizes[\"y\"], autocorr_range=20, seed=1)"), "{t}");
        assert!(t.contains("classes = ls.reclassify(noise, {(0, np.nextafter(0.5, np.inf)): 1}, others=2)"), "{t}");
        assert!(t.contains("ls.save(mspa, OUT / \"mspa\")"), "{t}");
        assert!(t.contains("MODEL_DIR = Path(\"C:/models\")"), "{t}");
        assert_eq!(s.translated, 4);
        assert_eq!(s.left.len(), 1);
        assert_eq!(s.left[0].0, "flows");
        assert!(t.contains("flows = None"), "{t}");
        assert!(t.contains("def _calc(script, **inputs):"));
    }
}
