//! The Python counterparts of the machine learning operators (`ml.fit`, `ml.predict`, `ml.importance`,
//! `ml.partial_dependence`), of the randomisations and the properties of a basedata (`bio.randomise`,
//! `bio.properties`) and of the metrics of sample plots (`metrics.sample`), and the reasons of the operators of the
//! same kind that stay comments.
//!
//! The library has no function for a forest or a boosting fitted to a table: `ml.fit` is scikit-learn's own
//! (Terraflux's trees are scikit-learn's, to the bit), its inspection scikit-learn's `permutation_importance` and
//! `partial_dependence`. The library's `Estimator` (one model per class, `ml.potential`) is not the same object: a
//! model of `ml.potential` is not taken by these.
//!
//! Left as comments: `metrics.correlation` (the library takes and gives the long tables of the metrics, Terraflux's
//! is a wide table and its square matrix), `distance.proximity` and `raster.mosaic` (GDAL's own algorithms, which the
//! library does not have), `crafty.allocate` (CRAFTY is not in the library), `ml.pca` (scikit-learn's PCA gives the
//! same numbers, but the script has no stack of scores nor model to hand on to `ml.predict` yet), `ml.kmeans`
//! (scikit-learn's KMeans is the same algorithm, but its random starts are not Terraflux's, so the clusters would be
//! other ones, and the script has no map of clusters nor model to hand on either), `ml.som` (minisom.MiniSom is the
//! algorithm Terraflux's engine ports to the bit, but the starting weights are not the same, and the script has no map
//! of nodes, grouping of the nodes nor model to hand on).

#![allow(unused_imports)]

use super::*;

/// The operator that made the node an input points at: the `@kind` it put among its outputs (only the operators that
/// give a model do, as the script has to treat a model by what it is).
pub(super) fn kind_of(n: &Node, port: &str, sc: &Scope) -> Option<String> {
    let Some(Source::Ref(r)) = n.inputs.get(port) else { return None };
    match Ref::parse(r).ok()? {
        Ref::Port { node, .. } => sc.made(&node)?.outs.get("@kind").cloned(),
        _ => None,
    }
}

/// The operators of this family translated by code of their own (as `special` in python.rs); `None`: not one of them.
pub(super) fn special(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    match n.op.as_str() {
        "ml.fit" => fit(n, var, sc).map(Some),
        "ml.predict" => predict(n, var, sc).map(Some),
        "ml.importance" => importance(n, var, sc).map(Some),
        "ml.partial_dependence" => partial_dependence(n, var, sc).map(Some),
        "bio.randomise" => randomise(n, var, sc).map(Some),
        "bio.properties" => properties(n, var, sc).map(Some),
        "metrics.sample" => sample(n, var, sc).map(Some),
        "metrics.correlation" => Err("the Python library's calculate_correlation takes the long table of the metrics \
             (level, class, id, metric, value) and gives the lower triangle of the matrix as pairs; Terraflux's takes a \
             wide table and gives the square matrix, the same R correlations in another shape: not in the script yet"
            .into()),
        "distance.proximity" => Err("gdal_proximity's distances (two passes over the cells, an approximation of the \
             Euclidean distance): the Python library has no function for it, and scipy's exact distance transform \
             gives other numbers"
            .into()),
        "raster.mosaic" => Err("gdalwarp's mosaic of the tiles (its CRS, resolution and resampling): the Python \
             library has none (tf.landscape_mosaic is GuidosToolbox's Landscape Mosaic, another thing), and \
             rasterio's merge does not resample as gdalwarp does"
            .into()),
        "crafty.allocate" => Err("CRAFTY's competition between agent functional types is not in the Python library".into()),
        "segment.from_map" | "segment.chessboard" | "segment.quadtree" | "segment.from_vector" | "objects.to_map"
        | "objects.to_vector" | "segment.multiresolution" | "segment.slic" | "segment.snic" | "segment.felzenszwalb"
        | "segment.watershed" | "segment.meanshift" | "segment.threshold" | "segment.spectral_difference" | "segment.region_growing" | "segment.quickshift"
        | "segment.scale" | "segment.optimize" | "objects.features"
        | "objects.samples" | "objects.classify" | "objects.calc"
        | "objects.rules" | "objects.nearest" | "objects.resolve" | "objects.temporal" | "objects.transition_matrix"
        | "segment.sam" | "objects.embed" | "objects.accuracy" | "objects.sample" | "objects.merge" | "objects.grow"
        | "objects.remove_small" | "objects.reclassify" | "objects.edit"
        | "objects.split" | "objects.resize" | "objects.fuse" | "objects.extrema" => Err("objects (object-based image analysis: an image divided into objects with their \
             attributes, classes and levels) are a Terraflux type the Python library has no counterpart for: no \
             equivalent in the script"
            .into()),
        "ml.pca" => Err("scikit-learn's PCA (fit_transform on the z-scores of the cells with every variable, the \
             components' signs flipped to the largest loading positive) gives the same components, variances and \
             scores, but the script has no stack of scores and no model to hand on to ml.predict yet: no equivalent \
             in the script"
            .into()),
        "ml.kmeans" => Err("scikit-learn's KMeans is the same algorithm (greedy k-means++ start, Lloyd's iterations, the \
             best of n_init runs), but its random draws are not Terraflux's, so the clusters would be other ones \
             from the same seed, and the script has no map of clusters (numbered by the first principal component of \
             the centres) nor model to hand on to ml.predict, nor the choice of k by the silhouette: no equivalent \
             in the script"
            .into()),
        "ml.som" => Err("minisom.MiniSom trains the same map (Terraflux's engine is a port of it that gives the \
             same bits), but the weights it starts from are other ones (Terraflux's principal-component start spans \
             two standard deviations, its random one draws with numpy's new generator, MiniSom's with RandomState), \
             the batch takes MiniSom's train_batch_offline with a learning rate of 1, and the script has no map of \
             nodes, grouping of the nodes (Ward's method) nor model to hand on to ml.predict: no equivalent in the \
             script"
            .into()),
        "nlm.image" => Err("a synthetic multispectral image of a map of classes, with the truth known (for templates \
             and tests): the Python library has no function for it, so there is no equivalent in the script"
            .into()),
        _ => Ok(None),
    }
}

