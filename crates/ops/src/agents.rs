//! Agents: populations of decision makers whose state changes step by step alongside the map, and that act on
//! it. A population is a table of attributes (a column per attribute, a row per agent): macro agents (countries,
//! markets) that give the demand, parcel managers that change the potential or the rules of their parcels.
//!
//! - `agents.table`: agents from a table written in the model (macro agents).
//! - `agents.parcels`: an agent per parcel of a map of parcel ids, with the parcel's size and the mean of maps
//!   over it.
//! - `agents.update`: the agents' behaviour: rules `attribute = expression`, in order, over their attributes
//!   and numbers (map algebra's expressions, without neighbourhoods).
//! - `agents.map`: an attribute painted on the agents' parcels (a potential, a restriction, a multiplier).
//! - `agents.summary`: the count, sums and means of the attributes (to collect along a loop).

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::algebra::{Expr, Scope, parse};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

/// A population: its attributes by name, a value per agent each (NaN: none).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Agents {
    pub columns: Vec<String>,
    /// the values of each column
    pub values: Vec<Vec<f64>>,
}

impl Agents {
    pub fn len(&self) -> usize {
        self.values.first().map_or(0, Vec::len)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn column(&self, name: &str) -> Option<&Vec<f64>> {
        self.columns.iter().position(|c| c == name).map(|i| &self.values[i])
    }

    /// Set a column (added when new).
    pub fn set(&mut self, name: &str, v: Vec<f64>) {
        match self.columns.iter().position(|c| c == name) {
            Some(i) => self.values[i] = v,
            None => {
                self.columns.push(name.to_string());
                self.values.push(v);
            }
        }
    }

    /// The agents as a table (a row per agent).
    pub fn to_table(&self) -> Table {
        let mut t = Table::new(self.columns.clone());
        for i in 0..self.len() {
            t.rows.push(self.values.iter().map(|c| c[i]).collect());
        }
        t
    }

    pub fn from_table(t: &Table) -> Agents {
        Agents {
            columns: t.columns.clone(),
            values: (0..t.columns.len()).map(|j| t.rows.iter().map(|r| r[j]).collect()).collect(),
        }
    }
}

fn agents_of<'a>(args: &'a Args, port: &str) -> Result<&'a Arc<Agents>> {
    match args.get(port) {
        Some(Value::Agents(a)) => Ok(a),
        Some(v) => Err(Error::msg(format!("node {:?}: {port} is a {:?}, not agents", args.node, v.ty()))),
        None => Err(Error::msg(format!("node {:?}: no input {port:?}", args.node))),
    }
}

/// `agents.table`.
pub struct FromTable;

impl Operator for FromTable {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "agents.table",
            "Agents from a table written in the model (macro agents: countries, markets): `agents` is a list of \
             objects of numbers, one per agent, or the input `table`.",
            Access::None,
        )
        .input(PortSpec::optional("table", Type::Table, "a table, a row per agent"))
        .param(PortSpec::optional("agents", Type::Any, "[{\"id\": 1, \"population\": 1e6, ...}, ...]"))
        .output(PortSpec::new("agents", Type::Agents, "the agents"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let a = if let Some(Value::Table(t)) = args.get("table") {
            Agents::from_table(t)
        } else {
            let rows: Vec<BTreeMap<String, f64>> = args
                .opt_param("agents")?
                .ok_or_else(|| Error::msg(format!("node {:?}: give the agents or a table", args.node)))?;
            let mut columns: Vec<String> = Vec::new();
            for r in &rows {
                for k in r.keys() {
                    if !columns.contains(k) {
                        columns.push(k.clone());
                    }
                }
            }
            let values =
                columns.iter().map(|c| rows.iter().map(|r| r.get(c).copied().unwrap_or(f64::NAN)).collect()).collect();
            Agents { columns, values }
        };
        Ok(outputs([("agents", Value::Agents(Arc::new(a)))]))
    }
}

/// `agents.parcels`.
pub struct Parcels;

impl Operator for Parcels {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "agents.parcels",
            "An agent per parcel of a map of parcel ids (its cells sharing an id): the columns `id` and `cells` \
             (its size), and for every other map input (any name) the mean of the map over the parcel \
             (`name`). Parcels in increasing order of id.",
            Access::Global,
        )
        .input(PortSpec::new("parcels", Type::Map, "the parcel id of each cell (missing: no parcel)"))
        .extra(Type::Map)
        .together()
        .output(PortSpec::new("agents", Type::Agents, "an agent per parcel"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let parcels = args.map("parcels")?;
        let ids = parcels.classes();
        let index: BTreeMap<u64, usize> = ids.iter().enumerate().map(|(i, v)| (v.to_bits(), i)).collect();
        let n = ids.len();
        let mut cells = vec![0.0; n];
        for v in parcels.data.iter() {
            if let Some(&i) = index.get(&v.to_bits()) {
                cells[i] += 1.0;
            }
        }
        let mut a = Agents::default();
        a.set("id", ids.clone());
        a.set("cells", cells);
        for (name, v) in &args.inputs {
            if name == "parcels" {
                continue;
            }
            let Value::Map(m) = v else {
                return Err(Error::msg(format!("node {:?}: {name} is not a map", args.node)));
            };
            if m.data.len() != parcels.data.len() {
                return Err(Error::msg(format!("node {:?}: {name} is not on the grid of the parcels", args.node)));
            }
            let (mut sum, mut count) = (vec![0.0; n], vec![0.0; n]);
            for (p, x) in parcels.data.iter().zip(m.data.iter()) {
                if let Some(&i) = index.get(&p.to_bits())
                    && !x.is_nan()
                {
                    sum[i] += x;
                    count[i] += 1.0;
                }
            }
            a.set(name, sum.iter().zip(&count).map(|(s, c)| if *c > 0.0 { s / c } else { f64::NAN }).collect());
        }
        Ok(outputs([("agents", Value::Agents(Arc::new(a)))]))
    }
}

