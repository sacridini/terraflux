//! Crop pollination by wild bees (InVEST's pollination model, natcap.invest 3.20) as landschaft 1.54 computes it
//! (`ls.services.pollination`): the nesting and floral resources of the land uses (and of the farms), each
//! species' supply and abundance by season through the convolutions of its flight distance (landschaft's C++),
//! and on the farms the pollinator abundance and the yields.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::{LandUse, lookup, table_of, text_table, threads, zonal};
use crate::change::numpy_sum;
use crate::{Args, Ctx, Error, Key, Operator, Outputs, Result, Stack, Table, Value};

pub(super) struct Pollination;

impl Operator for Pollination {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "services.pollination",
            "Crop pollination by wild bees: InVEST's pollination model (natcap.invest 3.20) bit for bit, through \
             landschaft. Each species nests where the land uses (and the farms) offer its substrates and forages \
             on their flowers by season within its flight distance (an exponential kernel of mean alpha); its \
             supply and its abundance by season follow, summed over the species by season; with farms, the \
             pollinator abundance on them (their half saturation), the total and wild pollinator yields and the \
             means by farm.",
            Access::Global,
        )
        .input(PortSpec::new("lulc", Type::Map, "the land use (class codes; projected, metres)"))
        .input(PortSpec::new(
            "biophysical",
            Type::Table,
            "by class: lucode, nesting_<substrate>_availability_index, floral_resources_<season>_index",
        ))
        .input(PortSpec::optional(
            "farms",
            Type::Vector,
            "farm polygons: crop_type, half_sat, season, p_dep, p_managed, fr_<season>, n_<substrate>",
        ))
        .output(PortSpec::new("supply", Type::Stack, "the pollinator supply of each species"))
        .output(PortSpec::new(
            "abundance",
            Type::Stack,
            "the abundance of each species in each season (<species>_<season>)",
        ))
        .output(PortSpec::new("total_abundance", Type::Stack, "the abundance of all species, by season"))
        .output(PortSpec::new("farm_pollinators", Type::Map, "the pollinator abundance on the farms (with farms)"))
        .output(PortSpec::new("total_yield", Type::Map, "the yield owed to pollinators, wild and managed"))
        .output(PortSpec::new("wild_yield", Type::Map, "the yield owed to wild pollinators"))
        .output(PortSpec::new("farm_table", Type::Table, "by farm: p_abund, y_tot, pdep_y_w, y_wild"))
        .param(PortSpec::new(
            "guilds",
            Type::Any,
            "a list of {species, alpha (metres), relative_abundance, foraging_activity_<season>_index, \
             nesting_suitability_<substrate>_index}, or InVEST's guild table (a file)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        pollination(args).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

/// A guild table: the species sorted (as InVEST), its seasons and substrates sorted, the values by species.
struct Guilds {
    species: Vec<String>,
    seasons: Vec<String>,
    substrates: Vec<String>,
    alpha: Vec<f64>,
    relative: Vec<f64>,
    activity: Vec<Vec<f64>>,
    suitability: Vec<Vec<f64>>,
}

/// The names in the columns `<prefix><name><suffix>` (the name without an underscore), sorted.
fn names_in<'a>(columns: impl Iterator<Item = &'a str>, prefix: &str, suffix: &str) -> Vec<String> {
    let mut out: Vec<String> = columns
        .filter_map(|c| c.strip_prefix(prefix).and_then(|r| r.strip_suffix(suffix)))
        .filter(|s| !s.is_empty() && !s.contains('_'))
        .map(String::from)
        .collect();
    out.sort();
    out
}

fn check_names(found: &[String], wanted: &[String], kind: &str, what: &str) -> Result<()> {
    let mut f = found.to_vec();
    f.sort();
    if f != wanted {
        return Err(Error::msg(format!("the {what} must name the {kind} of the guild table, {wanted:?}: got {f:?}")));
    }
    Ok(())
}

fn guilds_of(args: &Args) -> Result<Guilds> {
    let v = args.params.get("guilds").ok_or_else(|| Error::msg("give the guilds (a row per species)"))?;
    let rows: Vec<serde_json::Map<String, serde_json::Value>> = match v {
        serde_json::Value::String(path) => {
            let text = std::fs::read_to_string(path.trim()).map_err(|e| Error::msg(format!("{path}: {e}")))?;
            let t = text_table(&text);
            let head: Vec<String> = t.first().map(|h| h.iter().map(|c| c.to_lowercase()).collect()).unwrap_or_default();
            t.iter()
                .skip(1)
                .map(|r| head.iter().cloned().zip(r.iter().map(|x| serde_json::Value::String(x.clone()))).collect())
                .collect()
        }
        serde_json::Value::Array(a) => a
            .iter()
            .map(|r| {
                r.as_object()
                    .map(|o| o.iter().map(|(k, v)| (k.trim().to_lowercase(), v.clone())).collect())
                    .ok_or_else(|| Error::msg("each guild is an object: species, alpha, relative_abundance..."))
            })
            .collect::<Result<_>>()?,
        _ => return Err(Error::msg("the guilds are a list of {species, alpha, ...} or a file")),
    };
    let columns: Vec<String> = rows.first().map(|r| r.keys().cloned().collect()).unwrap_or_default();
    for c in ["species", "alpha", "relative_abundance"] {
        if !columns.iter().any(|k| k == c) {
            return Err(Error::msg(format!("the guild table lacks the column {c:?}")));
        }
    }
    let seasons = names_in(columns.iter().map(String::as_str), "foraging_activity_", "_index");
    let substrates = names_in(columns.iter().map(String::as_str), "nesting_suitability_", "_index");
    if seasons.is_empty() {
        return Err(Error::msg("the guild table has no foraging_activity_<season>_index column"));
    }
    if substrates.is_empty() {
        return Err(Error::msg("the guild table has no nesting_suitability_<substrate>_index column"));
    }
    let num = |r: &serde_json::Map<String, serde_json::Value>, k: &str| -> Result<f64> {
        match r.get(k) {
            Some(serde_json::Value::Number(n)) => Ok(n.as_f64().unwrap()),
            Some(serde_json::Value::String(s)) => {
                s.trim().parse().map_err(|_| Error::msg(format!("the guilds' {k} {s:?} is not a number")))
            }
            _ => Err(Error::msg("the guild table has empty values")),
        }
    };
    let mut table: Vec<(String, &serde_json::Map<String, serde_json::Value>)> = rows
        .iter()
        .map(|r| {
            let name = match r.get("species") {
                Some(serde_json::Value::String(s)) => s.trim().to_string(),
                Some(v) if !v.is_null() => v.to_string(),
                _ => String::new(),
            };
            (name, r)
        })
        .collect();
    table.sort_by(|a, b| a.0.cmp(&b.0));
    if table.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err(Error::msg("the guild table lists a species more than once"));
    }
    let mut g = Guilds {
        species: Vec::new(),
        seasons,
        substrates,
        alpha: Vec::new(),
        relative: Vec::new(),
        activity: Vec::new(),
        suitability: Vec::new(),
    };
    for (name, r) in &table {
        g.species.push(name.clone());
        g.alpha.push(num(r, "alpha")?);
        g.relative.push(num(r, "relative_abundance")?);
        g.activity
            .push(g.seasons.iter().map(|s| num(r, &format!("foraging_activity_{s}_index"))).collect::<Result<_>>()?);
        g.suitability.push(
            g.substrates.iter().map(|s| num(r, &format!("nesting_suitability_{s}_index"))).collect::<Result<_>>()?,
        );
    }
    if g.alpha.iter().any(|a| a.is_nan() || *a <= 0.0) {
        return Err(Error::msg("alpha (the mean flight distance, metres) must be positive"));
    }
    Ok(g)
}

