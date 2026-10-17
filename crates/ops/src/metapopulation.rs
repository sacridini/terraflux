//! A metapopulation in a network of habitat patches, Hanski's incidence function model as MetaLandSim's spom (2.0)
//! simulates it: every step, each patch's connectivity to the occupied patches (a kernel of the distance times the
//! sources' areas), the colonisation of an empty patch and the extinction of an occupied one (smaller patches go
//! extinct sooner; with the rescue effect, less so when colonisation is likely), drawn with R's runif after
//! set.seed (landschaft's port of R's generator), so a seed gives MetaLandSim's run.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::RRng;
use terraflux_raster::vector::Geometry;

use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Metapopulation));
}

fn fail(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

/// The dispersal kernel of a distance: exp(-α d) (spom's op1) or 1 / (1 + α d^β) (op2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kernel {
    Exponential,
    Power { beta: f64 },
}

/// The connectivity: the kernel times the occupied patches' areas^b (op1), or that times the patch's area^c1 (op2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Connectivity {
    Plain,
    Area { c1: f64 },
}

/// The colonisation probability of a connectivity S: S² / (S² + y²) (op1), 1 - exp(-y S) (op2), or
/// S^z / (S^z + 1/c2) (op3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Colonisation {
    Logistic,
    Exponential,
    Power { c2: f64, z: f64 },
}

/// The extinction probability of an area A: min(1, e / A^x) (op1), 1 - exp(-e / A^x) (op2), or the rescue effect
/// min(1, e / A^x (1 - C)^R) (op3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Extinction {
    Area,
    Exponential,
    Rescue { r: f64 },
}

/// The model's functions and parameters (spom's param_df: alpha, x, y, e; and b).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ifm {
    pub kernel: Kernel,
    pub connectivity: Connectivity,
    pub colonisation: Colonisation,
    pub extinction: Extinction,
    pub alpha: f64,
    pub x: f64,
    pub y: f64,
    pub e: f64,
    pub b: f64,
}

/// R's `^` (R_POW: a square multiplied, a power of 1 or of a base 1 exactly 1).
fn rpow(x: f64, y: f64) -> f64 {
    if y == 2.0 {
        x * x
    } else if x == 1.0 || y == 0.0 {
        1.0
    } else {
        x.powf(y)
    }
}

impl Ifm {
    /// Every patch's colonisation and extinction probabilities given the patches occupied.
    pub fn probabilities(&self, xy: &[[f64; 2]], area: &[f64], occupied: &[bool]) -> (Vec<f64>, Vec<f64>) {
        let n = xy.len();
        let ab: Vec<f64> = area.iter().map(|a| rpow(*a, self.b)).collect();
        let mut col = Vec::with_capacity(n);
        let mut ext = Vec::with_capacity(n);
        for i in 0..n {
            let mut s = 0.0;
            for j in (0..n).filter(|&j| occupied[j] && j != i) {
                let (dx, dy) = (xy[i][0] - xy[j][0], xy[i][1] - xy[j][1]);
                let d = (dx * dx + dy * dy).sqrt();
                let k = match self.kernel {
                    Kernel::Exponential => (-self.alpha * d).exp(),
                    Kernel::Power { beta } => 1.0 / (1.0 + self.alpha * rpow(d, beta)),
                };
                s += k * ab[j];
            }
            if let Connectivity::Area { c1 } = self.connectivity {
                s *= rpow(area[i], c1);
            }
            let c = match self.colonisation {
                Colonisation::Logistic => rpow(s, 2.0) / (rpow(s, 2.0) + rpow(self.y, 2.0)),
                Colonisation::Exponential => 1.0 - (-self.y * s).exp(),
                Colonisation::Power { c2, z } => rpow(s, z) / (rpow(s, z) + (1.0 / c2)),
            };
            let ax = rpow(area[i], self.x);
            let e = match self.extinction {
                Extinction::Area => (self.e / ax).min(1.0),
                Extinction::Exponential => 1.0 - ((-self.e) / ax).exp(),
                Extinction::Rescue { r } => ((self.e / ax) * rpow(1.0 - c, r)).min(1.0),
            };
            col.push(c);
            ext.push(e);
        }
        (col, ext)
    }

