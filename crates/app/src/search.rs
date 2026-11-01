//! Finding operators and templates by what they are to do, in English or in Portuguese: the words of a query
//! without their accents and the small words, each found in a text by itself, by its stem (plurals), or by the
//! English words a Portuguese one (or a word of the field) stands for. What has all of the words first, else what
//! has the most of them.

/// The words a query is not looked up by.
const SMALL: &[&str] = &[
    "a", "an", "and", "as", "at", "by", "for", "from", "in", "into", "of", "on", "or", "the", "to", "with", "o", "os",
    "um", "uma", "uns", "umas", "de", "da", "do", "das", "dos", "e", "em", "na", "no", "nas", "nos", "para", "por",
    "com", "que", "se", "ao", "aos", "entre", "sobre", "pela", "pelo",
];

/// What a word (without its accents) stands for: Portuguese words of the field, and English ones the operators'
/// documentation says otherwise.
const SYNONYMS: &[(&str, &[&str])] = &[
    ("desmatamento", &["deforest", "forest loss"]),
    ("deforestation", &["deforest", "forest loss"]),
    ("uso", &["land use", "use"]),
    ("solo", &["land", "soil"]),
    ("terra", &["land"]),
    ("cobertura", &["cover"]),
    ("mudanca", &["change"]),
    ("simulacao", &["simulat"]),
    ("simular", &["simulat"]),
    ("validacao", &["validat", "figure of merit"]),
    ("validar", &["validat", "figure of merit"]),
    ("calibracao", &["calibrat", "fit"]),
    ("calibrar", &["calibrat", "fit"]),
    ("conectividade", &["connectivity"]),
    ("corredor", &["corridor"]),
    ("fragmentacao", &["fragmentation"]),
    ("paisagem", &["landscape"]),
    ("metrica", &["metric"]),
    ("mancha", &["patch"]),
    ("borda", &["edge"]),
    ("densidade", &["density"]),
    ("floresta", &["forest"]),
    ("vegetacao", &["vegetation"]),
    ("especie", &["species"]),
    ("distribuicao", &["distribution"]),
    ("nicho", &["niche", "distribution"]),
    ("ocorrencia", &["occurrence", "record"]),
    ("registro", &["record"]),
    ("priorizacao", &["prioriti", "planning"]),
    ("priorizar", &["prioriti", "planning"]),
    ("prioridade", &["priorit"]),
    ("conservacao", &["conservation"]),
    ("restauracao", &["restoration"]),
    ("custo", &["cost"]),
    ("resistencia", &["resistance"]),
    ("distancia", &["distance"]),
    ("declividade", &["slope"]),
    ("relevo", &["terrain", "elevation"]),
    ("elevacao", &["elevation"]),
    ("altitude", &["elevation"]),
    ("rio", &["river", "stream"]),
    ("hidrografia", &["river", "stream"]),
    ("bacia", &["watershed", "catchment"]),
    ("precipitacao", &["precipitation", "rain"]),
    ("chuva", &["rain", "precipitation"]),
    ("clima", &["climate"]),
    ("carbono", &["carbon"]),
    ("biomassa", &["biomass"]),
    ("reclassificar", &["reclassif"]),
    ("reclassificacao", &["reclassif"]),
    ("recortar", &["clip"]),
    ("recorte", &["clip"]),
    ("reprojetar", &["reproject"]),
    ("reprojecao", &["reproject"]),
    ("reamostrar", &["resampl"]),
    ("mosaico", &["mosaic"]),
    ("mapa", &["map"]),
    ("vetor", &["vector"]),
    ("poligono", &["polygon"]),
    ("ponto", &["point"]),
    ("tabela", &["table"]),
    ("estatistica", &["statistic"]),
    ("amostra", &["sample"]),
    ("amostragem", &["sampl"]),
    ("acuracia", &["accuracy"]),
    ("exatidao", &["accuracy"]),
    ("incerteza", &["uncertainty"]),
    ("sensibilidade", &["sensitivity"]),
    ("cenario", &["scenario"]),
    ("demanda", &["demand"]),
    ("peso", &["weight"]),
    ("evidencia", &["evidence"]),
    ("regressao", &["regression"]),
    ("logistica", &["logistic"]),
    ("aleatoria", &["random"]),
    ("aleatorio", &["random"]),
    ("transicao", &["transition"]),
    ("agente", &["agent"]),
    ("cidade", &["urban", "city"]),
    ("urbano", &["urban"]),
    ("urbana", &["urban"]),
    ("agricultura", &["farm", "crop", "agricultur"]),
    ("agricola", &["farm", "crop", "agricultur"]),
    ("pastagem", &["pasture"]),
    ("estrada", &["road"]),
    ("circuito", &["circuit"]),
    ("caminho", &["path"]),
    ("menor", &["least"]),
    ("fluxo", &["flow"]),
    ("interpolacao", &["interpol", "kriging", "idw"]),
    ("interpolar", &["interpol", "kriging", "idw"]),
    ("krigagem", &["kriging"]),
    ("tendencia", &["trend"]),
    ("autocorrelacao", &["autocorrel"]),
    ("morfologia", &["morpholog"]),
    ("morfologica", &["morpholog"]),
    ("nucleo", &["core"]),
    ("protegida", &["protected"]),
    ("protegidas", &["protected"]),
    ("servicos", &["service"]),
    ("ecossistemicos", &["ecosystem"]),
    ("polinizacao", &["pollination"]),
    ("erosao", &["erosion", "sediment"]),
    ("agua", &["water"]),
    ("neutra", &["neutral"]),
    ("neutro", &["neutral"]),
];

