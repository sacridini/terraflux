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
    r.add(Arc::new(Load));
    r.add(Arc::new(Set));
    r.add(Arc::new(Run));
}

/// The parameters SWAT+'s hard calibration changes in the port (calibration.cpp's cal_parm_select, the objects
/// the port covers; not the plants', routing channels' or reservoirs'): name, kind of object (cal_parms.cal's
/// obj_typ: hru, sol, bsn) and the default limits of cal_parms.cal (the project's own, when it has the
/// parameter, are the ones used).
pub const PARAMETERS: [(&str, &str, f64, f64); 69] = [
    ("cn2", "hru", 35.0, 95.0),
    ("cn3_swf", "hru", 0.0, 1.0),
    ("usle_p", "hru", 0.0, 1.0),
    ("ovn", "hru", 0.01, 30.0),
    ("elev", "hru", 0.0, 5000.0),
    ("slope", "hru", 0.0001, 0.9),
    ("slope_len", "hru", 10.0, 150.0),
    ("lat_ttime", "hru", 0.5, 180.0),
    ("lat_sed", "hru", 0.0, 5000.0),
    ("lat_len", "hru", 1.0, 150.0),
    ("latq_co", "hru", 0.0, 1.0),
    ("canmx", "hru", 0.0, 100.0),
    ("esco", "hru", 0.0, 1.0),
    ("epco", "hru", 0.0, 1.0),
    ("erorgn", "hru", 0.0, 5.0),
    ("erorgp", "hru", 0.0, 5.0),
    ("dis_stream", "hru", 0.0, 100000.0),
    ("biomix", "hru", 0.0, 1.0),
    ("perco", "hru", 0.0, 1.0),
    ("petco", "hru", 0.7, 1.3),
    ("lat_orgn", "hru", 0.0, 200.0),
    ("lat_orgp", "hru", 0.0, 200.0),
    ("field_len", "hru", 0.01, 2.0),
    ("field_wid", "hru", 0.01, 2.0),
    ("field_ang", "hru", 0.0, 360.0),
    ("snofall_tmp", "hru", -5.0, 5.0),
    ("snomelt_tmp", "hru", -5.0, 5.0),
    ("snomelt_max", "hru", 0.0, 10.0),
    ("snomelt_min", "hru", 0.0, 10.0),
    ("snomelt_lag", "hru", 0.0, 1.0),
    ("anion_excl", "sol", 0.1, 0.9),
    ("crk", "sol", 0.0, 1.0),
    ("z", "sol", 0.0, 3500.0),
    ("bd", "sol", 0.9, 2.5),
    ("awc", "sol", 0.01, 1.0),
    ("k", "sol", 0.0001, 2000.0),
    ("cbn", "sol", 0.05, 10.0),
    ("clay", "sol", 0.0, 100.0),
    ("silt", "sol", 0.0, 100.0),
    ("sand", "sol", 0.0, 100.0),
    ("rock", "sol", 0.0, 100.0),
    ("alb", "sol", 0.0, 0.25),
    ("usle_k", "sol", 0.0, 0.65),
    ("ec", "sol", 0.0, 100.0),
    ("cal", "sol", 0.0, 65.0),
    ("ph", "sol", 3.0, 10.0),
    ("plaps", "bsn", 0.0, 200.0),
    ("tlaps", "bsn", -10.0, 10.0),
    ("surlag", "bsn", 0.05, 24.0),
    ("adj_pkr", "bsn", 0.5, 2.0),
    ("prf", "bsn", 0.0, 2.0),
    ("evrch", "bsn", 0.5, 1.0),
    ("evlai", "bsn", 0.0, 10.0),
    ("ffcb", "bsn", 0.0, 1.0),
    ("cmn", "bsn", 0.001, 0.003),
    ("nperco", "bsn", 0.0, 1.0),
    ("pperco", "bsn", 10.0, 17.5),
    ("phoskd", "bsn", 100.0, 200.0),
    ("psp", "bsn", 0.01, 0.7),
    ("rsdco", "bsn", 0.02, 0.1),
    ("percop", "bsn", 0.0, 1.0),
    ("msk_co1", "bsn", 0.0, 10.0),
    ("msk_co2", "bsn", 0.0, 10.0),
    ("msk_x", "bsn", 0.0, 0.3),
    ("cdn", "bsn", 0.0, 3.0),
    ("tb_adj", "bsn", 0.0, 1.0),
    ("sdnco", "bsn", 0.0, 1.0),
    ("n_updis", "bsn", 0.0, 100.0),
    ("dorm_hr", "bsn", 0.0, 24.0),
];

