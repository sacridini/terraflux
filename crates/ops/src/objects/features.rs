//! `objects.features`: the attributes of each object, in groups: spectral (the bands' statistics over the object's
//! cells, and what they say together), shape (what the cells' outline and spread say), texture (the grey-level
//! co-occurrence of the cells), context (the neighbours, the classes, the levels above and beneath). A group is one
//! [`Group`] in [`GROUPS`]: a name, its features, the ones `basic` takes, and a function that gives a column per
//! feature.
//!
//! The cells of each object are indexed once (a list per object, in cell order), then each object is worked out on
//! its own, objects in parallel on the shared pool (the central cap of threads). Nothing is merged between threads:
//! an object's numbers come from its own cells in a fixed order, so the result is the same for any number of threads.

use std::borrow::Cow;
use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::big::{self, Layered, Part, strips};
use super::{Objects, band_names, objects_in};
use crate::{Args, Ctx, Error, Operator, Outputs, Result, Value, outputs};

/// The features of a group to give: name, and whether the user named it.
pub(super) type Want<'a> = &'a [(&'static str, bool)];

/// The columns a group gives: name and a value per object (id order).
pub(super) type Columns = Vec<(String, Vec<f64>)>;

type Pick = fn(&Stat) -> f64;

/// The bytes above which the medians of an image read by strips are told to hold much memory (1 GiB).
const MEDIAN_WARN: u64 = 1 << 30;

/// A group of features: its name, its features in column order, those `basic` takes, and what gives their columns
/// (a feature name with whether the user named it: a feature that cannot be worked out is an error when named, left
/// out when it came with a group).
pub(super) struct Group {
    pub name: &'static str,
    pub features: &'static [&'static str],
    pub basic: &'static [&'static str],
    pub run: fn(&Input, Want) -> Result<Columns>,
}

pub(super) const GROUPS: [Group; 4] = [
    Group {
        name: "spectral",
        features: &["mean", "sd", "min", "max", "median", "brightness", "max_diff", "ratio", "ndvi", "ndwi", "ndmi"],
        basic: &["mean", "sd", "ndvi"],
        run: spectral,
    },
    Group { name: "shape", features: SHAPE, basic: &["area", "compactness"], run: shape },
    Group { name: "texture", features: GLCM, basic: &["homogeneity"], run: texture },
    Group { name: "context", features: CONTEXT, basic: &[], run: context },
];

/// The bands the indices read: positions among the layers.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(super) struct Roles {
    pub red: Option<usize>,
    pub nir: Option<usize>,
    pub green: Option<usize>,
    pub swir: Option<usize>,
}

/// What a group reads: the objects, the cells of each, and the layers (the image's first, then `more`'s).
pub(super) struct Input<'a> {
    pub node: &'a str,
    pub o: &'a Objects,
    /// Where each object's cells start (count + 1 numbers), and the cells' positions, in cell order: made when a
    /// group first asks for the cells of an object (a u32 a cell, so not for a scene read by strips that needs none).
    index: std::sync::OnceLock<(Vec<usize>, Vec<u32>)>,
    pub names: Vec<String>,
    /// The layers in memory (none when the image is read by strips).
    pub data: Vec<Arc<Vec<f64>>>,
    /// The layers read by strips of rows, when some are rasters on disk: then `data` is empty and every group reads
    /// the strips (the same numbers, a strip at a time in memory).
    pub strips: Option<Layered>,
    /// How many layers are the image's (the rest come from `more`).
    pub image: usize,
    pub roles: Roles,
    /// Grey levels of the texture, and the layers it reads (names, or `brightness`), when the user gave them.
    pub levels: usize,
    pub texture_bands: Option<Vec<String>>,
    /// The level beneath these objects (its parent is these objects), when given.
    pub below: Option<&'a Objects>,
}

impl Input<'_> {
    /// The cells of an object (index k = id - 1) by position row * width + column, ascending.
    pub fn cells_of(&self, k: usize) -> &[u32] {
        let (offsets, cells) = self.index.get_or_init(|| index(self.o));
        &cells[offsets[k]..offsets[k + 1]]
    }
}

/// The index of cells per object (a counting sort of the labels).
fn index(o: &Objects) -> (Vec<usize>, Vec<u32>) {
    let mut offsets = vec![0usize; o.count + 1];
    for k in 0..o.count {
        offsets[k + 1] = offsets[k] + o.cells[k] as usize;
    }
    let mut next = offsets.clone();
    let mut cells = vec![0u32; offsets[o.count]];
    for (i, &l) in o.labels.iter().enumerate() {
        if l != 0 {
            let k = l as usize - 1;
            cells[next[k]] = i as u32;
            next[k] += 1;
        }
    }
    (offsets, cells)
}

// ---- the bands the indices read

/// A band's number as its name says it: "B4", "SR_B4", "B04", "band 4", "S2_B8A_10m" are 4, 4, 4, 4 and 8A (as the
/// Viewer's colour composites read them).
fn band_id(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    let words: Vec<&str> = lower.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    words
        .iter()
        .find_map(|w| {
            let rest = w.strip_prefix("band").or_else(|| w.strip_prefix('b'))?;
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let tail = &rest[digits.len()..];
            if digits.is_empty() || !(tail.is_empty() || tail == "a") {
                return None;
            }
            Some(format!("{}{tail}", digits.trim_start_matches('0')))
        })
        .or_else(|| {
            // ("band 4": the word "band" and the number apart)
            words
                .windows(2)
                .find(|w| w[0] == "band" && w[1].chars().all(|c| c.is_ascii_digit()))
                .map(|w| w[1].trim_start_matches('0').to_string())
        })
}

/// The bands red, near infrared, green and short-wave infrared among the names: a word of a name that says it
/// (`red`, `nir`, `green`, `swir1` or `swir`, any case), else the band's number: Sentinel-2's B4, B8, B3 and B11,
/// Landsat 8/9's 4, 5, 3 and 6 (Landsat when a name has `SR_B` or there is a B10). Only the first of each is taken.
pub(super) fn detect(names: &[String]) -> Roles {
    let words: Vec<Vec<String>> = names
        .iter()
        .map(|n| {
            n.to_lowercase()
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|w| !w.is_empty())
                .map(String::from)
                .collect()
        })
        .collect();
    let ids: Vec<Option<String>> = names.iter().map(|n| band_id(n)).collect();
    let landsat =
        ids.iter().any(|i| i.as_deref() == Some("10")) || names.iter().any(|n| n.to_lowercase().contains("sr_b"));
    let find = |says: &[&str], sentinel: &str, landsat_id: &str| {
        words.iter().position(|w| w.iter().any(|x| says.contains(&x.as_str()))).or_else(|| {
            let want = if landsat { landsat_id } else { sentinel };
            ids.iter().position(|i| i.as_deref() == Some(want))
        })
    };
    Roles {
        red: find(&["red"], "4", "4"),
        nir: find(&["nir"], "8", "5"),
        green: find(&["green"], "3", "3"),
        swir: find(&["swir1", "swir"], "11", "6"),
    }
}

// ---- the operator

/// The groups and features of the names in `features`: per group, the features chosen (in the group's order) with
/// whether each was named.
fn choose(node: &str, spec: &[String]) -> Result<Vec<Vec<(&'static str, bool)>>> {
    let mut on: Vec<Vec<Option<bool>>> = GROUPS.iter().map(|g| vec![None; g.features.len()]).collect();
    let mut set = |g: usize, f: usize, named: bool| on[g][f] = Some(on[g][f].unwrap_or(false) || named);
    for s in spec {
        match s.to_lowercase().as_str() {
            "basic" => {
                for (g, grp) in GROUPS.iter().enumerate() {
                    for (f, name) in grp.features.iter().enumerate() {
                        if grp.basic.contains(name) {
                            set(g, f, false);
                        }
                    }
                }
            }
            "all" => {
                for (g, grp) in GROUPS.iter().enumerate() {
                    (0..grp.features.len()).for_each(|f| set(g, f, false));
                }
            }
            name => {
                if let Some(g) = GROUPS.iter().position(|g| g.name == name) {
                    (0..GROUPS[g].features.len()).for_each(|f| set(g, f, false));
                } else if let Some((g, f)) = GROUPS
                    .iter()
                    .enumerate()
                    .find_map(|(g, grp)| grp.features.iter().position(|x| *x == name).map(|f| (g, f)))
                {
                    set(g, f, true);
                } else {
                    let list: Vec<String> =
                        GROUPS.iter().map(|g| format!("{}: {}", g.name, g.features.join(", "))).collect();
                    let groups: Vec<&str> = GROUPS.iter().map(|g| g.name).collect();
                    return Err(Error::msg(format!(
                        "node {node:?}: unknown feature {s:?}; use basic, all, a group ({}) or a feature ({})",
                        groups.join(", "),
                        list.join("; ")
                    )));
                }
            }
        }
    }
    Ok(GROUPS
        .iter()
        .enumerate()
        .map(|(g, grp)| {
            grp.features.iter().enumerate().filter_map(|(f, name)| on[g][f].map(|named| (*name, named))).collect()
        })
        .collect())
}

