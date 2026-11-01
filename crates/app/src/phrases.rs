//! What a step does with its settings, in a sentence written by hand for the operators models use most ("A
//! Gaussian random field of 100 × 100 cells: autocorrelation range 10 cells…"), shown in the document and the
//! report with the settings listed.
//!
//! A phrase is a template: `{name}` is a setting's value in words (the one given, else the operator's default),
//! `<port>` where an input comes from (a node's name, a model input's), and `[…]` a part said only when every
//! setting and input it names is given.

use terraflux_graph::{Node, OpSignature};

use crate::document::From;
use crate::methods::Lang;

/// The phrases, by operator.
const PHRASES: &[(&str, &str)] = &[
    ("raster.read", "Reads the rasters of {path}[, band {band}][, with {nodata} as no data]."),
    ("raster.write", "Writes <value> to {path}[ as {type}]."),
    ("vector.read", "Reads the features of {path}[, layer {layer}]."),
    ("vector.write", "Writes the features of <vector> to {path}[, layer {layer}]."),
    ("table.read", "Reads the table {path}."),
    ("move.read_tracks", "Reads the animal tracks of {path}."),
    ("view.plot", "Shows[ <value>][ <features>][ <table>][ over <under>][ as \"{title}\"]."),
    ("nlm.random", "A random map of {rows} × {columns} cells[ of side {resolution}], seed {seed}."),
    (
        "nlm.gaussian_field",
        "A Gaussian random field of {rows} × {columns} cells[ of side {resolution}]: autocorrelation range \
         {range} cells, variance {variance}, nugget {nugget}; seed {seed}.",
    ),
    (
        "nlm.random_cluster",
        "Random clusters on {rows} × {columns} cells[ of side {resolution}], a share {p} of the cells in clusters\
         [ ({neighbourhood} neighbours)][, the classes weighted {weights}]; seed {seed}.",
    ),
    (
        "nlm.fbm",
        "Fractional Brownian motion on {rows} × {columns} cells[ of side {resolution}], fractal dimension \
         {dimension}; seed {seed}.",
    ),
    (
        "nlm.planar_gradient",
        "A planar gradient on {rows} × {columns} cells[ of side {resolution}][, facing {direction}°]; seed {seed}.",
    ),
    (
        "nlm.distance_gradient",
        "A gradient of the distance to a rectangle[ ({origin})] on {rows} × {columns} cells; seed {seed}.",
    ),
    ("nlm.binarize", "<map> split into habitat and matrix, a share {breaks} of it habitat."),
    ("raster.reclassify", "<map> with its values reclassified: {rules}."),
    ("raster.reproject", "<raster> reprojected[ to {crs}][, cells of side {resolution}][, by {resampling}]."),
    ("raster.resample", "<raster> resampled to cells of side {resolution}[, by {resampling}]."),
    ("markov.fit", "The Markov chain of the changes from <t0> to <t1>[, over <steps> steps]."),
    ("glm.fit", "A logistic regression of each class of <landscape> on <drivers>[ (formulas {formulas})]."),
    ("glm.predict", "The probability of each class by the regression <model>, on <drivers>[ and <landscape>]."),
    (
        "woe.fit",
        "The weights of evidence of each change from <t0> to <t1> by <drivers>[, ranges by increments of \
         {increment}][, {categorical} taken as categories].",
    ),
    ("woe.probability", "The probability of each change by the weights <model>, on <landscape> and <drivers>."),
    ("demand.from_maps", "The area of each class over time, from <maps>[ dated {times}][, for {out_times}]."),
    ("dinamica.step", "A step of Dinamica's allocation of the changes <rates> on <landscape>, by <probability>."),
    ("lulcc.step", "A step of the allocation of <demand> on <landscape>, by <suitability>."),
    (
        "patches.observe",
        "The patches of the changes from <t0> to <t1>: how much they expand and their isometry[ ({isometry})].",
    ),
    ("metrics.class", "The metrics of each class of <map>[: {metrics}][, patches of {directions} neighbours]."),
    (
        "metrics.landscape",
        "The metrics of the whole landscape <map>[: {metrics}][, patches of {directions} neighbours].",
    ),
    ("metrics.patch", "The metrics of each patch of <map>[: {metrics}][, patches of {directions} neighbours]."),
    (
        "landscape.mspa",
        "MSPA's patterns of <map>[, its foreground {foreground}]: connected by {connectivity} neighbours, the \
         edges {edge_width} cell wide.",
    ),
    ("validate.agreement", "How well <simulated> agrees with <observed>, both from <start>."),
    ("validate.kappa", "The kappa of <simulated> against <reference>."),
    (
        "sdm.fit",
        "A species distribution model fitted to <records>[ on {predictors}][, its features {classes}]\
         [, regularisation {regmult}], output {type}.",
    ),
    (
        "sdm.area_of_habitat",
        "Each species' area of habitat within <ranges> on <land_cover>[, natural classes {natural}].",
    ),
    (
        "services.carbon",
        "The carbon stored in <lulc> by the pools <pools>[, and its change to <future>][ valued at {price} a ton].",
    ),
    (
        "prioritize.optimize",
        "The planning units chosen[ for <features>][ at the cost <cost>], the objective {objective}[ within a budget \
         of {budget}].",
    ),
];

