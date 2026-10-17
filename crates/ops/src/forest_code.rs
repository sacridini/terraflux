//! Brazil's Forest Code (Lei 12.651/2012, with Lei 12.727/2012): its Áreas de Preservação Permanente
//! (`forest_code.app`: art. 4's margins of rivers by their width, of natural lakes by their area, the springs, the
//! veredas, the slopes above 45°, the hilltops, the land above 1,800 m and the protected classes such as mangroves;
//! art. 61-A's narrower margins in the areas consolidated by 22 July 2008, by the property's fiscal modules) and the
//! balance of every property's Reserva Legal (`forest_code.reserve`: art. 12's shares, art. 15's APP counted, art.
//! 67's small properties).

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::Geometry;

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(App));
    r.add(Arc::new(Reserve));
}

fn fail(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

fn on(v: f64) -> bool {
    !v.is_nan() && v != 0.0
}

// -- distances ------------------------------------------------------------------------------------------------------

/// The exact Euclidean distance of every cell's centre to the nearest cell of `features`, and that cell (Felzenszwalb
/// and Huttenlocher's lower envelopes, columns then rows; cells `dx` wide and `dy` high). None without features.
pub fn nearest(features: &[bool], ny: usize, nx: usize, dx: f64, dy: f64) -> (Vec<f64>, Vec<Option<usize>>) {
    const INF: f64 = f64::INFINITY;
    // along the columns: the squared distance to the nearest feature of the column, and its row
    let mut g = vec![INF; ny * nx];
    let mut gr = vec![usize::MAX; ny * nx];
    for c in 0..nx {
        let mut last: Option<usize> = None;
        for r in 0..ny {
            if features[r * nx + c] {
                last = Some(r);
            }
            if let Some(l) = last {
                let d = (r - l) as f64 * dy;
                g[r * nx + c] = d * d;
                gr[r * nx + c] = l;
            }
        }
        let mut next: Option<usize> = None;
        for r in (0..ny).rev() {
            if features[r * nx + c] {
                next = Some(r);
            }
            if let Some(n) = next {
                let d = (n - r) as f64 * dy;
                if d * d < g[r * nx + c] {
                    g[r * nx + c] = d * d;
                    gr[r * nx + c] = n;
                }
            }
        }
    }
    // along the rows: the lower envelope of the parabolas of the columns
    let mut dist = vec![INF; ny * nx];
    let mut near = vec![None; ny * nx];
    let mut v = vec![0usize; nx];
    let mut z = vec![0f64; nx + 1];
    for r in 0..ny {
        let f = |c: usize| g[r * nx + c];
        let x = |c: usize| c as f64 * dx;
        let mut k = 0usize;
        let mut first = None;
        for c in 0..nx {
            if f(c).is_finite() {
                first = Some(c);
                break;
            }
        }
        let Some(c0) = first else { continue };
        v[0] = c0;
        z[0] = -INF;
        z[1] = INF;
        for q in c0 + 1..nx {
            if !f(q).is_finite() {
                continue;
            }
            loop {
                let p = v[k];
                let s = ((f(q) + x(q) * x(q)) - (f(p) + x(p) * x(p))) / (2.0 * (x(q) - x(p)));
                if s <= z[k] && k > 0 {
                    k -= 1;
                    continue;
                }
                if s <= z[k] {
                    // the first parabola is replaced
                    v[0] = q;
                    z[0] = -INF;
                    z[1] = INF;
                    break;
                }
                k += 1;
                v[k] = q;
                z[k] = s;
                z[k + 1] = INF;
                break;
            }
        }
        let mut k = 0;
        for c in 0..nx {
            while z[k + 1] < x(c) {
                k += 1;
            }
            let p = v[k];
            let d = (x(c) - x(p)) * (x(c) - x(p)) + f(p);
            dist[r * nx + c] = d.sqrt();
            near[r * nx + c] = Some(gr[r * nx + p] * nx + p);
        }
    }
    (dist, near)
}

/// The cells of every group of touching cells (8 neighbours) of `features`: the group of each cell (usize::MAX
/// outside) and the cells of each group.
fn groups(features: &[bool], ny: usize, nx: usize) -> (Vec<usize>, Vec<usize>) {
    let mut label = vec![usize::MAX; ny * nx];
    let mut sizes = Vec::new();
    let mut stack = Vec::new();
    for start in 0..ny * nx {
        if !features[start] || label[start] != usize::MAX {
            continue;
        }
        let id = sizes.len();
        let mut n = 0;
        label[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            n += 1;
            let (r, c) = ((i / nx) as i64, (i % nx) as i64);
            for dr in -1..=1 {
                for dc in -1..=1 {
                    let (rr, cc) = (r + dr, c + dc);
                    if rr < 0 || cc < 0 || rr >= ny as i64 || cc >= nx as i64 {
                        continue;
                    }
                    let j = rr as usize * nx + cc as usize;
                    if features[j] && label[j] == usize::MAX {
                        label[j] = id;
                        stack.push(j);
                    }
                }
            }
        }
        sizes.push(n);
    }
    (label, sizes)
}

// -- the APPs -------------------------------------------------------------------------------------------------------

/// Art. 4, I: the margin of a river of a width.
pub fn river_margin(width: f64) -> f64 {
    match width {
        w if w < 10.0 => 30.0,
        w if w <= 50.0 => 50.0,
        w if w <= 200.0 => 100.0,
        w if w <= 600.0 => 200.0,
        _ => 500.0,
    }
}

/// Art. 61-A, §§ 1-4: a river's margin in a consolidated area, by the property's fiscal modules (above 4, the
/// programme's width, at least 20 m: `over4`).
pub fn consolidated_river(modules: f64, over4: f64) -> f64 {
    match modules {
        m if m <= 1.0 => 5.0,
        m if m <= 2.0 => 8.0,
        m if m <= 4.0 => 15.0,
        _ => over4,
    }
}

/// Art. 61-A, § 6: a lake's margin in a consolidated area.
pub fn consolidated_lake(modules: f64) -> f64 {
    match modules {
        m if m <= 1.0 => 5.0,
        m if m <= 2.0 => 8.0,
        m if m <= 4.0 => 15.0,
        _ => 30.0,
    }
}

const KINDS: [&str; 8] = ["river", "lake", "spring", "vereda", "slope", "hilltop", "altitude", "class"];

/// The fiscal modules of the property of every cell (NaN outside the properties).
fn modules_of(properties: Option<&Map>, module_ha: f64, cell_ha: f64) -> Vec<f64> {
    let Some(p) = properties else { return Vec::new() };
    // the cells counted, then their area (a sum of areas would drift across a class's bound)
    let mut cells: BTreeMap<u64, u64> = BTreeMap::new();
    for &v in p.data.iter() {
        if on(v) {
            *cells.entry(v.to_bits()).or_insert(0) += 1;
        }
    }
    p.data.iter().map(|&v| if on(v) { cells[&v.to_bits()] as f64 * cell_ha / module_ha } else { f64::NAN }).collect()
}

struct App;

impl Operator for App {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "forest_code.app",
            "The Áreas de Preservação Permanente of Brazil's Forest Code (Lei 12.651/2012, art. 4): the margins of \
             rivers from the channel's edge by their width (30 m below 10 m wide, 50 m to 50, 100 m to 200, 200 m to \
             600, 500 m beyond), of natural lakes by their area (100 m, 50 m up to 20 ha; 30 m in urban zones), 50 m \
             around springs and from veredas, slopes above 45°, hilltops, land above 1,800 m and protected classes \
             (mangroves, restingas); in rural areas consolidated by 22 July 2008, art. 61-A's narrower margins by the \
             property's fiscal modules (rivers and lakes 5, 8 and 15 m up to 1, 2 and 4 modules, lakes 30 m and \
             rivers the programme's width beyond; springs 15 m; veredas 30 or 50 m). In map units of metres.",
            Access::Global,
        )
        .input(PortSpec::optional("rivers", Type::Map, "the rivers' channels (non-zero)"))
        .input(PortSpec::optional("river_width", Type::Map, "the width of the river at its cells, metres"))
        .input(PortSpec::optional("lakes", Type::Map, "the natural lakes and lagoons (non-zero)"))
        .input(PortSpec::optional("urban", Type::Map, "the urban zones (non-zero): lakes' margins of 30 m"))
        .input(PortSpec::optional("springs", Type::Vector, "the springs (points)"))
        .input(PortSpec::optional("veredas", Type::Map, "the veredas' flooded areas (non-zero)"))
        .input(PortSpec::optional("slope", Type::Map, "the slope, degrees (above 45°)"))
        .input(PortSpec::optional("elevation", Type::Map, "the elevation, metres (above 1,800 m)"))
        .input(PortSpec::optional("hilltops", Type::Map, "the hilltops' APPs (surface.hilltop, non-zero)"))
        .input(PortSpec::optional("landcover", Type::Map, "the land cover, for the protected classes"))
        .input(PortSpec::optional("properties", Type::Map, "the rural properties (ids), for the consolidated areas"))
        .input(PortSpec::optional("consolidated", Type::Map, "the rural areas consolidated by 2008 (non-zero)"))
        .output(PortSpec::new(
            "app",
            Type::Map,
            "the APP of every cell: 1 river, 2 lake, 3 spring, 4 vereda, 5 slope, 6 hilltop, 7 altitude, 8 protected \
             class (the first that applies); 0 none",
        ))
        .output(PortSpec::new("table", Type::Table, "a row per kind: kind (1-8), cells, area_ha (overlaps in each)"))
        .param(PortSpec::optional("river_width", Type::Number, "the rivers' width without a map, metres (default 5)"))
        .param(PortSpec::optional("protected_classes", Type::Any, "the classes of landcover that are APP (a list)"))
        .param(PortSpec::optional("fiscal_module", Type::Number, "the fiscal module, hectares (with consolidated)"))
        .param(PortSpec::optional(
            "over_four_modules",
            Type::Number,
            "the river margin of consolidated areas of properties above 4 modules, metres (default 20: the PRA's, \
             20 to 100)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let maps: Vec<(&str, &Map)> = [
            "rivers",
            "river_width",
            "lakes",
            "urban",
            "veredas",
            "slope",
            "elevation",
            "hilltops",
            "landcover",
            "properties",
            "consolidated",
        ]
        .iter()
        .filter_map(|n| args.opt_map(n).ok().flatten().map(|m| (*n, m)))
        .collect();
        let Some(&(_, first)) = maps.first() else { return Err(fail(args, "connect a map at least")) };
        if let Some((n, _)) = maps.iter().find(|(_, m)| m.data.len() != first.data.len()) {
            return Err(fail(args, format!("{n} and the other maps differ in size")));
        }
        let get = |n: &str| maps.iter().find(|(k, _)| *k == n).map(|(_, m)| *m);
        let g = first.grid.clone();
        let (ny, nx) = (g.height, g.width);
        let (dx, dy) = g.resolution();
        let half = dx.min(dy) / 2.0;
        let cell_ha = dx * dy / 10_000.0;
        let n = ny * nx;
        let consolidated = get("consolidated");
        let modules = if consolidated.is_some() {
            let module: f64 = args.param("fiscal_module", f64::NAN)?;
            if module.is_nan() || module <= 0.0 {
                return Err(fail(args, "fiscal_module: the hectares of a fiscal module, with the consolidated areas"));
            }
            modules_of(get("properties"), module, cell_ha)
        } else {
            Vec::new()
        };
        let over4: f64 = args.param("over_four_modules", 20.0)?;
        let cons = |i: usize| consolidated.is_some_and(|c| on(c.data[i]));
        let module_at = |i: usize| modules.get(i).copied().unwrap_or(f64::INFINITY);
        // every kind's cells
        let mut kinds: Vec<Vec<bool>> = vec![vec![false; n]; KINDS.len()];
        if let Some(r) = get("rivers") {
            ctx.progress(Some(0.0), "rivers");
            let f: Vec<bool> = r.data.iter().map(|v| on(*v)).collect();
            let (d, near) = nearest(&f, ny, nx, dx, dy);
            let default: f64 = args.param("river_width", 5.0)?;
            let width = get("river_width");
            for i in 0..n {
                let Some(j) = near[i] else { continue };
                if f[i] {
                    continue;
                }
                let w = width.map_or(default, |m| if m.data[j].is_nan() { default } else { m.data[j] });
                let margin = if cons(i) { consolidated_river(module_at(i), over4) } else { river_margin(w) };
                kinds[0][i] = d[i] - half <= margin;
            }
        }
        if let Some(l) = get("lakes") {
            ctx.progress(Some(0.2), "lakes");
            let f: Vec<bool> = l.data.iter().map(|v| on(*v)).collect();
            let (label, sizes) = groups(&f, ny, nx);
            let (d, near) = nearest(&f, ny, nx, dx, dy);
            let urban = get("urban");
            for i in 0..n {
                let Some(j) = near[i] else { continue };
                if f[i] {
                    continue;
                }
                let area = sizes[label[j]] as f64 * cell_ha;
                let margin = if cons(i) {
                    consolidated_lake(module_at(i))
                } else if urban.is_some_and(|u| on(u.data[i])) {
                    30.0
                } else if area <= 20.0 {
                    50.0
                } else {
                    100.0
                };
                kinds[1][i] = d[i] - half <= margin;
            }
        }
        if let Some(Value::Vector(v)) = args.get("springs") {
            let v = crate::vector::on_grid(v, &g)?;
            let t = g.transform;
            let mut f = vec![false; n];
            for geom in &v.geometries {
                let pts: Vec<[f64; 2]> = match geom {
                    Geometry::Point(p) => vec![*p],
                    Geometry::MultiPoint(ps) => ps.clone(),
                    _ => return Err(fail(args, "springs: points")),
                };
                for p in pts {
                    let (c, r) = (((p[0] - t[0]) / t[1]).floor(), ((p[1] - t[3]) / t[5]).floor());
                    if c >= 0.0 && r >= 0.0 && (c as usize) < nx && (r as usize) < ny {
                        f[r as usize * nx + c as usize] = true;
                    }
                }
            }
            let (d, _) = nearest(&f, ny, nx, dx, dy);
            for i in 0..n {
                kinds[2][i] = d[i] <= if cons(i) { 15.0 } else { 50.0 };
            }
        }
        if let Some(vr) = get("veredas") {
            let f: Vec<bool> = vr.data.iter().map(|v| on(*v)).collect();
            let (d, _) = nearest(&f, ny, nx, dx, dy);
            for i in 0..n {
                let margin = if cons(i) { if module_at(i) <= 4.0 { 30.0 } else { 50.0 } } else { 50.0 };
                kinds[3][i] = !f[i] && d[i] - half <= margin;
            }
        }
        if let Some(s) = get("slope") {
            kinds[4] = s.data.iter().map(|v| *v > 45.0).collect();
        }
        if let Some(h) = get("hilltops") {
            kinds[5] = h.data.iter().map(|v| on(*v)).collect();
        }
        if let Some(e) = get("elevation") {
            kinds[6] = e.data.iter().map(|v| *v > 1800.0).collect();
        }
        if let Some(lc) = get("landcover") {
            let classes: Vec<f64> = args.param("protected_classes", Vec::new())?;
            kinds[7] = lc.data.iter().map(|v| classes.contains(v)).collect();
        }
        let app: Vec<f64> = (0..n)
            .map(|i| match kinds.iter().position(|k| k[i]) {
                Some(k) => (k + 1) as f64,
                None if first.data[i].is_nan() => f64::NAN,
                None => 0.0,
            })
            .collect();
        let mut t = Table::new(["kind", "cells", "area_ha"].map(String::from).to_vec());
        for (k, cells) in kinds.iter().enumerate() {
            let c = cells.iter().filter(|b| **b).count() as f64;
            t.rows.push(vec![(k + 1) as f64, c, c * cell_ha]);
        }
        Ok(outputs([("app", Value::Map(Map::new(g, app))), ("table", Value::Table(t))]))
    }
}