/// An input that must be connected.
fn need(n: &Node, k: &str, sc: &Scope) -> Result<String, String> {
    value_of(n, k, sc)?.ok_or_else(|| format!("its {k} is not connected"))
}

/// A setting that is a switch: `None` when not given, an error when it comes from a reference (the code differs).
fn flag(n: &Node, k: &str) -> Result<Option<bool>, String> {
    match n.params.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("its {k} as a reference or not a switch: not in the script yet")),
    }
}

/// The label of a class in the names of columns (`p_1`): whole numbers without their point.
const LABEL: &str = "int(_c) if float(_c).is_integer() else _c";

// -- ml.fit -------------------------------------------------------------------------------------------------------

/// The model of the node's method as scikit-learn writes it, over the classes or the values (`kind` of the task).
/// `cls` is the expression that says classes at run time (the task `auto`), else the task decides.
fn estimator(n: &Node, sc: &Scope, cls: Option<&str>, code: &mut String) -> Result<String, String> {
    let boosting = match choice(n, "method").unwrap_or("random_forest") {
        "random_forest" => false,
        "gradient_boosting" => true,
        other => return Err(format!("the method {other:?}: not in the script yet")),
    };
    let (mut args, classifier, regressor) = if boosting {
        code.push_str("from sklearn.ensemble import HistGradientBoostingClassifier, HistGradientBoostingRegressor\n");
        let mut a = given(
            n,
            &[
                ("max_depth", "max_depth"),
                ("min_samples_leaf", "min_samples_leaf"),
                ("max_features", "max_features"),
                ("learning_rate", "learning_rate"),
                ("max_iter", "max_iter"),
                ("max_leaf_nodes", "max_leaf_nodes"),
                ("l2_regularization", "l2_regularization"),
                ("max_bins", "max_bins"),
            ],
            sc,
        )?;
        // (Terraflux's boosting never stops early)
        a.push("early_stopping=False".into());
        (a, "HistGradientBoostingClassifier", "HistGradientBoostingRegressor")
    } else {
        code.push_str("from sklearn.ensemble import RandomForestClassifier, RandomForestRegressor\n");
        let mut a = given(
            n,
            &[
                ("n_estimators", "n_estimators"),
                ("max_depth", "max_depth"),
                ("min_samples_split", "min_samples_split"),
                ("min_samples_leaf", "min_samples_leaf"),
                ("bootstrap", "bootstrap"),
            ],
            sc,
        )?;
        match choice(n, "max_features") {
            Some("all") => a.push("max_features=None".into()),
            _ => a.extend(given(n, &[("max_features", "max_features")], sc)?),
        }
        (a, "RandomForestClassifier", "RandomForestRegressor")
    };
    args.push(format!("random_state={}", setting(n, "random_state", sc)?.unwrap_or_else(|| "0".into())));
    let name = match (cls, n.params.get("task")) {
        (_, Some(Value::String(t))) if t == "classes" => classifier.to_string(),
        (_, Some(Value::String(t))) if t == "values" => regressor.to_string(),
        (Some(c), _) => format!("({classifier} if {c} else {regressor})"),
        (None, _) => classifier.to_string(),
    };
    Ok(format!("{name}({})", args.join(", ")))
}