/// The phrases in Portuguese, the same placeholders as the English.
const PHRASES_PT: &[(&str, &str)] = &[
    ("raster.read", "Lê os rasters de {path}[, banda {band}][, com {nodata} como sem dado]."),
    ("raster.write", "Grava <value> em {path}[ como {type}]."),
    ("vector.read", "Lê as feições de {path}[, camada {layer}]."),
    ("vector.write", "Grava as feições de <vector> em {path}[, camada {layer}]."),
    ("table.read", "Lê a tabela {path}."),
    ("move.read_tracks", "Lê as trilhas dos animais de {path}."),
    ("view.plot", "Mostra[ <value>][ <features>][ <table>][ sobre <under>][ como \"{title}\"]."),
    ("nlm.random", "Um mapa aleatório de {rows} × {columns} células[ de lado {resolution}], semente {seed}."),
    (
        "nlm.gaussian_field",
        "Um campo aleatório gaussiano de {rows} × {columns} células[ de lado {resolution}]: alcance da \
         autocorrelação de {range} células, variância {variance}, efeito pepita {nugget}; semente {seed}.",
    ),
    (
        "nlm.random_cluster",
        "Agrupamentos aleatórios em {rows} × {columns} células[ de lado {resolution}], uma fração {p} das células \
         em agrupamentos[ ({neighbourhood} vizinhos)][, as classes com pesos {weights}]; semente {seed}.",
    ),
    (
        "nlm.fbm",
        "Movimento browniano fracionário em {rows} × {columns} células[ de lado {resolution}], dimensão fractal \
         {dimension}; semente {seed}.",
    ),
    (
        "nlm.planar_gradient",
        "Um gradiente planar em {rows} × {columns} células[ de lado {resolution}][, voltado para {direction}°]; \
         semente {seed}.",
    ),
    (
        "nlm.distance_gradient",
        "Um gradiente da distância a um retângulo[ ({origin})] em {rows} × {columns} células; semente {seed}.",
    ),
    ("nlm.binarize", "<map> dividido em habitat e matriz, uma fração {breaks} dele habitat."),
    ("raster.reclassify", "<map> com os valores reclassificados: {rules}."),
    ("raster.reproject", "<raster> reprojetado[ para {crs}][, células de lado {resolution}][, por {resampling}]."),
    ("raster.resample", "<raster> reamostrado para células de lado {resolution}[, por {resampling}]."),
    ("markov.fit", "A cadeia de Markov das mudanças de <t0> para <t1>[, em <steps> passos]."),
    ("glm.fit", "Uma regressão logística de cada classe de <landscape> em <drivers>[ (fórmulas {formulas})]."),
    ("glm.predict", "A probabilidade de cada classe pela regressão <model>, em <drivers>[ e <landscape>]."),
    (
        "woe.fit",
        "Os pesos de evidência de cada mudança de <t0> para <t1> por <drivers>[, faixas em incrementos de \
         {increment}][, {categorical} tomadas como categorias].",
    ),
    ("woe.probability", "A probabilidade de cada mudança pelos pesos <model>, em <landscape> e <drivers>."),
    ("demand.from_maps", "A área de cada classe ao longo do tempo, de <maps>[ datados {times}][, para {out_times}]."),
    ("dinamica.step", "Um passo da alocação das mudanças <rates> do Dinamica em <landscape>, pela <probability>."),
    ("lulcc.step", "Um passo da alocação de <demand> em <landscape>, pela <suitability>."),
    (
        "patches.observe",
        "As manchas das mudanças de <t0> para <t1>: quanto elas se expandem e a sua isometria[ ({isometry})].",
    ),
    ("metrics.class", "As métricas de cada classe de <map>[: {metrics}][, manchas de {directions} vizinhos]."),
    ("metrics.landscape", "As métricas da paisagem inteira <map>[: {metrics}][, manchas de {directions} vizinhos]."),
    ("metrics.patch", "As métricas de cada mancha de <map>[: {metrics}][, manchas de {directions} vizinhos]."),
    (
        "landscape.mspa",
        "Os padrões do MSPA de <map>[, o primeiro plano {foreground}]: conectados por {connectivity} vizinhos, as \
         bordas de {edge_width} células de largura.",
    ),
    ("validate.agreement", "Quanto <simulated> concorda com <observed>, os dois a partir de <start>."),
    ("validate.kappa", "O kappa de <simulated> contra <reference>."),
    (
        "sdm.fit",
        "Um modelo de distribuição de espécies ajustado a <records>[ em {predictors}][, as feições {classes}]\
         [, regularização {regmult}], saída {type}.",
    ),
    (
        "sdm.area_of_habitat",
        "A área de habitat de cada espécie dentro de <ranges> em <land_cover>[, as classes naturais {natural}].",
    ),
    (
        "services.carbon",
        "O carbono estocado em <lulc> pelos estoques <pools>[, e a sua mudança para <future>][ a {price} a tonelada].",
    ),
    (
        "prioritize.optimize",
        "As unidades de planejamento escolhidas[ para <features>][ ao custo <cost>], o objetivo {objective}[ num \
         orçamento de {budget}].",
    ),
];

