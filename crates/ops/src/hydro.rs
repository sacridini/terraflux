//! Terrain hydrology through landschaft's C++ (its `ls.hydro`): depression filling (Priority-Flood) and breaching
//! (least cost), flow directions (D8, D-infinity, pygeoprocessing's multiple flow directions), flow accumulation
//! (D8, D-infinity, FD8, Quinn's, MFD), streams, the distance to them, HAND, stream links with their table,
//! watersheds and the wetness, stream power and sediment transport indices. WhiteboxTools' algorithms cell for cell
//! where it has them, pygeoprocessing's (the InVEST models' routing) for the multiple flow directions; the same bits
//! as landschaft's.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::{Grid, vector::Geometry};

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    for op in OPS {
        r.add(Arc::new(Hydro { op }));
    }
}

const OPS: [&str; 13] = [
    "fill",
    "breach",
    "flow_direction",
    "flow_accumulation",
    "distance_to_channel",
    "streams",
    "hand",
    "stream_links",
    "watersheds",
    "twi",
    "spi",
    "sti",
    "connectivity_index",
];

/// The block pygeoprocessing iterates a DEM by (the order its routing visits the cells).
pub const MFD_BLOCK: i32 = 256;

/// ESRI's D8 codes by landschaft's neighbour index (0 NE, 1 E ... 7 N).
const ESRI: [f64; 8] = [128.0, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0];

/// A DEM as the kernels take it: its values, size and cell sizes (metres at the centre latitude for longitude /
/// latitude, as landschaft's _cells).
pub struct Dem<'a> {
    pub z: &'a [f64],
    pub ny: i32,
    pub nx: i32,
    pub dx: f64,
    pub dy: f64,
}

impl Dem<'_> {
    pub fn of(m: &Map) -> Dem<'_> {
        let (ny, nx) = m.shape();
        let (dx, dy) = cells(&m.grid);
        Dem { z: &m.data, ny: ny as i32, nx: nx as i32, dx, dy }
    }

    /// D8 directions (0 NE ... 7 N; -1 no flow; -2 missing).
    pub fn d8(&self, flats: bool) -> Result<Vec<i8>> {
        Ok(ffi::hydro_d8(self.z, self.ny, self.nx, self.dx, self.dy, flats, threads())?)
    }

    /// The directions of the flat cells (Barnes et al. 2014), -1 elsewhere.
    pub fn flat_dirs(&self) -> Result<Vec<i8>> {
        let plain = ffi::hydro_d8(self.z, self.ny, self.nx, self.dx, self.dy, false, threads())?;
        Ok(ffi::hydro_flat_directions(self.z, &plain, self.ny, self.nx, self.dx, self.dy)?)
    }

    /// pygeoprocessing's packed multiple flow directions.
    pub fn mfd(&self) -> Result<Vec<i32>> {
        Ok(ffi::hydro_mfd_directions(self.z, self.ny, self.nx, MFD_BLOCK)?)
    }

    /// The flow accumulation by a method, in cells, of weights (empty: 1 per cell).
    pub fn accumulation(&self, method: &str, weights: &[f64], exponent: Option<f64>, flats: bool) -> Result<Vec<f64>> {
        Ok(match method {
            "d8" => ffi::hydro_d8_accumulation(&self.d8(flats)?, weights, self.ny, self.nx)?,
            "mfd" => {
                let mut acc = ffi::hydro_mfd_packed_accumulation(&self.mfd()?, weights, self.ny, self.nx)?;
                for (a, z) in acc.iter_mut().zip(self.z) {
                    if z.is_nan() {
                        *a = f64::NAN;
                    }
                }
                acc
            }
            "dinf" => {
                let flat = if flats { self.flat_dirs()? } else { Vec::new() };
                let angle = ffi::hydro_dinf(self.z, self.ny, self.nx, self.dx, self.dy, &flat, threads())?;
                ffi::hydro_dinf_accumulation(&angle, weights, self.ny, self.nx)?
            }
            "fd8" | "quinn" => {
                let p = exponent.unwrap_or(if method == "fd8" { 1.1 } else { 1.0 });
                let flat = if flats { self.flat_dirs()? } else { Vec::new() };
                ffi::hydro_mfd_accumulation(
                    self.z,
                    self.ny,
                    self.nx,
                    self.dx,
                    self.dy,
                    p,
                    method == "quinn",
                    &flat,
                    weights,
                )?
            }
            m => {
                return Err(Error::msg(format!("method must be one of d8, dinf, fd8, quinn, mfd, got {m:?}")));
            }
        })
    }
}

