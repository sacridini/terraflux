//! Biodiverse's spatial conditions (landschaft's bio/_conditions.py): which groups are neighbours of which. The
//! Perl is parsed, what depends on one group only (text and regex matches, polygons, richness) is evaluated
//! once per group here, and the rest runs in landschaft's C++ for every candidate pair with Biodiverse's
//! arithmetic.

use std::collections::{BTreeSet, HashMap};
use std::sync::LazyLock;

use regex::Regex;
use terraflux_ls_sys::ffi;

use super::basedata::BaseData;
use super::names::py_float;
use super::perl::{Ast, parse};
use crate::{Error, Result};

/// Math::Trig::deg2rad (reduced modulo 2 pi), with Math::Complex's pi (4 atan2(1, 1)).
fn deg2rad(deg: f64) -> f64 {
    let pi2 = 2.0 * (4.0 * 1f64.atan2(1.0));
    let d = (pi2 / 360.0) * deg;
    d - pi2 * (d / pi2).trunc()
}

fn op(name: &str) -> i32 {
    [
        "Num", "Var", "IdxVar", "Neg", "Not", "Add", "Sub", "Mul", "Div", "Mod", "Pow", "Lt", "Le", "Gt", "Ge", "Eq",
        "Ne", "Cmp", "And", "Or", "Dor", "Tern", "Func", "StrEq", "StrNe", "Call", "Local", "SetLocal", "Seq",
        "StrConst", "CoordId", "AxisStr", "Undef",
    ]
    .iter()
    .position(|o| *o == name)
    .unwrap() as i32
}

fn var_id(name: &str) -> Option<i32> {
    ["D", "C", "Dsqr", "Csqr", "x", "y", "z", "nbr_x", "nbr_y", "nbr_z"]
        .iter()
        .position(|v| *v == name)
        .map(|p| p as i32)
}

fn arr_id(name: &str) -> Option<i32> {
    ["d", "D", "c", "C", "coord", "nbrcoord"].iter().position(|v| *v == name).map(|p| p as i32)
}

fn func_id(name: &str) -> Option<i32> {
    ["abs", "sqrt", "sin", "cos", "log", "exp", "int", "atan2"].iter().position(|v| *v == name).map(|p| p as i32)
}

fn binop(op: &str) -> Option<&'static str> {
    Some(match op {
        "+" => "Add",
        "-" => "Sub",
        "*" => "Mul",
        "/" => "Div",
        "%" => "Mod",
        "**" => "Pow",
        "<" => "Lt",
        "<=" => "Le",
        ">" => "Gt",
        ">=" => "Ge",
        "==" => "Eq",
        "!=" => "Ne",
        "<=>" => "Cmp",
        "&&" | "and" => "And",
        "||" | "or" => "Or",
        "//" => "Dor",
        _ => return None,
    })
}

fn call_kind(name: &str) -> i32 {
    [
        "Circle",
        "CircleCell",
        "Rectangle",
        "Annulus",
        "Square",
        "SquareCell",
        "Ellipse",
        "Block",
        "SelfOnly",
        "SelectAll",
        "Side",
        "Mask",
        "Const",
        "SameFeature",
        "ExplicitSet",
        "ElementIs",
        "TextEq",
    ]
    .iter()
    .position(|c| *c == name)
    .unwrap() as i32
}

/// The value of an expression without variables.
#[derive(Debug, Clone)]
enum Const {
    Num(f64),
    Str(String),
    Regex(Regex),
    None,
    List(Vec<Const>),
}

impl Const {
    /// Python's float() of the value.
    fn float(&self) -> Result<f64> {
        match self {
            Const::Num(v) => Ok(*v),
            Const::Str(s) => {
                let t = s.trim();
                match t.to_lowercase().trim_start_matches(['+', '-']) {
                    "inf" | "infinity" => Ok(if t.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY }),
                    "nan" => Ok(f64::NAN),
                    _ => t.parse().map_err(|_| unsupported(format!("not a number: {s:?}"))),
                }
            }
            _ => Err(unsupported("not a number".into())),
        }
    }

    /// Python's truth of the value.
    fn truthy(&self) -> bool {
        match self {
            Const::Num(v) => *v != 0.0,
            Const::Str(s) => !s.is_empty(),
            Const::Regex(_) => true,
            Const::None => false,
            Const::List(l) => !l.is_empty(),
        }
    }

    /// Python's str() of the value.
    fn text(&self) -> String {
        match self {
            Const::Num(v) => py_float(*v),
            Const::Str(s) => s.clone(),
            Const::Regex(r) => r.as_str().to_string(),
            Const::None => "None".into(),
            Const::List(l) => format!("[{}]", l.iter().map(Const::text).collect::<Vec<_>>().join(", ")),
        }
    }

    fn list(&self) -> Result<Vec<f64>> {
        match self {
            Const::List(l) => l.iter().map(Const::float).collect(),
            _ => Err(unsupported("not a list".into())),
        }
    }
}

fn unsupported(m: String) -> Error {
    Error::msg(m)
}

/// A Perl regex as one of Rust's (the common subset).
fn perl_regex(pattern: &str, flags: &str) -> Result<Regex> {
    let mut f = String::new();
    for (c, g) in [('i', 'i'), ('x', 'x'), ('s', 's'), ('m', 'm')] {
        if flags.contains(c) {
            f.push(g);
        }
    }
    let pat = pattern.replace(r"\Z", r"\z");
    let full = if f.is_empty() { pat } else { format!("(?{f}){pat}") };
    Regex::new(&full).map_err(|e| Error::msg(format!("regex {pattern:?}: {e}")))
}

fn is_const(a: &Ast) -> bool {
    constant(a).is_ok()
}

fn constant(a: &Ast) -> Result<Const> {
    Ok(match a {
        Ast::Num(v) => Const::Num(*v),
        Ast::Str(s) => Const::Str(s.clone()),
        Ast::Regex(p, f) => Const::Regex(perl_regex(p, f)?),
        Ast::Undef => Const::None,
        Ast::List(items) => Const::List(items.iter().map(constant).collect::<Result<_>>()?),
        Ast::Un(op, x) => {
            let v = constant(x)?;
            match op.as_str() {
                "-" => Const::Num(-v.float()?),
                _ => Const::Num(if v.truthy() { 0.0 } else { 1.0 }),
            }
        }
        Ast::Bin(op, x, y) if ["+", "-", "*", "/", "**", "."].contains(&op.as_str()) => {
            let (x, y) = (constant(x)?, constant(y)?);
            if op == "." {
                return Ok(Const::Str(format!("{}{}", x.text(), y.text())));
            }
            let (x, y) = (x.float()?, y.float()?);
            Const::Num(match op.as_str() {
                "+" => x + y,
                "-" => x - y,
                "*" => x * y,
                "/" => {
                    if y == 0.0 {
                        return Err(Error::msg("division by zero"));
                    }
                    x / y
                }
                _ => x.powf(y),
            })
        }
        Ast::Call(name, items) if (name == "deg2rad" || name == "Math::Trig::deg2rad") && items.len() == 1 => {
            Const::Num(deg2rad(constant(&items[0])?.float()?))
        }
        Ast::Call(name, items) if name == "atan2" && items.len() == 2 => {
            Const::Num(constant(&items[0])?.float()?.atan2(constant(&items[1])?.float()?))
        }
        a => return Err(unsupported(format!("not a constant: {a:?}"))),
    })
}