/// The phrase of an operator, if it has one.
pub fn phrase(op: &str) -> Option<&'static str> {
    phrase_in(op, Lang::En)
}

/// The phrase of an operator in a language, if it has one.
pub fn phrase_in(op: &str, lang: Lang) -> Option<&'static str> {
    let table = if lang == Lang::Pt { PHRASES_PT } else { PHRASES };
    table.iter().find(|(o, _)| *o == op).map(|(_, p)| *p)
}

/// The operators with a phrase.
pub fn operators() -> impl Iterator<Item = &'static str> {
    PHRASES.iter().map(|(o, _)| *o)
}

/// A setting's value in words: given, else the operator's default (none: neither).
fn value(n: &Node, sig: Option<&OpSignature>, name: &str, given_only: bool, lang: Lang) -> Option<String> {
    if let Some(v) = n.params.get(name) {
        return Some(crate::document::words_in(v, lang).replace('\n', "; "));
    }
    if given_only {
        return None;
    }
    let doc = sig?.params.iter().find(|p| p.name == name)?.doc.clone();
    let d = crate::node_view::default_of(&doc);
    let d = d.strip_prefix("default ").unwrap_or(&d).trim().to_string();
    if !d.is_empty() && d != "default" {
        return Some(d);
    }
    // "8 (default) or 4": the word before it
    let before = doc.split_once(" (default)")?.0;
    let word = before.rsplit([' ', ':', ',', '(']).next()?.trim();
    (!word.is_empty()).then(|| word.to_string())
}

/// Where an input comes from, in a word: the node (not its port), the model's input, the value.
fn source(n: &Node, port: &str) -> Option<String> {
    Some(match From::of(n.inputs.get(port)?) {
        From::Node { node, .. } | From::Within { node, .. } => node,
        From::Input(name) | From::Carried(name) => name,
        From::Value(v) => v,
    })
}