/// The kernels' threads of this thread.
fn threads() -> i32 {
    crate::threads::current() as i32
}

/// Cell width and height in map units (metres at the centre latitude for longitude / latitude).
pub fn cells(g: &Grid) -> (f64, f64) {
    let (e, lonlat) = crate::surface::extent_of(g);
    let (dx, dy) = ((e[1] - e[0]) / g.width as f64, (e[3] - e[2]) / g.height as f64);
    if lonlat {
        let lat = ((e[2] + e[3]) / 2.0).to_radians();
        return (dx * 111320.0 * lat.cos(), dy * 110574.0);
    }
    (dx, dy)
}

/// A raster on the DEM's grid (same shape).
fn same<'a>(dem: &Map, m: &'a Map, what: &str) -> Result<&'a [f64]> {
    if m.shape() != dem.shape() {
        return Err(Error::msg(format!("the {what} differ in shape from the DEM")));
    }
    Ok(&m.data)
}

/// Weights with no value counted as 0, as landschaft's flow_accumulation.
fn weights_of(args: &Args, dem: &Map) -> Result<Vec<f64>> {
    match args.opt_map("weights")? {
        None => Ok(Vec::new()),
        Some(w) => Ok(same(dem, w, "weights")?.iter().map(|v| if v.is_nan() { 0.0 } else { *v }).collect()),
    }
}

/// Stream cells (non-zero: `any`; else 1) of a raster, as bytes.
fn mask(v: &[f64], any: bool) -> Vec<u8> {
    v.iter().map(|x| u8::from(if any { !x.is_nan() && *x != 0.0 } else { *x == 1.0 })).collect()
}

struct Hydro {
    op: &'static str,
}

fn dem_in(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::new("dem", Type::Map, "the elevations (no value: missing), best with depressions filled"))
}

fn flats(sig: OpSignature) -> OpSignature {
    sig.param(PortSpec::optional(
        "flats",
        Type::Bool,
        "drain flat areas towards lower terrain and away from higher terrain (Barnes et al. 2014; default true)",
    ))
}

fn method_param(sig: OpSignature, about: &str, default: &str) -> OpSignature {
    let choices: &[&str] = if default == "mfd" { &["mfd", "d8"] } else { &["d8", "dinf", "fd8", "quinn", "mfd"] };
    sig.param(PortSpec::optional("method", Type::Text, about).choices(choices.iter().copied()))
}

