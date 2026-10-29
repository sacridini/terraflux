//! What is shown first of the operators of many settings: the inputs and settings most models set, besides those
//! the operator needs. An interface shows these (and those it needs, and those given) and folds the others away
//! as advanced; an operator of few settings has no list: all of its settings are shown.

/// The operators of many settings and their essential inputs and settings, by name (those they need are shown
/// anyway and are not listed; a name both an input's and a setting's is the input's).
const ESSENTIALS: &[(&str, &[&str])] = &[
    // conservation planning
    (
        "prioritize.optimize",
        &[
            "features",
            "cost",
            "locked_in",
            "locked_out",
            "objective",
            "budget",
            "relative_targets",
            "absolute_targets",
            "boundary_penalty",
            "decisions",
            "gap",
            "time_limit",
            "solver",
        ],
    ),
    (
        "prioritize.zonation",
        &["cost", "hierarchic_mask", "analysis_area", "rule", "rule_param", "weights", "connectivity"],
    ),
    (
        "prioritize.calibrate",
        &["features", "cost", "approach", "penalty", "values", "n", "objective", "relative_targets"],
    ),
    (
        "prioritize.marxan",
        &["features", "cost", "pu", "spec", "puvspr", "bound", "blm", "prop", "targets", "num_reps", "seed"],
    ),
    (
        "prioritize.evaluate",
        &["features", "cost", "solution", "relative_targets", "absolute_targets", "replacement", "rank"],
    ),
    (
        "prioritize.actions_raster",
        &["action_cost", "target_recovery", "target_conservation", "relative", "model", "budget"],
    ),
    (
        "restoration.plan",
        &["restorable", "cost", "regions", "method", "target", "sense", "relative", "budget", "z", "levels"],
    ),
    // landscape metrics
    ("metrics.landscape", &["metrics", "type", "directions"]),
    ("metrics.class", &["metrics", "type", "directions", "show"]),
    ("metrics.patch", &["metrics", "type", "directions", "show"]),
    ("metrics.window", &["metrics", "type", "directions", "window", "shape"]),
    ("metrics.series", &["plots", "metrics", "type", "directions", "level", "dates"]),
    ("metrics.sample", &["metrics", "type", "directions", "level", "size", "shape", "id_column"]),
    ("metrics.signatures", &["plots", "landscape_metrics", "class_metrics", "classes", "directions"]),
    // connectivity
    (
        "circuit.omniscape",
        &[
            "source",
            "block_size",
            "source_threshold",
            "resistance_is_conductance",
            "flow_potential",
            "normalized_current",
        ],
    ),
    ("circuit.one_to_all", &["mask", "habitat_is_resistance", "cum_map", "max_map", "all_to_one"]),
    ("conn.graph_from_raster", &["map", "resistance", "habitat", "distance", "max_distance", "kind", "area_unit"]),
    (
        "conn.graph_from_vector",
        &["vector", "resistance", "id", "attribute", "distance", "max_distance", "kind", "area_unit"],
    ),
    ("conn.graph_from_samc", &["absorption", "dispersal_distance", "attribute", "kind", "directions"]),
    ("conn.protconn", &["resistance", "probability", "distance_type", "transboundary", "area_unit"]),
    ("conn.protconn_regions", &["resistance", "id", "probability", "distance_type", "transboundary", "area_unit"]),
    // data
    (
        "data.download",
        &["like", "source", "variable", "layers", "years", "property", "depth", "url", "extent", "cellsize"],
    ),
    // machine learning
    ("ml.fit", &["table", "predictors", "target", "column", "task", "method", "sample", "n_estimators", "max_iter"]),
    ("ml.potential", &["landscape", "later", "drivers", "target", "classes", "method", "sample", "n_estimators"]),
    // species distributions and biodiversity
    ("sdm.fit", &["predictors", "categorical", "model", "classes", "regmult", "cv"]),
    ("sdm.migrate", &["initial", "suitability", "barriers", "steps", "kernel", "ldd_frequency", "seed"]),
    (
        "conn.metapopulation",
        &["patches", "steps", "alpha", "x", "y", "e", "kernel", "colonisation", "extinction", "seed"],
    ),
    ("sdm.prepare", &["species", "thin", "area", "predictors", "background", "folds"]),
    (
        "sdm.clean_occurrences",
        &["x", "y", "species", "action", "capitals", "centroids", "seas", "outliers", "duplicates"],
    ),
    ("sdm.spatial_blocks", &["presence", "size", "k", "hexagon", "selection", "seed"]),
    ("bio.randomise", &["function", "iterations", "rand_seed"]),
    ("bio.occurrences", &["occurrences", "path", "label", "to_crs", "max_uncertainty", "basis_of_record"]),
    ("bio.matrix", &["element_columns", "sparse", "separator"]),
    ("bio.table", &["count_columns", "cell_sizes", "crs", "separator"]),
    // land change
    ("futures.simulation", &[]),
    ("futures.calibrate", &["compactness_means", "compactness_ranges", "discount_factors", "repeat", "seed"]),
    ("woe.fit", &["markov", "increment", "ranges", "categorical", "transitions"]),
    // movement ecology
    ("move.distribution", &["shape", "scale", "rate", "kappa"]),
    // neutral landscape models
    ("nlm.mosaic_field", &["rows", "columns", "resolution", "seed", "n", "mean", "sd"]),
    ("nlm.neigh", &["rows", "columns", "resolution", "seed", "categories", "proportions"]),
    // ecosystem services
    (
        "services.ndr",
        &[
            "nitrogen",
            "phosphorus",
            "subsurface_critical_length",
            "subsurface_efficiency",
            "load_type_n",
            "load_type_p",
        ],
    ),
];

/// The essential inputs and settings of an operator of many settings (besides those it needs); none for an
/// operator of few, all of whose settings are shown.
pub fn essentials(op: &str) -> Option<&'static [&'static str]> {
    ESSENTIALS.iter().find(|(o, _)| *o == op).map(|(_, e)| *e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;

    /// Every operator of ten settings or more has its list, and every name of a list is one of its operator's
    /// inputs or settings (a renamed setting shows here).
    #[test]
    fn the_lists_are_of_the_operators_settings() {
        let r = Registry::standard();
        for (op, names) in ESSENTIALS {
            let sig = r.sig(op).unwrap_or_else(|| panic!("no operator {op}"));
            for n in *names {
                assert!(
                    sig.params.iter().any(|p| p.name == *n) || sig.inputs.iter().any(|p| p.name == *n),
                    "{op} has no input or setting {n}"
                );
            }
        }
        for sig in r.signatures() {
            if sig.params.len() >= 10 {
                assert!(essentials(&sig.name).is_some(), "{} has {} settings and no list", sig.name, sig.params.len());
            }
        }
    }
}