/// Whether `objects.features` with this setting `features` (none: the default) gives the column `column`: `None` when
/// the column is not of the kinds it makes (an attribute of the user's, or the table's own id, cells and area) or the
/// setting cannot be read; else its group and whether the setting makes it. For the static advice on rules that read what no node
/// makes.
pub fn features_give(spec: Option<&serde_json::Value>, column: &str) -> Option<(&'static str, bool)> {
    let spec: Vec<String> = match spec {
        None => vec!["basic".into()],
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(serde_json::Value::Array(a)) => a.iter().map(|v| v.as_str().map(String::from)).collect::<Option<_>>()?,
        Some(_) => return None,
    };
    if spec.iter().any(|s| s.starts_with(['$', '@'])) {
        return None;
    }
    let (group, feature) = feature_of(column)?;
    let chosen = choose("", &spec).ok()?;
    let g = GROUPS.iter().position(|g| g.name == group)?;
    Some((group, chosen[g].iter().any(|(name, _)| *name == feature)))
}

/// The group and the feature of `objects.features` that make a column, by its name.
fn feature_of(column: &str) -> Option<(&'static str, &'static str)> {
    const FAMILIES: [&str; 6] = ["mean", "sd", "min", "max", "median", "ratio"];
    const ALONE: [&str; 5] = ["brightness", "max_diff", "ndvi", "ndwi", "ndmi"];
    const CONTEXT_PREFIX: [(&str, &str); 6] = [
        ("nb_diff_", "nb_diff"),
        ("rel_border_", "rel_border"),
        ("dist_", "dist"),
        ("parent_", "parent"),
        ("children_", "children"),
        ("nb_", "nb_class"),
    ];
    if let Some(f) = ALONE.iter().find(|f| **f == column) {
        return Some(("spectral", f));
    }
    if matches!(column, "id" | "cells" | "area") {
        return None;
    }
    if let Some(f) = SHAPE.iter().find(|f| **f == column) {
        return Some(("shape", f));
    }
    if column == "nb_count" {
        return Some(("context", "nb_count"));
    }
    if column == "border_image" {
        return Some(("context", "border_image"));
    }
    if let Some(rest) = column.strip_prefix("glcm_") {
        return GLCM
            .iter()
            .find(|g| rest == **g || rest.strip_prefix(**g).is_some_and(|t| t.starts_with('_')))
            .map(|g| ("texture", *g));
    }
    if let Some(f) = FAMILIES.iter().find(|f| column.strip_prefix(**f).is_some_and(|t| t.starts_with('_'))) {
        return Some(("spectral", f));
    }
    CONTEXT_PREFIX.iter().find(|(p, _)| column.starts_with(p)).map(|(_, f)| ("context", *f))
}

/// A layer's position by the name a setting gives (the exact name, else any case).
fn layer_named(node: &str, names: &[String], setting: &str, name: &str) -> Result<usize> {
    names.iter().position(|n| n == name).or_else(|| names.iter().position(|n| n.eq_ignore_ascii_case(name))).ok_or_else(
        || Error::msg(format!("node {node:?}: {setting} {name:?} is none of the layers: {}", names.join(", "))),
    )
}

pub(super) struct Features;

impl Operator for Features {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.features",
            "The attributes of each object, from the image under it: spectral (per band the mean, standard \
             deviation (population), min, max and median over the object's cells with a value; brightness, the mean \
             of the image bands' means; max_diff, (greatest mean - least) / brightness; ratio, a band's mean over \
             the sum of the means; and, from the objects' mean bands, NDVI = (nir - red) / (nir + red), NDWI = \
             (green - nir) / (green + nir) and NDMI = (nir - swir) / (nir + swir), as eCognition's customised \
             features on the mean layers) and shape (cells turned into map units: area, perimeter = the cell edges \
             that touch another object, no object or the image's edge, compactness = length x width / area of the \
             minimum bounding rectangle, shape_index = perimeter / (4 sqrt(area)), length and width = the sides of \
             the minimum-area rectangle around the cells' corners (convex hull and rotating calipers), \
             length_width, asymmetry = 1 - minor / major axis of the cells' spread, direction = degrees from the \
             x axis counter-clockwise, 0 to 180, of the major axis, elliptic_fit and rectangular_fit = the share \
             of the cells inside the ellipse and the rectangle of the object's area and axes centred on it, \
             border_index = perimeter / the rectangle's, density, centroid_x and centroid_y in map coordinates), \
             texture (Haralick's grey-level co-occurrence matrix of the pairs of cells both in the object at \
             distance 1 in the four directions, symmetric, summed and normalised, as eCognition's GLCM all \
             directions: glcm_homogeneity, glcm_contrast, glcm_entropy, glcm_correlation (1 for a constant \
             object) and glcm_asm, of the brightness and of each image band, grey levels cut linearly between \
             the band's 1st and 99th percentile; NaN for an object with no pair) and context (nb_diff_<band> = \
             the mean difference to neighbours weighted by border length, nb_count, border_image = the share of \
             the perimeter on the image's edge or no object; with classes per class c: rel_border_<c>, dist_<c> \
             = map distance from the centroid to the nearest centroid of an object of the class, nb_<c>; with a \
             parent level: parent_id and parent_<column>; with `below`: children and per class of below \
             children_<c> and children_area_<c>). \
             The attributes join the objects' table: a column of an old name is replaced, the others kept.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects"))
        .input(PortSpec::new(
            "image",
            Type::Raster,
            "a map, or a stack of bands, on the objects' grid; rasters on disk too big to hold are read by strips of \
             rows (the medians need one band's values of the objects in memory at a time)",
        ))
        .input(PortSpec::optional(
            "more",
            Type::Raster,
            "any raster as more layers, on the objects' grid: a DEM, a height model, a class map, another date; \
             their names kept (a name already taken gets _2)",
        ))
        .input(PortSpec::optional(
            "below",
            Type::Objects,
            "the level beneath, whose parent level is these objects (for the context's children features)",
        ))
        .output(PortSpec::new("objects", Type::Objects, "the same objects, with the attributes in their table"))
        .output(PortSpec::new("table", Type::Table, "the objects' table"))
        .param(PortSpec::optional(
            "features",
            Type::Any,
            "basic (the default: mean, sd and ndvi per band, area, compactness, glcm_homogeneity of the \
             brightness), all, or a list of groups (spectral, shape, texture, context) and feature names",
        ))
        .param(PortSpec::optional("levels", Type::Integer, "the grey levels of the texture, 2 to 256 (default 32)"))
        .param(PortSpec::optional(
            "texture_bands",
            Type::Any,
            "the layers the texture reads, a list of names or brightness (default: brightness for basic, \
             brightness and every image band otherwise)",
        ))
        .param(PortSpec::optional("red", Type::Text, "the red band's name (default: found by the names)"))
        .param(PortSpec::optional("nir", Type::Text, "the near infrared band's name (default: found)"))
        .param(PortSpec::optional("green", Type::Text, "the green band's name (default: found)"))
        .param(PortSpec::optional("swir", Type::Text, "the short-wave infrared band's name (default: found)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        let spec: Vec<String> = match args.opt_param::<serde_json::Value>("features")? {
            None => vec!["basic".into()],
            Some(serde_json::Value::String(s)) => vec![s],
            Some(serde_json::Value::Array(a)) => a
                .iter()
                .map(|v| v.as_str().map(String::from).ok_or_else(|| bad("features: names (text)".into())))
                .collect::<Result<_>>()?,
            Some(_) => return Err(bad("features: basic, all, or a list of names".into())),
        };
        if spec.is_empty() {
            return Err(bad("features: no name given".into()));
        }
        let chosen = choose(node, &spec)?;
        // rasters on disk too big to hold are read by strips of rows; the rest are in memory
        let by_strips = big::strips_wanted(args, &["image", "more"]);
        let (image_names, image_grid, mut parts) = if by_strips {
            big::parts_of(args, "image")?
        } else {
            let l = big::layers(args, "image")?;
            super::warnings::image(ctx, node, &l.grid, &l.names, &l.data, false);
            (l.names, l.grid, l.data.into_iter().map(Part::Mem).collect())
        };
        let mut names = band_names(&image_names);
        let img_n = parts.len();
        let on_grid = |what: &str, g: &terraflux_raster::Grid| {
            if g.same_as(&o.grid) {
                Ok(())
            } else {
                Err(bad(format!(
                    "{what} is on a {}x{} grid, the objects' is {}x{}: put it on the objects' grid with raster.match",
                    g.height, g.width, o.grid.height, o.grid.width
                )))
            }
        };
        on_grid("the image", &image_grid)?;
        let roles_found = detect(&names);
        if args.get("more").is_some() {
            let (more_names, more_grid, more_parts) = if by_strips {
                big::parts_of(args, "more")?
            } else {
                let l = big::layers(args, "more")?;
                (l.names, l.grid, l.data.into_iter().map(Part::Mem).collect())
            };
            on_grid("more", &more_grid)?;
            for (n, d) in band_names(&more_names).into_iter().zip(more_parts) {
                let mut name = n.clone();
                let mut k = 2;
                while names.contains(&name) {
                    name = format!("{n}_{k}");
                    k += 1;
                }
                names.push(name);
                parts.push(d);
            }
        }
        let (strips, data) = if by_strips {
            (Some(Layered::new(parts, o.grid.width, o.grid.height)), Vec::new())
        } else {
            let data = parts.into_iter().map(|p| if let Part::Mem(d) = p { d } else { unreachable!("in memory") });
            (None, data.collect())
        };
        let pick = |setting: &str, found: Option<usize>| -> Result<Option<usize>> {
            match args.opt_param::<String>(setting)? {
                Some(n) => layer_named(node, &names, setting, &n).map(Some),
                None => Ok(found),
            }
        };
        let roles = Roles {
            red: pick("red", roles_found.red)?,
            nir: pick("nir", roles_found.nir)?,
            green: pick("green", roles_found.green)?,
            swir: pick("swir", roles_found.swir)?,
        };
        let levels = args.param::<usize>("levels", 32)?;
        if !(2..=256).contains(&levels) {
            return Err(bad(format!("levels: from 2 to 256, not {levels}")));
        }
        let texture_bands = args.opt_param::<Vec<String>>("texture_bands")?;
        let below = match args.get("below") {
            None => None,
            Some(Value::Objects(b)) => {
                let same = b.parent.as_ref().is_some_and(|p| {
                    p.count == o.count && (Arc::ptr_eq(&p.labels, &o.labels) || *p.labels == *o.labels)
                });
                if !same {
                    return Err(
                        bad("below: objects whose parent level is these objects (segment from a level)".into()),
                    );
                }
                Some(&**b)
            }
            Some(v) => return Err(bad(format!("below is a {:?}, not objects", v.ty()))),
        };
        let held = o.cells.iter().map(|&c| c as u64).sum::<u64>() * 8;
        if strips.is_some() && chosen[0].iter().any(|f| f.0 == "median") && held > MEDIAN_WARN {
            ctx.warn(format!(
                "node {node:?}: the median of each band is worked out holding that band's values of the cells in \
                 objects in memory ({:.1} GB as numbers), a band at a time: leave `median` out of the features to \
                 spare it",
                held as f64 / 1e9
            ));
        }
        let index = Default::default();
        let input = Input { node, o, index, names, data, strips, image: img_n, roles, levels, texture_bands, below };
        let mut columns: Vec<(String, Vec<f64>)> = Vec::new();
        for (grp, want) in GROUPS.iter().zip(&chosen) {
            if !want.is_empty() {
                columns.extend((grp.run)(&input, want)?);
            }
        }
        // the table: the old columns (a recomputed one replaced in place) and the new
        let mut t = o.table.clone();
        for (name, values) in columns {
            match t.columns.iter().position(|c| *c == name) {
                Some(j) => {
                    t.labels.remove(&name);
                    t.rows.iter_mut().zip(&values).for_each(|(r, v)| r[j] = *v);
                }
                None => {
                    t.columns.push(name);
                    t.rows.iter_mut().zip(&values).for_each(|(r, v)| r.push(*v));
                }
            }
        }
        let out = Objects::clone(o).with_table(t.clone())?;
        Ok(outputs([("objects", Value::Objects(Arc::new(out))), ("table", Value::Table(t))]))
    }
}