fn fit(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(
        n,
        &[
            "table",
            "predictors",
            "target",
            "cells",
            "column",
            "columns",
            "task",
            "sample",
            "stratify",
            "seed",
            "method",
            "n_estimators",
            "max_depth",
            "min_samples_split",
            "min_samples_leaf",
            "max_features",
            "bootstrap",
            "learning_rate",
            "max_iter",
            "max_leaf_nodes",
            "l2_regularization",
            "max_bins",
            "random_state",
        ],
    )?;
    let task = match n.params.get("task") {
        None | Some(Value::Null) => "auto",
        Some(Value::String(t)) if matches!(t.as_str(), "auto" | "classes" | "values") => t.as_str(),
        Some(_) => return Err("its task as a reference: not in the script yet".into()),
    };
    let c = format!("_{var}");
    let boosting = choice(n, "method") == Some("gradient_boosting");
    let mut code = String::from("import pandas as pd\n");
    match (value_of(n, "table", sc)?, value_of(n, "predictors", sc)?) {
        (Some(_), Some(_)) => return Err("a table or maps (predictors), not both".into()),
        (None, None) => return Err("the records are not connected: a table, or maps (predictors and target)".into()),
        // the rows with every predictor and the target
        (Some(t), None) => {
            let column = setting(n, "column", sc)?.unwrap_or_else(|| "\"target\"".into());
            let columns = match setting(n, "columns", sc)? {
                Some(l) => l,
                None => format!("[k for k in {t}.columns if k != {column}]"),
            };
            code.push_str(&format!(
                "{c}_cols = {columns}\n{c}_d = {t}[{c}_cols + [{column}]].dropna()\n{c}_X, {c}_y = {c}_d[{c}_cols], {c}_d[{column}]\n"
            ));
        }
        // the cells of the maps (its layers by name, increasing), those not 0 of `cells`
        (None, Some(p)) => {
            let target = need(n, "target", sc)?;
            code.push_str(&format!(
                "{c}_cols = sorted(str(b) for b in {p}[\"band\"].values)\n\
                 {c}_X = pd.DataFrame({{k: {p}.sel(band=k).values.ravel() for k in {c}_cols}})\n\
                 {c}_y = pd.Series(np.asarray({target}, dtype=float).ravel())\n\
                 {c}_ok = {c}_X.notna().all(axis=1).to_numpy() & {c}_y.notna().to_numpy()\n"
            ));
            if let Some(cells) = value_of(n, "cells", sc)? {
                code.push_str(&format!(
                    "{c}_cells = np.asarray({cells}, dtype=float).ravel()\n{c}_ok &= ~np.isnan({c}_cells) & ({c}_cells != 0)\n"
                ));
            }
            code.push_str(&format!("{c}_X, {c}_y = {c}_X[{c}_ok], {c}_y[{c}_ok]\n"));
        }
    }
    // classes when the target holds 50 whole numbers or fewer (before the sample)
    let cls = (task == "auto").then(|| format!("{c}_cls"));
    if let Some(k) = &cls {
        code.push_str(&format!("{k} = {c}_y.nunique() <= 50 and bool(({c}_y % 1 == 0).all())\n"));
    }
    // the sample, drawn as numpy's default_rng(seed) draws it (from each class apart when stratified)
    if let Some(s) = setting(n, "sample", sc)? {
        let seed = setting(n, "seed", sc)?.unwrap_or_else(|| "1".into());
        code.push_str(&format!("{c}_s = {s}\n{c}_rng = np.random.default_rng({seed})\n"));
        if flag(n, "stratify")? == Some(true) && task != "values" {
            code.push_str(&format!(
                "{c}_frac = min(1.0, int({c}_s) / len({c}_y) if {c}_s >= 1 else {c}_s)\n\
                 {c}_keep = []\n\
                 for {c}_v in np.unique({c}_y):\n\
                 \x20   {c}_g = np.flatnonzero({c}_y.to_numpy() == {c}_v)\n\
                 \x20   {c}_m = min({c}_g.size, max(1, round({c}_frac * {c}_g.size)))\n\
                 \x20   {c}_keep.append({c}_g[{c}_rng.choice({c}_g.size, size={c}_m, replace=False)])\n\
                 {c}_pick = np.sort(np.concatenate({c}_keep))\n\
                 {c}_X, {c}_y = {c}_X.iloc[{c}_pick], {c}_y.iloc[{c}_pick]\n"
            ));
        } else {
            code.push_str(&format!(
                "{c}_n = int({c}_s) if {c}_s >= 1 else round({c}_s * len({c}_y))\n\
                 if {c}_n < len({c}_y):\n\
                 \x20   {c}_pick = np.sort({c}_rng.choice(len({c}_y), size={c}_n, replace=False))\n\
                 \x20   {c}_X, {c}_y = {c}_X.iloc[{c}_pick], {c}_y.iloc[{c}_pick]\n"
            ));
        }
    } else {
        for k in ["stratify", "seed"] {
            if n.params.contains_key(k) {
                code.push_str(&format!("# (not passed: {k}: there is no sample)\n"));
            }
        }
    }
    let model = estimator(n, sc, cls.as_deref(), &mut code)?;
    code.push_str(&format!("{var} = {model}.fit({c}_X, {c}_y)\n"));
    let importance = if boosting {
        code.push_str(
            "# (importance: the share of the gain of the splits of Terraflux's boosting is not in scikit-learn)\n",
        );
        "None".to_string()
    } else {
        code.push_str(&format!(
            "{var}_importance = pd.DataFrame([{var}.feature_importances_], columns={var}.feature_names_in_)\n"
        ));
        format!("{var}_importance")
    };
    let outs = HashMap::from([
        ("model".to_string(), var.to_string()),
        ("importance".to_string(), importance),
        ("@kind".to_string(), "ml.fit".to_string()),
    ]);
    Ok((code, outs, Need::Nothing))
}

// -- what a fitted model does -----------------------------------------------------------------------------------

/// The model of a node of `ml.fit`: a model of `ml.potential` (one per class) is not a model of these.
fn fitted(n: &Node, sc: &Scope) -> Result<String, String> {
    let model = need(n, "model", sc)?;
    if kind_of(n, "model", sc).as_deref() == Some("ml.potential") {
        return Err(format!(
            "a model of ml.potential (one model per class): the probabilities are `{model}.predict(drivers)` \
             (tf.simulate.Estimator), the rest not in the script yet"
        ));
    }
    Ok(model)
}

