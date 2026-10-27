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

    // inside a loop, a group runs whole at each step (nothing of its body kept)
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
    assert_eq!(cache.lock().unwrap().inner_nodes().count(), 0);
}