// -- the Reserva Legal ----------------------------------------------------------------------------------------------

struct Reserve;

impl Operator for Reserve {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "forest_code.reserve",
            "The balance of every rural property's Reserva Legal (Lei 12.651/2012): the share of its area it must \
             keep in native vegetation (art. 12: 80% in the Amazon's forests, 35% in its cerrado, 20% in its campos \
             gerais and the rest of the country; a map of the share, or one for all), its native vegetation counted \
             outside the APPs and, as art. 15 allows, inside them, and its deficit or surplus; the properties of up \
             to 4 fiscal modules on 22 July 2008 keep only the vegetation they had then (art. 67).",
            Access::Global,
        )
        .input(PortSpec::new("properties", Type::Map, "the rural properties (ids)"))
        .input(PortSpec::new("vegetation", Type::Map, "the native vegetation (non-zero)"))
        .input(PortSpec::optional("share", Type::Map, "the Reserva Legal's share of every cell, percent"))
        .input(PortSpec::optional("app", Type::Map, "the APPs (forest_code.app; non-zero)"))
        .input(PortSpec::optional("vegetation_2008", Type::Map, "the native vegetation of 22 July 2008 (art. 67)"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per property: property, area_ha, modules, native_ha, app_ha, app_native_ha, share, required_ha, \
             counted_ha, deficit_ha, surplus_ha",
        ))
        .output(PortSpec::new("balance", Type::Map, "every property's surplus (positive) or deficit (negative), ha"))
        .param(PortSpec::optional("share", Type::Number, "the share without a map, percent (default 20)"))
        .param(PortSpec::optional("fiscal_module", Type::Number, "the fiscal module, hectares (default 0: no art. 67)"))
        .param(PortSpec::optional(
            "count_app",
            Type::Bool,
            "count the native vegetation of the APPs (art. 15; true)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let p = args.map("properties")?;
        let veg = args.map("vegetation")?;
        let n = p.data.len();
        let check = |m: Option<&Map>, what: &str| -> Result<()> {
            if m.is_some_and(|m| m.data.len() != n) {
                return Err(fail(args, format!("{what} and the properties differ in size")));
            }
            Ok(())
        };
        check(Some(veg), "vegetation")?;
        let share_map = args.opt_map("share")?;
        let app = args.opt_map("app")?;
        let veg08 = args.opt_map("vegetation_2008")?;
        check(share_map, "share")?;
        check(app, "app")?;
        check(veg08, "vegetation_2008")?;
        let share: f64 = args.param("share", 20.0)?;
        let module: f64 = args.param("fiscal_module", 0.0)?;
        let count_app: bool = args.param("count_app", true)?;
        let (dx, dy) = p.grid.resolution();
        let ha = dx * dy / 10_000.0;
        // cells counted (and the shares added), their areas at the end
        #[derive(Default)]
        struct Acc {
            area: f64,
            native: f64,
            app: f64,
            app_native: f64,
            required: f64,
            counted: f64,
            counted08: f64,
        }
        let mut by: BTreeMap<u64, Acc> = BTreeMap::new();
        for i in 0..n {
            if !on(p.data[i]) {
                continue;
            }
            let a = by.entry(p.data[i].to_bits()).or_default();
            let in_app = app.is_some_and(|m| on(m.data[i]));
            let native = on(veg.data[i]);
            a.area += 1.0;
            a.required += share_map.map_or(share, |m| if m.data[i].is_nan() { share } else { m.data[i] });
            if native {
                a.native += 1.0;
            }
            if in_app {
                a.app += 1.0;
                if native {
                    a.app_native += 1.0;
                }
            }
            if native && (count_app || !in_app) {
                a.counted += 1.0;
            }
            if veg08.is_some_and(|m| on(m.data[i])) && (count_app || !in_app) {
                a.counted08 += 1.0;
            }
        }
        for a in by.values_mut() {
            let share_sum = a.required;
            a.required = share_sum * ha / 100.0;
            for v in [&mut a.area, &mut a.native, &mut a.app, &mut a.app_native, &mut a.counted, &mut a.counted08] {
                *v *= ha;
            }
        }
        let mut t = Table::new(
            [
                "property",
                "area_ha",
                "modules",
                "native_ha",
                "app_ha",
                "app_native_ha",
                "share",
                "required_ha",
                "counted_ha",
                "deficit_ha",
                "surplus_ha",
            ]
            .map(String::from)
            .to_vec(),
        );
        let mut balance: BTreeMap<u64, f64> = BTreeMap::new();
        for (id, a) in &by {
            let modules = if module > 0.0 { a.area / module } else { f64::NAN };
            let mut required = a.required;
            // art. 67: up to 4 fiscal modules, the vegetation of 2008
            if veg08.is_some() && modules <= 4.0 {
                required = required.min(a.counted08);
            }
            let (deficit, surplus) = ((required - a.counted).max(0.0), (a.counted - required).max(0.0));
            balance.insert(*id, surplus - deficit);
            t.rows.push(vec![
                f64::from_bits(*id),
                a.area,
                modules,
                a.native,
                a.app,
                a.app_native,
                a.required / a.area * 100.0,
                required,
                a.counted,
                deficit,
                surplus,
            ]);
        }
        let map: Vec<f64> = p.data.iter().map(|v| if on(*v) { balance[&v.to_bits()] } else { f64::NAN }).collect();
        Ok(outputs([("table", Value::Table(t)), ("balance", Value::Map(Map::new(p.grid.clone(), map)))]))
    }
}
