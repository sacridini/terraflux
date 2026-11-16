//! Zonation 5's impact assessment files (impacts/<scenario>/...), as terraflux-py writes them (`_impacts.write_source`
//! and `write_cumulative`): per measure, area and impact group the features' distribution and loss in the zones of
//! the sources (with value bins), and the direct and cumulative impacts per feature.

use std::path::Path;

use terraflux_raster::{DataType, Grid};

use crate::zonation::{CumulativeFiles, ImpactFiles, SourceFiles};
use crate::{Error, Result};

const MEASURES: [&str; 5] =
    ["area", "fractional_distribution", "fractional_loss", "absolute_distribution", "absolute_loss"];

/// What the files say of the features kept: their numbers (0-based among those given), the sums of their input
/// values, the name in a row and the unique name of the binned files.
pub struct Layers<'a> {
    pub numbers: &'a [usize],
    pub abs_sums: &'a [f64],
    pub names: &'a [String],
    pub unique: &'a [String],
}

type E = [f64; 3];

/// C's `%f`.
fn f(x: f64) -> String {
    if x.is_nan() {
        "nan".into()
    } else if x.is_infinite() {
        if x > 0.0 { "inf".into() } else { "-inf".into() }
    } else {
        format!("{x:.6}")
    }
}

fn add(a: E, b: E) -> E {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: E, b: E) -> E {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn unit(area_unit: &str, impact_unit: &str, measure: &str) -> String {
    let n = if measure == "area" {
        area_unit
    } else if measure.starts_with("absolute") {
        impact_unit
    } else {
        ""
    };
    if n.is_empty() { String::new() } else { format!("_{n}") }
}

fn value(measure: &str, e: E, abs_sum: f64, factor: f64) -> f64 {
    match measure {
        "area" => e[2] * factor,
        "fractional_distribution" => e[0],
        "fractional_loss" => e[1],
        "absolute_distribution" => e[0] * abs_sum,
        _ => e[1] * abs_sum,
    }
}

fn put(path: &Path, lines: &[String]) -> Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| Error::msg(format!("{}: {e}", d.display())))?;
    }
    std::fs::write(path, lines.join("\n") + "\n").map_err(|e| Error::msg(format!("{}: {e}", path.display())))
}

fn mkdir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(|e| Error::msg(format!("{}: {e}", path.display())))
}