/// The named arguments of a call (`key => value` pairs or a flat list).
fn call_args(items: &[Ast]) -> Result<HashMap<String, Ast>> {
    let mut out = HashMap::new();
    let mut flat = Vec::new();
    for it in items {
        match it {
            Ast::Pair(k, v) => {
                out.insert(constant(k)?.text(), (**v).clone());
            }
            x => flat.push(x.clone()),
        }
    }
    for k in (0..flat.len().saturating_sub(1)).step_by(2) {
        out.insert(constant(&flat[k])?.text(), flat[k + 1].clone());
    }
    Ok(out)
}

/// Half-widths per axis of the box holding every neighbour (inf: unbounded).
fn bounds(ast: &Ast, n_axes: usize) -> Vec<f64> {
    let inf = vec![f64::INFINITY; n_axes];
    fn per_axes(axes: &[f64], v: f64, n_axes: usize) -> Vec<f64> {
        let mut out = vec![f64::INFINITY; n_axes];
        for &k in axes {
            if k >= 0.0 && (k as usize) < n_axes {
                out[k as usize] = v;
            }
        }
        out
    }
    let rec = |a: &Ast| bounds(a, n_axes);
    match ast {
        Ast::Bin(op, x, y) => {
            let var_d = Ast::Var("D".into());
            match op.as_str() {
                "&&" | "and" => rec(x).iter().zip(rec(y)).map(|(p, q)| p.min(q)).collect(),
                "||" | "or" => rec(x).iter().zip(rec(y)).map(|(p, q)| p.max(q)).collect(),
                "<=" | "<" if **x == var_d && is_const(y) => {
                    constant(y).and_then(|c| c.float()).map_or(inf, |v| vec![v; n_axes])
                }
                ">=" | ">" if **y == var_d && is_const(x) => {
                    constant(x).and_then(|c| c.float()).map_or(inf, |v| vec![v; n_axes])
                }
                "<=" | "<" => match &**x {
                    Ast::Idx(name, idx) if name == "D" && is_const(y) && is_const(idx) => {
                        match (constant(idx).and_then(|c| c.float()), constant(y).and_then(|c| c.float())) {
                            (Ok(k), Ok(v)) => per_axes(&[k.trunc()], v, n_axes),
                            _ => inf,
                        }
                    }
                    _ => inf,
                },
                _ => inf,
            }
        }
        Ast::Call(name, items) => {
            let Ok(args) = call_args(items) else { return inf };
            let num = |k: &str| args.get(k).filter(|a| is_const(a)).and_then(|a| constant(a).ok()?.float().ok());
            let axes = || args.get("axes").and_then(|a| constant(a).ok()?.list().ok());
            match name.as_str() {
                "sp_circle" | "sp_annulus" => {
                    let key = if name == "sp_circle" { "radius" } else { "outer_radius" };
                    let Some(r) = num(key) else { return inf };
                    match axes() {
                        Some(ax) => per_axes(&ax.iter().map(|k| k.trunc()).collect::<Vec<_>>(), r, n_axes),
                        None if args.contains_key("axes") => inf,
                        None => vec![r; n_axes],
                    }
                }
                "sp_rectangle" => {
                    let Some(sizes) = args.get("sizes").and_then(|a| constant(a).ok()?.list().ok()) else { return inf };
                    let ax = if args.contains_key("axes") {
                        match axes() {
                            Some(a) => a,
                            None => return inf,
                        }
                    } else {
                        (0..sizes.len()).map(|k| k as f64).collect()
                    };
                    let mut out = inf.clone();
                    for (k, s) in ax.iter().zip(&sizes) {
                        let k = k.trunc();
                        if k >= 0.0 && (k as usize) < n_axes {
                            out[k as usize] = s / 2.0;
                        }
                    }
                    out
                }
                "sp_square" => num("size").map_or(inf, |s| vec![s / 2.0; n_axes]),
                "sp_ellipse" => {
                    let Some(r) = num("major_radius") else { return inf };
                    let ax = if args.contains_key("axes") {
                        match axes() {
                            Some(a) => a,
                            None => return inf,
                        }
                    } else {
                        vec![0.0, 1.0]
                    };
                    per_axes(&ax.iter().map(|k| k.trunc()).collect::<Vec<_>>(), r, n_axes)
                }
                "sp_block" => {
                    let Some(size) = args.get("size").filter(|a| is_const(a)).and_then(|a| constant(a).ok()) else {
                        return inf;
                    };
                    let sizes: Vec<Option<f64>> = match &size {
                        Const::List(l) => {
                            l.iter().map(|c| if let Const::None = c { None } else { c.float().ok() }).collect()
                        }
                        c => match c.float() {
                            Ok(v) => vec![Some(v); n_axes],
                            Err(_) => return inf,
                        },
                    };
                    let mut out: Vec<f64> = sizes.iter().take(n_axes).map(|s| s.unwrap_or(f64::INFINITY)).collect();
                    out.resize(n_axes, f64::INFINITY);
                    out
                }
                "sp_self_only" => vec![0.0; n_axes],
                _ => inf,
            }
        }
        Ast::Seq(items) => items.last().map_or(inf.clone(), rec),
        _ => inf,
    }
}

/// Biodiverse's result types of the sp_* functions.
fn sp_result_type(name: &str) -> Option<&'static str> {
    Some(match name {
        "sp_circle" | "sp_circle_cell" | "sp_rectangle" | "sp_annulus" => "circle",
        "sp_square" | "sp_square_cell" => "square",
        "sp_ellipse" => "ellipse",
        "sp_block" | "sp_match_regex" | "sp_points_in_same_poly_shape" => "non_overlapping",
        "sp_self_only" => "self_only",
        "sp_select_all" => "always_true",
        "sp_select_element"
        | "sp_richness_greater_than"
        | "sp_redundancy_greater_than"
        | "sp_point_in_poly"
        | "sp_point_in_poly_shape" => "always_same",
        "sp_select_sequence" => "subset",
        "sp_select_block" => "complex",
        "sp_is_left_of" | "sp_is_right_of" | "sp_in_line_with" => "side",
        "sp_match_text" => "text_match_exact",
        "sp_group_not_empty" => "",
        _ => return None,
    })
}

const SP_NAMES: [&str; 24] = [
    "sp_circle",
    "sp_circle_cell",
    "sp_rectangle",
    "sp_annulus",
    "sp_square",
    "sp_square_cell",
    "sp_ellipse",
    "sp_block",
    "sp_self_only",
    "sp_select_all",
    "sp_select_element",
    "sp_select_sequence",
    "sp_select_block",
    "sp_is_left_of",
    "sp_is_right_of",
    "sp_in_line_with",
    "sp_match_text",
    "sp_match_regex",
    "sp_group_not_empty",
    "sp_richness_greater_than",
    "sp_redundancy_greater_than",
    "sp_point_in_poly",
    "sp_point_in_poly_shape",
    "sp_points_in_same_poly_shape",
];