    /// One step (spom): an occupied patch lost with probability (1 - C) E, an empty one colonised with C.
    pub fn step(&self, xy: &[[f64; 2]], area: &[f64], occupied: &[bool], rng: &mut RRng) -> Vec<bool> {
        let (c, e) = self.probabilities(xy, area, occupied);
        let u = rng.runif(occupied.len());
        (0..occupied.len())
            .map(|i| {
                let cond = if occupied[i] { (1.0 - c[i]) * e[i] } else { c[i] };
                if u[i] < cond { !occupied[i] } else { occupied[i] }
            })
            .collect()
    }
}

/// The patches' centres, areas and occupancy.
type Patches = (Vec<[f64; 2]>, Vec<f64>, Vec<bool>);

/// The patches of a node's port: a table (x, y, area and occupancy columns) or point features (their attributes).
fn patches_of(args: &Args, area_col: &str, occ_col: &str) -> Result<Patches> {
    let column = |t: &Table, name: &str| -> Result<Vec<f64>> {
        let k = t.columns.iter().position(|c| c == name).ok_or_else(|| fail(args, format!("patches: no {name}")))?;
        Ok(t.rows.iter().map(|r| r[k]).collect())
    };
    let (xy, t): (Vec<[f64; 2]>, Table) = match args.get("patches") {
        Some(Value::Table(t)) => {
            let (x, y) = (column(t, "x")?, column(t, "y")?);
            (x.into_iter().zip(y).map(|(a, b)| [a, b]).collect(), t.clone())
        }
        Some(Value::Vector(v)) => {
            let xy = v
                .geometries
                .iter()
                .map(|g| match g {
                    Geometry::Point(p) => Ok(*p),
                    _ => Err(fail(args, "patches: points (a patch's centre)")),
                })
                .collect::<Result<Vec<_>>>()?;
            (xy, crate::vector::attribute_table(v))
        }
        Some(v) => return Err(fail(args, format!("patches: a table or points, not a {:?}", v.ty()))),
        None => return Err(fail(args, "connect the patches")),
    };
    let area = column(&t, area_col)?;
    let occupied: Vec<bool> = column(&t, occ_col)?.iter().map(|v| *v != 0.0 && !v.is_nan()).collect();
    if xy.len() < 2 || area.iter().any(|a| a.is_nan() || *a <= 0.0) {
        return Err(fail(args, "patches: two at least, every one with an area above 0"));
    }
    Ok((xy, area, occupied))
}

struct Metapopulation;