impl Operator for Hydro {
    fn signature(&self) -> OpSignature {
        let name = format!("hydro.{}", self.op);
        let sig = |doc: &str| OpSignature::new(&name, doc, Access::Global);
        let map_out = |s: OpSignature, about: &str| s.output(PortSpec::new("map", Type::Map, about));
        match self.op {
            "fill" => map_out(
                dem_in(sig(
                    "Fill the depressions of a DEM so that water flows from every cell to the edge: each depression \
                     raised to where it would spill over (Priority-Flood, Barnes, Lehman & Mulla 2014; the DEM of \
                     WhiteboxTools' FillDepressions), through landschaft's C++.",
                )),
                "the filled DEM",
            )
            .param(PortSpec::optional(
                "drain_flats",
                Type::Bool,
                "raise each filled cell by the smallest step above the cell it drains to (Priority-Flood+epsilon), \
                 so filled areas slope towards their outlet",
            )),
            "breach" => map_out(
                dem_in(sig(
                    "Remove the depressions of a DEM by cutting channels through their rims (least-cost breaching, \
                     WhiteboxTools' BreachDepressionsLeastCost after Lindsay & Dhun 2015): each pit, from the \
                     lowest, joined to the nearest cell it can drain to by the channel of least cost; often changes \
                     the terrain less than filling.",
                )),
                "the breached DEM",
            )
            .param(PortSpec::optional("max_length", Type::Integer, "the longest channel, in cells (100)"))
            .param(PortSpec::optional("max_cost", Type::Number, "the costliest channel to cut (default: any)"))
            .param(PortSpec::optional(
                "min_dist",
                Type::Bool,
                "count the channel's length in its cost, preferring shorter channels (default true)",
            ))
            .param(PortSpec::optional(
                "flat_increment",
                Type::Number,
                "the drop per cell along a channel (default: from the elevations, as WhiteboxTools)",
            ))
            .param(PortSpec::optional(
                "fill_rest",
                Type::Bool,
                "fill the depressions no channel could solve (default true)",
            )),
            "flow_direction" => flats(
                map_out(
                    dem_in(sig(
                        "The direction each cell drains to: D8 (O'Callaghan & Mark 1984) as ESRI's codes (1 E, 2 SE, \
                         4 S, 8 SW, 16 W, 32 NW, 64 N, 128 NE; 0 no flow), D-infinity (Tarboton 1997) as the azimuth \
                         in degrees clockwise from north (-1 no flow), or pygeoprocessing's multiple flow directions \
                         (the InVEST models' routing) as eight packed 4-bit weights.",
                    )),
                    "the codes or azimuths",
                )
                .param(
                    PortSpec::optional("method", Type::Text, "d8 (default), dinf or mfd")
                        .choices(["d8", "dinf", "mfd"]),
                ),
            ),
            "flow_accumulation" => flats(method_param(
                map_out(
                    dem_in(sig(
                        "The area that drains through each cell (itself included): all to the steepest neighbour \
                         (d8), split between the two cells of the D-infinity azimuth (dinf), among the lower \
                         neighbours by slope^exponent (fd8, Freeman 1991) or by slope times contour length (quinn, \
                         Quinn et al. 1991), or pygeoprocessing's multiple flow directions (mfd, the InVEST \
                         models'). WhiteboxTools' accumulations cell for cell.",
                    ))
                    .input(PortSpec::optional(
                        "weights",
                        Type::Map,
                        "what each cell contributes instead of 1",
                    )),
                    "the accumulation",
                ),
                "d8 (default), dinf, fd8, quinn or mfd",
                "d8",
            ))
            .param(
                PortSpec::optional(
                    "units",
                    Type::Text,
                    "cells (default), area (map units squared; m² for longitude / latitude) or sca (the area per \
                     unit contour width)",
                )
                .choices(["cells", "area", "sca"]),
            )
            .param(PortSpec::optional("exponent", Type::Number, "of fd8 (1.1) and quinn (1)")),
            "distance_to_channel" => map_out(
                dem_in(sig(
                    "The mean distance water travels from each cell to a stream along pygeoprocessing's multiple \
                     flow directions (InVEST's sediment delivery model's downslope component): in cell lengths, or \
                     in the units of the weights; 0 on the streams, no value where no flow reaches one.",
                ))
                .input(PortSpec::new("streams", Type::Map, "1 on the stream cells"))
                .input(PortSpec::optional(
                    "weights",
                    Type::Map,
                    "the length of each step onto a cell",
                )),
                "the distance",
            ),
            "streams" => map_out(
                sig("The stream cells: those whose flow accumulation is at least a threshold; or, with the DEM and \
                     trace, pygeoprocessing's stream extraction from the mfd accumulation as InVEST's sediment model \
                     runs it (trace 0.7): cells of the threshold traced up from the outlets, tributaries down to \
                     trace x threshold kept where they join one.")
                .input(PortSpec::new("accumulation", Type::Map, "a flow accumulation"))
                .input(PortSpec::optional("dem", Type::Map, "the (filled) DEM whose mfd directions are traced")),
                "1 on the streams, 0 elsewhere",
            )
            .param(PortSpec::new("threshold", Type::Number, "the smallest accumulation of a stream, in its units"))
            .param(PortSpec::optional("trace", Type::Number, "0 to 1, with the DEM (InVEST's: 0.7)")),
            "hand" => flats(map_out(
                dem_in(sig(
                    "Height Above the Nearest Drainage (HAND; Rennó et al. 2008, Nobre et al. 2011): the height of \
                     each cell above the stream cell its water reaches along the D8 directions; 0 on the streams, \
                     no value where it reaches none.",
                ))
                .input(PortSpec::optional(
                    "streams",
                    Type::Map,
                    "the drainage: non-zero on the stream cells",
                )),
                "the height in the DEM's units",
            ))
            .param(PortSpec::optional(
                "threshold",
                Type::Number,
                "or the drainage as the cells of D8 accumulation (cells) of at least this",
            )),
            "stream_links" => flats(
                dem_in(sig(
                    "The links of a stream network: the stretches between sources, junctions and outlets along the \
                     D8 directions, numbered 1, 2... in the raster order of their first cells; and their table \
                     (downstream link, whether it ends at an outlet, first and last cell, cells, length, \
                     elevations, Strahler order).",
                ))
                .input(PortSpec::new("streams", Type::Map, "1 on the stream cells"))
                .input(PortSpec::optional(
                    "outlets",
                    Type::Vector,
                    "points where a link must end (gauges, dams, the outlet), each snapped to the nearest stream cell",
                ))
                .output(PortSpec::new("map", Type::Map, "the link of each stream cell, 0 elsewhere"))
                .output(PortSpec::new("table", Type::Table, "a row per link"))
                .param(PortSpec::optional(
                    "snap",
                    Type::Number,
                    "the largest move of an outlet, map units (300 m; 3 cells without a projection)",
                )),
            ),
            "watersheds" => flats(map_out(
                dem_in(sig(
                    "The area draining to each seed: every cell labelled with the first seed below it along the D8 \
                     directions (seeds keep their own label); 0 where its path reaches none.",
                ))
                .input(PortSpec::new(
                    "seeds",
                    Type::Map,
                    "positive whole labels on the cells water drains to (stream links, outlets, lakes), 0 elsewhere",
                )),
                "the label of the seed each cell drains to",
            )),
            "twi" | "spi" | "sti" => {
                let doc = match self.op {
                    "twi" => {
                        "Topographic wetness index ln(a / tan(slope)) (Beven & Kirkby 1979), WhiteboxTools' \
                         WetnessIndex: high where much area drains through a gentle slope; a proxy of soil moisture."
                    }
                    "spi" => {
                        "Stream power index a x tan(slope) (Moore, Grayson & Ladson 1991), WhiteboxTools' \
                         StreamPowerIndex: the erosive power of overland flow."
                    }
                    _ => {
                        "Sediment transport index (m + 1) (a / 22.13)^m (sin(slope) / 0.0896)^n (Moore & Burch \
                         1986), WhiteboxTools' SedimentTransportIndex: the LS factor of the USLE from the \
                         contributing area."
                    }
                };
                let s = flats(method_param(
                    map_out(
                        dem_in(sig(&format!(
                            "{doc} a: the specific contributing area of a multiple-flow accumulation by default; the \
                             slope: Horn's gradient of the DEM."
                        )))
                        .input(PortSpec::optional("sca", Type::Map, "a specific contributing area to use instead"))
                        .input(PortSpec::optional(
                            "slope",
                            Type::Map,
                            "a slope in degrees to use instead",
                        )),
                        &format!("the {}", self.op.to_uppercase()),
                    ),
                    "the accumulation of a: fd8 (default), quinn, dinf, d8 or mfd",
                    "fd8",
                ));
                match self.op {
                    "twi" => s.param(PortSpec::optional(
                        "min_slope",
                        Type::Number,
                        "a floor for tan(slope), so flat cells get a value (0: they have none)",
                    )),
                    "sti" => s
                        .param(PortSpec::optional("sca_exponent", Type::Number, "m (0.4)"))
                        .param(PortSpec::optional("slope_exponent", Type::Number, "n (1.3)")),
                    _ => s,
                }
            }
            _ => map_out(
                dem_in(sig(
                    "The index of connectivity (Borselli et al. 2008) as InVEST's sediment delivery ratio model \
                     computes it: log10 of the upslope over the downslope component, the weight (a land use's \
                     USLE C) and pygeoprocessing's slope along its routing; no value on the streams.",
                ))
                .input(PortSpec::new("weight", Type::Map, "the weight W of each cell (USLE C)"))
                .input(PortSpec::optional(
                    "streams",
                    Type::Map,
                    "1 on the streams (else from the threshold)",
                )),
                "the index",
            )
            .param(PortSpec::optional(
                "threshold",
                Type::Number,
                "the flow accumulation (cells) of a stream, without streams",
            ))
            .param(PortSpec::optional("method", Type::Text, "the routing: mfd (default) or d8").choices(["mfd", "d8"])),
        }
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        match self.op {
            "streams" => return streams(args).map_err(fail),
            "connectivity_index" => return crate::services::connectivity_index(args).map_err(fail),
            _ => {}
        }
        let m = args.map("dem")?;
        let d = Dem::of(m);
        let one = |v: Vec<f64>| outputs([("map", Value::Map(Map::new(m.grid.clone(), v)))]);
        let flats: bool = args.param("flats", true)?;
        let r = || -> Result<Outputs> {
            Ok(match self.op {
                "fill" => one(ffi::hydro_fill(d.z, d.ny, d.nx, args.param("drain_flats", false)?)?),
                "breach" => {
                    let max_cost: Option<f64> = args.opt_param("max_cost")?;
                    let inc: Option<f64> = args.opt_param("flat_increment")?;
                    if inc.is_some_and(|v| v <= 0.0) {
                        return Err(Error::msg("flat_increment must be positive"));
                    }
                    one(ffi::hydro_breach(
                        d.z,
                        d.ny,
                        d.nx,
                        d.dx,
                        d.dy,
                        args.param("max_length", 100)?,
                        max_cost.unwrap_or(f64::INFINITY),
                        args.param("min_dist", true)?,
                        inc.unwrap_or(0.0),
                        args.param("fill_rest", true)?,
                    )?)
                }
                "flow_direction" => {
                    let method: String = args.param("method", "d8".to_string())?;
                    match method.as_str() {
                        "d8" => one(d
                            .d8(flats)?
                            .iter()
                            .map(|&k| match k {
                                -2 => f64::NAN,
                                k if k >= 0 => ESRI[k as usize],
                                _ => 0.0,
                            })
                            .collect()),
                        "dinf" => {
                            let flat = if flats { d.flat_dirs()? } else { Vec::new() };
                            one(ffi::hydro_dinf(d.z, d.ny, d.nx, d.dx, d.dy, &flat, threads())?)
                        }
                        "mfd" => one(d.mfd()?.iter().map(|&v| f64::from(v)).collect()),
                        m => return Err(Error::msg(format!("method must be d8, dinf or mfd, got {m:?}"))),
                    }
                }
                "flow_accumulation" => {
                    let method: String = args.param("method", "d8".to_string())?;
                    let units: String = args.param("units", "cells".to_string())?;
                    if !["cells", "area", "sca"].contains(&units.as_str()) {
                        return Err(Error::msg(format!("units must be cells, area or sca, got {units:?}")));
                    }
                    let w = weights_of(args, m)?;
                    let mut acc = d.accumulation(&method, &w, args.opt_param("exponent")?, flats)?;
                    if units != "cells" {
                        for a in &mut acc {
                            *a *= d.dx * d.dy;
                            if units == "sca" {
                                *a /= (d.dx + d.dy) / 2.0;
                            }
                        }
                    }
                    one(acc)
                }
                "distance_to_channel" => {
                    let st = same(m, args.map("streams")?, "streams")?;
                    let w = match args.opt_map("weights")? {
                        Some(w) => same(m, w, "weights")?.to_vec(),
                        None => Vec::new(),
                    };
                    let channel: Vec<u8> = st.iter().map(|v| u8::from(*v == 1.0)).collect();
                    one(ffi::hydro_mfd_distance_to_channel(&d.mfd()?, &channel, &w, d.ny, d.nx)?)
                }
                "hand" => {
                    let dir = d.d8(flats)?;
                    let streams = match args.opt_map("streams")? {
                        Some(s) => mask(same(m, s, "streams")?, true),
                        None => {
                            let t: f64 = args
                                .opt_param("threshold")?
                                .ok_or_else(|| Error::msg("give the streams or a threshold of accumulation"))?;
                            ffi::hydro_d8_accumulation(&dir, &[], d.ny, d.nx)?
                                .iter()
                                .map(|a| u8::from(*a >= t))
                                .collect()
                        }
                    };
                    one(ffi::hydro_hand(d.z, &dir, &streams, d.ny, d.nx)?)
                }
                "stream_links" => stream_links(args, m, &d, flats)?,
                "watersheds" => {
                    let dir = d.d8(flats)?;
                    let seeds = same(m, args.map("seeds")?, "seeds")?;
                    let mut lab = Vec::with_capacity(seeds.len());
                    for v in seeds {
                        let v = if v.is_nan() { 0.0 } else { *v };
                        if v < 0.0 || v.fract() != 0.0 {
                            return Err(Error::msg("seeds are non-negative whole labels"));
                        }
                        lab.push(v as i32);
                    }
                    let out = ffi::hydro_watersheds(&dir, &lab, d.ny, d.nx)?;
                    one(out.iter().zip(&dir).map(|(&w, &k)| if k == -2 { f64::NAN } else { f64::from(w) }).collect())
                }
                _ => one(index(self.op, args, m, &d, flats)?),
            })
        };
        r().map_err(fail)
    }
}