fn predict(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["model", "predictors", "table"])?;
    let m = fitted(n, sc)?;
    let c = format!("_{var}");
    let mut code = format!("import pandas as pd\n{c}_names = list({m}.feature_names_in_)\n");
    match (value_of(n, "predictors", sc)?, value_of(n, "table", sc)?) {
        (Some(_), Some(_)) => Err("maps (predictors) or a table, not both".into()),
        (None, None) => Err("the maps (predictors) or a table are not connected".into()),
        // the cells with every predictor: the value or the class of the largest probability, and each class's
        (Some(p), None) => {
            code.push_str(&format!(
                "{c}_X = pd.DataFrame({{k: {p}.sel(band=k).values.ravel() for k in {c}_names}})\n\
                 {c}_ok = {c}_X.notna().all(axis=1).to_numpy()\n\
                 {c}_tpl = {p}.isel(band=0, drop=True)\n\
                 {c}_map = np.full(len({c}_X), np.nan)\n\
                 {c}_map[{c}_ok] = {m}.predict({c}_X[{c}_ok])\n\
                 {var} = {c}_tpl.copy(data={c}_map.reshape({c}_tpl.shape))\n\
                 {var}_probability = None\n\
                 if hasattr({m}, \"classes_\"):\n\
                 \x20   {c}_p = np.full((len({m}.classes_), len({c}_X)), np.nan)\n\
                 \x20   {c}_p[:, {c}_ok] = {m}.predict_proba({c}_X[{c}_ok]).T\n\
                 \x20   {var}_probability = {c}_tpl.expand_dims(band=list({m}.classes_)).copy(data={c}_p.reshape(len({m}.classes_), *{c}_tpl.shape))\n"
            ));
            let outs = HashMap::from([
                ("map".to_string(), var.to_string()),
                ("probability".to_string(), format!("{var}_probability")),
            ]);
            Ok((code, outs, Need::Nothing))
        }
        // the rows' columns, then the prediction and p_<class>
        (None, Some(t)) => {
            code.push_str(&format!(
                "{c}_X = {t}[{c}_names]\n\
                 {c}_ok = {c}_X.notna().all(axis=1).to_numpy()\n\
                 {var} = {t}.copy()\n\
                 {var}[\"prediction\"] = np.nan\n\
                 {var}.loc[{c}_ok, \"prediction\"] = {m}.predict({c}_X[{c}_ok])\n\
                 for _j, _c in enumerate(getattr({m}, \"classes_\", [])):\n\
                 \x20   {var}[f\"p_{{{LABEL}}}\"] = np.nan\n\
                 \x20   {var}.loc[{c}_ok, f\"p_{{{LABEL}}}\"] = {m}.predict_proba({c}_X[{c}_ok])[:, _j]\n"
            ));
            Ok((code, HashMap::from([("table".to_string(), var.to_string())]), Need::Nothing))
        }
    }
}

fn importance(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["model", "table", "column", "n_repeats", "random_state"])?;
    let m = fitted(n, sc)?;
    let t = need(n, "table", sc)?;
    let column = setting(n, "column", sc)?.unwrap_or_else(|| "\"target\"".into());
    let c = format!("_{var}");
    let repeats = setting(n, "n_repeats", sc)?.unwrap_or_else(|| "5".into());
    let seed = setting(n, "random_state", sc)?.unwrap_or_else(|| "0".into());
    // (the rows with every predictor and the target)
    let code = format!(
        "import pandas as pd\n\
         from sklearn.inspection import permutation_importance\n\
         {c}_names = list({m}.feature_names_in_)\n\
         {c}_d = {t}[{c}_names + [{column}]].dropna()\n\
         {var} = permutation_importance({m}, {c}_d[{c}_names], {c}_d[{column}], n_repeats={repeats}, random_state={seed})\n\
         {var}_importance = pd.DataFrame([{var}.importances_mean], columns={c}_names)\n\
         {var}_sd = pd.DataFrame([{var}.importances_std], columns={c}_names)\n\
         {var}_repeats = pd.DataFrame({var}.importances.T, columns={c}_names)\n\
         {var}_repeats.insert(0, \"repeat\", np.arange(1, len({var}_repeats) + 1))\n\
         {var}_score = {m}.score({c}_d[{c}_names], {c}_d[{column}])\n"
    );
    let outs = HashMap::from([
        ("importance".to_string(), format!("{var}_importance")),
        ("sd".to_string(), format!("{var}_sd")),
        ("repeats".to_string(), format!("{var}_repeats")),
        ("score".to_string(), format!("{var}_score")),
    ]);
    Ok((code, outs, Need::Nothing))
}

fn partial_dependence(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["model", "table", "predictor", "grid_resolution", "percentiles"])?;
    let m = fitted(n, sc)?;
    let t = need(n, "table", sc)?;
    let predictor = setting(n, "predictor", sc)?.ok_or("its predictor is not given")?;
    let c = format!("_{var}");
    let mut args = vec![format!("[{predictor}]")];
    args.extend(given(n, &[("grid_resolution", "grid_resolution")], sc)?);
    if let Some(p) = setting(n, "percentiles", sc)? {
        args.push(format!("percentiles=tuple({p})"));
    }
    // (the grid, then the mean prediction: the value, or the probability of each class, the second of two)
    let code = format!(
        "import pandas as pd\n\
         from sklearn.inspection import partial_dependence\n\
         {c}_names = list({m}.feature_names_in_)\n\
         {c}_pd = partial_dependence({m}, {t}[{c}_names].dropna(), {}, kind=\"average\", method=\"brute\")\n\
         {c}_classes = list(getattr({m}, \"classes_\", []))\n\
         {c}_shown = {c}_classes[1:] if len({c}_classes) == 2 else {c}_classes\n\
         {c}_cols = [f\"p_{{{LABEL}}}\" for _c in {c}_shown] or [\"value\"]\n\
         {var} = pd.DataFrame({{{predictor}: {c}_pd[\"grid_values\"][0], **dict(zip({c}_cols, np.atleast_2d({c}_pd[\"average\"])))}})\n",
        args.join(", ")
    );
    Ok((code, HashMap::from([("table".to_string(), var.to_string())]), Need::Nothing))
}

