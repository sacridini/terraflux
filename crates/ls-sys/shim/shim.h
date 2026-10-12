// SPDX-License-Identifier: GPL-3.0-or-later
//
// The shim between Terraflux (Rust, cxx) and landschaft's C++ kernels.
#pragma once

#include <cstdint>
#include <memory>

#include "nlm/rng.hpp"
#include "prio/prio.hpp"
#include "rust/cxx.h"

namespace tfx {

struct CluesOut;
struct OrderedOut;
struct NbRule;
struct GlmFit;
struct WoeWeights;
struct PatchSpec;
struct PatchOut;
struct ObservedPatches;
struct CarsOut;
struct SimilarityOut;
struct FadOut;
struct FadScalesOut;
struct SpaOut;
struct AccountingOut;
struct EntropyOut;
struct MosaicStatsOut;
struct ParcellationOut;
struct ChangeOut;
struct DistanceOut;
struct InfluenceOut;
struct LcpOut;
struct SecantOut;
struct LsmOptions;
struct LsmTable;
struct VegetationOut;
struct HabitatOut;
struct GainOut;
struct PrioStatsOut;
struct PrioSettings;
struct PrioRankOut;
struct PrioDataInfo;
struct PrioFlexOut;
struct RunsOut;
struct MarxanData;
struct MarxanOptions;
struct MarxanOut;
struct ProxirrOut;
struct FeaturesOut;
struct MilpModel;
struct SolveSettings;
struct SolveOut;
struct CscOut;

// the threads of the kernels (OpenMP) of the calling thread, and their most
void set_threads(std::int32_t n);
std::int32_t max_threads();

std::unique_ptr<nlm::RRng> rrng_new(std::int32_t seed);
double rrng_unif_rand(nlm::RRng& rng);
rust::Vec<double> rrng_runif(nlm::RRng& rng, std::size_t n);

rust::Vec<double> map_classes(rust::Slice<const double> values);
rust::Vec<std::int64_t> crosstab(rust::Slice<const double> a, rust::Slice<const double> b,
                                 rust::Slice<const double> classes_a, rust::Slice<const double> classes_b);
double r_sum(rust::Slice<const double> x);
rust::Vec<double> focal_mean(rust::Slice<const double> x, std::int64_t ny, std::int64_t nx,
                             rust::Slice<const double> w, std::int32_t wy, std::int32_t wx);
rust::Vec<double> distance_exact(rust::Slice<const std::uint8_t> origin, std::int64_t ny, std::int64_t nx, double res);

// GuidosToolbox's MSPA (gtb::mspa): cells coded 0 missing, 1 background, 2 foreground
// NLMR's neutral landscape models (nlm::), drawn with R's generator from `seed` (row-major ny x nx)
rust::Vec<double> nlm_random_cluster(std::int32_t seed, std::int64_t ny, std::int64_t nx, double p,
                                     rust::Slice<const double> ai, std::int32_t neighbourhood);
rust::Vec<double> nlm_gaussian_field(std::int32_t seed, std::int64_t ny, std::int64_t nx, double autocorr_range,
                                     double mag_var, double nug, double mean);
rust::Vec<double> nlm_fbm(std::int32_t seed, std::int64_t ny, std::int64_t nx, double fract_dim);

rust::Vec<std::uint8_t> mspa(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                             std::int32_t connectivity, std::int32_t edge_width, bool internal);
// the rest of GuidosToolbox (gtb::): maps coded as for MSPA, with 3 and 4 special backgrounds where GTB takes them
FadOut fad(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx, std::int32_t window);
FadScalesOut fad_multiscale(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                            rust::Slice<const std::int32_t> windows);
rust::Vec<std::uint8_t> fad_density(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                    std::int32_t window, bool gtb_codes);
rust::Vec<std::uint8_t> clustering_fac(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                       std::int32_t window, bool gtb_codes);
rust::Vec<std::uint8_t> grayspatcon_fos(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                        std::int32_t metric, std::int32_t window);
SpaOut spa(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx, std::int32_t variant);
AccountingOut accounting(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                         std::int32_t connectivity, rust::Slice<const std::int64_t> thresholds, bool highlight_largest);
rust::Vec<std::uint8_t> contagion_pff(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                      std::int32_t window);
EntropyOut spatial_entropy(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx);
rust::Vec<float> spatial_entropy_map(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                     std::int32_t window);
rust::Vec<std::uint8_t> landscape_mosaic(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                         std::int32_t window, std::int32_t classes);
MosaicStatsOut landscape_mosaic_stats(rust::Slice<const std::uint8_t> lm103, std::int64_t ny, std::int64_t nx);
rust::Vec<float> gray_convolution(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                  rust::Str metric, std::int32_t window, bool mask_missing);
ParcellationOut parcellation(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                             std::int32_t connectivity);
rust::Vec<std::uint32_t> contortion(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                    std::int64_t min_area);
ChangeOut simple_change(rust::Slice<const std::uint8_t> a, rust::Slice<const std::uint8_t> b, std::int64_t ny,
                        std::int64_t nx);
ChangeOut morphological_change(rust::Slice<const std::uint8_t> a, rust::Slice<const std::uint8_t> b, std::int64_t ny,
                               std::int64_t nx);
ChangeOut fos_change(rust::Slice<const std::uint8_t> a, rust::Slice<const std::uint8_t> b, std::int64_t ny,
                     std::int64_t nx);
ChangeOut fad_change_counts(rust::Slice<const std::uint8_t> a, rust::Slice<const std::uint8_t> b, std::int64_t ny,
                            std::int64_t nx);
DistanceOut euclidean_distance(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx);
rust::Vec<double> restoration_status(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx);
rust::Vec<float> cost_distance(rust::Slice<const std::uint8_t> resistance, rust::Slice<const std::uint8_t> sources,
                               std::int64_t ny, std::int64_t nx);
LcpOut least_cost_path(rust::Slice<const std::uint8_t> resistance, rust::Slice<const std::uint8_t> a,
                       rust::Slice<const std::uint8_t> b, rust::Slice<const float> distance, std::int64_t ny,
                       std::int64_t nx);
rust::Vec<std::uint32_t> geodesic_time(rust::Slice<const std::uint8_t> mask, rust::Slice<const std::uint8_t> reference,
                                       std::int64_t ny, std::int64_t nx);
rust::Vec<std::uint8_t> thin_anchored(rust::Slice<const std::uint8_t> input, std::int32_t stype,
                                      rust::Slice<const std::uint8_t> anchor, std::int64_t ny, std::int64_t nx);
rust::Vec<float> morph_distance(rust::Slice<const std::uint8_t> mask, std::int64_t ny, std::int64_t nx, bool background);
InfluenceOut influence(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx, std::int64_t min_area,
                       std::int32_t core_buffer, std::int32_t buffer, bool proximity);

// restoration planning's inputs after plangea (rest::); `past`, `ecoregion`, `domain` empty for none
rust::Vec<double> rest_footprint(rust::Slice<const double> hfi, rust::Slice<const double> reduction,
                                 rust::Slice<const double> restored, std::int64_t ny, std::int64_t nx);
rust::Vec<double> rest_intactness(rust::Slice<const double> hfi, std::int64_t ny, std::int64_t nx, double gamma,
                                  double beta, double z, std::int32_t radius, double dx, double dy);
rust::Vec<double> rest_intactness_gradient(rust::Slice<const double> hfi, rust::Slice<const double> reduction,
                                           std::int64_t ny, std::int64_t nx, double gamma, double beta, double z,
                                           std::int32_t radius, double dx, double dy);
SecantOut rest_intactness_secant(rust::Slice<const double> h0, rust::Slice<const double> tlo,
                                 rust::Slice<const double> thi, std::int64_t ny, std::int64_t nx, double gamma,
                                 double beta, double z, std::int32_t radius, double dx, double dy);
VegetationOut rest_original_vegetation(rust::Slice<const double> natural, std::int32_t k,
                                       rust::Slice<const double> anthropic, std::int32_t m, std::int64_t n,
                                       rust::Slice<const double> past, rust::Slice<const double> ecoregion,
                                       rust::Slice<const std::uint8_t> domain, rust::Slice<const std::uint8_t> fill,
                                       double threshold, bool restore_current);
HabitatOut rest_habitat(rust::Slice<const double> current, rust::Slice<const double> original,
                        rust::Slice<const double> area, std::int64_t n, std::int32_t k,
                        rust::Slice<const std::int64_t> range_ptr, rust::Slice<const std::int64_t> range_idx,
                        rust::Slice<const std::uint8_t> suitable, std::int64_t ns);
GainOut rest_habitat_gain(rust::Slice<const double> restore, rust::Slice<const double> area, std::int64_t n,
                          std::int32_t k, rust::Slice<const std::int64_t> range_ptr,
                          rust::Slice<const std::int64_t> range_idx, rust::Slice<const std::uint8_t> suitable,
                          std::int64_t ns, rust::Slice<const std::uint8_t> domain);

// Zonation 5's ranking (prio::): features (layers, ny, nx) float32, NaN or `nodata` without data; `area` empty
// or 1 inside the analysis area
PrioStatsOut prio_stats(rust::Slice<const float> values, std::int32_t layers, std::int64_t ny, std::int64_t nx,
                        rust::Slice<const float> nodata, rust::Slice<const float> extract,
                        rust::Slice<const float> thresholds, rust::Slice<const std::uint8_t> area);
// Zonation's quantized data (prio::Data) of the layers given (those with data): made, conditioned, filled (and
// finished) by the layers (layers, ny, nx) with their conditions (conditions, ny, nx; empty for none), then
// transformed (a layer replaced, retention, levels) before the ranking
std::unique_ptr<prio::Data> prio_data_new(rust::Slice<const float> nodata, rust::Slice<const float> extract,
                                          rust::Slice<const float> thresholds, rust::Slice<const float> max_values,
                                          rust::Slice<const std::uint8_t> keep_zeros, std::int32_t bits);
void prio_data_set_conditions(prio::Data& data, rust::Slice<const std::int32_t> condition,
                              rust::Slice<const std::uint8_t> renormalize);
void prio_data_add(prio::Data& data, rust::Slice<const float> values, std::int64_t ny, std::int64_t nx,
                   rust::Slice<const std::uint8_t> area, rust::Slice<const float> conditions, std::int64_t block_rows);
rust::Vec<std::int64_t> prio_data_cells(const prio::Data& data);
PrioDataInfo prio_data_info(const prio::Data& data);
rust::Vec<double> prio_data_layer(const prio::Data& data, std::int32_t layer, bool scaled, float missing);
double prio_data_replace_layer(prio::Data& data, std::int32_t layer, rust::Slice<const float> values, double level);
std::int64_t prio_data_retain(prio::Data& data, std::int32_t layer, std::int32_t kind, rust::Slice<const float> values,
                              float fraction);
rust::Vec<float> prio_data_preprocessing(const prio::Data& data, rust::Slice<const float> mult,
                                         rust::Slice<const float> weights, bool unscaled);
void prio_data_scale_level(prio::Data& data, std::int32_t layer, double factor);
void prio_data_set_layer_values(prio::Data& data, std::int32_t layer, rust::Slice<const float> values);
// the ranking, with Zonation's settings; cost, hmask and solution are maps (ny x nx, empty for none) gathered at
// the cells with data as Zonation does
PrioRankOut prio_rank(const prio::Data& data, const PrioSettings& settings, std::int64_t ny, std::int64_t nx,
                      rust::Slice<const double> cost, rust::Slice<const double> hmask,
                      rust::Slice<const double> solution);
// Zonation's flexibility map of a ranking (its order and curves)
PrioFlexOut prio_flexibility(const prio::Data& data, const PrioSettings& settings, std::int64_t ny, std::int64_t nx,
                             rust::Slice<const double> cost, rust::Slice<const double> hmask,
                             rust::Slice<const std::uint32_t> order, rust::Slice<const float> curve_rank,
                             rust::Slice<const float> curve_levels, std::int64_t curve_interval, double max_error,
                             bool up, bool down);
// landschaft's zones of polygons (lsio::): their cells as runs, burnt, their statistics; exactextract's sums
RunsOut io_polygon_runs(rust::Slice<const double> xy, rust::Slice<const std::int64_t> ring_offsets,
                        rust::Slice<const std::int64_t> geom_rings, rust::Slice<const double> gt, std::int32_t nx,
                        std::int32_t ny, bool all_touched);
void io_burn_runs(rust::Slice<double> out, std::int32_t ny, std::int32_t nx, const RunsOut& runs,
                  rust::Slice<const double> value, bool add);
rust::Vec<double> io_zonal_stats(rust::Slice<const double> values, std::int32_t bands, std::int32_t ny,
                                 std::int32_t nx, const RunsOut& runs, rust::Slice<const std::int32_t> stats,
                                 rust::Slice<const double> percentiles);
rust::Vec<double> io_exact_sums(rust::Slice<const double> values, std::int64_t layers, std::int64_t ny,
                                std::int64_t nx, rust::Slice<const double> grid, rust::Slice<const double> xy,
                                rust::Slice<const std::int64_t> ring_offsets,
                                rust::Slice<const std::int64_t> polygon_offsets,
                                rust::Slice<const std::int64_t> feature_offsets);
// Zonation's (Eigen AVX) sums
double prio_eigen_sum(rust::Slice<const double> values);
float prio_eigen_sum_f32(rust::Slice<const float> values, std::int64_t offset);

// Marxan 4 (opt::marxan): simulated annealing and iterative improvement of a reserve
MarxanOut opt_marxan(const MarxanData& data, const MarxanOptions& options);

// ProxIRR (opt::proxirr): alpha of every entry, then beta and gamma of every unit from its alphas
// (start[u] .. start[u + 1])
rust::Vec<double> proxirr_alpha(rust::Slice<const double> local, rust::Slice<const double> global,
                                rust::Slice<const double> target, bool triage);
ProxirrOut proxirr_beta_gamma(rust::Slice<const std::int64_t> start, rust::Slice<const double> alphas);

// the MILP prioritization (opt::): the features' amounts in the planning units (values: layers x n, unit: the
// unit of each cell, -1 none), and a model solved by HiGHS (method: highs, covering or wide)
FeaturesOut opt_features(rust::Slice<const double> values, std::int64_t layers, rust::Slice<const std::int64_t> unit);
SolveOut opt_solve(const MilpModel& model, const SolveSettings& settings, rust::Str method, std::int64_t n_units);

// prioritizr's evaluation of solutions (opt::eval): the boundary of a solution (lower: the strictly lower
// triangle of the boundary matrix in compressed columns), Ferrier's scores (rij: features x units in compressed
// columns), Armadillo's accumulate
double eval_boundary(double edge_factor, rust::Slice<const std::int64_t> start, rust::Slice<const std::int64_t> index,
                     rust::Slice<const double> value, rust::Slice<const double> exposed,
                     rust::Slice<const double> total, rust::Slice<const double> solution);
CscOut eval_ferrier(rust::Slice<const std::int64_t> start, rust::Slice<const std::int64_t> index,
                    rust::Slice<const double> value, std::int64_t nrow, rust::Slice<const double> targets,
                    rust::Slice<const std::uint8_t> selected);
double eval_accumulate(rust::Slice<const double> x);
// the connectivity of a solution (rcpp_connectivity, rcpp_asym_connectivity): the units' matrix in compressed columns
double eval_connectivity(rust::Slice<const std::int64_t> start, rust::Slice<const std::int64_t> index,
                         rust::Slice<const double> value, rust::Slice<const double> solution, bool asym);
// with management zones: zones nz x nz (row-major), the solution zone after zone (unit i in zone z at z * n + i)
double eval_boundary_zones(rust::Slice<const double> edge_factor, rust::Slice<const double> zones,
                           rust::Slice<const std::int64_t> start, rust::Slice<const std::int64_t> index,
                           rust::Slice<const double> value, rust::Slice<const double> exposed,
                           rust::Slice<const double> total, rust::Slice<const double> solution);
double eval_connectivity_zones(rust::Slice<const std::int64_t> start, rust::Slice<const std::int64_t> index,
                               rust::Slice<const double> value, rust::Slice<const double> zones,
                               rust::Slice<const double> solution, bool asym);

// prioriactions (opt::actions)
struct ActionsInput;
struct ActionsSettings;
struct ActionsModel;
struct ActionsBenefit;
struct ActionsPairs;
ActionsModel actions_compile(const ActionsInput& input, const ActionsSettings& settings);
SolveOut actions_solve(const ActionsInput& input, const ActionsSettings& settings, double gap, double time_limit,
                       rust::Slice<const double> start);
ActionsBenefit actions_benefit(const ActionsInput& input, rust::Slice<const double> solution);
rust::Vec<double> actions_costs(const ActionsInput& input, rust::Slice<const double> units,
                                rust::Slice<const double> actions);
rust::Vec<double> actions_connectivity(const ActionsInput& input, rust::Slice<const double> units,
                                       rust::Slice<const double> actions);
ActionsPairs actions_pairs(const ActionsInput& input);

// landscape metrics (lsm::), as landscapemetrics: classes as 32-bit integers, `na` empty or 1 where a cell is NA
LsmTable lsm_table(rust::Slice<const std::int32_t> values, rust::Slice<const std::uint8_t> na, std::int64_t ny,
                   std::int64_t nx, double resx, double resy, const LsmOptions& options,
                   rust::Slice<const std::int32_t> levels, rust::Slice<const rust::String> names, bool labels);
rust::Vec<double> lsm_window(rust::Slice<const std::int32_t> values, rust::Slice<const std::uint8_t> na, std::int64_t ny,
                             std::int64_t nx, double resx, double resy, rust::Slice<const std::uint8_t> window,
                             std::int64_t wy, std::int64_t wx, rust::Slice<const rust::String> names,
                             const LsmOptions& options);

rust::Vec<double> allow_matrix(rust::Slice<const double> x, rust::Slice<const std::int64_t> hist,
                               rust::Slice<const double> classes, rust::Slice<const double> cd,
                               rust::Slice<const std::int64_t> rules);
rust::Vec<double> auto_convert(rust::Slice<const double> x, rust::Slice<const double> prob,
                               rust::Slice<const double> classes, rust::Slice<const std::int64_t> mask);
rust::Vec<double> neighbourhood_allow(rust::Slice<const double> landscape, std::int64_t ny, std::int64_t nx,
                                      rust::Slice<const double> classes, rust::Slice<const NbRule> rules);

CluesOut clues(rust::Slice<const double> tprob, rust::Slice<const double> landuse, rust::Slice<const double> demand,
               rust::Slice<const double> codes, double jitter, double scale, double max_iter, double max_diff,
               double mean_diff, nlm::RRng& rng);
OrderedOut ordered(rust::Slice<const double> tprob, rust::Slice<const double> x, rust::Slice<const double> demand,
                   rust::Slice<const double> classes, rust::Slice<const double> order, bool stochastic,
                   nlm::RRng& rng);

rust::Vec<double> approx_extrap(rust::Slice<const double> x, rust::Slice<const double> y,
                                rust::Slice<const double> xout);
rust::Vec<double> round_sum(rust::Slice<const double> x, std::int64_t nrow, std::int64_t ncol, std::int64_t ncell);
rust::Vec<double> markov_from_counts(rust::Slice<const double> counts, std::int32_t k, rust::Vec<std::int32_t>& kept);
rust::Vec<double> markov_root(rust::Slice<const double> p, std::int32_t k, std::int32_t n);
rust::Vec<double> markov_project(rust::Slice<const double> areas, rust::Slice<const double> p, std::int32_t k,
                                 std::int32_t steps);

rust::Vec<std::int32_t> woe_buckets(rust::Slice<const double> var, bool categorical, rust::Slice<const double> classes,
                                    double start, double inc, std::int64_t n_buckets);
rust::Vec<std::int64_t> woe_counts(rust::Slice<const double> t0, rust::Slice<const double> t1,
                                   rust::Slice<const std::int32_t> buckets, std::int64_t n_buckets, double from,
                                   rust::Slice<const double> final_classes);
rust::Vec<double> woe_probability(rust::Slice<const double> landscape, double from,
                                  rust::Slice<const std::int32_t> buckets, rust::Slice<const double> weights,
                                  rust::Slice<const std::int64_t> offsets);
WoeWeights woe_weights(double nb, double b, double n, double a, bool fix);
rust::Vec<std::int32_t> woe_generalise(rust::Slice<const std::int64_t> possible,
                                       rust::Slice<const std::int64_t> executed, double min_delta, double max_delta,
                                       double tolerance, bool fix);
void woe_normalise(rust::Slice<double> sums, std::int32_t k, std::int64_t n);

GlmFit glm_logit(rust::Slice<const double> x, rust::Slice<const double> y, std::int32_t n, std::int32_t p,
                 std::int32_t maxit, double epsilon);
rust::Vec<double> glm_predict(rust::Slice<const double> x, std::int64_t n, std::int32_t p, const GlmFit& fit);

PatchOut allocate_patches(std::int64_t ny, std::int64_t nx, rust::Slice<const std::int32_t> landscape,
                          rust::Slice<const PatchSpec> specs, double prune_factor, std::int32_t win_rows,
                          std::int32_t win_cols, std::uint64_t seed);
CarsOut cars(std::int64_t ny, std::int64_t nx, std::int32_t k, rust::Slice<const std::int32_t> landscape,
             rust::Slice<const std::uint8_t> fixed, rust::Slice<const std::int32_t> region, std::int32_t nreg,
             rust::Slice<const double> probability, rust::Slice<const double> weights,
             rust::Slice<const double> seeds, rust::Slice<const std::uint8_t> allowed,
             rust::Slice<const std::int64_t> demand, std::int32_t window, double decay, double step,
             std::int64_t max_iter, std::uint64_t seed);
SimilarityOut similarity(rust::Slice<const std::int32_t> a, rust::Slice<const std::int32_t> b, std::int64_t ny,
                         std::int64_t nx, std::int32_t window, bool exponential);
ObservedPatches observed_patches(rust::Slice<const double> a, rust::Slice<const double> b, std::int64_t ny,
                                 std::int64_t nx, double from, double to);

}  // namespace tfx
