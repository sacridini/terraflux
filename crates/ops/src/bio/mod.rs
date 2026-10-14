//! Biodiversity analyses (landschaft's bio module, a port of Biodiverse): basedata of groups x labels sample
//! counts from occurrence tables or points, Biodiverse's spatial conditions (its Perl, parsed and compiled here)
//! and its indices over the neighbourhood of every group, computed by landschaft's C++.

pub mod basedata;
pub mod cluster;
pub mod conditions;
pub mod imports;
pub mod names;
pub mod perl;
pub mod properties;
pub mod rand;
pub mod spatial;
pub mod tree;

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;
use terraflux_raster::vector::Column;

pub use basedata::BaseData;
use basedata::TableImport;
use conditions::Condition;
use names::{csv_field, dequote_element, perl_number};

use crate::{Args, Ctx, Error, Key, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(FromTable));
    r.add(Arc::new(FromPoints));
    r.add(Arc::new(Spatial));
    r.add(Arc::new(ReadTree));
    r.add(Arc::new(Randomise));
    r.add(Arc::new(Cluster { region_grower: false }));
    r.add(Arc::new(Cluster { region_grower: true }));
    r.add(Arc::new(ReadProperties));
    r.add(Arc::new(ReadMatrix));
    r.add(Arc::new(FromRaster));
    r.add(Arc::new(FromVector));
    r.add(Arc::new(FromHex));
    r.add(Arc::new(FromH3));
    r.add(Arc::new(FromOccurrences));
}

impl BaseData {
    /// The counts as a long table: a row per group and label, with the group's axes.
    pub fn to_csv(&self) -> String {
        let q = |s: &str| {
            if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
        };
        let n_axes = self.n_axes();
        let mut out = String::from("group");
        for k in 0..n_axes {
            out.push_str(&format!(",axis_{k}"));
        }
        out.push_str(",label,count\n");
        for g in 0..self.n_groups() {
            for e in self.indptr[g] as usize..self.indptr[g + 1] as usize {
                out.push_str(&q(&self.groups[g]));
                for k in 0..n_axes {
                    out.push(',');
                    out.push_str(&perl_number(self.coords[g * n_axes + k]));
                }
                out.push_str(&format!(
                    ",{},{}\n",
                    q(&self.labels[self.indices[e] as usize]),
                    perl_number(self.counts[e])
                ));
            }
        }
        out
    }
}

/// A parameter of one text or a list of them (numbers as written).
fn texts(args: &Args, name: &str) -> Result<Vec<String>> {
    Ok(match args.params.get(name) {
        None | Some(serde_json::Value::Null) => Vec::new(),
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(serde_json::Value::Array(a)) => a
            .iter()
            .map(|v| match v {
                serde_json::Value::String(s) => Ok(s.clone()),
                serde_json::Value::Number(n) => Ok(n.to_string()),
                _ => Err(Error::msg(format!("node {:?}: {name}: texts or numbers", args.node))),
            })
            .collect::<Result<_>>()?,
        Some(serde_json::Value::Number(n)) => vec![n.to_string()],
        Some(_) => return Err(Error::msg(format!("node {:?}: {name}: a text or a list of them", args.node))),
    })
}

/// A parameter of one number or a list of them.
fn numbers(args: &Args, name: &str) -> Result<Option<Vec<f64>>> {
    Ok(match args.params.get(name) {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Number(n)) => Some(vec![n.as_f64().unwrap()]),
        Some(_) => Some(args.param::<Vec<f64>>(name, Vec::new())?),
    })
}

fn one_char(args: &Args, name: &str) -> Result<Option<char>> {
    Ok(match args.opt_param::<String>(name)?.as_deref() {
        None | Some("") | Some("guess") => None,
        Some("tab") => Some('\t'),
        Some("space") => Some(' '),
        Some(s) if s.chars().count() == 1 => s.chars().next(),
        Some(s) => return Err(Error::msg(format!("node {:?}: {name}: one character, not {s:?}", args.node))),
    })
}

struct FromTable;

impl Operator for FromTable {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "bio.table",
            "A basedata of a delimited text file read like Biodiverse's import (landschaft's BaseData.from_table): \
             group coordinates from the group columns snapped to cells (> 0), kept as given (0) or as text \
             (< 0), labels from the label columns joined with ':', counts summed per group and label (1 per \
             record without count columns). Columns by name or 0-based position; separator and quote guessed.",
            Access::None,
        )
        .output(PortSpec::new("basedata", Type::BaseData, "groups x labels sample counts"))
        .param(PortSpec::new("path", Type::Text, "the file (CSV, tab-separated...)"))
        .param(PortSpec::new("group_columns", Type::Any, "[\"x\", \"y\"]"))
        .param(PortSpec::new("label_columns", Type::Any, "[\"genus\", \"species\"]"))
        .param(PortSpec::optional("count_columns", Type::Any, "sample counts, summed (default 1 per record)"))
        .param(PortSpec::optional("cell_sizes", Type::Any, "per group column (default 0)"))
        .param(PortSpec::optional("cell_origins", Type::Any, "per group column (default 0)"))
        .param(PortSpec::optional("crs", Type::Text, "of the coordinates (EPSG:..., WKT)"))
        .param(PortSpec::optional("separator", Type::Text, "guess (default), tab, space or a character"))
        .param(PortSpec::optional("quote", Type::Text, "guess (default) or a character"))
        .param(PortSpec::optional("binarise", Type::Bool, "counts become 1"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let path: String = args.param("path", String::new())?;
        let text = std::fs::read(&path).map_err(|e| fail(Error::msg(format!("{path}: {e}"))))?;
        let text = String::from_utf8_lossy(&text);
        let o = TableImport {
            group_columns: texts(args, "group_columns")?,
            label_columns: texts(args, "label_columns")?,
            sample_count_columns: texts(args, "count_columns")?,
            cell_sizes: numbers(args, "cell_sizes")?,
            cell_origins: numbers(args, "cell_origins")?,
            sep: one_char(args, "separator")?,
            quote: one_char(args, "quote")?,
            binarise_counts: args.param("binarise", false)?,
            crs: args.opt_param::<String>("crs")?.filter(|c| !c.is_empty()),
            ..TableImport::default()
        };
        if o.group_columns.is_empty() {
            return Err(fail(Error::msg("give the group columns")));
        }
        let bd = BaseData::from_table(&text, &o).map_err(fail)?;
        Ok(outputs([("basedata", Value::BaseData(Arc::new(bd)))]))
    }
}

struct FromPoints;