// -- the basedata ---------------------------------------------------------------------------------------------

/// The groups' columns of a table of results: their number, then their coordinates, as Terraflux's tables have them.
fn group_table(var: &str, bd: &str, df: &str, out: &str) -> String {
    format!(
        "_{var}_g = [{bd}.group_index(g) for g in {df}.index]\n{out} = {df}.reset_index(drop=True)\n[{out}.insert(i, [\"x\", \"y\", \"z\"][i] if i < 3 else f\"axis_{{i}}\", {bd}.coords[_{var}_g, i]) for i in range({bd}.n_axes)]\n{out}.insert(0, \"group\", np.array(_{var}_g) + 1)\n"
    )
}

/// The `definition_query` as the library takes it: none when empty.
fn query(n: &Node, k: &str, sc: &Scope) -> Result<Option<String>, String> {
    match n.params.get(k) {
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        _ => setting(n, k, sc),
    }
}

/// The randomisations of a spatial analysis: the analysis itself, then the library's `randomise` of its result.
fn randomise(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    const OPTIONS: &[(&str, &str)] = &[
        ("richness_multiplier", "richness_multiplier"),
        ("richness_addition", "richness_addition"),
        ("spatial_allocation_order", "spatial_allocation_order"),
        ("label_allocation_backtracking", "label_allocation_backtracking"),
        ("spatial_allocation_reseed_prob", "spatial_allocation_reseed_prob"),
        ("swap_count", "swap_count"),
        ("max_swap_attempts", "max_swap_attempts"),
        ("stop_on_all_swapped", "stop_on_all_swapped"),
        ("use_max_swap", "use_max_swap"),
        ("labels_not_to_randomise", "labels_not_to_randomise"),
        ("randomise_group_props_by", "randomise_group_props_by"),
        ("randomise_trees_by", "randomise_trees_by"),
    ];
    const TEXTS: &[(&str, &str)] = &[
        ("spatial_conditions_for_label_allocation", "spatial_conditions_for_label_allocation"),
        ("spatial_conditions_for_seed_location", "spatial_conditions_for_seed_location"),
        ("spatial_conditions_for_subset", "spatial_conditions_for_subset"),
        // (the randomisation's own query: the library calls it definition_query, the analysis's the same)
        ("subset_definition_query", "definition_query"),
    ];
    let mut known = vec![
        "basedata",
        "tree",
        "matrix",
        "conditions",
        "calculations",
        "definition_query",
        "seed",
        "function",
        "iterations",
        "rand_seed",
    ];
    known.extend(OPTIONS.iter().chain(TEXTS).map(|(k, _)| *k));
    refuse_others(n, &known)?;
    let bd = need(n, "basedata", sc)?;
    let conditions = setting(n, "conditions", sc)?.ok_or("give the conditions")?;
    let calculations = setting(n, "calculations", sc)?.ok_or("give the calculations")?;
    let mut analysis = vec![bd.clone(), conditions, calculations];
    if let Some(q) = query(n, "definition_query", sc)? {
        analysis.push(format!("definition_query={q}"));
    }
    for k in ["tree", "matrix"] {
        if let Some(x) = value_of(n, k, sc)? {
            analysis.push(format!("{k}={x}"));
        }
    }
    analysis.push(format!("prng_seed={}", setting(n, "seed", sc)?.unwrap_or_else(|| "0".into())));
    let function = setting(n, "function", sc)?.unwrap_or_else(|| "\"rand_structured\"".into());
    let mut args = vec![
        format!("{var}_analysis"),
        format!("function={function}"),
        format!("iterations={}", setting(n, "iterations", sc)?.unwrap_or_else(|| "99".into())),
        format!("seed={}", setting(n, "rand_seed", sc)?.unwrap_or_else(|| "0".into())),
    ];
    args.extend(given(n, OPTIONS, sc)?);
    for (ours, theirs) in TEXTS {
        if let Some(v) = query(n, ours, sc)? {
            args.push(format!("{theirs}={v}"));
        }
    }
    let mut code = format!(
        "{var}_analysis = tf.bio.spatial_analysis({})\n{var} = tf.bio.randomise({})\n",
        analysis.join(", "),
        args.join(", ")
    );
    code.push_str(&group_table(var, &bd, &format!("{var}_analysis.table"), &format!("{var}_table")));
    let mut outs = HashMap::new();
    let mut names = vec![("table", format!("{var}_table"))];
    for (port, method) in [("p_rank", "p_rank"), ("z_scores", "z_scores"), ("comparison", "comparison")] {
        let out = format!("{var}_{port}");
        code.push_str(&format!(
            "_{out}_raw = {var}.{method}()
"
        ));
        code.push_str(&group_table(&out, &bd, &format!("_{out}_raw"), &out));
        names.push((port, out));
    }
    // the maps of the p-ranks (and of CANAPE with pe and phylo_rpe2), when the groups are cells of two axes
    let mut layers =
        format!("{{f\"P_RANK_{{c}}\": {bd}.to_raster(_{var}_p_rank_raw[c]) for c in _{var}_p_rank_raw.columns}}");
    let canape = match n.params.get("calculations") {
        Some(Value::Array(a)) => ["pe", "phylo_rpe2"].iter().all(|k| a.iter().any(|v| v.as_str() == Some(k))),
        _ => false,
    };
    if canape {
        let out = format!("{var}_canape");
        code.push_str(&format!(
            "_{out}_raw = {var}.canape()
"
        ));
        code.push_str(&group_table(&out, &bd, &format!("_{out}_raw"), &out));
        names.push(("canape", out.clone()));
        layers = format!("{{**{layers}, \"CANAPE_CODE\": {bd}.to_raster(_{out}_raw[\"CANAPE_CODE\"])}}");
    }
    code.push_str(&format!(
        "{var}_maps = _stack({layers}) if {bd}.n_axes == 2 else None
"
    ));
    names.push(("maps", format!("{var}_maps")));
    for (port, e) in names {
        outs.insert(port.to_string(), e);
    }
    Ok((code, outs, Need::Flow))
}