// ---- spectral

#[derive(Clone, Copy)]
struct Stat {
    mean: f64,
    sd: f64,
    min: f64,
    max: f64,
    median: f64,
}

const NO_STAT: Stat = Stat { mean: f64::NAN, sd: f64::NAN, min: f64::NAN, max: f64::NAN, median: f64::NAN };

fn spectral(inp: &Input, want: Want) -> Result<Columns> {
    let has = |n: &str| want.iter().any(|w| w.0 == n);
    let named = |n: &str| want.iter().any(|w| w.0 == n && w.1);
    let r = inp.roles;
    let nb = inp.names.len();
    // each index: the two bands it reads (a - b) / (a + b)
    let index_bands = |name: &str| -> Result<Option<(usize, usize)>> {
        let (a, b, need) = match name {
            "ndvi" => (r.nir, r.red, "nir and red"),
            "ndwi" => (r.green, r.nir, "green and nir"),
            _ => (r.nir, r.swir, "nir and swir"),
        };
        match (a, b) {
            (Some(a), Some(b)) => Ok(Some((a, b))),
            _ if named(name) => Err(Error::msg(format!(
                "node {:?}: {name} needs the {need} bands and they were not found by the names: give them by the \
                 settings red, nir, green and swir (the layers are {})",
                inp.node,
                inp.names.join(", ")
            ))),
            _ => Ok(None),
        }
    };
    let (ndvi, ndwi, ndmi) = (
        if has("ndvi") { index_bands("ndvi")? } else { None },
        if has("ndwi") { index_bands("ndwi")? } else { None },
        if has("ndmi") { index_bands("ndmi")? } else { None },
    );
    let per_band = ["mean", "sd", "min", "max", "median"].iter().any(|n| has(n));
    let image_means = has("brightness") || has("max_diff") || has("ratio");
    let mut used = vec![per_band; nb];
    for u in used.iter_mut().take(inp.image) {
        *u |= image_means;
    }
    for (a, b) in [ndvi, ndwi, ndmi].into_iter().flatten() {
        used[a] = true;
        used[b] = true;
    }
    let (sd, mm, med) = (has("sd"), has("min") || has("max"), has("median"));
    let o = inp.o;
    let stats: Vec<Vec<Stat>> = if let Some(src) = &inp.strips {
        spectral_strips(inp, src, &used, [sd, mm, med])?
    } else {
        (0..o.count)
            .into_par_iter()
            .map_init(Vec::<f64>::new, |buf, k| {
                let cells = inp.cells_of(k);
                (0..nb)
                    .map(|j| {
                        if !used[j] {
                            return NO_STAT;
                        }
                        let d = &inp.data[j];
                        buf.clear();
                        buf.extend(cells.iter().map(|&i| d[i as usize]).filter(|v| !v.is_nan()));
                        if buf.is_empty() {
                            return NO_STAT;
                        }
                        let n = buf.len() as f64;
                        let mean = buf.iter().sum::<f64>() / n;
                        let mut s = Stat { mean, ..NO_STAT };
                        if sd {
                            s.sd = (buf.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n).sqrt();
                        }
                        if mm {
                            s.min = buf.iter().copied().fold(f64::INFINITY, f64::min);
                            s.max = buf.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                        }
                        if med {
                            buf.sort_unstable_by(f64::total_cmp);
                            let m = buf.len() / 2;
                            s.median = if buf.len() % 2 == 1 { buf[m] } else { (buf[m - 1] + buf[m]) / 2.0 };
                        }
                        s
                    })
                    .collect()
            })
            .collect()
    };
    let mut out: Columns = Vec::new();
    let fams: [(&str, Pick); 5] =
        [("mean", |s| s.mean), ("sd", |s| s.sd), ("min", |s| s.min), ("max", |s| s.max), ("median", |s| s.median)];
    for (fam, get) in fams {
        if has(fam) {
            for j in 0..nb {
                out.push((format!("{fam}_{}", inp.names[j]), stats.iter().map(|s| get(&s[j])).collect()));
            }
        }
    }
    // (the means of the image bands, NaN when a band has no cell with a value)
    let st = &stats;
    let img_means = |k: usize| (0..inp.image).map(move |j| st[k][j].mean);
    let bright: Vec<f64> = (0..o.count).map(|k| img_means(k).sum::<f64>() / inp.image as f64).collect();
    if has("brightness") {
        out.push(("brightness".into(), bright.clone()));
    }
    if has("max_diff") {
        let v = (0..o.count)
            .map(|k| {
                let hi = img_means(k).fold(f64::NEG_INFINITY, f64::max);
                let lo = img_means(k).fold(f64::INFINITY, f64::min);
                if bright[k].is_nan() || bright[k] == 0.0 { f64::NAN } else { (hi - lo) / bright[k] }
            })
            .collect();
        out.push(("max_diff".into(), v));
    }
    if has("ratio") {
        for (j, name) in inp.names.iter().enumerate().take(inp.image) {
            let v = (0..o.count)
                .map(|k| {
                    let sum: f64 = img_means(k).sum();
                    if sum == 0.0 { f64::NAN } else { stats[k][j].mean / sum }
                })
                .collect();
            out.push((format!("ratio_{name}"), v));
        }
    }
    for (name, bands) in [("ndvi", ndvi), ("ndwi", ndwi), ("ndmi", ndmi)] {
        if let Some((a, b)) = bands {
            let v = (0..o.count)
                .map(|k| {
                    let (x, y) = (stats[k][a].mean, stats[k][b].mean);
                    if x + y == 0.0 { f64::NAN } else { (x - y) / (x + y) }
                })
                .collect();
            out.push((name.into(), v));
        }
    }
    Ok(out)
}

