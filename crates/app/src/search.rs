//! Finding operators and templates by what they are to do: the words of a query without their accents and the
//! small words, each found in a text by itself, by its stem (plurals), or by the words the operators' documentation
//! says otherwise. What has all of the words first, else what has the most of them.

/// The words a query is not looked up by.
const SMALL: &[&str] =
    &["a", "an", "and", "as", "at", "by", "for", "from", "in", "into", "of", "on", "or", "the", "to", "with"];

/// What a word stands for, in the words the operators' documentation uses.
const SYNONYMS: &[(&str, &[&str])] = &[("deforestation", &["deforest", "forest loss"])];

/// The words an operator is also found by, beyond its name and documentation (folded; empty for most).
const KEYWORDS: &[(&str, &str)] = &[
    (
        "vector.select_by_location",
        "within inside dentro contains contem intersects intersecta touches toca disjoint dwithin within distance \
         st_within st_intersects st_contains st_dwithin select by location extract by location selecionar por \
         localizacao extrair por localizacao filtro espacial spatial filter",
    ),
    (
        "vector.spatial_join",
        "sjoin spatial join join by location join attributes by location juntar atributos por localizacao uniao \
         espacial within dentro points in polygons pontos em poligonos count points in polygon contar pontos \
         st_within",
    ),
    ("vector.nearest", "sjoin_nearest nearest neighbour nearest neighbor vizinho mais proximo join by nearest knn"),
];

/// The extra words an operator is found by (folded; empty when it has none).
pub fn keywords(op: &str) -> &'static str {
    KEYWORDS.iter().find(|(o, _)| *o == op).map_or("", |(_, k)| k)
}

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
    let singular = if word.len() > 4 && word.ends_with("es") && !word.ends_with("ses") {
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
        assert_eq!(
            words("Fragmentation analysis of the landscape, Año"),
            ["fragmentation", "analysis", "landscape", "ano"]
        );
        assert_eq!(words("the of"), ["the", "of"]);
    }

    #[test]
    fn keywords_of_the_vector_predicates() {
        assert!(keywords("vector.select_by_location").contains("dentro"));
        assert!(keywords("vector.spatial_join").contains("sjoin"));
        assert!(keywords("vector.nearest").contains("knn"));
        assert_eq!(keywords("raster.read"), "");
        for (_, k) in KEYWORDS {
            assert_eq!(fold(k), *k, "keywords are folded");
        }
    }

    #[test]
    fn found_by_plurals() {
        let text = fold("Landscape metrics of every class: the patches' edge density");
        assert_eq!(score(&text, &words("landscape metric")), 2);
        assert_eq!(score(&text, &words("density of edges")), 2);
        assert_eq!(score(&text, &words("patch")), 1);
        assert_eq!(score(&text, &words("connectivity")), 0);
        assert_eq!(score("transitions between dates", &words("transition")), 1);
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