/// A column of a delimited text by its name or its 0-based position: its position (a name's from the header `h`).
fn column_at(v: &Value, h: &str) -> Result<String, String> {
    match v {
        Value::Number(i) => Ok(i.to_string()),
        Value::String(s) => Ok(format!("{h}.index({})", text(s))),
        other => Err(format!("the column {}: not in the script yet", literal(other))),
    }
}

/// The element's and the properties' columns of a properties file, read into the basedata's labels or groups.
fn properties(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["basedata", "path", "of", "element_columns", "property_columns", "separator", "quote"])?;
    let bd = need(n, "basedata", sc)?;
    let path = setting(n, "path", sc)?.ok_or("its path is not given")?;
    let of = match choice(n, "of").unwrap_or("labels") {
        "labels" => "set_label_properties",
        "groups" => "set_group_properties",
        other => return Err(format!("of: labels or groups, not {other:?}")),
    };
    let list = |k: &str| -> Result<Vec<Value>, String> {
        match n.params.get(k) {
            Some(Value::Array(a)) => Ok(a.clone()),
            Some(v) if terraflux_graph::setting_ref(v).is_some() => {
                Err(format!("its {k} as a reference: not in the script yet"))
            }
            Some(v @ (Value::String(_) | Value::Number(_))) => Ok(vec![v.clone()]),
            _ => Err(format!("give the {k}")),
        }
    };
    let h = format!("_{var}_hdr");
    let element: Vec<String> = list("element_columns")?.iter().map(|v| column_at(v, &h)).collect::<Result<_, _>>()?;
    // (each property is named by its header)
    let properties: Vec<String> = list("property_columns")?
        .iter()
        .map(|v| match v {
            Value::Number(i) => Ok(format!("{h}[{i}]: {i}")),
            Value::String(s) => Ok(format!("{}: {h}.index({})", text(s), text(s))),
            other => Err(format!("the column {}: not in the script yet", literal(other))),
        })
        .collect::<Result<_, _>>()?;
    let sep = setting(n, "separator", sc)?;
    let quote = setting(n, "quote", sc)?;
    let header_sep = match &sep {
        Some(s) => format!("{{\"tab\": \"\\t\", \"space\": \" \", \"guess\": None}}.get({s}, {s})"),
        None => "None".into(),
    };
    let mut args = vec![
        path.clone(),
        format!("element_columns=[{}]", element.join(", ")),
        format!("property_columns={{{}}}", properties.join(", ")),
    ];
    args.extend(sep.map(|s| format!("sep={s}")));
    args.extend(quote.map(|q| format!("quote={q}")));
    let code = format!(
        "import copy\nimport pandas as pd\n\
         # (the properties' names are the header's: read here as pandas reads a delimited text)\n\
         {h} = [str(k) for k in pd.read_csv({path}, sep={header_sep}, engine=\"python\", nrows=0).columns]\n\
         {var} = copy.deepcopy({bd})\n\
         {var}.{of}(tf.bio.read_properties({}))\n",
        args.join(", ")
    );
    Ok((code, HashMap::from([("basedata".to_string(), var.to_string())]), Need::Nothing))
}

// -- landscape metrics of sample plots ------------------------------------------------------------------------