const RE_NUMBER: &str = r"[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?";

static BOUNDED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"\$nbr(?:_[xyz]|coord\[[+-]?\d+\])\s*(?:<|>|<=|>=|==)\s*(?:{RE_NUMBER})")).unwrap()
});
static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#[^\n]*\n").unwrap());
static LOOKS_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"^\s*{RE_NUMBER}\s*$")).unwrap());

fn perl_looks_like_number(s: &str) -> bool {
    LOOKS_NUMBER.is_match(s)
}

fn word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn full(re: &str) -> Regex {
    Regex::new(&format!("^(?:{re})$")).unwrap()
}

/// Biodiverse's result type of a condition text: the result types of the sp_* functions it calls (plus
/// non_overlapping for each `$nbr_x < number`-like term), else a guess from the text.
pub fn result_type(text: &str) -> String {
    let cond = format!("{}\n", text.trim());
    let cond = COMMENT.replace_all(&cond, "").to_string();
    let mut types = " non_overlapping".repeat(BOUNDED.find_iter(&cond).count());
    let mut names: Vec<&str> = SP_NAMES.to_vec();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let c: Vec<char> = cond.chars().collect();
    let mut pos = 0;
    'scan: while pos < c.len() {
        if pos == 0 || !word_char(c[pos - 1]) {
            for name in &names {
                let nc: Vec<char> = name.chars().collect();
                let end = pos + nc.len();
                if end <= c.len() && c[pos..end] == nc[..] && (end == c.len() || !word_char(c[end])) {
                    types.push(' ');
                    types.push_str(sp_result_type(name).unwrap());
                    pos = end;
                    while pos < c.len() && c[pos].is_whitespace() {
                        pos += 1;
                    }
                    if pos < c.len() && c[pos] == '(' {
                        let mut depth = 0;
                        while pos < c.len() {
                            if c[pos] == '(' {
                                depth += 1;
                            } else if c[pos] == ')' {
                                depth -= 1;
                                if depth == 0 {
                                    pos += 1;
                                    break;
                                }
                            }
                            pos += 1;
                        }
                    }
                    continue 'scan;
                }
            }
        }
        pos += 1;
    }
    let types = types.trim_start().to_string();
    if types.chars().any(|ch| !ch.is_whitespace()) {
        return types;
    }
    let c = cond.strip_suffix('\n').unwrap_or(&cond).to_string();
    let float = |s: &str| s.trim().parse::<f64>().unwrap_or(f64::NAN);
    if perl_looks_like_number(&c) && float(&c) != 0.0 {
        return "always_true".into();
    }
    if let Some(m) = full(&format!(r"\s*\$[DC]\s*>=\s*({RE_NUMBER})\s*(?:#.*)*")).captures(&c)
        && float(&m[1]) == 0.0
    {
        return "always_true".into();
    }
    let m = full(&format!(r"\s*({RE_NUMBER})+\s*(?:#.*)*"))
        .captures(&c)
        .or_else(|| full(&format!(r"\$[DC]\s*<\s*({RE_NUMBER})")).captures(&c));
    if m.is_some_and(|m| float(&m[1]) == 0.0) || c.is_empty() || full(r"[\r\n]+").is_match(&c) {
        return "always_false".into();
    }
    if let Some(m) = full(r"\$[DC]\s*<=+\s*(.+)").captures(&c)
        && perl_looks_like_number(&m[1])
    {
        return "circle".into();
    }
    if full(r"\$[DC][<=]=0").is_match(&c) {
        return "self_only".into();
    }
    if let Some(m) =
        full(&format!(r"\$[DC]\s*([<>])=\s*({RE_NUMBER})\s*(?:and|&&)\s*\$[DC]\s*([<>])=\s*({RE_NUMBER})\s*(?:#.*)*"))
            .captures(&c)
        && m[1] != m[3]
        && float(&m[2]) <= float(&m[4])
    {
        return "circle".into();
    }
    "complex".into()
}

/// Whether the neighbour lists of each set can be recycled (Biodiverse's).
fn recyclable_sets(types: &[String]) -> Vec<bool> {
    let mut out: Vec<bool> = Vec::new();
    for (i, t) in types.iter().enumerate() {
        let prev = if i > 0 { out[i - 1] } else { true };
        let ok = prev
            && match t.as_str() {
                "non_overlapping" => i == 0,
                "always_true" | "text_match_exact" | "always_same" => true,
                _ => false,
            };
        out.push(ok);
    }
    out
}

/// A spatial condition: its text and syntax tree.
#[derive(Debug, Clone)]
pub struct Condition {
    pub text: String,
    pub ast: Ast,
}

impl Condition {
    pub fn new(text: &str) -> Result<Condition> {
        let text = text.trim().to_string();
        let ast = parse(&text).map_err(|e| Error::msg(format!("cannot parse spatial condition {text:?}: {e}")))?;
        Ok(Condition { text, ast })
    }

    pub fn result_type(&self) -> String {
        result_type(&self.text)
    }

    /// The program for the C++ evaluator, and its string table.
    fn compile(&self, bd: &BaseData, procs: Option<&[i32]>) -> Result<(ffi::BioProgram, Vec<String>)> {
        let mut ctx = Context::new(bd, procs);
        let root = ctx.node(&self.ast)?;
        let c = &ctx.c;
        let mut iter_axes: Vec<i32> = c
            .euc_axes
            .iter()
            .chain(&c.cell_axes)
            .chain(&c.abs_cell_axes)
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        iter_axes.sort_by_key(|a| a.to_string());
        let n = bd.n_groups();
        let mut set_indptr_off = vec![0i64];
        let mut set_indices_off = vec![0i64];
        let (mut set_indptr, mut set_indices) = (Vec::new(), Vec::new());
        for (ip, ix) in &c.sets {
            set_indptr.extend_from_slice(ip);
            set_indices.extend_from_slice(ix);
            set_indptr_off.push(set_indptr.len() as i64);
            set_indices_off.push(set_indices.len() as i64);
        }
        let prog = ffi::BioProgram {
            ops: c.nodes.iter().map(|n| n.0).collect(),
            abc: c.nodes.iter().flat_map(|n| [n.1, n.2, n.3]).collect(),
            v: c.nodes.iter().map(|n| n.4).collect(),
            seq: c.seq.clone(),
            root,
            n_locals: c.locals.len() as i32,
            calls: c.calls.clone(),
            n_masks: c.masks.len() as i32,
            masks: c.masks.iter().flatten().copied().collect(),
            set_indptr,
            set_indptr_off,
            set_indices,
            set_indices_off,
            n_rotations: c.rotations.len() as i32,
            rotations: c.rotations.iter().flatten().copied().collect(),
            use_euc: c.use_euc,
            use_cell: c.use_cell,
            cell_lists: c.use_cell || !c.cell_axes.is_empty(),
            iter_axes,
            bounds: bounds(&self.ast, bd.n_axes()),
            self_only: false,
            always_true: false,
            always_false: false,
        };
        debug_assert!(c.masks.iter().all(|m| m.len() == n));
        Ok((prog, ctx.strings))
    }
}

