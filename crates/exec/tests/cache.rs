//! Runs that keep the outputs of the nodes: a node runs again only when it or what it reads changed, a run
//! stops at a node, loops give back what they collected.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use terraflux_exec::{Cache, Engine, Observer};
use terraflux_graph::{Access, Body, Model, Node, OpSignature, PortSpec, Type};
use terraflux_ops::{Args, Ctx, Operator, Outputs, Registry, Result, Value};

/// `test.inc`: x + 1, counting its runs.
struct Inc(Arc<AtomicUsize>);

impl Operator for Inc {
    fn signature(&self) -> OpSignature {
        OpSignature::new("test.inc", "", Access::None)
            .input(PortSpec::new("x", Type::Number, ""))
            .output(PortSpec::new("y", Type::Number, ""))
            .param(PortSpec::optional("by", Type::Number, ""))
    }
    fn run(&self, _ctx: &mut Ctx, a: &Args) -> Result<Outputs> {
        self.0.fetch_add(1, Ordering::SeqCst);
        let by: f64 = a.param("by", 1.0)?;
        Ok([("y".to_string(), Value::Number(a.number("x")? + by))].into())
    }
}

/// `test.two`: whether x is 2 or more.
struct Two;

impl Operator for Two {
    fn signature(&self) -> OpSignature {
        OpSignature::new("test.two", "", Access::None)
            .input(PortSpec::new("x", Type::Number, ""))
            .output(PortSpec::new("yes", Type::Bool, ""))
    }
    fn run(&self, _ctx: &mut Ctx, a: &Args) -> Result<Outputs> {
        Ok([("yes".to_string(), Value::Bool(a.number("x")? >= 2.0))].into())
    }
}

#[derive(Default)]
struct Seen {
    done: Vec<(String, bool)>,
    collected: usize,
}

impl Observer for Seen {
    fn done(&mut self, at: &str, cached: bool) {
        self.done.push((at.to_string(), cached));
    }
    fn collected(&mut self, _at: &str, _step: usize, _of: usize, _v: &BTreeMap<String, Value>) {
        self.collected += 1;
    }
}

/// The steps and collected values a run reports, in order.
#[derive(Default)]
struct Events(Vec<String>);

impl Observer for Events {
    fn step(&mut self, at: &str, step: usize, of: usize) {
        self.0.push(format!("step {at} {step}/{of}"));
    }
    fn collected(&mut self, at: &str, step: usize, of: usize, v: &BTreeMap<String, Value>) {
        self.0.push(format!("collected {at} {step}/{of} {v:?}"));
    }
}

fn engine(runs: &Arc<AtomicUsize>) -> Engine {
    let mut r = Registry::standard();
    r.add(Arc::new(Inc(runs.clone())));
    r.add(Arc::new(Two));
    Engine { registry: r }
}

/// The places of the nodes run (not taken from the cache), in order.
#[derive(Default)]
struct Ran(Vec<String>);

impl Observer for Ran {
    fn timed(&mut self, at: &str, _seconds: f64, _bytes: usize) {
        self.0.push(at.to_string());
    }
}

/// A repeat of `times` steps: `step` adds 1 to the carried `v` (first 0), `konst` reads the input `$k`.
fn counting(times: i64) -> Model {
    let mut m = Model::new("t");
    m.inputs.insert("k".into(), terraflux_graph::Input::Value { value: 1.into(), note: None });
    m.nodes.push(Node::new("loop", "control.repeat").literal("times", times).literal("v", 0).body(Body {
        nodes: vec![Node::new("step", "test.inc").input("x", "@v"), Node::new("konst", "test.inc").input("x", "$k")],
        carry: [("v".to_string(), "step.y".to_string())].into(),
        collect: [("ys".to_string(), "step.y".to_string())].into(),
        ..Default::default()
    }));
    m
}

