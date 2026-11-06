//! The methods of a sensitivity analysis as a paper tells them: one paragraph from what was run (the method, the
//! design, the factors and their ranges, the output and its statistic, the replicates, whether the indices
//! settled), and the references of the method. Written as `methods.md` in the analysis's folder, and shown with a
//! button to copy it. The papers are those of `methods.rs` (the website's "Cite Terraflux" page); no others.

use std::path::{Path, PathBuf};

use terraflux_dist::sensitivity::{Design, Indices, Method, TOLERANCE, runs};

use crate::methods::{Reference, reference, software_apa};

/// What an analysis ran, for its paragraph.
pub struct Run<'a> {
    pub indices: &'a Indices,
    pub design: &'a Design,
    /// The range of each factor, in the order of `indices.factors`.
    pub bounds: &'a [(f64, f64)],
    /// The output measured (`node.port`) and the statistic of it (none: a number, or a table's last column).
    pub output: &'a str,
    pub statistic: Option<&'a str>,
    /// The bootstraps of the intervals.
    pub resamples: usize,
    pub version: &'a str,
}

/// What the statistic of an output measures, as words: "the mean of `m.map`".
fn measured(output: &str, statistic: Option<&str>) -> String {
    let stat = statistic.map(str::trim).filter(|s| !s.is_empty());
    let Some(stat) = stat else { return output.to_string() };
    let mut parts = stat.split(':');
    let kind = parts.next().unwrap_or_default();
    let class = parts.next().unwrap_or_default();
    match kind {
        "mean" => format!("the mean of {output}"),
        "sum" => format!("the sum of {output}"),
        "min" => format!("the minimum of {output}"),
        "max" => format!("the maximum of {output}"),
        "cells" => format!("the number of cells of class {class} in {output}"),
        "share" => format!("the share of the cells of {output} that are of class {class}"),
        "area" => format!("the area of class {class} in {output}"),
        "changed" => format!("the share of the cells of {output} that changed"),
        "metric" => {
            let metric = match class {
                "np" => "the number of patches",
                "pland" => "the percentage of the landscape",
                "ed" => "the edge density",
                "area_mn" => "the mean patch area",
                "lpi" => "the largest patch index",
                other => other,
            };
            format!("{metric} of class {} in {output}", parts.next().unwrap_or_default())
        }
        // a table's column
        _ => format!("column {stat} of {output}"),
    }
}

/// "Morris (1991)", "Campolongo et al. (2007)": a reference cited by its authors.
fn by(key: &str) -> String {
    match reference(key) {
        Some(r) => {
            let c = r.cite();
            match c.rsplit_once(", ") {
                Some((who, year)) => format!("{who} ({year})"),
                None => c,
            }
        }
        None => String::new(),
    }
}

/// "Herman & Usher, 2017; Iwanaga et al., 2022": references cited together in brackets.
fn together(keys: &[&str]) -> String {
    keys.iter().filter_map(|k| reference(k)).map(Reference::cite).collect::<Vec<_>>().join("; ")
}

/// The keys of the references of a method (SALib's too, which Morris and Sobol reproduce).
fn keys(method: Method) -> Vec<&'static str> {
    match method {
        Method::Oat => Vec::new(),
        Method::Morris => vec!["campolongo2007", "herman2017", "iwanaga2022", "morris1991"],
        Method::Sobol => vec!["herman2017", "iwanaga2022", "saltelli2002", "saltelli2010", "sobol2001"],
    }
}

/// The references of a method, in alphabetical order.
pub fn references(method: Method) -> Vec<&'static Reference> {
    let mut all: Vec<&'static Reference> = keys(method).into_iter().filter_map(reference).collect();
    all.sort_by_key(|r| (r.authors[0].to_lowercase(), r.year));
    all
}