/// The farms: on the land use's grid, polygons, with the columns the model reads (names not case-sensitive).
struct Farms {
    v: Arc<Vector>,
    season: Vec<String>,
    column: std::collections::BTreeMap<String, Vec<f64>>,
}

fn farms_of(v: &Arc<Vector>, grid: &Grid, g: &Guilds) -> Result<Farms> {
    let lower: Vec<(String, &Column)> = v.attributes.iter().map(|(n, c)| (n.to_lowercase(), c)).collect();
    let mut need: Vec<String> = ["crop_type", "half_sat", "season", "p_dep", "p_managed"].map(String::from).to_vec();
    need.extend(g.seasons.iter().map(|s| format!("fr_{s}")));
    need.extend(g.substrates.iter().map(|s| format!("n_{s}")));
    let missing: Vec<&String> = need.iter().filter(|c| !lower.iter().any(|(n, _)| n == *c)).collect();
    if !missing.is_empty() {
        return Err(Error::msg(format!("the farms lack the column(s) {missing:?}")));
    }
    check_names(
        &names_in(lower.iter().map(|c| c.0.as_str()), "fr_", ""),
        &g.seasons,
        "seasons",
        "farms' fr_<season> columns",
    )?;
    check_names(
        &names_in(lower.iter().map(|c| c.0.as_str()), "n_", ""),
        &g.substrates,
        "substrates",
        "farms' n_<substrate> columns",
    )?;
    let get = |name: &str| lower.iter().find(|(n, _)| n == name).unwrap().1;
    let season: Vec<String> = match get("season") {
        Column::Texts(t) => t.iter().map(|s| s.to_string()).collect(),
        Column::Numbers(x) => x.iter().map(|s| s.to_string()).collect(),
    };
    let mut unknown: Vec<&String> = season.iter().filter(|s| !g.seasons.contains(s)).collect();
    unknown.sort();
    unknown.dedup();
    if !unknown.is_empty() {
        return Err(Error::msg(format!(
            "the farms name season(s) {unknown:?} that the tables lack (seasons: {:?})",
            g.seasons
        )));
    }
    if v.geometries.iter().any(|g| !g.is_polygonal() && !matches!(g, Geometry::Empty)) {
        return Err(Error::msg("the farms must be polygons"));
    }
    let column =
        need.iter().filter(|c| *c != "season" && *c != "crop_type").map(|c| (c.clone(), get(c).numbers())).collect();
    // InVEST reprojects the farms to the land use map
    Ok(Farms { v: crate::vector::on_grid(v, grid)?, season, column })
}