/// A text without its accents, in small letters.
pub fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

/// A query's words: without accents, the small words left out (all of them kept when it has only those).
pub fn words(query: &str) -> Vec<String> {
    let all: Vec<String> = fold(query)
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '_').to_string())
        .filter(|w| !w.is_empty())
        .collect();
    let kept: Vec<String> = all.iter().filter(|w| !SMALL.contains(&w.as_str())).cloned().collect();
    if kept.is_empty() { all } else { kept }
}

/// The forms a word is found by: itself, its singular, what it stands for.
fn forms(word: &str) -> Vec<String> {
    let mut out = vec![word.to_string()];
    let singular = if word.len() > 4 && (word.ends_with("oes") || word.ends_with("aes")) {
        Some(format!("{}ao", &word[..word.len() - 3]))
    } else if word.len() > 4 && word.ends_with("es") && !word.ends_with("ses") {
        Some(word[..word.len() - 2].to_string())
    } else if word.len() > 3 && word.ends_with('s') && !word.ends_with("ss") {
        Some(word[..word.len() - 1].to_string())
    } else {
        None
    };
    if let Some(s) = singular {
        out.push(s);
    }
    for f in out.clone() {
        if let Some((_, also)) = SYNONYMS.iter().find(|(w, _)| *w == f) {
            out.extend(also.iter().map(|a| a.to_string()));
        }
    }
    out
}

/// How many of the words a text (folded) has.
pub fn score(text: &str, words: &[String]) -> usize {
    words.iter().filter(|w| forms(w).iter().any(|f| text.contains(f.as_str()))).count()
}

/// The items found by a query's words, the best first: those that have all of them (and whether they do), else
/// those that have the most (at least one). `text` gives an item's text (folded here); `first` the part of it
/// whose words count first (its name).
pub fn rank<'a, T>(
    items: &'a [T],
    words: &[String],
    text: impl Fn(&T) -> String,
    first: impl Fn(&T) -> String,
) -> (Vec<&'a T>, bool) {
    let mut scored: Vec<(usize, usize, &T)> = items
        .iter()
        .map(|i| (score(&fold(&text(i)), words), score(&fold(&first(i)), words), i))
        .filter(|(s, _, _)| *s > 0)
        .collect();
    let best = scored.iter().map(|(s, _, _)| *s).max().unwrap_or(0);
    let all = best == words.len() && best > 0;
    scored.retain(|(s, _, _)| *s == best || (!all && *s + 1 >= best && best > 1));
    // (stable: equal ones keep their order)
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    (scored.into_iter().map(|(_, _, i)| i).collect(), all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_without_accents_and_small_words() {
        assert_eq!(words("Análise de fragmentação da paisagem"), ["analise", "fragmentacao", "paisagem"]);
        assert_eq!(words("the of"), ["the", "of"]);
    }

    #[test]
    fn found_in_portuguese_and_by_plurals() {
        let text = fold("Landscape metrics of every class: the patches' edge density");
        assert_eq!(score(&text, &words("métricas de paisagem")), 2);
        assert_eq!(score(&text, &words("densidade de bordas")), 2);
        assert_eq!(score(&text, &words("manchas")), 1);
        assert_eq!(score(&text, &words("conectividade")), 0);
        assert_eq!(score("transitions between dates", &words("transições")), 1);
    }

    #[test]
    fn ranked_all_first_else_the_most() {
        let items = ["edge density of patches", "patch area", "river network"];
        let (found, all) = rank(&items, &words("patch edge"), |t| t.to_string(), |t| t.to_string());
        assert!(all);
        assert_eq!(found, [&"edge density of patches"]);
        let (found, all) = rank(&items, &words("patch river kriging"), |t| t.to_string(), |t| t.to_string());
        assert!(!all);
        assert_eq!(found.len(), 3, "{found:?}");
    }
}