/// The statistics of each object in each band `used` (`[sd, min or max, median]` asked) from an image read by strips
/// of rows, as the cell lists give them: each object's values are summed in cell order (strip after strip, row after
/// row, so the sums are the same), the deviation by a second pass over the strips with the mean, the median from the
/// values of the objects' cells of one band at a time (the one thing here that holds a band's cells in memory).
fn spectral_strips(inp: &Input, src: &Layered, used: &[bool], [sd, mm, med]: [bool; 3]) -> Result<Vec<Vec<Stat>>> {
    /// A band's numbers per object: count, sum (then the mean), minimum, maximum, sum of squared deviations.
    struct Acc {
        n: Vec<u32>,
        sum: Vec<f64>,
        lo: Vec<f64>,
        hi: Vec<f64>,
        sq: Vec<f64>,
    }
    let o = inp.o;
    let (w, h, n) = (o.grid.width, o.grid.height, o.count);
    let bands: Vec<usize> = (0..used.len()).filter(|&j| used[j]).collect();
    let mut acc: Vec<Acc> = bands
        .iter()
        .map(|_| Acc {
            n: vec![0; n],
            sum: vec![0.0; n],
            lo: if mm { vec![f64::INFINITY; n] } else { Vec::new() },
            hi: if mm { vec![f64::NEG_INFINITY; n] } else { Vec::new() },
            sq: Vec::new(),
        })
        .collect();
    strips(src, h, &bands, |r0, r1, rows| {
        let labels = &o.labels[r0 * w..r1 * w];
        acc.par_iter_mut().zip(rows.par_iter()).for_each(|(a, v)| {
            for (&l, &x) in labels.iter().zip(v.iter()) {
                if l == 0 || x.is_nan() {
                    continue;
                }
                let k = l as usize - 1;
                a.n[k] += 1;
                a.sum[k] += x;
                if mm {
                    a.lo[k] = a.lo[k].min(x);
                    a.hi[k] = a.hi[k].max(x);
                }
            }
        });
    })?;
    for a in acc.iter_mut() {
        for k in 0..n {
            a.sum[k] = if a.n[k] > 0 { a.sum[k] / a.n[k] as f64 } else { f64::NAN };
        }
    }
    if sd {
        acc.iter_mut().for_each(|a| a.sq = vec![0.0; n]);
        strips(src, h, &bands, |r0, r1, rows| {
            let labels = &o.labels[r0 * w..r1 * w];
            acc.par_iter_mut().zip(rows.par_iter()).for_each(|(a, v)| {
                for (&l, &x) in labels.iter().zip(v.iter()) {
                    if l != 0 && !x.is_nan() {
                        let k = l as usize - 1;
                        let d = x - a.sum[k];
                        a.sq[k] += d * d;
                    }
                }
            });
        })?;
    }
    let mut stats: Vec<Vec<Stat>> = vec![vec![NO_STAT; used.len()]; n];
    for (a, &j) in acc.iter().zip(&bands) {
        for (k, row) in stats.iter_mut().enumerate() {
            if a.n[k] > 0 {
                let s = &mut row[j];
                s.mean = a.sum[k];
                if sd {
                    s.sd = (a.sq[k] / a.n[k] as f64).sqrt();
                }
                if mm {
                    s.min = a.lo[k];
                    s.max = a.hi[k];
                }
            }
        }
    }
    drop(acc);
    if med {
        // the object's values in one list per object (a band at a time), sorted object by object in parallel
        let mut off = vec![0usize; n + 1];
        for k in 0..n {
            off[k + 1] = off[k] + o.cells[k] as usize;
        }
        for &j in &bands {
            let mut vals = vec![0.0f64; off[n]];
            let mut fill = vec![0usize; n];
            strips(src, h, &[j], |r0, r1, rows| {
                for (&l, &x) in o.labels[r0 * w..r1 * w].iter().zip(rows[0].iter()) {
                    if l != 0 && !x.is_nan() {
                        let k = l as usize - 1;
                        vals[off[k] + fill[k]] = x;
                        fill[k] += 1;
                    }
                }
            })?;
            let mut lists: Vec<&mut [f64]> = Vec::with_capacity(n);
            let mut rest = vals.as_mut_slice();
            for k in 0..n {
                let (a, b) = rest.split_at_mut(o.cells[k] as usize);
                lists.push(&mut a[..fill[k]]);
                rest = b;
            }
            let medians: Vec<f64> = lists
                .into_par_iter()
                .map(|v| {
                    if v.is_empty() {
                        return f64::NAN;
                    }
                    v.sort_unstable_by(f64::total_cmp);
                    let m = v.len() / 2;
                    if v.len() % 2 == 1 { v[m] } else { (v[m - 1] + v[m]) / 2.0 }
                })
                .collect();
            for (s, m) in stats.iter_mut().zip(medians) {
                s[j].median = m;
            }
        }
    }
    Ok(stats)
}

// ---- shape

const SHAPE: &[&str] = &[
    "area",
    "perimeter",
    "compactness",
    "shape_index",
    "length",
    "width",
    "length_width",
    "asymmetry",
    "direction",
    "elliptic_fit",
    "rectangular_fit",
    "border_index",
    "density",
    "centroid_x",
    "centroid_y",
];

/// The convex hull of integer points, counter-clockwise, without collinear points (Andrew's monotone chain).
fn hull(mut p: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    p.sort_unstable();
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let cross = |o: (i64, i64), a: (i64, i64), b: (i64, i64)| (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
    let mut h: Vec<(i64, i64)> = Vec::with_capacity(p.len() * 2);
    for &q in &p {
        while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0 {
            h.pop();
        }
        h.push(q);
    }
    let lower = h.len() + 1;
    for &q in p.iter().rev().skip(1) {
        while h.len() >= lower && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0 {
            h.pop();
        }
        h.push(q);
    }
    h.pop();
    h
}

/// The sides (long, short) of the minimum-area rectangle around a convex polygon (counter-clockwise, 3 points or
/// more, not all on a line), by rotating calipers: the best rectangle has a side along one edge of the polygon; the
/// three far points (along the edge, across it, back along it) only move forward as the edge turns.
fn min_rect(h: &[(f64, f64)]) -> (f64, f64) {
    let n = h.len();
    let at = |i: usize| h[i % n];
    let dot = |a: (f64, f64), b: (f64, f64)| a.0 * b.0 + a.1 * b.1;
    let step = |i: usize| (at(i + 1).0 - at(i).0, at(i + 1).1 - at(i).1);
    let (mut right, mut top, mut left) = (1usize, 1usize, 1usize);
    let mut best = (f64::INFINITY, 0.0, 0.0);
    for i in 0..n {
        let e = step(i);
        let len = dot(e, e).sqrt();
        if len == 0.0 {
            continue;
        }
        let d = (e.0 / len, e.1 / len);
        let nrm = (-d.1, d.0);
        right = right.max(i + 1);
        while dot(step(right), d) > 0.0 {
            right += 1;
        }
        top = top.max(right);
        while dot(step(top), nrm) > 0.0 {
            top += 1;
        }
        left = left.max(top);
        while dot(step(left), d) < 0.0 {
            left += 1;
        }
        let along = dot((at(right).0 - at(left).0, at(right).1 - at(left).1), d);
        let across = dot((at(top).0 - at(i).0, at(top).1 - at(i).1), nrm);
        if along * across < best.0 {
            best = (along * across, along, across);
        }
    }
    (best.1.max(best.2), best.1.min(best.2))
}

fn shape(inp: &Input, want: Want) -> Result<Columns> {
    let mut on = [false; 15];
    for (name, _) in want {
        if let Some(f) = SHAPE.iter().position(|s| s == name) {
            on[f] = true;
        }
    }
    let o = inp.o;
    let rows: Vec<[f64; 15]> = (0..o.count).into_par_iter().map(|k| shape_of(inp, k, &on)).collect();
    Ok(SHAPE
        .iter()
        .enumerate()
        .filter(|(f, _)| on[*f])
        .map(|(f, name)| (name.to_string(), rows.iter().map(|r| r[f]).collect()))
        .collect())
}

/// An object's shape numbers (those `on`, in the order of [`SHAPE`]; NaN for the others).
///
/// The definitions follow eCognition's Reference Book (wording from memory, not quoted): compactness is the
/// product of the length and the width of the object divided by its number of cells (area); border index is the
/// object's border length over the border length of its smallest enclosing rectangle; asymmetry is one minus the
/// ratio of the minor to the major axis of the ellipse of the object's spread; density is the square root of the
/// number of cells over one plus the square root of the sum of the variances of the cells' x and y; direction is the
/// angle of the major axis. Taken from the covariance of the cells as an area: each cell adds 1/12 of a cell squared
/// to the variance of the centres, so a single cell is round and a one-cell-wide line has a width.
fn shape_of(inp: &Input, k: usize, on: &[bool; 15]) -> [f64; 15] {
    let o = inp.o;
    let g = &o.grid;
    let w = g.width;
    let t = g.transform;
    let (rw, rh) = g.resolution();
    let id = k as u32 + 1;
    let cells = inp.cells_of(k);
    let n = cells.len() as f64;
    let [r0, c0, ..] = o.boxes[k];
    let (r0, c0) = (r0 as f64, c0 as f64);
    let need_hull = on[2] || on[4] || on[5] || on[6] || on[11];
    let need_fit = on[9] || on[10];
    let mut out = [f64::NAN; 15];
    // moments of the cells' rows and columns (from the box corner), the border, the extreme columns of each row
    let (mut sr, mut sc, mut srr, mut scc, mut src) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut vert, mut horiz) = (0usize, 0usize);
    let mut pts: Vec<(i64, i64)> = Vec::new();
    let mut run: Option<(usize, usize, usize)> = None;
    let flush = |run: Option<(usize, usize, usize)>, pts: &mut Vec<(i64, i64)>| {
        if let Some((r, a, b)) = run {
            let (r, a, b) = (r as i64, a as i64, b as i64 + 1);
            pts.extend([(a, r), (a, r + 1), (b, r), (b, r + 1)]);
        }
    };
    for &i in cells {
        let i = i as usize;
        let (r, c) = (i / w, i % w);
        let (dr, dc) = (r as f64 - r0, c as f64 - c0);
        sr += dr;
        sc += dc;
        srr += dr * dr;
        scc += dc * dc;
        src += dr * dc;
        if c == 0 || o.labels[i - 1] != id {
            vert += 1;
        }
        if c + 1 == w || o.labels[i + 1] != id {
            vert += 1;
        }
        if r == 0 || o.labels[i - w] != id {
            horiz += 1;
        }
        if r + 1 == g.height || o.labels[i + w] != id {
            horiz += 1;
        }
        if need_hull {
            run = match run {
                Some((rr, a, _)) if rr == r => Some((rr, a, c)),
                prev => {
                    flush(prev, &mut pts);
                    Some((r, c, c))
                }
            };
        }
    }
    flush(run, &mut pts);
    let (mr, mc) = (sr / n, sc / n);
    let (vr, vc, cv) = (srr / n - mr * mr, scc / n - mc * mc, src / n - mr * mc);
    let area = n * rw * rh;
    let perimeter = vert as f64 * rh + horiz as f64 * rw;
    out[0] = area;
    out[1] = perimeter;
    out[3] = perimeter / (4.0 * area.sqrt());
    out[12] = n.sqrt() / (1.0 + (vr + vc).sqrt());
    let (cr, cc) = (r0 + mr + 0.5, c0 + mc + 0.5);
    out[13] = t[0] + t[1] * cc + t[2] * cr;
    out[14] = t[3] + t[4] * cc + t[5] * cr;
    // the spread as an area, in map units, x to the east and y to the north: the principal axes
    let (vx, vy) = ((vc + 1.0 / 12.0) * rw * rw, (vr + 1.0 / 12.0) * rh * rh);
    let cxy = cv * t[1] * t[5];
    let half = ((vx - vy) / 2.0).hypot(cxy);
    let (l1, l2) = ((vx + vy) / 2.0 + half, (vx + vy) / 2.0 - half);
    let theta = 0.5 * (2.0 * cxy).atan2(vx - vy);
    let ratio = (l1 / l2).sqrt();
    out[7] = 1.0 - 1.0 / ratio;
    out[8] = theta.to_degrees().rem_euclid(180.0) + 0.0;
    if need_hull {
        let h: Vec<(f64, f64)> = hull(pts).into_iter().map(|(c, r)| (c as f64 * rw, r as f64 * rh)).collect();
        let (length, width) = min_rect(&h);
        out[2] = length * width / area;
        out[4] = length;
        out[5] = width;
        out[6] = length / width;
        out[11] = perimeter / (2.0 * (length + width));
    }
    if need_fit {
        // the ellipse and the rectangle of the object's area, with its axes ratio, on the centroid
        let b = (area / (std::f64::consts::PI * ratio)).sqrt();
        let a = ratio * b;
        let rect_w = (area / ratio).sqrt();
        let rect_l = ratio * rect_w;
        let (s, c) = theta.sin_cos();
        let (mut ell, mut rec) = (0usize, 0usize);
        for &i in cells {
            let i = i as usize;
            let (dx, dy) = (t[1] * ((i % w) as f64 - c0 - mc), t[5] * ((i / w) as f64 - r0 - mr));
            let (u, v) = (dx * c + dy * s, -dx * s + dy * c);
            if (u / a).powi(2) + (v / b).powi(2) <= 1.0 + 1e-9 {
                ell += 1;
            }
            if u.abs() <= rect_l / 2.0 * (1.0 + 1e-9) && v.abs() <= rect_w / 2.0 * (1.0 + 1e-9) {
                rec += 1;
            }
        }
        out[9] = ell as f64 / n;
        out[10] = rec as f64 / n;
    }
    out
}

