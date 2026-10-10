//! Agents and cellular automata in one model: macro agents (a population that grows) give the demand for food
//! and timber; parcel managers decide subsidies that raise the fertility of their parcels; CRAFTY's agent
//! functional types compete for the cells to meet the demand.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_exec::{Engine, Quiet};
use terraflux_graph::{Body, Input, Model, Node};
use terraflux_ops::{Key, Map, Stack, Value};
use terraflux_raster::Grid;

const W: usize = 40;

fn inputs() -> BTreeMap<String, Value> {
    let g = Arc::new(Grid::new(W, W));
    let (mut fert, mut wood, mut parcels) = (Vec::new(), Vec::new(), Vec::new());
    for y in 0..W {
        for x in 0..W {
            fert.push((x as f64 + 0.5) / W as f64);
            wood.push(1.0 - (y as f64 + 0.5) / W as f64);
            // four parcels: the quadrants
            parcels.push(1.0 + (x >= W / 2) as u8 as f64 + 2.0 * (y >= W / 2) as u8 as f64);
        }
    }
    [
        ("land".to_string(), Value::Map(Map::new(g.clone(), vec![0.0; W * W]))),
        ("parcels".to_string(), Value::Map(Map::new(g.clone(), parcels))),
        (
            "capitals".to_string(),
            Value::Stack(Stack {
                grid: g,
                keys: vec![Key::Name("fertility".into()), Key::Name("wood".into())],
                layers: vec![Arc::new(fert), Arc::new(wood)],
            }),
        ),
        ("seed".to_string(), Value::Integer(3)),
    ]
    .into()
}

fn model(subsidy: &str) -> Model {
    let mut m = Model::new("agents and land");
    m.inputs.insert("land".into(), Input::Map { path: "land.tif".into(), band: None, nodata: None, note: None });
    m.inputs.insert("parcels".into(), Input::Map { path: "parcels.tif".into(), band: None, nodata: None, note: None });
    m.inputs
        .insert("capitals".into(), Input::Stack { path: "capitals.tif".into(), bands: None, nodata: None, note: None });
    m.inputs.insert("seed".into(), Input::Value { value: 3.into(), note: None });
    m.nodes.push(Node::new("people", "agents.table").param(
        "agents",
        serde_json::json!([{"id": 1, "population": 1000, "food_per_head": 0.25, "timber_per_head": 0.15}]),
    ));
    m.nodes.push(Node::new("managers", "agents.parcels").input("parcels", "$parcels"));
    let afts = serde_json::json!([
        {"name": "farmer", "class": 2, "production": {"food": 1.0}, "sensitivity": {"fertility": 2.0}, "giving_up": 0.05, "giving_in": 0.1},
        {"name": "forester", "class": 3, "production": {"timber": 1.0}, "sensitivity": {"wood": 1.0}, "giving_up": 0.05, "giving_in": 0.1}
    ]);
    let body = Body {
        nodes: vec![
            Node::new("grow", "agents.update").input("agents", "@people").param(
                "rules",
                "population = population * 1.02\nfood = population * food_per_head\ntimber = population * timber_per_head",
            ),
            Node::new("need", "agents.summary").input("agents", "grow.agents"),
            Node::new("decide", "agents.update").input("agents", "managers.agents").input("step", "@step").param("rules", subsidy),
            Node::new("boost", "agents.map").input("agents", "decide.agents").input("parcels", "$parcels").param("column", "subsidy"),
            Node::new("fert", "map.calc")
                .input("caps", "$capitals")
                .input("boost", "boost.map")
                .param("expression", "min(caps.fertility * boost, 1)"),
            Node::new("caps", "stack.set").input("stack", "$capitals").input("map", "fert.map").param("name", "fertility"),
            Node::new("alloc", "crafty.allocate")
                .input("land", "@land")
                .input("capitals", "caps.stack")
                .input("demand", "need.table")
                .input("step", "@step")
                .input("seed", "$seed")
                .param("afts", afts)
                .param("services", serde_json::json!({"food": "food_sum", "timber": "timber_sum"}))
                .param("search", 0.2)
                .param("giving_up_rate", 0.2),
        ],
        carry: [("land".to_string(), "alloc.land".to_string()), ("people".to_string(), "grow.agents".to_string())].into(),
        collect: [("cover".to_string(), "alloc.cover".to_string()), ("info".to_string(), "alloc.info".to_string())].into(),
        ..Default::default()
    };
    m.nodes.push(
        Node::new("simulation", "control.repeat")
            .literal("times", 30)
            .input("land", "$land")
            .input("people", "people.agents")
            .body(body),
    );
    m
}

fn farmers_in_parcel_one(r: &terraflux_exec::Results) -> usize {
    let Some(Value::Map(land)) = r.get("simulation.land") else { panic!() };
    (0..W * W).filter(|&i| land.data[i] == 1.0 && i % W < W / 2 && i / W < W / 2).count()
}

#[test]
fn agents_drive_the_land() {
    let inputs = inputs();
    let with =
        Engine::default().run(&model("subsidy = if id == 1 and step > 5 then 3 else 1"), &inputs, &mut Quiet).unwrap();
    // the population grew, and with it the demand; the supply follows the demand
    let Some(Value::Agents(people)) = with.get("simulation.people") else { panic!() };
    let pop = people.column("population").unwrap()[0];
    assert!((pop - 1000.0 * 1.02f64.powi(30)).abs() < 1e-6, "{pop}");
    let Some(Value::Series(info)) = with.get("simulation.info") else { panic!() };
    let Value::Table(last) = info.last().unwrap() else { panic!() };
    for row in &last.rows {
        let (demand, supply) = (row[1], row[3]);
        assert!((supply - demand).abs() / demand < 0.15, "demand {demand}, supply {supply}");
    }
    // the land cover of the AFTs: farmers 2, foresters 3
    let Some(Value::Series(cover)) = with.get("simulation.cover") else { panic!() };
    let Value::Map(c) = cover.last().unwrap() else { panic!() };
    assert!(c.data.contains(&2.0) && c.data.contains(&3.0));
    // the subsidy of the first parcel's manager draws farmers there
    let without = Engine::default().run(&model("subsidy = 1"), &inputs, &mut Quiet).unwrap();
    let (a, b) = (farmers_in_parcel_one(&with), farmers_in_parcel_one(&without));
    assert!(a > b, "farmers in the subsidised parcel: {a} with the subsidy, {b} without");
}