fn streams(args: &Args) -> Result<Outputs> {
    let acc = args.map("accumulation")?;
    let threshold: f64 =
        args.opt_param("threshold")?.ok_or_else(|| Error::msg("give the threshold of accumulation"))?;
    let trace: Option<f64> = args.opt_param("trace")?;
    let out: Vec<f64> = match trace {
        None => {
            acc.data.iter().map(|a| if a.is_nan() { f64::NAN } else { f64::from(u8::from(*a >= threshold)) }).collect()
        }
        Some(t) => {
            let dem = args.opt_map("dem")?.ok_or_else(|| Error::msg("tracing streams needs the DEM (dem)"))?;
            if !(0.0..=1.0).contains(&t) {
                return Err(Error::msg("trace must be between 0 and 1"));
            }
            if dem.shape() != acc.shape() {
                return Err(Error::msg("the DEM differs in shape from the accumulation"));
            }
            let (ny, nx) = acc.shape();
            let packed = ffi::hydro_mfd_directions(&dem.data, ny as i32, nx as i32, MFD_BLOCK)?;
            ffi::hydro_mfd_streams(&acc.data, &packed, ny as i32, nx as i32, threshold, t, MFD_BLOCK)?
                .iter()
                .map(|&c| if c == 255 { f64::NAN } else { f64::from(c) })
                .collect()
        }
    };
    Ok(outputs([("map", Value::Map(Map::new(acc.grid.clone(), out)))]))
}