/// outputSourceAnalysisFiles: impacts/<scenario>/<area>/...
fn write_source(root: &Path, st: &SourceFiles, imp: &ImpactFiles, l: &Layers) -> Result<()> {
    let out = root.join(&st.name);
    let get = |key: Vec<i64>| st.acc.get(&key).copied().unwrap_or([0.0; 3]);
    let areas: Vec<i64> = (0..=st.areas.len() as i64).collect();
    for measure in MEASURES {
        let un = unit(&st.area_unit, &st.impact_unit, measure);
        let val = |e: E, k: usize| value(measure, e, l.abs_sums[k], st.area_factor);
        for &a in &areas {
            let aname = if a == 0 { "global".to_string() } else { format!("area{}", st.areas[a as usize - 1]) };
            let apath = out.join(&aname);
            mkdir(&apath)?;
            for (&g, layers) in &imp.group_layers {
                let Some(nz) = st.nzones.get(&g) else { continue };
                let sources: Vec<i64> = nz.keys().copied().collect();
                let zone_e = |s: i64, k: usize, b: i64, zn: usize| get(vec![1, a, g, s, k as i64, b, zn as i64]);
                let src_total =
                    |s: i64, k: usize, b: i64| (0..nz[&s]).fold([0.0; 3], |t, zn| add(t, zone_e(s, k, b, zn)));
                let group_file = |path: &Path, layers: &[usize], binned: bool| -> Result<()> {
                    let kind = if binned { "bin_low" } else { "feature" };
                    let mut head = format!("{kind} total_all{un} total_footprints{un} total_zones{un}");
                    for s in &sources {
                        head += &format!(" s{s}total{un} s{s}footprint{un} s{s}zones{un}");
                    }
                    if !binned {
                        head += " name";
                    }
                    let mut lines = vec![head];
                    for &k in layers {
                        let mut row = |b: i64, label: String| {
                            let mut tf = [0.0; 3];
                            let mut parts = String::new();
                            for &s in &sources {
                                let stt = src_total(s, k, b);
                                let fp = zone_e(s, k, b, 0);
                                tf = add(tf, fp);
                                parts += &format!(" {} {} {}", f(val(stt, k)), f(val(fp, k)), f(val(sub(stt, fp), k)));
                            }
                            let tot = get(vec![0, a, g, k as i64, b]);
                            let mut line = format!(
                                "{label} {} {} {}{parts}",
                                f(val(tot, k)),
                                f(val(tf, k)),
                                f(val(sub(tot, tf), k))
                            );
                            if b < 0 {
                                line += &format!(" {}", l.names[k]);
                            }
                            lines.push(line);
                        };
                        if binned {
                            for (bi, (lo, _)) in imp.bins[&k].iter().enumerate() {
                                row(bi as i64, f(f64::from((f64::from(*lo) * l.abs_sums[k]) as f32)));
                            }
                        } else {
                            row(-1, l.numbers[k].to_string());
                        }
                    }
                    put(path, &lines)
                };
                group_file(&apath.join(format!("{aname}_ig{g}.{measure}.csv")), layers, false)?;
                let binned_layers: Vec<usize> = layers.iter().copied().filter(|k| imp.bins.contains_key(k)).collect();
                if !binned_layers.is_empty() {
                    mkdir(&apath.join("binned"))?;
                    for &k in &binned_layers {
                        let name = format!("{aname}_ig{g}.binned.{}.{measure}.csv", l.unique[k]);
                        group_file(&apath.join("binned").join(name), &[k], true)?;
                    }
                }
                for &s in &sources {
                    let spath = apath.join(format!("source{s}"));
                    mkdir(&spath)?;
                    let source_file = |path: &Path, binned: Option<usize>| -> Result<()> {
                        let kind = if binned.is_some() { "bin_low" } else { "feature" };
                        let mut head = format!("{kind} total_source{un} footprint{un} total_zones{un}");
                        for zn in 1..nz[&s] {
                            head += &format!(" z{zn}{un}");
                        }
                        if binned.is_none() {
                            head += " name";
                        }
                        let mut lines = vec![head];
                        let mut zrow = |k: usize, b: i64, label: String, name: bool| {
                            let zs: Vec<E> = (0..nz[&s]).map(|zn| zone_e(s, k, b, zn)).collect();
                            let t = zs.iter().fold([0.0; 3], |t, e| add(t, *e));
                            let mut line =
                                format!("{label} {} {} {}", f(val(t, k)), f(val(zs[0], k)), f(val(sub(t, zs[0]), k)));
                            for e in &zs[1..] {
                                line += &format!(" {}", f(val(*e, k)));
                            }
                            if name {
                                line += &format!(" {}", l.names[k]);
                            }
                            lines.push(line);
                        };
                        if let Some(k) = binned {
                            for (bi, (lo, _)) in imp.bins[&k].iter().enumerate() {
                                zrow(k, bi as i64, f(f64::from((f64::from(*lo) * l.abs_sums[k]) as f32)), false);
                            }
                        } else {
                            for &k in layers {
                                zrow(k, -1, l.numbers[k].to_string(), true);
                            }
                        }
                        put(path, &lines)
                    };
                    source_file(&spath.join(format!("{aname}_source{s}_ig{g}.{measure}.csv")), None)?;
                    for &k in layers {
                        if imp.bins.contains_key(&k) {
                            mkdir(&spath.join("binned"))?;
                            let name = format!("{aname}_source{s}_ig{g}.binned.{}.{measure}.csv", l.unique[k]);
                            source_file(&spath.join("binned").join(name), Some(k))?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// outputCumulativeAnalysisFiles: impacts/<scenario>/impacts.<measure>.csv
fn write_cumulative(root: &Path, ct: &CumulativeFiles, imp: &ImpactFiles, l: &Layers) -> Result<()> {
    let out = root.join(&ct.name);
    mkdir(&out)?;
    let get = |key: Vec<i64>| ct.acc.get(&key).copied().unwrap_or([0.0; 3]);
    let areas: Vec<i64> = (0..=ct.areas.len() as i64).collect();
    for measure in MEASURES {
        let un = unit(&ct.area_unit, &ct.impact_unit, measure);
        let mut head = format!("feature remaining{un}[total] ");
        for &a in &areas {
            let label = if a == 0 { "total".to_string() } else { format!("SA{}", ct.areas[a as usize - 1]) };
            head += &format!("direct{un}[{label}] cumulative{un}[{label}] ");
        }
        let mut lines = vec![head];
        let val = |e: E, k: usize, remaining: bool| -> f64 {
            let [imp_, loss, area] = e;
            match measure {
                "area" => (if remaining { ct.maxarea[&k] - area } else { area }) * ct.area_factor,
                "fractional_distribution" => {
                    if remaining {
                        1.0 - imp_
                    } else {
                        imp_
                    }
                }
                "fractional_loss" => {
                    if remaining {
                        1.0 - loss
                    } else {
                        loss
                    }
                }
                "absolute_distribution" => (if remaining { 1.0 - imp_ } else { imp_ }) * l.abs_sums[k],
                _ => (if remaining { 1.0 - loss } else { loss }) * l.abs_sums[k],
            }
        };
        for &k in ct.maxarea.keys() {
            let tot = get(vec![0, 0, k as i64]);
            let mut line = format!("{} {} ", l.numbers[k], f(val(tot, k, true)));
            for &a in &areas {
                let (mut d, mut c) = ([0.0; 3], [0.0; 3]);
                for &g in imp.group_layers.keys() {
                    d = add(d, get(vec![1, a, g, k as i64]));
                    c = add(c, get(vec![2, a, g, k as i64]));
                }
                line += &format!("{} {} ", f(val(d, k, false)), f(val(c, k, false)));
            }
            lines.push(line);
        }
        put(&out.join(format!("impacts.{measure}.csv")), &lines)?;
    }
    Ok(())
}

/// impacts/ of a ranking: the scenarios' tables and the maps of the groups (`ig<g>_zones` in the source
/// scenario's folder, the others in the cumulative one's).
pub fn write(dir: &Path, imp: &ImpactFiles, maps: &[(String, Vec<f32>)], l: &Layers, grid: &Grid) -> Result<()> {
    let root = dir.join("impacts");
    if let Some(st) = &imp.source {
        write_source(&root, st, imp, l)?;
        write_maps(&root.join(&st.name), maps, true, grid)?;
    }
    if let Some(ct) = &imp.cumulative {
        write_cumulative(&root, ct, imp, l)?;
        write_maps(&root.join(&ct.name), maps, false, grid)?;
    }
    Ok(())
}

fn write_maps(dir: &Path, maps: &[(String, Vec<f32>)], zones: bool, grid: &Grid) -> Result<()> {
    for (name, v) in maps.iter().filter(|(n, _)| n.ends_with("_zones") == zones) {
        let data: Vec<f64> = v.iter().map(|x| f64::from(*x)).collect();
        let nodata = data.iter().any(|x| x.is_nan()).then_some(f64::NAN);
        crate::zonation_files::write_tif(&dir.join(format!("{name}.tif")), grid, &data, DataType::F32, nodata)?;
    }
    Ok(())
}