impl Operator for Metapopulation {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "conn.metapopulation",
            "A metapopulation in a network of habitat patches over time, Hanski's incidence function model as R's \
             MetaLandSim (spom) simulates it, the same run for a seed: every step, each patch's connectivity to the \
             occupied patches (a kernel of the distance times their areas), an empty patch colonised with a \
             probability that grows with it, an occupied one lost with a probability that falls with its area (and \
             with the rescue effect, with its colonisation). Every patch's final occupancy and incidence (the share \
             of steps occupied), and the patches occupied at every step.",
            Access::None,
        )
        .input(PortSpec::new("patches", Type::Any, "the patches: a table (x, y, area, occupied) or points"))
        .output(PortSpec::new(
            "patches",
            Type::Table,
            "per patch: patch, x, y, area, occupied (at the end), incidence (the share of steps occupied)",
        ))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "per step (0 the start): step, occupied, occupied_area, colonised, extinct",
        ))
        .param(PortSpec::optional("steps", Type::Integer, "the steps (default 100)"))
        .param(PortSpec::new("alpha", Type::Number, "the kernel's α: 1 / the mean dispersal distance"))
        .param(PortSpec::new("x", Type::Number, "extinction's scaling with area (x)"))
        .param(PortSpec::new("y", Type::Number, "colonisation's y"))
        .param(PortSpec::new("e", Type::Number, "extinction's e: the probability of a patch of area 1"))
        .param(PortSpec::optional("b", Type::Number, "the sources' areas' exponent in the connectivity (default 1)"))
        .param(
            PortSpec::optional("kernel", Type::Text, "exponential (default) or power: 1 / (1 + α d^beta)")
                .choices(["exponential", "power"]),
        )
        .param(PortSpec::optional("beta", Type::Number, "the power kernel's beta (default 1)"))
        .param(
            PortSpec::optional("connectivity", Type::Text, "plain (default) or area: times the patch's area^c1")
                .choices(["plain", "area"]),
        )
        .param(PortSpec::optional("c1", Type::Number, "with area connectivity: the exponent"))
        .param(
            PortSpec::optional(
                "colonisation",
                Type::Text,
                "logistic (default, S²/(S²+y²)), exponential (1 - exp(-y S)) or power (S^z/(S^z + 1/c2))",
            )
            .choices(["logistic", "exponential", "power"]),
        )
        .param(PortSpec::optional("c2", Type::Number, "with power colonisation: c2"))
        .param(PortSpec::optional("z", Type::Number, "with power colonisation: z"))
        .param(
            PortSpec::optional(
                "extinction",
                Type::Text,
                "area (default, min(1, e/A^x)), exponential (1 - exp(-e/A^x)) or rescue (e/A^x (1 - C)^r)",
            )
            .choices(["area", "exponential", "rescue"]),
        )
        .param(PortSpec::optional("r", Type::Number, "with the rescue effect: its exponent"))
        .param(PortSpec::optional("area", Type::Text, "the patches' area column or attribute (default area)"))
        .param(PortSpec::optional("occupied", Type::Text, "the occupancy column or attribute (default occupied)"))
        .param(PortSpec::optional("seed", Type::Integer, "R's set.seed (default 1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (xy, area, mut occupied) = patches_of(
            args,
            &args.param("area", "area".to_string())?,
            &args.param("occupied", "occupied".to_string())?,
        )?;
        let need = |name: &str| -> Result<f64> {
            args.opt_param::<f64>(name)?.ok_or_else(|| fail(args, format!("{name}: needed by the model chosen")))
        };
        let text = |name: &str, default: &str| -> Result<String> { args.param(name, default.to_string()) };
        let m = Ifm {
            kernel: match text("kernel", "exponential")?.as_str() {
                "exponential" => Kernel::Exponential,
                "power" => Kernel::Power { beta: args.param("beta", 1.0)? },
                k => return Err(fail(args, format!("kernel: exponential or power, not {k:?}"))),
            },
            connectivity: match text("connectivity", "plain")?.as_str() {
                "plain" => Connectivity::Plain,
                "area" => Connectivity::Area { c1: need("c1")? },
                c => return Err(fail(args, format!("connectivity: plain or area, not {c:?}"))),
            },
            colonisation: match text("colonisation", "logistic")?.as_str() {
                "logistic" => Colonisation::Logistic,
                "exponential" => Colonisation::Exponential,
                "power" => Colonisation::Power { c2: need("c2")?, z: need("z")? },
                c => return Err(fail(args, format!("colonisation: logistic, exponential or power, not {c:?}"))),
            },
            extinction: match text("extinction", "area")?.as_str() {
                "area" => Extinction::Area,
                "exponential" => Extinction::Exponential,
                "rescue" => Extinction::Rescue { r: need("r")? },
                e => return Err(fail(args, format!("extinction: area, exponential or rescue, not {e:?}"))),
            },
            alpha: need("alpha")?,
            x: need("x")?,
            y: need("y")?,
            e: need("e")?,
            b: args.param("b", 1.0)?,
        };
        let steps: usize = args.param("steps", 100)?;
        let mut rng = RRng::new(args.param("seed", 1)?);
        let n = xy.len();
        let summary = |t: usize, occ: &[bool], before: &[bool]| -> Vec<f64> {
            let count = occ.iter().filter(|o| **o).count() as f64;
            let occ_area = (0..n).filter(|&i| occ[i]).fold(0.0, |a, i| a + area[i]);
            let colonised = (0..n).filter(|&i| occ[i] && !before[i]).count() as f64;
            let extinct = (0..n).filter(|&i| !occ[i] && before[i]).count() as f64;
            vec![t as f64, count, occ_area, colonised, extinct]
        };
        let mut t =
            Table::new(["step", "occupied", "occupied_area", "colonised", "extinct"].map(String::from).to_vec());
        t.rows.push(summary(0, &occupied, &occupied));
        let mut times = vec![0usize; n];
        for step in 1..=steps {
            let next = m.step(&xy, &area, &occupied, &mut rng);
            t.rows.push(summary(step, &next, &occupied));
            occupied = next;
            for i in 0..n {
                times[i] += usize::from(occupied[i]);
            }
        }
        let mut p = Table::new(["patch", "x", "y", "area", "occupied", "incidence"].map(String::from).to_vec());
        p.rows = (0..n)
            .map(|i| {
                let inc = if steps > 0 { times[i] as f64 / steps as f64 } else { f64::from(occupied[i]) };
                vec![(i + 1) as f64, xy[i][0], xy[i][1], area[i], f64::from(occupied[i]), inc]
            })
            .collect();
        Ok(outputs([("patches", Value::Table(p)), ("table", Value::Table(t))]))
    }
}
