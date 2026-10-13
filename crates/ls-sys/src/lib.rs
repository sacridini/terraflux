//! Landschaft's C++ kernels of land change simulation, for Terraflux.
//!
//! [`ffi`] is the cxx bridge to the shim (`shim/shim.cc`) over landschaft's
//! `simcore`, `lsmcore`, `nlmcore` and `gtbcore` libraries; the functions of this crate
//! wrap it with Rust types. Maps are row-major `f64` cells with NaN for
//! missing ones; matrices of cells x classes are row-major (cell `i`, class
//! `j` at `i * ncode + j`).
//!
//! The kernels are the same code that landschaft's Python package runs, so
//! a model run here gives the same bits as `ls.simulate` with the same seed.

#[allow(clippy::too_many_arguments)] // the kernels take their settings one by one
#[cxx::bridge(namespace = "tfx")]
pub mod ffi {
    struct CluesOut {
        allocation: Vec<f64>,
        iterations: i32,
        converged: bool,
        max_diff: f64,
        mean_diff: f64,
    }

    struct OrderedOut {
        allocation: Vec<f64>,
        iterations: i32,
        converged: bool,
        short_cells: i64,
    }

    struct NbRule {
        cls: f64,
        threshold: f64,
        wy: i32,
        wx: i32,
        weights: Vec<f64>,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct GlmFit {
        coefficients: Vec<f64>,
        pivot: Vec<i32>,
        rank: i32,
        iterations: i32,
        converged: bool,
        boundary: bool,
        deviance: f64,
        null_deviance: f64,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct WoeWeights {
        plus: f64,
        contrast: f64,
        significant: bool,
    }

    struct PatchSpec {
        from: i32,
        to: i32,
        probability: Vec<f64>,
        expand: i64,
        patch: i64,
        e_mean: f64,
        e_var: f64,
        e_iso: f64,
        p_mean: f64,
        p_var: f64,
        p_iso: f64,
    }

    struct PatchOut {
        landscape: Vec<i32>,
        expanded: Vec<i64>,
        patched: Vec<i64>,
        patches: Vec<i64>,
    }

    struct SimilarityOut {
        first: Vec<f32>,
        second: Vec<f32>,
        first_mean: f64,
        second_mean: f64,
    }

    struct CarsOut {
        landscape: Vec<i32>,
        iterations: i64,
        converged: bool,
        decays: i64,
        /// the cells of each region x class after the last iteration
        last: Vec<i64>,
    }

    /// GuidosToolbox's FAD at one window: the density (0-100 on the foreground, GTB's codes 101 background, 102
    /// missing, 105 and 106 special backgrounds elsewhere), the same density everywhere, the classes 1 (rare) to
    /// 6 (intact), the foreground's cells and those of each class, the density summed over the foreground.
    struct FadOut {
        density: Vec<u8>,
        raw: Vec<u8>,
        classes: Vec<u8>,
        foreground: i64,
        class_counts: Vec<i64>,
        density_sum: i64,
    }

    /// FAD at several windows: the density at each, one map after the other, their average and its classes;
    /// rows of the window (0 for the average), the shares of intact, interior, dominant, transitional, patchy
    /// and rare foreground, and the average density.
    struct FadScalesOut {
        maps: Vec<u8>,
        multiscale: Vec<u8>,
        classes: Vec<u8>,
        stats: Vec<f64>,
    }

    /// SPA's classes and the objects of the foreground, cores, margins, core-openings, edges, perforations, islets
    /// and contiguous foreground (-1 where the variant has none).
    struct SpaOut {
        map: Vec<u8>,
        counts: Vec<i64>,
    }

    /// GTB's Accounting: the thresholds used, the size classes in GTB's colour codes, the objects' ids and each
    /// cell's object's area (0 background, -1 missing, -3 and -4 special backgrounds), the area of each object.
    struct AccountingOut {
        thresholds: Vec<i64>,
        classes: Vec<u8>,
        ids: Vec<i32>,
        area: Vec<i32>,
        object_area: Vec<i64>,
        /// the ids of the largest, second and third largest objects (ties share a rank): per rank its count, then
        /// its ids
        largest: Vec<i64>,
    }

    /// GTB's spatial joint entropy of a map: the index (% of the most) and the counts of each pair of neighbours.
    struct EntropyOut {
        index: f64,
        histogram: Vec<u64>,
    }

    /// The cells of each class of a landscape mosaic of 103 classes, and their % of the cells with data.
    struct MosaicStatsOut {
        codes: Vec<i32>,
        counts: Vec<u64>,
        percentages: Vec<f64>,
    }

    /// Parcellation: rows of the class, its parcels, cells, APS, AWAPS, AWAPS over the data, division and PARC;
    /// the whole map's parcels, cells with data, APS, AWAPS, division and PARC; each cell's parcel.
    struct ParcellationOut {
        classes: Vec<f64>,
        global: Vec<f64>,
        labels: Vec<u32>,
    }

    /// A change between two maps: its map in GTB's codes, the counts and values of its report (in the order the
    /// function says), and a matrix of classes where it has one.
    struct ChangeOut {
        map: Vec<u8>,
        counts: Vec<i64>,
        values: Vec<f64>,
        matrix: Vec<i64>,
    }

    /// GTB's Euclidean distance: signed (positive in the foreground), GTB's byte map, and the foreground's cells,
    /// mean and largest distance, then the background's.
    struct DistanceOut {
        distance: Vec<f32>,
        bytes: Vec<u8>,
        stats: Vec<f64>,
    }

    /// GTB's Influence Zones or Proximity: the classes (GTB's viewport codes), the object ids, the zones, the
    /// proximity distance and connected area gain (proximity), the objects' areas (index 0 the rest), their count;
    /// the watershed lines (proximity) as x, y, nbr, clength, cag, k, then k ids and k areas; the least proximity
    /// and its cells (-1 without lines).
    struct InfluenceOut {
        classes: Vec<u8>,
        ids: Vec<i32>,
        zones: Vec<i32>,
        distance: Vec<i32>,
        cag: Vec<i64>,
        object_area: Vec<i64>,
        n_objects: i32,
        lines: Vec<i64>,
        prox_min: i64,
        prox_min_count: i64,
    }

    /// GTB's least-cost path between two markers: the cost from A plus from B, the path (1), its length, its cells
    /// in the background, their resistance and mean distance (with a distance map).
    struct LcpOut {
        cost: Vec<f32>,
        path: Vec<u8>,
        path_length: i64,
        rest_pixels: i64,
        expense: f64,
        avg_distance: f64,
    }

    /// The secant bound of the total intactness: u0 + sum_i sigma_i t_i.
    struct SecantOut {
        u0: f64,
        sigma: Vec<f64>,
    }

    /// The options of the landscape metrics, as landschaft's (and landscapemetrics'): NaN for a distance or a
    /// number not given; contrast and similarity weights as (class a, class b, weight) one after the other.
    #[derive(Debug, Clone, PartialEq)]
    struct LsmOptions {
        directions: i32,
        count_boundary: bool,
        consider_boundary: bool,
        edge_depth: i32,
        cell_center: bool,
        classes_max: f64,
        neighbourhood: i32,
        ordered: bool,
        base: String,
        fragstats: bool,
        search_radius: f64,
        connect_threshold: f64,
        boundary_contrast: f64,
        contrast: Vec<f64>,
        similarity: Vec<f64>,
    }

    /// Landscape metrics as landscapemetrics' calculate_lsm: a row per value, by metric (in the order asked),
    /// class and patch; the class and the patch flagged when the row has none. The map's classes; with the
    /// labels asked, each cell's patch (1.., 0 none) and each patch's class (an index into the classes).
    struct LsmTable {
        class_values: Vec<i32>,
        metric: Vec<i16>,
        cls: Vec<i32>,
        cls_na: Vec<u8>,
        id: Vec<i32>,
        id_na: Vec<u8>,
        value: Vec<f64>,
        labels: Vec<i32>,
        patch_class: Vec<i32>,
    }

    /// plangea's original vegetation of the pixels with land: the share of every natural class originally and
    /// that restored land returns to (k x n), where it came from (0 past cover, 1 the ecoregion's, 2 the
    /// failsafe), and the failsafe shares (k).
    struct VegetationOut {
        original: Vec<f64>,
        restore: Vec<f64>,
        source: Vec<u8>,
        fill: Vec<f64>,
    }

    /// The current and potential area of habitat of every species.
    struct HabitatOut {
        now: Vec<f64>,
        potential: Vec<f64>,
    }

    /// The habitat each species gains per unit of a pixel restored: CSR rows by species.
    struct GainOut {
        indptr: Vec<i64>,
        index: Vec<i64>,
        gain: Vec<f64>,
    }

    /// Zonation's statistics of the layers: the cells seen, and per layer its zeros, negative values, cells
    /// without data, its largest value (after the thresholds) and its largest one before them.
    struct PrioStatsOut {
        pixels: i64,
        zeros: Vec<i64>,
        negative: Vec<i64>,
        nodata: Vec<i64>,
        max_values: Vec<f32>,
        orig_max_values: Vec<f32>,
    }

    /// Zonation's settings of a ranking: the rule (ABF, CAZ1, CAZ2, CAZMAX, CAZP, CAZPW, RAND, LOAD) and its
    /// parameter, per layer the weight and ABF's z (NaN: the parameter), and the iterations' settings.
    struct PrioSettings {
        rule: String,
        param: f64,
        weights: Vec<f32>,
        abf_z: Vec<f32>,
        cost_in_ranking: bool,
        max_iterations: i32,
        ranking_error: f32,
        stop_on_plateau: bool,
        smoothing: f32,
        curves_interval: f32,
        seed: u64,
    }

    /// Zonation's ranking: the cells with data (their index in the map), the rank of each and the order they
    /// were removed in, the iterations and the ranking error (a fraction) of each; the curves (the rank at each
    /// sample, the remaining level of every layer (samples x layers), the cost), the AUC and total cost; per
    /// layer its sum and the quantization's error (%).
    struct PrioRankOut {
        cell_index: Vec<i64>,
        rank: Vec<f32>,
        order: Vec<u32>,
        iterations: i32,
        ranking_error: f64,
        history_error: Vec<f64>,
        curve_rank: Vec<f32>,
        curve_levels: Vec<f32>,
        curve_cost: Vec<f64>,
        auc: f64,
        total_cost: f64,
        curve_interval: i64,
        orig_abs_sums: Vec<f64>,
        quantization_errors: Vec<f64>,
    }

    /// Zonation's data: its bits; per layer the sums of the normalized and of the input values, the scale of the
    /// quantized values, the initial levels and the quantization errors (%).
    struct PrioDataInfo {
        bits: i32,
        max_norm_sums: Vec<f64>,
        orig_abs_sums: Vec<f64>,
        layer_scale: Vec<f64>,
        initial_levels: Vec<f64>,
        quantization_errors: Vec<f64>,
    }

    /// The cells of zones (polygons) as runs of one row, [x0, x1) in row y; zone z owns the runs offsets[z] ..
    /// offsets[z + 1].
    struct RunsOut {
        offsets: Vec<i64>,
        y: Vec<i32>,
        x0: Vec<i32>,
        x1: Vec<i32>,
    }

    /// Zonation's flexibility map per data cell: how far up and down the order a cell could move.
    struct PrioFlexOut {
        up: Vec<f32>,
        down: Vec<f32>,
    }

    /// Marxan's problem: per planning unit its cost and status (0 available, 1 initially in, 2 locked in, 3
    /// locked out), the features in it (offsets into spindex and amount, by unit), per feature its target,
    /// proportion target, SPF (below 0: Marxan's default) and occurrence target; per unit its fixed (external)
    /// boundary and its neighbours with their shared boundary (offsets into nb and nb_cost).
    struct MarxanData {
        cost: Vec<f64>,
        status: Vec<i32>,
        offset: Vec<i64>,
        spindex: Vec<i32>,
        amount: Vec<f64>,
        target: Vec<f64>,
        prop: Vec<f64>,
        spf: Vec<f64>,
        targetocc: Vec<i32>,
        fixedcost: Vec<f64>,
        nb_start: Vec<i64>,
        nb: Vec<i32>,
        nb_cost: Vec<f64>,
    }

    /// Marxan's input.dat settings.
    struct MarxanOptions {
        blm: f64,
        prop: f64,
        iterations: i64,
        tinit: f64,
        tcool: f64,
        titns: i64,
        repeats: i64,
        costthresh: f64,
        tpf1: f64,
        tpf2: f64,
        misslevel: f64,
        runmode: i32,
        heurtype: i32,
        itimptype: i32,
        connectivity_in: bool,
        seed: u32,
    }

    /// Marxan's runs: the reserves (runs x units: 0 out, 1 in, 2 locked in, 3 locked out), the best run, and per
    /// run the summary's figures (the final score; the cost, penalty and score as updated; the units, the
    /// boundary length, the shortfall, the features missing and the least proportion met); per feature its
    /// penalty and target.
    struct MarxanOut {
        solutions: Vec<i32>,
        best: i32,
        score: Vec<f64>,
        cost: Vec<f64>,
        penalty_total: Vec<f64>,
        incremental_score: Vec<f64>,
        pus: Vec<i64>,
        connectivity: Vec<f64>,
        shortfall: Vec<f64>,
        missing: Vec<i64>,
        mpm: Vec<f64>,
        penalty: Vec<f64>,
        target: Vec<f64>,
        unmet: i32,
    }

    /// ProxIRR's beta and gamma of every unit.
    struct ProxirrOut {
        beta: Vec<f64>,
        gamma: Vec<f64>,
    }

    /// The features x planning units matrix of raster units: every feature's total (of the whole raster, as
    /// terra adds it) and its amounts in the units (compressed rows: start, unit, amount).
    struct FeaturesOut {
        totals: Vec<f64>,
        start: Vec<i64>,
        index: Vec<i64>,
        value: Vec<f64>,
    }

    /// A multi-action problem's inputs as prioriactions's inputData() leaves them (internal ids from 0): the units'
    /// monitoring cost and status, the features' targets, their amounts (unit, feature, amount), the threats'
    /// boundary weights, the actions (unit, threat, amount, cost, status), the sensitivity (feature, threat; the
    /// deltas 4 x rows: delta1 for every row, then delta2...), the boundary (id1, id2, value).
    struct ActionsInput {
        n_units: i32,
        n_features: i32,
        n_threats: i32,
        monitoring_cost: Vec<f64>,
        unit_status: Vec<f64>,
        target_recovery: Vec<f64>,
        target_conservation: Vec<f64>,
        df_unit: Vec<i64>,
        df_feature: Vec<i64>,
        df_amount: Vec<f64>,
        blm_actions: Vec<f64>,
        act_unit: Vec<i64>,
        act_threat: Vec<i64>,
        act_amount: Vec<f64>,
        act_cost: Vec<f64>,
        act_status: Vec<f64>,
        sens_feature: Vec<i64>,
        sens_threat: Vec<i64>,
        sens_delta: Vec<f64>,
        has_boundary: bool,
        bnd_id1: Vec<i64>,
        bnd_id2: Vec<i64>,
        bnd_value: Vec<f64>,
    }

    /// prioriactions's problem(): maximizeBenefits (else minimizeCosts), the boundary's weight, the response
    /// curve (1-3), its pieces and their length, the budget.
    struct ActionsSettings {
        maximize: bool,
        blm: f64,
        curve: i32,
        segments: i32,
        step: f64,
        budget: f64,
    }

    /// prioriactions's model: rows compressed (start, column, value), the objective, the columns' bounds, the
    /// right sides and senses (0 <=, 1 >=, 2 =), the integer columns.
    struct ActionsModel {
        start: Vec<i64>,
        index: Vec<i64>,
        value: Vec<f64>,
        obj: Vec<f64>,
        lb: Vec<f64>,
        ub: Vec<f64>,
        rhs: Vec<f64>,
        sense: Vec<i32>,
        integer: Vec<u8>,
    }

    /// Every feature's distribution (units, threatened units) and its benefit by conservation and recovery: the
    /// most the actions can give, or a solution's.
    struct ActionsBenefit {
        dist: Vec<f64>,
        dist_threatened: Vec<f64>,
        conservation: Vec<f64>,
        recovery: Vec<f64>,
    }

    /// Every (feature, unit) pair in prioriactions's order, and whether a threat in the unit harms the feature.
    struct ActionsPairs {
        feature: Vec<i64>,
        unit: Vec<i64>,
        threatened: Vec<u8>,
    }

    /// A MILP model: its rows compressed (start, column, value), the objective and the columns' bounds, the
    /// rows' bounds (-inf / inf open), the integer columns (1), the sense and a constant.
    struct MilpModel {
        start: Vec<i64>,
        index: Vec<i64>,
        value: Vec<f64>,
        obj: Vec<f64>,
        lb: Vec<f64>,
        ub: Vec<f64>,
        row_lo: Vec<f64>,
        row_hi: Vec<f64>,
        integer: Vec<u8>,
        maximize: bool,
        offset: f64,
    }

    /// The solver's settings: the relative gap, a time limit (seconds; inf none), presolve (choose, off, on),
    /// the LP relaxation only, a solution to start from (empty: none).
    struct SolveSettings {
        gap: f64,
        time_limit: f64,
        presolve: String,
        relaxation: bool,
        start: Vec<f64>,
    }

    /// The solver's result: its status (optimal, time_limit, infeasible...), the solution, its objective, the
    /// bound and the gap.
    struct SolveOut {
        status: String,
        has_solution: bool,
        x: Vec<f64>,
        objective: f64,
        bound: f64,
        gap: f64,
    }

    /// A sparse matrix in compressed columns: start (columns + 1), row, value.
    struct CscOut {
        start: Vec<i64>,
        index: Vec<i64>,
        value: Vec<f64>,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct ObservedPatches {
        cells: i64,
        expansion: f64,
        expansion_patches: i64,
        expansion_mean: f64,
        expansion_variance: f64,
        new_patches: i64,
        new_mean: f64,
        new_variance: f64,
    }

    // simulate bridge (landschaft-bb): MOLUSCE
    #[derive(Debug, Clone, PartialEq)]
    struct MolusceForecastOut {
        prediction: Vec<f64>,
        confidence: Vec<f64>,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct MolusceChange {
        from: f64,
        to: f64,
        cells: i64,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct MolusceStepOut {
        state: Vec<f64>,
        changed: Vec<MolusceChange>,
    }

    /// CLUMondo's settings of a run (landschaft's `ClumondoSettings`; empty vectors for the extensions not used).
    #[derive(Debug, Clone, PartialEq, Default)]
    struct ClumondoSettingsIn {
        nsys: i32,
        ndem: i32,
        allowed: Vec<i32>,
        lusconv: Vec<i32>,
        lusmatrix: Vec<f32>,
        weight: Vec<f32>,
        resistance: Vec<f32>,
        itermode: i32,
        mean_dev: f32,
        max_dev: f32,
        speed: f32,
        max_loops: i32,
        kind: Vec<i32>,
        dev_limit: Vec<f32>,
        nx: i64,
        patch_radius: Vec<i32>,
        patch_threshold: Vec<f32>,
        patch_min: Vec<i32>,
        pair_resistance: Vec<f32>,
        cap_pairs: Vec<u8>,
        cap_cells: Vec<i64>,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct ClumondoYearOut {
        newco: Vec<i32>,
        age: Vec<i32>,
        supply: Vec<f64>,
        cells: Vec<i64>,
        loops: i32,
        converged: bool,
        max_diff: f32,
        mean_diff: f32,
        dem_elas: Vec<f32>,
    }

    /// FUTURES' settings (landschaft's `FuturesSettings`); the patch libraries flat, `patch_offsets` where each
    /// starts (and the end).
    #[derive(Debug, Clone, PartialEq, Default)]
    struct FuturesIn {
        rows: i64,
        cols: i64,
        nregions: i32,
        npot: i32,
        intercept: Vec<f64>,
        devpressure_coef: Vec<f64>,
        neighborhood: i32,
        approach: i32,
        gamma: f32,
        scaling: f32,
        patch_sizes: Vec<i32>,
        patch_offsets: Vec<i32>,
        single_column: bool,
        compactness_mean: f32,
        compactness_range: f32,
        num_neighbors: i32,
        seed_probability: bool,
        incentive: Vec<f32>,
        user_effects: bool,
        zone_ids: Vec<i32>,
        zone_regions: Vec<i32>,
        zone_effects: Vec<f32>,
        stringency: Vec<f32>,
        demand: Vec<f32>,
        nsteps: i32,
        seed: i64,
    }

    /// FUTURES' flood submodel (landschaft's `FuturesClimate`), maps flat (`present` false: none).
    #[derive(Debug, Clone, PartialEq, Default)]
    struct FuturesClimateIn {
        present: bool,
        depth_mode: bool,
        huc: Vec<i32>,
        nhuc: i32,
        huc_ids: Vec<i32>,
        flood_steps: Vec<i32>,
        probabilities: Vec<f32>,
        depths: Vec<f32>,
        flood_probability: Vec<f32>,
        hand: Vec<f32>,
        percentile: f32,
        adaptive_capacity: Vec<f32>,
        adaptation: Vec<i32>,
        levels: Vec<f64>,
        damage: Vec<f64>,
        ddf_region: Vec<i32>,
        response: Vec<f32>,
        stddev: f32,
        has_matrix: bool,
        from_ids: Vec<i32>,
        to_ids: Vec<i32>,
        matrix: Vec<f32>,
        population: Vec<f32>,
        region_ids: Vec<i32>,
    }

    /// A FUTURES run: the developed codes after each step (flat), cells per step and region, the floods.
    #[derive(Debug, Clone, PartialEq, Default)]
    struct FuturesOut {
        series: Vec<i32>,
        converted: Vec<i64>,
        overflow: Vec<i64>,
        warnings: Vec<i32>,
        adaptation: Vec<i32>,
        flood_steps: Vec<i32>,
        flood_hucs: Vec<i32>,
        flood_probabilities: Vec<f32>,
        moved: Vec<f32>,
        demand: Vec<f32>,
    }

    /// A neighbourhood function of CLUMondo (landschaft's `ClumondoNeighbourhood`).
    #[derive(Debug, Clone, PartialEq)]
    struct ClumondoNbIn {
        system: i32,
        weight: f32,
        kernel: Vec<f32>,
        radius: i32,
        from: Vec<u8>,
        constant: f32,
        coef_systems: Vec<i32>,
        coef_values: Vec<f32>,
    }

    /// The weights of one transition code for one factor (MOLUSCE's weights of evidence).
    #[derive(Debug, Clone, PartialEq)]
    struct MolusceFactor {
        categories: Vec<f64>,
        weights: Vec<f64>,
    }

    unsafe extern "C++" {
        include!("terraflux-ls-sys/shim/shim.h");

        /// R's random number generator (Mersenne-Twister, set.seed), as lulcc and landschaft draw from it.
        #[namespace = "nlm"]
        type RRng;

        /// Zonation's quantized feature data.
        #[namespace = "prio"]
        #[cxx_name = "Data"]
        type PrioData;

        fn set_threads(n: i32);
        fn max_threads() -> i32;
        fn rrng_new(seed: i32) -> UniquePtr<RRng>;
        fn rrng_unif_rand(rng: Pin<&mut RRng>) -> f64;
        fn rrng_runif(rng: Pin<&mut RRng>, n: usize) -> Vec<f64>;

        fn map_classes(values: &[f64]) -> Vec<f64>;
        fn crosstab(a: &[f64], b: &[f64], classes_a: &[f64], classes_b: &[f64]) -> Result<Vec<i64>>;
        fn r_sum(x: &[f64]) -> f64;
        fn focal_mean(x: &[f64], ny: i64, nx: i64, w: &[f64], wy: i32, wx: i32) -> Result<Vec<f64>>;
        fn distance_exact(origin: &[u8], ny: i64, nx: i64, res: f64) -> Result<Vec<f64>>;
        fn nlm_random_cluster(seed: i32, ny: i64, nx: i64, p: f64, ai: &[f64], neighbourhood: i32) -> Result<Vec<f64>>;
        fn nlm_gaussian_field(
            seed: i32,
            ny: i64,
            nx: i64,
            autocorr_range: f64,
            mag_var: f64,
            nug: f64,
            mean: f64,
        ) -> Result<Vec<f64>>;
        fn nlm_fbm(seed: i32, ny: i64, nx: i64, fract_dim: f64) -> Result<Vec<f64>>;
        fn mspa(input: &[u8], ny: i64, nx: i64, connectivity: i32, edge_width: i32, internal: bool) -> Result<Vec<u8>>;
        fn fad(input: &[u8], ny: i64, nx: i64, window: i32) -> Result<FadOut>;
        fn fad_multiscale(input: &[u8], ny: i64, nx: i64, windows: &[i32]) -> Result<FadScalesOut>;
        fn fad_density(input: &[u8], ny: i64, nx: i64, window: i32, gtb_codes: bool) -> Result<Vec<u8>>;
        fn clustering_fac(input: &[u8], ny: i64, nx: i64, window: i32, gtb_codes: bool) -> Result<Vec<u8>>;
        fn grayspatcon_fos(input: &[u8], ny: i64, nx: i64, metric: i32, window: i32) -> Result<Vec<u8>>;
        fn spa(input: &[u8], ny: i64, nx: i64, variant: i32) -> Result<SpaOut>;
        fn accounting(
            input: &[u8],
            ny: i64,
            nx: i64,
            connectivity: i32,
            thresholds: &[i64],
            highlight_largest: bool,
        ) -> Result<AccountingOut>;
        fn contagion_pff(input: &[u8], ny: i64, nx: i64, window: i32) -> Result<Vec<u8>>;
        fn spatial_entropy(input: &[u8], ny: i64, nx: i64) -> Result<EntropyOut>;
        fn spatial_entropy_map(input: &[u8], ny: i64, nx: i64, window: i32) -> Result<Vec<f32>>;
        fn landscape_mosaic(input: &[u8], ny: i64, nx: i64, window: i32, classes: i32) -> Result<Vec<u8>>;
        fn landscape_mosaic_stats(lm103: &[u8], ny: i64, nx: i64) -> Result<MosaicStatsOut>;
        fn gray_convolution(
            input: &[u8],
            ny: i64,
            nx: i64,
            metric: &str,
            window: i32,
            mask_missing: bool,
        ) -> Result<Vec<f32>>;
        fn parcellation(input: &[u8], ny: i64, nx: i64, connectivity: i32) -> Result<ParcellationOut>;
        fn contortion(input: &[u8], ny: i64, nx: i64, min_area: i64) -> Result<Vec<u32>>;
        /// stable background, gain, loss, stable foreground, missing, cells with data
        fn simple_change(a: &[u8], b: &[u8], ny: i64, nx: i64) -> Result<ChangeOut>;
        /// the foreground at 1 and 2, the net change, stable background, stable foreground, gain, loss, spurious
        /// and missing cells; the elasticity
        fn morphological_change(a: &[u8], b: &[u8], ny: i64, nx: i64) -> Result<ChangeOut>;
        /// foreground gained and lost, stable background, common foreground, in the same class and in another;
        /// the % of fragmentation decreasing and increasing; the 6 x 6 matrix of the classes (rare..intact)
        fn fos_change(a: &[u8], b: &[u8], ny: i64, nx: i64) -> Result<ChangeOut>;
        /// GTB's Change -> FAD Multiscale counts of one scale between two FAD maps: the 7 x 7 class matrix (background,
        /// rare .. intact), and the foreground at A and B, in both, the sums of the foreground's FAD at A and B, the
        /// cells with data at A and B.
        fn fad_change_counts(a: &[u8], b: &[u8], ny: i64, nx: i64) -> Result<ChangeOut>;
        fn euclidean_distance(input: &[u8], ny: i64, nx: i64) -> Result<DistanceOut>;
        /// GTB's Restoration Status (RSS) of a map coded 0 missing, 1 background, 2 foreground: data area, objects,
        /// habitat (eca_max), the largest object, APS, CNOA, ECA, DOC, the restoration potential, RAC.
        fn restoration_status(input: &[u8], ny: i64, nx: i64) -> Result<Vec<f64>>;
        /// GTB's cost distance from the sources (> 0) across a resistance (0 impassable), 8 neighbours.
        fn cost_distance(resistance: &[u8], sources: &[u8], ny: i64, nx: i64) -> Result<Vec<f32>>;
        /// GTB's least-cost path between markers A and B (> 0), with a distance map (empty: none).
        fn least_cost_path(resistance: &[u8], a: &[u8], b: &[u8], distance: &[f32], ny: i64, nx: i64)
        -> Result<LcpOut>;
        /// GTB's ggeo: the symmetric geodesic time from the cells ref == 1 across the mask (a step p -> q costs
        /// m[p] + m[q]); ref == 3 and the image's frame not crossed; unreached UINT32_MAX.
        fn geodesic_time(mask: &[u8], reference: &[u8], ny: i64, nx: i64) -> Result<Vec<u32>>;
        /// miallib's binOIthin_FIFO: anchored order-independent homotopic thinning of a binary image.
        fn thin_anchored(input: &[u8], stype: i32, anchor: &[u8], ny: i64, nx: i64) -> Result<Vec<u8>>;
        /// IDL's MORPH_DISTANCE (NEIGHBOR_SAMPLING=3) as GTB uses it: quasi-Euclidean, of the zero cells to the
        /// nearest non-zero (background) or of the non-zero to the nearest zero.
        fn morph_distance(mask: &[u8], ny: i64, nx: i64, background: bool) -> Result<Vec<f32>>;
        /// GTB's Influence Zones (proximity false; `buffer` the background buffer's width) or Proximity (`buffer`
        /// the largest proximity marked) of a map coded 0 missing, 1 background, 2 foreground; IDL's quasi-Euclidean
        /// MORPH_DISTANCE.
        fn influence(
            input: &[u8],
            ny: i64,
            nx: i64,
            min_area: i64,
            core_buffer: i32,
            buffer: i32,
            proximity: bool,
        ) -> Result<InfluenceOut>;
        /// The human footprint after restoring the shares `restored` with `reduction` per unit (at least 0).
        fn rest_footprint(hfi: &[f64], reduction: &[f64], restored: &[f64], ny: i64, nx: i64) -> Result<Vec<f64>>;
        fn rest_intactness(
            hfi: &[f64],
            ny: i64,
            nx: i64,
            gamma: f64,
            beta: f64,
            z: f64,
            radius: i32,
            dx: f64,
            dy: f64,
        ) -> Result<Vec<f64>>;
        fn rest_intactness_gradient(
            hfi: &[f64],
            reduction: &[f64],
            ny: i64,
            nx: i64,
            gamma: f64,
            beta: f64,
            z: f64,
            radius: i32,
            dx: f64,
            dy: f64,
        ) -> Result<Vec<f64>>;
        /// A concave upper bound of the total intactness over the plans whose footprint reductions lie in
        /// [tlo, thi] per cell (the secant of every pair of a window).
        fn rest_intactness_secant(
            h0: &[f64],
            tlo: &[f64],
            thi: &[f64],
            ny: i64,
            nx: i64,
            gamma: f64,
            beta: f64,
            z: f64,
            radius: i32,
            dx: f64,
            dy: f64,
        ) -> Result<SecantOut>;
        fn rest_original_vegetation(
            natural: &[f64],
            k: i32,
            anthropic: &[f64],
            m: i32,
            n: i64,
            past: &[f64],
            ecoregion: &[f64],
            domain: &[u8],
            fill: &[u8],
            threshold: f64,
            restore_current: bool,
        ) -> Result<VegetationOut>;
        fn rest_habitat(
            current: &[f64],
            original: &[f64],
            area: &[f64],
            n: i64,
            k: i32,
            range_ptr: &[i64],
            range_idx: &[i64],
            suitable: &[u8],
            ns: i64,
        ) -> Result<HabitatOut>;
        fn rest_habitat_gain(
            restore: &[f64],
            area: &[f64],
            n: i64,
            k: i32,
            range_ptr: &[i64],
            range_idx: &[i64],
            suitable: &[u8],
            ns: i64,
            domain: &[u8],
        ) -> Result<GainOut>;
        fn prio_stats(
            values: &[f32],
            layers: i32,
            ny: i64,
            nx: i64,
            nodata: &[f32],
            extract: &[f32],
            thresholds: &[f32],
            area: &[u8],
        ) -> Result<PrioStatsOut>;
        /// Zonation's quantized data of the layers given (those with data); filled once (finished), then
        /// transformed before the ranking.
        fn prio_data_new(
            nodata: &[f32],
            extract: &[f32],
            thresholds: &[f32],
            max_values: &[f32],
            keep_zeros: &[u8],
            bits: i32,
        ) -> Result<UniquePtr<PrioData>>;
        fn prio_data_set_conditions(data: Pin<&mut PrioData>, condition: &[i32], renormalize: &[u8]) -> Result<()>;
        /// The layers (layers, ny, nx), their conditions (conditions, ny, nx; empty: none), Zonation's blocks of
        /// rows (0: one); the data finished.
        fn prio_data_add(
            data: Pin<&mut PrioData>,
            values: &[f32],
            ny: i64,
            nx: i64,
            area: &[u8],
            conditions: &[f32],
            block_rows: i64,
        ) -> Result<()>;
        /// The cells with data (their index in the map, row-major).
        fn prio_data_cells(data: &PrioData) -> Vec<i64>;
        fn prio_data_info(data: &PrioData) -> PrioDataInfo;
        /// A layer per data cell, quantized or scaled (to the sum-normalized values), `missing` without data.
        fn prio_data_layer(data: &PrioData, layer: i32, scaled: bool, missing: f32) -> Result<Vec<f64>>;
        /// A layer replaced by transformed values per data cell, its level becoming `level`; the quantization
        /// error (%).
        fn prio_data_replace_layer(data: Pin<&mut PrioData>, layer: i32, values: &[f32], level: f64) -> Result<f64>;
        /// Zonation's retention (kind 0 absolute values, 1 fractions per data cell, 2 the fraction everywhere); the
        /// cells clipped.
        fn prio_data_retain(
            data: Pin<&mut PrioData>,
            layer: i32,
            kind: i32,
            values: &[f32],
            fraction: f32,
        ) -> Result<i64>;
        /// Zonation's pre-processing map per data cell: the sum of mult (times weights) times the values,
        /// sum-normalized or scaled back to the inputs.
        fn prio_data_preprocessing(data: &PrioData, mult: &[f32], weights: &[f32], unscaled: bool) -> Result<Vec<f32>>;
        fn prio_data_scale_level(data: Pin<&mut PrioData>, layer: i32, factor: f64) -> Result<()>;
        /// The quantized values of a layer where it has data (truncated to the storage type).
        fn prio_data_set_layer_values(data: Pin<&mut PrioData>, layer: i32, values: &[f32]) -> Result<()>;
        fn prio_rank(
            data: &PrioData,
            settings: &PrioSettings,
            ny: i64,
            nx: i64,
            cost: &[f64],
            hmask: &[f64],
            solution: &[f64],
        ) -> Result<PrioRankOut>;
        #[allow(clippy::too_many_arguments)]
        fn prio_flexibility(
            data: &PrioData,
            settings: &PrioSettings,
            ny: i64,
            nx: i64,
            cost: &[f64],
            hmask: &[f64],
            order: &[u32],
            curve_rank: &[f32],
            curve_levels: &[f32],
            curve_interval: i64,
            max_error: f64,
            up: bool,
            down: bool,
        ) -> Result<PrioFlexOut>;
        /// The cells of polygons on an nx x ny grid (GDAL's geotransform `gt`, no rotation) as GDAL's rasterizer burns
        /// them (centre rule or all touched): geometry g has the rings ring_offsets[geom_rings[g]] ..
        /// ring_offsets[geom_rings[g + 1]] (even-odd over them), ring r the points ring_offsets[r] ..
        /// ring_offsets[r + 1] of xy (x, y pairs).
        fn io_polygon_runs(
            xy: &[f64],
            ring_offsets: &[i64],
            geom_rings: &[i64],
            gt: &[f64],
            nx: i32,
            ny: i32,
            all_touched: bool,
        ) -> Result<RunsOut>;
        /// value[z] burnt into the cells of zone z of out (ny x nx), zone after zone (the later win), or added.
        fn io_burn_runs(out: &mut [f64], ny: i32, nx: i32, runs: &RunsOut, value: &[f64], add: bool) -> Result<()>;
        /// Statistics of every zone and band of values (bands, ny, nx; NaN no data): (zones, bands, stats); the
        /// statistics landschaft's (0 count, 1 nodata, 2 sum, 3 mean, 4 min, 5 max, 6 range, 7 std, 8 var, 9
        /// median, 10 majority, 11 minority, 12 unique, 13 percentile).
        fn io_zonal_stats(
            values: &[f64],
            bands: i32,
            ny: i32,
            nx: i32,
            runs: &RunsOut,
            stats: &[i32],
            percentiles: &[f64],
        ) -> Result<Vec<f64>>;
        /// exactextractr's exact_extract(fun = "sum"): the coverage-weighted sums of every layer (layers, ny, nx)
        /// over every feature (polygons: rings of xy points), features x layers. grid: xmin, ymin, xmax, ymax, dx,
        /// dy.
        #[allow(clippy::too_many_arguments)]
        fn io_exact_sums(
            values: &[f64],
            layers: i64,
            ny: i64,
            nx: i64,
            grid: &[f64],
            xy: &[f64],
            ring_offsets: &[i64],
            polygon_offsets: &[i64],
            feature_offsets: &[i64],
        ) -> Result<Vec<f64>>;
        /// Zonation's (Eigen AVX) sum of doubles, and of floats from an offset of the packets.
        fn prio_eigen_sum(values: &[f64]) -> f64;
        fn prio_eigen_sum_f32(values: &[f32], offset: i64) -> f32;
        fn opt_marxan(data: &MarxanData, options: &MarxanOptions) -> Result<MarxanOut>;
        /// ProxIRR's alpha of every entry (a feature in a unit): its local and global amounts and target.
        fn proxirr_alpha(local: &[f64], global: &[f64], target: &[f64], triage: bool) -> Result<Vec<f64>>;
        fn proxirr_beta_gamma(start: &[i64], alphas: &[f64]) -> Result<ProxirrOut>;
        fn opt_features(values: &[f64], layers: i64, unit: &[i64]) -> Result<FeaturesOut>;
        fn opt_solve(model: &MilpModel, settings: &SolveSettings, method: &str, n_units: i64) -> Result<SolveOut>;
        /// prioritizr's boundary of a solution (rcpp_boundary): the strictly lower triangle of the boundary
        /// matrix in compressed columns, every unit's exposed and total boundary.
        fn eval_boundary(
            edge_factor: f64,
            start: &[i64],
            index: &[i64],
            value: &[f64],
            exposed: &[f64],
            total: &[f64],
            solution: &[f64],
        ) -> Result<f64>;
        /// prioritizr's Ferrier scores (rcpp_ferrier_score) of the selected units: rij features x units in
        /// compressed columns; the scores as features x units in compressed columns.
        fn eval_ferrier(
            start: &[i64],
            index: &[i64],
            value: &[f64],
            nrow: i64,
            targets: &[f64],
            selected: &[u8],
        ) -> Result<CscOut>;
        /// Armadillo's accumulate (two interleaved partial sums).
        fn eval_accumulate(x: &[f64]) -> f64;
        /// prioritizr's connectivity of a solution (rcpp_connectivity, or rcpp_asym_connectivity with asym): the
        /// units' matrix in compressed columns.
        fn eval_connectivity(start: &[i64], index: &[i64], value: &[f64], solution: &[f64], asym: bool) -> Result<f64>;
        /// prioritizr's boundary of a solution with zones (rcpp_boundary): edge factors per zone, zones nz x nz
        /// (row-major), the solution zone after zone.
        #[allow(clippy::too_many_arguments)]
        fn eval_boundary_zones(
            edge_factor: &[f64],
            zones: &[f64],
            start: &[i64],
            index: &[i64],
            value: &[f64],
            exposed: &[f64],
            total: &[f64],
            solution: &[f64],
        ) -> Result<f64>;
        /// prioritizr's connectivity of a solution with zones (asym: the asymmetric one).
        fn eval_connectivity_zones(
            start: &[i64],
            index: &[i64],
            value: &[f64],
            zones: &[f64],
            solution: &[f64],
            asym: bool,
        ) -> Result<f64>;
        /// prioriactions's model (problem()'s matrices) bit for bit.
        fn actions_compile(input: &ActionsInput, settings: &ActionsSettings) -> Result<ActionsModel>;
        /// A multi-action problem solved (landschaft's closure engine, then HiGHS): x the units, the actions, the
        /// pairs' benefits; start: units then actions (empty: none).
        fn actions_solve(
            input: &ActionsInput,
            settings: &ActionsSettings,
            gap: f64,
            time_limit: f64,
            start: &[f64],
        ) -> Result<SolveOut>;
        /// rcpp_stats_benefit: the most benefit (an empty solution) or a solution's (units, then actions).
        fn actions_benefit(input: &ActionsInput, solution: &[f64]) -> Result<ActionsBenefit>;
        /// (monitoring, every threat's actions' cost) of a solution.
        fn actions_costs(input: &ActionsInput, units: &[f64], actions: &[f64]) -> Result<Vec<f64>>;
        /// (units, every threat's actions' boundary) of a solution.
        fn actions_connectivity(input: &ActionsInput, units: &[f64], actions: &[f64]) -> Result<Vec<f64>>;
        /// Every (feature, unit) pair.
        fn actions_pairs(input: &ActionsInput) -> Result<ActionsPairs>;
        fn lsm_table(
            values: &[i32],
            na: &[u8],
            ny: i64,
            nx: i64,
            resx: f64,
            resy: f64,
            options: &LsmOptions,
            levels: &[i32],
            names: &[String],
            labels: bool,
        ) -> Result<LsmTable>;
        /// Landscape-level metrics in a moving window (wy x wx, odd sides, non-zero cells part of it): the maps
        /// of the metrics one after the other.
        fn lsm_window(
            values: &[i32],
            na: &[u8],
            ny: i64,
            nx: i64,
            resx: f64,
            resy: f64,
            window: &[u8],
            wy: i64,
            wx: i64,
            names: &[String],
            options: &LsmOptions,
        ) -> Result<Vec<f64>>;

        /// The cells (wy x wx) of polygon rings (sizes: points per ring, then their xs and ys) burnt as GDAL's
        /// rasterizer with ALL_TOUCHED=TRUE; gt: GDAL's geotransform of the grid.
        fn lsm_rasterize(sizes: &[i64], xs: &[f64], ys: &[f64], gt: &[f64], nx: i32, ny: i32) -> Result<Vec<u8>>;
        /// R's sin(x) and cos(x) (the x87 bits of R on x86-64 Windows): the sines, then the cosines.
        fn lsm_r_sincos(x: &[f64]) -> Vec<f64>;
        /// R's cor(x, method = "pearson" | "spearman" | "kendall") of the columns of the n x m column-major x: m x m.
        fn lsm_r_cor(x: &[f64], n: i64, m: i64, method: &str) -> Result<Vec<f64>>;

        fn allow_matrix(x: &[f64], hist: &[i64], classes: &[f64], cd: &[f64], rules: &[i64]) -> Result<Vec<f64>>;
        fn auto_convert(x: &[f64], prob: &[f64], classes: &[f64], mask: &[i64]) -> Result<Vec<f64>>;
        fn neighbourhood_allow(
            landscape: &[f64],
            ny: i64,
            nx: i64,
            classes: &[f64],
            rules: &[NbRule],
        ) -> Result<Vec<f64>>;

        fn clues(
            tprob: &[f64],
            landuse: &[f64],
            demand: &[f64],
            codes: &[f64],
            jitter: f64,
            scale: f64,
            max_iter: f64,
            max_diff: f64,
            mean_diff: f64,
            rng: Pin<&mut RRng>,
        ) -> Result<CluesOut>;
        fn ordered(
            tprob: &[f64],
            x: &[f64],
            demand: &[f64],
            classes: &[f64],
            order: &[f64],
            stochastic: bool,
            rng: Pin<&mut RRng>,
        ) -> Result<OrderedOut>;

        fn approx_extrap(x: &[f64], y: &[f64], xout: &[f64]) -> Result<Vec<f64>>;
        fn round_sum(x: &[f64], nrow: i64, ncol: i64, ncell: i64) -> Result<Vec<f64>>;
        fn markov_from_counts(counts: &[f64], k: i32, kept: &mut Vec<i32>) -> Result<Vec<f64>>;
        fn markov_root(p: &[f64], k: i32, n: i32) -> Result<Vec<f64>>;
        fn markov_project(areas: &[f64], p: &[f64], k: i32, steps: i32) -> Result<Vec<f64>>;

        fn woe_buckets(
            var: &[f64],
            categorical: bool,
            classes: &[f64],
            start: f64,
            inc: f64,
            n_buckets: i64,
        ) -> Result<Vec<i32>>;
        fn woe_counts(
            t0: &[f64],
            t1: &[f64],
            buckets: &[i32],
            n_buckets: i64,
            from: f64,
            final_classes: &[f64],
        ) -> Result<Vec<i64>>;
        fn woe_probability(
            landscape: &[f64],
            from: f64,
            buckets: &[i32],
            weights: &[f64],
            offsets: &[i64],
        ) -> Result<Vec<f64>>;
        fn woe_weights(nb: f64, b: f64, n: f64, a: f64, fix: bool) -> WoeWeights;
        fn woe_generalise(
            possible: &[i64],
            executed: &[i64],
            min_delta: f64,
            max_delta: f64,
            tolerance: f64,
            fix: bool,
        ) -> Result<Vec<i32>>;
        fn woe_normalise(sums: &mut [f64], k: i32, n: i64) -> Result<()>;

        fn glm_logit(x: &[f64], y: &[f64], n: i32, p: i32, maxit: i32, epsilon: f64) -> Result<GlmFit>;
        fn glm_predict(x: &[f64], n: i64, p: i32, fit: &GlmFit) -> Result<Vec<f64>>;

        fn allocate_patches(
            ny: i64,
            nx: i64,
            landscape: &[i32],
            specs: &[PatchSpec],
            prune_factor: f64,
            win_rows: i32,
            win_cols: i32,
            seed: u64,
        ) -> Result<PatchOut>;
        fn cars(
            ny: i64,
            nx: i64,
            k: i32,
            landscape: &[i32],
            fixed: &[u8],
            region: &[i32],
            nreg: i32,
            probability: &[f64],
            weights: &[f64],
            seeds: &[f64],
            allowed: &[u8],
            demand: &[i64],
            window: i32,
            decay: f64,
            step: f64,
            max_iter: i64,
            seed: u64,
        ) -> Result<CarsOut>;
        fn similarity(a: &[i32], b: &[i32], ny: i64, nx: i64, window: i32, exponential: bool) -> Result<SimilarityOut>;
        fn observed_patches(a: &[f64], b: &[f64], ny: i64, nx: i64, from: f64, to: f64) -> Result<ObservedPatches>;

        // simulate bridge (landschaft-bb)
        #[allow(clippy::too_many_arguments)]
        fn cost_accumulated(
            values: &[f64],
            ny: i64,
            nx: i64,
            xmin: f64,
            xmax: f64,
            ymin: f64,
            ymax: f64,
            wrap: bool,
            directions: i32,
            symmetric: bool,
            sources: &[i64],
        ) -> Result<Vec<f64>>;

        /// numpy's legacy RandomState seeded with an integer (MOLUSCE's numpy.random.seed).
        #[namespace = "sim"]
        type NumpyRandomState;
        fn numpy_rng_new(seed: u32) -> UniquePtr<NumpyRandomState>;
        fn numpy_rng_random_sample(rng: Pin<&mut NumpyRandomState>, n: usize) -> Vec<f64>;
        fn molusce_categories(values: &[f64], bounds: &[f64]) -> Result<Vec<f64>>;
        fn molusce_total(factors: &[f64], nf: i32, n: i64, weights: &[MolusceFactor]) -> Result<Vec<f64>>;
        fn molusce_prediction_woe(
            state: &[f64],
            classes: &[f64],
            codes: &[i64],
            totals: &[f64],
        ) -> Result<MolusceForecastOut>;
        fn molusce_prediction_classes(state: &[f64], layers: &[f64], classes: &[f64]) -> Result<MolusceForecastOut>;
        fn molusce_ca_step(
            state: &[f64],
            prediction: &[f64],
            confidence: &[f64],
            counts: &[MolusceChange],
            rng: Pin<&mut NumpyRandomState>,
        ) -> Result<MolusceStepOut>;
        #[allow(clippy::too_many_arguments)]
        fn clumondo_year(
            s: &ClumondoSettingsIn,
            oldco: &[i32],
            region: &[i32],
            age: &[i32],
            suit: &[f32],
            allow_maps: &[f32],
            demand: &[f64],
            dem_elas: &[f32],
        ) -> Result<ClumondoYearOut>;
        fn clumondo_logit(region: &[i32], x: &[f32], constant: f64, coef: &[f32]) -> Result<Vec<f32>>;
        fn clumondo_add_neighbourhood(
            suit: &[f32],
            ny: i64,
            nx: i64,
            oldco: &[i32],
            region: &[i32],
            parts: &[ClumondoNbIn],
        ) -> Result<Vec<f32>>;
        fn clumondo_add_preference(row: &[f32], region: &[i32], values: &[f32], weight: f32) -> Result<Vec<f32>>;
        fn clumondo_random_age(ncell: i64, max_age: i32, seed: u32) -> Result<Vec<i32>>;
        #[allow(clippy::too_many_arguments)]
        fn futures_run(
            s: &FuturesIn,
            climate: &FuturesClimateIn,
            developed: &[i32],
            region: &[i32],
            pot_region: &[i32],
            devpressure: &[f32],
            aggregated: &[f32],
            weight: &[f32],
            zone: &[i32],
        ) -> Result<FuturesOut>;

        // out-of-core landscapes (tiles)

        /// landschaft's out-of-core landscape: a raster labelled block by block of rows (a multiple of 64), the
        /// pieces of the patches joined across the seams; the same metrics as the map in memory.
        #[namespace = "lsm"]
        #[cxx_name = "TiledLandscape"]
        type LsmTiled;

        /// An out-of-core landscape of nx x ny cells and the map's classes (those of every block).
        fn lsm_tiled_new(
            nx: i64,
            ny: i64,
            resx: f64,
            resy: f64,
            options: &LsmOptions,
            class_values: &[i32],
        ) -> Result<UniquePtr<LsmTiled>>;
        /// The classes of a block (landscapemetrics' get_unique_values), increasing.
        fn lsm_unique_values(values: &[i32], na: &[u8]) -> Vec<i32>;
        /// The first pass: a block of `rows` rows from row `y0`, with `above` and `below` rows of context.
        fn lsm_tiled_add(
            t: Pin<&mut LsmTiled>,
            values: &[i32],
            na: &[u8],
            y0: i64,
            rows: i64,
            above: i64,
            below: i64,
        ) -> Result<()>;
        fn lsm_tiled_finish(t: Pin<&mut LsmTiled>) -> Result<()>;
        /// The rows of context the detail pass's blocks need (core areas, enn).
        fn lsm_tiled_detail_halo(t: &LsmTiled, core: bool, enn: bool) -> i64;
        fn lsm_tiled_begin_detail(
            t: Pin<&mut LsmTiled>,
            core: bool,
            gyrate: bool,
            circle: bool,
            enn: bool,
        ) -> Result<()>;
        /// The detail pass (core areas, gyrate, circle, enn): the blocks again, with their context.
        fn lsm_tiled_detail(
            t: Pin<&mut LsmTiled>,
            values: &[i32],
            na: &[u8],
            y0: i64,
            rows: i64,
            above: i64,
            below: i64,
        ) -> Result<()>;
        /// The end of the detail pass: whether gyrate with cell_center needs the pass of ties.
        fn lsm_tiled_end_detail(t: Pin<&mut LsmTiled>) -> Result<bool>;
        fn lsm_tiled_tie(
            t: Pin<&mut LsmTiled>,
            values: &[i32],
            na: &[u8],
            y0: i64,
            rows: i64,
            above: i64,
            below: i64,
        ) -> Result<()>;
        fn lsm_tiled_end_ties(t: Pin<&mut LsmTiled>) -> Result<()>;
        /// The metrics, as lsm_table (the patches' classes too).
        fn lsm_tiled_table(t: Pin<&mut LsmTiled>, levels: &[i32], names: &[String]) -> Result<LsmTable>;
        /// The patch of every cell of a block of the first pass (1.., 0 NA), read again without context.
        fn lsm_tiled_labels(t: &LsmTiled, values: &[i32], na: &[u8], y0: i64, rows: i64) -> Result<Vec<i32>>;
        /// The values of every band (bands x ny x nx, NaN no data) at points: (points x bands); NaN outside the
        /// grid or on no data; bilinear from the four cell centres around, the weights over those with data
        /// (landschaft's extract). The values are the rows [row0, row0 + ny) of a grid of ny_total rows (gt the
        /// grid's): the points in those rows (with one row around for bilinear) get the whole grid's values.
        #[allow(clippy::too_many_arguments)]
        fn io_extract(
            values: &[f64],
            bands: i32,
            ny: i32,
            nx: i32,
            gt: &[f64],
            x: &[f64],
            y: &[f64],
            bilinear: bool,
            row0: i32,
            ny_total: i32,
        ) -> Result<Vec<f64>>;
    }
}

pub use ffi::{
    CarsOut, CluesOut, ClumondoNbIn, ClumondoSettingsIn, ClumondoYearOut, FuturesClimateIn, FuturesIn, FuturesOut,
    GlmFit, MolusceChange, MolusceFactor, MolusceForecastOut, MolusceStepOut, NbRule, ObservedPatches, OrderedOut,
    PatchOut, PatchSpec, SimilarityOut, WoeWeights,
};

/// The threads Terraflux uses: never every core of the machine (all but one by default).
pub mod threads {
    use std::sync::Once;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static INIT: Once = Once::new();
    static THREADS: AtomicUsize = AtomicUsize::new(0);

    /// The cores of the machine.
    pub fn cores() -> usize {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
    }

    /// The most threads Terraflux may use: all the cores but one (one on a machine of one).
    pub fn most() -> usize {
        cores().saturating_sub(1).max(1)
    }

    /// The threads by default: `TERRAFLUX_THREADS`, else all the cores but one; never more than [`most`].
    pub fn default_threads() -> usize {
        std::env::var("TERRAFLUX_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or_else(most).clamp(1, most())
    }

    /// Set the threads once for the process (later calls do nothing): the kernels' OpenMP threads and the
    /// pool of parallel tasks, whose threads run the kernels with one thread each (no more threads than
    /// asked in all). Returns the threads in use.
    pub fn init(n: Option<usize>) -> usize {
        INIT.call_once(|| {
            let n = n.unwrap_or_else(default_threads).clamp(1, most());
            // SAFETY: before any other thread of ours runs
            unsafe { std::env::set_var("OMP_NUM_THREADS", n.to_string()) };
            crate::ffi::set_threads(n as i32);
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .start_handler(|_| crate::ffi::set_threads(1))
                .build_global();
            THREADS.store(n, Ordering::Relaxed);
        });
        THREADS.load(Ordering::Relaxed)
    }

    /// The threads in use (setting them by default first).
    pub fn ensure() -> usize {
        init(None)
    }

    /// The kernels' threads for the calling thread (a worker sharing the machine with others).
    pub fn set_kernel_threads(n: usize) {
        crate::ffi::set_threads(n.clamp(1, most()) as i32);
    }
}

/// numpy's legacy generator after `numpy.random.seed(seed)` (MOLUSCE's).
pub struct NumpyRng(cxx::UniquePtr<ffi::NumpyRandomState>);

impl NumpyRng {
    pub fn new(seed: u32) -> Self {
        NumpyRng(ffi::numpy_rng_new(seed))
    }

    /// `random_sample(n)`: numbers in [0, 1).
    pub fn random_sample(&mut self, n: usize) -> Vec<f64> {
        ffi::numpy_rng_random_sample(self.0.pin_mut(), n)
    }

    pub fn pin_mut(&mut self) -> std::pin::Pin<&mut ffi::NumpyRandomState> {
        self.0.pin_mut()
    }
}

// SAFETY: the generator is plain state, used by one thread at a time (behind a lock)
unsafe impl Send for NumpyRng {}

/// R's generator after `set.seed(seed)`.
pub struct RRng(cxx::UniquePtr<ffi::RRng>);

impl RRng {
    pub fn new(seed: i32) -> Self {
        RRng(ffi::rrng_new(seed))
    }

    /// `unif_rand()`: a number in (0, 1).
    pub fn unif_rand(&mut self) -> f64 {
        ffi::rrng_unif_rand(self.0.pin_mut())
    }

    /// `runif(n)`.
    pub fn runif(&mut self, n: usize) -> Vec<f64> {
        ffi::rrng_runif(self.0.pin_mut(), n)
    }

    pub fn pin_mut(&mut self) -> std::pin::Pin<&mut ffi::RRng> {
        self.0.pin_mut()
    }
}

// the generator is used by one thread at a time
unsafe impl Send for RRng {}

/// An error raised by a kernel (its message).
pub type Error = cxx::Exception;