#[test]
fn a_loop_runs_again_only_the_steps_that_changed() {
    let runs = Arc::new(AtomicUsize::new(0));
    let engine = engine(&runs);
    let mut m = counting(3);
    let cache = Mutex::new(Cache::default());
    let given = |k: f64| BTreeMap::from([("k".to_string(), Value::Number(k))]);
    let count = || runs.load(Ordering::SeqCst);
    let mut first = Events::default();
    let r = engine.run_with(&m, &given(1.0), &mut first, Some(&cache), None).unwrap();
    assert_eq!((count(), r.get("loop.v").unwrap().as_number()), (6, Some(3.0)));
    assert_eq!(cache.lock().unwrap().iter_entries().0, 6, "2 nodes at each of 3 steps");

    // the loop changed (what it collects), its body not: it runs, its steps do not
    m.nodes[0].body.as_mut().unwrap().collect = [("zs".to_string(), "step.y".to_string())].into();
    let (mut ran, mut ev) = (Ran::default(), Events::default());
    let r = engine.run_with(&m, &given(1.0), &mut ran, Some(&cache), None).unwrap();
    assert_eq!(count(), 6, "no node of the body ran");
    assert_eq!(ran.0, ["loop"]);
    assert!(matches!(r.get("loop.zs"), Some(Value::Series(s)) if s.len() == 3));
    engine.run_with(&m, &given(1.0), &mut ev, Some(&cache), None).unwrap();
    assert_eq!(ev.0.iter().filter(|e| e.starts_with("step ")).count(), 3, "the loop from the cache: its steps");

    // one more step: only its nodes run (`times` is not part of what the steps were made from)
    m.nodes[0].inputs.insert("times".into(), terraflux_graph::Source::Literal(4.into()));
    let mut ran = Ran::default();
    let r = engine.run_with(&m, &given(1.0), &mut ran, Some(&cache), None).unwrap();
    assert_eq!(ran.0, ["loop/step", "loop/konst", "loop"], "(a node is timed when it is done)");
    assert_eq!((count(), r.get("loop.v").unwrap().as_number()), (8, Some(4.0)));

    // an input the body reads changed: that node runs again at every step, the other not
    let mut ran = Ran::default();
    engine.run_with(&m, &given(2.0), &mut ran, Some(&cache), None).unwrap();
    assert_eq!(ran.0.iter().filter(|a| *a == "loop/konst").count(), 4);
    assert!(!ran.0.iter().any(|a| a == "loop/step"));
    assert_eq!(count(), 12);

    // the value carried in changed: the steps that read it, all of them, run again
    m.nodes[0].inputs.insert("v".into(), terraflux_graph::Source::Literal(10.into()));
    let mut ran = Ran::default();
    let r = engine.run_with(&m, &given(2.0), &mut ran, Some(&cache), None).unwrap();
    assert_eq!(ran.0.iter().filter(|a| *a == "loop/step").count(), 4);
    assert!(!ran.0.iter().any(|a| a == "loop/konst"));
    assert_eq!(r.get("loop.v").unwrap().as_number(), Some(14.0));
}

#[test]
fn a_loop_that_stops_at_until_keeps_its_steps() {
    let runs = Arc::new(AtomicUsize::new(0));
    let engine = engine(&runs);
    let mut m = Model::new("t");
    m.nodes.push(Node::new("loop", "control.repeat").literal("times", 5).literal("v", 0).body(Body {
        nodes: vec![Node::new("step", "test.inc").input("x", "@v"), Node::new("stop", "test.two").input("x", "step.y")],
        carry: [("v".to_string(), "step.y".to_string())].into(),
        collect: [("ys".to_string(), "step.y".to_string())].into(),
        until: Some("stop.yes".into()),
        ..Default::default()
    }));
    let none = BTreeMap::new();
    let plain = engine.run(&m, &none, &mut Seen::default()).unwrap();
    let before = runs.load(Ordering::SeqCst);
    assert_eq!(before, 2, "stopped at the second step");
    let cache = Mutex::new(Cache::default());
    let mut first = Events::default();
    let r = engine.run_with(&m, &none, &mut first, Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), before + 2);
    assert_eq!(format!("{:?}", r.get("loop.v")), format!("{:?}", plain.get("loop.v")));
    assert_eq!(format!("{:?}", r.get("loop.ys")), format!("{:?}", plain.get("loop.ys")));
    // the loop changed, its steps not: the same, none of its nodes run, it stops at the same step
    m.nodes[0].body.as_mut().unwrap().collect = [("zs".to_string(), "step.y".to_string())].into();
    let r = engine.run_with(&m, &none, &mut Events::default(), Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), before + 2);
    assert_eq!(format!("{:?}", r.get("loop.v")), format!("{:?}", plain.get("loop.v")));
    assert!(matches!(r.get("loop.zs"), Some(Value::Series(s)) if s.len() == 1), "the step it stopped at not collected");
    // taken whole from the cache: what it told, as it told it (of = times, 1 step collected)
    let mut again = Events::default();
    engine.run_with(&m, &none, &mut again, Some(&cache), None).unwrap();
    let mut last = Events::default();
    engine.run_with(&m, &none, &mut last, Some(&cache), None).unwrap();
    assert_eq!(again.0, last.0);
    assert!(again.0[1].starts_with("step loop 1/5"), "{:?}", again.0);
}

#[test]
fn what_is_not_kept_runs_at_every_step() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("loop-not-kept");
    std::fs::create_dir_all(&dir).unwrap();
    let runs = Arc::new(AtomicUsize::new(0));
    let engine = engine(&runs);
    let mut m = Model::new("t");
    let land = Node::new("land", "nlm.random_cluster")
        .param("rows", 8)
        .param("columns", 8)
        .param("p", 0.5)
        .param("weights", serde_json::json!([0.5, 0.5]));
    let write =
        Node::new("w", "raster.write").input("value", "land.map").param("path", dir.join("s.tif").to_string_lossy());
    let body = Body {
        nodes: vec![
            Node::new("r", "rng.r").literal("seed", 1),
            land,
            write,
            Node::new("step", "test.inc").input("x", "@v"),
        ],
        carry: [("v".to_string(), "step.y".to_string())].into(),
        collect: [("ys".to_string(), "step.y".to_string())].into(),
        ..Default::default()
    };
    m.nodes.push(Node::new("loop", "control.repeat").literal("times", 2).literal("v", 0).body(body));
    let cache = Mutex::new(Cache::default());
    let none = BTreeMap::new();
    engine.run_with(&m, &none, &mut Ran::default(), Some(&cache), None).unwrap();
    m.nodes[0].body.as_mut().unwrap().collect = [("zs".to_string(), "step.y".to_string())].into();
    let mut ran = Ran::default();
    engine.run_with(&m, &none, &mut ran, Some(&cache), None).unwrap();
    let times = |p: &str| ran.0.iter().filter(|a| *a == p).count();
    assert_eq!((times("loop/r"), times("loop/w")), (2, 2), "a generator and a write run at every step");
    assert_eq!((times("loop/land"), times("loop/step")), (0, 0), "the rest kept");
}