/// The metrics of plots: the library's `sample_metrics` (`scale_sample` for several sizes), which gives the long table
/// of the metrics (a row per plot, class or patch and metric) as the nodes of the metrics do.
fn sample(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let map = need(n, "map", sc)?;
    let plots = need(n, "plots", sc)?;
    let several = matches!(n.params.get("size"), Some(Value::Array(_)));
    let mut args = vec![map, plots.clone()];
    let mut code = String::new();
    let mut skipped = Vec::new();
    for (k, x) in &n.params {
        let v = || setting(n, k, sc).map(|s| s.unwrap_or_else(|| literal(x)));
        match k.as_str() {
            "block_rows" => skipped.push("block_rows: out of core, the same result"),
            "id_column" if several => return Err("its id_column with several sizes: not in the script yet".into()),
            "all_classes" if several => return Err("its all_classes with several sizes: not in the script yet".into()),
            "id_column" => args.push(format!("plot_id={plots}[{}].to_numpy()", v()?)),
            "level" | "size" | "shape" | "all_classes" => args.push(format!("{k}={}", v()?)),
            "metrics" => args.push(format!("metric={}", v()?)),
            // (the library's types end in " metric": "diversity metric")
            "type" if x.as_str().is_some_and(|t| !t.ends_with(" metric")) => {
                args.push(format!("type={}", text(&format!("{} metric", x.as_str().unwrap_or("")))))
            }
            _ => args.push(format!("{k}={}", v()?)),
        }
    }
    for s in skipped {
        code.push_str(&format!("# (not passed: {s})\n"));
    }
    let f = if several { "tf.scale_sample" } else { "tf.sample_metrics" };
    code.push_str(&format!("{var} = {f}({})\n", args.join(", ")));
    Ok((code, HashMap::from([("table".to_string(), var.to_string())]), Need::Nothing))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The script parses as Python (when there is a Python to ask).
    fn parses(t: &str) {
        if let Ok(o) = std::process::Command::new("python")
            .args(["-I", "-c", "import ast, sys; ast.parse(sys.stdin.read())"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.take().unwrap().write_all(t.as_bytes())?;
                c.wait_with_output()
            })
        {
            assert!(o.status.success(), "{}\n{t}", String::from_utf8_lossy(&o.stderr));
        }
    }

    fn script_of(toml: &str) -> Script {
        let m = Model::from_toml(toml).unwrap();
        script(&m, &Registry::standard(), None)
    }

    #[test]
    fn a_forest_fitted_predicted_and_inspected() {
        let s = script_of(
            r#"
format = "terraflux/1"

[[nodes]]
id = "rec"
op = "table.of"
params = { columns = ["a", "b", "target"], rows = [[1, 2, 0], [2, 1, 1], [3, 5, 0], [4, 3, 1]] }

[[nodes]]
id = "fit"
op = "ml.fit"
inputs = { table = "rec.table" }
params = { n_estimators = 20, sample = 3, seed = 2, max_features = "all" }

[[nodes]]
id = "pred"
op = "ml.predict"
inputs = { model = "fit.model", table = "rec.table" }

[[nodes]]
id = "imp"
op = "ml.importance"
inputs = { model = "fit.model", table = "rec.table" }
params = { n_repeats = 3 }

[[nodes]]
id = "pd"
op = "ml.partial_dependence"
inputs = { model = "fit.model", table = "rec.table" }
params = { predictor = "a", grid_resolution = 10, percentiles = [0.1, 0.9] }
"#,
        );
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(t.contains("_fit_d = rec[_fit_cols + [\"target\"]].dropna()"), "{t}");
        assert!(t.contains("_fit_cls = _fit_y.nunique() <= 50 and bool((_fit_y % 1 == 0).all())"), "{t}");
        assert!(t.contains("_fit_rng = np.random.default_rng(2)"), "{t}");
        assert!(
            t.contains("fit = (RandomForestClassifier if _fit_cls else RandomForestRegressor)(n_estimators=20, max_features=None, random_state=0).fit(_fit_X, _fit_y)"),
            "{t}"
        );
        assert!(
            t.contains("fit_importance = pd.DataFrame([fit.feature_importances_], columns=fit.feature_names_in_)"),
            "{t}"
        );
        assert!(t.contains("pred.loc[_pred_ok, \"prediction\"] = fit.predict(_pred_X[_pred_ok])"), "{t}");
        assert!(
            t.contains(
                "imp = permutation_importance(fit, _imp_d[_imp_names], _imp_d[\"target\"], n_repeats=3, random_state=0)"
            ),
            "{t}"
        );
        assert!(t.contains("_pd_pd = partial_dependence(fit, rec[_pd_names].dropna(), [\"a\"], grid_resolution=10, percentiles=tuple([0.1, 0.9]), kind=\"average\", method=\"brute\")"), "{t}");
        parses(t);
    }

    #[test]
    fn the_maps_a_stratified_sample_and_a_boosting() {
        let s = script_of(
            r#"
format = "terraflux/1"

[[nodes]]
id = "a"
op = "nlm.random"

[[nodes]]
id = "b"
op = "nlm.random"

[[nodes]]
id = "x"
op = "stack.of"
inputs = { temp = "a.map", rain = "b.map" }

[[nodes]]
id = "fit"
op = "ml.fit"
inputs = { predictors = "x.stack", target = "a.map", cells = "b.map" }
params = { method = "gradient_boosting", task = "classes", sample = 0.5, stratify = true, max_iter = 50 }

[[nodes]]
id = "pred"
op = "ml.predict"
inputs = { model = "fit.model", predictors = "x.stack" }
"#,
        );
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(t.contains("_fit_cols = sorted(str(b) for b in x[\"band\"].values)"), "{t}");
        assert!(t.contains("_fit_ok &= ~np.isnan(_fit_cells) & (_fit_cells != 0)"), "{t}");
        assert!(t.contains("for _fit_v in np.unique(_fit_y):"), "{t}");
        assert!(
            t.contains("fit = HistGradientBoostingClassifier(max_iter=50, early_stopping=False, random_state=0).fit(_fit_X, _fit_y)"),
            "{t}"
        );
        assert!(t.contains("# (importance: the share of the gain"), "{t}");
        assert!(!t.contains("_fit_cls"), "{t}");
        assert!(t.contains("pred_probability = _pred_tpl.expand_dims(band=list(fit.classes_))"), "{t}");
        parses(t);
    }

    #[test]
    fn a_model_per_class_is_not_a_model_to_inspect() {
        let s = script_of(
            r#"
format = "terraflux/1"

[[nodes]]
id = "a"
op = "nlm.random"

[[nodes]]
id = "pot"
op = "ml.potential"
inputs = { landscape = "a.map", drivers = "a.map" }

[[nodes]]
id = "pred"
op = "ml.predict"
inputs = { model = "pot.model", predictors = "a.map" }
"#,
        );
        assert_eq!(s.left.len(), 1, "{:?}", s.left);
        assert!(s.left[0].2.contains("one model per class"), "{:?}", s.left);
    }

    #[test]
    fn the_basedata_randomised_and_given_properties() {
        let s = script_of(
            r#"
format = "terraflux/1"

[[nodes]]
id = "t"
op = "bio.table"
params = { path = "data/d.csv", group_columns = ["x", "y"], label_columns = "sp", cell_sizes = [1, 1] }

[[nodes]]
id = "props"
op = "bio.properties"
inputs = { basedata = "t.basedata" }
params = { path = "data/p.csv", element_columns = ["genus", 1], property_columns = ["mass", 3], separator = "tab" }

[[nodes]]
id = "rnd"
op = "bio.randomise"
inputs = { basedata = "props.basedata" }
params = { conditions = "sp_self_only()", calculations = ["richness", "pe", "phylo_rpe2"], function = "rand_csr_by_group", iterations = 9, rand_seed = 4, subset_definition_query = "$x > 1" }
"#,
        );
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(
            t.contains("_props_hdr = [str(k) for k in pd.read_csv(MODEL_DIR / \"data/p.csv\", sep={\"tab\": \"\\t\""),
            "{t}"
        );
        assert!(t.contains("props = copy.deepcopy(t)\nprops.set_label_properties(tf.bio.read_properties(MODEL_DIR / \"data/p.csv\", element_columns=[_props_hdr.index(\"genus\"), 1], property_columns={\"mass\": _props_hdr.index(\"mass\"), _props_hdr[3]: 3}, sep=\"tab\"))"), "{t}");
        assert!(
            t.contains("rnd_analysis = tf.bio.spatial_analysis(props, \"sp_self_only()\", [\"richness\", \"pe\", \"phylo_rpe2\"], prng_seed=0)"),
            "{t}"
        );
        assert!(
            t.contains("rnd = tf.bio.randomise(rnd_analysis, function=\"rand_csr_by_group\", iterations=9, seed=4, definition_query=\"$x > 1\")"),
            "{t}"
        );
        assert!(t.contains("rnd.canape()") && t.contains("\"CANAPE_CODE\""), "{t}");
        parses(t);
    }

    #[test]
    fn the_metrics_of_the_plots() {
        let s = script_of(
            r#"
format = "terraflux/1"

[[nodes]]
id = "lc"
op = "nlm.random"

[[nodes]]
id = "pts"
op = "vector.read"
params = { path = "data/s.gpkg" }

[[nodes]]
id = "one"
op = "metrics.sample"
inputs = { map = "lc.map", plots = "pts.vector" }
params = { level = "class", size = 500, shape = "circle", metrics = ["area", "ed"], all_classes = true, id_column = "id", block_rows = 100 }

[[nodes]]
id = "many"
op = "metrics.sample"
inputs = { map = "lc.map", plots = "pts.vector" }
params = { size = [200, 400], type = "shape" }
"#,
        );
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(
            t.contains("one = tf.sample_metrics(lc, pts, all_classes=True, plot_id=pts[\"id\"].to_numpy(), level=\"class\", metric=[\"area\", \"ed\"], shape=\"circle\", size=500)"),
            "{t}"
        );
        assert!(t.contains("# (not passed: block_rows: out of core, the same result)"), "{t}");
        assert!(t.contains("many = tf.scale_sample(lc, pts, size=[200, 400], type=\"shape metric\")"), "{t}");
        parses(t);
    }

    #[test]
    fn the_operators_the_library_does_not_have() {
        let s = script_of(
            r#"
format = "terraflux/1"

[[nodes]]
id = "a"
op = "nlm.random"

[[nodes]]
id = "prox"
op = "distance.proximity"
inputs = { raster = "a.map" }
params = { values = 1 }

[[nodes]]
id = "mos"
op = "raster.mosaic"
inputs = { raster = "a.map" }
"#,
        );
        assert_eq!(s.left.len(), 2, "{:?}", s.left);
        assert!(s.left[0].2.contains("gdal_proximity") && s.left[1].2.contains("gdalwarp"), "{:?}", s.left);
    }

    #[test]
    fn objects_and_the_synthetic_image_have_no_equivalent() {
        let s = script_of(
            r#"
format = "terraflux/1"

[[nodes]]
id = "a"
op = "nlm.random"

[[nodes]]
id = "img"
op = "nlm.image"
inputs = { classes = "a.map" }

[[nodes]]
id = "seg"
op = "segment.chessboard"
inputs = { image = "a.map" }

[[nodes]]
id = "poly"
op = "objects.to_vector"
inputs = { objects = "seg.objects" }
"#,
        );
        let mut left: Vec<&str> = s.left.iter().map(|(id, _, _)| id.as_str()).collect();
        left.sort();
        assert_eq!(left, ["img", "poly", "seg"], "{:?}", s.left);
        assert!(s.left.iter().all(|(_, _, why)| why.contains("no equivalent")), "{:?}", s.left);
        assert!(s.text.contains("# not translated: objects (object-based"), "{}", s.text);
    }
}