/// InVEST's _rasterize_vector_onto_base: the values of the farms `keep` (float32) burnt over `base` (NaN: none).
fn burn(f: &Farms, keep: &[usize], column: &str, grid: &Grid, base: Option<&[f32]>) -> Result<Vec<f32>> {
    let mut out = base.map(<[f32]>::to_vec).unwrap_or_else(|| vec![f32::NAN; grid.cells()]);
    if !keep.is_empty() {
        let sub = f.v.select(keep);
        let all = &f.column[column];
        let values: Vec<f64> = keep.iter().map(|&i| all[i]).collect();
        let mut burned = vec![f64::NAN; grid.cells()];
        crate::vector::burn(&mut burned, grid, &sub, &values, false, false)?;
        for (o, b) in out.iter_mut().zip(&burned) {
            if !b.is_nan() {
                *o = *b as f32;
            }
        }
    }
    Ok(out)
}

fn stack(grid: &Arc<Grid>, layers: Vec<(String, Vec<f32>)>) -> Value {
    let (keys, layers): (Vec<Key>, Vec<Arc<Vec<f64>>>) =
        layers.into_iter().map(|(k, v)| (Key::Name(k), Arc::new(v.iter().map(|&x| f64::from(x)).collect()))).unzip();
    Value::Stack(Stack { grid: grid.clone(), keys, layers })
}