#[test]
fn the_nodes_of_loops_kept_are_limited() {
    let runs = Arc::new(AtomicUsize::new(0));
    let engine = engine(&runs);
    let mut m = counting(8);
    let cache = Mutex::new(Cache::default());
    cache.lock().unwrap().limit_iters(6, usize::MAX);
    let given = BTreeMap::from([("k".to_string(), Value::Number(1.0))]);
    let r = engine.run_with(&m, &given, &mut Ran::default(), Some(&cache), None).unwrap();
    assert_eq!(r.get("loop.v").unwrap().as_number(), Some(8.0));
    assert!(cache.lock().unwrap().iter_entries().0 <= 6);
    // what was let go runs again, the result the same
    m.nodes[0].body.as_mut().unwrap().collect = [("zs".to_string(), "step.y".to_string())].into();
    let r = engine.run_with(&m, &given, &mut Ran::default(), Some(&cache), None).unwrap();
    assert_eq!(r.get("loop.v").unwrap().as_number(), Some(8.0));
    cache.lock().unwrap().clear();
    assert_eq!(cache.lock().unwrap().iter_entries(), (0, 0));
}

#[test]
fn a_loop_in_a_loop_runs_its_body_at_every_step() {
    let runs = Arc::new(AtomicUsize::new(0));
    let engine = engine(&runs);
    // 2 outer steps of 3 inner ones: 6 runs of the node, carried from one to the next
    let mut m = Model::new("t");
    m.nodes.push(Node::new("outer", "control.repeat").literal("times", 2).literal("v", 0).body(Body {
        nodes: vec![Node::new("inner", "control.repeat").literal("times", 3).input("v", "@v").body(Body {
            nodes: vec![Node::new("step", "test.inc").input("x", "@v")],
            carry: [("v".to_string(), "step.y".to_string())].into(),
            collect: [("ys".to_string(), "step.y".to_string())].into(),
            ..Default::default()
        })],
        carry: [("v".to_string(), "inner.v".to_string())].into(),
        collect: [("last".to_string(), "inner.v".to_string())].into(),
        ..Default::default()
    }));
    let cache = Mutex::new(Cache::default());
    let none = BTreeMap::new();
    let mut ev = Events::default();
    let r = engine.run_with(&m, &none, &mut ev, Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 6);
    assert_eq!(r.get("outer.v").unwrap().as_number(), Some(6.0));
    let Some(Value::Series(last)) = r.get("outer.last") else { panic!() };
    assert_eq!(last.iter().map(|v| v.as_number().unwrap()).collect::<Vec<_>>(), [3.0, 6.0]);
    // the inner loop's steps are reported at its place
    assert_eq!(ev.0.iter().filter(|e| e.starts_with("step outer ")).count(), 2);
    assert_eq!(ev.0.iter().filter(|e| e.starts_with("step outer/inner ")).count(), 6);
    // nothing changed: the outer loop gives back what it collected, nothing runs
    engine.run_with(&m, &none, &mut Events::default(), Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 6);
}

#[test]
fn an_each_in_a_group_in_a_loop_runs_whole_at_every_step() {
    let runs = Arc::new(AtomicUsize::new(0));
    let engine = engine(&runs);
    // 2 steps, each a group with an each over 3 items: 6 runs; the group gives the last element's value
    let mut m = Model::new("t");
    m.nodes.push(Node::new("loop", "control.repeat").literal("times", 2).literal("v", 0).body(Body {
        nodes: vec![Node::new("g", terraflux_graph::GROUP).input("x", "@v").body(Body {
            nodes: vec![
                Node::new("e", "control.each")
                    .literal("over", serde_json::json!(["a", "b", "c"]))
                    .input("base", "@x")
                    .body(Body {
                        nodes: vec![Node::new("add", "test.inc").input("x", "@base")],
                        collect: [("ys".to_string(), "add.y".to_string())].into(),
                        ..Default::default()
                    }),
                Node::new("next", "test.inc").input("x", "@x"),
            ],
            outputs: [("y".to_string(), "next.y".to_string()), ("ys".to_string(), "e.ys".to_string())].into(),
            ..Default::default()
        })],
        carry: [("v".to_string(), "g.y".to_string())].into(),
        ..Default::default()
    }));
    let cache = Mutex::new(Cache::default());
    let none = BTreeMap::new();
    let mut ev = Events::default();
    let r = engine.run_with(&m, &none, &mut ev, Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 8, "2 steps of 3 elements and the node after");
    assert_eq!(r.get("loop.v").unwrap().as_number(), Some(2.0), "each step: the base plus 1");
    assert_eq!(ev.0.iter().filter(|e| e.starts_with("step loop/g/e ")).count(), 6);
    assert_eq!(cache.lock().unwrap().inner_nodes().count(), 0, "nothing of a loop's body kept");
}

