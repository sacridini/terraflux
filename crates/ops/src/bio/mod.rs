//! Biodiversity analyses (landschaft's bio module, a port of Biodiverse): basedata of groups x labels sample
//! counts from occurrence tables or points, Biodiverse's spatial conditions (its Perl, parsed and compiled here)
//! and its indices over the neighbourhood of every group, computed by landschaft's C++.

pub mod basedata;
pub mod conditions;
pub mod names;
pub mod perl;
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
        let r = spatial::spatial_analysis(
            &bd,
            &conditions,
            &calcs,
            query.as_ref(),
            tree.as_deref(),
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
        let run = spatial::prepare(&bd, &conditions, &calcs, query.as_ref(), tree.as_deref(), seed as u64, parallel)
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