#[derive(Default)]
struct Compiled {
    /// op, a, b, c, v
    nodes: Vec<(i32, i32, i32, i32, f64)>,
    seq: Vec<i32>,
    calls: Vec<ffi::BioCall>,
    masks: Vec<Vec<u8>>,
    sets: Vec<(Vec<i64>, Vec<i32>)>,
    rotations: Vec<Vec<f64>>,
    locals: HashMap<String, i32>,
    use_euc: bool,
    use_cell: bool,
    euc_axes: BTreeSet<i32>,
    cell_axes: BTreeSet<i32>,
    abs_cell_axes: BTreeSet<i32>,
}

struct Context<'a> {
    bd: &'a BaseData,
    c: Compiled,
    procs: Option<&'a [i32]>,
    strings: Vec<String>,
    string_ids: HashMap<String, i32>,
}

/// Math::Polygon's `contains` of many points (the boundary inside).
pub fn polygon_contains(points: &[[f64; 2]], polygon: &[[f64; 2]]) -> Result<Vec<bool>> {
    if polygon.len() < 3 {
        return Ok(vec![false; points.len()]);
    }
    if polygon[0] != polygon[polygon.len() - 1] {
        return Err(Error::msg("polygon must be closed: begin==end"));
    }
    Ok(points
        .iter()
        .map(|&[x, y]| {
            let (mut inside, mut px, mut py) = (false, polygon[0][0], polygon[0][1]);
            for &[nx, ny] in &polygon[1..] {
                if y == py && py == ny && (x >= px || x >= nx) && (x <= px || x <= nx) {
                    return true;
                }
                if x == px && px == nx && (y >= py || y >= ny) && (y <= py || y <= ny) {
                    return true;
                }
                let skip = py == ny || (y <= py && y <= ny) || (y > py && y > ny) || (x > px && x > nx);
                if !skip {
                    let xinters = (y - py) * (nx - px) / (ny - py) + px;
                    if px == nx || x <= xinters {
                        inside = !inside;
                    }
                }
                (px, py) = (nx, ny);
            }
            inside
        })
        .collect())
}

