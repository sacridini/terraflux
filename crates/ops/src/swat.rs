//! SWAT+ (landschaft's `ls.hydro.swat`): the watershed model of USDA-ARS and Texas A&M, SWAT+ 62.0.1 ported to C++
//! by landschaft and run through it, the results of SWAT+ itself bit for bit for the part the port covers (weather,
//! HRUs, routing units, aquifers, SWAT-DEG channels): a TxtInOut project run, and the tables SWAT+ prints.

use std::io::Read;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::swat_tables::TABLES;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Run));
}

/// The print steps of SWAT+ (print.prt's columns), in the order of the record tags.
pub const STEPS: [&str; 4] = ["day", "month", "year", "aa"];

/// climate_module's weather_daily, in the order the port keeps them.
pub const WEATHER_COLUMNS: [&str; 16] = [
    "precip",
    "precip_next",
    "tmax",
    "tmin",
    "tave",
    "solrad",
    "solradmx",
    "rhum",
    "dewpt",
    "windsp",
    "pet",
    "wndir",
    "phubase0",
    "ppet",
    "daylength",
    "precip_half_hr",
];

/// A SWAT+ project: the files of a TxtInOut folder by name (LF line ends, a final newline), as landschaft's
/// SwatProject holds them.
#[derive(Debug, Clone, Default)]
pub struct Project {
    pub files: Vec<(String, Vec<u8>)>,
}

/// What SWAT+ writes (outputs of earlier runs found in a TxtInOut folder): landschaft's _OUTPUT.
fn is_output(name: &str) -> bool {
    let l = name.to_lowercase();
    [".txt", ".out", ".fin", ".csv", ".bin", ".log"].iter().any(|e| l.ends_with(e))
        || l.strip_prefix("fort.").is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()))
}

/// LF line ends and a final newline (the Fortran runtime reads records).
fn normalize(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len() + 1);
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\r' {
            out.push(b'\n');
            if b.get(i + 1) == Some(&b'\n') {
                i += 1;
            }
        } else {
            out.push(b[i]);
        }
        i += 1;
    }
    if !out.is_empty() && !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    out
}