/// The rules of `agents.update`: `name = expression` per line (or `;`), in order.
pub fn parse_rules(text: &str) -> Result<Vec<(String, Expr)>> {
    text.split(['\n', ';'])
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|line| {
            let (name, expr) = line
                .split_once('=')
                .filter(|(_, e)| !e.starts_with('='))
                .ok_or_else(|| Error::msg(format!("{line:?}: give attribute = expression")))?;
            let name = name.trim();
            if !terraflux_graph::valid_id(name) || name.contains('-') {
                return Err(Error::msg(format!("{name:?} is not an attribute name")));
            }
            let e = parse(expr).map_err(|e| Error::msg(format!("{line:?}: {}", e.0)))?;
            Ok((name.to_string(), e))
        })
        .collect()
}

/// `agents.update`.
pub struct Update;

impl Operator for Update {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "agents.update",
            "The agents' behaviour at a step: `rules`, lines `attribute = expression` applied in order (each line \
             sees the lines before), over the agents' attributes and the node's number inputs (any name: the step, \
             a price...): map algebra's expressions, `if then else`, functions; an attribute may be new.",
            Access::None,
        )
        .input(PortSpec::new("agents", Type::Agents, "the agents"))
        .extra(Type::Any)
        .param(PortSpec::new("rules", Type::Text, "e.g. wealth = wealth + income - costs; farmer = wealth > 100"))
        .output(PortSpec::new("agents", Type::Agents, "the agents after the rules"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let at = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let mut a = (**agents_of(args, "agents")?).clone();
        let text: String = args.opt_param("rules")?.ok_or_else(|| at("no rules".into()))?;
        let rules = parse_rules(&text).map_err(|e| at(e.0))?;
        let mut numbers: BTreeMap<String, f64> = BTreeMap::new();
        for (k, v) in &args.inputs {
            if k != "agents" {
                numbers.insert(k.clone(), v.as_number().ok_or_else(|| at(format!("{k} is not a number")))?);
            }
        }
        let n = a.len();
        for (name, e) in rules {
            if e.radius() > 0 {
                return Err(at(format!("{name}: agents have no neighbourhoods")));
            }
            let scope = Scope {
                width: n,
                height: 1,
                maps: a.columns.iter().map(String::as_str).zip(a.values.iter().map(Vec::as_slice)).collect(),
                numbers: numbers.iter().map(|(k, v)| (k.as_str(), *v)).collect(),
                stacks: BTreeMap::new(),
            };
            let v = e.evaluate(&scope).map_err(|x| at(format!("{name}: {}", x.0)))?;
            a.set(&name, v);
        }
        Ok(outputs([("agents", Value::Agents(Arc::new(a)))]))
    }
}

/// `agents.map`.
pub struct ToMap;

impl Operator for ToMap {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "agents.map",
            "An attribute of the agents painted on their parcels: each cell takes the value of the agent whose \
             `id` is the cell's parcel (missing where none).",
            Access::Local,
        )
        .input(PortSpec::new("agents", Type::Agents, "agents with an id"))
        .input(PortSpec::new("parcels", Type::Map, "the parcel id of each cell"))
        .param(PortSpec::new("column", Type::Text, "the attribute"))
        .output(PortSpec::new("map", Type::Map, "the attribute of each cell's agent"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let at = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let a = agents_of(args, "agents")?;
        let parcels = args.map("parcels")?;
        let column: String = args.opt_param("column")?.ok_or_else(|| at("no column".into()))?;
        let ids = a.column("id").ok_or_else(|| at("the agents have no id".into()))?;
        let v = a.column(&column).ok_or_else(|| at(format!("the agents have no {column:?}")))?;
        let by_id: BTreeMap<u64, f64> = ids.iter().zip(v).map(|(i, x)| (i.to_bits(), *x)).collect();
        let data = parcels.data.iter().map(|p| by_id.get(&p.to_bits()).copied().unwrap_or(f64::NAN)).collect();
        Ok(outputs([("map", Value::Map(Map::new(parcels.grid.clone(), data)))]))
    }
}

/// `agents.summary`.
pub struct Summary;