/// tan(slope) by Horn (1981); a missing or outside neighbour takes the cell's own value (landschaft's _tan_slope).
fn tan_slope(d: &Dem) -> Vec<f64> {
    let (ny, nx) = (d.ny as isize, d.nx as isize);
    let at = |r: isize, c: isize, centre: f64| -> f64 {
        if r < 0 || c < 0 || r >= ny || c >= nx {
            return centre;
        }
        let v = d.z[(r * nx + c) as usize];
        if v.is_nan() { centre } else { v }
    };
    let mut out = vec![f64::NAN; d.z.len()];
    for r in 0..ny {
        for c in 0..nx {
            let z = d.z[(r * nx + c) as usize];
            let (z1, z2, z3) = (at(r - 1, c - 1, z), at(r - 1, c, z), at(r - 1, c + 1, z));
            let (z4, z6) = (at(r, c - 1, z), at(r, c + 1, z));
            let (z7, z8, z9) = (at(r + 1, c - 1, z), at(r + 1, c, z), at(r + 1, c + 1, z));
            let gx = ((z3 + 2.0 * z6 + z9) - (z1 + 2.0 * z4 + z7)) / (8.0 * d.dx);
            let gy = ((z7 + 2.0 * z8 + z9) - (z1 + 2.0 * z2 + z3)) / (8.0 * d.dy);
            out[(r * nx + c) as usize] = (gx * gx + gy * gy).sqrt();
        }
    }
    out
}