impl Project {
    /// A TxtInOut folder, or a zip file of one (landschaft's load_txtinout).
    pub fn load(path: &std::path::Path) -> Result<Project> {
        let mut files = Vec::new();
        let zip = path.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"));
        if zip {
            let base = format!("/vsizip/{}", path.to_string_lossy().replace('\\', "/"));
            let mut stack = vec![base.clone()];
            let mut found = Vec::new();
            while let Some(dir) = stack.pop() {
                for n in terraflux_gdal::read_dir(&dir).map_err(Error::msg)? {
                    let full = format!("{dir}/{n}");
                    if terraflux_gdal::read_dir(&full).is_ok_and(|v| !v.is_empty()) {
                        stack.push(full);
                    } else {
                        found.push((n, full));
                    }
                }
            }
            found.sort();
            for (name, full) in found {
                if is_output(&name) {
                    continue;
                }
                let mut f = terraflux_gdal::VsiFile::open(&full).map_err(Error::msg)?;
                let mut b = Vec::new();
                f.read_to_end(&mut b).map_err(|e| Error::msg(format!("{full}: {e}")))?;
                files.push((name, normalize(&b)));
            }
        } else if path.is_dir() {
            let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(path)
                .map_err(|e| Error::msg(format!("{}: {e}", path.display())))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file())
                .collect();
            entries.sort();
            for p in entries {
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                if is_output(&name) {
                    continue;
                }
                let b = std::fs::read(&p).map_err(|e| Error::msg(format!("{}: {e}", p.display())))?;
                files.push((name, normalize(&b)));
            }
        } else {
            return Err(Error::msg(format!("not a folder or a zip file: {}", path.display())));
        }
        let project = Project { files };
        if project.key("file.cio").is_none() {
            return Err(Error::msg(format!("no file.cio in {}: not a SWAT+ TxtInOut folder", path.display())));
        }
        Ok(project)
    }

    /// The index of a file, its name matched without regard to case.
    fn key(&self, name: &str) -> Option<usize> {
        let low = name.to_lowercase();
        self.files.iter().position(|(k, _)| k.to_lowercase() == low)
    }

    pub fn text(&self, name: &str) -> Result<String> {
        let k = self.key(name).ok_or_else(|| Error::msg(format!("no file {name:?} in the project")))?;
        Ok(String::from_utf8_lossy(&self.files[k].1).into_owned())
    }

    pub fn set_text(&mut self, name: &str, text: &str) {
        let b = normalize(text.as_bytes());
        match self.key(name) {
            Some(k) => self.files[k].1 = b,
            None => self.files.push((name.to_string(), b)),
        }
    }

    /// The file named on a line of file.cio (its third word), else a default.
    fn cio(&self, line: &str, default: &str) -> String {
        if let Ok(t) = self.text("file.cio") {
            for l in t.split('\n') {
                let p: Vec<&str> = l.split_whitespace().collect();
                if p.len() >= 3 && p[0] == line {
                    return p[2].to_string();
                }
            }
        }
        default.to_string()
    }

    /// Choose what SWAT+ prints (landschaft's SwatProject.set_print): the years skipped and, for each table,
    /// the steps printed.
    pub fn set_print(&mut self, tables: &[(&str, Vec<&str>)], nyskip: Option<i64>) -> Result<()> {
        let name = self.cio("simulation", "print.prt");
        let text = self.text(&name)?;
        let mut lines: Vec<String> = text.split('\n').map(String::from).collect();
        if let Some(n) = nyskip {
            let mut vals: Vec<String> = lines[2].split_whitespace().map(String::from).collect();
            vals[0] = n.to_string();
            lines[2] = vals.iter().map(|v| format!("{v:<10}")).collect::<Vec<_>>().join(" ");
        }
        for (table, steps) in tables {
            let row = format!("{table:<24}")
                + &STEPS
                    .iter()
                    .map(|s| format!("{:>14}", if steps.contains(s) { "y" } else { "n" }))
                    .collect::<String>();
            let k = lines
                .iter()
                .position(|l| {
                    let p: Vec<&str> = l.split_whitespace().collect();
                    p.len() == 5 && p[0] == *table
                })
                .ok_or_else(|| Error::msg(format!("{name} has no row {table:?}")))?;
            lines[k] = row;
        }
        self.set_text(&name, &lines.join("\n"));
        Ok(())
    }

    /// landschaft's own check: a peak rate factor written for an earlier SWAT+ (the 5th value of parameters.bsn).
    fn check(&self) -> Option<String> {
        let name = self.cio("basin", "parameters.bsn");
        let text = self.text(&name).ok()?;
        let row: Vec<&str> = text.lines().nth(2)?.split_whitespace().collect();
        let prf: f64 = row.get(4)?.parse().ok()?;
        (prf < 10.0).then(|| {
            format!(
                "the peak rate factor prf is {prf} (5th value of {name}); SWAT+ 62 takes 484 (the SWAT+ Editor 4 writes \
                 it): the file seems written for an earlier SWAT+, and peak rates (hence erosion) come out about {:.0} \
                 times too low",
                484.0 / prf.max(1e-6)
            )
        })
    }

    /// Simulate it, as SWAT+ 62.0.1 does.
    pub fn run(&self, c_factor_plant: bool) -> Result<ffi::SwatOut> {
        let names: Vec<String> = self.files.iter().map(|(n, _)| n.clone()).collect();
        let mut data = Vec::new();
        let mut offsets = vec![0i64];
        for (_, b) in &self.files {
            data.extend_from_slice(b);
            offsets.push(data.len() as i64);
        }
        Ok(ffi::swat_run(&names, &data, &offsets, c_factor_plant)?)
    }
}

/// A table of records: year, day of the year, the object (none for the basin) and the columns.
fn records_table(r: &ffi::SwatRecords, object: Option<&str>, columns: &[&str]) -> Table {
    let mut cols = vec!["year".to_string(), "day".to_string()];
    cols.extend(object.map(String::from));
    cols.extend(columns.iter().map(|c| c.to_string()));
    let mut t = Table::new(cols);
    let w = r.width as usize;
    for (i, key) in r.keys.chunks(3).enumerate() {
        let mut row = vec![f64::from(key[0]), f64::from(key[1])];
        if object.is_some() {
            row.push(f64::from(key[2]));
        }
        row.extend(r.values[i * w..i * w + columns.len().min(w)].iter().map(|&v| f64::from(v)));
        t.rows.push(row);
    }
    t
}

struct Run;