/// The paragraph: what was done, in the words of a methods section.
pub fn paragraph(r: &Run) -> String {
    let i = r.indices;
    let k = i.factors.len();
    let points = runs(i.method, k, r.design);
    let total = points * i.replicates.max(1);
    let factors: Vec<String> =
        i.factors.iter().zip(r.bounds).map(|(f, (lo, hi))| format!("{f} [{lo}\u{2013}{hi}]")).collect();
    let s = if k == 1 { "" } else { "s" };
    let mut t = format!(
        "We assessed the sensitivity of {} to {k} parameter{s} ({}) ",
        measured(r.output, r.statistic),
        factors.join(", ")
    );
    // (with replicates the design's points are fewer than the runs)
    let size = if i.replicates > 1 { format!("{points} design points") } else { format!("{points} model runs") };
    let conf = format!("95 % bootstrap interval from {} resamples", r.resamples);
    match i.method {
        Method::Oat => t.push_str(&format!(
            "with a one-at-a-time design: a base run with every parameter in the middle of its range, and each \
             parameter set to its minimum and to its maximum with the others at the base ({size}); a parameter's \
             influence is the swing of the output between its two ends."
        )),
        Method::Morris => t.push_str(&format!(
            "with the elementary effects method of {} as improved by {}: r = {} trajectories on a {}-level grid, \
             {size}, each parameter's mean absolute effect (\u{3bc}*, with a {conf}) and the standard deviation of \
             its effects (\u{3c3}).",
            by("morris1991"),
            by("campolongo2007"),
            r.design.n,
            r.design.levels
        )),
        Method::Sobol => t.push_str(&format!(
            "with the variance-based indices of {}, computed from the sampling of {} with the estimators of {}: \
             N = {} base points, {size}, the first-order (S1) and total-order (ST) indices of each parameter, each \
             with a {conf}{}.",
            by("sobol2001"),
            by("saltelli2002"),
            by("saltelli2010"),
            r.design.n,
            if r.design.second_order { ", and the second-order indices (S2) of each pair" } else { "" }
        )),
    }
    if i.replicates > 1 {
        t.push_str(&format!(
            " Each design point was run with {} random seeds ({total} model runs in all) and the outputs averaged",
            i.replicates
        ));
        match i.chance {
            Some(c) => t.push_str(&format!("; the seed accounted for {:.0} % of the output's variance.", c * 100.0)),
            None => t.push('.'),
        }
    }
    if let Some(c) = &i.convergence {
        let tol = TOLERANCE * 100.0;
        if c.stable {
            t.push_str(&format!(
                " Indices computed with half of the design were within {tol:.0} % of the final ones (of the largest \
                 value), and ranked the influential parameters in the same order."
            ));
        } else {
            t.push_str(&format!(
                " The indices had not settled: those computed with half of the design differed from the final ones \
                 by more than {tol:.0} % of the largest value, or ranked the influential parameters differently, \
                 so the ranking is to be read with caution."
            ));
        }
    }
    t.push_str(&format!(" Computed with Terraflux {} (Lacerda, 2026)", r.version));
    match i.method {
        Method::Oat => t.push('.'),
        _ => t.push_str(&format!(
            ", whose sensitivity methods reproduce SALib 1.6 ({}).",
            together(&["herman2017", "iwanaga2022"])
        )),
    }
    t
}

/// The text of `methods.md`: the paragraph and the references.
pub fn text(r: &Run) -> String {
    let mut refs: Vec<String> = references(r.indices.method).iter().map(|x| x.apa()).collect();
    refs.push(software_apa(r.version));
    refs.sort_by_key(|x| x.to_lowercase());
    let list: Vec<String> = refs.iter().map(|x| format!("- {x}")).collect();
    format!("# Methods: sensitivity analysis\n\n{}\n\n## References\n\n{}\n", paragraph(r), list.join("\n"))
}