impl Operator for FromPoints {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "bio.points",
            "A basedata of point records read like Biodiverse's feature import (landschaft's BaseData.from_points): \
             each point in the cell holding it, its label the label attributes joined with ':', its count 1 or a \
             count attribute's.",
            Access::None,
        )
        .input(PortSpec::new("points", Type::Vector, "the records"))
        .output(PortSpec::new("basedata", Type::BaseData, "groups x labels sample counts"))
        .param(PortSpec::new("label", Type::Any, "the label attribute(s)"))
        .param(PortSpec::optional("count", Type::Text, "a count attribute (default 1 per point)"))
        .param(PortSpec::new("cell_sizes", Type::Any, "[x, y] in the points' units (0 keeps the coordinates)"))
        .param(PortSpec::optional("cell_origins", Type::Any, "[x, y] (default 0)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let Some(Value::Vector(v)) = args.get("points") else { return Err(fail("connect points".into())) };
        let column = |name: &str| -> Result<&Column> {
            v.attributes
                .iter()
                .find(|(n, _)| n == name)
                .or_else(|| v.attributes.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)))
                .map(|(_, c)| c)
                .ok_or_else(|| fail(format!("no attribute {name:?}")))
        };
        let label_cols: Vec<&Column> = texts(args, "label")?.iter().map(|n| column(n)).collect::<Result<_>>()?;
        if label_cols.is_empty() {
            return Err(fail("give the label attribute".into()));
        }
        let text_of = |c: &Column, i: usize| -> String {
            match c {
                Column::Numbers(x) if x[i].is_nan() => String::new(),
                Column::Numbers(x) => perl_number(x[i]),
                Column::Texts(t) => t[i].clone(),
            }
        };
        let count = match args.opt_param::<String>("count")?.filter(|c| !c.is_empty()) {
            Some(name) => Some(match column(&name)? {
                Column::Numbers(x) => x.iter().map(|v| if v.is_nan() { 0.0 } else { *v }).collect::<Vec<f64>>(),
                Column::Texts(t) => t.iter().map(|s| s.trim().parse().unwrap_or(0.0)).collect(),
            }),
            None => None,
        };
        let sizes = numbers(args, "cell_sizes")?.unwrap_or_default();
        if sizes.len() != 2 {
            return Err(fail("feature imports need two cell sizes (x and y)".into()));
        }
        let origins = numbers(args, "cell_origins")?.unwrap_or_else(|| vec![0.0, 0.0]);
        let (mut xy, mut labels, mut counts) = (Vec::new(), Vec::new(), Vec::new());
        for (i, g) in v.geometries.iter().enumerate() {
            let joined = label_cols.iter().map(|c| csv_field(&text_of(c, i))).collect::<Vec<_>>().join(":");
            let label = if label_cols.len() <= 1 { dequote_element(&joined) } else { joined };
            let base = count.as_ref().map_or(1.0, |c| c[i]);
            g.points(&mut |p| {
                xy.push(p);
                labels.push(label.clone());
                counts.push(base);
            });
        }
        let bd =
            BaseData::from_points(&xy, labels, counts, [sizes[0], sizes[1]], [origins[0], origins[1]], v.crs.clone())
                .map_err(|e| fail(e.to_string()))?;
        Ok(outputs([("basedata", Value::BaseData(Arc::new(bd)))]))
    }
}

/// The label matrix of a node's `matrix` port, if connected.
fn matrix_of(args: &Args) -> Result<Option<Arc<properties::LabelMatrix>>> {
    match args.get("matrix") {
        Some(Value::LabelMatrix(m)) => Ok(Some(m.clone())),
        Some(_) => Err(Error::msg(format!("node {:?}: matrix: connect a label matrix (bio.matrix)", args.node))),
        None => Ok(None),
    }
}

/// The basedata of a node's port.
fn basedata_of(args: &Args) -> Result<Arc<BaseData>> {
    match args.get("basedata") {
        Some(Value::BaseData(b)) => Ok(b.clone()),
        _ => Err(Error::msg(format!("node {:?}: connect a basedata (bio.table, bio.points)", args.node))),
    }
}

/// The grid of a basedata's groups when they are cells of two numeric axes, and each group's cell.
fn group_grid(bd: &BaseData) -> Option<(Grid, Vec<usize>)> {
    if bd.n_axes() != 2 || bd.cell_sizes.iter().any(|s| *s <= 0.0) || bd.n_groups() == 0 {
        return None;
    }
    let (sx, sy) = (bd.cell_sizes[0], bd.cell_sizes[1]);
    let xs: Vec<f64> = (0..bd.n_groups()).map(|g| bd.coords[2 * g]).collect();
    let ys: Vec<f64> = (0..bd.n_groups()).map(|g| bd.coords[2 * g + 1]).collect();
    let (x0, x1) = xs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
    let (y0, y1) = ys.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &y| (a.min(y), b.max(y)));
    let width = ((x1 - x0) / sx).round() as usize + 1;
    let height = ((y1 - y0) / sy).round() as usize + 1;
    if width.saturating_mul(height) > 100_000_000 {
        return None;
    }
    let mut grid = Grid::new(width, height);
    grid.transform = [x0 - sx / 2.0, sx, 0.0, y1 + sy / 2.0, 0.0, -sy];
    grid.geo.wkt = bd.crs.clone();
    let cells = xs
        .iter()
        .zip(&ys)
        .map(|(x, y)| ((y1 - y) / sy).round() as usize * width + ((x - x0) / sx).round() as usize)
        .collect();
    Some((grid, cells))
}

struct Spatial;