impl Operator for Run {
    fn signature(&self) -> OpSignature {
        let mut sig = OpSignature::new(
            "swat.run",
            "Simulate a SWAT+ project (a TxtInOut folder or a zip of one) as SWAT+ 62.0.1 does, through landschaft's \
             port of it (bit for bit with SWAT+ for what the port covers: measured and generated weather, HRUs with \
             snow, ET, curve number runoff, MUSLE erosion, percolation, lateral flow, nutrients, plant growth and \
             management, routing units, aquifers, SWAT-DEG channels). The tables SWAT+ prints, at a step: a row per \
             date (year, day of the year) and object (HRU, channel, aquifer; the basin's by date).",
            Access::None,
        )
        .output(PortSpec::new(
            "weather",
            Type::Table,
            "the daily weather of each station (year, day, station)",
        ));
        for t in &TABLES {
            sig = sig.output(PortSpec::new(t.name, Type::Table, "what SWAT+ prints (columns and units as its own)"));
        }
        sig.param(PortSpec::new("path", Type::Text, "the TxtInOut folder (with file.cio) or a zip of it"))
            .param(
                PortSpec::optional(
                    "step",
                    Type::Text,
                    "of the tables: day, month, year (default) or aa (average annual)",
                )
                .choices(STEPS),
            )
            .param(PortSpec::optional(
                "tables",
                Type::Any,
                "the tables printed (default every one): hru_wb, hru_nb, hru_ls, hru_pw, basin_wb, basin_nb, \
                 basin_ls, basin_pw, channel_sd, aquifer",
            ))
            .param(PortSpec::optional("nyskip", Type::Integer, "years simulated before printing (spin-up)"))
            .param(
                PortSpec::optional(
                    "c_factor",
                    Type::Text,
                    "MUSLE's C: swatplus (default, SWAT+ 62's from APEX) or plant (the plants' minimum, SWAT+'s \
                     earlier method)",
                )
                .choices(["swatplus", "plant"]),
            )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        run(ctx, args).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

fn run(ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
    let path: String = args.param("path", String::new())?;
    if path.trim().is_empty() {
        return Err(Error::msg("choose the TxtInOut folder"));
    }
    let mut project = Project::load(std::path::Path::new(path.trim()))?;
    let step: String = args.param("step", "year".to_string())?;
    if !STEPS.contains(&step.as_str()) {
        return Err(Error::msg(format!("step must be day, month, year or aa, got {step:?}")));
    }
    let chosen: Vec<String> = match args.opt_param::<Vec<String>>("tables")? {
        Some(t) => t,
        None => TABLES.iter().map(|t| t.name.to_string()).collect(),
    };
    if let Some(bad) = chosen.iter().find(|c| !TABLES.iter().any(|t| t.name == c.as_str())) {
        return Err(Error::msg(format!("unknown table {bad:?}")));
    }
    let c_factor: String = args.param("c_factor", "swatplus".to_string())?;
    if c_factor != "swatplus" && c_factor != "plant" {
        return Err(Error::msg(format!("c_factor must be swatplus or plant, got {c_factor:?}")));
    }
    // the tables chosen printed at the step, the others not at all
    let print: Vec<(&str, Vec<&str>)> = TABLES
        .iter()
        .map(|t| (t.name, if chosen.iter().any(|c| c == t.name) { vec![step.as_str()] } else { Vec::new() }))
        .collect();
    project.set_print(&print, args.opt_param("nyskip")?)?;
    if let Some(w) = project.check() {
        ctx.warn(format!("node {:?}: {w}", args.node));
    }
    let out = project.run(c_factor == "plant")?;
    for w in &out.warnings {
        ctx.warn(format!("node {:?}: SWAT+: {w}", args.node));
    }
    let mut result = Outputs::new();
    if let Some(w) = out.dumps.iter().find(|r| r.tag == 1) {
        result.insert("weather".into(), Value::Table(records_table(w, Some("station"), &WEATHER_COLUMNS)));
    }
    let k = STEPS.iter().position(|s| *s == step).unwrap() as i32;
    for t in &TABLES {
        if let Some(r) = out.prints.iter().find(|r| r.tag == t.tag + k) {
            let object = (t.object != "basin").then_some(t.object);
            result.insert(t.name.into(), Value::Table(records_table(r, object, t.columns)));
        }
    }
    Ok(result)
}