impl<'a> Context<'a> {
    fn new(bd: &'a BaseData, procs: Option<&'a [i32]>) -> Context<'a> {
        let mut ctx = Context { bd, c: Compiled::default(), procs, strings: Vec::new(), string_ids: HashMap::new() };
        for g in &bd.groups {
            ctx.sid(g);
        }
        ctx
    }

    fn sid(&mut self, s: &str) -> i32 {
        if let Some(&i) = self.string_ids.get(s) {
            return i;
        }
        let i = self.strings.len() as i32;
        self.strings.push(s.to_string());
        self.string_ids.insert(s.to_string(), i);
        i
    }

    fn add(&mut self, name: &str, a: i32, b: i32, c: i32, v: f64) -> i32 {
        self.c.nodes.push((op(name), a, b, c, v));
        self.c.nodes.len() as i32 - 1
    }

    fn num(&mut self, v: f64) -> i32 {
        self.add("Num", -1, -1, -1, v)
    }

    fn node(&mut self, a: &Ast) -> Result<i32> {
        Ok(match a {
            Ast::Num(v) => self.num(*v),
            Ast::Undef => self.add("Undef", -1, -1, -1, 0.0),
            Ast::Str(s) => {
                let id = self.sid(s);
                self.add("StrConst", id, -1, -1, 0.0)
            }
            Ast::Var(name) => {
                if let Some(&slot) = self.c.locals.get(name) {
                    return Ok(self.add("Local", slot, -1, -1, 0.0));
                }
                if name == "coord_id1" || name == "coord_id2" {
                    return Ok(self.add("CoordId", i32::from(name == "coord_id2"), -1, -1, 0.0));
                }
                let Some(id) = var_id(name) else { return Err(Error::msg(format!("unknown variable ${name}"))) };
                if name == "D" {
                    self.c.use_euc = true;
                } else if name == "C" {
                    self.c.use_cell = true;
                }
                self.add("Var", id, -1, -1, 0.0)
            }
            Ast::Idx(name, idx) => {
                let Some(id) = arr_id(name) else { return Err(Error::msg(format!("unknown array @{name}"))) };
                if ["d", "D", "c", "C"].contains(&name.as_str()) {
                    let k = constant(idx)
                        .and_then(|c| c.float())
                        .map_err(|_| Error::msg(format!("the subscript of ${name}[...] must be a constant")))?
                        .trunc() as i32;
                    match name.as_str() {
                        "d" | "D" => self.c.euc_axes.insert(k),
                        "c" => self.c.cell_axes.insert(k),
                        _ => self.c.abs_cell_axes.insert(k),
                    };
                }
                let b = self.node(idx)?;
                self.add("IdxVar", id, b, -1, 0.0)
            }
            Ast::Un(o, x) => {
                let x = self.node(x)?;
                self.add(if o == "-" { "Neg" } else { "Not" }, x, -1, -1, 0.0)
            }
            Ast::Bin(o, x, y) => match o.as_str() {
                "eq" | "ne" => {
                    let (x, y) = (self.str_node(x)?, self.str_node(y)?);
                    self.add(if o == "eq" { "StrEq" } else { "StrNe" }, x, y, -1, 0.0)
                }
                "=~" | "!~" => {
                    return Err(unsupported("regex matches outside sp_match_regex are not supported".into()));
                }
                "xor" => {
                    let (x, y) = (self.node(x)?, self.node(y)?);
                    let nx = self.add("Not", x, -1, -1, 0.0);
                    let ny = self.add("Not", y, -1, -1, 0.0);
                    self.add("Ne", nx, ny, -1, 0.0)
                }
                _ => {
                    let Some(name) = binop(o) else {
                        return Err(unsupported(format!("operator {o:?} is not supported")));
                    };
                    let (x, y) = (self.node(x)?, self.node(y)?);
                    self.add(name, x, y, -1, 0.0)
                }
            },
            Ast::Tern(c, x, y) => {
                let (c, x, y) = (self.node(c)?, self.node(x)?, self.node(y)?);
                self.add("Tern", c, x, y, 0.0)
            }
            Ast::My(name, x) => {
                let n = self.c.locals.len() as i32;
                let slot = *self.c.locals.entry(name.clone()).or_insert(n);
                let x = self.node(x)?;
                self.add("SetLocal", slot, x, -1, 0.0)
            }
            Ast::Seq(items) => {
                let kids: Vec<i32> = items.iter().map(|x| self.node(x)).collect::<Result<_>>()?;
                let start = self.c.seq.len() as i32;
                let len = kids.len() as i32;
                self.c.seq.extend(kids);
                self.add("Seq", start, len, -1, 0.0)
            }
            Ast::Call(name, items) => self.call(name, items)?,
            a => return Err(unsupported(format!("unsupported expression {a:?}"))),
        })
    }

    fn str_node(&mut self, a: &Ast) -> Result<i32> {
        if let Ast::Idx(name, idx) = a
            && (name == "coord" || name == "nbrcoord")
        {
            let b = self.node(idx)?;
            return Ok(self.add("AxisStr", i32::from(name == "nbrcoord"), b, -1, 0.0));
        }
        self.node(a)
    }

    fn add_call(&mut self, kind: &str, f: impl FnOnce(&mut ffi::BioCall)) -> i32 {
        let mut call = ffi::BioCall {
            kind: call_kind(kind),
            p: Vec::new(),
            axes: Vec::new(),
            sizes: Vec::new(),
            origin: Vec::new(),
            which: 0,
            index: -1,
            text: -1,
            axis: -1,
        };
        f(&mut call);
        self.c.calls.push(call);
        let i = self.c.calls.len() as i32 - 1;
        self.add("Call", i, -1, -1, 0.0)
    }

    fn const_call(&mut self, value: bool) -> i32 {
        self.add_call("Const", |c| c.p = vec![f64::from(u8::from(value))])
    }

    fn mask(&mut self, m: Vec<bool>, which: i32) -> i32 {
        self.c.masks.push(m.into_iter().map(u8::from).collect());
        let i = self.c.masks.len() as i32 - 1;
        self.add_call("Mask", |c| {
            c.index = i;
            c.which = which;
        })
    }

    fn which(args: &HashMap<String, Ast>) -> Result<i32> {
        let t = match args.get("type") {
            Some(a) => constant(a)?.text(),
            None => "nbr".into(),
        };
        match t.as_str() {
            "nbr" => Ok(0),
            "proc" => Ok(1),
            _ => Err(Error::msg(format!("type must be 'nbr' or 'proc', not {t:?}"))),
        }
    }

    /// The text each group offers to sp_match_text and sp_match_regex.
    fn comparators(&self, args: &HashMap<String, Ast>) -> Result<Vec<String>> {
        let bd = self.bd;
        if let Some(a) = args.get("axis") {
            let k = constant(a)?.float()?.trunc() as i64;
            let n = bd.n_axes() as i64;
            if k.abs() > n - 1 {
                return Err(Error::msg(format!("axis argument {k} beyond array bounds")));
            }
            let k = if k < 0 { (k + n) as usize } else { k as usize };
            return Ok((0..bd.n_groups()).map(|g| bd.axes_of(g)[k].clone()).collect());
        }
        Ok(bd.groups.clone())
    }

    fn call(&mut self, name: &str, items: &[Ast]) -> Result<i32> {
        if let Some(f) = func_id(name) {
            if name == "atan2" {
                if items.len() != 2 {
                    return Err(Error::msg("atan2 takes two arguments"));
                }
                let (b, c) = (self.node(&items[0])?, self.node(&items[1])?);
                return Ok(self.add("Func", f, b, c, 0.0));
            }
            if items.len() != 1 {
                return Err(Error::msg(format!("{name} takes one argument")));
            }
            let b = self.node(&items[0])?;
            return Ok(self.add("Func", f, b, -1, 0.0));
        }
        if name == "deg2rad" || name == "Math::Trig::deg2rad" {
            let v = constant(&Ast::Call("deg2rad".into(), items.to_vec()))?.float()?;
            return Ok(self.num(v));
        }
        if !name.starts_with("sp_") {
            return Err(unsupported(format!("function {name} is not supported")));
        }
        let args = call_args(items)?;
        let bd = self.bd;
        let n_axes = bd.n_axes();
        let get = |k: &str| -> Result<Const> {
            constant(args.get(k).ok_or_else(|| Error::msg(format!("{name}: argument {k} is missing")))?)
        };
        let axes_arg = |default: Option<Vec<i32>>| -> Result<Option<Vec<i32>>> {
            match args.get("axes") {
                None => Ok(default),
                Some(a) => match constant(a)? {
                    Const::List(l) => {
                        Ok(Some(l.iter().map(|c| c.float().map(|v| v.trunc() as i32)).collect::<Result<_>>()?))
                    }
                    _ => Err(Error::msg("axes argument must be an array ref")),
                },
            }
        };
        Ok(match name {
            "sp_circle" | "sp_annulus" => {
                let ax = axes_arg(None)?;
                match &ax {
                    None => self.c.use_euc = true,
                    Some(a) => self.c.euc_axes.extend(a),
                }
                let p = if name == "sp_circle" {
                    vec![get("radius")?.float()?]
                } else {
                    vec![get("inner_radius")?.float()?, get("outer_radius")?.float()?]
                };
                let kind = if name == "sp_circle" { "Circle" } else { "Annulus" };
                self.add_call(kind, |c| {
                    c.p = p;
                    c.axes = ax.unwrap_or_default();
                })
            }
            "sp_circle_cell" => {
                self.c.use_cell = true;
                let (r, ax) = (get("radius")?.float()?, axes_arg(None)?.unwrap_or_default());
                self.add_call("CircleCell", |c| {
                    c.p = vec![r];
                    c.axes = ax;
                })
            }
            "sp_rectangle" => {
                self.c.use_euc = true;
                let sizes = get("sizes")?.list()?;
                let ax = axes_arg(Some((0..sizes.len() as i32).collect()))?.unwrap();
                if ax.len() > sizes.len() {
                    return Err(Error::msg("Too many axes in call to sp_rectangle"));
                }
                self.add_call("Rectangle", |c| {
                    c.sizes = sizes;
                    c.axes = ax;
                })
            }
            "sp_square" => {
                let Const::Num(size) = get("size")? else {
                    return Err(Error::msg(
                        "Size argument to sp_square is not numeric.  Did you mean to use sp_rectangle?",
                    ));
                };
                self.c.use_euc = true;
                self.add_call("Square", |c| c.p = vec![size])
            }
            "sp_square_cell" => {
                self.c.use_cell = true;
                let size = get("size")?.float()?;
                self.add_call("SquareCell", |c| c.p = vec![size])
            }
            "sp_ellipse" => {
                let ax = axes_arg(Some(vec![0, 1]))?.unwrap();
                if ax.len() != 2 {
                    return Err(Error::msg(format!(
                        "sp_ellipse:  axes array needs two axes, you have given {}",
                        ax.len()
                    )));
                }
                self.c.euc_axes.extend(&ax);
                let angle = if args.contains_key("rotate_angle") {
                    get("rotate_angle")?.float()?
                } else if args.contains_key("rotate_angle_deg") {
                    deg2rad(get("rotate_angle_deg")?.float()?)
                } else {
                    0.0
                };
                let p = vec![get("major_radius")?.float()?, get("minor_radius")?.float()?, angle];
                self.add_call("Ellipse", |c| {
                    c.p = p;
                    c.axes = ax;
                })
            }
            "sp_block" => {
                let size = get("size")?;
                let mut sizes: Vec<f64> = match &size {
                    Const::List(l) => l
                        .iter()
                        .map(|c| if let Const::None = c { Ok(f64::NAN) } else { c.float() })
                        .collect::<Result<_>>()?,
                    c => vec![c.float()?; n_axes],
                };
                sizes.truncate(n_axes);
                let origin = match args.get("origin") {
                    None => Const::None,
                    Some(a) => constant(a)?,
                };
                let mut origin: Vec<f64> = match origin {
                    c if !c.truthy() => vec![0.0; n_axes],
                    Const::List(l) => l
                        .iter()
                        .map(|c| if let Const::None = c { Ok(0.0) } else { c.float() })
                        .collect::<Result<_>>()?,
                    c => vec![c.float()?; n_axes],
                };
                origin.truncate(n_axes);
                self.add_call("Block", |c| {
                    c.sizes = sizes;
                    c.origin = origin;
                })
            }
            "sp_self_only" => self.add_call("SelfOnly", |_| ()),
            "sp_select_all" => self.add_call("SelectAll", |_| ()),
            "sp_is_left_of" | "sp_is_right_of" | "sp_in_line_with" => {
                let ax = axes_arg(Some(vec![0, 1]))?.unwrap();
                if ax.len() != 2 {
                    return Err(Error::msg(format!(
                        "_sp_side:  axes array needs two axes, you have given {}",
                        ax.len()
                    )));
                }
                let va = if args.contains_key("vector_angle") {
                    get("vector_angle")?.float()?
                } else {
                    deg2rad(if args.contains_key("vector_angle_deg") { get("vector_angle_deg")?.float()? } else { 0.0 })
                };
                let angle = (4.0 * 1f64.atan2(1.0)) / 2.0 - va;
                let rot = ffi::bio_side_rotation(&bd.coords, n_axes as i32, ax[0], ax[1], angle)?;
                self.c.rotations.push(rot);
                let index = self.c.rotations.len() as i32 - 1;
                let want = match name {
                    "sp_is_left_of" => -1.0,
                    "sp_is_right_of" => 1.0,
                    _ => 0.0,
                };
                self.add_call("Side", |c| {
                    c.p = vec![want];
                    c.axes = ax;
                    c.index = index;
                })
            }
            "sp_select_element" => {
                let el = get("element")?.text();
                let idx = bd.group_index(&el).map_or(-1.0, |i| i as f64);
                let which = Self::which(&args)?;
                self.add_call("ElementIs", |c| {
                    c.p = vec![idx];
                    c.which = which;
                })
            }
            "sp_match_text" => {
                let which = Self::which(&args)?;
                let text =
                    args.get("text").ok_or_else(|| Error::msg("sp_match_text: argument text is missing"))?.clone();
                if is_const(&text) {
                    let t = constant(&text)?.text();
                    let m = self.comparators(&args)?.iter().map(|c| *c == t).collect();
                    return Ok(self.mask(m, which));
                }
                let mut axis = match args.get("axis") {
                    Some(a) => constant(a)?.float()?.trunc() as i32,
                    None => -1,
                };
                if axis < -1 {
                    axis += n_axes as i32;
                }
                let t = self.str_node(&text)?;
                self.add_call("TextEq", |c| {
                    c.text = t;
                    c.axis = axis;
                    c.which = which;
                })
            }
            "sp_match_regex" => {
                let which = Self::which(&args)?;
                let rx = match get("re")? {
                    Const::Regex(r) => r,
                    c => Regex::new(&c.text()).map_err(|e| Error::msg(format!("regex: {e}")))?,
                };
                let m = self.comparators(&args)?.iter().map(|c| rx.is_match(c)).collect();
                self.mask(m, which)
            }
            "sp_group_not_empty" | "sp_richness_greater_than" | "sp_redundancy_greater_than" => {
                let rich: Vec<f64> = bd.indptr.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
                let values: Vec<bool> = if name == "sp_group_not_empty" {
                    rich.iter().map(|r| *r > 0.0).collect()
                } else {
                    let thr = get("threshold")?.float()?;
                    if name == "sp_richness_greater_than" {
                        rich.iter().map(|r| *r > thr).collect()
                    } else {
                        let samples = bd.group_sample_count();
                        rich.iter().zip(&samples).map(|(r, s)| 1.0 - r / s > thr).collect()
                    }
                };
                if let Some(a) = args.get("element") {
                    let el = constant(a)?.text();
                    let Some(g) = bd.group_index(&el) else {
                        return Err(Error::msg(format!("element {el} is not in basedata")));
                    };
                    return Ok(self.const_call(values[g]));
                }
                self.mask(values, 0)
            }
            "sp_point_in_poly" => {
                let polygon: Vec<[f64; 2]> = match get("polygon")? {
                    Const::List(l) => l
                        .iter()
                        .map(|p| {
                            let v = p.list()?;
                            if v.len() < 2 { Err(Error::msg("polygon points need x and y")) } else { Ok([v[0], v[1]]) }
                        })
                        .collect::<Result<_>>()?,
                    _ => return Err(Error::msg("sp_point_in_poly: polygon must be a list of points")),
                };
                let mut which = 0;
                if let Some(pa) = args.get("point") {
                    match pa {
                        Ast::Aref(n) if n == "nbrcoord" || n == "coord" => which = i32::from(n == "coord"),
                        pa if is_const(pa) => {
                            let p = constant(pa)?.list()?;
                            let inside = polygon_contains(&[[p[0], p[1]]], &polygon)?[0];
                            return Ok(self.const_call(inside));
                        }
                        _ => {
                            return Err(unsupported(
                                "sp_point_in_poly: point must be \\@nbrcoord, \\@coord or a constant".into(),
                            ));
                        }
                    }
                }
                let pts: Vec<[f64; 2]> =
                    (0..bd.n_groups()).map(|g| [bd.coords[g * n_axes], bd.coords[g * n_axes + 1]]).collect();
                let m = polygon_contains(&pts, &polygon)?;
                self.mask(m, which)
            }
            "sp_select_sequence" => {
                let Some(procs) = self.procs else {
                    return Err(unsupported("sp_select_sequence needs the processing order".into()));
                };
                let set = select_sequence(bd, procs, &args)?;
                self.c.sets.push(set);
                let i = self.c.sets.len() as i32 - 1;
                self.add_call("ExplicitSet", |c| c.index = i)
            }
            _ => return Err(unsupported(format!("{name} is not supported yet"))),
        })
    }
}