/// The wetness, stream power or sediment transport index (landschaft's twi, spi, sti).
fn index(op: &str, args: &Args, m: &Map, d: &Dem, flats: bool) -> Result<Vec<f64>> {
    let method: String = args.param("method", "fd8".to_string())?;
    let a = match args.opt_map("sca")? {
        Some(s) => same(m, s, "specific contributing area")?.to_vec(),
        None => {
            let mut acc = d.accumulation(&method, &[], None, flats)?;
            for v in &mut acc {
                *v = *v * (d.dx * d.dy) / ((d.dx + d.dy) / 2.0);
            }
            acc
        }
    };
    let slope = args.opt_map("slope")?.map(|s| same(m, s, "slope")).transpose()?;
    let mut t = match slope {
        Some(deg) => deg.iter().map(|v| v.to_radians().tan()).collect(),
        None => tan_slope(d),
    };
    Ok(match op {
        "twi" => {
            let min_slope: f64 = args.param("min_slope", 0.0)?;
            if min_slope > 0.0 {
                for v in &mut t {
                    *v = v.max(min_slope);
                }
            }
            a.iter().zip(&t).map(|(a, t)| if *t > 0.0 { (a / t).ln() } else { f64::NAN }).collect()
        }
        "spi" => a.iter().zip(&t).map(|(a, t)| a * t).collect(),
        _ => {
            let sin: Vec<f64> = match slope {
                None => t.iter().map(|t| t / (1.0 + t * t).sqrt()).collect(),
                Some(deg) => deg.iter().map(|v| v.to_radians().sin()).collect(),
            };
            let (mm, n): (f64, f64) = (args.param("sca_exponent", 0.4)?, args.param("slope_exponent", 1.3)?);
            a.iter()
                .zip(&sin)
                .map(|(a, s)| (mm + 1.0) * crate::npmath::pow(a / 22.13, mm) * crate::npmath::pow(s / 0.0896, n))
                .collect()
        }
    })
}