#[test]
fn a_switch_in_a_loop_takes_its_branch_at_every_step() {
    let runs = Arc::new(AtomicUsize::new(0));
    let engine = engine(&runs);
    let branch = |id: &str, by: i64| {
        Node::new(id, terraflux_graph::GROUP).input("x", "@x").body(Body {
            nodes: vec![Node::new("add", "test.inc").input("x", "@x").param("by", by)],
            outputs: [("y".to_string(), "add.y".to_string())].into(),
            ..Default::default()
        })
    };
    // the case is the carried number: 0 takes case_0 (+1), then 1 takes otherwise (+10)
    let mut m = Model::new("t");
    m.nodes.push(Node::new("loop", "control.repeat").literal("times", 2).literal("v", 0).body(Body {
        nodes: vec![
            Node::new("sw", terraflux_graph::SWITCH).input("case", "@v").input("x", "@v").body(Body {
                nodes: vec![branch("case_0", 1), branch("otherwise", 10)],
                ..Default::default()
            }),
        ],
        carry: [("v".to_string(), "sw.y".to_string())].into(),
        collect: [("branch".to_string(), "sw.branch".to_string())].into(),
        ..Default::default()
    }));
    let none = BTreeMap::new();
    let r = engine.run(&m, &none, &mut Seen::default()).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 2, "one branch per step");
    assert_eq!(r.get("loop.v").unwrap().as_number(), Some(11.0));
    let Some(Value::Series(b)) = r.get("loop.branch") else { panic!() };
    assert!(matches!((&b[0], &b[1]), (Value::Text(a), Value::Text(c)) if a == "case_0" && c == "otherwise"));
    // with a cache: the branch taken at each step kept with the step, so the loop run again (what it collects
    // changed) runs no node of any branch; one more step runs only the branch of its case
    let cache = Mutex::new(Cache::default());
    engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 4);
    m.nodes[0].body.as_mut().unwrap().collect = [("which".to_string(), "sw.branch".to_string())].into();
    let mut ran = Ran::default();
    let r = engine.run_with(&m, &none, &mut ran, Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 4, "no node of a branch ran again");
    assert_eq!(ran.0, ["loop"]);
    assert!(matches!(r.get("loop.which"), Some(Value::Series(s)) if s.len() == 2));
    m.nodes[0].inputs.insert("times".into(), terraflux_graph::Source::Literal(3.into()));
    let mut ran = Ran::default();
    let r = engine.run_with(&m, &none, &mut ran, Some(&cache), None).unwrap();
    assert_eq!(r.get("loop.v").unwrap().as_number(), Some(21.0), "the third step: otherwise again (+10)");
    assert_eq!(ran.0, ["loop/sw/otherwise/add", "loop/sw", "loop"], "only the third step's branch");
}

#[test]
fn a_cached_each_in_a_group_gives_its_steps_again() {
    let runs = Arc::new(AtomicUsize::new(0));
    let engine = engine(&runs);
    // g: an each over 3 items, and a node alone that changes between the runs (the group runs again, the each is
    // kept by its place)
    let mut m = Model::new("t");
    m.nodes.push(Node::new("g", terraflux_graph::GROUP).body(Body {
        nodes: vec![
            Node::new("e", "control.each").literal("over", serde_json::json!(["a", "b", "c"])).body(Body {
                nodes: vec![Node::new("add", "test.inc").input("x", "@index")],
                collect: [("ys".to_string(), "add.y".to_string()), ("names".to_string(), "@name".to_string())].into(),
                ..Default::default()
            }),
            Node::new("alone", "test.inc").literal("x", 0),
        ],
        outputs: [("z".to_string(), "alone.y".to_string())].into(),
        ..Default::default()
    }));
    let cache = Mutex::new(Cache::default());
    let none = BTreeMap::new();
    let mut first = Events::default();
    engine.run_with(&m, &none, &mut first, Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 3 + 1);
    assert_eq!(first.0.len(), 6, "{:?}", first.0);
    m.nodes[0].body.as_mut().unwrap().nodes[1].params.insert("by".into(), 5.into());
    let mut again = Events::default();
    let r = engine.run_with(&m, &none, &mut again, Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 3 + 1 + 1, "only the node that changed");
    assert_eq!(r.get("g.z").unwrap().as_number(), Some(5.0));
    assert_eq!(first.0, again.0, "the each's steps and what it collected, from the cache");
}