/// Write `methods.md` in `dir`.
pub fn write(dir: &Path, r: &Run) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let file = dir.join("methods.md");
    std::fs::write(&file, text(r))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use terraflux_dist::sensitivity::Convergence;

    use super::*;

    fn indices(method: Method, k: usize) -> Indices {
        Indices {
            method,
            columns: Vec::new(),
            factors: (0..k).map(|n| format!("p.f{n}")).collect(),
            rows: Vec::new(),
            base: None,
            s2: None,
            replicates: 1,
            chance: None,
            convergence: None,
        }
    }

    fn run<'a>(i: &'a Indices, d: &'a Design, b: &'a [(f64, f64)], statistic: Option<&'a str>) -> Run<'a> {
        Run { indices: i, design: d, bounds: b, output: "sim.map", statistic, resamples: 100, version: "9.9.9" }
    }

    const B: [(f64, f64); 2] = [(0.8, 1.2), (3.0, 7.0)];

    #[test]
    fn morris_is_told_with_its_trajectories_and_papers() {
        let i = indices(Method::Morris, 2);
        let d = Design::new(Method::Morris);
        let p = paragraph(&run(&i, &d, &B, Some("share:2")));
        assert!(p.starts_with("We assessed the sensitivity of the share of the cells of sim.map that are of class 2 to 2 parameters (p.f0 [0.8\u{2013}1.2], p.f1 [3\u{2013}7]) "), "{p}");
        assert!(
            p.contains("elementary effects method of Morris (1991) as improved by Campolongo et al. (2007)"),
            "{p}"
        );
        assert!(p.contains("r = 10 trajectories on a 4-level grid, 30 model runs"), "{p}");
        assert!(p.contains("95 % bootstrap interval from 100 resamples"), "{p}");
        assert!(p.ends_with("Computed with Terraflux 9.9.9 (Lacerda, 2026), whose sensitivity methods reproduce SALib 1.6 (Herman & Usher, 2017; Iwanaga et al., 2022)."), "{p}");
        // nothing about replicates or convergence when there are none
        assert!(!p.contains("seeds") && !p.contains("half of the design"), "{p}");
    }

    #[test]
    fn sobol_names_its_design_and_estimators() {
        let i = indices(Method::Sobol, 2);
        let d = Design { n: 64, second_order: true, ..Design::new(Method::Sobol) };
        let p = paragraph(&run(&i, &d, &B, None));
        assert!(p.contains("variance-based indices of Sobol' (2001)"), "{p}");
        assert!(p.contains("sampling of Saltelli (2002) with the estimators of Saltelli et al. (2010)"), "{p}");
        // 64 x (2 x 2 + 2)
        assert!(p.contains("N = 64 base points, 384 model runs"), "{p}");
        assert!(p.contains("second-order indices (S2)"), "{p}");
        let first = Design { second_order: false, ..d };
        let p = paragraph(&run(&i, &first, &B, None));
        assert!(p.contains("256 model runs") && !p.contains("S2"), "{p}");
        assert!(p.starts_with("We assessed the sensitivity of sim.map to"), "{p}");
    }

    #[test]
    fn oat_cites_no_method_paper_and_no_salib() {
        let i = indices(Method::Oat, 1);
        let d = Design::new(Method::Oat);
        let p = paragraph(&run(&i, &d, &B[..1], Some("mean")));
        assert!(p.contains("the mean of sim.map to 1 parameter (p.f0"), "{p}");
        assert!(p.contains("one-at-a-time") && p.contains("(3 model runs)"), "{p}");
        assert!(p.ends_with("Computed with Terraflux 9.9.9 (Lacerda, 2026)."), "{p}");
        assert!(!p.contains("SALib"), "{p}");
        assert!(references(Method::Oat).is_empty());
    }

    #[test]
    fn replicates_chance_and_convergence_are_told_only_when_they_apply() {
        let mut i = indices(Method::Morris, 2);
        i.replicates = 3;
        i.chance = Some(0.123);
        let d = Design::new(Method::Morris);
        let p = paragraph(&run(&i, &d, &B, None));
        assert!(p.contains("30 design points"), "{p}");
        assert!(p.contains("Each design point was run with 3 random seeds (90 model runs in all) and the outputs averaged; the seed accounted for 12 % of the output's variance."), "{p}");
        // replicates without a chance (no variance): the sentence ends there
        i.chance = None;
        let p = paragraph(&run(&i, &d, &B, None));
        assert!(p.contains("and the outputs averaged. ") && !p.contains("accounted"), "{p}");
        // the convergence, settled or not
        let conv = |stable| Convergence { by: "mu_star".into(), n: vec![5, 10], values: Vec::new(), stable };
        i.convergence = Some(conv(true));
        let p = paragraph(&run(&i, &d, &B, None));
        assert!(p.contains("half of the design were within 5 % of the final ones"), "{p}");
        i.convergence = Some(conv(false));
        let p = paragraph(&run(&i, &d, &B, None));
        assert!(p.contains("had not settled") && !p.contains("were within"), "{p}");
    }

    #[test]
    fn the_references_are_those_of_the_method() {
        let names = |m| references(m).iter().map(|r| r.key).collect::<Vec<_>>();
        assert_eq!(names(Method::Morris), ["campolongo2007", "herman2017", "iwanaga2022", "morris1991"]);
        assert_eq!(names(Method::Sobol), ["herman2017", "iwanaga2022", "saltelli2002", "saltelli2010", "sobol2001"]);
        let i = indices(Method::Sobol, 2);
        let d = Design::new(Method::Sobol);
        let t = text(&run(&i, &d, &B, None));
        assert!(t.starts_with("# Methods: sensitivity analysis\n\nWe assessed"), "{t}");
        assert!(t.contains("\n## References\n\n- Herman, J., & Usher, W. (2017). SALib: An open-source Python library for sensitivity analysis. Journal of Open Source Software, 2(9), 97. https://doi.org/10.21105/joss.00097\n"), "{t}");
        assert!(t.contains("- Sobol', I. M. (2001). Global sensitivity indices"), "{t}");
        assert!(t.contains("- Lacerda, E. (2026). Terraflux:") && t.contains("(Version 9.9.9)"), "{t}");
        // Terraflux is listed with the rest, in alphabetical order
        let list = &t[t.find("## References").unwrap()..];
        let at = |x: &str| list.find(x).unwrap_or_else(|| panic!("{x}: {list}"));
        assert!(at("Iwanaga") < at("Lacerda") && at("Lacerda") < at("Saltelli, A. (2002)"), "{list}");
    }

    #[test]
    fn it_is_written_in_the_folder() {
        let dir = std::env::temp_dir().join(format!("terraflux-sens-methods-{}", std::process::id()));
        let i = indices(Method::Morris, 2);
        let d = Design::new(Method::Morris);
        let f = write(&dir, &run(&i, &d, &B, None)).unwrap();
        assert_eq!(f, dir.join("methods.md"));
        assert!(std::fs::read_to_string(&f).unwrap().contains("Morris (1991)"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