// neighbour offsets of the internal D8 codes (0 north-east, clockwise ... 7 north)
const DROW: [i64; 8] = [-1, 0, 1, 1, 1, 0, -1, -1];
const DCOL: [i64; 8] = [1, 1, 1, 0, -1, -1, -1, 0];

/// The cell each cell drains to (-1: none, off the map or missing), as landschaft's _down.
pub(crate) fn down(d: &[i8], ny: i64, nx: i64) -> Vec<i64> {
    let mut out = vec![-1; d.len()];
    for (i, &k) in d.iter().enumerate() {
        if k < 0 {
            continue;
        }
        let (r, c) = (i as i64 / nx + DROW[k as usize], i as i64 % nx + DCOL[k as usize]);
        if r >= 0 && c >= 0 && r < ny && c < nx && d[(r * nx + c) as usize] != -2 {
            out[i] = r * nx + c;
        }
    }
    out
}

/// The stream cell each outlet point stands for (landschaft's snap_outlets): the nearest stream cell within the
/// distance, of equally near ones the first in raster order.
fn snap(points: &[[f64; 2]], on: &[u8], grid: &Grid, distance: Option<f64>) -> Result<Vec<usize>> {
    let (e, lonlat) = crate::surface::extent_of(grid);
    let (ny, nx) = (grid.height, grid.width);
    let (rx, ry) = ((e[1] - e[0]) / nx as f64, (e[3] - e[2]) / ny as f64);
    let cells: Vec<usize> = (0..on.len()).filter(|&i| on[i] == 1).collect();
    if cells.is_empty() {
        return Err(Error::msg("there are no stream cells to snap the outlets to"));
    }
    let projected = crate::prepare::crs_of(grid).is_some() && !lonlat;
    let distance = distance.unwrap_or(if projected { 300.0 } else { 3.0 * rx.max(ry) });
    let mut out = Vec::with_capacity(points.len());
    for (i, &[x, y]) in points.iter().enumerate() {
        let mut best = (f64::INFINITY, 0);
        for &k in &cells {
            let (cx, cy) = (e[0] + ((k % nx) as f64 + 0.5) * rx, e[3] - ((k / nx) as f64 + 0.5) * ry);
            let d2 = (cx - x).powi(2) + (cy - y).powi(2);
            if d2 < best.0 {
                best = (d2, k);
            }
        }
        if best.0 > distance * distance * (1.0 + 1e-12) {
            return Err(Error::msg(format!("outlet {i} ({x}, {y}) is farther than {distance} from every stream cell")));
        }
        out.push(best.1);
    }
    Ok(out)
}