impl Operator for Summary {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "agents.summary",
            "The number of agents and the sum and mean of each attribute (a row), to collect along a loop.",
            Access::None,
        )
        .input(PortSpec::new("agents", Type::Agents, "the agents"))
        .output(PortSpec::new("table", Type::Table, "n, then <attribute>_sum and <attribute>_mean"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let a = agents_of(args, "agents")?;
        let mut cols = vec!["n".to_string()];
        let mut row = vec![a.len() as f64];
        for (c, v) in a.columns.iter().zip(&a.values) {
            let ok: Vec<f64> = v.iter().copied().filter(|x| !x.is_nan()).collect();
            let sum: f64 = ok.iter().sum();
            cols.push(format!("{c}_sum"));
            row.push(sum);
            cols.push(format!("{c}_mean"));
            row.push(if ok.is_empty() { f64::NAN } else { sum / ok.len() as f64 });
        }
        let mut t = Table::new(cols);
        t.rows.push(row);
        Ok(outputs([("table", Value::Table(t))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(FromTable));
    r.add(Arc::new(Parcels));
    r.add(Arc::new(Update));
    r.add(Arc::new(ToMap));
    r.add(Arc::new(Summary));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(op: &dyn Operator, inputs: BTreeMap<String, Value>, params: serde_json::Value) -> Result<Outputs> {
        let p = params.as_object().unwrap().clone();
        op.run(&mut Ctx::default(), &Args { node: "n", inputs, params: &p })
    }

    fn agents(o: &Outputs) -> Arc<Agents> {
        match &o["agents"] {
            Value::Agents(a) => a.clone(),
            _ => panic!(),
        }
    }

    #[test]
    fn parcels_update_and_paint() {
        // 2 x 3: parcels 1 and 2, a cell without one; land values to average
        let g = Arc::new(terraflux_raster::Grid::new(3, 2));
        let parcels = Map::new(g.clone(), vec![1.0, 1.0, 2.0, 2.0, 2.0, f64::NAN]);
        let land = Map::new(g, vec![10.0, 20.0, 1.0, 2.0, f64::NAN, 5.0]);
        let o = run(
            &Parcels,
            [("parcels".to_string(), Value::Map(parcels.clone())), ("land".to_string(), Value::Map(land))].into(),
            serde_json::json!({}),
        )
        .unwrap();
        let a = agents(&o);
        assert_eq!(a.column("id").unwrap(), &vec![1.0, 2.0]);
        assert_eq!(a.column("cells").unwrap(), &vec![2.0, 3.0]);
        assert_eq!(a.column("land").unwrap(), &vec![15.0, 1.5]);
        // behaviour: wealth grows with the land's value and the step; who is rich
        let o = run(
            &Update,
            [("agents".to_string(), Value::Agents(a)), ("step".to_string(), Value::Integer(2))].into(),
            serde_json::json!({"rules": "wealth = land * cells + step\nrich = 1 if wealth > 10 else 0"}),
        )
        .unwrap();
        let a = agents(&o);
        assert_eq!(a.column("wealth").unwrap(), &vec![32.0, 6.5]);
        assert_eq!(a.column("rich").unwrap(), &vec![1.0, 0.0]);
        // painted back on the parcels
        let o = run(
            &ToMap,
            [("agents".to_string(), Value::Agents(a.clone())), ("parcels".to_string(), Value::Map(parcels))].into(),
            serde_json::json!({"column": "rich"}),
        )
        .unwrap();
        let Value::Map(m) = &o["map"] else { panic!() };
        assert_eq!(m.data[..5], [1.0, 1.0, 0.0, 0.0, 0.0]);
        assert!(m.data[5].is_nan());
        let o = run(&Summary, [("agents".to_string(), Value::Agents(a))].into(), serde_json::json!({})).unwrap();
        let Value::Table(t) = &o["table"] else { panic!() };
        assert_eq!(t.column("n").unwrap(), vec![2.0]);
        assert_eq!(t.column("wealth_sum").unwrap(), vec![38.5]);
    }

    #[test]
    fn macro_agents_and_bad_rules() {
        let o = run(
            &FromTable,
            BTreeMap::new(),
            serde_json::json!({"agents": [{"id": 1, "population": 100}, {"id": 2, "population": 50, "growth": 0.5}]}),
        )
        .unwrap();
        let a = agents(&o);
        assert_eq!(a.len(), 2);
        assert!(a.column("growth").unwrap()[0].is_nan());
        let o = run(
            &Update,
            [("agents".to_string(), Value::Agents(a.clone()))].into(),
            serde_json::json!({"rules": "population = population * (1 + coalesce(growth, 0))"}),
        )
        .unwrap();
        assert_eq!(agents(&o).column("population").unwrap(), &vec![100.0, 75.0]);
        for bad in ["population", "1x = 2", "x = nbsum(population, 1)", "x = nothing + 1", "x == 1"] {
            let r = run(
                &Update,
                [("agents".to_string(), Value::Agents(a.clone()))].into(),
                serde_json::json!({"rules": bad}),
            );
            assert!(r.is_err(), "{bad}");
        }
    }
}
