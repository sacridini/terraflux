// SPDX-License-Identifier: GPL-3.0-or-later
//! SWAT+ results on the map (landschaft's `hru_map`, `channel_map` and `reach_discharge`, in its
//! `hydro/swat/_landscape.py`): an HRU's or a channel's value painted on the cells of the watershed the project
//! came from (swat.from_tables), and the daily discharge of the channels. The tables they read are those of a
//! swat.run of the same project, connected; without them the project is run here, as landschaft does with no
//! result.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::swat::{Project, records_table};
use crate::swat_tables::{SwatTable, TABLES};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(HruMap));
    r.add(Arc::new(ChannelMap));
    r.add(Arc::new(ReachDischarge));
}

/// The HRU tables a map can show, in landschaft's order (HRU_COLUMNS).
const HRU_TABLES: [&str; 4] = ["hru_wb", "hru_nb", "hru_ls", "hru_pw"];
const HOW: [&str; 4] = ["annual", "mean", "max", "sum"];

fn def(name: &str) -> &'static SwatTable {
    TABLES.iter().find(|t| t.name == name).expect("a table of SWAT+")
}

/// The project of the input, the SWAT+ one.
fn project_of<'a>(args: &'a Args) -> Result<Option<&'a Arc<Project>>> {
    match args.get("project") {
        Some(Value::SwatProject(p)) => Ok(Some(p)),
        Some(v) => Err(Error::msg(format!("the input project is a {:?}, not a SWAT+ project", v.ty()))),
        None => Ok(None),
    }
}

fn watershed_of(p: &Project) -> Result<&Arc<crate::swat_watershed::Watershed>> {
    p.watershed.as_ref().ok_or_else(|| {
        Error::msg(
            "the project has no map of its HRUs and channels: connect a project built by swat.from_tables from a \
             swat.watershed (not one of swat.load)",
        )
    })
}

fn table_input<'a>(args: &'a Args, port: &str) -> Result<Option<&'a Table>> {
    match args.get(port) {
        Some(Value::Table(t)) => Ok(Some(t)),
        Some(v) => Err(Error::msg(format!("the input {port} is a {:?}, not a table", v.ty()))),
        None => Ok(None),
    }
}

/// The settings that run the project here: the years skipped and C's method.
fn run_params() -> [PortSpec; 2] {
    [
        PortSpec::optional(
            "nyskip",
            Type::Integer,
            "when the project is run here (no table connected): the years simulated before printing (default \
             print.prt's)",
        ),
        PortSpec::optional(
            "c_factor",
            Type::Text,
            "when the project is run here: MUSLE's C, swatplus (default) or plant (the plants' minimum)",
        )
        .choices(["swatplus", "plant"]),
    ]
}

/// The table `name` at `step` from a run of the project made here: only it is printed.
fn simulate(ctx: &mut Ctx, args: &Args, project: &Project, name: &str, step: &str) -> Result<Table> {
    let c_factor: String = args.param("c_factor", "swatplus".to_string())?;
    if c_factor != "swatplus" && c_factor != "plant" {
        return Err(Error::msg(format!("c_factor must be swatplus or plant, got {c_factor:?}")));
    }
    let print: Vec<(&str, Vec<&str>)> =
        TABLES.iter().map(|t| (t.name, if t.name == name { vec![step] } else { Vec::new() })).collect();
    let mut p = project.clone();
    p.set_print(&print, args.opt_param("nyskip")?)?;
    let out = p.run(c_factor == "plant")?;
    for w in &out.warnings {
        ctx.warn(format!("node {:?}: SWAT+: {w}", args.node));
    }
    let t = def(name);
    let k = crate::swat::STEPS.iter().position(|s| *s == step).unwrap() as i32;
    let r = out.prints.iter().find(|r| r.tag == t.tag + k).ok_or_else(|| {
        Error::msg(format!("SWAT+ did not print {name} by {step}: the project's print.prt has no such table"))
    })?;
    Ok(records_table(r, Some(t.object), t.columns))
}

/// The table `name` at `step` (annual: `aa`; else the daily one): the connected one, checked, or a run here.
fn source<'a>(
    ctx: &mut Ctx,
    args: &'a Args,
    project: Option<&Project>,
    name: &str,
    annual: bool,
) -> Result<Cow<'a, Table>> {
    let step = if annual { "aa" } else { "day" };
    match table_input(args, name)? {
        Some(t) => {
            check_step(t, name, annual)?;
            Ok(Cow::Borrowed(t))
        }
        None => {
            let p = project.ok_or_else(|| {
                Error::msg(format!("connect a project (swat.from_tables), or the table {name} of a swat.run"))
            })?;
            Ok(Cow::Owned(simulate(ctx, args, p, name, step)?))
        }
    }
}