#[test]
fn only_what_changed_runs_again() {
    let runs = Arc::new(AtomicUsize::new(0));
    let mut r = Registry::standard();
    r.add(Arc::new(Inc(runs.clone())));
    let engine = Engine { registry: r };
    // a -> b -> c, and d alone; a loop of 3 steps on c
    let mut m = Model::new("t");
    m.nodes.push(Node::new("a", "test.inc").literal("x", 0));
    m.nodes.push(Node::new("b", "test.inc").input("x", "a.y"));
    m.nodes.push(Node::new("c", "test.inc").input("x", "b.y"));
    m.nodes.push(Node::new("d", "test.inc").literal("x", 10));
    m.nodes.push(Node::new("loop", "control.repeat").literal("times", 3).input("v", "c.y").body(Body {
        nodes: vec![Node::new("step", "test.inc").input("x", "@v")],
        carry: [("v".to_string(), "step.y".to_string())].into(),
        collect: [("each".to_string(), "step.y".to_string())].into(),
        ..Default::default()
    }));
    let cache = Mutex::new(Cache::default());
    let none = BTreeMap::new();
    let first = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 4 + 3);
    assert_eq!(first.get("loop.v").unwrap().as_number(), Some(6.0));

    // nothing changed: nothing runs, the loop gives back its steps
    let mut seen = Seen::default();
    let again = engine.run_with(&m, &none, &mut seen, Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 7);
    assert!(seen.done.iter().all(|(_, cached)| *cached));
    assert_eq!(seen.collected, 3, "the loop's steps, from the cache");
    assert_eq!(again.get("loop.v").unwrap().as_number(), Some(6.0));

    // moved on the canvas, a note: the same outputs
    m.nodes[1].pos = Some([100.0, 50.0]);
    m.nodes[1].note = Some("why".into());
    engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 7);

    // b's setting changed: b, c and the loop run again, a and d do not
    m.nodes[1].params.insert("by".into(), 5.into());
    let mut seen = Seen::default();
    let r = engine.run_with(&m, &none, &mut seen, Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 7 + 2 + 3);
    let ran: Vec<&str> = seen.done.iter().filter(|(_, c)| !c).map(|(n, _)| n.as_str()).collect();
    assert_eq!(ran, ["b", "c", "loop"]);
    assert_eq!(r.get("loop.v").unwrap().as_number(), Some(10.0));

    // up to c only: a, b, c from the cache, nothing else
    let mut seen = Seen::default();
    let r = engine.run_with(&m, &none, &mut seen, Some(&cache), Some("c")).unwrap();
    assert_eq!(seen.done.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
    assert!(r.get("d.y").is_none());

    // a node removed is forgotten
    m.nodes.retain(|n| n.id != "d");
    engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert!(cache.lock().unwrap().outputs("d").is_none());
    assert!(cache.lock().unwrap().outputs("c").is_some());
}

#[test]
fn a_changed_file_of_a_swat_project_runs_its_node_again() {
    let zip = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extern/landschaft/src/landschaft/hydro/swat/data/hubbard_brook.zip");
    let project = terraflux_ops::swat::Project::load(&zip).unwrap();
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("swat-cache");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in &project.files {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    let mut m = Model::new("t");
    m.nodes.push(Node::new("load", "swat.load").param("path", dir.to_string_lossy().as_ref()));
    let engine = Engine { registry: Registry::standard() };
    let cache = Mutex::new(Cache::default());
    let none = BTreeMap::new();
    let ran = |seen: &Seen| seen.done.iter().any(|(n, cached)| n == "load" && !cached);
    let mut seen = Seen::default();
    engine.run_with(&m, &none, &mut seen, Some(&cache), None).unwrap();
    assert!(ran(&seen), "the first run");
    let mut seen = Seen::default();
    engine.run_with(&m, &none, &mut seen, Some(&cache), None).unwrap();
    assert!(!ran(&seen), "nothing changed: from the cache");
    // a file of the project changed (its size), an output of SWAT+ added (not part of the project)
    std::fs::write(dir.join("simulation.out"), "an output").unwrap();
    let mut seen = Seen::default();
    engine.run_with(&m, &none, &mut seen, Some(&cache), None).unwrap();
    assert!(!ran(&seen), "an output is not the project");
    let cio = dir.join("file.cio");
    let mut text = std::fs::read(&cio).unwrap();
    text.extend_from_slice(b"\n");
    std::fs::write(&cio, text).unwrap();
    let mut seen = Seen::default();
    engine.run_with(&m, &none, &mut seen, Some(&cache), None).unwrap();
    assert!(ran(&seen), "a file changed: runs again");
}

#[test]
fn a_changed_table_runs_its_reader_again() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("table-cache");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let csv = dir.join("t.csv");
    std::fs::write(&csv, "a,b\n1,2\n").unwrap();
    let mut m = Model::new("t");
    m.nodes.push(Node::new("read", "table.read").param("path", csv.to_string_lossy().as_ref()));
    let engine = Engine { registry: Registry::standard() };
    let cache = Mutex::new(Cache::default());
    let none = BTreeMap::new();
    let ran = |seen: &Seen| seen.done.iter().any(|(n, cached)| n == "read" && !cached);
    let mut seen = Seen::default();
    engine.run_with(&m, &none, &mut seen, Some(&cache), None).unwrap();
    assert!(ran(&seen), "the first run");
    let mut seen = Seen::default();
    engine.run_with(&m, &none, &mut seen, Some(&cache), None).unwrap();
    assert!(!ran(&seen), "nothing changed: from the cache");
    std::fs::write(&csv, "a,b\n1,2\n3,4\n").unwrap();
    let mut seen = Seen::default();
    engine.run_with(&m, &none, &mut seen, Some(&cache), None).unwrap();
    assert!(ran(&seen), "the file changed: runs again");
}

