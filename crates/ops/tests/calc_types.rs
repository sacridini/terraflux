//! The outputs of a calculator's script (`out`) are of the type of their value, known before the model runs: a
//! number, a map, a stack, rasters read (a map or a stack, by their bands). The graph's checker refuses one where
//! it does not fit (a number of all the cells where a map is taken, a map where a number is).

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, Model, Node, OpSignature, PortSpec, Severity, Type};
use terraflux_ops::{Args, Ctx, Operator, Outputs, Registry, algebra::out_types};

/// `test.takes`: an input of each type, none required; it runs nothing.
struct Takes;

impl Operator for Takes {
    fn signature(&self) -> OpSignature {
        OpSignature::new("test.takes", "", Access::Local)
            .input(PortSpec::optional("number", Type::Number, ""))
            .input(PortSpec::optional("map", Type::Map, ""))
            .input(PortSpec::optional("stack", Type::Stack, ""))
    }

    fn run(&self, _ctx: &mut Ctx, _a: &Args) -> terraflux_ops::Result<Outputs> {
        Ok(Outputs::new())
    }
}

fn types(script: &str, inputs: &[(&str, Type)]) -> Vec<(String, Type)> {
    let inputs: BTreeMap<String, Type> = inputs.iter().map(|(n, t)| (n.to_string(), *t)).collect();
    out_types(script, &inputs).unwrap()
}

#[test]
fn each_output_has_the_type_of_its_value() {
    let t = types(
        "forest = land == 3\n\
         out steep = forest and slope > 20\n\
         out share = forest.mean()\n\
         out twice = k * 2\n\
         out per_year = years * 2\n\
         out spread = sd(years)\n\
         out smooth = nbmean(years, 1)\n\
         out read = dem - 100\n\
         out low = mean(dem)\n\
         out first = years.y2010\n\
         out unknown = series + 1",
        &[
            ("land", Type::Map),
            ("slope", Type::Map),
            ("k", Type::Integer),
            ("years", Type::Stack),
            ("dem", Type::Raster),
            ("series", Type::Series),
        ],
    );
    let want = [
        ("steep", Type::Map),
        ("share", Type::Number),
        ("twice", Type::Number),
        ("per_year", Type::Stack),
        ("spread", Type::Map),
        ("smooth", Type::Stack),
        ("read", Type::Raster),
        ("low", Type::Map),
        ("first", Type::Map),
        ("unknown", Type::Any),
    ];
    assert_eq!(t, want.iter().map(|(n, t)| (n.to_string(), *t)).collect::<Vec<_>>());
}

#[test]
fn a_value_given_where_a_condition_holds_takes_its_cells() {
    // a reclassification: a number, then a map where the condition is one
    let t = types(
        "classes = 1\nclasses[elev >= 700] = 2\nout classes\nout n = 1\nn[k > 2] = 3",
        &[("elev", Type::Map), ("k", Type::Number)],
    );
    assert_eq!(t, vec![("classes".to_string(), Type::Map), ("n".to_string(), Type::Number)]);
    // a stack's layer given makes a stack; a script without out, none (its value is the operator's map)
    assert_eq!(types("s.a = elev\nout s", &[("elev", Type::Map)]), vec![("s".to_string(), Type::Stack)]);
    assert!(out_types("elev * 2", &BTreeMap::new()).is_none());
    assert!(out_types("out x = (", &BTreeMap::new()).is_none());
}

fn errors(m: &Model) -> Vec<String> {
    let mut r = Registry::standard();
    r.add(Arc::new(Takes));
    terraflux_graph::check(m, &r).into_iter().filter(|i| i.severity == Severity::Error).map(|i| i.to_string()).collect()
}

fn model(calc: &str, takes: &[(&str, &str)]) -> Model {
    let mut m = Model::new("t");
    m.nodes.push(
        Node::new("land", "nlm.random_cluster")
            .param("rows", 10)
            .param("columns", 10)
            .param("p", 0.5)
            .param("weights", serde_json::json!([0.5, 0.5])),
    );
    m.nodes.push(Node::new("calc", "raster.calc").input("land", "land.map").param("expression", calc));
    let mut t = Node::new("use", "test.takes");
    for (port, from) in takes {
        t = t.input(port, from);
    }
    m.nodes.push(t);
    m
}

#[test]
fn the_checker_refuses_an_output_where_its_type_does_not_fit() {
    let script = "forest = land == 1\nout forest\nout share = forest.mean()";
    // each where it fits
    assert_eq!(errors(&model(script, &[("map", "calc.forest"), ("number", "calc.share")])), Vec::<String>::new());
    // a number of all the cells where a map is taken, a map where a number is
    let e = errors(&model(script, &[("map", "calc.share")]));
    assert!(e.len() == 1 && e[0].contains("use.map"), "{e:?}");
    let e = errors(&model(script, &[("number", "calc.forest")]));
    assert!(e.len() == 1 && e[0].contains("use.number"), "{e:?}");
    // a map is not a stack; what is not known is let through (the operator checks it when it runs)
    assert_eq!(errors(&model(script, &[("stack", "calc.forest")])).len(), 1);
    assert!(errors(&model("out x = nothing + 1", &[("stack", "calc.x")])).iter().all(|e| !e.contains("use.stack")));
    // the types told (the assistants' replies): the node's outputs, from what its inputs are given
    let r = Registry::standard();
    let m = model(script, &[]);
    assert_eq!(
        terraflux_graph::script_types(&m, &r, &[], "calc"),
        Some(vec![("forest".to_string(), Type::Map), ("share".to_string(), Type::Number)])
    );
    assert_eq!(terraflux_graph::script_types(&m, &r, &[], "land"), None);
}