/// What a connected table must be: a swat.run's, by the step the result needs (the average annual one has a row
/// for each object; the daily one has its days in a row).
fn check_step(t: &Table, name: &str, annual: bool) -> Result<()> {
    let object = def(name).object;
    let at = t.columns.iter().position(|c| c == object);
    if at != Some(2) || t.columns.get(..2) != Some(&["year".to_string(), "day".to_string()]) {
        return Err(Error::msg(format!(
            "the table connected to {name} is not the {name} of a swat.run (year, day, {object} and the columns)"
        )));
    }
    if t.rows.is_empty() {
        return Err(Error::msg(format!(
            "the table {name} is empty: SWAT+ printed nothing (set swat.run's tables to include {name} and nyskip \
             below the years simulated)"
        )));
    }
    if annual {
        let mut seen = std::collections::HashSet::new();
        if !t.rows.iter().all(|r| seen.insert(r[2] as i64)) {
            return Err(Error::msg(format!(
                "the table {name} has several rows for an object: it is not the average annual one; set swat.run's \
                 step to aa (with {name} in its tables), or use how mean, max or sum for the daily values"
            )));
        }
    } else {
        let first = t.rows[0][2];
        let days: Vec<(f64, f64)> = t.rows.iter().filter(|r| r[2] == first).map(|r| (r[0], r[1])).collect();
        let daily = days.len() > 1
            && days
                .windows(2)
                .all(|w| (w[1].0 == w[0].0 && w[1].1 == w[0].1 + 1.0) || (w[1].0 == w[0].0 + 1.0 && w[1].1 == 1.0));
        if !daily {
            return Err(Error::msg(format!(
                "the table {name} is not a daily one (the days of its first object do not follow each other); set \
                 swat.run's step to day (with {name} in its tables), or use how annual with the step aa"
            )));
        }
    }
    Ok(())
}

fn how_of(args: &Args, default: &str) -> Result<String> {
    let how: String = args.param("how", default.to_string())?;
    if HOW.contains(&how.as_str()) {
        Ok(how)
    } else {
        Err(Error::msg(format!("how must be 'annual', 'mean', 'max' or 'sum', got {how:?}")))
    }
}

/// A column by object, as landschaft's `_by_object`: the value of the average annual table, or the mean, the
/// largest or the sum of the daily ones (pandas' groupby on float32: the sum and the mean with Kahan's
/// compensation).
fn by_object(t: &Table, column: &str, how: &str) -> Result<Vec<(i64, f64)>> {
    let j = t
        .columns
        .iter()
        .position(|c| c == column)
        .ok_or_else(|| Error::msg(format!("the table has no column {column:?}")))?;
    if how == "annual" {
        return Ok(t.rows.iter().map(|r| (r[2] as i64, r[j])).collect());
    }
    struct Acc {
        sum: f32,
        comp: f32,
        n: i64,
        max: f32,
    }
    let mut order: Vec<i64> = Vec::new();
    let mut acc: HashMap<i64, Acc> = HashMap::new();
    for r in &t.rows {
        let id = r[2] as i64;
        let a = acc.entry(id).or_insert_with(|| {
            order.push(id);
            Acc { sum: 0.0, comp: 0.0, n: 0, max: f32::NAN }
        });
        let v = r[j] as f32;
        if v.is_nan() {
            continue;
        }
        a.n += 1;
        let y = v - a.comp;
        let s = a.sum + y;
        a.comp = (s - a.sum) - y;
        a.sum = s;
        if a.max.is_nan() || v > a.max {
            a.max = v;
        }
    }
    Ok(order
        .iter()
        .map(|id| {
            let a = &acc[id];
            let v = match how {
                "sum" => a.sum,
                "max" => a.max,
                _ if a.n == 0 => f32::NAN,
                _ => a.sum / a.n as f32,
            };
            (*id, f64::from(v))
        })
        .collect())
}

/// The values of a table of two columns, the number of the object and the value (landschaft's pandas Series).
fn series_of(t: &Table) -> Result<Vec<(i64, f64)>> {
    if t.columns.len() < 2 {
        return Err(Error::msg("the table of values needs two columns: the number of the object and its value"));
    }
    Ok(t.rows.iter().map(|r| (r[0] as i64, r[1])).collect())
}