impl Operator for Spatial {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "bio.spatial",
            "Biodiverse's spatial analysis (landschaft's spatial_analysis): its indices over the neighbourhood of \
             every group. Neighbourhoods are spatial conditions in Biodiverse's Perl (sp_circle (radius => \
             200000), sp_block (size => 500000), $D <= 3 && $nbr_y > $y, sp_select_all()...): one, or two (the \
             second leaves out the first) for the indices comparing two sets. Calculations by Biodiverse's names, \
             with or without calc_ (richness, endemism_whole, sorenson, rarity_central, abc_counts...). A table \
             of the indices of each group, and their maps when the groups are cells of two axes.",
            Access::None,
        )
        .input(PortSpec::new("basedata", Type::BaseData, "groups x labels sample counts"))
        .input(PortSpec::optional("tree", Type::Tree, "a phylogeny for the phylogenetic indices (bio.tree)"))
        .input(PortSpec::optional("matrix", Type::LabelMatrix, "a label matrix for the matrix indices (bio.matrix)"))
        .output(PortSpec::new("table", Type::Table, "a row per processed group: its coordinates, then each index"))
        .output(PortSpec::new("maps", Type::Stack, "an index per layer (groups that are cells of two axes)"))
        .output(PortSpec::new("lists", Type::Any, "the list indices: per list and group, its keys and values (JSON)"))
        .param(PortSpec::new("conditions", Type::Any, "one or two spatial conditions"))
        .param(PortSpec::new("calculations", Type::Any, "[\"richness\", \"endemism_whole\", ...]"))
        .param(PortSpec::optional("definition_query", Type::Text, "only the groups passing it are processed"))
        .param(PortSpec::optional(
            "seed",
            Type::Integer,
            "Biodiverse's prng_seed: the random tips of the expected NTI (trees not ultrametric) and the net VPD (default 0)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let bd = basedata_of(args)?;
        let conditions: Vec<Condition> =
            texts(args, "conditions")?.iter().map(|c| Condition::new(c)).collect::<Result<_>>().map_err(fail)?;
        let calcs = texts(args, "calculations")?;
        let query = args
            .opt_param::<String>("definition_query")?
            .filter(|q| !q.trim().is_empty())
            .map(|q| Condition::new(&q))
            .transpose()
            .map_err(fail)?;
        let tree = match args.get("tree") {
            Some(Value::Tree(t)) => Some(t.clone()),
            Some(_) => return Err(fail(Error::msg("tree: connect a tree (bio.tree)"))),
            None => None,
        };
        let seed: i64 = args.param("seed", 0)?;
        let matrix = matrix_of(args)?;
        let r = spatial::spatial_analysis(
            &bd,
            &conditions,
            &calcs,
            query.as_ref(),
            tree.as_deref(),
            matrix.as_deref(),
            seed as u64,
            rayon::current_num_threads() > 1,
        )
        .map_err(fail)?;
        let mut out = vec![
            ("table", Value::Table(index_table(&bd, &r.groups, &r.names, &r.rows))),
            ("lists", Value::Json(list_json(&bd, &r))),
        ];
        if let Some(maps) = index_maps(&bd, &r.groups, &r.names, &r.rows) {
            out.push(("maps", maps));
        }
        Ok(out.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
}

/// A table of indices: a row per group (its number and coordinates), a column per index.
fn index_table(bd: &BaseData, groups: &[i32], names: &[String], rows: &[Vec<f64>]) -> Table {
    let n_axes = bd.n_axes();
    let mut table = Table::new(
        ["group".to_string()]
            .into_iter()
            .chain((0..n_axes).map(|k| ["x", "y", "z"].get(k).map_or(format!("axis_{k}"), |s| s.to_string())))
            .chain(names.iter().cloned())
            .collect(),
    );
    for (row, &g) in rows.iter().zip(groups) {
        let g = g as usize;
        table.rows.push(
            std::iter::once((g + 1) as f64)
                .chain(bd.coords[g * n_axes..(g + 1) * n_axes].iter().copied())
                .chain(row.iter().copied())
                .collect(),
        );
    }
    table
}

/// Maps of indices, when the groups are cells of two axes: a layer per index.
fn index_maps(bd: &BaseData, groups: &[i32], names: &[String], rows: &[Vec<f64>]) -> Option<Value> {
    let (grid, cells) = group_grid(bd)?;
    let n = grid.cells();
    let layers = (0..names.len())
        .map(|j| {
            let mut m = vec![f64::NAN; n];
            for (row, &g) in rows.iter().zip(groups) {
                m[cells[g as usize]] = row[j];
            }
            Arc::new(m)
        })
        .collect();
    Some(Value::Stack(Stack {
        grid: Arc::new(grid),
        keys: names.iter().map(|n| Key::Name(n.clone())).collect(),
        layers,
    }))
}

struct ReadTree;

impl Operator for ReadTree {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "bio.tree",
            "A phylogenetic tree of a Newick or Nexus file read as Biodiverse reads it (landschaft's read_tree): \
             unnamed nodes 0___, 1___..., duplicated names __dup1..., the Nexus Translate table applied, all-zero \
             lengths made 1, a forest given a new root.",
            Access::None,
        )
        .output(PortSpec::new("tree", Type::Tree, "the tree"))
        .param(PortSpec::new("path", Type::Text, "the file (or the Newick text itself)"))
        .param(
            PortSpec::optional("format", Type::Text, "nexus or newick (default: from the text)")
                .choices(["nexus", "newick"]),
        )
        .param(PortSpec::optional("index", Type::Any, "the tree's number (0, default) or name"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let path: String = args.param("path", String::new())?;
        let text = if !path.contains('(') && std::path::Path::new(&path).exists() {
            String::from_utf8_lossy(&std::fs::read(&path).map_err(|e| fail(Error::msg(format!("{path}: {e}"))))?)
                .into_owned()
        } else {
            path.clone()
        };
        let format = args.opt_param::<String>("format")?.filter(|f| !f.is_empty());
        let trees = tree::read_trees(&text, format.as_deref()).map_err(fail)?;
        let t = match args.params.get("index") {
            Some(serde_json::Value::String(name)) => trees.into_iter().find(|t| t.name == *name),
            Some(v) => trees.into_iter().nth(v.as_u64().unwrap_or(0) as usize),
            None => trees.into_iter().next(),
        }
        .ok_or_else(|| fail(Error::msg("no such tree")))?;
        Ok(outputs([("tree", Value::Tree(Arc::new(t)))]))
    }
}

/// The list indices as Biodiverse shows them: per list and processing group, its keys (labels, groups, texts,
/// numbers, tree nodes) and values, or the keys in order (arrays).
fn list_json(bd: &BaseData, r: &spatial::SpatialResult) -> serde_json::Value {
    let key = |kind: i32, k: f64| -> serde_json::Value {
        let s = |v: &Vec<String>| v.get(k as usize).cloned().unwrap_or_default();
        match kind {
            0 => s(&bd.labels).into(),
            1 | 4 => s(&bd.groups).into(),
            2 => s(&r.texts).into(),
            5 => s(&r.nodes).into(),
            6 => serde_json::json!(k),
            _ => perl_number(k).into(),
        }
    };
    let lists = r
        .lists
        .iter()
        .map(|l| {
            let groups = r
                .groups
                .iter()
                .zip(&l.rows)
                .map(|(&g, row)| {
                    let v = if l.kind == 4 || l.kind == 6 {
                        serde_json::Value::Array(row.iter().map(|e| key(l.kind, e.0)).collect())
                    } else {
                        let mut m = serde_json::Map::new();
                        for e in row {
                            let k = match key(l.kind, e.0) {
                                serde_json::Value::String(s) => s,
                                v => v.to_string(),
                            };
                            m.insert(k, serde_json::json!(e.1));
                        }
                        serde_json::Value::Object(m)
                    };
                    (bd.groups[g as usize].clone(), v)
                })
                .collect::<serde_json::Map<_, _>>();
            (l.name.clone(), serde_json::Value::Object(groups))
        })
        .collect::<serde_json::Map<_, _>>();
    serde_json::Value::Object(lists)
}

struct Randomise;

impl Operator for Randomise {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "bio.randomise",
            "Randomisations of a spatial analysis (landschaft's randomise, Biodiverse's): each iteration \
             randomises the basedata (rand_structured, Biodiverse's default; its spatial forms \
             rand_spatially_structured, rand_diffusion, rand_random_walk; the swaps rand_independent_swaps, \
             rand_independent_swaps_modified, rand_curveball; rand_csr_by_group; rand_nochange), runs the \
             analysis again with the same neighbourhoods and counts how often each observed index exceeds, ties \
             and is compared with the randomised one. The p-ranks, z-scores, the counters (C, P, Q, SUMX, SUMXX, \
             T) and, with pe and phylo_rpe2, CANAPE (0 not significant, 1 neo, 2 palaeo, 3 mixed, 4 super). \
             Biodiverse's Mersenne Twister: the same seed gives its randomisations.",
            Access::None,
        )
        .input(PortSpec::new("basedata", Type::BaseData, "groups x labels sample counts"))
        .input(PortSpec::optional("tree", Type::Tree, "a phylogeny for the phylogenetic indices (bio.tree)"))
        .input(PortSpec::optional("matrix", Type::LabelMatrix, "a label matrix for the matrix indices (bio.matrix)"))
        .output(PortSpec::new("table", Type::Table, "the observed indices"))
        .output(PortSpec::new("p_rank", Type::Table, "the p-rank of each index"))
        .output(PortSpec::new("z_scores", Type::Table, "the z-score of each index"))
        .output(PortSpec::new("comparison", Type::Table, "the counters: C_, P_, Q_, SUMX_, SUMXX_, T_ of each index"))
        .output(PortSpec::new("canape", Type::Table, "CANAPE (with pe and phylo_rpe2)"))
        .output(PortSpec::new("maps", Type::Stack, "the p-ranks, and CANAPE (groups that are cells of two axes)"))
        .param(PortSpec::new("conditions", Type::Any, "one or two spatial conditions (bio.spatial)"))
        .param(PortSpec::new("calculations", Type::Any, "[\"richness\", \"pe\", \"phylo_rpe2\", ...]"))
        .param(PortSpec::optional("definition_query", Type::Text, "of the analysis: only the groups passing it"))
        .param(PortSpec::optional("seed", Type::Integer, "of the analysis (Biodiverse's prng_seed; default 0)"))
        .param(PortSpec::optional("function", Type::Text, "rand_structured (default)...").choices(rand::FUNCTIONS))
        .param(PortSpec::optional("iterations", Type::Integer, "99 (default)"))
        .param(PortSpec::optional("rand_seed", Type::Integer, "of the randomisations (default 0)"))
        .param(PortSpec::optional("richness_multiplier", Type::Number, "rand_structured: richness targets x (1)"))
        .param(PortSpec::optional("richness_addition", Type::Number, "rand_structured: richness targets + (0)"))
        .param(PortSpec::optional(
            "spatial_conditions_for_label_allocation",
            Type::Text,
            "rand_structured: occurrences around seed groups (sp_select_all() for its spatial forms)",
        ))
        .param(PortSpec::optional("spatial_allocation_order", Type::Text, "diffusion, random_walk, random, proximity"))
        .param(PortSpec::optional("label_allocation_backtracking", Type::Text, "from_end, from_start, random"))
        .param(PortSpec::optional("spatial_allocation_reseed_prob", Type::Number, "0 to 1"))
        .param(PortSpec::optional("spatial_conditions_for_seed_location", Type::Text, "the seed groups"))
        .param(PortSpec::optional("swap_count", Type::Integer, "swaps (default twice the non-zero cells)"))
        .param(PortSpec::optional("max_swap_attempts", Type::Integer, "(default 100 times the swaps)"))
        .param(PortSpec::optional("stop_on_all_swapped", Type::Bool, "swaps: stop when every cell was swapped"))
        .param(PortSpec::optional("use_max_swap", Type::Bool, "curveball"))
        .param(PortSpec::optional("spatial_conditions_for_subset", Type::Text, "randomise within subsets of groups"))
        .param(PortSpec::optional("subset_definition_query", Type::Text, "groups randomised (others kept)"))
        .param(PortSpec::optional("labels_not_to_randomise", Type::Any, "labels kept in place"))
        .param(
            PortSpec::optional("randomise_group_props_by", Type::Text, "no_change (default), by_set, by_item")
                .choices(["no_change", "by_set", "by_item"]),
        )
        .param(
            PortSpec::optional("randomise_trees_by", Type::Text, "shuffle_terminal_names")
                .choices(["shuffle_no_change", "shuffle_terminal_names"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let bd = basedata_of(args)?;
        let conditions: Vec<Condition> =
            texts(args, "conditions")?.iter().map(|c| Condition::new(c)).collect::<Result<_>>().map_err(fail)?;
        let calcs = texts(args, "calculations")?;
        let text = |name: &str| -> Result<Option<String>> {
            Ok(args.opt_param::<String>(name)?.filter(|t| !t.trim().is_empty()))
        };
        let query = text("definition_query")?.map(|q| Condition::new(&q)).transpose().map_err(fail)?;
        let tree = match args.get("tree") {
            Some(Value::Tree(t)) => Some(t.clone()),
            Some(_) => return Err(fail(Error::msg("tree: connect a tree (bio.tree)"))),
            None => None,
        };
        let seed: i64 = args.param("seed", 0)?;
        let parallel = rayon::current_num_threads() > 1;
        let matrix = matrix_of(args)?;
        let run = spatial::prepare(
            &bd,
            &conditions,
            &calcs,
            query.as_ref(),
            tree.as_deref(),
            matrix.as_deref(),
            seed as u64,
            parallel,
        )
        .map_err(fail)?;
        let o = rand::Options {
            richness_multiplier: args.opt_param("richness_multiplier")?,
            richness_addition: args.opt_param("richness_addition")?,
            spatial_conditions_for_label_allocation: text("spatial_conditions_for_label_allocation")?,
            spatial_allocation_order: text("spatial_allocation_order")?,
            label_allocation_backtracking: text("label_allocation_backtracking")?,
            spatial_allocation_reseed_prob: args.opt_param("spatial_allocation_reseed_prob")?,
            spatial_conditions_for_seed_location: text("spatial_conditions_for_seed_location")?,
            swap_count: args.opt_param("swap_count")?,
            max_swap_attempts: args.opt_param("max_swap_attempts")?,
            stop_on_all_swapped: args.param("stop_on_all_swapped", false)?,
            use_max_swap: args.param("use_max_swap", false)?,
            spatial_conditions_for_subset: text("spatial_conditions_for_subset")?,
            definition_query: text("subset_definition_query")?,
            labels_not_to_randomise: texts(args, "labels_not_to_randomise")?,
            randomise_trees_by: text("randomise_trees_by")?,
            randomise_group_props_by: text("randomise_group_props_by")?,
        };
        let function: String = args.param("function", "rand_structured".to_string())?;
        let rand_seed: i64 = args.param("rand_seed", 0)?;
        let mut rnd = rand::Randomisation::new(&bd, run, &function, rand_seed as u64, o).map_err(fail)?;
        let iterations: i64 = args.param("iterations", 99)?;
        rnd.run(iterations.max(0) as usize).map_err(fail)?;
        let r = &rnd.result;
        let k = r.names.len();
        let rows = |v: &[f64]| -> Vec<Vec<f64>> { v.chunks(k.max(1)).map(<[f64]>::to_vec).collect() };
        let s = &rnd.scalars;
        let p_rank = s.p_rank();
        let p = s.p();
        let mut comp_names = Vec::new();
        for part in ["C", "P", "Q", "SUMX", "SUMXX", "T"] {
            comp_names.extend(r.names.iter().map(|n| format!("{part}_{n}")));
        }
        let comp_rows: Vec<Vec<f64>> = (0..r.groups.len())
            .map(|g| {
                [&s.c, &p, &s.q, &s.sumx, &s.sumxx, &s.t].iter().flat_map(|v| v[g * k..(g + 1) * k].to_vec()).collect()
            })
            .collect();
        let mut out = vec![
            ("table", Value::Table(index_table(&bd, &r.groups, &r.names, &r.rows))),
            ("p_rank", Value::Table(index_table(&bd, &r.groups, &r.names, &rows(&p_rank)))),
            ("z_scores", Value::Table(index_table(&bd, &r.groups, &r.names, &rows(&s.z_scores())))),
            ("comparison", Value::Table(index_table(&bd, &r.groups, &comp_names, &comp_rows))),
        ];
        let mut map_names: Vec<String> = r.names.iter().map(|n| format!("P_RANK_{n}")).collect();
        let mut map_rows = rows(&p_rank);
        if let Some(code) = rnd.canape(false, false) {
            let names: Vec<String> =
                ["CANAPE_CODE", "NEO", "PALAEO", "MIXED", "SUPER"].iter().map(|s| s.to_string()).collect();
            let crow: Vec<Vec<f64>> = code
                .iter()
                .map(|&c| {
                    std::iter::once(c)
                        .chain((1..=4).map(|k| if c.is_nan() { f64::NAN } else { f64::from(u8::from(c == k as f64)) }))
                        .collect()
                })
                .collect();
            out.push(("canape", Value::Table(index_table(&bd, &r.groups, &names, &crow))));
            map_names.push("CANAPE_CODE".into());
            for (row, c) in map_rows.iter_mut().zip(&code) {
                row.push(*c);
            }
        }
        if let Some(maps) = index_maps(&bd, &r.groups, &map_names, &map_rows) {
            out.push(("maps", maps));
        }
        Ok(out.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
}

struct Cluster {
    region_grower: bool,
}

impl Operator for Cluster {
    fn signature(&self) -> OpSignature {
        let sig =
            if self.region_grower {
                OpSignature::new(
                "bio.region_grower",
                "Biodiverse's region grower (landschaft's region_grower): pairs of groups joined in decreasing order \
                 of an index of the pair's groups together (RICHNESS_ALL, PE_WE...), recomputed for each merged \
                 region; with one condition selecting every group, joining stops once a region reaches the value \
                 of all the groups. The dendrogram, its nodes, the calculations of each node and, with k, k \
                 regions.",
                Access::None,
            )
            .param(PortSpec::optional("index", Type::Text, "the index to maximise (RICHNESS_ALL, default)"))
            } else {
                OpSignature::new(
                    "bio.cluster",
                    "Biodiverse's cluster analysis (landschaft's cluster): groups joined pair by pair by the \
                 dissimilarity of their labels (SORENSON, JACCARD, S2, BRAY_CURTIS, PHYLO_SORENSON...; the linkage \
                 giving cluster to cluster values), within the first spatial condition first. Ties broken by \
                 Biodiverse's Mersenne Twister. The dendrogram, its nodes (Biodiverse's values), the calculations \
                 of each node's groups and, with k, k clusters.",
                    Access::None,
                )
                .param(PortSpec::optional("index", Type::Text, "the dissimilarity (SORENSON, default)"))
                .param(
                    PortSpec::optional("linkage", Type::Text, "link_average (default)...").choices(cluster::LINKAGES),
                )
            };
        sig.input(PortSpec::new("basedata", Type::BaseData, "groups x labels sample counts"))
            .input(PortSpec::optional("tree", Type::Tree, "a phylogeny for phylogenetic indices (bio.tree)"))
            .input(PortSpec::optional("matrix", Type::LabelMatrix, "a label matrix (bio.matrix)"))
            .output(PortSpec::new("tree", Type::Tree, "the dendrogram: groups at the tips, joins named 0___..."))
            .output(PortSpec::new(
                "nodes",
                Type::Table,
                "a row per node (name order): its parent, length, Biodiverse's values, its group",
            ))
            .output(PortSpec::new("table", Type::Table, "the calculations of each node's groups"))
            .output(PortSpec::new("clusters", Type::Table, "with k: each group's cluster"))
            .output(PortSpec::new("map", Type::Map, "with k: the clusters (groups that are cells of two axes)"))
            .param(PortSpec::optional(
                "conditions",
                Type::Any,
                "spatial conditions limiting the pairs (sp_select_all())",
            ))
            .param(PortSpec::optional("definition_query", Type::Text, "only the groups passing it"))
            .param(PortSpec::optional("calculations", Type::Any, "indices of each node's groups"))
            .param(PortSpec::optional("seed", Type::Integer, "Biodiverse's prng_seed, of the ties (default 1)"))
            .param(PortSpec::optional(
                "tie_breaker",
                Type::Any,
                "[\"random\", \"maximise\"] (default: random shuffles)",
            ))
            .param(PortSpec::optional("clear_singletons", Type::Bool, "leave out groups that joined nothing"))
            .param(PortSpec::optional("k", Type::Integer, "cut the dendrogram into k clusters"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let bd = basedata_of(args)?;
        let tree = match args.get("tree") {
            Some(Value::Tree(t)) => Some(t.clone()),
            Some(_) => return Err(fail(Error::msg("tree: connect a tree (bio.tree)"))),
            None => None,
        };
        let rg = self.region_grower;
        let o = cluster::ClusterOptions {
            index: args.param("index", if rg { "RICHNESS_ALL".to_string() } else { "SORENSON".to_string() })?,
            linkage: if rg { "link_recalculate".into() } else { args.param("linkage", "link_average".to_string())? },
            conditions: texts(args, "conditions")?,
            definition_query: args.opt_param::<String>("definition_query")?.filter(|q| !q.trim().is_empty()),
            calculations: texts(args, "calculations")?,
            seed: args.param::<i64>("seed", 1)? as u64,
            tie_breaker: texts(args, "tie_breaker")?,
            clear_singletons: args.param("clear_singletons", false)?,
            region_grower: rg,
        };
        let matrix = matrix_of(args)?;
        let r = cluster::cluster(&bd, tree.as_deref(), matrix.as_deref(), &o, rayon::current_num_threads() > 1)
            .map_err(fail)?;
        let mut nodes = Table::new(
            ["node", "parent", "length"]
                .iter()
                .map(|s| s.to_string())
                .chain(cluster::VALUES.iter().map(|s| s.to_string()))
                .chain(["group".to_string()])
                .collect(),
        );
        let index: std::collections::HashMap<&str, usize> =
            r.nodes.iter().enumerate().map(|(i, n)| (n.name.as_str(), i)).collect();
        for (i, n) in r.nodes.iter().enumerate() {
            let group = if n.children.is_empty() {
                bd.group_index(&n.name).map_or(f64::NAN, |g| (g + 1) as f64)
            } else {
                f64::NAN
            };
            nodes.rows.push(
                [(i + 1) as f64, n.parent.as_deref().map_or(f64::NAN, |p| (index[p] + 1) as f64), n.length]
                    .into_iter()
                    .chain(n.values.iter().map(|v| v.unwrap_or(f64::NAN)))
                    .chain([group])
                    .collect(),
            );
        }
        let mut out = vec![("tree", Value::Tree(Arc::new(r.tree.clone()))), ("nodes", Value::Table(nodes))];
        if let Some(t) = &r.table {
            let mut table = Table::new(std::iter::once("node".to_string()).chain(t.names.iter().cloned()).collect());
            table.rows = t
                .rows
                .iter()
                .enumerate()
                .map(|(i, row)| std::iter::once((i + 1) as f64).chain(row.iter().copied()).collect())
                .collect();
            out.push(("table", Value::Table(table)));
        }
        if let Some(k) = args.opt_param::<i64>("k")? {
            let cut = r.cut(k.max(0) as usize).map_err(fail)?;
            let groups: Vec<i32> = cut.iter().map(|(g, _)| bd.group_index(g).unwrap() as i32).collect();
            let rows: Vec<Vec<f64>> = cut.iter().map(|(_, c)| vec![*c as f64]).collect();
            let names = vec!["cluster".to_string()];
            out.push(("clusters", Value::Table(index_table(&bd, &groups, &names, &rows))));
            if let Some(Value::Stack(s)) = index_maps(&bd, &groups, &names, &rows) {
                out.push(("map", Value::Map(crate::Map::new(s.grid.clone(), (*s.layers[0]).clone()))));
            }
        }
        Ok(out.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
}

/// Columns given by 0-based position or by name in the header of a delimited text.
fn columns(args: &Args, name: &str, header: &[String]) -> Result<Vec<usize>> {
    texts(args, name)?
        .iter()
        .map(|c| {
            header
                .iter()
                .position(|h| h == c)
                .or_else(|| c.parse().ok())
                .ok_or_else(|| Error::msg(format!("node {:?}: {name}: no column {c:?}", args.node)))
        })
        .collect()
}

fn header_of(rows: &[Vec<Option<String>>]) -> Vec<String> {
    rows.first().map_or(Vec::new(), |h| h.iter().map(|c| c.clone().unwrap_or_default()).collect())
}

struct ReadProperties;

impl Operator for ReadProperties {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "bio.properties",
            "Label or group properties of a basedata from a Biodiverse properties file (landschaft's \
             read_properties and set_label_properties / set_group_properties): the element from its columns \
             joined with ':', property names upper-cased, only values that look like numbers (empty fields set \
             but undefined), a later row overriding. They feed the lbprop and gpprop calculations; RANGE and \
             ABUNDANCE set the labels' ranges and abundances.",
            Access::None,
        )
        .input(PortSpec::new("basedata", Type::BaseData, "groups x labels sample counts"))
        .output(PortSpec::new("basedata", Type::BaseData, "with the properties"))
        .param(PortSpec::new("path", Type::Text, "the properties file"))
        .param(PortSpec::optional("of", Type::Text, "labels (default) or groups").choices(["labels", "groups"]))
        .param(PortSpec::new("element_columns", Type::Any, "the element's columns (names or 0-based positions)"))
        .param(PortSpec::new("property_columns", Type::Any, "the property columns (each named by its header)"))
        .param(PortSpec::optional("separator", Type::Text, "guess (default), tab, space or a character"))
        .param(PortSpec::optional("quote", Type::Text, "guess (default) or a character"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let mut bd = (*basedata_of(args)?).clone();
        let path: String = args.param("path", String::new())?;
        let text =
            String::from_utf8_lossy(&std::fs::read(&path).map_err(|e| fail(Error::msg(format!("{path}: {e}"))))?)
                .into_owned();
        let (sep, quote) = (one_char(args, "separator")?, one_char(args, "quote")?);
        let rows = properties::rows_of(&text, sep, quote);
        let header = header_of(&rows);
        let element = columns(args, "element_columns", &header)?;
        let props: Vec<(String, usize)> = columns(args, "property_columns", &header)?
            .into_iter()
            .map(|c| (header.get(c).cloned().unwrap_or_else(|| c.to_string()), c))
            .collect();
        let (names, per) = properties::read_properties(&text, &element, &props, sep, quote).map_err(fail)?;
        match args.param("of", "labels".to_string())?.as_str() {
            "groups" => bd.set_group_properties(&names, &per),
            "labels" => bd.set_label_properties(&names, &per),
            o => return Err(fail(Error::msg(format!("of: labels or groups, not {o:?}")))),
        }
        Ok(outputs([("basedata", Value::BaseData(Arc::new(bd)))]))
    }
}

struct ReadMatrix;

impl Operator for ReadMatrix {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "bio.matrix",
            "A label matrix from a Biodiverse matrix file (landschaft's read_matrix): full or a triangle (the \
             element of each row from its columns, the values in the rows' order), or sparse pairs; the first \
             value read for a pair wins. For the matrix calculations (matrix_stats, compare_dissim_matrix_values, \
             mx_rao_qe).",
            Access::None,
        )
        .output(PortSpec::new("matrix", Type::LabelMatrix, "values between labels"))
        .param(PortSpec::new("path", Type::Text, "the matrix file"))
        .param(PortSpec::optional("element_columns", Type::Any, "the element's columns ([1, 2], default)"))
        .param(PortSpec::optional("values_start_col", Type::Integer, "the first value column (after the elements)"))
        .param(PortSpec::optional("sparse", Type::Bool, "one pair per line"))
        .param(PortSpec::optional("row_columns", Type::Any, "sparse: the row element's columns"))
        .param(PortSpec::optional("col_columns", Type::Any, "sparse: the column element's columns"))
        .param(PortSpec::optional("value_column", Type::Any, "sparse: the value's column"))
        .param(PortSpec::optional("self_similarity", Type::Number, "the value of a missing (a, a)"))
        .param(PortSpec::optional("separator", Type::Text, "guess (default), tab, space or a character"))
        .param(PortSpec::optional("quote", Type::Text, "guess (default) or a character"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let path: String = args.param("path", String::new())?;
        let text =
            String::from_utf8_lossy(&std::fs::read(&path).map_err(|e| fail(Error::msg(format!("{path}: {e}"))))?)
                .into_owned();
        let (sep, quote) = (one_char(args, "separator")?, one_char(args, "quote")?);
        let header = header_of(&properties::rows_of(&text, sep, quote));
        let mut o = properties::MatrixImport { sep, quote, ..Default::default() };
        if args.params.contains_key("element_columns") {
            o.element_columns = columns(args, "element_columns", &header)?;
        }
        o.values_start_col = args.opt_param::<i64>("values_start_col")?.map(|v| v as usize);
        o.sparse = args.param("sparse", false)?;
        o.row_columns = columns(args, "row_columns", &header)?;
        o.col_columns = columns(args, "col_columns", &header)?;
        o.value_column = columns(args, "value_column", &header)?.first().copied();
        o.self_similarity = args.opt_param("self_similarity")?;
        let m = properties::read_matrix(&text, &o).map_err(fail)?;
        Ok(outputs([("matrix", Value::LabelMatrix(Arc::new(m)))]))
    }
}

fn basedata_out(bd: BaseData) -> Outputs {
    outputs([("basedata", Value::BaseData(Arc::new(bd)))])
}

fn two(args: &Args, name: &str) -> Result<Option<[f64; 2]>> {
    match numbers(args, name)? {
        None => Ok(None),
        Some(v) if v.len() == 2 => Ok(Some([v[0], v[1]])),
        Some(_) => Err(Error::msg(format!("node {:?}: {name}: two numbers (x and y)", args.node))),
    }
}

struct FromRaster;

impl Operator for FromRaster {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "bio.raster",
            "A basedata of rasters read like Biodiverse's raster import (landschaft's BaseData.from_raster): each \
             layer a label (its name) and the values its counts in the cells' groups (with a threshold, presence \
             from species distribution models), or the values themselves labels, each cell counting 1; the cells \
             snapped to the group cells (default the raster's).",
            Access::Global,
        )
        .input(PortSpec::new("raster", Type::Any, "a stack (a label per layer) or a map"))
        .output(PortSpec::new("basedata", Type::BaseData, "groups x labels sample counts"))
        .param(PortSpec::optional("labels_as_bands", Type::Bool, "a label per layer (default true), else values"))
        .param(PortSpec::optional("threshold", Type::Number, "values >= it count 1, others 0"))
        .param(PortSpec::optional("cell_sizes", Type::Any, "[x, y] of the group cells (default the raster's)"))
        .param(PortSpec::optional("cell_origins", Type::Any, "[x, y] (default 0)"))
        .param(PortSpec::optional("given_label", Type::Text, "one label for every layer"))
        .param(PortSpec::optional("label", Type::Text, "the label of a map (default its node's output name)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let read;
        let raster = match args.get("raster") {
            Some(Value::Raster(r)) => {
                read = Value::Stack(r.stack()?);
                Some(&read)
            }
            v => v,
        };
        type Layers = Vec<(String, Arc<Vec<f64>>)>;
        let (grid, layers): (Arc<Grid>, Layers) = match raster {
            Some(Value::Stack(s)) => {
                (s.grid.clone(), s.keys.iter().map(key_text).zip(s.layers.iter().cloned()).collect())
            }
            Some(_) => {
                let m = args.map("raster")?;
                (m.grid.clone(), vec![(args.param("label", "layer".to_string())?, m.data.clone())])
            }
            None => return Err(fail(Error::msg("connect a raster"))),
        };
        let refs: Vec<(String, &[f64])> = layers.iter().map(|(n, l)| (n.clone(), &l[..])).collect();
        let o = imports::GridImport {
            labels_as_bands: args.param("labels_as_bands", true)?,
            threshold: args.opt_param("threshold")?,
            cell_sizes: two(args, "cell_sizes")?,
            cell_origins: two(args, "cell_origins")?.unwrap_or([0.0, 0.0]),
            given_label: args.opt_param::<String>("given_label")?.filter(|l| !l.is_empty()),
        };
        Ok(basedata_out(imports::from_grid(&grid, &refs, &o, grid.geo.wkt.clone()).map_err(fail)?))
    }
}

struct FromVector;

impl Operator for FromVector {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "bio.vector",
            "A basedata of vector features read like Biodiverse's feature import (landschaft's \
             BaseData.from_vector): points in the cell holding them; lines and polygons over a fishnet of cells \
             spanning the layer, every cell they touch counting 1, or (measure) the area or length of their part \
             in it (GEOS through GDAL); the labels from attributes joined with ':'.",
            Access::Global,
        )
        .input(PortSpec::new("features", Type::Vector, "points, lines or polygons"))
        .output(PortSpec::new("basedata", Type::BaseData, "groups x labels sample counts"))
        .param(PortSpec::new("label", Type::Any, "the label attribute(s)"))
        .param(PortSpec::new("cell_sizes", Type::Any, "[x, y] in the features' units (0 keeps point coordinates)"))
        .param(PortSpec::optional("cell_origins", Type::Any, "[x, y] (default 0)"))
        .param(PortSpec::optional("count", Type::Any, "count attribute(s), summed (default 1, or the measure)"))
        .param(
            PortSpec::optional("measure", Type::Text, "none (the cells touched, default), area or length")
                .choices(["none", "area", "length"]),
        )
        .param(PortSpec::optional("binarise", Type::Bool, "counts become 1"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let Some(Value::Vector(v)) = args.get("features") else { return Err(fail(Error::msg("connect features"))) };
        let sizes = two(args, "cell_sizes")?.ok_or_else(|| fail(Error::msg("feature imports need two cell sizes")))?;
        let measure: String = args.param("measure", "none".into())?;
        let (labels, counts) = (texts(args, "label")?, texts(args, "count")?);
        let o = imports::FeatureImport {
            label_columns: &labels,
            count_columns: &counts,
            cell_sizes: sizes,
            cell_origins: two(args, "cell_origins")?.unwrap_or([0.0, 0.0]),
            measure: match measure.as_str() {
                "none" | "" => None,
                "area" => Some("area"),
                "length" => Some("length"),
                m => return Err(fail(Error::msg(format!("measure is none, area or length, not {m:?}")))),
            },
            binarise: args.param("binarise", false)?,
        };
        Ok(basedata_out(imports::from_features(v, &o).map_err(fail)?))
    }
}

/// Records' coordinates (the columns given, else the points or the coordinate columns), labels and counts.
fn grid_records(args: &Args, xname: &str, yname: &str) -> Result<(imports::Records, Option<String>)> {
    let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
    let occ = match args.get("records") {
        Some(Value::Vector(v)) => crate::sdm::points::Occurrences::Vector(v.clone()),
        Some(Value::Table(t)) => crate::sdm::points::Occurrences::Table(t.clone()),
        _ => return Err(fail("connect the records: points or a table".into())),
    };
    let x: Option<String> = args.opt_param(xname)?;
    let y: Option<String> = args.opt_param(yname)?;
    let (xs, ys, crs) = occ.coordinates(x.as_deref(), y.as_deref()).map_err(fail)?;
    let label_names = texts(args, "label")?;
    if label_names.is_empty() {
        return Err(fail("give the label column".into()));
    }
    let cols: Vec<Column> = label_names
        .iter()
        .map(|n| occ.column(n).ok_or_else(|| fail(format!("no column {n:?}"))))
        .collect::<Result<_>>()?;
    let refs: Vec<&Column> = cols.iter().collect();
    let labels = (0..xs.len())
        .map(|i| {
            let text = |c: &Column| match c {
                Column::Numbers(x) if x[i].is_nan() => String::new(),
                Column::Numbers(x) => perl_number(x[i]),
                Column::Texts(t) => t[i].clone(),
            };
            let joined = refs.iter().map(|c| csv_field(&text(c))).collect::<Vec<_>>().join(":");
            if refs.len() <= 1 { dequote_element(&joined) } else { joined }
        })
        .collect();
    let counts = match args.opt_param::<String>("count")?.filter(|c| !c.is_empty()) {
        Some(c) => Some(crate::sdm::points::numbers(occ.column(&c).ok_or_else(|| fail(format!("no column {c:?}")))?)),
        None => None,
    };
    Ok((imports::Records { x: xs, y: ys, labels, counts }, crs))
}

struct FromHex;

impl Operator for FromHex {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "bio.hex",
            "A basedata of records in a grid of hexagons (landschaft's BaseData.from_hex): the groups the centres \
             of the hexagons (pointy or flat, of a circumradius) holding the records, so that Biodiverse's \
             distance conditions work on them.",
            Access::Global,
        )
        .input(PortSpec::new("records", Type::Any, "points, or a table with coordinate columns"))
        .output(PortSpec::new("basedata", Type::BaseData, "groups x labels sample counts"))
        .param(PortSpec::new("label", Type::Any, "the label column(s)"))
        .param(PortSpec::new("size", Type::Number, "from a hexagon's centre to its vertices (projected units)"))
        .param(PortSpec::optional("orientation", Type::Text, "pointy (default) or flat").choices(["pointy", "flat"]))
        .param(PortSpec::optional("origin", Type::Any, "[x, y], a hexagon's centre (default 0)"))
        .param(PortSpec::optional("count", Type::Text, "a count column (default 1 per record)"))
        .param(PortSpec::optional("x", Type::Text, "the x column (default the points, or x, lon...)"))
        .param(PortSpec::optional("y", Type::Text, "the y column"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let (records, crs) = grid_records(args, "x", "y")?;
        let size: f64 = args.opt_param("size")?.ok_or_else(|| fail(Error::msg("set the size")))?;
        let orientation: String = args.param("orientation", "pointy".into())?;
        if orientation != "pointy" && orientation != "flat" {
            return Err(fail(Error::msg("orientation is pointy or flat")));
        }
        let origin = two(args, "origin")?.unwrap_or([0.0, 0.0]);
        Ok(basedata_out(imports::from_hex(records, size, orientation == "flat", origin, crs).map_err(fail)?))
    }
}

struct FromH3;

impl Operator for FromH3 {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "bio.h3",
            "A basedata of lon/lat records in H3 cells (landschaft's BaseData.from_h3): the groups the centres of \
             the cells of a resolution holding the records, the cells' ids kept.",
            Access::Global,
        )
        .input(PortSpec::new("records", Type::Any, "points, or a table with lon/lat columns"))
        .output(PortSpec::new("basedata", Type::BaseData, "groups x labels sample counts"))
        .param(PortSpec::new("label", Type::Any, "the label column(s)"))
        .param(PortSpec::new("resolution", Type::Integer, "H3's resolution, 0 to 15"))
        .param(PortSpec::optional("count", Type::Text, "a count column (default 1 per record)"))
        .param(PortSpec::optional("lon", Type::Text, "the longitude column (default the points, or lon...)"))
        .param(PortSpec::optional("lat", Type::Text, "the latitude column"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let (mut records, crs) = grid_records(args, "lon", "lat")?;
        crate::sdm::points::to_crs(&mut records.x, &mut records.y, crs.as_deref(), Some("EPSG:4326"))
            .map_err(|e| fail(Error::msg(e)))?;
        let res: u8 = args.opt_param("resolution")?.ok_or_else(|| fail(Error::msg("set the resolution")))?;
        Ok(basedata_out(imports::from_h3(records, res).map_err(fail)?))
    }
}

struct FromOccurrences;

impl Operator for FromOccurrences {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "bio.occurrences",
            "A basedata of occurrence records (landschaft's BaseData.from_occurrences and read_occurrences): a GBIF \
             download (the zip or its table), a Darwin Core Archive (a zip or a folder with meta.xml), a \
             delimited file, or records with Darwin Core columns (sdm.download_gbif's); those without a label or \
             valid coordinates, absent, too uncertain, of other bases of record or with some GBIF issues left \
             out, one per label and place; in cells (degrees, or of to_crs), read a line at a time.",
            Access::Global,
        )
        .input(PortSpec::optional("occurrences", Type::Any, "records with Darwin Core columns, in place of a file"))
        .output(PortSpec::new("basedata", Type::BaseData, "groups x labels sample counts"))
        .param(PortSpec::optional("path", Type::Text, "the GBIF download, Darwin Core Archive or table"))
        .param(PortSpec::new("cell_sizes", Type::Any, "[x, y]: degrees, or the units of to_crs"))
        .param(PortSpec::optional("cell_origins", Type::Any, "[x, y] (default 0)"))
        .param(PortSpec::optional("to_crs", Type::Text, "reproject the records first (EPSG:...)"))
        .param(PortSpec::optional("label", Type::Any, "the column(s) naming the taxon (default species)"))
        .param(PortSpec::optional("count", Type::Text, "the column of individuals (individualCount; default 1)"))
        .param(PortSpec::optional("max_uncertainty", Type::Number, "drop records less precise (metres)"))
        .param(PortSpec::optional("basis_of_record", Type::Any, "keep only these bases (HUMAN_OBSERVATION...)"))
        .param(PortSpec::optional("exclude_issues", Type::Any, "drop records with any of these GBIF issues"))
        .param(PortSpec::optional("present_only", Type::Bool, "drop the ABSENT records (default true)"))
        .param(PortSpec::optional("drop_duplicates", Type::Bool, "one per label and place (default true)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let mut label = texts(args, "label")?;
        if label.is_empty() {
            label = vec!["species".into()];
        }
        let bor = texts(args, "basis_of_record")?;
        let count: Option<String> = args.opt_param::<String>("count")?.filter(|c| !c.is_empty());
        let f = imports::OccurrenceFilter {
            label: &label,
            count: count.as_deref(),
            max_uncertainty: args.opt_param("max_uncertainty")?,
            basis_of_record: (args.params.get("basis_of_record").is_some_and(|v| !v.is_null()))
                .then(|| bor.iter().map(|b| b.to_uppercase()).collect()),
            exclude_issues: texts(args, "exclude_issues")?.into_iter().collect(),
            present_only: args.param("present_only", true)?,
            drop_duplicates: args.param("drop_duplicates", true)?,
        };
        let occ = match args.get("occurrences") {
            Some(Value::Vector(v)) => imports::occurrences_of_columns(&v.attributes, v.len(), &f).map_err(fail)?,
            Some(Value::Table(t)) => {
                let cols: Vec<(String, Column)> = t
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(j, n)| (n.clone(), Column::Numbers(t.rows.iter().map(|r| r[j]).collect())))
                    .collect();
                imports::occurrences_of_columns(&cols, t.rows.len(), &f).map_err(fail)?
            }
            Some(_) => return Err(fail(Error::msg("occurrences: records with Darwin Core columns"))),
            None => {
                let path: String = args.param("path", String::new())?;
                if path.is_empty() {
                    return Err(fail(Error::msg("choose the file, or connect the records")));
                }
                let (occ, bad) = imports::read_occurrences(&path, &f).map_err(fail)?;
                if bad > 0 {
                    ctx.warn(format!("node {:?}: {bad} lines with too many fields skipped", args.node));
                }
                occ
            }
        };
        let sizes = numbers(args, "cell_sizes")?.ok_or_else(|| fail(Error::msg("set the cell sizes")))?;
        let to_crs: Option<String> = args.opt_param::<String>("to_crs")?.filter(|c| !c.is_empty());
        let bd =
            imports::from_occurrences(occ, sizes, numbers(args, "cell_origins")?, to_crs.as_deref()).map_err(fail)?;
        Ok(basedata_out(bd))
    }
}

/// A stack's key as a label.
fn key_text(k: &Key) -> String {
    match k {
        Key::Name(n) => n.clone(),
        Key::Class(c) => perl_number(*c),
        k => format!("{k:?}"),
    }
}