// ---- texture

const GLCM: &[&str] = &["homogeneity", "contrast", "entropy", "correlation", "asm"];

/// The grey level of a cell without a value.
const NO_GREY: u16 = u16::MAX;

/// The quantile p (0 to 1) of values, interpolated between the two in the middle (numpy's default); reorders them.
fn quantile(v: &mut [f64], p: f64) -> f64 {
    let pos = p * (v.len() - 1) as f64;
    let lo = pos.floor() as usize;
    v.select_nth_unstable_by(lo, f64::total_cmp);
    let a = v[lo];
    if lo + 1 >= v.len() || pos == lo as f64 {
        return a;
    }
    let b = v[lo + 1..].iter().copied().fold(f64::INFINITY, f64::min);
    a + (b - a) * (pos - lo as f64)
}

/// The grey levels of a band: linear between its 1st and 99th percentile over the whole image, outside them the
/// first and last level; a constant band is level 0.
fn grey(values: &[f64], levels: usize) -> Vec<u16> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| !x.is_nan()).collect();
    if v.is_empty() {
        return vec![NO_GREY; values.len()];
    }
    let (lo, hi) = (quantile(&mut v, 0.01), quantile(&mut v, 0.99));
    let scale = if hi > lo { levels as f64 / (hi - lo) } else { 0.0 };
    values.iter().map(|&x| level_of(x, lo, scale, levels)).collect()
}

/// The grey level of a value for the cut `lo`, `scale` ([`grey`]).
fn level_of(x: f64, lo: f64, scale: f64, levels: usize) -> u16 {
    if x.is_nan() { NO_GREY } else { (((x - lo) * scale).floor().max(0.0) as usize).min(levels - 1) as u16 }
}

/// The offsets (row, column) of the pairs at distance 1: 0, 45, 90 and 135 degrees (the opposite ones come with the
/// matrix being symmetric).
const PAIRS: [(i64, i64); 4] = [(0, 1), (-1, 1), (-1, 0), (-1, -1)];

/// The five statistics of [`GLCM`] of one object's matrix: `pairs` pairs counted in the entries `touched` (positions
/// in the levels x levels matrix, in the order they came) whose counts are in `cnt`; NaN without a pair.
fn glcm_stats(touched: &[usize], cnt: &[u32], pairs: u64, l: usize, on: &[bool; 5]) -> [f64; 5] {
    let mut out = [f64::NAN; 5];
    if pairs > 0 {
        let total = (2 * pairs) as f64;
        let (mut hom, mut con, mut ent, mut asm, mut mu) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for &e in touched.iter() {
            let (i, j) = ((e / l) as f64, (e % l) as f64);
            let p = cnt[e] as f64 / total;
            hom += p / (1.0 + (i - j) * (i - j));
            con += p * (i - j) * (i - j);
            ent -= p * p.ln();
            asm += p * p;
            mu += i * p;
        }
        out[0] = hom;
        out[1] = con;
        out[2] = ent;
        out[4] = asm;
        // (by the symmetry both axes have the same mean and spread; one entry is one level on the
        // diagonal: the object is constant, no spread, and the correlation is taken as 1)
        out[3] = if touched.len() == 1 {
            1.0
        } else if on[3] {
            let (mut var, mut cov) = (0.0f64, 0.0f64);
            for &e in touched.iter() {
                let (i, j) = ((e / l) as f64 - mu, (e % l) as f64 - mu);
                let p = cnt[e] as f64 / total;
                var += i * i * p;
                cov += i * j * p;
            }
            cov / var
        } else {
            f64::NAN
        };
    }
    out
}

/// The texture of the objects over grey levels: per object the five statistics of [`GLCM`] (NaN where not wanted or
/// there is no pair).
///
/// The matrix of an object counts the pairs of its cells with a grey level, both in the object, in the four
/// directions at distance 1, each pair in both orders (symmetric), and is divided by the count. A dense levels x levels
/// array per thread holds it, and only the entries touched are read and cleared, in the order they came.
fn glcm(inp: &Input, grey: &[u16], on: &[bool; 5]) -> Vec<[f64; 5]> {
    let o = inp.o;
    let w = o.grid.width;
    let l = inp.levels;
    (0..o.count)
        .into_par_iter()
        .map_init(
            || (vec![0u32; l * l], Vec::<usize>::new()),
            |(cnt, touched), k| {
                let id = k as u32 + 1;
                let mut pairs = 0u64;
                let mut bump = |a: usize, b: usize, touched: &mut Vec<usize>| {
                    let e = &mut cnt[a * l + b];
                    if *e == 0 {
                        touched.push(a * l + b);
                    }
                    *e += 1;
                };
                for &i in inp.cells_of(k) {
                    let i = i as usize;
                    let a = grey[i];
                    if a == NO_GREY {
                        continue;
                    }
                    let (r, c) = (i / w, i % w);
                    for (dr, dc) in PAIRS {
                        let nc = c as i64 + dc;
                        if (dr < 0 && r == 0) || nc < 0 || nc >= w as i64 {
                            continue;
                        }
                        let j = (r as i64 + dr) as usize * w + nc as usize;
                        let b = grey[j];
                        if o.labels[j] != id || b == NO_GREY {
                            continue;
                        }
                        pairs += 1;
                        bump(a as usize, b as usize, touched);
                        bump(b as usize, a as usize, touched);
                    }
                }
                let out = glcm_stats(touched, cnt, pairs, l, on);
                for &e in touched.iter() {
                    cnt[e] = 0;
                }
                touched.clear();
                out
            },
        )
        .collect()
}