/// Fill a template's placeholders (`given_only`: inside brackets, nothing taken from the defaults); none when one
/// has no value.
fn fill(t: &str, n: &Node, sig: Option<&OpSignature>, given_only: bool, lang: Lang) -> Option<String> {
    let mut out = String::new();
    let mut rest = t;
    while let Some(at) = rest.find(['{', '<']) {
        out.push_str(&rest[..at]);
        let close = if rest[at..].starts_with('{') { '}' } else { '>' };
        let end = at + rest[at..].find(close)?;
        let name = &rest[at + 1..end];
        let v = if close == '}' { value(n, sig, name, given_only, lang)? } else { source(n, name)? };
        out.push_str(&v);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// The sentence of a step: its operator's phrase with its settings and inputs (none: no phrase, or a setting it
/// needs has no value).
pub fn sentence(n: &Node, sig: Option<&OpSignature>) -> Option<String> {
    sentence_in(n, sig, Lang::En)
}

/// The sentence of a step in a language.
pub fn sentence_in(n: &Node, sig: Option<&OpSignature>, lang: Lang) -> Option<String> {
    let t = phrase_in(&n.op, lang)?;
    let mut out = String::new();
    let mut rest = t;
    while let Some(at) = rest.find('[') {
        out.push_str(&fill(&rest[..at], n, sig, false, lang)?);
        let end = at + rest[at..].find(']')?;
        out.push_str(&fill(&rest[at + 1..end], n, sig, true, lang).unwrap_or_default());
        rest = &rest[end + 1..];
    }
    out.push_str(&fill(rest, n, sig, false, lang)?);
    // (a sentence starts with a capital, an input's name first too)
    let mut c = out.chars();
    Some(c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_ops::Registry;

    /// The settings a step's sentence says (left out of the list under it, unless they differ from the default).
    fn said(op: &str) -> std::collections::BTreeMap<&'static str, ()> {
        let mut out = std::collections::BTreeMap::new();
        if let Some(t) = phrase(op) {
            let mut rest = t;
            while let Some(at) = rest.find('{') {
                let Some(end) = rest[at..].find('}') else { break };
                out.insert(&rest[at + 1..at + end], ());
                rest = &rest[at + end + 1..];
            }
        }
        out
    }

    #[test]
    fn a_step_reads_as_a_sentence() {
        let r = Registry::standard();
        let sig = |op: &str| r.sig(op).map(|s| s.as_ref().clone());
        let n = Node::new("aptitude", "nlm.gaussian_field").param("rows", 100).param("columns", 120).param("seed", 7);
        assert_eq!(
            sentence(&n, sig("nlm.gaussian_field").as_ref()).as_deref(),
            Some(
                "A Gaussian random field of 100 × 120 cells: autocorrelation range 10 cells, variance 5, nugget 0.2; seed 7."
            )
        );
        let n = Node::new("habitat", "nlm.binarize").input("map", "aptitude.map").param("breaks", 0.3);
        assert_eq!(
            sentence(&n, sig("nlm.binarize").as_ref()).as_deref(),
            Some("Aptitude split into habitat and matrix, a share 0.3 of it habitat.")
        );
        let n = Node::new("chain", "markov.fit").input("t0", "$landuse_2000").input("t1", "$landuse_2010");
        assert_eq!(
            sentence(&n, sig("markov.fit").as_ref()).as_deref(),
            Some("The Markov chain of the changes from landuse_2000 to landuse_2010.")
        );
        // in Portuguese, the same operators, settings and inputs
        let marks = |t: &str| {
            let mut m: Vec<String> = t
                .split(['{', '<'])
                .skip(1)
                .filter_map(|p| p.split_once(['}', '>']).map(|(a, _)| a.to_string()))
                .collect();
            m.sort();
            m
        };
        for op in operators() {
            let pt = phrase_in(op, Lang::Pt).unwrap_or_else(|| panic!("{op}: no phrase in Portuguese"));
            assert_eq!(marks(pt), marks(phrase(op).unwrap()), "{op}");
            assert_eq!(pt.matches('[').count(), phrase(op).unwrap().matches('[').count(), "{op}");
        }
        assert_eq!(PHRASES_PT.len(), PHRASES.len());
        let n = Node::new("habitat", "nlm.binarize").input("map", "aptitude.map").param("breaks", 0.3);
        assert_eq!(
            sentence_in(&n, sig("nlm.binarize").as_ref(), Lang::Pt).as_deref(),
            Some("Aptitude dividido em habitat e matriz, uma fração 0.3 dele habitat.")
        );
        // an input it needs not given: no sentence
        assert_eq!(sentence(&Node::new("k", "validate.kappa"), sig("validate.kappa").as_ref()), None);
        // every phrase names settings and inputs its operator has
        for op in operators() {
            let s = sig(op).unwrap_or_else(|| panic!("{op}: no such operator"));
            for name in said(op).keys() {
                assert!(s.params.iter().any(|p| p.name == *name), "{op}: no setting {name}");
            }
            let mut rest = phrase(op).unwrap();
            while let Some(at) = rest.find('<') {
                let end = at + rest[at..].find('>').unwrap();
                let port = &rest[at + 1..end];
                assert!(s.inputs.iter().any(|p| p.name == port), "{op}: no input {port}");
                rest = &rest[end + 1..];
            }
        }
    }
}