/// sp_select_sequence: every `frequency`-th group, the offset cycling per processing group.
fn select_sequence(bd: &BaseData, procs: &[i32], args: &HashMap<String, Ast>) -> Result<(Vec<i64>, Vec<i32>)> {
    let get = |k: &str| args.get(k).map(constant).transpose();
    let spacing = get("frequency")?.ok_or_else(|| Error::msg("sp_select_sequence: frequency is missing"))?.float()?;
    let first = match get("first_offset")? {
        Some(c) if c.truthy() => c.float()?,
        _ => 0.0,
    };
    let cycle = get("cycle_offset")?.is_none_or(|c| c.truthy());
    let reverse = get("reverse_order")?.is_some_and(|c| c.truthy());
    let n_axes = bd.n_axes();
    // get_element_list_sorted: by coordinates (text axes as strings), ties in name order
    let mut order: Vec<usize> = (0..bd.n_groups()).collect();
    order.sort_by(|&a, &b| {
        for k in 0..n_axes {
            let o = if bd.cell_sizes[k] < 0.0 {
                bd.axes_of(a)[k].cmp(&bd.axes_of(b)[k])
            } else {
                bd.coords[a * n_axes + k].total_cmp(&bd.coords[b * n_axes + k])
            };
            if o.is_ne() {
                return o;
            }
        }
        std::cmp::Ordering::Equal
    });
    if reverse {
        order.reverse();
    }
    let mut by_group: HashMap<i32, Vec<i32>> = HashMap::new();
    let mut offset = first;
    for (k, &g) in procs.iter().enumerate() {
        if k > 0 {
            offset += 1.0;
            if cycle && offset >= spacing {
                offset = 0.0;
            }
        }
        let mut sel: BTreeSet<i32> = BTreeSet::new();
        let mut last = -1i64;
        let mut i = offset;
        while i <= order.len() as f64 - 1.0 {
            let ii = i.trunc() as i64;
            if ii != last {
                sel.insert(order[ii as usize] as i32);
                last = ii;
            }
            i += spacing;
        }
        by_group.insert(g, sel.into_iter().collect());
    }
    let mut indptr = vec![0i64; bd.n_groups() + 1];
    let mut data = Vec::new();
    for g in 0..bd.n_groups() {
        let r = by_group.get(&(g as i32)).cloned().unwrap_or_default();
        data.extend_from_slice(&r);
        indptr[g + 1] = indptr[g] + r.len() as i64;
    }
    Ok((indptr, data))
}