#[test]
fn a_target_ignores_the_errors_of_other_nodes() {
    let mut r = Registry::standard();
    r.add(Arc::new(Inc(Arc::new(AtomicUsize::new(0)))));
    let engine = Engine { registry: r };
    let mut m = Model::new("t");
    m.nodes.push(Node::new("a", "test.inc").literal("x", 1));
    m.nodes.push(Node::new("broken", "test.inc"));
    let cache = Mutex::new(Cache::default());
    let none = BTreeMap::new();
    assert!(engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).is_err());
    let r = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), Some("a")).unwrap();
    assert_eq!(r.get("a.y").unwrap().as_number(), Some(2.0));
}

#[test]
fn generators_are_not_kept() {
    // a generator's state moves as it is drawn from: its node and those after it run every time
    let engine = Engine::default();
    let mut m = Model::new("t");
    m.nodes.push(Node::new("rng", "rng.r").literal("seed", 1));
    let cache = Mutex::new(Cache::default());
    let none = BTreeMap::new();
    let mut seen = Seen::default();
    engine.run_with(&m, &none, &mut seen, Some(&cache), None).unwrap();
    engine.run_with(&m, &none, &mut seen, Some(&cache), None).unwrap();
    assert_eq!(seen.done, [("rng".to_string(), false), ("rng".to_string(), false)]);
}

#[test]
fn a_frozen_result_is_used_whatever_it_came_from() {
    let runs = Arc::new(AtomicUsize::new(0));
    let mut r = Registry::standard();
    r.add(Arc::new(Inc(runs.clone())));
    let engine = Engine { registry: r };
    // a -> b -> c, b frozen
    let mut m = Model::new("t");
    m.nodes.push(Node::new("a", "test.inc").literal("x", 0));
    m.nodes.push(Node::new("b", "test.inc").input("x", "a.y"));
    m.nodes.push(Node::new("c", "test.inc").input("x", "b.y"));
    let none = BTreeMap::new();
    // a later session: b's result read from disk and held; a, only read by b, does not run
    let cache = Mutex::new(Cache::default());
    m.nodes[1].frozen = true;
    let held: Outputs = [("y".to_string(), Value::Number(41.0))].into();
    assert!(cache.lock().unwrap().hold("b", Some(held), 7));
    let mut seen = Seen::default();
    let r = engine.run_with(&m, &none, &mut seen, Some(&cache), None).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    assert_eq!(seen.done, [("b".to_string(), true), ("c".to_string(), false)]);
    assert_eq!(r.get("c.y").unwrap().as_number(), Some(42.0));
    // what it reads changed: still the frozen result
    m.nodes[0].params.insert("by".into(), 5.into());
    let r = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), Some("c")).unwrap();
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    assert_eq!(r.get("c.y").unwrap().as_number(), Some(42.0));
    // thawed: b runs again on what a gives now
    m.nodes[1].frozen = false;
    cache.lock().unwrap().release("b");
    let r = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(r.get("c.y").unwrap().as_number(), Some(7.0));
    assert!(!cache.lock().unwrap().holds("b"));
}

