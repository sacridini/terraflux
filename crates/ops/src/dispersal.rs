//! Species dispersal in a changing climate, as MigClim 1.6 (Engler and Guisan 2009; Engler et al. 2012) simulates
//! it: a cellular automaton on the maps of habitat suitability of successive periods, where a suitable empty cell is
//! colonised from an occupied cell within the dispersal distance with the kernel's probability at that distance
//! (times the source's propagule production while it matures, times the sink's suitability), unless barriers lie
//! between; long-distance dispersal to a random cell at a random distance; the cells that turn unsuitable kept
//! as temporarily resilient until the period ends. Its random numbers are the C runtime's (`rand()`, Microsoft's
//! linear congruential generator, the one MigClim's Windows builds draw from), so a seed gives MigClim's run.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Migrate));
}

fn fail(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

/// The C runtime's rand() of Microsoft's CRT (msvcrt and the UCRT): a linear congruential generator, 15 bits.
pub struct CrtRand(u32);

impl CrtRand {
    pub fn new(seed: u32) -> Self {
        CrtRand(seed)
    }

    pub fn rand(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        ((self.0 >> 16) & 0x7fff) as i32
    }

    /// MigClim's UNIF01: rand() / RAND_MAX.
    fn unif01(&mut self) -> f64 {
        self.rand() as f64 / 32767.0
    }
}

/// MigClim's no data.
pub const NO_DATA: i32 = -9999;

/// A barrier blocks when it crosses all five lines between the cells (weak), or two of them (strong).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierType {
    Weak,
    Strong,
}

/// A MigClim simulation's settings (its parameter file's).
#[derive(Debug, Clone)]
pub struct Settings {
    /// dispersal steps per period (dispSteps)
    pub steps: usize,
    /// the probability of colonisation at a distance of 1, 2... cells (dispKernel; its length the dispersal distance)
    pub kernel: Vec<f64>,
    /// the age a cell starts and fully produces propagules (iniMatAge, fullMatAge)
    pub ini_maturity: i32,
    pub full_maturity: i32,
    /// the propagule production of the ages between (propaguleProd)
    pub propagules: Vec<f64>,
    /// suitability below it 0, else 1000 (rcThreshold; 0 none)
    pub threshold: i32,
    /// long-distance dispersal: its frequency and its distances (lddFreq, lddMinDist, lddMaxDist)
    pub ldd_frequency: f64,
    pub ldd_min: i32,
    pub ldd_max: i32,
    pub barrier_type: BarrierType,
    pub seed: u32,
}

/// A simulation's result: the final state, the state after every dispersal step and the statistics of every step.
pub struct Run {
    pub fin: Vec<i32>,
    pub steps: Vec<(i32, Vec<i32>)>,
    /// envChgStep, dispStep, stepID, univDispersal, noDispersal, occupied, absent, stepColonized, stepDecolonized,
    /// stepLDDsuccess
    pub stats: Vec<[i32; 10]>,
}

struct Automaton<'a> {
    ny: usize,
    nx: usize,
    s: &'a Settings,
    rng: CrtRand,
    use_barrier: bool,
}