fn project_port(optional: bool) -> PortSpec {
    let help = "the project (swat.from_tables, swat.set)";
    if optional {
        PortSpec::optional("project", Type::SwatProject, help)
    } else {
        PortSpec::new("project", Type::SwatProject, help)
    }
}

struct HruMap;

impl Operator for HruMap {
    fn signature(&self) -> OpSignature {
        let mut variables: Vec<&'static str> = Vec::new();
        for n in HRU_TABLES {
            for c in def(n).columns {
                if !variables.contains(c) {
                    variables.push(c);
                }
            }
        }
        let mut sig = OpSignature::new(
            "swat.hru_map",
            "An HRU result of SWAT+ on the map (landschaft's hru_map): each cell takes the value of its HRU (NaN \
             outside the watershed and on cells whose land use, soil and slope combination was folded into other \
             HRUs). The project must carry its watershed (swat.from_tables). It reads the table of the variable from \
             a swat.run of the same project (hru_wb, hru_nb, hru_ls or hru_pw: the average annual one, step aa, for \
             how annual; the daily one, step day, for the others), so that several variables of one run do not \
             simulate it again; with no table connected it runs the project itself.",
            Access::None,
        )
        .input(project_port(false))
        .input(PortSpec::optional(
            "values",
            Type::Table,
            "instead of a variable: values by HRU, a table of the HRU's number (from 1) and the value",
        ));
        for n in HRU_TABLES {
            sig = sig.input(PortSpec::optional(n, Type::Table, "the table of a swat.run of the project, to read"));
        }
        sig = sig
            .output(PortSpec::new("map", Type::Map, "the value of each cell's HRU, on the DEM's grid"))
            .param(
                PortSpec::optional(
                    "variable",
                    Type::Text,
                    "a column of an HRU table: e.g. sedyld (the soil loss, t/ha), wateryld, surq_gen, et (mm), bioms \
                     (kg/ha)",
                )
                .choices(variables),
            )
            .param(
                PortSpec::optional(
                    "how",
                    Type::Text,
                    "annual (default): SWAT+'s average annual value (the yearly total of a flux, the mean of a state, \
                     after the years print.prt skips); or of the daily values: mean, max, sum",
                )
                .choices(HOW),
            );
        for p in run_params() {
            sig = sig.param(p);
        }
        sig
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        hru_map(ctx, args).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

fn hru_map(ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
    let project = project_of(args)?;
    let w = watershed_of(project.ok_or_else(|| Error::msg("connect a project (swat.from_tables)"))?)?;
    let how = how_of(args, "annual")?;
    let variable: Option<String> = args.opt_param("variable")?;
    let values = match (table_input(args, "values")?, &variable) {
        (Some(t), _) => series_of(t)?,
        (None, Some(v)) => {
            let name = HRU_TABLES
                .iter()
                .find(|n| def(n).columns.contains(&v.as_str()))
                .ok_or_else(|| Error::msg(format!("unknown HRU output {v:?}")))?;
            let t = source(ctx, args, project.map(|p| &**p), name, how == "annual")?;
            by_object(&t, v, &how)?
        }
        (None, None) => return Err(Error::msg("choose the variable (or connect the values by HRU)")),
    };
    let lut: HashMap<i64, f64> = values.into_iter().collect();
    let ids = &w.hrus;
    let data = ids
        .data
        .iter()
        .map(|&id| {
            if id.is_finite() {
                lut.get(&(id as i64)).copied().filter(|_| id as i64 >= 0).unwrap_or(f64::NAN)
            } else {
                f64::NAN
            }
        })
        .collect();
    let mut out = Outputs::new();
    out.insert("map".into(), Value::Map(Map::new(ids.grid.clone(), data)));
    Ok(out)
}

struct ChannelMap;

impl Operator for ChannelMap {
    fn signature(&self) -> OpSignature {
        let mut sig = OpSignature::new(
            "swat.channel_map",
            "A channel result of SWAT+ on the stream cells (landschaft's channel_map; by default the mean discharge, \
             m3/s): NaN off the streams. The project must carry its watershed (swat.from_tables). It reads the \
             channel_sd table of a swat.run of the same project (the average annual one, step aa, for how annual; \
             the daily one, step day, for the others); with none connected it runs the project itself.",
            Access::None,
        )
        .input(project_port(false))
        .input(PortSpec::optional(
            "values",
            Type::Table,
            "instead of a column: values by channel, a table of the channel's number (from 1) and the value",
        ))
        .input(PortSpec::optional(
            "channel_sd",
            Type::Table,
            "the channel_sd table of a swat.run of the project, to read",
        ))
        .output(PortSpec::new("map", Type::Map, "the value of each channel, on the channels' cells"))
        .param(
            PortSpec::optional("column", Type::Text, "a column of the channel table (default flo_out, m3/s)")
                .choices(def("channel_sd").columns.iter().copied()),
        )
        .param(
            PortSpec::optional(
                "how",
                Type::Text,
                "mean (default), max or sum of the daily values; or annual: SWAT+'s average annual value",
            )
            .choices(HOW),
        );
        for p in run_params() {
            sig = sig.param(p);
        }
        sig
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        channel_map(ctx, args).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

fn channel_map(ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
    let project = project_of(args)?;
    let w = watershed_of(project.ok_or_else(|| Error::msg("connect a project (swat.from_tables)"))?)?;
    let how = how_of(args, "mean")?;
    let values = match table_input(args, "values")? {
        Some(t) => series_of(t)?,
        None => {
            let column: String = args.param("column", "flo_out".to_string())?;
            if !def("channel_sd").columns.contains(&column.as_str()) {
                return Err(Error::msg(format!("unknown channel output {column:?}")));
            }
            let t = source(ctx, args, project.map(|p| &**p), "channel_sd", how == "annual")?;
            by_object(&t, &column, &how)?
        }
    };
    let lut: HashMap<i64, f64> = values.into_iter().collect();
    let ch = &w.channels;
    let data = ch
        .data
        .iter()
        .map(|&id| {
            if id.is_finite() && id > 0.0 && id as i64 > 0 {
                lut.get(&(id as i64)).copied().unwrap_or(f64::NAN)
            } else {
                f64::NAN
            }
        })
        .collect();
    let mut out = Outputs::new();
    out.insert("map".into(), Value::Map(Map::new(ch.grid.clone(), data)));
    Ok(out)
}

struct ReachDischarge;

impl Operator for ReachDischarge {
    fn signature(&self) -> OpSignature {
        let mut sig = OpSignature::new(
            "swat.reach_discharge",
            "The daily discharge leaving each channel (landschaft's reach_discharge; m3/s, channel_sd's flo_out): a \
             row per date (year, day of the year) and a column per channel (channel_1...). It reads the daily \
             channel_sd table (step day) of a swat.run of the project; with none connected it runs the project \
             itself.",
            Access::None,
        )
        .input(project_port(true))
        .input(PortSpec::optional("channel_sd", Type::Table, "the daily channel_sd table of a swat.run, to read"))
        .output(PortSpec::new("table", Type::Table, "the discharge by date and channel (m3/s)"));
        for p in run_params() {
            sig = sig.param(p);
        }
        sig
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        reach_discharge(ctx, args).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

fn reach_discharge(ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
    let project = project_of(args)?;
    let t = source(ctx, args, project.map(|p| &**p), "channel_sd", false)?;
    let j =
        t.columns.iter().position(|c| c == "flo_out").ok_or_else(|| Error::msg("the table has no column flo_out"))?;
    let mut channels: Vec<i64> = t.rows.iter().map(|r| r[2] as i64).collect();
    channels.sort_unstable();
    channels.dedup();
    let col: HashMap<i64, usize> = channels.iter().enumerate().map(|(k, c)| (*c, k)).collect();
    let mut dates: Vec<(f64, f64)> = t.rows.iter().map(|r| (r[0], r[1])).collect();
    dates.sort_by(|a, b| a.partial_cmp(b).unwrap());
    dates.dedup();
    let row: HashMap<(u64, u64), usize> =
        dates.iter().enumerate().map(|(k, d)| ((d.0 as u64, d.1 as u64), k)).collect();
    let mut cols = vec!["year".to_string(), "day".to_string()];
    cols.extend(channels.iter().map(|c| format!("channel_{c}")));
    let mut out = Table::new(cols);
    out.rows = dates
        .iter()
        .map(|d| {
            let mut r = vec![f64::NAN; channels.len() + 2];
            r[0] = d.0;
            r[1] = d.1;
            r
        })
        .collect();
    for r in &t.rows {
        out.rows[row[&(r[0] as u64, r[1] as u64)]][2 + col[&(r[2] as i64)]] = r[j];
    }
    let mut o = Outputs::new();
    o.insert("table".into(), Value::Table(out));
    Ok(o)
}