/// The texture: per layer (the brightness, then the bands) and statistic, a column `glcm_<stat>` for the brightness
/// (the mean of the image bands in each cell) and `glcm_<stat>_<band>`, the statistics by [`glcm`] of the grey levels
/// of [`grey`]. `basic` alone (homogeneity not named) reads the brightness only; `texture_bands` says the layers.
fn texture(inp: &Input, want: Want) -> Result<Columns> {
    let on: [bool; 5] = std::array::from_fn(|f| want.iter().any(|w| w.0 == GLCM[f]));
    let basic_only = want.len() == 1 && want[0].0 == "homogeneity" && !want[0].1;
    // the layers: None is the brightness
    let targets: Vec<Option<usize>> = match &inp.texture_bands {
        Some(list) if list.is_empty() => {
            return Err(Error::msg(format!("node {:?}: texture_bands: no name given", inp.node)));
        }
        Some(list) => list
            .iter()
            .map(|n| {
                if n == "brightness" {
                    Ok(None)
                } else {
                    layer_named(inp.node, &inp.names, "texture_bands", n).map(Some)
                }
            })
            .collect::<Result<_>>()?,
        None if basic_only => vec![None],
        None => std::iter::once(None).chain((0..inp.image).map(Some)).collect(),
    };
    let cells = inp.o.grid.cells();
    let tables: Vec<Vec<[f64; 5]>> = if let Some(src) = &inp.strips {
        texture_strips(inp, src, &targets, &on)?
    } else {
        targets
            .iter()
            .map(|t| {
                let g = match t {
                    Some(j) => grey(&inp.data[*j], inp.levels),
                    None => {
                        let b: Vec<f64> = (0..cells)
                            .into_par_iter()
                            .map(|i| {
                                let (s, c) = (0..inp.image)
                                    .map(|j| inp.data[j][i])
                                    .filter(|v| !v.is_nan())
                                    .fold((0.0, 0usize), |(s, c), v| (s + v, c + 1));
                                if c == 0 { f64::NAN } else { s / c as f64 }
                            })
                            .collect();
                        grey(&b, inp.levels)
                    }
                };
                glcm(inp, &g, &on)
            })
            .collect()
    };
    let mut out: Columns = Vec::new();
    for (f, stat) in GLCM.iter().enumerate().filter(|(f, _)| on[*f]) {
        for (t, rows) in targets.iter().zip(&tables) {
            let name = match t {
                None => format!("glcm_{stat}"),
                Some(j) => format!("glcm_{stat}_{}", inp.names[*j]),
            };
            out.push((name, rows.iter().map(|r| r[f]).collect()));
        }
    }
    Ok(out)
}

// ---- texture by strips

/// The order-preserving integer of a number (for [`f64::total_cmp`]'s order), and back.
fn key(v: f64) -> u64 {
    let b = v.to_bits();
    if b >> 63 == 1 { !b } else { b | 1 << 63 }
}

fn unkey(k: u64) -> f64 {
    f64::from_bits(if k >> 63 == 1 { k ^ 1 << 63 } else { !k })
}

/// The 1st and 99th percentile of values met strip after strip, exactly, as [`quantile`] gives them over all the
/// values: the four order statistics they need (two each, the second for the interpolation) found by radix passes of
/// 16 bits over the numbers' keys, each pass a histogram of the digit among the values whose higher digits are the
/// ones found so far. Four passes over the strips, nothing else held.
struct Quant {
    n: u64,
    /// The ranks wanted, the digits found so far of each, and the rank within the values of those digits.
    ranks: [u64; 4],
    prefix: [u64; 4],
    rem: [u64; 4],
    /// A histogram of the next digit for each distinct prefix.
    hist: Vec<(u64, Vec<u64>)>,
}

impl Quant {
    fn new() -> Quant {
        Quant { n: 0, ranks: [0; 4], prefix: [0; 4], rem: [0; 4], hist: vec![(0, vec![0; 1 << 16])] }
    }

    /// The values of a strip in pass `pass` (0 to 3).
    fn add(&mut self, pass: u32, values: &[f64]) {
        if pass > 0 && self.n == 0 {
            return;
        }
        let (shift, above) = (48 - 16 * pass, 64 - 16 * pass);
        for &x in values.iter().filter(|x| !x.is_nan()) {
            let k = key(x);
            for (pre, h) in self.hist.iter_mut() {
                if pass == 0 || k >> above == *pre {
                    h[((k >> shift) & 0xFFFF) as usize] += 1;
                }
            }
        }
    }

    /// The pass over: the next digit of each rank.
    fn finish(&mut self, pass: u32) {
        if pass == 0 {
            self.n = self.hist[0].1.iter().sum();
            if self.n == 0 {
                return;
            }
            let at = |p: f64| ((p * (self.n - 1) as f64).floor() as u64).min(self.n - 1);
            let (a, b) = (at(0.01), at(0.99));
            self.ranks = [a, (a + 1).min(self.n - 1), b, (b + 1).min(self.n - 1)];
            self.rem = self.ranks;
        }
        if self.n == 0 {
            return;
        }
        for r in 0..4 {
            let h = &self.hist.iter().find(|(p, _)| *p == self.prefix[r]).expect("a histogram of the prefix").1;
            let mut cum = 0;
            for (b, &c) in h.iter().enumerate() {
                if self.rem[r] < cum + c {
                    self.prefix[r] = (self.prefix[r] << 16) | b as u64;
                    self.rem[r] -= cum;
                    break;
                }
                cum += c;
            }
        }
        let mut pre = self.prefix.to_vec();
        pre.sort_unstable();
        pre.dedup();
        self.hist = pre.into_iter().map(|p| (p, vec![0; 1 << 16])).collect();
    }

    /// The cut of the grey levels (the first percentile and the scale), none without values.
    fn cut(&self, levels: usize) -> Option<(f64, f64)> {
        if self.n == 0 {
            return None;
        }
        let q = |p: f64, at: usize| {
            let pos = p * (self.n - 1) as f64;
            let lo = pos.floor() as u64;
            let a = unkey(self.prefix[at]);
            if lo + 1 >= self.n || pos == lo as f64 {
                return a;
            }
            let b = unkey(self.prefix[at + 1]);
            a + (b - a) * (pos - lo as f64)
        };
        let (lo, hi) = (q(0.01, 0), q(0.99, 2));
        Some((lo, if hi > lo { levels as f64 / (hi - lo) } else { 0.0 }))
    }
}

/// The bands the targets read (the image's for the brightness), ascending.
fn bands_of(targets: &[Option<usize>], image: usize) -> Vec<usize> {
    let mut which: Vec<usize> =
        targets.iter().flat_map(|t| t.map_or_else(|| (0..image).collect(), |j| vec![j])).collect();
    which.sort_unstable();
    which.dedup();
    which
}

/// The values of each target over a strip (`rows` are the bands `which`): a band's as they are, the brightness (the
/// mean of the image bands' values in each cell, summed in the band order) made.
fn target_rows<'a>(
    rows: &'a [Cow<'_, [f64]>],
    which: &[usize],
    image: usize,
    targets: &[Option<usize>],
) -> Vec<Cow<'a, [f64]>> {
    let at = |j: usize| which.iter().position(|&x| x == j).expect("a band read");
    targets
        .iter()
        .map(|t| match t {
            Some(j) => Cow::Borrowed(&*rows[at(*j)]),
            None => {
                let bands: Vec<&[f64]> = (0..image).map(|j| &*rows[at(j)]).collect();
                Cow::Owned(
                    (0..bands[0].len())
                        .map(|i| {
                            let (s, c) = bands
                                .iter()
                                .map(|b| b[i])
                                .filter(|v| !v.is_nan())
                                .fold((0.0, 0usize), |(s, c), v| (s + v, c + 1));
                            if c == 0 { f64::NAN } else { s / c as f64 }
                        })
                        .collect(),
                )
            }
        })
        .collect()
}

/// An object's co-occurrence counts, the pairs of grey levels (a * levels + b) in the order they came and their counts.
#[derive(Default)]
struct Sparse {
    pairs: u64,
    entries: Vec<(u32, u32)>,
    /// The keys in order, with the place of each in `entries`.
    order: Vec<(u32, u32)>,
}

impl Sparse {
    fn bump(&mut self, key: u32) {
        match self.order.binary_search_by_key(&key, |e| e.0) {
            Ok(p) => self.entries[self.order[p].1 as usize].1 += 1,
            Err(p) => {
                self.order.insert(p, (key, self.entries.len() as u32));
                self.entries.push((key, 1));
            }
        }
    }
}

