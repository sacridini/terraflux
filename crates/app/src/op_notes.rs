//! What describe_operator tells beyond an operator's ports: how it is used, learnt from the templates (the
//! operators whose outputs it usually reads, those that read its outputs, the templates it is in), the method it
//! is part of and the papers to cite ([`crate::methods`]), and the advice of a modeller for the operators where
//! a choice is easy to get wrong (`terraflux_ops::tips`).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use terraflux_graph::Node;

/// How the templates use an operator.
#[derive(Default)]
struct Use {
    /// The operators whose outputs it reads, and how often.
    reads: BTreeMap<String, usize>,
    /// The operators that read its outputs.
    read_by: BTreeMap<String, usize>,
    templates: Vec<&'static str>,
}

/// The operators' uses in the templates, gathered once.
fn uses() -> &'static BTreeMap<String, Use> {
    static USES: OnceLock<BTreeMap<String, Use>> = OnceLock::new();
    USES.get_or_init(|| {
        let mut out: BTreeMap<String, Use> = BTreeMap::new();
        for t in crate::templates::TEMPLATES {
            let m = t.model();
            let mut nodes = Vec::new();
            every(&m.nodes, &mut nodes);
            let op_of: BTreeMap<&str, &str> = nodes.iter().map(|n| (n.id.as_str(), n.op.as_str())).collect();
            for n in &nodes {
                let u = out.entry(n.op.clone()).or_default();
                if !u.templates.contains(&t.name) {
                    u.templates.push(t.name);
                }
                // its own inputs and settings (not its body's: those are the body's nodes')
                let own = Node { body: None, ..(*n).clone() };
                for r in terraflux_graph::references(&own) {
                    let Some((id, _)) = r.split_once('.') else { continue };
                    let Some(from) = op_of.get(id) else { continue };
                    if *from == n.op {
                        continue;
                    }
                    *out.entry(n.op.clone()).or_default().reads.entry(from.to_string()).or_default() += 1;
                    *out.entry(from.to_string()).or_default().read_by.entry(n.op.clone()).or_default() += 1;
                }
            }
        }
        out
    })
}

fn every<'a>(ns: &'a [Node], out: &mut Vec<&'a Node>) {
    for n in ns {
        out.push(n);
        if let Some(b) = &n.body {
            every(&b.nodes, out);
        }
    }
}

/// The most frequent operators, the plots and neutral landscapes left out (every template draws and plots).
fn most(counts: &BTreeMap<String, usize>) -> Vec<&str> {
    let mut v: Vec<(&str, usize)> = counts
        .iter()
        .filter(|(op, _)| *op != "view.plot" && !op.starts_with("nlm."))
        .map(|(op, n)| (op.as_str(), *n))
        .collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    v.into_iter().take(6).map(|(op, _)| op).collect()
}

/// What describe_operator adds of an operator (empty: nothing to add): its advice (the signature's, given by the
/// operator or by `terraflux_ops::tips`), the method, how the templates use it.
pub fn notes(op: &str, advice: &str) -> String {
    let mut s = String::new();
    if !advice.is_empty() {
        s.push_str(&format!("\nAdvice: {advice}\n"));
    }
    if let Some(m) = crate::methods::method_of(op) {
        let cites: Vec<String> = m
            .refs
            .iter()
            .chain(m.software_refs.iter())
            .filter_map(|k| crate::methods::REFERENCES.iter().find(|r| r.key == *k))
            .map(|r| r.cite())
            .collect();
        s.push_str(&format!("\nMethod: {}", m.name));
        if !m.software.is_empty() {
            s.push_str(&format!(" (as {})", m.software));
        }
        if !cites.is_empty() {
            s.push_str(&format!("; cite {}", cites.join("; ")));
        }
        s.push_str(" (methods drafts the paper's section).\n");
    }
    if let Some(u) = uses().get(op) {
        let (reads, read_by) = (most(&u.reads), most(&u.read_by));
        if !reads.is_empty() {
            s.push_str(&format!("\nIn the templates it reads the outputs of: {}.\n", reads.join(", ")));
        }
        if !read_by.is_empty() {
            s.push_str(&format!("Its outputs are read by: {}.\n", read_by.join(", ")));
        }
        let shown: Vec<String> = u.templates.iter().take(5).map(|t| format!("{t:?}")).collect();
        s.push_str(&format!(
            "Templates with it: {}{}.\n",
            shown.join(", "),
            if u.templates.len() > 5 { format!(" and {} more", u.templates.len() - 5) } else { String::new() }
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn learnt_from_the_templates() {
        let r = terraflux_ops::Registry::standard();
        let notes = |op: &str| super::notes(op, r.sig(op).map_or("", |s| s.advice.as_str()));
        let n = notes("woe.fit");
        assert!(n.contains("Advice: Correlated drivers"), "{n}");
        assert!(n.contains("Its outputs are read by:") && n.contains("woe.probability"), "{n}");
        assert!(n.contains("\"Dinamica EGO\""), "{n}");
        let n = notes("landscape.mspa");
        assert!(n.contains("Method: morphological spatial pattern analysis"), "{n}");
        assert!(n.contains("cite Soille"), "{n}");
        assert!(notes("no.such").is_empty());
    }
}