#[test]
fn a_group_runs_again_only_the_nodes_of_its_body_that_changed() {
    let runs = Arc::new(AtomicUsize::new(0));
    let mut r = Registry::standard();
    r.add(Arc::new(Inc(runs.clone())));
    let engine = Engine { registry: r };
    // a -> g (first -> second -> third, and inner: one -> two) -> after
    let mut m = Model::new("t");
    m.nodes.push(Node::new("a", "test.inc").literal("x", 0));
    m.nodes.push(Node::new("g", terraflux_graph::GROUP).input("x", "a.y").body(Body {
        nodes: vec![
            Node::new("first", "test.inc").input("x", "@x"),
            Node::new("second", "test.inc").input("x", "first.y"),
            Node::new("third", "test.inc").input("x", "second.y"),
            Node::new("inner", terraflux_graph::GROUP).input("x", "first.y").body(Body {
                nodes: vec![
                    Node::new("one", "test.inc").input("x", "@x"),
                    Node::new("two", "test.inc").input("x", "one.y"),
                ],
                outputs: [("y".to_string(), "two.y".to_string())].into(),
                ..Default::default()
            }),
        ],
        outputs: [("y".to_string(), "third.y".to_string()), ("z".to_string(), "inner.y".to_string())].into(),
        ..Default::default()
    }));
    m.nodes.push(Node::new("after", "test.inc").input("x", "g.y"));
    let cache = Mutex::new(Cache::default());
    let none = BTreeMap::new();
    let count = || runs.load(Ordering::SeqCst);
    let first = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(count(), 7);
    assert_eq!(first.get("after.y").unwrap().as_number(), Some(5.0));
    assert_eq!(first.get("g.z").unwrap().as_number(), Some(4.0));

    // the second node of the group changed: it and the third run again, not the first nor the inner group's
    let body = &mut m.nodes[1].body.as_mut().unwrap().nodes;
    body[1].params.insert("by".into(), 10.into());
    let r = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(count(), 7 + 3, "second, third and after");
    assert_eq!(r.get("after.y").unwrap().as_number(), Some(14.0));
    assert_eq!(r.get("g.z").unwrap().as_number(), Some(4.0));
    assert_eq!(r.values["g"]["inner.two.y"].as_number(), Some(4.0), "the inner group's nodes, as a group gives them");

    // a node of the inner group changed: only it (and what reads the group, its outputs the same) runs again
    let inner = &mut m.nodes[1].body.as_mut().unwrap().nodes[3].body.as_mut().unwrap().nodes;
    inner[1].params.insert("by".into(), 2.into());
    let r = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(count(), 10 + 2, "two and after");
    assert_eq!(r.get("g.z").unwrap().as_number(), Some(5.0));

    // what the group reads changed: its whole body runs again
    m.nodes[0].params.insert("by".into(), 5.into());
    engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(count(), 12 + 7);

    // a node taken out of the group: what was kept of it let go
    m.nodes[1].body.as_mut().unwrap().nodes[3].body.as_mut().unwrap().outputs.clear();
    m.nodes[1].body.as_mut().unwrap().outputs.remove("z");
    m.nodes[1].body.as_mut().unwrap().nodes.remove(3);
    engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    let places: Vec<String> = cache.lock().unwrap().inner_nodes().map(String::from).collect();
    assert_eq!(places, ["g/first", "g/second", "g/third"]);

    // inside a loop, a group's body nodes are kept by iteration (not by place: nothing in `inner_nodes`), so a loop
    // run again with one more step runs only the new step's
    let before = count();
    let mut l = Model::new("l");
    l.nodes.push(Node::new("loop", "control.repeat").literal("times", 2).literal("v", 0).body(Body {
        nodes: vec![Node::new("in", terraflux_graph::GROUP).input("x", "@v").body(Body {
            nodes: vec![Node::new("add", "test.inc").input("x", "@x")],
            outputs: [("y".to_string(), "add.y".to_string())].into(),
            ..Default::default()
        })],
        carry: [("v".to_string(), "in.y".to_string())].into(),
        ..Default::default()
    }));
    let cache = Mutex::new(Cache::default());
    let r = engine.run_with(&l, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(r.get("loop.v").unwrap().as_number(), Some(2.0));
    assert_eq!(count(), before + 2);
    assert_eq!(cache.lock().unwrap().inner_nodes().count(), 0);
    assert_eq!(cache.lock().unwrap().iter_entries().0, 2, "the group's node at each of the 2 steps");
    l.nodes[0].inputs.insert("times".into(), terraflux_graph::Source::Literal(3.into()));
    let r = engine.run_with(&l, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(r.get("loop.v").unwrap().as_number(), Some(3.0));
    assert_eq!(count(), before + 3, "only the third step ran");
}

#[test]
fn a_frozen_node_of_a_group_is_used_whatever_it_came_from() {
    let runs = Arc::new(AtomicUsize::new(0));
    let mut r = Registry::standard();
    r.add(Arc::new(Inc(runs.clone())));
    let engine = Engine { registry: r };
    // a -> g (first -> second -> third), second frozen
    let mut m = Model::new("t");
    m.nodes.push(Node::new("a", "test.inc").literal("x", 0));
    let mut second = Node::new("second", "test.inc").input("x", "first.y");
    second.frozen = true;
    m.nodes.push(Node::new("g", terraflux_graph::GROUP).input("x", "a.y").body(Body {
        nodes: vec![
            Node::new("first", "test.inc").input("x", "@x"),
            second,
            Node::new("third", "test.inc").input("x", "second.y"),
        ],
        outputs: [("y".to_string(), "third.y".to_string())].into(),
        ..Default::default()
    }));
    assert_eq!(terraflux_exec::frozen_places(&m), ["g/second"]);
    // a later session: its result read from disk and held by its place
    let cache = Mutex::new(Cache::default());
    let held: Outputs = [("y".to_string(), Value::Number(41.0))].into();
    assert!(cache.lock().unwrap().hold("g/second", Some(held), 7));
    assert!(cache.lock().unwrap().holds("g/second") && !cache.lock().unwrap().holds("second"));
    let none = BTreeMap::new();
    let r = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(r.get("g.y").unwrap().as_number(), Some(42.0));
    // first, read only by second, does not run (as a node read only by a frozen one at the top level)
    assert_eq!(runs.load(Ordering::SeqCst), 2, "a and third");
    assert!(!r.values["g"].contains_key("first.y"));
    // what it reads changed: the group runs again, still with the frozen result
    m.nodes[0].params.insert("by".into(), 5.into());
    let r = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(r.get("g.y").unwrap().as_number(), Some(42.0));
    assert_eq!(r.values["g"]["second.y"].as_number(), Some(41.0));
    // thawed: it runs again on what first gives now (a 5, first 6, second 7, third 8)
    m.nodes[1].body.as_mut().unwrap().nodes[1].frozen = false;
    cache.lock().unwrap().release("g/second");
    let r = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(r.get("g.y").unwrap().as_number(), Some(8.0));
}

#[test]
fn a_frozen_node_of_a_switch_branch_is_used_whatever_it_came_from() {
    let runs = Arc::new(AtomicUsize::new(0));
    let engine = engine(&runs);
    // a -> sw (branch double: first -> second), second frozen
    let mut second = Node::new("second", "test.inc").input("x", "first.y");
    second.frozen = true;
    let mut m = Model::new("t");
    m.nodes.push(Node::new("a", "test.inc").literal("x", 0));
    m.nodes.push(Node::new("sw", terraflux_graph::SWITCH).literal("case", "double").input("x", "a.y").body(Body {
        nodes: vec![Node::new("double", terraflux_graph::GROUP).input("x", "@x").body(Body {
            nodes: vec![Node::new("first", "test.inc").input("x", "@x"), second],
            outputs: [("y".to_string(), "second.y".to_string())].into(),
            ..Default::default()
        })],
        ..Default::default()
    }));
    assert_eq!(terraflux_exec::frozen_places(&m), ["sw/double/second"]);
    let cache = Mutex::new(Cache::default());
    let held: Outputs = [("y".to_string(), Value::Number(41.0))].into();
    assert!(cache.lock().unwrap().hold("sw/double/second", Some(held), 7));
    let none = BTreeMap::new();
    let r = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(r.get("sw.y").unwrap().as_number(), Some(41.0));
    assert_eq!(runs.load(Ordering::SeqCst), 1, "only a: first, read only by the frozen node, does not run");
    // what it reads changed: still the frozen result
    m.nodes[0].params.insert("by".into(), 5.into());
    let r = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(r.get("sw.y").unwrap().as_number(), Some(41.0));
    // thawed: it runs again on what first gives now (a 5, first 6, second 7)
    m.nodes[1].body.as_mut().unwrap().nodes[0].body.as_mut().unwrap().nodes[1].frozen = false;
    cache.lock().unwrap().release("sw/double/second");
    let r = engine.run_with(&m, &none, &mut Seen::default(), Some(&cache), None).unwrap();
    assert_eq!(r.get("sw.y").unwrap().as_number(), Some(7.0));
}

#[test]
fn a_loop_gives_what_its_body_gave_at_the_last_step() {
    let runs = Arc::new(AtomicUsize::new(0));
    let mut r = Registry::standard();
    r.add(Arc::new(Inc(runs.clone())));
    let engine = Engine { registry: r };
    let mut l = Model::new("l");
    l.nodes.push(Node::new("loop", "control.repeat").literal("times", 3).literal("v", 0).body(Body {
        nodes: vec![
            Node::new("add", "test.inc").input("x", "@v"),
            Node::new("in", terraflux_graph::GROUP).input("x", "add.y").body(Body {
                nodes: vec![Node::new("more", "test.inc").input("x", "@x").param("by", 10)],
                outputs: [("y".to_string(), "more.y".to_string())].into(),
                ..Default::default()
            }),
        ],
        carry: [("v".to_string(), "add.y".to_string())].into(),
        ..Default::default()
    }));
    let none = BTreeMap::new();
    let r = engine.run(&l, &none, &mut Seen::default()).unwrap();
    assert_eq!(r.get("loop.v").unwrap().as_number(), Some(3.0));
    // the body's nodes, as `node.port`, at the last step: add 3, in 13 (a group in it, `in.more.y`)
    assert_eq!(r.values["loop"]["add.y"].as_number(), Some(3.0));
    assert_eq!(r.values["loop"]["in.y"].as_number(), Some(13.0));
    assert_eq!(r.values["loop"]["in.more.y"].as_number(), Some(13.0));
    // stopped by until: the step it stopped at
    l.nodes[0].body.as_mut().unwrap().until = Some("stop.yes".into());
    l.nodes[0].body.as_mut().unwrap().nodes.push(Node::new("stop", "test.two").input("x", "add.y"));
    let mut r = Registry::standard();
    r.add(Arc::new(Inc(runs.clone())));
    r.add(Arc::new(Two));
    let engine = Engine { registry: r };
    let r = engine.run(&l, &none, &mut Seen::default()).unwrap();
    assert_eq!(r.values["loop"]["add.y"].as_number(), Some(2.0));
}