fn pollination(args: &Args) -> Result<Outputs> {
    let lu = LandUse::new(args.map("lulc")?, "the land use map")?;
    let grid = lu.map.grid.clone();
    let n = grid.cells();
    let g = guilds_of(args)?;
    let mut cols: Vec<String> = g.substrates.iter().map(|s| format!("nesting_{s}_availability_index")).collect();
    cols.extend(g.seasons.iter().map(|s| format!("floral_resources_{s}_index")));
    let t = table_of(args, "biophysical")?;
    let lc: Vec<String> = t.columns.iter().map(|c| c.trim().to_lowercase()).collect();
    check_names(
        &names_in(lc.iter().map(String::as_str), "nesting_", "_availability_index"),
        &g.substrates,
        "substrates",
        "biophysical table's nesting columns",
    )?;
    check_names(
        &names_in(lc.iter().map(String::as_str), "floral_resources_", "_index"),
        &g.seasons,
        "seasons",
        "biophysical table's floral columns",
    )?;
    let col_refs: Vec<&str> = cols.iter().map(String::as_str).collect();
    let (codes, table) = lookup(t, &col_refs, "the biophysical table", "lucode")?;
    let cls = lu.classes(&codes)?;
    // utils.reclassify_raster to float32
    let reclass =
        |v: &[f64]| -> Vec<f32> { cls.iter().map(|&c| if c >= 0 { v[c as usize] as f32 } else { f32::NAN }).collect() };
    let nsub = g.substrates.len();
    let mut nesting: Vec<Vec<f32>> = (0..nsub).map(|s| reclass(&table[s])).collect();
    let mut floral: Vec<Vec<f32>> = (0..g.seasons.len()).map(|j| reclass(&table[nsub + j])).collect();
    let farms = match args.get("farms") {
        Some(_) => Some(farms_of(crate::vector::vector_of(args, "farms")?, &grid, &g)?),
        None => None,
    };
    if let Some(f) = &farms {
        let all: Vec<usize> = (0..f.v.len()).collect();
        for (s, sub) in g.substrates.iter().enumerate() {
            nesting[s] = burn(f, &all, &format!("n_{sub}"), &grid, Some(&nesting[s]))?;
        }
        for (j, se) in g.seasons.iter().enumerate() {
            floral[j] = burn(f, &all, &format!("fr_{se}"), &grid, Some(&floral[j]))?;
        }
    }
    // the scalars as InVEST computes them from its pandas tables (numpy float64)
    let total_rel = numpy_sum(&g.relative);
    let (dx, dy) = lu.pixel()?;
    let pixel = if (dx - dy).abs() <= 1e-8 + 1e-5 * dy.abs() { dx } else { dx.min(dy) };
    let (ny, nx) = (grid.height as i64, grid.width as i64);
    let nest_flat: Vec<f32> = nesting.concat();
    let floral_flat: Vec<f32> = floral.concat();
    let nseason = g.seasons.len();
    let mut total: Vec<Vec<f32>> = vec![vec![f32::NAN; n]; nseason];
    let mut kernels: Vec<(u64, Vec<f32>)> = Vec::new();
    let (mut supply, mut abundance) = (Vec::new(), Vec::new());
    for (k, sp) in g.species.iter().enumerate() {
        let act = &g.activity[k];
        let sum = numpy_sum(act);
        let fa: Vec<f64> = act.iter().map(|a| a / sum).collect();
        let local = ffi::serv_poll_local(&nest_flat, &g.suitability[k], &floral_flat, &fa, n as i64, threads())?;
        let (hn, fe) = (&local[..n], &local[n..2 * n]);
        let alpha = g.alpha[k] / pixel;
        // one kernel per alpha (InVEST shares a kernel between alphas equal to 6 decimals)
        let kernel = match kernels.iter().find(|x| x.0 == alpha.to_bits()) {
            Some(x) => x.1.clone(),
            None => {
                let kv = ffi::serv_poll_kernel(alpha, alpha * 5.0)?;
                kernels.push((alpha.to_bits(), kv.clone()));
                kv
            }
        };
        let fr = ffi::serv_poll_convolve(fe, ny, nx, &kernel, threads())?;
        let sa = g.relative[k] / total_rel;
        let ps = ffi::serv_poll_supply(hn, &fr, sa, threads())?;
        let cps = ffi::serv_poll_convolve(&ps, ny, nx, &kernel, threads())?;
        for (j, se) in g.seasons.iter().enumerate() {
            let ff = &local[(2 + j) * n..(3 + j) * n];
            let pa = ffi::serv_poll_abundance(ff, &fr, &cps, &mut total[j], threads())?;
            abundance.push((format!("{sp}_{se}"), pa));
        }
        supply.push((sp.clone(), ps));
    }
    let mut out = Outputs::new();
    out.insert("supply".into(), stack(&grid, supply));
    out.insert("abundance".into(), stack(&grid, abundance));
    out.insert("total_abundance".into(), stack(&grid, g.seasons.iter().cloned().zip(total.iter().cloned()).collect()));
    let Some(f) = farms else { return Ok(out) };
    let mut farm_total = vec![f32::NAN; n];
    for (j, se) in g.seasons.iter().enumerate() {
        let keep: Vec<usize> = (0..f.v.len()).filter(|&i| f.season[i] == *se).collect();
        let h = burn(&f, &keep, "half_sat", &grid, None)?;
        ffi::serv_poll_farm(&h, &total[j], &mut farm_total, threads())?;
    }
    let all: Vec<usize> = (0..f.v.len()).collect();
    let mp = burn(&f, &all, "p_managed", &grid, None)?;
    let y = ffi::serv_poll_yield(&mp, &farm_total, threads())?;
    let (pyt, pyw) = y.split_at(n);
    // InVEST's farm results: means over each farm of the yields and of the abundance in its season
    let wide = |v: &[f32]| -> Vec<f64> { v.iter().map(|&x| f64::from(x)).collect() };
    let mut maps = vec![wide(pyt), wide(pyw)];
    maps.extend(total.iter().map(|t| wide(t)));
    let st = zonal(&f.v, &grid, &maps.iter().map(|m| m.as_slice()).collect::<Vec<_>>(), &[2, 0])?;
    let nm = maps.len();
    let stat = |z: usize, k: usize| (st[(z * nm + k) * 2], st[(z * nm + k) * 2 + 1]);
    let mut farm_table = Table::new(["farm", "p_abund", "y_tot", "pdep_y_w", "y_wild"].map(String::from).to_vec());
    for (z, nu) in f.column["p_dep"].iter().enumerate() {
        let (st_t, ct) = stat(z, 0);
        let (sw, cw) = stat(z, 1);
        let j = g.seasons.iter().position(|s| *s == f.season[z]).unwrap();
        let (sa, ca) = stat(z, 2 + j);
        let row = if ct > 0.0 {
            let mean_w = sw / cw;
            vec![sa / ca, 1.0 - nu * (1.0 - st_t / ct), mean_w, nu * mean_w]
        } else {
            vec![f64::NAN; 4]
        };
        farm_table.rows.push(std::iter::once((z + 1) as f64).chain(row).collect());
    }
    out.insert("farm_pollinators".into(), super::f32_map(&grid, &farm_total));
    out.insert("total_yield".into(), super::f32_map(&grid, pyt));
    out.insert("wild_yield".into(), super::f32_map(&grid, pyw));
    out.insert("farm_table".into(), Value::Table(farm_table));
    Ok(out)
}