fn stream_links(args: &Args, m: &Map, d: &Dem, flats: bool) -> Result<Outputs> {
    let dir = d.d8(flats)?;
    let st = same(m, args.map("streams")?, "streams")?;
    let on: Vec<u8> = st.iter().zip(&dir).map(|(v, &k)| u8::from(*v == 1.0 && k != -2)).collect();
    let mut breaks = Vec::new();
    if args.get("outlets").is_some() {
        let v = crate::vector::on_grid(crate::vector::vector_of(args, "outlets")?, &m.grid)?;
        let mut pts = Vec::new();
        for g in &v.geometries {
            match g {
                Geometry::Point(p) => pts.push(*p),
                Geometry::MultiPoint(ps) => pts.extend(ps.iter().copied()),
                Geometry::Empty => {}
                _ => return Err(Error::msg("the outlets are points")),
            }
        }
        breaks = vec![0u8; on.len()];
        for k in snap(&pts, &on, &m.grid, args.opt_param("snap")?)? {
            breaks[k] = 1;
        }
    }
    let links = ffi::hydro_stream_links(&dir, &on, &breaks, d.ny, d.nx)?;
    let table = link_table(d, &dir, &links, &breaks);
    let map: Vec<f64> =
        links.link.iter().zip(&dir).map(|(&l, &k)| if k == -2 { f64::NAN } else { f64::from(l) }).collect();
    Ok(outputs([("map", Value::Map(Map::new(m.grid.clone(), map))), ("table", Value::Table(table))]))
}

/// landschaft's _link_table: by link, its downstream link, outlet, first and last cell, cells, length, elevations
/// and Strahler order.
fn link_table(d: &Dem, dir: &[i8], links: &ffi::HydroLinks, breaks: &[u8]) -> Table {
    let (ny, nx) = (i64::from(d.ny), i64::from(d.nx));
    let n = links.start.len();
    let diag = d.dx.hypot(d.dy);
    let steps = [diag, d.dx, diag, d.dy, diag, d.dx, diag, d.dy];
    let nxt = down(dir, ny, nx);
    let (mut length, mut count) = (vec![0.0; n + 1], vec![0i64; n + 1]);
    for (i, &l) in links.link.iter().enumerate() {
        if l > 0 {
            let k = dir[i];
            length[l as usize] += if k >= 0 { steps[k as usize] } else { d.dx.min(d.dy) };
            count[l as usize] += 1;
        }
    }
    // Strahler order, from the sources down
    let mut order = vec![0i64; n + 1];
    let mut ups: Vec<Vec<usize>> = vec![Vec::new(); n + 1];
    for k in 0..n {
        if links.down[k] > 0 {
            ups[links.down[k] as usize].push(k + 1);
        }
    }
    let mut pending: Vec<usize> = (1..=n).filter(|&k| ups[k].is_empty()).collect();
    let mut remaining: Vec<usize> = ups.iter().map(Vec::len).collect();
    while let Some(k) = pending.pop() {
        let u: Vec<i64> = ups[k].iter().map(|&i| order[i]).collect();
        order[k] = match u.iter().max() {
            None => 1,
            Some(&top) => {
                if u.iter().filter(|&&o| o == top).count() > 1 {
                    top + 1
                } else {
                    top
                }
            }
        };
        let dn = links.down[k - 1];
        if dn > 0 {
            remaining[dn as usize] -= 1;
            if remaining[dn as usize] == 0 {
                pending.push(dn as usize);
            }
        }
    }
    let cols = [
        "link",
        "downstream",
        "outlet",
        "start_row",
        "start_col",
        "end_row",
        "end_col",
        "cells",
        "length",
        "z_start",
        "z_end",
        "order",
    ];
    let mut t = Table::new(cols.iter().map(|c| c.to_string()).collect());
    for k in 0..n {
        let (s, e) = (links.start[k], links.end[k]);
        let below = nxt[e as usize];
        let z_end = if below >= 0 { d.z[below as usize] } else { d.z[e as usize] };
        let outlet = !breaks.is_empty() && breaks[e as usize] == 1;
        t.rows.push(vec![
            (k + 1) as f64,
            f64::from(links.down[k]),
            f64::from(u8::from(outlet)),
            (s / nx) as f64,
            (s % nx) as f64,
            (e / nx) as f64,
            (e % nx) as f64,
            count[k + 1] as f64,
            length[k + 1],
            d.z[s as usize],
            z_end,
            order[k + 1] as f64,
        ]);
    }
    t
}
