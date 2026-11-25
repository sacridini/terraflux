//! Shortcuts of the search: an entry that adds a node of an existing operator with its settings already filled
//! ("Within" adds `vector.select_by_location` with `predicate = "within"`). No operator of its own.

/// A shortcut: what the search shows, the operator it adds and the settings it fills.
pub struct Preset {
    pub key: &'static str,
    pub title: &'static str,
    pub op: &'static str,
    pub params: &'static [(&'static str, &'static str)],
    /// What it gives, in a sentence (with a few words to be found by).
    pub doc: &'static str,
}

/// An entry's name is this and the key of the preset: `preset:within`.
pub const PREFIX: &str = "preset:";

pub const PRESETS: &[Preset] = &[
    Preset {
        key: "within",
        title: "Within",
        op: "vector.select_by_location",
        params: &[("predicate", "within")],
        doc: "Select by location with the predicate within: the features of a layer inside those of another (ST_Within, Extract by location, dentro).",
    },
    Preset {
        key: "contains",
        title: "Contains",
        op: "vector.select_by_location",
        params: &[("predicate", "contains")],
        doc: "Select by location with the predicate contains: the features of a layer that hold those of another (ST_Contains, Extract by location, contem).",
    },
    Preset {
        key: "intersects",
        title: "Intersects",
        op: "vector.select_by_location",
        params: &[("predicate", "intersects")],
        doc: "Select by location with the predicate intersects: the features of a layer that meet those of another (ST_Intersects, Extract by location, interseta).",
    },
    Preset {
        key: "touches",
        title: "Touches",
        op: "vector.select_by_location",
        params: &[("predicate", "touches")],
        doc: "Select by location with the predicate touches: the features of a layer that share a border with those of another (ST_Touches, Extract by location, toca).",
    },
    Preset {
        key: "disjoint",
        title: "Disjoint",
        op: "vector.select_by_location",
        params: &[("predicate", "disjoint")],
        doc: "Select by location with the predicate disjoint: the features of a layer that meet none of another (ST_Disjoint, Extract by location, fora, disjunto).",
    },
    Preset {
        key: "covered_by",
        title: "Covered by",
        op: "vector.select_by_location",
        params: &[("predicate", "covered_by")],
        doc: "Select by location with the predicate covered by: the features of a layer with no point outside those of another (ST_CoveredBy, Extract by location, coberto por).",
    },
    Preset {
        key: "dwithin",
        title: "Within a distance",
        op: "vector.select_by_location",
        params: &[("predicate", "dwithin")],
        doc: "Select by location with the predicate within a distance (set the distance): the features of a layer near those of another (ST_DWithin, buffer, perto, distancia).",
    },
    Preset {
        key: "count_in_polygons",
        title: "Count points in polygons",
        op: "vector.spatial_join",
        params: &[("mode", "summary"), ("predicate", "intersects")],
        doc: "Spatial join in summary mode: how many points fall in each polygon (Count points in polygon, contar pontos em poligonos, join by location).",
    },
];

/// The preset an entry's name (`preset:key`) is.
pub fn named(name: &str) -> Option<&'static Preset> {
    let key = name.strip_prefix(PREFIX)?;
    PRESETS.iter().find(|p| p.key == key)
}

/// A preset's settings as values (all text).
pub fn params(p: &Preset) -> Vec<(&'static str, serde_json::Value)> {
    p.params.iter().map(|(k, v)| (*k, serde_json::Value::String((*v).to_string()))).collect()
}

/// What an entry's name adds: the operator, and the settings to fill (none for an operator).
pub fn resolve(name: &str) -> (String, Vec<(&'static str, serde_json::Value)>) {
    match named(name) {
        Some(p) => (p.op.to_string(), params(p)),
        None => (name.to_string(), Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_ops::Registry;

    #[test]
    fn every_preset_is_an_operator_with_settings_it_takes() {
        let r = Registry::standard();
        for p in PRESETS {
            let sig = r.sig(p.op).unwrap_or_else(|| panic!("{}: no operator {}", p.key, p.op));
            for (k, v) in p.params {
                let spec =
                    sig.params.iter().find(|s| s.name == *k).unwrap_or_else(|| panic!("{}: no setting {k}", p.key));
                assert!(spec.choices.is_empty() || spec.choices.iter().any(|c| c == v), "{}: {k} = {v}", p.key);
            }
            assert!(!p.doc.is_empty() && !p.title.is_empty());
        }
    }

    #[test]
    fn the_keys_are_unique_and_found_by_name() {
        for (i, p) in PRESETS.iter().enumerate() {
            assert!(PRESETS[..i].iter().all(|q| q.key != p.key), "{} twice", p.key);
            assert_eq!(named(&format!("{PREFIX}{}", p.key)).map(|q| q.op), Some(p.op));
        }
        let p = named("preset:within").unwrap();
        assert_eq!(params(p), vec![("predicate", serde_json::json!("within"))]);
        assert!(named("vector.select_by_location").is_none());
        assert!(named("preset:nothing").is_none());
    }
}