/// The groups as the C++ evaluator takes them, with the string table (group names, the text axes' values,
/// then each program's strings).
struct Payload {
    strings: Vec<String>,
    ids: HashMap<String, i32>,
    axis_str: Vec<i32>,
}

impl Payload {
    fn new(bd: &BaseData) -> Payload {
        let mut p =
            Payload { strings: Vec::new(), ids: HashMap::new(), axis_str: vec![-1; bd.n_groups() * bd.n_axes()] };
        for g in &bd.groups {
            p.id(g);
        }
        for k in 0..bd.n_axes() {
            if bd.cell_sizes[k] < 0.0 {
                for g in 0..bd.n_groups() {
                    p.axis_str[g * bd.n_axes() + k] = p.id(&bd.axes_of(g)[k]);
                }
            }
        }
        p
    }

    fn id(&mut self, s: &str) -> i32 {
        if let Some(&i) = self.ids.get(s) {
            return i;
        }
        let i = self.strings.len() as i32;
        self.strings.push(s.to_string());
        self.ids.insert(s.to_string(), i);
        i
    }

    /// A program's string constants renumbered in the payload's table.
    fn adopt(&mut self, prog: &mut ffi::BioProgram, strings: &[String]) {
        let remap: Vec<i32> = strings.iter().map(|s| self.id(s)).collect();
        let str_const = op("StrConst");
        for (k, o) in prog.ops.iter().enumerate() {
            if *o == str_const {
                prog.abc[3 * k] = remap[prog.abc[3 * k] as usize];
            }
        }
    }

    fn groups(&self, bd: &BaseData) -> ffi::BioGroups {
        ffi::BioGroups {
            n: bd.n_groups() as i64,
            ndim: bd.n_axes() as i32,
            coords: bd.coords.clone(),
            numeric: bd.cell_sizes.iter().map(|c| u8::from(*c >= 0.0)).collect(),
            axis_str: self.axis_str.clone(),
            name_str: (0..bd.n_groups() as i32).collect(),
            cell_sizes: bd.cell_sizes.clone(),
            strings: self.strings.clone(),
        }
    }
}

/// The groups passing a definition query.
pub fn definition_query(bd: &BaseData, query: &Condition, parallel: bool) -> Result<Vec<bool>> {
    let (mut prog, strings) = query.compile(bd, None)?;
    let mut payload = Payload::new(bd);
    payload.adopt(&mut prog, &strings);
    Ok(ffi::bio_definition_query(&payload.groups(bd), &prog, parallel)?.into_iter().map(|v| v != 0).collect())
}

/// A neighbour table: a row per processing group.
pub type Csr = (Vec<i64>, Vec<i32>);

fn rows_to_csr(rows: &[Vec<i32>]) -> Csr {
    let mut indptr = vec![0i64; rows.len() + 1];
    for (k, r) in rows.iter().enumerate() {
        indptr[k + 1] = indptr[k] + r.len() as i64;
    }
    (indptr, rows.concat())
}

fn row(c: &Csr, r: usize) -> &[i32] {
    &c.1[c.0[r] as usize..c.0[r + 1] as usize]
}

/// Row by row a - b.
fn csr_minus(a: &Csr, b: &Csr) -> Csr {
    let rows: Vec<Vec<i32>> = (0..a.0.len() - 1)
        .map(|r| {
            let other: BTreeSet<i32> = row(b, r).iter().copied().collect();
            row(a, r).iter().copied().filter(|x| !other.contains(x)).collect()
        })
        .collect();
    rows_to_csr(&rows)
}

const PURE_CALLS: [&str; 21] = [
    "sp_block",
    "sp_select_all",
    "sp_self_only",
    "sp_match_text",
    "sp_match_regex",
    "sp_select_element",
    "sp_richness_greater_than",
    "sp_redundancy_greater_than",
    "sp_group_not_empty",
    "sp_point_in_poly",
    "sp_point_in_poly_shape",
    "sp_circle",
    "sp_circle_cell",
    "sp_rectangle",
    "sp_annulus",
    "sp_square",
    "sp_square_cell",
    "sp_ellipse",
    "sp_is_left_of",
    "sp_is_right_of",
    "sp_in_line_with",
];

/// A single sp_* call whose recycled or cached lists equal every group's own.
fn is_pure(cond: &Condition) -> bool {
    let Ast::Call(name, items) = &cond.ast else { return false };
    if !PURE_CALLS.contains(&name.as_str()) {
        return false;
    }
    let Ok(args) = call_args(items) else { return false };
    match name.as_str() {
        "sp_match_text" | "sp_match_regex" | "sp_select_element" => {
            if let Some(t) = args.get("type")
                && let Ok(c) = constant(t)
                && c.text() == "proc"
            {
                return false;
            }
            !(name == "sp_match_text" && !args.get("text").is_some_and(is_const))
        }
        "sp_point_in_poly" | "sp_point_in_poly_shape" => {
            args.get("point").is_none_or(|p| *p == Ast::Aref("nbrcoord".into()))
        }
        "sp_richness_greater_than" | "sp_redundancy_greater_than" | "sp_group_not_empty" => true,
        _ => cond.result_type() != "always_same",
    }
}

/// The neighbour sets of the processing groups.
#[derive(Debug, Clone)]
pub struct Neighbourhoods {
    /// The processing groups (those passing the definition query), in name order.
    pub groups: Vec<i32>,
    /// One table per condition, rows as `groups`; the second leaves out the first.
    pub sets: Vec<Csr>,
    /// Whether Biodiverse copies results between groups with these neighbourhoods.
    pub results_recyclable: bool,
    /// The lists as Biodiverse leaves them stored after its analysis (recycling can overwrite lists of groups
    /// processed before), rows as `groups`.
    pub stored: Vec<Csr>,
}