impl Automaton<'_> {
    fn idx(&self, r: i64, c: i64) -> usize {
        r as usize * self.nx + c as usize
    }

    /// mcIntersectsBarrier: the cells along lines from the sink to the source.
    fn intersects_barrier(&self, snk_x: i64, snk_y: i64, src_x: i64, src_y: i64, bar: &[i32]) -> bool {
        let (dst_x, dst_y) = (src_x - snk_x, src_y - snk_y);
        let dist_max = dst_x.abs().max(dst_y.abs());
        let (dx, dy, dm) = (dst_x as f64, dst_y as f64, dist_max as f64);
        let (sx, sy) = (snk_x as f64, snk_y as f64);
        let hit = |ox: f64, oy: f64, centred: bool| -> bool {
            (1..=dist_max).any(|i| {
                let i = i as f64;
                let (px, py) = if centred {
                    (
                        (sx + ox + (((i - 1.0) / dm * dx) + ((1.0 / dm * dx) / 2.0))).round(),
                        (sy + oy + (((i - 1.0) / dm * dy) + ((1.0 / dm * dy) / 2.0))).round(),
                    )
                } else {
                    ((sx + ox + (1.0 * i / dm * dx)).round(), (sy + oy + (1.0 * i / dm * dy)).round())
                };
                bar[self.idx(px as i64, py as i64)] == 1
            })
        };
        match self.s.barrier_type {
            BarrierType::Weak => {
                hit(0.0, 0.0, false)
                    && hit(-0.49, -0.49, false)
                    && hit(0.49, -0.49, false)
                    && hit(-0.49, 0.49, false)
                    && hit(0.49, 0.49, false)
            }
            BarrierType::Strong => {
                let mut count = usize::from(hit(0.0, 0.0, false));
                for (ox, oy) in [(-0.49, -0.49), (0.49, -0.49), (-0.49, 0.49), (0.49, 0.49)] {
                    count += usize::from(hit(ox, oy, true));
                    if count > 1 {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// mcSrcCell: a source within the dispersal distance colonising the sink, its draws in MigClim's order.
    #[allow(clippy::too_many_arguments)]
    fn source_cell(
        &mut self,
        i: usize,
        j: usize,
        cur: &[i32],
        age: &[i32],
        loop_id: i32,
        suit: i32,
        bar: &[i32],
    ) -> bool {
        let d = self.s.kernel.len() as i64;
        let (i, j) = (i as i64, j as i64);
        for k in i - d..=i + d {
            for l in j - d..=j + d {
                if k < 0 || k >= self.ny as i64 || l < 0 || l >= self.nx as i64 {
                    continue;
                }
                let at = self.idx(k, l);
                if cur[at] <= 0 || cur[at] == loop_id || age[at] < self.s.ini_maturity {
                    continue;
                }
                let real = (((k - i) * (k - i) + (l - j) * (l - j)) as f64).sqrt().round() as i64;
                if real == 0 || real > d {
                    continue;
                }
                let kernel = self.s.kernel[real as usize - 1] * 1.0;
                let p = if age[at] >= self.s.full_maturity {
                    kernel * (suit as f64 / 1000.0)
                } else {
                    kernel * self.s.propagules[(age[at] - self.s.ini_maturity) as usize] * (suit as f64 / 1000.0)
                };
                let rnd = self.rng.unif01();
                if (rnd < p || p == 1.0) && !(self.use_barrier && self.intersects_barrier(i, j, k, l, bar)) {
                    return true;
                }
            }
        }
        false
    }
}

/// MigClim's simulation (mcMigrate) of an initial distribution (1 present, 0 absent, `NO_DATA`) over periods of
/// habitat suitability (0 to 1000), with barriers (1) or none.
pub fn migrate(
    (ny, nx): (usize, usize),
    initial: &[i32],
    suitability: &[Vec<i32>],
    barriers: Option<&[i32]>,
    s: &Settings,
) -> Run {
    let n = ny * nx;
    let mut a = Automaton { ny, nx, s, rng: CrtRand::new(s.seed), use_barrier: barriers.is_some() };
    let mut cur = initial.to_vec();
    let mut bar: Vec<i32> = barriers.map_or(vec![0; n], |b| b.to_vec());
    // mcFilterMatrix(barriers, currentState, true, false, true) and, with barriers, (currentState, barriers, ones)
    for k in 0..n {
        if bar[k] < 0 {
            bar[k] = 0;
        }
        if cur[k] == NO_DATA {
            bar[k] = NO_DATA;
        }
    }
    if a.use_barrier {
        for k in 0..n {
            if bar[k] == 1 {
                cur[k] = 0;
            }
        }
    }
    let mut age: Vec<i32> = cur.iter().map(|&c| if c == 1 { s.full_maturity } else { 0 }).collect();
    let mut no_disp = cur.clone();
    let initial_count = cur.iter().filter(|&&c| c == 1).count() as i32;
    let mut absent = cur.iter().filter(|&&c| c == 0).count() as i32;
    let (mut colonized, mut no_disp_count, mut univ) = (initial_count, initial_count, initial_count);
    let mut stats = vec![[0, 0, 1, univ, no_disp_count, colonized, absent, 0, 0, 0]];
    let mut steps = Vec::new();
    let mut hs = vec![0; n];
    let mut step_decolonized;
    for (e, layer) in suitability.iter().enumerate() {
        let env = e as i32 + 1;
        hs.copy_from_slice(layer);
        if s.threshold > 0 {
            hs.iter_mut().for_each(|v| *v = if *v < s.threshold { 0 } else { 1000 });
        }
        for k in 0..n {
            if hs[k] < 0 {
                hs[k] = 0;
            }
            if bar[k] == 1 {
                hs[k] = 0;
            }
            if bar[k] == NO_DATA {
                hs[k] = NO_DATA;
            }
        }
        let mut loop_id = env * 100;
        univ = hs.iter().filter(|&&v| v > 0).count() as i32;
        if no_disp_count > 0 {
            for k in 0..n {
                if no_disp[k] == 1 && hs[k] == 0 {
                    no_disp[k] = 0;
                    no_disp_count -= 1;
                }
            }
        }
        step_decolonized = 0;
        for k in 0..n {
            if hs[k] == 0 && cur[k] > 0 {
                cur[k] = 29900;
                step_decolonized += 1;
            }
        }
        for d in 1..=s.steps {
            loop_id += 1;
            let (mut step_colonized, mut step_ldd) = (0, 0);
            if d > 1 {
                step_decolonized = 0;
            }
            for i in 0..ny {
                for j in 0..nx {
                    let k = i * nx + j;
                    if hs[k] > 0 && cur[k] <= 0 && a.source_cell(i, j, &cur, &age, loop_id, hs[k], &bar) {
                        cur[k] = loop_id;
                        step_colonized += 1;
                        age[k] = 0;
                    }
                }
            }
            if s.ldd_frequency > 0.0 {
                for i in 0..ny {
                    for j in 0..nx {
                        let k = i * nx + j;
                        if cur[k] <= 0 || cur[k] == loop_id || age[k] < s.ini_maturity {
                            continue;
                        }
                        let p = if age[k] >= s.full_maturity {
                            s.ldd_frequency
                        } else {
                            s.ldd_frequency * s.propagules[(age[k] - s.ini_maturity) as usize]
                        };
                        if a.rng.unif01() < p || p == 1.0 {
                            // mcRandomPixel
                            let dist = (a.rng.unif01() * (s.ldd_max - s.ldd_min) as f64) + s.ldd_min as f64;
                            // (MigClim's 2π, to six decimals)
                            #[allow(clippy::approx_constant)]
                            let angle = a.rng.unif01() * 6.283185;
                            let r = (dist * angle.cos()) as i64 + i as i64;
                            let c = (dist * angle.sin()) as i64 + j as i64;
                            // mcSinkCellCheck
                            if r < 0 || r >= ny as i64 || c < 0 || c >= nx as i64 {
                                continue;
                            }
                            let t = r as usize * nx + c as usize;
                            if cur[t] > 0 || hs[t] == 0 {
                                continue;
                            }
                            if a.rng.unif01() * 1000.0 > hs[t] as f64 {
                                continue;
                            }
                            cur[t] = loop_id;
                            step_colonized += 1;
                            step_ldd += 1;
                            age[t] = 0;
                        }
                    }
                }
            }
            for k in 0..n {
                if cur[k] > 0 {
                    age[k] += 1;
                }
                if cur[k] >= 29900 {
                    cur[k] += 1;
                }
            }
            colonized = colonized + step_colonized - step_decolonized;
            absent = absent - step_colonized + step_decolonized;
            stats.push([
                env,
                d as i32,
                loop_id,
                univ,
                no_disp_count,
                colonized,
                absent,
                step_colonized,
                step_decolonized,
                step_ldd,
            ]);
            steps.push((loop_id, cur.clone()));
        }
        // the temporarily resilient cells at the end of the period (MigClim's dispSteps - loopID - 1)
        for k in 0..n {
            if cur[k] >= 29900 {
                cur[k] = s.steps as i32 - loop_id - 1;
                age[k] = 0;
            }
        }
    }
    for k in 0..n {
        if hs[k] > 0 && cur[k] <= 0 {
            cur[k] = 30000;
        }
    }
    Run { fin: cur, steps, stats }
}

/// A map's values as MigClim's integers (no value: its no data).
fn ints(v: &[f64]) -> Vec<i32> {
    v.iter().map(|x| if x.is_nan() { NO_DATA } else { x.round() as i32 }).collect()
}

fn floats(v: &[i32]) -> Vec<f64> {
    v.iter().map(|&x| if x == NO_DATA { f64::NAN } else { x as f64 }).collect()
}

/// A list of probabilities of a setting, as MigClim reads them (C floats).
fn probabilities(args: &Args, name: &str, default: Option<Vec<f64>>) -> Result<Vec<f64>> {
    let v: Vec<f64> = match (args.opt_param::<Vec<f64>>(name)?, default) {
        (Some(v), _) | (None, Some(v)) => v,
        (None, None) => return Err(fail(args, format!("{name}: a list of probabilities"))),
    };
    if v.is_empty() || v.iter().any(|p| !(0.0..=1.0).contains(p)) {
        return Err(fail(args, format!("{name}: probabilities from 0 to 1")));
    }
    Ok(v.iter().map(|p| *p as f32 as f64).collect())
}

struct Migrate;

impl Operator for Migrate {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "sdm.migrate",
            "A species' dispersal through the habitat suitability of successive periods (MigClim 1.6's cellular \
             automaton, the same run for a seed): every step, a suitable empty cell is colonised from an occupied \
             cell within the kernel's distance with the kernel's probability (times the source's propagule \
             production while it matures, times the sink's suitability), unless barriers lie between; long-distance \
             dispersal to random cells; cells that turn unsuitable stay until their period ends. The final map is \
             MigClim's: 1 the initial cells, a step's number (period × 100 + step) where it colonised, 30000 \
             suitable but not reached, below 0 lost, 0 absent.",
            Access::Global,
        )
        .input(PortSpec::new("initial", Type::Map, "the initial distribution: 1 present, 0 absent"))
        .input(PortSpec::new(
            "suitability",
            Type::Raster,
            "the habitat suitability of each period, 0 to 1000 (a stack, a loop's series, rasters)",
        ))
        .input(PortSpec::optional("barriers", Type::Map, "the barriers (1)"))
        .output(PortSpec::new("map", Type::Map, "the final state (MigClim's codes)"))
        .output(PortSpec::new("occupied", Type::Map, "1 where the species is at the end, 0 elsewhere"))
        .output(PortSpec::new("steps", Type::Stack, "the state after every dispersal step"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "per step: period, step, step_id, unlimited, no_dispersal, occupied, absent, colonized, decolonized, ldd",
        ))
        .param(PortSpec::optional("steps", Type::Integer, "dispersal steps per period (default 1)"))
        .param(PortSpec::new("kernel", Type::Any, "the probability of colonisation at 1, 2... cells: [1, 0.4, 0.1]"))
        .param(PortSpec::optional("ini_maturity", Type::Integer, "the age a cell first disperses (default 1)"))
        .param(PortSpec::optional("full_maturity", Type::Integer, "the age it fully disperses (default 1)"))
        .param(PortSpec::optional(
            "propagules",
            Type::Any,
            "the propagule production of the ages from ini_maturity to full_maturity (default 1 each)",
        ))
        .param(PortSpec::optional("threshold", Type::Integer, "suitability below it 0, else 1000 (default 0: none)"))
        .param(PortSpec::optional("ldd_frequency", Type::Number, "long-distance dispersal's frequency (default 0)"))
        .param(PortSpec::optional("ldd_min", Type::Integer, "its smallest distance in cells (default 10)"))
        .param(PortSpec::optional("ldd_max", Type::Integer, "its largest distance in cells (default 15)"))
        .param(
            PortSpec::optional("barrier_type", Type::Text, "strong (default: two of five lines) or weak (all five)")
                .choices(["strong", "weak"]),
        )
        .param(PortSpec::optional("seed", Type::Integer, "the seed of the C runtime's rand() (default 1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let initial = args.map("initial")?;
        let (grid, layers) = crate::stats::layers_of(args, "suitability")?;
        let (ny, nx) = initial.shape();
        if grid.width != nx || grid.height != ny {
            return Err(fail(args, "suitability: the initial distribution's size"));
        }
        let barriers = args.opt_map("barriers")?;
        if barriers.is_some_and(|b| b.shape() != (ny, nx)) {
            return Err(fail(args, "barriers: the initial distribution's size"));
        }
        let kernel = probabilities(args, "kernel", None)?;
        let ini_maturity: i32 = args.param("ini_maturity", 1)?;
        let full_maturity: i32 = args.param("full_maturity", 1)?;
        if ini_maturity < 1 || full_maturity < ini_maturity {
            return Err(fail(args, "ini_maturity: 1 or more, full_maturity: ini_maturity or more"));
        }
        let ages = (full_maturity - ini_maturity).max(1) as usize;
        let propagules = probabilities(args, "propagules", Some(vec![1.0; ages]))?;
        if propagules.len() < ages {
            return Err(fail(args, format!("propagules: {ages} values (from ini_maturity to full_maturity)")));
        }
        let barrier_type: String = args.param("barrier_type", "strong".to_string())?;
        let s = Settings {
            steps: args.param("steps", 1)?,
            kernel,
            ini_maturity,
            full_maturity,
            propagules,
            threshold: args.param("threshold", 0)?,
            ldd_frequency: args.param::<f64>("ldd_frequency", 0.0)? as f32 as f64,
            ldd_min: args.param("ldd_min", 10)?,
            ldd_max: args.param("ldd_max", 15)?,
            barrier_type: match barrier_type.as_str() {
                "strong" => BarrierType::Strong,
                "weak" => BarrierType::Weak,
                t => return Err(fail(args, format!("barrier_type: strong or weak, not {t:?}"))),
            },
            seed: args.param("seed", 1)?,
        };
        if !(0..=1000).contains(&s.threshold) || !(0.0..=1.0).contains(&s.ldd_frequency) || s.ldd_min < 0 {
            return Err(fail(args, "threshold: 0 to 1000; ldd_frequency: 0 to 1; ldd_min: 0 or more"));
        }
        if s.ldd_frequency > 0.0 && s.ldd_max <= s.ldd_min {
            return Err(fail(args, "ldd_max: above ldd_min"));
        }
        let suit: Vec<Vec<i32>> = layers.iter().map(|l| ints(l)).collect();
        let bar = barriers.map(|b| ints(&b.data));
        let r = migrate((ny, nx), &ints(&initial.data), &suit, bar.as_deref(), &s);
        let g = initial.grid.clone();
        let occupied: Vec<f64> =
            r.fin.iter().map(|&c| if c == NO_DATA { f64::NAN } else { f64::from(c > 0 && c < 30000) }).collect();
        let stack = Stack {
            grid: g.clone(),
            keys: r.steps.iter().map(|(id, _)| Key::Name(id.to_string())).collect(),
            layers: r.steps.iter().map(|(_, m)| Arc::new(floats(m))).collect(),
        };
        let mut t = Table::new(
            [
                "period",
                "step",
                "step_id",
                "unlimited",
                "no_dispersal",
                "occupied",
                "absent",
                "colonized",
                "decolonized",
                "ldd",
            ]
            .map(String::from)
            .to_vec(),
        );
        t.rows = r.stats.iter().map(|row| row.iter().map(|&v| v as f64).collect()).collect();
        Ok(outputs([
            ("map", Value::Map(Map::new(g.clone(), floats(&r.fin)))),
            ("occupied", Value::Map(Map::new(g, occupied))),
            ("steps", Value::Stack(stack)),
            ("table", Value::Table(t)),
        ]))
    }
}