/// The pairs of cells of a strip (rows `r0..r1`, whole width) of one target counted for the objects, those across
/// the strip's top edge with the last row of the strip before (`prev`, kept for the next).
fn glcm_strip(inp: &Input, r0: usize, vals: &[f64], cut: Option<(f64, f64)>, prev: &mut Vec<u16>, sp: &mut [Sparse]) {
    let o = inp.o;
    let (w, l) = (o.grid.width, inp.levels);
    let rows = vals.len() / w;
    let g: Vec<u16> = vals
        .iter()
        .map(|&x| match cut {
            Some((lo, scale)) => level_of(x, lo, scale, l),
            None => NO_GREY,
        })
        .collect();
    for r in r0..r0 + rows {
        for c in 0..w {
            let id = o.labels[r * w + c];
            let a = g[(r - r0) * w + c];
            if id == 0 || a == NO_GREY {
                continue;
            }
            for (dr, dc) in PAIRS {
                let nc = c as i64 + dc;
                if (dr < 0 && r == 0) || nc < 0 || nc >= w as i64 {
                    continue;
                }
                let (nr, nc) = ((r as i64 + dr) as usize, nc as usize);
                let b = if nr >= r0 { g[(nr - r0) * w + nc] } else { prev[nc] };
                if o.labels[nr * w + nc] != id || b == NO_GREY {
                    continue;
                }
                let s = &mut sp[id as usize - 1];
                s.pairs += 1;
                s.bump(a as u32 * l as u32 + b as u32);
                s.bump(b as u32 * l as u32 + a as u32);
            }
        }
    }
    prev.clear();
    prev.extend_from_slice(&g[(rows - 1) * w..]);
}

/// The texture statistics of each object for each target, from an image read by strips of rows: the grey levels' cut
/// by four passes ([`Quant`]), then the pairs counted strip by strip in the order of the cells, so each matrix is the
/// one [`glcm`] makes. The matrices are kept sparse per object; targets go in groups when they would take much memory.
fn texture_strips(inp: &Input, src: &Layered, targets: &[Option<usize>], on: &[bool; 5]) -> Result<Vec<Vec<[f64; 5]>>> {
    let o = inp.o;
    let (h, n, l) = (o.grid.height, o.count, inp.levels);
    let which = bands_of(targets, inp.image);
    let mut quants: Vec<Quant> = targets.iter().map(|_| Quant::new()).collect();
    for pass in 0..4 {
        strips(src, h, &which, |_, _, rows| {
            let tv = target_rows(rows, &which, inp.image, targets);
            quants.par_iter_mut().zip(tv.par_iter()).for_each(|(q, v)| q.add(pass, v));
        })?;
        quants.iter_mut().for_each(|q| q.finish(pass));
    }
    let cuts: Vec<Option<(f64, f64)>> = quants.iter().map(|q| q.cut(l)).collect();
    drop(quants);
    // (some 600 bytes an object and target: groups of targets that keep it under 1 GiB)
    let group = ((1usize << 30) / (n.max(1) * 600)).max(1);
    let mut tables: Vec<Vec<[f64; 5]>> = Vec::with_capacity(targets.len());
    for ids in (0..targets.len()).collect::<Vec<usize>>().chunks(group) {
        let sub: Vec<Option<usize>> = ids.iter().map(|&t| targets[t]).collect();
        let which = bands_of(&sub, inp.image);
        let mut sp: Vec<Vec<Sparse>> = sub.iter().map(|_| (0..n).map(|_| Sparse::default()).collect()).collect();
        let mut prev: Vec<Vec<u16>> = sub.iter().map(|_| Vec::new()).collect();
        strips(src, h, &which, |r0, _, rows| {
            let tv = target_rows(rows, &which, inp.image, &sub);
            sp.par_iter_mut()
                .zip(prev.par_iter_mut())
                .zip(tv.par_iter())
                .zip(ids.par_iter())
                .for_each(|(((s, p), v), &t)| glcm_strip(inp, r0, v, cuts[t], p, s));
        })?;
        for s in sp {
            tables.push(
                s.into_par_iter()
                    .map_init(
                        || (vec![0u32; l * l], Vec::<usize>::new()),
                        |(cnt, touched), s| {
                            for &(k, c) in &s.entries {
                                cnt[k as usize] = c;
                                touched.push(k as usize);
                            }
                            let out = glcm_stats(touched, cnt, s.pairs, l, on);
                            for &e in touched.iter() {
                                cnt[e] = 0;
                            }
                            touched.clear();
                            out
                        },
                    )
                    .collect(),
            );
        }
    }
    Ok(tables)
}

// ---- context

const CONTEXT: &[&str] =
    &["nb_diff", "nb_count", "border_image", "rel_border", "dist", "nb_class", "parent", "children"];