/// The neighbour sets of every group as Biodiverse's spatial analysis finds them (landschaft's neighbours).
pub fn neighbours(
    bd: &BaseData,
    conditions: &[Condition],
    query: Option<&Condition>,
    parallel: bool,
    recycle: bool,
    results_recyclable: Option<bool>,
) -> Result<Neighbourhoods> {
    if conditions.is_empty() || conditions.len() > 2 {
        return Err(Error::msg("one or two spatial conditions are supported"));
    }
    let n = bd.n_groups();
    let passed = match query {
        Some(q) => definition_query(bd, q, parallel)?,
        None => vec![true; n],
    };
    let procs: Vec<i32> = (0..n as i32).filter(|&g| passed[g as usize]).collect();
    if procs.is_empty() {
        return Err(Error::msg("Nothing passed the definition query"));
    }
    let types: Vec<String> = conditions.iter().map(Condition::result_type).collect();
    let shortcut = |t: &str| ["self_only", "always_true", "always_false"].contains(&t);
    let same = |t: &str| t == "always_same" || t == "always_same_current_label";
    let mut payload = Payload::new(bd);
    let mut progs = Vec::new();
    for (c, t) in conditions.iter().zip(&types) {
        if shortcut(t) {
            progs.push(None);
            continue;
        }
        let (mut prog, strings) = c.compile(bd, Some(&procs))?;
        payload.adopt(&mut prog, &strings);
        progs.push(Some(prog));
    }
    let garr = payload.groups(bd);
    let mut own: Vec<Option<Csr>> = Vec::new();
    for (i, prog) in progs.iter().enumerate() {
        let Some(prog) = prog else {
            own.push(None);
            continue;
        };
        let rows = if same(&types[i]) { &procs[..1] } else { &procs[..] };
        let c = ffi::bio_find_neighbours(&garr, prog, rows, parallel)?;
        own.push(Some((c.indptr, c.indices)));
    }
    let recyclable = if recycle { recyclable_sets(&types) } else { vec![false; conditions.len()] };
    let results_recyclable =
        results_recyclable.unwrap_or_else(|| recyclable.iter().all(|r| *r)) && recyclable.iter().all(|r| *r);
    let mut row_of = vec![-1i64; n];
    for (r, &g) in procs.iter().enumerate() {
        row_of[g as usize] = r as i64;
    }
    let all_groups: Vec<i32> = (0..n as i32).collect();
    let own_list = |i: usize, g: i32| -> Vec<i32> {
        match types[i].as_str() {
            "self_only" => vec![g],
            "always_true" => all_groups.clone(),
            "always_false" => Vec::new(),
            t => {
                let c = own[i].as_ref().unwrap();
                if same(t) { row(c, 0).to_vec() } else { row(c, row_of[g as usize] as usize).to_vec() }
            }
        }
    };
    let simple = conditions.iter().all(is_pure) || (!recyclable.iter().any(|r| *r) && types.iter().all(|t| !same(t)));
    if simple {
        let mut sets: Vec<Csr> = Vec::new();
        for i in 0..conditions.len() {
            let mut cur = match &own[i] {
                Some(c) if !same(&types[i]) => c.clone(),
                _ => rows_to_csr(&procs.iter().map(|&g| own_list(i, g)).collect::<Vec<_>>()),
            };
            for prev in &sets {
                cur = csr_minus(&cur, prev);
            }
            sets.push(cur);
        }
        return Ok(Neighbourhoods { groups: procs, stored: sets.clone(), sets, results_recyclable });
    }
    let n_sets = conditions.len();
    let mut recycled: Vec<HashMap<i32, Vec<i32>>> = vec![HashMap::new(); n_sets];
    let mut stored: Vec<HashMap<i32, Vec<i32>>> = vec![HashMap::new(); n_sets];
    let mut used: HashMap<i32, Vec<Vec<i32>>> = HashMap::new();
    let mut has_lists = vec![false; n];
    let mut results_same = vec![-1i64; n];
    for &g in &procs {
        if results_recyclable && results_same[g as usize] >= 0 {
            let src = results_same[g as usize] as i32;
            let l = used[&src].clone();
            used.insert(g, l);
            continue;
        }
        let mut lists: Vec<Vec<i32>> = Vec::new();
        let mut excl: BTreeSet<i32> = BTreeSet::new();
        for (i, rec) in recycled.iter().enumerate() {
            let lst = match rec.get(&g) {
                Some(l) => l.clone(),
                None => {
                    let full = own_list(i, g);
                    if excl.is_empty() { full } else { full.into_iter().filter(|x| !excl.contains(x)).collect() }
                }
            };
            if i != n_sets - 1 {
                excl.extend(&lst);
            }
            stored[i].insert(g, lst.clone());
            lists.push(lst);
        }
        has_lists[g as usize] = true;
        let nbrs1 = lists[0].clone();
        if recyclable[0] {
            for &f in &nbrs1 {
                if f == g || has_lists[f as usize] {
                    continue;
                }
                let targets: Vec<i32> = nbrs1.iter().copied().filter(|&x| x != g && passed[x as usize]).collect();
                for i in 0..n_sets {
                    if !recyclable[i] {
                        break;
                    }
                    for &t in &targets {
                        recycled[i].insert(t, lists[i].clone());
                        stored[i].insert(t, lists[i].clone());
                    }
                }
                for &t in &targets {
                    has_lists[t as usize] = true;
                }
                break;
            }
        }
        if results_recyclable {
            for &x in &nbrs1 {
                if passed[x as usize] && results_same[x as usize] < 0 {
                    results_same[x as usize] = i64::from(g);
                }
            }
        }
        used.insert(g, lists);
    }
    let sets =
        (0..n_sets).map(|i| rows_to_csr(&procs.iter().map(|g| used[g][i].clone()).collect::<Vec<_>>())).collect();
    let stored = (0..n_sets)
        .map(|i| rows_to_csr(&procs.iter().map(|g| stored[i].get(g).cloned().unwrap_or_default()).collect::<Vec<_>>()))
        .collect();
    Ok(Neighbourhoods { groups: procs, sets, results_recyclable, stored })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_types_are_biodiverses() {
        assert_eq!(result_type("sp_circle (radius => 1)"), "circle");
        assert_eq!(result_type("sp_block (size => 3) && $nbr_x < 5"), "non_overlapping non_overlapping");
        assert_eq!(result_type("$D <= 5"), "circle");
        assert_eq!(result_type("$D == 0"), "complex");
        assert_eq!(result_type("$D<=0"), "circle");
        assert_eq!(result_type("$D==0"), "self_only");
        assert_eq!(result_type("1"), "always_true");
        assert_eq!(result_type("0"), "always_false");
        assert_eq!(result_type("sp_select_all()"), "always_true");
        assert_eq!(result_type("$D >= 1 && $D <= 3"), "circle");
    }
}
