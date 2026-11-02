//! The model as a Python script of landschaft (the library whose kernels Terraflux runs): for the paper, the
//! supervisor or the reviewer who asks for the code. Each node becomes a call of landschaft's function of the same
//! method, in the order the model runs them: its inputs as the values of the nodes before, its settings as the
//! function's arguments (by their names in landschaft), its outputs as the result or its fields. A calculator's
//! script is kept as it is, run in numpy by a helper written into the script; reclassification rules become
//! `ls.reclassify`'s table; the foreground of the habitat patterns (MSPA, FAD...) `ls.reclassify(foreground=...)`.
//! Containers become Python's own: a loop (`control.repeat`) a `for` over its steps, its carried values assigned
//! at the end of each, what it collects appended to lists; `control.each` a `for` over the collection's elements;
//! `control.switch` an `if`/`elif` per branch; a group its body in place. The species distribution models
//! (landschaft's prepare, fit and predict), the landscape metrics, a prioritization (prioritizr's `problem()`, as
//! landschaft builds it), Zonation and the pieces of the land change models (Markov, weights of evidence, the
//! changes and their validation, the logistic regressions, the demands, Dinamica's roads and continuous patcher)
//! are calls of their own; the allocation of Dinamica, CLUE-S and PLUS, which landschaft runs only as a whole model
//! (`ls.simulate.run`), is said so.
//! Tables are pandas' DataFrames and vector features geopandas' GeoDataFrames. Reading features, burning them,
//! their zones' statistics and the values at points are landschaft's (`load_vector`, `rasterize`, `zonal_stats`,
//! `extract`); the operators of tables (filter, calc, aggregate, select, sort, join, pivot) and of features (filter,
//! calc, buffer, clip, dissolve, reproject, join, spatial_join, overlay, points, from_table, polygonize, geometry,
//! distance, grid_zones) helpers written into the script that do what Terraflux does, features given also as the
//! table of their attributes. The expressions of the filters and calculators are kept as they are, run in numpy
//! over the columns by the calculator's helper.
//! A node with no counterpart known stays as a comment with its settings, the script saying which; so does a setting
//! with none that would change the result.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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
        op: "sdm.background",
        call: "ls.sdm.background",
        args: &[
            ("env", ""),
            ("n", "n"),
            ("occurrences", "occ"),
            ("area", "area"),
            ("bias", "bias"),
            ("distance", "distance"),
            ("envelope", "envelope"),
            ("exclude_occurrences", "exclude_occurrences"),
            ("tryf", "tryf"),
            ("lonlat_correction", "lonlat_correction"),
            ("seed", "seed"),
            ("x", "x"),
            ("y", "y"),
        ],
        defaults: &[("seed", "1")],
        outs: &[("points", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.importance",
        call: "ls.sdm.importance",
        args: &[("model", ""), ("n_repeats", "n_repeats"), ("seed", "seed")],
        defaults: &[],
        outs: &[("importance", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.response_curves",
        call: "ls.sdm.response_curves",
        args: &[("model", ""), ("of", "predictors"), ("n", "n"), ("fixed", "fixed")],
        defaults: &[],
        outs: &[("curves", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "markov.fit",
        call: "ls.simulate.Markov.from_maps",
        args: &[("t0", ""), ("t1", ""), ("steps", "steps")],
        defaults: &[],
        outs: &[("markov", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "demand.from_maps",
        call: "ls.simulate.AreaDemand.from_maps",
        args: &[("maps", ""), ("times", "times"), ("out_times", "out_times")],
        defaults: &[],
        outs: &[("demand", ""), ("steps", ".steps")],
        kind: Kind::Plain,
    },
    Py {
        op: "woe.fit",
        call: "ls.simulate.weights_of_evidence",
        args: &[
            ("t0", ""),
            ("t1", ""),
            ("drivers", ""),
            ("ranges", "ranges"),
            ("increment", "increment"),
            ("min_delta", "min_delta"),
            ("max_delta", "max_delta"),
            ("tolerance", "tolerance"),
            ("categorical", "categorical"),
            ("fix_abnormal", "fix_abnormal"),
            ("possible", "possible"),
            ("transitions", "transitions"),
        ],
        defaults: &[],
        outs: &[("model", ""), ("table", ".table")],
        kind: Kind::Plain,
    },
    Py {
        op: "change.transitions",
        call: "ls.simulate.changes",
        args: &[("maps", ""), ("times", "times"), ("cell_area", "cell_area")],
        defaults: &[],
        outs: &[("table", ".table"), ("one_step", ".one_step"), ("areas", ".areas"), ("matrix", ".matrix()")],
        kind: Kind::Plain,
    },
    Py {
        op: "validate.three_map",
        call: "ls.simulate.three_map_comparison",
        args: &[("t0", ""), ("t1", ""), ("simulated", ""), ("factors", "factors"), ("categories", "categories")],
        defaults: &[],
        outs: &[("table", ".table"), ("budget", ".agreement_budget"), ("figure_of_merit", ".figure_of_merit")],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.terrain",
        call: "ls.surface.terrain",
        args: &[("map", ""), ("v", "v"), ("unit", "unit"), ("neighbors", "neighbors")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.tpi",
        call: "ls.surface.tpi",
        args: &[("map", ""), ("scale", "scale"), ("win", "win"), ("normalize", "normalize")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.vrm",
        call: "ls.surface.vrm",
        args: &[("map", ""), ("s", "s")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "surface.curvature",
        call: "ls.surface.curvature",
        args: &[("map", ""), ("type", "type")],
        defaults: &[],
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

/// The helpers of stacks, tables, features and containers (stack.of, table.of, control.each, control.switch; the
/// operators of tables and of vector features).
pub const FLOW_HELPER: &str = include_str!("python_flow.py");

/// Zonation's settings and inputs of the same name in landschaft's `zonation`.
const ZONATION: &[&str] = &[
    "rule",
    "rule_param",
    "weights",
    "abf_z",
    "weight_groups",
    "group_weights",
    "thresholds",
    "extract",
    "nodata",
    "zero_mode",
    "keep_zeros",
    "cost_in_ranking",
    "area",
    "bits",
    "max_iterations",
    "ranking_error",
    "stop_on_plateau",
    "smoothing_factor",
    "curves_interval",
    "seed",
    "condition",
    "renormalize",
    "retention",
    "connectivity",
    "self_multiply",
    "interaction",
    "interaction2",
    "matrix_connectivity",
    "matrices",
    "groups",
    "preprocessing",
    "impact_groups",
    "impact_bins",
    "impact",
    "cumulative_impact",
    "transformed",
    "cost",
    "hierarchic_mask",
    "analysis_area",
    "solution",
];

/// The objectives of a prioritization landschaft's problem takes, and whether they take a budget.
const OBJECTIVES: &[(&str, bool)] = &[
    ("min_set", false),
    ("max_utility", true),
    ("max_features", true),
    ("min_shortfall", true),
    ("min_largest_shortfall", true),
    ("max_cover", true),
];

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
    // Python's own functions, which the helpers call
    "all",
    "any",
    "bool",
    "enumerate",
    "filter",
    "float",
    "format",
    "getattr",
    "hasattr",
    "id",
    "int",
    "isinstance",
    "iter",
    "next",
    "object",
    "open",
    "repr",
    "reversed",
    "sorted",
    "str",
    "tuple",
    "zip",
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

/// Where references are looked up: the nodes made and the values carried (`@name`) in a scope (a container's
/// body), then in the scopes around it; the model's inputs (`$name`).
struct Scope<'a> {
    made: HashMap<String, Made>,
    carried: HashMap<String, String>,
    inputs: &'a BTreeMap<String, String>,
    parent: Option<&'a Scope<'a>>,
}

impl<'a> Scope<'a> {
    fn top(inputs: &'a BTreeMap<String, String>) -> Self {
        Scope { made: HashMap::new(), carried: HashMap::new(), inputs, parent: None }
    }

    fn child<'b>(&'b self, carried: HashMap<String, String>) -> Scope<'b>
    where
        'a: 'b,
    {
        Scope { made: HashMap::new(), carried, inputs: self.inputs, parent: Some(self) }
    }

    fn made(&self, id: &str) -> Option<&Made> {
        self.made.get(id).or_else(|| self.parent?.made(id))
    }

    fn carried(&self, name: &str) -> Option<&String> {
        self.carried.get(name).or_else(|| self.parent?.carried(name))
    }
}

/// The helpers a node's code calls (each written into the script once).
#[derive(Clone, Copy, PartialEq)]
enum Need {
    Nothing,
    Calc,
    Flow,
    /// Both: the expressions over a table's rows and features (python_flow.py's, run by python_calc.py's).
    Both,
}

/// The script being written: what was translated, the helpers it needs, the Python names taken.
struct Writer<'r> {
    r: &'r Registry,
    out: Script,
    calc: bool,
    flow: bool,
    names: HashSet<String>,
}

impl Writer<'_> {
    /// A Python name not taken yet: the name, else with `_2`, `_3`...
    fn fresh(&mut self, base: &str) -> String {
        let mut v = base.to_string();
        let mut k = 2;
        while self.names.contains(&v) {
            v = format!("{base}_{k}");
            k += 1;
        }
        self.names.insert(v.clone());
        v
    }

    /// The nodes of a scope in the order they run, a block each.
    fn nodes(&mut self, nodes: &[Node], sc: &mut Scope, prefix: &str) -> String {
        let ids = terraflux_graph::ids(nodes);
        let order = terraflux_graph::order(nodes, |n| terraflux_graph::dependencies_among(n, &ids))
            .unwrap_or_else(|_| nodes.iter().collect());
        let mut body = String::new();
        for n in order {
            body.push_str(&self.block(n, sc, prefix));
            body.push('\n');
        }
        body
    }

    /// A node's block: its note and why as comments, then its code; or why it is not translated and its settings.
    fn block(&mut self, n: &Node, sc: &mut Scope, prefix: &str) -> String {
        let mut block = String::new();
        block.push_str(&format!("# {} ({})", n.id, n.op));
        if let Some(note) = n.note.as_deref().filter(|t| !t.trim().is_empty()) {
            block.push_str(&format!(": {}", one_line(note)));
        }
        block.push('\n');
        if let Some(why) = n.why.as_deref().filter(|t| !t.trim().is_empty()) {
            block.push_str(&format!("# why: {}\n", one_line(why)));
        }
        let var = self.fresh(&variable(&n.id));
        let made = if n.body.is_some() { self.container(n, &var, sc, prefix) } else { node(n, self.r, &var, sc) };
        match made {
            Ok((code, outs, need)) => {
                self.calc |= matches!(need, Need::Calc | Need::Both);
                self.flow |= matches!(need, Need::Flow | Need::Both);
                self.out.translated += 1;
                block.push_str(&code);
                sc.made.insert(n.id.clone(), Made { outs });
            }
            Err(why) => {
                self.out.left.push((format!("{prefix}{}", n.id), n.op.clone(), why.clone()));
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
                sc.made.insert(n.id.clone(), Made { outs: HashMap::new() });
            }
        }
        block
    }

    /// A container in Python's own terms: a loop's steps a `for`, its carried values assigned at the end of each and
    /// what it collects appended to lists; a loop over a collection a `for` over its elements, what it collects
    /// gathered; a switch an `if` per branch; a group its body in place. Its inputs (`@name` inside) a variable each.
    fn container(&mut self, n: &Node, var: &str, sc: &Scope, prefix: &str) -> Result<Code, String> {
        let body = n.body.as_ref().ok_or("no body")?;
        let inner = format!("{prefix}{}/", n.id);
        let own: &[&str] = match n.op.as_str() {
            "control.repeat" => &["times"],
            terraflux_graph::EACH => &["over"],
            terraflux_graph::SWITCH => &["case"],
            _ => &[],
        };
        if n.op == "control.repeat"
            && let Some(name) = body.carry.keys().find(|k| !n.inputs.contains_key(*k))
        {
            return Err(format!("no first value for @{name}"));
        }
        let mut code = String::new();
        let mut carried = HashMap::new();
        for k in n.inputs.keys().filter(|k| !own.contains(&k.as_str())) {
            let v = value_of(n, k, sc)?.unwrap_or_else(|| "None".into());
            let name = self.fresh(&variable(&format!("{var}_{k}")));
            code.push_str(&format!("{name} = {v}\n"));
            carried.insert(k.clone(), name);
        }
        let mut outs: HashMap<String, String> = HashMap::new();
        let mut need = Need::Nothing;
        match n.op.as_str() {
            "control.repeat" | terraflux_graph::EACH => {
                let each = n.op == terraflux_graph::EACH;
                let (head, items) = if each {
                    let over = value_of(n, "over", sc)?.ok_or("nothing to go over (over)")?;
                    let items = self.fresh(&format!("{var}_elements"));
                    code.push_str(&format!("{items} = _elements({over})\n"));
                    let (index, name, item) = (self.fresh("index"), self.fresh("name"), self.fresh("item"));
                    let head = format!("for {index}, ({name}, {item}) in enumerate({items}, start=1):\n");
                    for (k, v) in [("index", index), ("name", name), ("item", item)] {
                        carried.insert(k.into(), v);
                    }
                    need = Need::Flow;
                    (head, Some(items))
                } else {
                    let times = value_of(n, "times", sc)?.ok_or("its times are not given")?;
                    let step = self.fresh("step");
                    let head = format!("for {step} in range(1, int({times}) + 1):\n");
                    carried.insert("step".into(), step);
                    (head, None)
                };
                let mut series = Vec::new();
                for k in body.collect.keys() {
                    let list = self.fresh(&variable(&format!("{var}_{k}")));
                    code.push_str(&format!("{list} = []\n"));
                    series.push((k.clone(), list));
                }
                let mut child = sc.child(carried.clone());
                let mut inside = self.nodes(&body.nodes, &mut child, &inner);
                if let Some(u) = body.until.as_ref().filter(|_| !each) {
                    let u = loose(u, &child, &mut inside);
                    inside.push_str(&format!(
                        "if {u}:  # the loop's end: this step neither collected nor carried\n    break\n"
                    ));
                }
                for (k, list) in &series {
                    let v = loose(&body.collect[k], &child, &mut inside);
                    inside.push_str(&format!("{list}.append({v})\n"));
                }
                if !each && !body.carry.is_empty() {
                    let names: Vec<&str> = body.carry.keys().map(|k| carried[k].as_str()).collect();
                    let values: Vec<String> = body.carry.values().map(|r| loose(r, &child, &mut inside)).collect();
                    inside.push_str(&format!("{} = {}\n", names.join(", "), values.join(", ")));
                    for k in body.carry.keys() {
                        outs.insert(k.clone(), carried[k].clone());
                    }
                }
                code.push_str(&head);
                code.push_str(&indent(&inside));
                for (k, list) in series {
                    if let Some(items) = &items {
                        code.push_str(&format!("{list} = _gather({list}, [k for k, _ in {items}], \"item\")\n"));
                    }
                    outs.insert(k, list);
                }
            }
            terraflux_graph::SWITCH => {
                let case = value_of(n, "case", sc)?.ok_or("no case")?;
                let cv = self.fresh(&format!("{var}_case"));
                code.push_str(&format!("{cv} = _case({case})\n"));
                need = Need::Flow;
                let mut branches: Vec<Branch> = Vec::new();
                for b in &body.nodes {
                    let mut child = sc.child(carried.clone());
                    let block = self.block(b, &mut child, &inner);
                    let made = child.made.remove(&b.id).map(|m| m.outs).unwrap_or_default();
                    let cond = match b.id.as_str() {
                        "otherwise" => None,
                        id => Some(match id.strip_prefix("case_") {
                            Some(v) => format!("{cv} in ({}, {})", text(id), text(v)),
                            None => format!("{cv} == {}", text(id)),
                        }),
                    };
                    branches.push((cond, block, made, b.id.clone()));
                }
                let ports: BTreeSet<String> = branches.iter().flat_map(|b| b.2.keys().cloned()).collect();
                // one output: the switch's own name; more, a name each
                let mut names: Vec<(String, String)> = Vec::new();
                for p in &ports {
                    let name =
                        if ports.len() == 1 { var.to_string() } else { self.fresh(&variable(&format!("{var}_{p}"))) };
                    names.push((p.clone(), name));
                }
                let branch = self.fresh(&format!("{var}_branch"));
                let filled = |b: &Branch| {
                    let mut t = b.1.clone();
                    for (p, name) in &names {
                        t.push_str(&format!("{name} = {}\n", b.2.get(p).map_or("None", String::as_str)));
                    }
                    t.push_str(&format!("{branch} = {}\n", text(&b.3)));
                    t
                };
                let (cases, otherwise): (Vec<_>, Vec<_>) = branches.iter().partition(|b| b.0.is_some());
                for (i, b) in cases.iter().enumerate() {
                    code.push_str(&format!("{} {}:\n", if i == 0 { "if" } else { "elif" }, b.0.as_deref().unwrap()));
                    code.push_str(&indent(&filled(b)));
                }
                let rest = match otherwise.first() {
                    Some(b) => filled(b),
                    None => format!("raise ValueError(f\"no branch for the case {{{cv}!r}}\")\n"),
                };
                if cases.is_empty() {
                    code.push_str(&rest);
                } else {
                    code.push_str("else:\n");
                    code.push_str(&indent(&rest));
                }
                for (p, name) in names {
                    outs.insert(p, name);
                }
                outs.insert("branch".into(), branch);
            }
            _ => {
                // a group: its body in place, its outputs what they name
                let mut child = sc.child(carried);
                let inside = self.nodes(&body.nodes, &mut child, &inner);
                if !inside.trim().is_empty() {
                    code.push_str(inside.trim_end());
                    code.push('\n');
                }
                for (name, r) in &body.outputs {
                    let mut said = String::new();
                    outs.insert(name.clone(), loose(r, &child, &mut said));
                    code.push_str(&said);
                }
            }
        }
        Ok((code, outs, need))
    }
}

/// The expression of a reference a container ends with (what it carries, collects, gives, its end): `None` when it
/// is not translated (a node of its body left as a comment), said in a comment, the container kept.
fn loose(r: &str, sc: &Scope, said: &mut String) -> String {
    reference(r, sc).unwrap_or_else(|why| {
        said.push_str(&format!("# ({why}: None here)\n"));
        "None".into()
    })
}

/// Code inside a block (a loop's, a branch's): four spaces in; `pass` when it has no statement.
fn indent(code: &str) -> String {
    let statement = code.lines().any(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
    let mut out: String =
        code.lines().map(|l| if l.trim().is_empty() { "\n".to_string() } else { format!("    {l}\n") }).collect();
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !statement {
        out.push_str("    pass\n");
    }
    out
}

/// The model as a Python script of landschaft.
pub fn script(m: &Model, r: &Registry, model_path: Option<&Path>) -> Script {
    let mut w = Writer { r, out: Script::default(), calc: false, flow: false, names: HashSet::new() };
    for taken in ["np", "ls", "Path", "MODEL_DIR", "OUT"] {
        w.names.insert(taken.into());
    }
    let mut body = String::new();
    let mut inputs: BTreeMap<String, String> = BTreeMap::new();
    // the model's inputs
    for (name, input) in &m.inputs {
        let v = w.fresh(&variable(name));
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
    let mut sc = Scope::top(&inputs);
    body.push_str(&w.nodes(&m.nodes, &mut sc, ""));
    // the model's outputs
    if !m.outputs.is_empty() {
        body.push_str("# the model's outputs\nOUT.mkdir(parents=True, exist_ok=True)\n");
        for (name, o) in &m.outputs {
            let file = text(&o.path.clone().unwrap_or_else(|| name.clone()));
            body.push_str(&match reference(&o.from, &sc) {
                Ok(value) => format!("ls.save({value}, OUT / {file})\n"),
                Err(e) => format!("# ls.save(..., OUT / {file})  # {e}\n"),
            });
        }
    }
    let mut out = w.out;
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
    if w.calc {
        head.push_str("\n# --- Terraflux's calculator (raster.calc, and the expressions over rows) in numpy ---\n\n");
        head.push_str(CALC_HELPER.trim_end());
        head.push_str("\n\n");
    }
    if w.flow {
        head.push_str("\n# --- Terraflux's stacks, tables, features and containers ---\n\n");
        head.push_str(FLOW_HELPER.trim_end());
        head.push_str("\n\n");
    }
    if w.calc || w.flow {
        head.push_str("# --- the model ---\n\n");
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

/// The expression of a reference: `$input`, `@name` (a container's), `node.port`.
fn reference(r: &str, sc: &Scope) -> Result<String, String> {
    match Ref::parse(r).map_err(|e| e.to_string())? {
        Ref::Input(name) => sc.inputs.get(&name).cloned().ok_or_else(|| format!("${name}: not an input of the model")),
        Ref::Carried(name) => {
            sc.carried(&name).cloned().ok_or_else(|| format!("@{name}: no value of a container here"))
        }
        Ref::Port { node, port } => {
            let m = sc.made(&node).ok_or_else(|| format!("{node}: not a node before"))?;
            m.outs.get(&port).cloned().ok_or_else(|| format!("{node}.{port}: not translated"))
        }
    }
}

/// The value of a node's input or setting.
fn value_of(n: &Node, name: &str, sc: &Scope) -> Result<Option<String>, String> {
    if let Some(s) = n.inputs.get(name) {
        return match s {
            Source::Ref(x) => reference(x, sc).map(Some),
            Source::Literal(v) => Ok(Some(literal(v))),
        };
    }
    setting(n, name, sc)
}

/// The value of a node's setting (not an input of the same name).
fn setting(n: &Node, name: &str, sc: &Scope) -> Result<Option<String>, String> {
    match n.params.get(name) {
        None => Ok(None),
        Some(v) => match terraflux_graph::setting_ref(v) {
            Some(x) => reference(x, sc).map(Some),
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

type Code = (String, HashMap<String, String>, Need);

/// A switch's branch: its condition (none: otherwise), its code, its outputs' expressions, its id.
type Branch = (Option<String>, String, HashMap<String, String>, String);

/// A node's code, and the expression of each output; or why it is not translated.
fn node(n: &Node, r: &Registry, var: &str, sc: &Scope) -> Result<Code, String> {
    if n.op.starts_with("view.") {
        // a chart: what it draws, by matplotlib (left as a comment: a window would stop the script)
        let shown = ["value", "table", "features"].iter().find_map(|k| value_of(n, k, sc).ok().flatten());
        let code = match shown {
            Some(v) => format!("# ls.plot({v})  # the chart (matplotlib)\n"),
            None => "# (a chart of a value not translated)\n".to_string(),
        };
        return Ok((code, HashMap::new(), Need::Nothing));
    }
    if let Some(code) = special(n, var, sc)? {
        return Ok(code);
    }
    if let Some(code) = tables_and_features(n, var, sc)? {
        return Ok(code);
    }
    let ports: Vec<String> = crate::editor::output_ports(n, r).into_iter().map(|(p, _)| p).collect();
    match n.op.as_str() {
        "raster.calc" => {
            let script = n.params.get("expression").and_then(Value::as_str).ok_or("no script")?;
            let mut args = Vec::new();
            for k in n.inputs.keys() {
                let v = value_of(n, k, sc)?.unwrap_or_else(|| "None".into());
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
            return Ok((code, outs, Need::Calc));
        }
        "raster.reclassify" => {
            let rules = n.params.get("rules").and_then(Value::as_str).ok_or("no rules")?;
            let (table, others) = reclassify_table(rules)?;
            let map = value_of(n, "map", sc)?.ok_or("its map is not connected")?;
            let others = others.map_or(String::new(), |o| format!(", others={o}"));
            let code = format!("{var} = ls.reclassify({map}, {table}{others})\n");
            return Ok((code, HashMap::from([("map".to_string(), var.to_string())]), Need::Nothing));
        }
        _ => {}
    }
    if let Some(py) = MAPPED.iter().find(|p| p.op == n.op) {
        let mut args: Vec<String> = Vec::new();
        let mut given: Vec<&str> = Vec::new();
        let mut skipped: Vec<String> = Vec::new();
        // a neutral landscape's size first: its `like`'s, else the settings', else 256
        if py.kind == Kind::Grid {
            let like = value_of(n, "like", sc)?;
            for (ours, theirs, dim) in [("columns", "ncol", "x"), ("rows", "nrow", "y")] {
                let v = match &like {
                    Some(l) => format!("{l}.sizes[\"{dim}\"]"),
                    None => value_of(n, ours, sc)?.unwrap_or_else(|| "256".into()),
                };
                args.push(format!("{theirs}={v}"));
                given.push(theirs);
            }
        }
        for (ours, theirs) in py.args {
            if !theirs.is_empty() && given.contains(theirs) {
                continue;
            }
            let Some(mut v) = value_of(n, ours, sc)? else { continue };
            if py.kind == Kind::Habitat && *ours == "map" {
                v = match value_of(n, "foreground", sc)? {
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
        for k in n.inputs.keys().filter(|k| !known.contains(&k.as_str())) {
            skipped.push(format!("the input {k}"));
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
        return Ok((code, outs, Need::Nothing));
    }
    if let Some((_, call)) = BY_NAME.iter().find(|(op, _)| *op == n.op) {
        let mut args = Vec::new();
        for k in n.inputs.keys().chain(n.params.keys()) {
            if let Some(v) = value_of(n, k, sc)? {
                args.push(format!("{k}={v}"));
            }
        }
        let code = format!("{var} = {call}({})  # its arguments by their names\n", args.join(", "));
        let outs = ports.iter().map(|p| (p.clone(), var.to_string())).collect();
        return Ok((code, outs, Need::Nothing));
    }
    Err(if r.sig(&n.op).is_some() {
        "no landschaft function known for it yet".into()
    } else {
        "an operator unknown".into()
    })
}

/// The settings given among `names`, as `theirs=value` (a name each: ours, theirs).
fn given(n: &Node, names: &[(&str, &str)], sc: &Scope) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for (ours, theirs) in names {
        if let Some(v) = setting(n, ours, sc)? {
            out.push(format!("{theirs}={v}"));
        }
    }
    Ok(out)
}

/// A setting's text as it is (a choice: `min_set`, `glm`...).
fn choice<'n>(n: &'n Node, name: &str) -> Option<&'n str> {
    n.params.get(name).and_then(Value::as_str)
}

/// The node's inputs and settings outside `known` refused: a setting landschaft is not given changes the result.
fn refuse_others(n: &Node, known: &[&str]) -> Result<(), String> {
    for (k, what) in n.inputs.keys().map(|k| (k, "input")).chain(n.params.keys().map(|k| (k, "setting"))) {
        if !known.contains(&k.as_str()) {
            return Err(format!("its {what} {k}: not in the script yet"));
        }
    }
    Ok(())
}

/// A class as Python writes the number (1, not 1.0).
fn class_number(c: f64) -> String {
    if c.fract() == 0.0 && c.abs() < 1e15 { format!("{}", c as i64) } else { format!("{c:?}") }
}

/// The seed of a random step as Terraflux makes it: `seed` (1) plus `step` times 0x9E3779B9, kept to `mask`.
fn step_seed(n: &Node, sc: &Scope, mask: Option<&str>) -> Result<String, String> {
    let int = |x: String| if x.parse::<i64>().is_ok() { x } else { format!("int({x})") };
    let seed = value_of(n, "seed", sc)?.map_or_else(|| "1".to_string(), int);
    let e = match value_of(n, "step", sc)? {
        Some(t) => format!("{seed} + {} * 0x9E3779B9", int(t)),
        None => seed,
    };
    Ok(match mask {
        Some(m) if e.parse::<u32>().is_err() => format!("({e}) & {m}"),
        _ => e,
    })
}

/// The operators translated by code of their own, not one call with the arguments mapped: stacks and tables
/// written in the model, the species distribution models (landschaft's prepare, fit and predict), the landscape
/// metrics, a prioritization (prioritizr's problem, as landschaft builds it), Zonation, weights of evidence's
/// probability, the logistic regressions (fitted, predicted, given), Dinamica's cost maps, destinations, pathways,
/// roads and continuous patcher (the seed of a step as Terraflux makes it), a demand given as a table; the
/// allocation of the land change models, which landschaft runs only as a whole model, said so.
fn special(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    let v = |k: &str| value_of(n, k, sc);
    let need =
        |k: &str, what: &str| -> Result<String, String> { v(k)?.ok_or_else(|| format!("{what} is not connected")) };
    let one = |port: &str| HashMap::from([(port.to_string(), var.to_string())]);
    // the settings and inputs given but not passed, as comments
    let unpassed = |names: &[&str]| -> String {
        names
            .iter()
            .filter_map(|k| match (n.inputs.get(*k), n.params.get(*k)) {
                (Some(_), _) => Some(format!("# (not passed: the input {k})\n")),
                (None, Some(x)) => Some(format!("# (not passed: {k} = {})\n", literal(x))),
                _ => None,
            })
            .collect()
    };
    let code = match n.op.as_str() {
        "stack.of" => {
            let mut entries = Vec::new();
            for k in n.inputs.keys() {
                entries.push(format!("{}: {}", text(k), v(k)?.unwrap_or_else(|| "None".into())));
            }
            (format!("{var} = _stack({{{}}})\n", entries.join(", ")), one("stack"), Need::Flow)
        }
        "table.of" => {
            let columns = setting(n, "columns", sc)?.unwrap_or_else(|| "[]".into());
            let rows = setting(n, "rows", sc)?.unwrap_or_else(|| "[]".into());
            (format!("{var} = _table({columns}, {rows})\n"), one("table"), Need::Flow)
        }
        "sdm.prepare" => {
            let occ = need("occurrences", "its occurrences")?;
            let env = need("env", "its predictors (env)")?;
            let mut args = vec![occ, env.clone()];
            args.extend(given(
                n,
                &[
                    ("species", "species"),
                    ("clean", "clean"),
                    ("one_per_cell", "one_per_cell"),
                    ("thin", "thin"),
                    ("predictors", "predictors"),
                    ("background", "background"),
                    ("folds", "folds"),
                    ("seed", "seed"),
                    ("x", "x"),
                    ("y", "y"),
                ],
                sc,
            )?);
            // the accessible area: polygons given, else its setting
            match n.inputs.get("area") {
                Some(_) => args.push(format!("area={}", need("area", "its area")?)),
                None => args.extend(given(n, &[("area", "area")], sc)?),
            }
            let mut code = String::new();
            for k in ["regions", "within"] {
                if n.inputs.contains_key(k) {
                    code.push_str(&format!("# (not passed: the input {k})\n"));
                }
            }
            if n.inputs.contains_key("background") {
                code.push_str("# (not passed: the input background; its points drawn by ls.sdm.background instead)\n");
            }
            code.push_str(&format!("{var} = ls.sdm.prepare({})\n", args.join(", ")));
            let outs = HashMap::from([
                ("records".to_string(), var.to_string()),
                ("env".to_string(), format!("{env}.sel(band={var}.predictors)")),
                ("area".to_string(), format!("{var}.area")),
                ("report".to_string(), format!("{var}.report")),
                ("settings".to_string(), format!("{var}.settings")),
            ]);
            (code, outs, Need::Nothing)
        }
        "sdm.fit" => {
            let records = need("records", "its records")?;
            let model = choice(n, "model").unwrap_or("maxnet");
            let mut code = unpassed(&["predictors", "presence", "fold", "weights"]);
            let estimator = match model {
                "maxnet" => format!(
                    "ls.sdm.MaxNet({})",
                    given(
                        n,
                        &[
                            ("classes", "classes"),
                            ("regmult", "regmult"),
                            ("add_samples_to_background", "add_samples_to_background"),
                            ("type", "type"),
                            ("clamp", "clamp"),
                            ("categorical", "categorical"),
                        ],
                        sc
                    )?
                    .join(", ")
                ),
                "glm" => {
                    code.push_str("from sklearn.linear_model import LogisticRegression\n");
                    let mut a = given(n, &[("C", "C")], sc)?;
                    a.push("max_iter=1000".into());
                    let glm = format!("LogisticRegression({})", a.join(", "));
                    if n.params.get("standardize").and_then(Value::as_bool) == Some(false) {
                        glm
                    } else {
                        code.push_str("from sklearn.pipeline import make_pipeline\n");
                        code.push_str("from sklearn.preprocessing import StandardScaler\n");
                        format!("make_pipeline(StandardScaler(), {glm})")
                    }
                }
                "random_forest" => {
                    code.push_str("from sklearn.ensemble import RandomForestClassifier\n");
                    let mut a = given(
                        n,
                        &[
                            ("n_estimators", "n_estimators"),
                            ("min_samples_leaf", "min_samples_leaf"),
                            ("min_samples_split", "min_samples_split"),
                            ("max_depth", "max_depth"),
                            ("bootstrap", "bootstrap"),
                        ],
                        sc,
                    )?;
                    match choice(n, "max_features") {
                        Some("all") => a.push("max_features=None".into()),
                        _ => a.extend(given(n, &[("max_features", "max_features")], sc)?),
                    }
                    let seed = setting(n, "random_state", sc)?.unwrap_or_else(|| "1".into());
                    a.push(format!("random_state={seed}"));
                    format!("RandomForestClassifier({})", a.join(", "))
                }
                other => return Err(format!("the model {other:?}: not in the script yet")),
            };
            let cv = given(n, &[("cv", "cv")], sc)?.into_iter().map(|c| format!(", {c}")).collect::<String>();
            code.push_str(&format!("{var} = ls.sdm.fit({records}, {{{}: {estimator}}}{cv})\n", text(model)));
            let outs = HashMap::from([
                ("model".to_string(), var.to_string()),
                ("metrics".to_string(), format!("{var}.metrics")),
                ("summary".to_string(), format!("{var}.summary")),
                ("threshold".to_string(), format!("{var}.thresholds")),
                ("predictions".to_string(), format!("{var}.predictions")),
            ]);
            (code, outs, Need::Nothing)
        }
        "sdm.maxnet_predict" => {
            let model = need("model", "its model")?;
            let env = need("env", "its predictors (env)")?;
            let mut code = unpassed(&["type"]);
            let clamp = given(n, &[("clamp", "clamp")], sc)?.into_iter().map(|c| format!(", {c}")).collect::<String>();
            code.push_str(&format!(
                "{var} = ls.sdm.predict({model}, {env}, ensemble=None, sd=False{clamp}).isel(band=0, drop=True)\n"
            ));
            (code, one("prediction"), Need::Nothing)
        }
        "metrics.class" | "metrics.landscape" | "metrics.patch" => {
            let map = need("map", "its map")?;
            let level = &n.op["metrics.".len()..];
            let mut args = vec![map, format!("level={}", text(level))];
            for (k, x) in &n.params {
                match k.as_str() {
                    "show" | "patches_path" => {}
                    "metrics" => args.push(format!("metric={}", setting(n, k, sc)?.unwrap_or_else(|| literal(x)))),
                    // (landschaft's types end in " metric": "diversity metric")
                    "type" if x.as_str().is_some_and(|t| !t.ends_with(" metric")) => {
                        args.push(format!("type={}", text(&format!("{} metric", x.as_str().unwrap_or("")))))
                    }
                    _ => args.push(format!("{k}={}", setting(n, k, sc)?.unwrap_or_else(|| literal(x)))),
                }
            }
            let code = format!("{var} = ls.landscape_metrics({})\n", args.join(", "));
            (code, one("table"), Need::Nothing)
        }
        "woe.probability" => {
            let model = need("model", "its model")?;
            let landscape = need("landscape", "its landscape")?;
            let drivers = need("drivers", "its drivers")?;
            (format!("{var} = {model}.probability({landscape}, {drivers})\n"), one("probability"), Need::Nothing)
        }
        "prioritize.zonation" => {
            let features = need("features", "its features")?;
            let mut args = vec![features];
            let mut skipped = Vec::new();
            for k in n.inputs.keys().chain(n.params.keys()).filter(|k| *k != "features") {
                if ZONATION.contains(&k.as_str()) {
                    args.push(format!("{k}={}", v(k)?.unwrap_or_else(|| "None".into())));
                } else {
                    skipped.push(k.as_str());
                }
            }
            let mut code = unpassed(&skipped);
            code.push_str(&format!("{var} = ls.prioritize.zonation({})\n", args.join(", ")));
            let outs = HashMap::from([
                ("rank".to_string(), format!("{var}.rank")),
                ("curves".to_string(), format!("{var}.summary_curves")),
                ("features".to_string(), format!("{var}.features")),
                ("auc".to_string(), format!("{var}.auc")),
                ("iterations".to_string(), format!("{var}.iterations")),
            ]);
            (code, outs, Need::Nothing)
        }
        "prioritize.optimize" => {
            for k in n.inputs.keys() {
                const INPUTS: &[&str] =
                    &["features", "cost", "locked_in", "locked_out", "connectivity_data", "asym_connectivity_data"];
                if !INPUTS.contains(&k.as_str()) {
                    return Err(format!("its input {k}: not in the script yet"));
                }
            }
            const KNOWN: &[&str] = &[
                "objective",
                "budget",
                "relative_targets",
                "absolute_targets",
                "feature_weights",
                "boundary_penalty",
                "edge_factor",
                "connectivity_penalty",
                "asym_connectivity_penalty",
                "neighbors",
                "clamp",
                "contiguity",
                "decisions",
                "portfolio",
                "number_solutions",
                "pool_gap",
                "remove_duplicates",
                "seed",
                "solver",
                "gap",
                "time_limit",
                "threads",
            ];
            let other: Vec<&str> = n.params.keys().map(String::as_str).filter(|k| !KNOWN.contains(k)).collect();
            if !other.is_empty() {
                return Err(format!("its settings {}: not in the script yet", other.join(", ")));
            }
            let cost = need("cost", "its cost")?;
            let features = need("features", "its features")?;
            let mut chain = vec![format!("ls.prioritize.problem({cost}, {features})")];
            let objective = choice(n, "objective").unwrap_or("min_set");
            match OBJECTIVES.iter().find(|(o, _)| *o == objective) {
                Some((o, false)) => chain.push(format!(".add_{o}_objective()")),
                Some((o, true)) => {
                    let budget = setting(n, "budget", sc)?.ok_or_else(|| format!("{o} needs a budget"))?;
                    chain.push(format!(".add_{o}_objective({budget})"));
                }
                None => return Err(format!("the objective {objective:?}: not in the script yet")),
            }
            for (k, method) in [
                ("relative_targets", "add_relative_targets"),
                ("absolute_targets", "add_absolute_targets"),
                ("feature_weights", "add_feature_weights"),
            ] {
                if let Some(x) = setting(n, k, sc)? {
                    chain.push(format!(".{method}({x})"));
                }
            }
            if let Some(p) = setting(n, "boundary_penalty", sc)? {
                let e = given(n, &[("edge_factor", "edge_factor")], sc)?.into_iter().map(|e| format!(", {e}"));
                chain.push(format!(".add_boundary_penalties({p}{})", e.collect::<String>()));
            }
            // the connectivity of pairs of units (a table i, j, x of every cell) as landschaft's units x units matrix
            let mut pairs = false;
            for (k, data, method) in [
                ("connectivity_penalty", "connectivity_data", "add_connectivity_penalties"),
                ("asym_connectivity_penalty", "asym_connectivity_data", "add_asym_connectivity_penalties"),
            ] {
                if let Some(p) = setting(n, k, sc)? {
                    let t = v(data)?.ok_or_else(|| format!("{k} needs {data}"))?;
                    chain.push(format!(".{method}({p}, _unit_pairs({t}, {cost}.size))"));
                    pairs = true;
                }
            }
            if let Some(k) = setting(n, "neighbors", sc)? {
                let c = given(n, &[("clamp", "clamp")], sc)?.into_iter().map(|e| format!(", {e}"));
                chain.push(format!(".add_neighbor_constraints({k}{})", c.collect::<String>()));
            }
            if n.params.get("contiguity").and_then(Value::as_bool) == Some(true) {
                chain.push(".add_contiguity_constraints()".into());
            }
            for (k, method) in
                [("locked_in", "add_locked_in_constraints"), ("locked_out", "add_locked_out_constraints")]
            {
                if let Some(x) = v(k)? {
                    chain.push(format!(".{method}({x})"));
                }
            }
            match choice(n, "decisions").unwrap_or("binary") {
                "binary" => chain.push(".add_binary_decisions()".into()),
                "proportion" => chain.push(".add_proportion_decisions()".into()),
                d => return Err(format!("the decisions {d:?}: not in the script yet")),
            }
            if let Some(p) = choice(n, "portfolio") {
                let names: &[(&str, &str)] = match p {
                    "gap" => &[("number_solutions", "number_solutions"), ("pool_gap", "pool_gap")],
                    "cuts" | "top" => &[("number_solutions", "number_solutions")],
                    "shuffle" => &[
                        ("number_solutions", "number_solutions"),
                        ("remove_duplicates", "remove_duplicates"),
                        ("seed", "seed"),
                    ],
                    p => return Err(format!("the portfolio {p:?}: not in the script yet")),
                };
                chain.push(format!(".add_{p}_portfolio({})", given(n, names, sc)?.join(", ")));
            }
            let solver = choice(n, "solver").unwrap_or("highs");
            let mut s = given(n, &[("gap", "gap"), ("time_limit", "time_limit")], sc)?;
            if solver == "gurobi" {
                s.extend(given(n, &[("threads", "threads")], sc)?);
            }
            if !s.is_empty() || solver != "highs" {
                chain.push(format!(".add_{solver}_solver({})", s.join(", ")));
            }
            chain.push(".solve()".into());
            let code = format!("{var} = (\n    {}\n)\n", chain.join("\n    "));
            // the solution as a map (`selection`; `values` is one per unit); a portfolio's first, all and how often
            let outs = if n.params.contains_key("portfolio") {
                HashMap::from([
                    ("solution".to_string(), format!("{var}[0].selection")),
                    ("solutions".to_string(), format!("{var}.selection")),
                    ("frequency".to_string(), format!("{var}.frequency")),
                    ("coverage".to_string(), format!("{var}[0].coverage")),
                ])
            } else {
                HashMap::from([
                    ("solution".to_string(), format!("{var}.selection")),
                    ("coverage".to_string(), format!("{var}.coverage")),
                ])
            };
            (code, outs, if pairs { Need::Flow } else { Need::Nothing })
        }
        "glm.fit" => {
            refuse_others(n, &["landscape", "drivers", "formulas", "maxit", "epsilon"])?;
            let landscape = need("landscape", "its landscape")?;
            let drivers = need("drivers", "its drivers")?;
            // Terraflux fits the classes in their order, landschaft in the formulas'
            let mut args = Vec::new();
            match n.params.get("formulas") {
                None | Some(Value::Null) => {}
                Some(f) => match (terraflux_graph::setting_ref(f), f.as_object()) {
                    (Some(x), _) => {
                        let x = reference(x, sc)?;
                        args.push(format!("dict(sorted({x}.items(), key=lambda kv: float(kv[0])))"));
                    }
                    (None, Some(o)) => {
                        let mut classes = Vec::new();
                        for (k, d) in o {
                            let c = k.trim().parse::<f64>().map_err(|_| format!("formulas: {k:?} is not a class"))?;
                            classes.push((c, d));
                        }
                        classes.sort_by(|a, b| a.0.total_cmp(&b.0));
                        let entries: Vec<String> =
                            classes.iter().map(|(c, d)| format!("{}: {}", class_number(*c), literal(d))).collect();
                        args.push(format!("{{{}}}", entries.join(", ")));
                    }
                    (None, None) => return Err(format!("formulas {}: not a table of classes", literal(f))),
                },
            }
            args.extend(given(n, &[("maxit", "maxit"), ("epsilon", "epsilon")], sc)?);
            let code =
                format!("{var} = ls.simulate.LogisticRegression({}).fit({landscape}, {drivers})\n", args.join(", "));
            (code, one("model"), Need::Nothing)
        }
        "glm.predict" => {
            refuse_others(n, &["model", "drivers", "landscape"])?;
            let model = need("model", "its model")?;
            let drivers = need("drivers", "its drivers")?;
            let landscape = v("landscape")?.map(|l| format!(", landscape={l}")).unwrap_or_default();
            (format!("{var} = _by_class({model}.predict({drivers}{landscape}))\n"), one("suitability"), Need::Flow)
        }
        "glm.coefficients" => {
            refuse_others(n, &["fits"])?;
            let fits = n.params.get("fits").and_then(Value::as_array).ok_or("its fits are not written in the model")?;
            let mut classes = Vec::new();
            let mut seen = Vec::new();
            for f in fits {
                let class = f.get("class").and_then(Value::as_f64).ok_or("a fit without its class")?;
                let what = format!("class {}", class_number(class));
                let list = |k: &str| f.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
                let drivers: Vec<String> =
                    list("drivers").iter().filter_map(|d| d.as_str().map(String::from)).collect();
                let coefficients: Vec<f64> = list("coefficients").iter().filter_map(Value::as_f64).collect();
                let pivot: Vec<i64> = list("pivot").iter().filter_map(Value::as_i64).collect();
                let p = drivers.len() + 1;
                // landschaft's from_coefficients makes every driver count (pivot 1, 2..., full rank)
                if coefficients.len() != p
                    || pivot != (1..=p as i64).collect::<Vec<_>>()
                    || f.get("rank").and_then(Value::as_i64) != Some(p as i64)
                {
                    return Err(format!(
                        "{what}: an aliased driver or a pivot of its own (landschaft's \
                         LogisticRegression.from_coefficients takes models of full rank)"
                    ));
                }
                let mut names = drivers.clone();
                names.sort_unstable();
                names.dedup();
                if names.len() != drivers.len() || seen.contains(&class.to_bits()) {
                    return Err(format!("{what}: a driver or the class twice"));
                }
                seen.push(class.to_bits());
                let terms: Vec<String> = std::iter::once("\"(Intercept)\"".to_string())
                    .chain(drivers.iter().map(|d| text(d)))
                    .zip(&coefficients)
                    .map(|(t, b)| format!("{t}: {}", literal(&Value::from(*b))))
                    .collect();
                classes.push(format!("{}: {{{}}}", class_number(class), terms.join(", ")));
            }
            let code = format!(
                "{var} = ls.simulate.LogisticRegression.from_coefficients({{\n    {},\n}})\n",
                classes.join(",\n    ")
            );
            (code, one("model"), Need::Nothing)
        }
        "dinamica.cost_map" => {
            refuse_others(n, &["sources", "friction", "diagonal", "relative"])?;
            let mut args = vec![need("sources", "its sources")?, need("friction", "its friction")?];
            args.extend(given(n, &[("diagonal", "diagonal"), ("relative", "relative")], sc)?);
            let code = format!("{var} = ls.simulate.friction_cost({}, directions=True)\n", args.join(", "));
            let outs = HashMap::from([
                ("cost".to_string(), format!("{var}[0]")),
                ("direction".to_string(), format!("{var}[1]")),
            ]);
            (code, outs, Need::Nothing)
        }
        "dinamica.destinations" => {
            const SETTINGS: &[(&str, &str)] =
                &[("per_quadrant", "per_quadrant"), ("min_distance", "min_distance"), ("max_attempts", "max_attempts")];
            refuse_others(
                n,
                &["attraction", "seed", "step", "quadrants", "per_quadrant", "min_distance", "max_attempts"],
            )?;
            let mut args = vec![need("attraction", "its attraction")?];
            args.extend(setting(n, "quadrants", sc)?);
            args.extend(given(n, SETTINGS, sc)?);
            args.push(format!("seed={}", step_seed(n, sc, None)?));
            (format!("{var} = ls.simulate.destinations({})\n", args.join(", ")), one("map"), Need::Nothing)
        }
        "dinamica.pathways" => {
            refuse_others(
                n,
                &["network", "sources", "cost", "directions", "seed", "step", "lottery", "merge", "value"],
            )?;
            let mut args = vec![need("network", "its network")?, need("sources", "its sources")?];
            for k in ["cost", "directions"] {
                if let Some(x) = v(k)? {
                    args.push(format!("{k}={x}"));
                }
            }
            args.extend(given(n, &[("lottery", "lottery"), ("merge", "merge"), ("value", "value")], sc)?);
            args.push(format!("seed={}", step_seed(n, sc, None)?));
            (format!("{var} = ls.simulate.pathways({})\n", args.join(", ")), one("map"), Need::Nothing)
        }
        "dinamica.roads" => {
            const SETTINGS: &[(&str, &str)] = &[
                ("quadrants", "quadrants"),
                ("distance", "distance"),
                ("spacing", "spacing"),
                ("per_quadrant", "per_quadrant"),
                ("max_attempts", "max_attempts"),
                ("lottery", "lottery"),
                ("diagonal", "diagonal"),
                ("relative", "relative"),
            ];
            let mut known = vec!["network", "friction", "attraction", "mask", "step", "seed"];
            known.extend(SETTINGS.iter().map(|(k, _)| *k));
            refuse_others(n, &known)?;
            let network = need("network", "its network")?;
            let mut args = vec![need("friction", "its friction")?, need("attraction", "its attraction")?];
            args.extend(given(n, SETTINGS, sc)?);
            if let Some(m) = v("mask")? {
                args.push(format!("mask={m}"));
            }
            // Terraflux's seed of a step kept to 63 bits, as landschaft's run gives its steps
            let seed = step_seed(n, sc, Some("0x7FFFFFFFFFFFFFFF"))?;
            let code = format!("{var} = ls.simulate.RoadBuilder({}).step({network}, seed={seed})\n", args.join(", "));
            let outs = HashMap::from([
                ("network".to_string(), var.to_string()),
                ("distance".to_string(), format!("_road_distance({var})")),
            ]);
            (code, outs, Need::Flow)
        }
        "dinamica.continuous_patcher" => {
            const SETTINGS: &[(&str, &str)] = &[
                ("mean", "mean"),
                ("variance", "variance"),
                ("isometry", "isometry"),
                ("unit", "unit"),
                ("prune_factor", "prune_factor"),
                ("window", "window"),
            ];
            let mut known = vec!["stock", "probability", "quota", "step", "seed"];
            known.extend(SETTINGS.iter().map(|(k, _)| *k));
            refuse_others(n, &known)?;
            let stock = need("stock", "its stock")?;
            let probability = need("probability", "its probability")?;
            let quota = need("quota", "its quota")?;
            let seed = step_seed(n, sc, None)?;
            let code = format!(
                "{var} = ls.simulate.ContinuousPatcher({}).allocate({stock}, {probability}, {quota}, seed={seed})\n",
                given(n, SETTINGS, sc)?.join(", ")
            );
            let outs = HashMap::from([
                ("stock".to_string(), format!("{var}.stock")),
                ("probability".to_string(), format!("{var}.probability")),
                ("taken".to_string(), format!("{var}.taken")),
                ("patch".to_string(), format!("{var}.patches")),
                ("info".to_string(), format!("_stock_info({var}, {quota})")),
            ]);
            (code, outs, Need::Flow)
        }
        "demand.table" => {
            refuse_others(n, &["table"])?;
            let table = setting(n, "table", sc)?.ok_or("its table is not given")?;
            let outs =
                HashMap::from([("demand".to_string(), var.to_string()), ("steps".to_string(), format!("{var}.steps"))]);
            (format!("{var} = _demand_table({table})\n"), outs, Need::Flow)
        }
        op @ ("dinamica.step" | "lulcc.step" | "cars.step") => {
            let preset = match op {
                "dinamica.step" => "preset=\"dinamica\"",
                "cars.step" => "preset=\"plus\"",
                _ => "preset=ls.simulate.presets.clue_s",
            };
            return Err(format!(
                "landschaft allocates only within a whole model: ls.simulate.run(t0, t1, drivers, {preset}, \
                 until=...) fits, validates and projects it"
            ));
        }
        _ => return Ok(None),
    };
    Ok(Some(code))
}

/// What the outputs of a helper's call are.
#[derive(Clone, Copy)]
enum Gives {
    /// A table (`table`).
    Table,
    /// Features (`vector`), and their attributes as a table (`table`: the feature and those that are numbers).
    Features,
    /// One output, of this name.
    One(&'static str),
}

/// An operator of tables or of features translated as a call of a helper of the script (python_flow.py), which
/// does what Terraflux does: the node's inputs or settings `first` by position (needed), then those of `named`
/// given, by their names there.
struct Frame {
    op: &'static str,
    call: &'static str,
    first: &'static [&'static str],
    named: &'static [(&'static str, &'static str)],
    gives: Gives,
}

const FRAMES: &[Frame] = &[
    Frame {
        op: "table.aggregate",
        call: "_aggregate",
        first: &["table"],
        named: &[("by", "by"), ("columns", "columns"), ("stats", "stats")],
        gives: Gives::Table,
    },
    Frame {
        op: "table.select",
        call: "_select",
        first: &["table"],
        named: &[("columns", "columns"), ("drop", "drop"), ("rename", "rename")],
        gives: Gives::Table,
    },
    Frame {
        op: "table.sort",
        call: "_sort",
        first: &["table", "by"],
        named: &[("descending", "descending"), ("top", "top")],
        gives: Gives::Table,
    },
    Frame {
        op: "table.join",
        call: "_join",
        first: &["left", "right", "on"],
        named: &[("right_on", "right_on"), ("how", "how"), ("suffix", "suffix")],
        gives: Gives::Table,
    },
    Frame {
        op: "table.pivot",
        call: "_pivot",
        first: &["table", "direction"],
        named: &[
            ("id", "id"),
            ("names", "names"),
            ("values", "values"),
            ("prefix", "prefix"),
            ("fill", "fill"),
            ("columns", "columns"),
            ("names_to", "names_to"),
            ("values_to", "values_to"),
        ],
        gives: Gives::Table,
    },
    Frame {
        op: "vector.extract",
        call: "_extract",
        first: &["map", "vector"],
        named: &[("method", "method"), ("buffer", "buffer"), ("stats", "stats"), ("all_touched", "all_touched")],
        gives: Gives::Features,
    },
    Frame {
        op: "vector.buffer",
        call: "_buffer",
        first: &["vector", "distance"],
        named: &[("segments", "segments"), ("dissolve", "dissolve")],
        gives: Gives::Features,
    },
    Frame { op: "vector.clip", call: "_clip", first: &["vector", "mask"], named: &[], gives: Gives::Features },
    Frame {
        op: "vector.dissolve",
        call: "_dissolve",
        first: &["vector"],
        named: &[("by", "by")],
        gives: Gives::Features,
    },
    Frame {
        op: "vector.reproject",
        call: "_reproject",
        first: &["vector"],
        named: &[("crs", "crs"), ("like", "like")],
        gives: Gives::Features,
    },
    Frame {
        op: "vector.join",
        call: "_join_features",
        first: &["vector", "table"],
        named: &[("on", "on"), ("key", "key")],
        gives: Gives::Features,
    },
    Frame {
        op: "vector.spatial_join",
        call: "_spatial_join",
        first: &["vector", "other"],
        named: &[("mode", "mode"), ("fields", "fields"), ("suffix", "suffix")],
        gives: Gives::Features,
    },
    Frame {
        op: "vector.overlay",
        call: "_overlay",
        first: &["vector", "other"],
        named: &[("how", "how"), ("suffix", "suffix")],
        gives: Gives::Features,
    },
    Frame {
        op: "vector.points",
        call: "_points_inside",
        first: &["vector"],
        named: &[("method", "method"), ("n", "n"), ("spacing", "spacing"), ("seed", "seed")],
        gives: Gives::Features,
    },
    Frame {
        op: "vector.from_table",
        call: "_points_from",
        first: &["table"],
        named: &[("x", "x"), ("y", "y"), ("crs", "crs")],
        gives: Gives::Features,
    },
    Frame {
        op: "vector.polygonize",
        call: "_polygonize",
        first: &["map"],
        named: &[("neighbours", "neighbours")],
        gives: Gives::Features,
    },
    Frame { op: "vector.geometry", call: "_measures", first: &["vector"], named: &[], gives: Gives::Features },
    Frame {
        op: "vector.distance",
        call: "_distance",
        first: &["vector", "like"],
        named: &[("all_touched", "all_touched")],
        gives: Gives::One("map"),
    },
    Frame {
        op: "vector.grid_zones",
        call: "_squares",
        first: &["map", "size"],
        named: &[],
        gives: Gives::One("zones"),
    },
];

/// A script's text as Python writes it: a script of several lines in triple quotes, as it is; else a string.
fn script_text(s: &str) -> String {
    if s.contains('\n') && !s.contains("\"\"\"") && !s.contains('\\') && !s.trim_end().ends_with('"') {
        format!("\"\"\"\n{}\n\"\"\"", s.trim_end())
    } else {
        text(s)
    }
}

/// The operators of tables (pandas' DataFrames) and of vector features (geopandas' GeoDataFrames): a call of
/// landschaft's (load_vector, rasterize, zonal_stats) or of a helper written into the script (python_flow.py) that
/// does what Terraflux does; the expressions of the filters and calculators as they are, run by the calculator's
/// helper over the columns. A setting with no counterpart that would change the result is refused.
fn tables_and_features(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    let v = |k: &str| value_of(n, k, sc);
    let need = |k: &str| -> Result<String, String> {
        v(k)?.ok_or_else(|| {
            if n.params.contains_key(k)
                || !["table", "vector", "map", "left", "right", "other", "mask", "like"].contains(&k)
            {
                format!("its {k} is not given")
            } else {
                format!("its {k} is not connected")
            }
        })
    };
    // the inputs and settings given among `names`, as `theirs=value`
    let named = |names: &[(&str, &str)]| -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        for (ours, theirs) in names {
            if let Some(x) = v(ours)? {
                out.push(format!("{theirs}={x}"));
            }
        }
        Ok(out)
    };
    // the settings and inputs given but not among `known`, as comments
    let unpassed = |known: &[&str]| -> String {
        let mut said = String::new();
        for (k, x) in &n.params {
            if !known.contains(&k.as_str()) {
                said.push_str(&format!("# (not passed: {k} = {})\n", literal(x)));
            }
        }
        for k in n.inputs.keys().filter(|k| !known.contains(&k.as_str())) {
            said.push_str(&format!("# (not passed: the input {k})\n"));
        }
        said
    };
    let features = |var: &str| {
        HashMap::from([("vector".to_string(), var.to_string()), ("table".to_string(), format!("_attributes({var})"))])
    };
    let one = |port: &str| HashMap::from([(port.to_string(), var.to_string())]);
    let code = match n.op.as_str() {
        "vector.read" => {
            let path = setting(n, "path", sc)?.ok_or("its file (path) is not given")?;
            let layer = named(&[("layer", "layer")])?.into_iter().map(|l| format!(", {l}")).collect::<String>();
            let code = format!("{}{var} = ls.load_vector({path}{layer})\n", unpassed(&["path", "layer"]));
            (code, features(var), Need::Flow)
        }
        "vector.rasterize" => {
            let mut args = vec![need("vector")?, need("like")?];
            args.extend(named(&[("field", "field"), ("value", "value")])?);
            match n.params.get("fill") {
                // (no value: NaN where no feature is)
                Some(Value::Null) => args.push("fill=np.nan".into()),
                _ => args.extend(named(&[("fill", "fill")])?),
            }
            args.extend(named(&[("all_touched", "all_touched"), ("merge", "merge")])?);
            let mut code = unpassed(&["vector", "like", "field", "value", "fill", "all_touched", "merge"]);
            code.push_str(&format!("{var} = ls.rasterize({})\n", args.join(", ")));
            (code, one("map"), Need::Nothing)
        }
        "vector.zonal" => {
            // a row per feature (1..), a column per statistic
            let mut args = vec![need("map")?, format!("{}.geometry", need("vector")?)];
            args.extend(named(&[("stats", "stats"), ("all_touched", "all_touched")])?);
            let mut code = unpassed(&["vector", "map", "stats", "all_touched"]);
            code.push_str(&format!("{var} = _attributes(ls.zonal_stats({}))\n", args.join(", ")));
            (code, one("table"), Need::Flow)
        }
        op @ ("table.filter" | "table.calc" | "vector.filter" | "vector.calc") => {
            let (port, call) = match op {
                "table.filter" => ("table", "_table_filter"),
                "table.calc" => ("table", "_table_calc"),
                "vector.filter" => ("vector", "_features_filter"),
                _ => ("vector", "_features_calc"),
            };
            let mut args = vec![need(port)?];
            if op == "table.calc" {
                args.push(setting(n, "column", sc)?.ok_or("its column's name is not given")?);
            }
            // the expression as it is (map algebra, run by _calc over the columns)
            args.push(match n.params.get("expression") {
                Some(x) if terraflux_graph::setting_ref(x).is_some() => {
                    setting(n, "expression", sc)?.unwrap_or_default()
                }
                Some(Value::String(s)) => script_text(s),
                _ => return Err("no expression".into()),
            });
            if op == "vector.calc"
                && let Some(c) = setting(n, "column", sc)?
            {
                args.push(format!("column={c}"));
            }
            // the node's other inputs, numbers and texts by their names
            let mut others = Vec::new();
            for k in n.inputs.keys().filter(|k| k.as_str() != port) {
                others.push(format!("{}: {}", text(k), v(k)?.unwrap_or_else(|| "None".into())));
            }
            if !others.is_empty() {
                args.push(format!("inputs={{{}}}", others.join(", ")));
            }
            let mut known: Vec<&str> = n.inputs.keys().map(String::as_str).collect();
            known.extend(["expression", "column"]);
            let code = format!("{}{var} = {call}({})\n", unpassed(&known), args.join(", "));
            let outs = if port == "table" { one("table") } else { features(var) };
            (code, outs, Need::Both)
        }
        "vector.geometry" if n.params.get("geodesic").and_then(Value::as_bool) == Some(true) => {
            return Err("its geodesic areas (GDAL's, on the WGS84 ellipsoid): not in the script yet".into());
        }
        "vector.density" => {
            return Err("no landschaft function for it: a kernel density of points on a map's grid (ArcGIS's \
                        Kernel Density), to write by hand"
                .into());
        }
        "vector.exact_sum" => {
            return Err("landschaft's exact sums (exactextract's) are internal to its prioritization \
                        (landschaft.prioritize._milp.exact_sums), not a function of its own"
                .into());
        }
        op => {
            let Some(f) = FRAMES.iter().find(|f| f.op == op) else { return Ok(None) };
            let mut args = Vec::new();
            for k in f.first {
                args.push(need(k)?);
            }
            args.extend(named(f.named)?);
            let known: Vec<&str> = f.first.iter().copied().chain(f.named.iter().map(|(k, _)| *k)).collect();
            // (vector.geometry's geodesic false: the areas in the features' units, as here)
            let known = if op == "vector.geometry" { [known, vec!["geodesic"]].concat() } else { known };
            let code = format!("{}{var} = {}({})\n", unpassed(&known), f.call, args.join(", "));
            let outs = match f.gives {
                Gives::Table => one("table"),
                Gives::Features => features(var),
                Gives::One(port) => one(port),
            };
            (code, outs, Need::Flow)
        }
    };
    Ok(Some(code))
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
        for f in FRAMES {
            let sig = r.sig(f.op).unwrap_or_else(|| panic!("{}", f.op));
            let names: Vec<&str> = sig.inputs.iter().chain(&sig.params).map(|x| x.name.as_str()).collect();
            for k in f.first.iter().chain(f.named.iter().map(|(k, _)| k)) {
                assert!(names.contains(k), "{}: {k}", f.op);
            }
            let ports: &[&str] = match f.gives {
                Gives::Table => &["table"],
                Gives::Features => &["vector"],
                Gives::One(p) => &[p],
            };
            for port in ports {
                assert!(sig.outputs.iter().any(|o| o.name == *port), "{}: {port}", f.op);
            }
        }
    }

    /// The script parses as Python (when there is a Python to ask).
    fn parses(t: &str) {
        if let Ok(o) = std::process::Command::new("python")
            .args(["-I", "-c", "import ast, sys; ast.parse(sys.stdin.read())"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.take().unwrap().write_all(t.as_bytes())?;
                c.wait_with_output()
            })
        {
            assert!(o.status.success(), "{}\n{t}", String::from_utf8_lossy(&o.stderr));
        }
    }

    #[test]
    fn tables_as_pandas() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[inputs.least]
kind = "value"
value = 10

[[nodes]]
id = "patches"
op = "table.of"
params = { columns = ["class", "area", "edge"], rows = [[1, 12, 4], [3, 20, 9], [3, 5, 2], [3, 40, 11]] }

[[nodes]]
id = "big"
op = "table.filter"
inputs = { table = "patches.table", least = "$least" }
params = { expression = "class == 3 and area >= least" }

[[nodes]]
id = "ratio"
op = "table.calc"
inputs = { table = "patches.table" }
params = { column = "ratio", expression = "edge / area if area > 0 else None" }

[[nodes]]
id = "sums"
op = "table.aggregate"
inputs = { table = "ratio.table" }
params = { by = "class", stats = "sum, sd" }

[[nodes]]
id = "top"
op = "table.sort"
inputs = { table = "ratio.table" }
params = { by = "area", descending = true, top = 2 }

[[nodes]]
id = "costs"
op = "table.of"
params = { columns = ["id", "cost"], rows = [[1, 100], [3, 10]] }

[[nodes]]
id = "joined"
op = "table.join"
inputs = { left = "top.table", right = "costs.table" }
params = { on = "class", right_on = "id", how = "full" }

[[nodes]]
id = "wide"
op = "table.pivot"
inputs = { table = "patches.table" }
params = { direction = "wider", id = "class", names = "edge", values = "area", fill = 0 }

[[nodes]]
id = "kept"
op = "table.select"
inputs = { table = "joined.table" }
params = { columns = ["edge", "class"], rename = { edge = "perimeter" } }

[outputs.kept]
from = "kept.table"
"#,
        )
        .unwrap();
        let s = script(&m, &r, None);
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}", s.left);
        assert!(
            t.contains("big = _table_filter(patches, \"class == 3 and area >= least\", inputs={\"least\": least})\n"),
            "{t}"
        );
        assert!(t.contains("ratio = _table_calc(patches, \"ratio\", \"edge / area if area > 0 else None\")\n"), "{t}");
        assert!(t.contains("sums = _aggregate(ratio, by=\"class\", stats=\"sum, sd\")\n"), "{t}");
        assert!(t.contains("top = _sort(ratio, \"area\", descending=True, top=2)\n"), "{t}");
        assert!(t.contains("joined = _join(top, costs, \"class\", right_on=\"id\", how=\"full\")\n"), "{t}");
        assert!(
            t.contains("wide = _pivot(patches, \"wider\", id=\"class\", names=\"edge\", values=\"area\", fill=0)\n"),
            "{t}"
        );
        assert!(
            t.contains("kept = _select(joined, columns=[\"edge\", \"class\"], rename={\"edge\": \"perimeter\"})\n")
        );
        assert!(t.contains("ls.save(kept, OUT / \"kept\")"), "{t}");
        // the expressions need both helpers: the tables' and the calculator's
        assert!(t.contains("def _calc(script, **inputs):") && t.contains("def _evaluate(frame, script"), "{t}");
        parses(t);
    }

    #[test]
    fn features_as_geopandas() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "plots"
op = "vector.read"
params = { path = "data/plots.gpkg", layer = "plots" }

[[nodes]]
id = "forest"
op = "vector.filter"
inputs = { vector = "plots.vector" }
params = { expression = 'uso == "forest" and geometry.area > 100' }

[[nodes]]
id = "dense"
op = "vector.calc"
inputs = { vector = "plots.vector" }
params = { expression = "d = pop / geometry.area\nout dense = d > 1" }

[[nodes]]
id = "ring"
op = "vector.buffer"
inputs = { vector = "plots.vector" }
params = { distance = 50, segments = 8 }

[[nodes]]
id = "land"
op = "nlm.random"
params = { columns = 20, rows = 10 }

[[nodes]]
id = "at"
op = "vector.extract"
inputs = { vector = "plots.vector", map = "land.map" }
params = { method = "bilinear", block_rows = 64 }

[[nodes]]
id = "zones"
op = "vector.zonal"
inputs = { vector = "ring.vector", map = "land.map" }
params = { stats = ["mean", "max"] }

[[nodes]]
id = "burnt"
op = "vector.rasterize"
inputs = { vector = "plots.vector", like = "land.map" }
params = { field = "pop", merge = "add" }

[[nodes]]
id = "counted"
op = "vector.spatial_join"
inputs = { vector = "ring.vector", other = "at.vector" }
params = { mode = "summary", fields = "value" }

[[nodes]]
id = "joined"
op = "vector.join"
inputs = { vector = "plots.vector", table = "zones.table" }

[[nodes]]
id = "near"
op = "vector.distance"
inputs = { vector = "plots.vector", like = "land.map" }

[[nodes]]
id = "squares"
op = "vector.grid_zones"
inputs = { map = "land.map" }
params = { size = 5 }

[[nodes]]
id = "heat"
op = "vector.density"
inputs = { vector = "plots.vector", map = "land.map" }

[[nodes]]
id = "sums"
op = "vector.exact_sum"
inputs = { vector = "plots.vector", map = "land.map" }

[[nodes]]
id = "areas"
op = "vector.geometry"
inputs = { vector = "plots.vector" }
params = { geodesic = true }

[outputs.counted]
from = "counted.table"
"#,
        )
        .unwrap();
        let s = script(&m, &r, None);
        let t = &s.text;
        assert!(t.contains("plots = ls.load_vector(MODEL_DIR / \"data/plots.gpkg\", layer=\"plots\")\n"), "{t}");
        assert!(
            t.contains("forest = _features_filter(plots, \"uso == \\\"forest\\\" and geometry.area > 100\")\n"),
            "{t}"
        );
        // a script of several lines as it is
        assert!(
            t.contains("dense = _features_calc(plots, \"\"\"\nd = pop / geometry.area\nout dense = d > 1\n\"\"\")\n")
        );
        assert!(t.contains("ring = _buffer(plots, 50, segments=8)\n"), "{t}");
        assert!(
            t.contains("# (not passed: block_rows = 64)\nat = _extract(land, plots, method=\"bilinear\")\n"),
            "{t}"
        );
        assert!(t.contains("zones = _attributes(ls.zonal_stats(land, ring.geometry, stats=[\"mean\", \"max\"]))\n"));
        assert!(t.contains("burnt = ls.rasterize(plots, land, field=\"pop\", merge=\"add\")\n"), "{t}");
        // features' table: their attributes
        assert!(t.contains("counted = _spatial_join(ring, at, mode=\"summary\", fields=\"value\")\n"), "{t}");
        assert!(t.contains("ls.save(_attributes(counted), OUT / \"counted\")"), "{t}");
        assert!(t.contains("joined = _join_features(plots, zones)\n"), "{t}");
        assert!(t.contains("near = _distance(plots, land)\n"), "{t}");
        assert!(t.contains("squares = _squares(land, 5)\n"), "{t}");
        // no counterpart, or a setting that would change the result: left, said why
        let left: Vec<&str> = s.left.iter().map(|(id, _, _)| id.as_str()).collect();
        assert_eq!(left, ["heat", "sums", "areas"], "{:?}", s.left);
        assert!(s.left[0].2.contains("kernel density") && s.left[2].2.contains("geodesic"), "{:?}", s.left);
        assert!(t.contains("def _evaluate(frame, script") && t.contains("def _calc(script, **inputs):"));
        parses(t);
        // no value where no feature is; a table's missing values
        let m = Model::from_json(
            r#"{"format": "terraflux/1", "nodes": [
                {"id": "plots", "op": "vector.read", "params": {"path": "C:/data/plots.gpkg"}},
                {"id": "land", "op": "nlm.random"},
                {"id": "burnt", "op": "vector.rasterize", "inputs": {"vector": "plots.vector", "like": "land.map"},
                 "params": {"fill": null, "all_touched": true}},
                {"id": "t", "op": "table.of", "params": {"columns": ["a"], "rows": [[1], [null]]}},
                {"id": "pts", "op": "vector.from_table", "inputs": {"table": "t.table"}, "params": {"x": "a", "y": "a"}}
            ]}"#,
        )
        .unwrap();
        let t = script(&m, &r, None).text;
        assert!(t.contains("plots = ls.load_vector(Path(\"C:/data/plots.gpkg\"))\n"), "{t}");
        assert!(t.contains("burnt = ls.rasterize(plots, land, fill=np.nan, all_touched=True)\n"), "{t}");
        assert!(t.contains("t = _table([\"a\"], [[1], [None]])\n"), "{t}");
        assert!(t.contains("pts = _points_from(t, x=\"a\", y=\"a\")\n"), "{t}");
        // only the helpers of features: not the calculator's
        assert!(!t.contains("def _calc("), "{t}");
        parses(&t);
    }

    #[test]
    fn land_change_as_landschaft() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "a"
op = "nlm.random"

[[nodes]]
id = "b"
op = "nlm.random"

[[nodes]]
id = "drivers"
op = "stack.of"
inputs = { slope = "a.map", dist = "b.map" }

[[nodes]]
id = "glm"
op = "glm.fit"
inputs = { landscape = "a.map", drivers = "drivers.stack" }
params = { formulas = { "10" = ["slope"], "2" = ["slope", "dist"] }, maxit = 30 }

[[nodes]]
id = "suitability"
op = "glm.predict"
inputs = { model = "glm.model", drivers = "drivers.stack", landscape = "a.map" }

[[nodes]]
id = "given"
op = "glm.coefficients"
params = { fits = [{ class = 1, drivers = ["slope"], coefficients = [-1.5, 0.25], pivot = [1, 2], rank = 2 }] }

[[nodes]]
id = "aliased"
op = "glm.coefficients"
params = { fits = [{ class = 1, drivers = ["slope", "dist"], coefficients = [-1.5, 0.25, 0], pivot = [1, 3, 2], rank = 2 }] }

[[nodes]]
id = "cost"
op = "dinamica.cost_map"
inputs = { sources = "a.map", friction = "b.map" }
params = { diagonal = false }

[[nodes]]
id = "ends"
op = "dinamica.destinations"
inputs = { attraction = "b.map", seed = 3, step = 2 }
params = { quadrants = [2, 3], min_distance = 50 }

[[nodes]]
id = "paths"
op = "dinamica.pathways"
inputs = { network = "a.map", sources = "ends.map", directions = "cost.direction" }
params = { merge = false }

[[nodes]]
id = "roads"
op = "dinamica.roads"
inputs = { network = "a.map", friction = "b.map", attraction = "b.map", step = 4 }
params = { distance = [100, 500], spacing = 200 }

[[nodes]]
id = "logged"
op = "dinamica.continuous_patcher"
inputs = { stock = "a.map", probability = "b.map", quota = 500 }
params = { mean = 4, unit = "cells", window = [5, 5] }

[[nodes]]
id = "wanted"
op = "demand.table"
params = { table = { classes = [1, 2], rows = [[5, 5], [4, 6]] } }

[[nodes]]
id = "observed"
op = "demand.from_maps"
inputs = { maps = "drivers.stack" }
params = { times = [2000, 2010] }

[[nodes]]
id = "pairs"
op = "table.of"
params = { columns = ["i", "j", "x"], rows = [[1, 2, 0.5], [2, 1, 0.5]] }

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { cost = "a.map", features = "drivers.stack", connectivity_data = "pairs.table" }
params = { boundary_penalty = 0.1, connectivity_penalty = 2, relative_targets = 0.2 }

[[nodes]]
id = "zoned"
op = "prioritize.optimize"
inputs = { zone_costs = "drivers.stack", features = "drivers.stack" }

[outputs.near]
from = "roads.distance"
"#,
        )
        .unwrap();
        let s = script(&m, &r, None);
        let t = &s.text;
        // the classes in their order, as Terraflux fits them
        assert!(
            t.contains("glm = ls.simulate.LogisticRegression({2: [\"slope\", \"dist\"], 10: [\"slope\"]}, maxit=30).fit(a, drivers)\n"),
            "{t}"
        );
        assert!(t.contains("suitability = _by_class(glm.predict(drivers, landscape=a))\n"), "{t}");
        assert!(
            t.contains("given = ls.simulate.LogisticRegression.from_coefficients({\n    1: {\"(Intercept)\": -1.5, \"slope\": 0.25},\n})\n"),
            "{t}"
        );
        assert!(t.contains("cost = ls.simulate.friction_cost(a, b, diagonal=False, directions=True)\n"), "{t}");
        // the seed of a step as Terraflux makes it
        assert!(
            t.contains("ends = ls.simulate.destinations(b, [2, 3], min_distance=50, seed=3 + 2 * 0x9E3779B9)\n"),
            "{t}"
        );
        assert!(t.contains("paths = ls.simulate.pathways(a, ends, directions=cost[1], merge=False, seed=1)\n"), "{t}");
        assert!(
            t.contains("roads = ls.simulate.RoadBuilder(b, b, distance=[100, 500], spacing=200).step(a, seed=(1 + 4 * 0x9E3779B9) & 0x7FFFFFFFFFFFFFFF)\n"),
            "{t}"
        );
        assert!(t.contains("ls.save(_road_distance(roads), OUT / \"near\""), "{t}");
        assert!(
            t.contains("logged = ls.simulate.ContinuousPatcher(mean=4, unit=\"cells\", window=[5, 5]).allocate(a, b, 500, seed=1)\n"),
            "{t}"
        );
        assert!(t.contains("wanted = _demand_table({\"classes\": [1, 2], \"rows\": [[5, 5], [4, 6]]})\n"), "{t}");
        assert!(t.contains("observed = ls.simulate.AreaDemand.from_maps(drivers, times=[2000, 2010])\n"), "{t}");
        assert!(
            t.contains(
                "    .add_boundary_penalties(0.1)\n    .add_connectivity_penalties(2, _unit_pairs(pairs, a.size))\n"
            ),
            "{t}"
        );
        // a model of not full rank, and zones: left, said why
        let left: Vec<&str> = s.left.iter().map(|(id, _, _)| id.as_str()).collect();
        assert_eq!(left, ["aliased", "zoned"], "{:?}", s.left);
        assert!(s.left[0].2.contains("full rank") && s.left[1].2.contains("zone_costs"), "{:?}", s.left);
        parses(t);
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
op = "raster.sieve"
inputs = { map = "classes.map" }

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

    #[test]
    fn containers_as_python() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[inputs.method]
kind = "value"
value = "near"

[[nodes]]
id = "land"
op = "nlm.random"
params = { columns = 20, rows = 10 }

[[nodes]]
id = "growth"
op = "control.repeat"
inputs = { times = 5, city = "land.map" }

[[nodes.body.nodes]]
id = "grow"
op = "raster.calc"
inputs = { c = "@city", s = "@step" }
params = { expression = "c + s" }

[[nodes.body.nodes]]
id = "full"
op = "value.calc"
params = { expression = "1 > 2" }

[nodes.body]
carry = { city = "grow.map" }
collect = { maps = "grow.map" }
until = "full.value"

[[nodes]]
id = "per"
op = "control.each"
inputs = { over = "growth.maps" }

[[nodes.body.nodes]]
id = "twice"
op = "raster.calc"
inputs = { m = "@item" }
params = { expression = "m * 2" }

[nodes.body]
collect = { doubled = "twice.map" }

[[nodes]]
id = "pick"
op = "control.switch"
inputs = { case = "$method", map = "growth.city" }

[[nodes.body.nodes]]
id = "near"
op = "control.group"
inputs = { m = "@map" }

[[nodes.body.nodes.body.nodes]]
id = "half"
op = "raster.calc"
inputs = { m = "@m" }
params = { expression = "m / 2" }

[nodes.body.nodes.body]
outputs = { map = "half.map" }

[[nodes.body.nodes]]
id = "case_3"
op = "control.group"
inputs = { m = "@map" }

[nodes.body.nodes.body]
outputs = { map = "@m" }

[[nodes.body.nodes]]
id = "otherwise"
op = "control.group"
inputs = { m = "@map" }

[nodes.body.nodes.body]
outputs = { map = "@m" }

[outputs.picked]
from = "pick.map"
"#,
        )
        .unwrap();
        let s = script(&m, &r, None);
        let t = &s.text;
        // a loop: its first value, a list of what it collects, the steps, its end, what it carries
        assert!(t.contains("growth_city = land\n"), "{t}");
        assert!(t.contains("growth_maps = []\nfor step in range(1, int(5) + 1):\n"), "{t}");
        assert!(t.contains("    grow = _calc(") && t.contains("c=growth_city, s=step,"), "{t}");
        // its end, of a node left as a comment: None, said (the loop kept)
        assert!(t.contains("    # (full.value: not translated: None here)\n    if None:  # the loop's end"), "{t}");
        assert!(t.contains("    growth_maps.append(grow[\"map\"])\n    growth_city = grow[\"map\"]\n"), "{t}");
        // over a collection: its elements, gathered
        assert!(t.contains("per_elements = _elements(growth_maps)\n"), "{t}");
        assert!(t.contains("for index, (name, item) in enumerate(per_elements, start=1):\n"), "{t}");
        assert!(t.contains("per_doubled = _gather(per_doubled, [k for k, _ in per_elements], \"item\")"), "{t}");
        // a switch: a branch each, its outputs the switch's
        assert!(t.contains("pick_case = _case(method)\n"), "{t}");
        assert!(t.contains("if pick_case == \"near\":\n"), "{t}");
        assert!(t.contains("elif pick_case in (\"case_3\", \"3\"):\n"), "{t}");
        assert!(t.contains("else:\n    # otherwise (control.group)"), "{t}");
        assert!(t.contains("    pick = half[\"map\"]\n    pick_branch = \"near\"\n"), "{t}");
        assert!(t.contains("    otherwise_m = pick_map\n    pick = otherwise_m\n"), "{t}");
        assert!(t.contains("ls.save(pick, OUT / \"picked\")"), "{t}");
        assert!(t.contains("def _elements(v):") && t.contains("def _gather(values, names, column):"));
        // value.calc has no counterpart: left, by its place in the loop; the containers translated
        assert!(s.left.iter().any(|(id, _, _)| id == "growth/full"), "{:?}", s.left);
        assert_eq!(s.left.len(), 1, "{:?}", s.left);
        parses(t);
    }

    #[test]
    fn models_of_species_plans_metrics_and_change() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "a"
op = "nlm.random"

[[nodes]]
id = "b"
op = "nlm.random"

[[nodes]]
id = "env"
op = "stack.of"
inputs = { temp = "a.map", "1_to_2" = "b.map" }

[[nodes]]
id = "occ"
op = "sdm.background"
inputs = { env = "env.stack" }
params = { n = 50 }

[[nodes]]
id = "data"
op = "sdm.prepare"
inputs = { occurrences = "occ.points", env = "env.stack" }
params = { clean = false, folds = 4 }

[[nodes]]
id = "fit"
op = "sdm.fit"
inputs = { records = "data.records" }
params = { model = "glm", C = 0.5 }

[[nodes]]
id = "map"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "data.env" }

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { cost = "a.map", features = "env.stack" }
params = { objective = "max_utility", budget = 100, boundary_penalty = 0.1, gap = 0.01 }

[[nodes]]
id = "metrics"
op = "metrics.landscape"
inputs = { map = "a.map" }
params = { type = "diversity", directions = 4 }

[[nodes]]
id = "markov"
op = "markov.fit"
inputs = { t0 = "a.map", t1 = "b.map", steps = 5 }

[[nodes]]
id = "step"
op = "dinamica.step"
"#,
        )
        .unwrap();
        let s = script(&m, &r, None);
        let t = &s.text;
        assert!(t.contains("env = _stack({\"1_to_2\": b, \"temp\": a})"), "{t}");
        assert!(t.contains("occ = ls.sdm.background(env, n=50, seed=1)"), "{t}");
        assert!(
            t.contains("data = ls.sdm.prepare(occ, env, clean=false, folds=4)".replace("false", "False").as_str()),
            "{t}"
        );
        assert!(t.contains("fit = ls.sdm.fit(data, {\"glm\": make_pipeline(StandardScaler(), LogisticRegression(C=0.5, max_iter=1000))})"), "{t}");
        assert!(t.contains("map_ = ls.sdm.predict(fit, env.sel(band=data.predictors), ensemble=None, sd=False).isel(band=0, drop=True)"), "{t}");
        assert!(t.contains("    ls.prioritize.problem(a, env)\n    .add_max_utility_objective(100)\n    .add_boundary_penalties(0.1)\n    .add_binary_decisions()\n    .add_highs_solver(gap=0.01)\n    .solve()\n"), "{t}");
        assert!(
            t.contains(
                "metrics = ls.landscape_metrics(a, level=\"landscape\", directions=4, type=\"diversity metric\")"
            ),
            "{t}"
        );
        assert!(t.contains("markov = ls.simulate.Markov.from_maps(a, b, steps=5)"), "{t}");
        // the allocation: landschaft runs it as a whole model
        let step = s.left.iter().find(|(id, _, _)| id == "step").unwrap();
        assert!(step.2.contains("ls.simulate.run(t0, t1, drivers, preset=\"dinamica\""), "{}", step.2);
    }
}