/// The distance from each point to the nearest of the `members` (indices into `pts`) other than itself: NaN when
/// there is none. A uniform grid of about one member to a bucket, rings of buckets around the point searched until
/// the best is nearer than the ring: about O(n + m) for n points and m members when they are spread, O(n m) at worst
/// (all in a bucket). The distance is exact, so the result does not depend on the order of the search.
pub(super) fn nearest(pts: &[(f64, f64)], members: &[usize]) -> Vec<f64> {
    if members.is_empty() {
        return vec![f64::NAN; pts.len()];
    }
    let m = members.len();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &k in members {
        let (x, y) = pts[k];
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    let (dx, dy) = (x1 - x0, y1 - y0);
    let mut s = if dx * dy > 0.0 { (dx * dy / m as f64).sqrt() } else { dx.max(dy) / m as f64 };
    // (at most 4 m + 1 buckets a side)
    s = s.max(dx.max(dy) / (4.0 * m as f64));
    if !(s > 0.0 && s.is_finite()) {
        s = 1.0;
    }
    let (nx, ny) = ((dx / s) as i64 + 1, (dy / s) as i64 + 1);
    let bucket = |p: (f64, f64)| (((p.0 - x0) / s).floor() as i64, ((p.1 - y0) / s).floor() as i64);
    let at = |bx: i64, by: i64| (by * nx + bx) as usize;
    let mut start = vec![0usize; (nx * ny) as usize + 1];
    for &k in members {
        let (bx, by) = bucket(pts[k]);
        start[at(bx, by) + 1] += 1;
    }
    for b in 0..(nx * ny) as usize {
        start[b + 1] += start[b];
    }
    let mut next = start.clone();
    let mut items = vec![0usize; m];
    for &k in members {
        let (bx, by) = bucket(pts[k]);
        items[next[at(bx, by)]] = k;
        next[at(bx, by)] += 1;
    }
    (0..pts.len())
        .into_par_iter()
        .map(|k| {
            let (qx, qy) = bucket(pts[k]);
            let rmax = qx.abs().max((nx - 1 - qx).abs()).max(qy.abs()).max((ny - 1 - qy).abs());
            let best = std::cell::Cell::new(f64::INFINITY);
            let visit = |bx: i64, by: i64| {
                let b = at(bx, by);
                for &j in &items[start[b]..start[b + 1]] {
                    if j != k {
                        let d = (pts[j].0 - pts[k].0).powi(2) + (pts[j].1 - pts[k].1).powi(2);
                        best.set(best.get().min(d));
                    }
                }
            };
            for r in 0..=rmax {
                if r == 0 {
                    if (0..nx).contains(&qx) && (0..ny).contains(&qy) {
                        visit(qx, qy);
                    }
                } else {
                    let (xa, xb) = ((qx - r).max(0), (qx + r).min(nx - 1));
                    for by in [qy - r, qy + r] {
                        if (0..ny).contains(&by) {
                            (xa..=xb).for_each(|bx| visit(bx, by));
                        }
                    }
                    let (ya, yb) = ((qy - r + 1).max(0), (qy + r - 1).min(ny - 1));
                    for bx in [qx - r, qx + r] {
                        if (0..nx).contains(&bx) {
                            (ya..=yb).for_each(|by| visit(bx, by));
                        }
                    }
                }
                // (anything in a ring further out is at least r buckets away)
                let reach = r as f64 * s * 0.999_999;
                if best.get() <= reach * reach {
                    break;
                }
            }
            let best = best.get();
            if best.is_finite() { best.sqrt() } else { f64::NAN }
        })
        .collect()
}

/// The mean of each object in each image band, from the strips of rows (summed in cell order, so as from memory).
fn means_strips(inp: &Input, src: &Layered) -> Result<Vec<Vec<f64>>> {
    let o = inp.o;
    let (w, h, n) = (o.grid.width, o.grid.height, o.count);
    let bands: Vec<usize> = (0..inp.image).collect();
    let mut acc: Vec<(Vec<f64>, Vec<usize>)> = bands.iter().map(|_| (vec![0.0; n], vec![0usize; n])).collect();
    strips(src, h, &bands, |r0, r1, rows| {
        let labels = &o.labels[r0 * w..r1 * w];
        acc.par_iter_mut().zip(rows.par_iter()).for_each(|((sum, cnt), v)| {
            for (&l, &x) in labels.iter().zip(v.iter()) {
                if l != 0 && !x.is_nan() {
                    sum[l as usize - 1] += x;
                    cnt[l as usize - 1] += 1;
                }
            }
        });
    })?;
    Ok(acc
        .into_iter()
        .map(|(sum, cnt)| sum.iter().zip(&cnt).map(|(s, &c)| if c == 0 { f64::NAN } else { s / c as f64 }).collect())
        .collect())
}

/// The context of the objects: their neighbours, classes, the level above and the level beneath.
///
/// Means are the image bands' (cells with a value), made here. `nb_diff_<band>` is eCognition's mean difference to
/// neighbours: the sum over neighbours of border length x (own mean - neighbour's) over the sum of border lengths;
/// a neighbour without a mean is left out, NaN when none is left. Borders are in cell edges (4-neighbourhood), the
/// perimeter being all the edges that face another object, no object or the image's edge; `border_image` is the share
/// of it that faces no object or the edge, `rel_border_<c>` the share that faces objects of class c. A feature that
/// needs classes, a parent level or `below` and has none is left out, or an error when it was named.
fn context(inp: &Input, want: Want) -> Result<Columns> {
    let o = inp.o;
    let n = o.count;
    let has = |f: &str| want.iter().any(|w| w.0 == f);
    let need = |f: &str, ok: bool, what: &str| -> Result<bool> {
        if !has(f) || !ok {
            if has(f) && want.iter().any(|w| w.0 == f && w.1) {
                return Err(Error::msg(format!("node {:?}: {f} needs {what}", inp.node)));
            }
            return Ok(false);
        }
        Ok(true)
    };
    let classes = !o.classes.is_empty();
    let cname: Vec<String> = o.classes.iter().map(|c| crate::vector_algebra::script_name(&c.name)).collect();
    let mut out: Columns = Vec::new();
    if has("nb_diff") {
        // (the means of the image bands by object, made by strips when the image is read so)
        let by_strips = match &inp.strips {
            Some(src) => Some(means_strips(inp, src)?),
            None => None,
        };
        for j in 0..inp.image {
            let mean: Vec<f64> = match &by_strips {
                Some(m) => m[j].clone(),
                None => {
                    let mut sum = vec![0.0f64; n];
                    let mut cnt = vec![0usize; n];
                    for (&l, &v) in o.labels.iter().zip(inp.data[j].iter()) {
                        if l != 0 && !v.is_nan() {
                            sum[l as usize - 1] += v;
                            cnt[l as usize - 1] += 1;
                        }
                    }
                    sum.iter().zip(&cnt).map(|(s, &c)| if c == 0 { f64::NAN } else { s / c as f64 }).collect()
                }
            };
            let v = (0..n)
                .map(|k| {
                    if mean[k].is_nan() {
                        return f64::NAN;
                    }
                    let (mut s, mut len) = (0.0f64, 0.0f64);
                    for &(nb, l) in &o.adjacency[k] {
                        let x = mean[nb as usize - 1];
                        if !x.is_nan() {
                            s += l as f64 * (mean[k] - x);
                            len += l as f64;
                        }
                    }
                    if len == 0.0 { f64::NAN } else { s / len }
                })
                .collect();
            out.push((format!("nb_diff_{}", inp.names[j]), v));
        }
    }
    if has("nb_count") {
        out.push(("nb_count".into(), o.adjacency.iter().map(|a| a.len() as f64).collect()));
    }
    // the perimeter in cell edges
    let rel = need("rel_border", classes, "objects with classes")?;
    let perimeter: Vec<f64> = if has("border_image") || rel {
        let (w, h) = (o.grid.width, o.grid.height);
        (0..n)
            .into_par_iter()
            .map(|k| {
                let id = k as u32 + 1;
                let mut e = 0usize;
                for &i in inp.cells_of(k) {
                    let i = i as usize;
                    let (r, c) = (i / w, i % w);
                    e += (c == 0 || o.labels[i - 1] != id) as usize
                        + (c + 1 == w || o.labels[i + 1] != id) as usize
                        + (r == 0 || o.labels[i - w] != id) as usize
                        + (r + 1 == h || o.labels[i + w] != id) as usize;
                }
                e as f64
            })
            .collect()
    } else {
        Vec::new()
    };
    if has("border_image") {
        let v = (0..n)
            .map(|k| {
                let inside: f64 = o.adjacency[k].iter().map(|&(_, l)| l as f64).sum();
                if perimeter[k] == 0.0 { f64::NAN } else { (perimeter[k] - inside) / perimeter[k] }
            })
            .collect();
        out.push(("border_image".into(), v));
    }
    if rel {
        for (c, name) in cname.iter().enumerate() {
            let v = (0..n)
                .map(|k| {
                    let l: f64 = o.adjacency[k]
                        .iter()
                        .filter(|&&(nb, _)| o.class_of[nb as usize - 1] as usize == c + 1)
                        .map(|&(_, l)| l as f64)
                        .sum();
                    l / perimeter[k]
                })
                .collect();
            out.push((format!("rel_border_{name}"), v));
        }
    }
    if need("dist", classes, "objects with classes")? {
        // the centroids in map coordinates, as the shape's
        let g = &o.grid;
        let (w, t) = (g.width, g.transform);
        let cent: Vec<(f64, f64)> = (0..n)
            .into_par_iter()
            .map(|k| {
                let cells = inp.cells_of(k);
                let (mut sr, mut sc) = (0.0f64, 0.0f64);
                for &i in cells {
                    sr += (i as usize / w) as f64;
                    sc += (i as usize % w) as f64;
                }
                let (cr, cc) = (sr / cells.len() as f64 + 0.5, sc / cells.len() as f64 + 0.5);
                (t[0] + t[1] * cc + t[2] * cr, t[3] + t[4] * cc + t[5] * cr)
            })
            .collect();
        for (c, name) in cname.iter().enumerate() {
            let members: Vec<usize> = (0..n).filter(|&k| o.class_of[k] as usize == c + 1).collect();
            out.push((format!("dist_{name}"), nearest(&cent, &members)));
        }
    }
    if need("nb_class", classes, "objects with classes")? {
        for (c, name) in cname.iter().enumerate() {
            let v = (0..n)
                .map(|k| {
                    o.adjacency[k].iter().filter(|&&(nb, _)| o.class_of[nb as usize - 1] as usize == c + 1).count()
                        as f64
                })
                .collect();
            out.push((format!("nb_{name}"), v));
        }
    }
    if need("parent", o.parent.is_some(), "a parent level")? {
        let p = o.parent.as_ref().expect("a parent level");
        let up = |f: &dyn Fn(usize) -> f64| -> Vec<f64> {
            (0..n).map(|k| if o.parent_of[k] == 0 { f64::NAN } else { f(o.parent_of[k] as usize - 1) }).collect()
        };
        out.push(("parent_id".into(), up(&|q| q as f64 + 1.0)));
        for (j, col) in p.table.columns.iter().enumerate() {
            if col != "id" && !p.table.is_text(col) {
                out.push((format!("parent_{col}"), up(&|q| p.table.rows[q][j])));
            }
        }
    }
    if need("children", inp.below.is_some(), "the input below")? {
        let b = inp.below.expect("a level beneath");
        let bc = b.classes.len();
        let mut count = vec![0.0f64; n];
        let mut by = vec![vec![0.0f64; n]; bc];
        let mut area = vec![vec![0.0f64; n]; bc];
        for (j, &p) in b.parent_of.iter().enumerate() {
            if p == 0 {
                continue;
            }
            let k = p as usize - 1;
            count[k] += 1.0;
            if let Some(c) = (b.class_of[j] as usize).checked_sub(1) {
                by[c][k] += 1.0;
                area[c][k] += b.cells[j] as f64;
            }
        }
        out.push(("children".into(), count));
        for (c, class) in b.classes.iter().enumerate() {
            let name = crate::vector_algebra::script_name(&class.name);
            out.push((format!("children_{name}"), by[c].clone()));
            let share = area[c].iter().zip(&o.cells).map(|(a, &cells)| a / cells as f64).collect();
            out.push((format!("children_area_{name}"), share));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same rectangle by trying every edge direction of the hull (and a little more).
    fn brute(h: &[(f64, f64)]) -> (f64, f64) {
        let mut best = (f64::INFINITY, 0.0, 0.0);
        for i in 0..h.len() {
            let (a, b) = (h[i], h[(i + 1) % h.len()]);
            let l = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let d = ((b.0 - a.0) / l, (b.1 - a.1) / l);
            let (mut lo, mut hi, mut dn, mut up) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
            for p in h {
                let (x, y) = (p.0 * d.0 + p.1 * d.1, -p.0 * d.1 + p.1 * d.0);
                lo = lo.min(x);
                hi = hi.max(x);
                dn = dn.min(y);
                up = up.max(y);
            }
            if (hi - lo) * (up - dn) < best.0 {
                best = ((hi - lo) * (up - dn), hi - lo, up - dn);
            }
        }
        (best.1.max(best.2), best.1.min(best.2))
    }

    #[test]
    fn the_calipers_find_the_rectangle_of_every_hull() {
        let mut s = 99u64;
        let mut next = |m: i64| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((s >> 33) as i64) % m
        };
        for round in 0..300 {
            let m = 3 + next(40) as usize;
            let pts: Vec<(i64, i64)> = (0..m).map(|_| (next(60), next(25))).collect();
            let h = hull(pts);
            if h.len() < 3 {
                continue;
            }
            let (sx, sy) = if round % 2 == 0 { (1.0, 1.0) } else { (30.0, 10.0) };
            let h: Vec<(f64, f64)> = h.into_iter().map(|(x, y)| (x as f64 * sx, y as f64 * sy)).collect();
            let (a, b) = (min_rect(&h), brute(&h));
            assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9, "round {round}: {a:?} {b:?}");
        }
    }
}