/// How a calibration changes a value (calibration.cpp's chg_par).
pub const CHANGES: [&str; 3] = ["pctchg", "abschg", "absval"];

/// Files as text in JSON: each byte a character (ISO 8859-1), so any file comes back as it went.
mod bytes_as_text {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(files: &[(String, Vec<u8>)], s: S) -> Result<S::Ok, S::Error> {
        let v: Vec<(&str, String)> =
            files.iter().map(|(n, b)| (n.as_str(), b.iter().map(|&c| c as char).collect())).collect();
        v.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<(String, Vec<u8>)>, D::Error> {
        let v: Vec<(String, String)> = Vec::deserialize(d)?;
        v.into_iter()
            .map(|(n, t)| {
                let b = t
                    .chars()
                    .map(|c| {
                        u8::try_from(u32::from(c)).map_err(|_| serde::de::Error::custom("a file with a byte over 255"))
                    })
                    .collect::<Result<Vec<u8>, _>>()?;
                Ok((n, b))
            })
            .collect()
    }
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
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Project {
    #[serde(with = "bytes_as_text")]
    pub files: Vec<(String, Vec<u8>)>,
}

/// What SWAT+ writes (outputs of earlier runs found in a TxtInOut folder): landschaft's _OUTPUT.
fn is_output(name: &str) -> bool {
    let l = name.to_lowercase();
    [".txt", ".out", ".fin", ".csv", ".bin", ".log"].iter().any(|e| l.ends_with(e))
        || l.strip_prefix("fort.").is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()))
}

/// The files of a TxtInOut folder a project holds (sorted; SWAT+'s own outputs and hidden files left out): what [`Project::load`]
/// reads, and what is sent to workers and watched for changes.
pub fn folder_files(dir: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            // (hidden files out too: a bundle's folder on a worker holds a `.complete` mark)
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            p.is_file() && !name.starts_with('.') && !is_output(&name)
        })
        .collect();
    entries.sort();
    Ok(entries)
}

/// Whether a folder is a SWAT+ TxtInOut folder (has a file.cio).
pub fn is_txtinout(dir: &std::path::Path) -> bool {
    folder_files(dir).is_ok_and(|v| {
        v.iter().any(|p| p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("file.cio")))
    })
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
            for p in folder_files(path).map_err(|e| Error::msg(format!("{}: {e}", path.display())))? {
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
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

    /// The files a line of file.cio names, the label left out.
    fn cio_items(&self, line: &str) -> Vec<String> {
        let t = self.text("file.cio").unwrap_or_default();
        for l in t.split('\n') {
            let p: Vec<&str> = l.split_whitespace().collect();
            if p.first() == Some(&line) {
                return p[1..].iter().map(|s| s.to_string()).collect();
            }
        }
        Vec::new()
    }

    /// Name the `k`th file of a line of file.cio (the line's other files kept).
    fn set_cio_item(&mut self, line: &str, k: usize, name: &str) -> Result<()> {
        let t = self.text("file.cio")?;
        let mut lines: Vec<String> = t.split('\n').map(String::from).collect();
        let at = lines
            .iter()
            .position(|l| l.split_whitespace().next() == Some(line))
            .ok_or_else(|| Error::msg(format!("file.cio has no line {line:?}")))?;
        let mut words: Vec<String> = lines[at].split_whitespace().map(String::from).collect();
        if words.len() <= k + 1 {
            words.resize(k + 2, "null".to_string());
        }
        words[k + 1] = name.to_string();
        // the columns of the SWAT+ Editor's file.cio: a label and the files, 18 characters each
        lines[at] = words.iter().map(|w| format!("{w:<17} ")).collect();
        self.set_text("file.cio", &lines.join("\n"));
        Ok(())
    }

    /// The file of a calibration (the `k`th of file.cio's chg line), named in file.cio when it was not.
    fn calibration_file(&mut self, k: usize, default: &str) -> Result<String> {
        let items = self.cio_items("chg");
        match items.get(k) {
            Some(n) if n != "null" => Ok(n.clone()),
            _ => {
                self.set_cio_item("chg", k, default)?;
                Ok(default.to_string())
            }
        }
    }

    /// Add a row to a file of a count and a header (cal_parms.cal, calibration.cal: a title, the number of rows, the
    /// header, the rows): after the counted rows, the count one more.
    fn add_row(&mut self, name: &str, title: &str, header: &str, row: &str) {
        let text = self.text(name).unwrap_or_else(|_| format!("{name}: {title}\n0\n{header}\n"));
        let mut lines: Vec<String> = text.split('\n').map(String::from).collect();
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        let n: usize = lines.get(1).and_then(|l| l.trim().parse().ok()).unwrap_or(0);
        let at = (3 + n).min(lines.len());
        lines.insert(at, row.to_string());
        lines[1] = (n + 1).to_string();
        self.set_text(name, &lines.join("\n"));
    }

    /// Change a parameter as SWAT+'s hard calibration does (calibration.cal, which the port reads as it starts the
    /// run): a row added to the project's calibration.cal, after the changes it has, and the parameter's to its
    /// cal_parms.cal when this lacks it. `layers` are the soil layers changed (0: from the first, to the last),
    /// `objects` the HRUs (empty: all; as SWAT+'s lists, `1 -5 8` is 1 to 5 and 8).
    pub fn set_parameter(
        &mut self,
        parameter: &str,
        change: &str,
        value: f64,
        layers: (i64, i64),
        objects: &[i64],
    ) -> Result<()> {
        let Some(&(_, kind, min, max)) = PARAMETERS.iter().find(|p| p.0 == parameter) else {
            let plants = ["usle_c", "bio_e", "hvsti", "rdmx", "t_opt", "t_base", "phu_mat", "lai_pot", "harv_idx"];
            return Err(Error::msg(if plants.contains(&parameter) {
                format!("the plants' parameters ({parameter}) are not calibrated by the port")
            } else {
                format!("the parameter {parameter:?} is not one the port calibrates")
            }));
        };
        if !CHANGES.contains(&change) {
            return Err(Error::msg(format!("change must be pctchg, abschg or absval, got {change:?}")));
        }
        if !value.is_finite() {
            return Err(Error::msg(format!("the value must be a number, got {value}")));
        }
        if layers != (0, 0) && kind != "sol" {
            return Err(Error::msg(format!("{parameter} is not a soil parameter: it has no layers")));
        }
        if layers.0 < 0 || layers.1 < 0 || (layers.1 > 0 && layers.1 < layers.0.max(1)) {
            return Err(Error::msg(format!("the soil layers {} to {} are not a range", layers.0, layers.1)));
        }
        if !objects.is_empty() && kind == "bsn" {
            return Err(Error::msg(format!("{parameter} is the basin's: it has no objects to choose")));
        }
        if objects.contains(&0) {
            return Err(Error::msg("the objects are numbered from 1"));
        }
        let parms = self.calibration_file(0, "cal_parms.cal")?;
        let known =
            self.text(&parms).is_ok_and(|t| t.lines().skip(3).any(|l| l.split_whitespace().next() == Some(parameter)));
        if !known {
            let row = format!("{parameter:<30}{kind:>10}{min:>14.5}{max:>14.5}{:>18}  ", "null");
            self.add_row(
                &parms,
                "written by Terraflux",
                "name                       obj_typ       abs_min       abs_max             units  ",
                &row,
            );
        }
        let upd = self.calibration_file(1, "calibration.cal")?;
        let mut row = format!(
            "{parameter:<24}{change:>10}{value:>14}{:>8}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}",
            0,
            layers.0,
            layers.1,
            0,
            0,
            0,
            0,
            objects.len()
        );
        for o in objects {
            row += &format!("{o:>8}");
        }
        self.add_row(
            &upd,
            "written by Terraflux",
            "cal_parm               chg_typ       chg_val     conds  soil_lyr1  soil_lyr2       yr1       yr2      day1      day2   obj_tot",
            &row,
        );
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

fn project_out(p: Project) -> Outputs {
    let mut o = Outputs::new();
    o.insert("project".into(), Value::SwatProject(Arc::new(p)));
    o
}

struct Load;

impl Operator for Load {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "swat.load",
            "Load a SWAT+ project (landschaft's load_txtinout): a TxtInOut folder (with file.cio) or a zip of one, as \
             a value to run (swat.run) or to change (swat.set). The files SWAT+ itself wrote in earlier runs are left \
             out.",
            Access::None,
        )
        .output(PortSpec::new("project", Type::SwatProject, "the project"))
        .param(PortSpec::new("path", Type::Text, "the TxtInOut folder (with file.cio) or a zip of it"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let path: String = args.param("path", String::new())?;
        if path.trim().is_empty() {
            return Err(Error::msg(format!("node {:?}: choose the TxtInOut folder", args.node)));
        }
        let project = Project::load(std::path::Path::new(path.trim()))
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        if let Some(w) = project.check() {
            ctx.warn(format!("node {:?}: {w}", args.node));
        }
        Ok(project_out(project))
    }
}

struct Set;

impl Operator for Set {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "swat.set",
            "A SWAT+ project with a parameter changed, as SWAT+'s hard calibration does (calibration.cal, which the run \
             applies before it starts): the soil's, the HRUs' or the basin's, by a percentage, an amount or a value, \
             in the soil layers or the HRUs chosen. The project given is not changed; nodes in a chain make their \
             changes in order. Settings read a model input (\"$x\"), which is how a batch or a calibration varies \
             them. SWAT+'s conditions and the plants' parameters are not supported by the port.",
            Access::None,
        )
        .input(PortSpec::new("project", Type::SwatProject, "the project (swat.load, swat.set)"))
        .output(PortSpec::new("project", Type::SwatProject, "the project with the change"))
        .param(
            PortSpec::new("parameter", Type::Text, "the parameter changed (cal_parms.cal's name)")
                .choices(PARAMETERS.iter().map(|p| p.0)),
        )
        .param(
            PortSpec::optional(
                "change",
                Type::Text,
                "how: pctchg (a percentage of the value, 10 is 10% more), abschg (an amount added) or absval (the value \
                 itself; default)",
            )
            .choices(["absval", "pctchg", "abschg"]),
        )
        .param(PortSpec::new("value", Type::Number, "the percentage, the amount or the value"))
        .param(PortSpec::optional(
            "first_layer",
            Type::Integer,
            "of a soil parameter: the first layer changed (default the first)",
        ))
        .param(PortSpec::optional(
            "last_layer",
            Type::Integer,
            "of a soil parameter: the last layer changed (default the last)",
        ))
        .param(PortSpec::optional(
            "objects",
            Type::Any,
            "the HRUs changed, by number from 1 (a list; as SWAT+'s lists, 1, -5, 8 is 1 to 5 and 8; default every one)",
        ))
    }

    fn run(&self, _: &mut Ctx, args: &Args) -> Result<Outputs> {
        let bad = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let Some(Value::SwatProject(p)) = args.get("project") else {
            return Err(bad(Error::msg("connect a project (swat.load)")));
        };
        let parameter: String = args.param("parameter", String::new())?;
        let change: String = args.param("change", "absval".to_string())?;
        let value: f64 = args.opt_param("value")?.ok_or_else(|| bad(Error::msg("give the value")))?;
        let first: i64 = args.param("first_layer", 0)?;
        let last: i64 = args.param("last_layer", 0)?;
        let objects: Vec<i64> = args.opt_param("objects")?.unwrap_or_default();
        let mut project = (**p).clone();
        project.set_parameter(&parameter, &change, value, (first, last), &objects).map_err(bad)?;
        Ok(project_out(project))
    }
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
        sig.input(PortSpec::optional(
            "project",
            Type::SwatProject,
            "the project (swat.load, swat.set), instead of path",
        ))
        .param(PortSpec::optional(
            "path",
            Type::Text,
            "the TxtInOut folder (with file.cio) or a zip of it, instead of a project",
        ))
        .param(
            PortSpec::optional("step", Type::Text, "of the tables: day, month, year (default) or aa (average annual)")
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
    let mut project = match (args.get("project"), path.trim().is_empty()) {
        (Some(Value::SwatProject(p)), true) => (**p).clone(),
        (Some(Value::SwatProject(_)), false) => {
            return Err(Error::msg("give a project or a path, not both (a path loads the project itself)"));
        }
        (Some(v), _) => return Err(Error::msg(format!("the input project is a {:?}, not a SWAT+ project", v.ty()))),
        (None, true) => return Err(Error::msg("connect a project (swat.load) or choose the TxtInOut folder")),
        (None, false) => {
            let project = Project::load(std::path::Path::new(path.trim()))?;
            if let Some(w) = project.check() {
                ctx.warn(format!("node {:?}: {w}", args.node));
            }
            project
        }
    };
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
