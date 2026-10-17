// SPDX-License-Identifier: GPL-3.0-or-later
//
// The shim between Terraflux (Rust, cxx) and landschaft's C++ kernels.
#pragma once

#include <cstdint>
#include <memory>

#include "nlm/rng.hpp"
#include "prio/prio.hpp"
#include "sim/molusce.hpp"
#include "rust/cxx.h"

#include "bio/phylocom.hpp"
#include "bio/randomise.hpp"

namespace tfx {

struct CluesOut;
struct NlmMosaicOut;
struct MaxnetFit;
struct ThinOut;
struct CirclesOut;
struct ExactPiece;
struct ConnAnalysis;
struct ConnGains;
struct ConnRasterNodes;
struct ConnPairs;
struct ConnSamc;
struct ConnConductance;
struct ConnCentrality;
struct NlmGermsOut;
struct OrderedOut;
struct NbRule;
struct GlmFit;
struct WoeWeights;
struct PatchSpec;
struct PatchOut;
struct ObservedPatches;
struct CarsOut;
struct MolusceForecastOut;
struct MolusceChange;
struct MolusceStepOut;
struct MolusceFactor;
struct ClumondoSettingsIn;
struct ClumondoYearOut;
struct ClumondoNbIn;
struct FuturesIn;
struct FuturesClimateIn;
struct FuturesOut;
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
struct OmniscapeOut;
struct CircuitOut;
struct PathsOut;
struct BioGroups;
struct BioCall;
struct BioProgram;
struct BioCsr;
struct BioCalcInfo;
struct BioView;
struct BioIndexOut;
struct BioTree;
struct BioCounts;
struct BioJoin;
struct BioClusterOut;
struct ClusterRecalc;
struct SolveWatch;
struct NetworkOut;
struct CsAdvancedOut;
struct EdgesOut;
struct CommuteOut;
struct PlaneOut;
struct BearingOut;
struct AacfOut;
struct PairwiseOut;
struct MultiOut;
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
rust::Vec<std::int64_t> rrng_sample_int(nlm::RRng& rng, std::int64_t n, std::int64_t size, bool replace,
                                        rust::Slice<const double> prob);
rust::Vec<std::int32_t> sdm_kmeans(nlm::RRng& rng, rust::Slice<const double> x, std::int64_t m, std::int64_t p,
                                   std::int32_t k, std::int32_t iter_max, std::int32_t nstart, std::int32_t algorithm);
double sdm_r_sd(rust::Slice<const double> x);
rust::Vec<double> sdm_vif(rust::Slice<const double> table, std::int64_t n, std::int64_t m,
                          rust::Slice<const std::int64_t> which);
rust::Vec<double> sdm_sample_variogram(rust::Slice<const double> x, rust::Slice<const double> y,
                                       rust::Slice<const double> z, rust::Slice<const double> bounds, bool lonlat);
rust::Vec<double> sdm_fit_variogram(rust::Str model, double kappa, rust::Slice<const double> dist,
                                    rust::Slice<const double> gamma, rust::Slice<const double> np, double nugget,
                                    double psill, double range);
ThinOut sdm_thin_spthin(rust::Slice<const double> lon, rust::Slice<const double> lat, double max_km, std::int32_t reps,
                        std::int32_t seed);
std::int64_t io_exact_piece_count(rust::Slice<const double> grid, std::int64_t ny, std::int64_t nx,
                                  rust::Slice<const double> xy, rust::Slice<const std::int64_t> ring_offsets,
                                  rust::Slice<const std::int64_t> polygon_offsets, std::int64_t max_cells);
ExactPiece io_exact_piece(rust::Slice<const double> grid, std::int64_t ny, std::int64_t nx, rust::Slice<const double> xy,
                          rust::Slice<const std::int64_t> ring_offsets, rust::Slice<const std::int64_t> polygon_offsets,
                          std::int64_t max_cells, std::int64_t k);
rust::Vec<float> sim_futures_mfilter(rust::Slice<const double> input, std::int64_t rows, std::int64_t cols,
                                     rust::Slice<const double> matrix, std::int32_t size);
rust::Vec<double> sim_tmc_table(std::int32_t n, rust::Slice<const double> x, rust::Slice<const double> x1,
                                rust::Slice<const double> y1, rust::Slice<const double> w);
CirclesOut sdm_circles(rust::Slice<const double> lon, rust::Slice<const double> lat, double d, rust::Str kind);
rust::Vec<double> sdm_outlier_statistic(rust::Slice<const double> lon, rust::Slice<const double> lat, bool use_min);
rust::Vec<double> sdm_geod_direct(rust::Slice<const double> lon, rust::Slice<const double> lat,
                                  rust::Slice<const double> azi, rust::Slice<const double> s);
rust::Vec<double> sdm_outlier_statistic_weighted(rust::Slice<const double> lon, rust::Slice<const double> lat,
                                                 rust::Slice<const double> count, double within, bool use_min);
rust::Vec<double> cost_geodesic(rust::Slice<const double> lon1, rust::Slice<const double> lat1,
                                rust::Slice<const double> lon2, rust::Slice<const double> lat2);

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
ConnAnalysis conn_analyse(rust::Slice<const std::int32_t> pi, rust::Slice<const std::int32_t> pj,
                          rust::Slice<const double> value, rust::Slice<const double> area, std::int32_t kind,
                          double threshold, double probability, double pij_min, double expand_limit, bool conefor,
                          bool links, bool probabilities, bool importance, bool edges, bool betweenness);
rust::Vec<double> conn_all_pairs(rust::Slice<const std::int32_t> pi, rust::Slice<const std::int32_t> pj,
                                 rust::Slice<const double> value, std::int32_t n, std::int32_t kind, double threshold,
                                 double probability, double pij_min, double expand_limit, bool conefor, bool links);
rust::Vec<double> conn_pair_lengths(rust::Slice<const double> value, std::int32_t kind, double threshold,
                                    double probability, double pij_min, bool conefor, bool links, bool cut);
ConnGains conn_addition_gains(rust::Slice<const double> d, rust::Slice<const double> area,
                              rust::Slice<const std::int64_t> cand_start, rust::Slice<const std::int32_t> cand_node,
                              rust::Slice<const double> cand_len, rust::Slice<const double> cand_area, bool hops);
ConnGains conn_edge_gains(rust::Slice<const double> d, rust::Slice<const double> area,
                          rust::Slice<const std::int32_t> ea, rust::Slice<const std::int32_t> eb,
                          rust::Slice<const double> elen, bool hops);
ConnRasterNodes conn_raster_nodes(rust::Slice<const std::int32_t> values, rust::Slice<const std::uint8_t> na,
                                  std::int64_t ny, std::int64_t nx, std::array<double, 4> extent, bool centroids,
                                  bool keep_index);
ConnPairs conn_point_distances(rust::Slice<const double> x, rust::Slice<const double> y, double max_distance);
ConnPairs conn_raster_edge_distances(rust::Slice<const std::int32_t> node, std::int32_t n_nodes, std::int64_t ny,
                                     std::int64_t nx, double resx, double resy, double max_distance);
rust::Vec<double> conn_gdal_areas(rust::Slice<const double> x, rust::Slice<const double> y,
                                  rust::Slice<const std::int64_t> ring_start, rust::Slice<const std::int64_t> poly_start,
                                  rust::Slice<const std::int64_t> geom_start);
ConnPairs conn_least_cost(rust::Slice<const double> resistance, std::int64_t ny, std::int64_t nx,
                          std::array<double, 4> extent, rust::Slice<const std::int64_t> node_cell, std::int32_t directions,
                          std::int32_t function, double max_cost, std::int32_t max_threads);
ConnConductance conn_conductance(rust::Slice<const double> resistance, std::int64_t ny, std::int64_t nx,
                                 std::array<double, 4> extent, std::int32_t directions, std::int32_t function);
ConnSamc conn_samc_transitions(rust::Slice<const double> resistance, rust::Slice<const double> absorption,
                               std::int64_t ny, std::int64_t nx, std::int32_t directions, std::int32_t function);
rust::Vec<double> samc_inverse_block(std::int64_t m, rust::Slice<const std::int32_t> from,
                                     rust::Slice<const std::int32_t> to, rust::Slice<const double> q,
                                     rust::Slice<const std::int64_t> targets);
ConnCentrality conn_centrality(rust::Slice<const std::int32_t> pi, rust::Slice<const std::int32_t> pj,
                               rust::Slice<const double> length, std::int32_t n, bool unit);
rust::Vec<double> sdm_knn_mean_distance(rust::Slice<const double> m, std::int64_t nm, rust::Slice<const double> g,
                                        std::int64_t ng, std::int64_t p, std::int64_t k, bool squared);
MaxnetFit sdm_maxnet_fit(rust::Slice<const double> cont, rust::Slice<const std::int32_t> cat, std::int64_t n,
                         rust::Slice<const std::int32_t> n_levels, rust::Slice<const double> p, rust::Str classes,
                         double regmult, bool add_samples_to_background);
rust::Vec<double> sdm_maxnet_predict(rust::Slice<const std::int32_t> kind, rust::Slice<const std::int32_t> var,
                                     rust::Slice<const std::int32_t> var2, rust::Slice<const double> pk1,
                                     rust::Slice<const double> pk2, rust::Slice<const double> betas,
                                     rust::Slice<const double> feature_min, rust::Slice<const double> feature_max,
                                     rust::Slice<const double> var_min, rust::Slice<const double> var_max, double alpha,
                                     double entropy, rust::Slice<const double> cont, rust::Slice<const std::int32_t> cat,
                                     std::int64_t n, rust::Slice<const std::int32_t> n_levels, bool clamp,
                                     std::int32_t output);
rust::String sdm_r_as_character(double x);
double sdm_r_cor2(rust::Slice<const double> x, rust::Slice<const double> y, bool kendall);
rust::Vec<double> sdm_r_colmeans(rust::Slice<const double> x, std::int64_t n, std::int64_t p);
rust::Vec<double> nlm_rescale(rust::Slice<const double> x);
rust::Vec<double> nlm_random(std::int32_t seed, std::int64_t ny, std::int64_t nx);
rust::Vec<double> nlm_percolation(std::int32_t seed, std::int64_t ny, std::int64_t nx, double prob);
rust::Vec<double> nlm_planar_gradient(std::int64_t ny, std::int64_t nx, double direction, bool rescale, bool edge);
rust::Vec<double> nlm_distance(rust::Slice<const std::uint8_t> origin, std::int64_t ny, std::int64_t nx,
                               std::int32_t method, double res, double xmin, double ymax, double xres, double yres);
rust::Vec<double> nlm_r_cumsum(rust::Slice<const double> x);
rust::Vec<double> nlm_cut(rust::Slice<const double> x, rust::Slice<const double> breaks);
rust::Vec<double> nlm_jenks_breaks(rust::Slice<const double> sorted, std::int32_t k);
rust::Vec<double> nlm_curds(std::int32_t seed, rust::Slice<const double> curds, rust::Slice<const std::int64_t> steps,
                            rust::Slice<const double> wheyes, bool with_wheyes);
rust::Vec<double> nlm_mpd(std::int64_t size, rust::Slice<const double> rand_dev, std::uint32_t seed, bool torus);
rust::Vec<double> nlm_neigh(std::int64_t ny, std::int64_t nx, std::int32_t n_categories,
                            rust::Slice<const std::int64_t> cells_per_cat, std::int32_t neighbourhood, float p_neigh,
                            float p_empty, std::uint32_t seed);
rust::Vec<double> nlm_random_rectangular_cluster(std::int64_t ny, std::int64_t nx, std::int32_t minl, std::int32_t maxl,
                                                 std::uint32_t seed);
NlmMosaicOut nlm_mosaic_field(std::int32_t seed, std::int64_t ny, std::int64_t nx, std::int32_t n, bool nlmr_compat,
                              double mean, double sd, bool collect, bool infinit);
NlmGermsOut nlm_tess_germs(std::int32_t seed, std::int64_t germs, double width, double height);
NlmGermsOut nlm_gibbs_germs(std::int32_t seed, std::int64_t germs, double r, std::int32_t patch_classes, double width,
                            double height, std::int32_t giveup);
rust::Vec<double> nlm_nearest_germ_raster(rust::Slice<const double> x, rust::Slice<const double> y,
                                          rust::Slice<const double> v, std::int64_t ny, std::int64_t nx);

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
SolveOut opt_solve_watched(const MilpModel& model, const SolveSettings& settings, rust::Str method,
                           std::int64_t n_units, SolveWatch& watch);
struct EngineOut;
EngineOut opt_solve_cut(rust::Slice<const double> cost, rust::Slice<const std::int32_t> status,
                        rust::Slice<const std::int64_t> offset, rust::Slice<const std::int32_t> spindex,
                        rust::Slice<const double> amount, rust::Slice<const double> target,
                        rust::Slice<const std::int64_t> nb_start, rust::Slice<const std::int32_t> nb,
                        rust::Slice<const double> nb_cost, double gap, double time_limit,
                        rust::Slice<const std::int32_t> initial, std::int32_t threads, SolveWatch& watch);
EngineOut opt_solve_closure(rust::Slice<const double> cost, rust::Slice<const std::int8_t> fixed,
                            rust::Slice<const std::int32_t> pen_i, rust::Slice<const std::int32_t> pen_j,
                            rust::Slice<const double> pen_w, rust::Slice<const double> rhs,
                            rust::Slice<const std::uint8_t> geq, rust::Slice<const std::int64_t> col_start,
                            rust::Slice<const std::int32_t> col_row, rust::Slice<const double> col_val, double gap,
                            double time_limit, std::int32_t threads, rust::Slice<const std::uint8_t> initial,
                            SolveWatch& watch);

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
struct HydroLinks;
struct ServClasses;
struct GeomorphonsOut;
struct HilltopsOut;
struct ContinuousOut;
struct FrictionOut;
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
rust::Vec<std::uint8_t> lsm_rasterize(rust::Slice<const std::int64_t> sizes, rust::Slice<const double> xs,
                                      rust::Slice<const double> ys, rust::Slice<const double> gt, std::int32_t nx,
                                      std::int32_t ny);
rust::Vec<double> lsm_r_sincos(rust::Slice<const double> x);
rust::Vec<double> lsm_r_cor(rust::Slice<const double> x, std::int64_t n, std::int64_t m, rust::Str method);
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

// simulate bridge (landschaft-bb)

// gdistance's accCost (landschaft's ls.cost.acc_cost): the accumulated cost from the nearest source cell over a
// transition of conductances (values: directions x cells, as ls.cost.Transition.values); inf where unreachable.
rust::Vec<double> cost_accumulated(rust::Slice<const double> values, std::int64_t ny, std::int64_t nx, double xmin,
                                   double xmax, double ymin, double ymax, bool wrap, std::int32_t directions,
                                   bool symmetric, rust::Slice<const std::int64_t> sources);

// MOLUSCE's simulator (landschaft's sim/molusce.hpp) and numpy's legacy generator
std::unique_ptr<sim::NumpyRandomState> numpy_rng_new(std::uint32_t seed);
rust::Vec<double> numpy_rng_random_sample(sim::NumpyRandomState& rng, std::size_t n);
rust::Vec<double> molusce_categories(rust::Slice<const double> values, rust::Slice<const double> bounds);
rust::Vec<double> molusce_total(rust::Slice<const double> factors, std::int32_t nf, std::int64_t n,
                                rust::Slice<const MolusceFactor> weights);
MolusceForecastOut molusce_prediction_woe(rust::Slice<const double> state, rust::Slice<const double> classes,
                                          rust::Slice<const std::int64_t> codes, rust::Slice<const double> totals);
MolusceForecastOut molusce_prediction_classes(rust::Slice<const double> state, rust::Slice<const double> layers,
                                              rust::Slice<const double> classes);
MolusceStepOut molusce_ca_step(rust::Slice<const double> state, rust::Slice<const double> prediction,
                               rust::Slice<const double> confidence, rust::Slice<const MolusceChange> counts,
                               sim::NumpyRandomState& rng);

// CLUMondo (landschaft's sim/clumondo.hpp)
ClumondoYearOut clumondo_year(const ClumondoSettingsIn& s, rust::Slice<const std::int32_t> oldco,
                              rust::Slice<const std::int32_t> region, rust::Slice<const std::int32_t> age,
                              rust::Slice<const float> suit, rust::Slice<const float> allow_maps,
                              rust::Slice<const double> demand, rust::Slice<const float> dem_elas);
rust::Vec<float> clumondo_logit(rust::Slice<const std::int32_t> region, rust::Slice<const float> x, double constant,
                                rust::Slice<const float> coef);
rust::Vec<float> clumondo_add_neighbourhood(rust::Slice<const float> suit, std::int64_t ny, std::int64_t nx,
                                            rust::Slice<const std::int32_t> oldco,
                                            rust::Slice<const std::int32_t> region, rust::Slice<const ClumondoNbIn> parts);
rust::Vec<float> clumondo_add_preference(rust::Slice<const float> row, rust::Slice<const std::int32_t> region,
                                         rust::Slice<const float> values, float weight);
rust::Vec<std::int32_t> clumondo_random_age(std::int64_t ncell, std::int32_t max_age, std::uint32_t seed);

// FUTURES (landschaft's sim/futures.hpp): every step in one run
FuturesOut futures_run(const FuturesIn& in, const FuturesClimateIn& climate, rust::Slice<const std::int32_t> developed,
                       rust::Slice<const std::int32_t> region, rust::Slice<const std::int32_t> pot_region,
                       rust::Slice<const float> devpressure, rust::Slice<const float> aggregated,
                       rust::Slice<const float> weight, rust::Slice<const std::int32_t> zone);

}  // namespace tfx

// out-of-core landscapes (tiles): landschaft's lsm::TiledLandscape, a raster labelled block by block of rows
#include "lsm/landscape.hpp"

namespace tfx {

std::unique_ptr<lsm::TiledLandscape> lsm_tiled_new(std::int64_t nx, std::int64_t ny, double resx, double resy,
                                                   const LsmOptions& options,
                                                   rust::Slice<const std::int32_t> class_values);
rust::Vec<std::int32_t> lsm_unique_values(rust::Slice<const std::int32_t> values, rust::Slice<const std::uint8_t> na);
void lsm_tiled_add(lsm::TiledLandscape& t, rust::Slice<const std::int32_t> values, rust::Slice<const std::uint8_t> na,
                   std::int64_t y0, std::int64_t rows, std::int64_t above, std::int64_t below);
void lsm_tiled_finish(lsm::TiledLandscape& t);
std::int64_t lsm_tiled_detail_halo(const lsm::TiledLandscape& t, bool core, bool enn);
void lsm_tiled_begin_detail(lsm::TiledLandscape& t, bool core, bool gyrate, bool circle, bool enn);
void lsm_tiled_detail(lsm::TiledLandscape& t, rust::Slice<const std::int32_t> values,
                      rust::Slice<const std::uint8_t> na, std::int64_t y0, std::int64_t rows, std::int64_t above,
                      std::int64_t below);
bool lsm_tiled_end_detail(lsm::TiledLandscape& t);
void lsm_tiled_tie(lsm::TiledLandscape& t, rust::Slice<const std::int32_t> values, rust::Slice<const std::uint8_t> na,
                   std::int64_t y0, std::int64_t rows, std::int64_t above, std::int64_t below);
void lsm_tiled_end_ties(lsm::TiledLandscape& t);
LsmTable lsm_tiled_table(lsm::TiledLandscape& t, rust::Slice<const std::int32_t> levels,
                         rust::Slice<const rust::String> names);
rust::Vec<std::int32_t> lsm_tiled_labels(const lsm::TiledLandscape& t, rust::Slice<const std::int32_t> values,
                                         rust::Slice<const std::uint8_t> na, std::int64_t y0, std::int64_t rows);

// Omniscape's moving-window circuits and Circuitscape's advanced mode on a raster (landschaft's circuit module)
OmniscapeOut circuit_omniscape(rust::Slice<const double> resistance, rust::Slice<const double> source, std::int64_t nx,
                               std::int64_t ny, std::int32_t radius, std::int32_t buffer, std::int32_t block_size,
                               bool four_neighbors, bool resistance_is_conductance, bool correct_artifacts,
                               bool flow_potential, rust::Slice<const double> condition1,
                               rust::Slice<const double> condition1_future, rust::Slice<const double> condition2,
                               rust::Slice<const double> condition2_future, rust::Slice<const double> comparisons,
                               double memory_limit);
CircuitOut circuit_advanced(rust::Slice<const double> conductance, rust::Slice<const double> source,
                            rust::Slice<const double> ground, std::int64_t nx, std::int64_t ny, bool four_neighbors);
PairwiseOut circuit_pairwise(rust::Slice<const double> gmap, rust::Slice<const std::int64_t> polymap, std::int64_t nx,
                             std::int64_t ny, rust::Slice<const std::int64_t> point_cells,
                             rust::Slice<const std::int64_t> region_ids, rust::Slice<const std::uint8_t> excluded,
                             bool avg_resistances, bool four_neighbors, rust::Slice<const std::uint8_t> flags);
MultiOut circuit_multi_advanced(rust::Slice<const double> gmap, rust::Slice<const std::int64_t> polymap, std::int64_t nx,
                                std::int64_t ny, bool avg_resistances, bool four_neighbors,
                                rust::Slice<const std::int64_t> src_cells, rust::Slice<const double> src_vals,
                                rust::Slice<const std::int64_t> src_offsets, rust::Slice<const std::int64_t> gnd_cells,
                                rust::Slice<const double> gnd_vals, rust::Slice<const std::int64_t> gnd_offsets,
                                rust::Slice<const std::int64_t> checks, std::int32_t policy);

// surface metrics (landschaft's surface module)
rust::Vec<double> surf_metrics(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                               rust::Slice<const double> extent, bool lonlat, rust::Slice<const std::int32_t> ids);
rust::Vec<double> surf_window(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                              rust::Slice<const double> extent, bool lonlat, std::int32_t size, bool circle,
                              rust::Slice<const std::int32_t> ids);

rust::Vec<double> surf_terrain(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                               rust::Slice<const double> extent, bool lonlat, std::int32_t neighbors, bool degrees,
                               bool aspect);
rust::Vec<double> surf_focal(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                             rust::Slice<const double> extent, std::int32_t wr, std::int32_t wc,
                             rust::Slice<const std::uint8_t> mask, std::int32_t stat);
rust::Vec<double> surf_tri(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                           rust::Slice<const double> extent, std::int32_t wr, std::int32_t wc);
rust::Vec<double> surf_curvature(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                                 rust::Slice<const double> extent, std::int32_t kind);
rust::Vec<double> surf_tpi(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                           rust::Slice<const double> extent, std::int32_t wr, std::int32_t wc,
                           rust::Slice<const std::uint8_t> mask, bool normalize);
rust::Vec<double> surf_hsp(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                           rust::Slice<const double> extent, rust::Slice<const std::int32_t> rows,
                           rust::Slice<const std::int32_t> cols, rust::Slice<const std::uint8_t> masks,
                           bool normalize);
PlaneOut surf_fit_plane(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                        rust::Slice<const double> extent, std::int32_t order);
PlaneOut surf_best_fit_plane(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                             rust::Slice<const double> extent);
rust::Vec<std::int64_t> surf_peaks(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                                   rust::Slice<const double> extent, bool valleys);
BearingOut surf_bearing(rust::Slice<const double> z, rust::Slice<const double> probs);
AacfOut surf_aacf(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny, rust::Slice<const double> extent,
                  bool lonlat);

// grid distances (landschaft's cost module, gdistance's)
rust::Vec<std::int32_t> cost_offsets(std::int32_t directions, bool symmetric);
rust::Vec<double> cost_builtin(rust::Slice<const double> x, rust::Slice<const double> grid, std::int32_t directions,
                               bool symmetric, rust::Str function);
rust::Vec<double> cost_geo_correction(rust::Slice<const double> values, rust::Slice<const double> grid,
                                      std::int32_t directions, bool symmetric, bool lonlat, rust::Str kind,
                                      bool resistance, bool scale, bool factors);
rust::Vec<double> cost_matrix(rust::Slice<const double> values, rust::Slice<const double> grid,
                              std::int32_t directions, bool symmetric, rust::Slice<const std::int64_t> from,
                              rust::Slice<const std::int64_t> to);
rust::Vec<std::int64_t> cost_neighbours(rust::Slice<const double> grid, std::int32_t directions, bool symmetric);
rust::Vec<double> cost_margin_sums(rust::Slice<const double> values, rust::Slice<const double> grid,
                                   std::int32_t directions, bool symmetric, std::int32_t margin, bool count);
rust::Vec<double> cost_rmath(rust::Str name, rust::Slice<const double> x, double y);
CommuteOut cost_commute(rust::Slice<const double> values, rust::Slice<const double> grid, std::int32_t directions,
                        bool symmetric, rust::Slice<const std::int64_t> cells);
rust::Vec<double> cost_rw_passages(rust::Slice<const double> values, rust::Slice<const double> grid,
                                   std::int32_t directions, bool symmetric, rust::Slice<const std::int64_t> origins,
                                   rust::Slice<const std::int64_t> goals, bool quirk);
rust::Vec<double> cost_rsp_passages(rust::Slice<const double> values, rust::Slice<const double> grid,
                                    std::int32_t directions, bool symmetric, rust::Slice<const std::int64_t> origins,
                                    rust::Slice<const std::int64_t> goals, double theta, bool net, bool edges);
rust::Vec<double> cost_rsp_distances(rust::Slice<const double> values, rust::Slice<const double> grid,
                                     std::int32_t directions, bool symmetric, rust::Slice<const std::int64_t> from,
                                     rust::Slice<const std::int64_t> to, double theta, bool net, std::int32_t method);
EdgesOut cost_graph_edges(rust::Slice<const double> values, rust::Slice<const double> grid, std::int32_t directions,
                          bool symmetric);
rust::Vec<double> cost_path_flows(rust::Slice<const double> values, rust::Slice<const double> grid,
                                  std::int32_t directions, bool symmetric, std::int64_t origin,
                                  rust::Slice<const std::int64_t> locs, double theta, bool quirk);
rust::Vec<double> cost_path_overlap(rust::Slice<const double> fa, std::int64_t na, rust::Slice<const double> fb,
                                    std::int64_t nb, std::int64_t ne, rust::Slice<const double> r, std::int64_t nr,
                                    rust::Slice<const std::int32_t> functions, bool upper);
rust::Vec<double> cost_r_cov(rust::Slice<const double> x, std::int64_t n, std::int64_t k);
double cost_r_mean(rust::Slice<const double> x);
// Biodiverse (landschaft's bio module)
BioCsr bio_find_neighbours(const BioGroups& groups, const BioProgram& program, rust::Slice<const std::int32_t> procs,
                           bool parallel);
rust::Vec<std::uint8_t> bio_definition_query(const BioGroups& groups, const BioProgram& program, bool parallel);
rust::Vec<double> bio_side_rotation(rust::Slice<const double> coords, std::int32_t ndim, std::int32_t axis0,
                                    std::int32_t axis1, double angle);
rust::Vec<BioCalcInfo> bio_calculations();
BioIndexOut bio_run_indices(const BioView& view, rust::Slice<const std::int32_t> procs, const BioCsr& set1, bool two,
                            const BioCsr& set2, const rust::Vec<rust::String>& calcs, bool parallel, bool with_tree,
                            const BioTree& tree);
rust::Vec<std::uint8_t> bio_ultrametric(const BioTree& tree);

struct BioRng {
    bio::MT64 mt;
    explicit BioRng(std::uint64_t seed) : mt({seed}) {}
};
struct BioAlloc {
    std::unique_ptr<bio::SpatialAlloc> a;
};
std::unique_ptr<BioRng> bio_rng_new(std::uint64_t seed);
rust::Vec<std::int64_t> bio_rng_shuffle(BioRng& rng, std::int64_t n);
std::unique_ptr<BioAlloc> bio_alloc_none();
std::unique_ptr<BioAlloc> bio_alloc_new(rust::Str order, rust::Str backtracking, double reseed_prob, std::int64_t n_groups,
                                        std::int32_t n_sets, rust::Slice<const std::int64_t> nbr_ptr,
                                        rust::Slice<const std::int32_t> nbr_idx, bool same,
                                        rust::Slice<const double> coords, std::int32_t ndim, bool have_seeds,
                                        rust::Slice<const std::int32_t> seeds);
BioCounts bio_rand_structured(rust::Slice<const std::int64_t> indptr, rust::Slice<const std::int32_t> indices,
                              rust::Slice<const double> counts, std::int64_t n_groups, std::int64_t n_labels,
                              BioRng& rng, double multiplier, double addition, BioAlloc& alloc);
BioCounts bio_rand_swaps(rust::Slice<const std::int64_t> indptr, rust::Slice<const std::int32_t> indices,
                         rust::Slice<const double> counts, std::int64_t n_groups, std::int64_t n_labels, BioRng& rng,
                         rust::Slice<const double> label_range, rust::Str kind, std::int64_t swap_count,
                         std::int64_t max_swap_attempts, bool stop_on_all_swapped, bool use_max_swap);
rust::Vec<double> bio_perl_sqrt(rust::Slice<const double> x);
CsAdvancedOut circuit_cs_advanced(rust::Slice<const double> gmap, rust::Slice<const std::int64_t> polymap, std::int64_t nx,
                                  std::int64_t ny, rust::Slice<const double> source, rust::Slice<const double> ground,
                                  bool avg_resistances, bool four_neighbors, std::int32_t policy);
NetworkOut circuit_network_advanced(rust::Slice<const std::int32_t> a, rust::Slice<const std::int32_t> b,
                                    rust::Slice<const double> w, std::int32_t m, rust::Slice<const double> source,
                                    rust::Slice<const double> ground, std::int32_t policy);
NetworkOut circuit_network_pairwise(rust::Slice<const std::int32_t> a, rust::Slice<const std::int32_t> b,
                                    rust::Slice<const double> w, std::int32_t m, rust::Slice<const std::int32_t> points,
                                    rust::Slice<const std::uint8_t> excluded, bool volt_maps, bool cur_maps, bool cum);
BioClusterOut bio_cluster(const rust::Vec<rust::String>& names, std::int32_t n_sets, rust::Slice<const std::int64_t> nbr_ptr,
                          rust::Slice<const std::int32_t> nbr_idx, bool trim_processed, rust::Str direct,
                          rust::Slice<const std::int64_t> label_ptr, rust::Slice<const std::int32_t> label_idx,
                          rust::Slice<const std::int64_t> pair_keys, rust::Slice<const double> pair_values,
                          rust::Str linkage, bool can_lump_zeroes, double min_poss, bool have_tie_breaker,
                          const rust::Vec<rust::String>& breaker_keys, const rust::Vec<rust::String>& breaker_minmax,
                          bool objective_max, double max_poss, BioRng& rng, bool use_recalc, ClusterRecalc& recalc);

PathsOut cost_paths(rust::Slice<const double> values, rust::Slice<const double> grid, std::int32_t directions,
                    bool symmetric, std::int64_t origin, rust::Slice<const std::int64_t> goals);

// the values of maps at points (landschaft's extract, lsio::extract): bands x ny x nx, NaN no data
rust::Vec<double> io_extract(rust::Slice<const double> values, std::int32_t bands, std::int32_t ny, std::int32_t nx,
                             rust::Slice<const double> gt, rust::Slice<const double> x, rust::Slice<const double> y,
                             bool bilinear, std::int32_t row0, std::int32_t ny_total);

// terrain hydrology (landschaft's hydro); an empty optional raster is none
rust::Vec<double> hydro_fill(rust::Slice<const double> z, std::int32_t ny, std::int32_t nx, bool epsilon);
rust::Vec<double> hydro_breach(rust::Slice<const double> z, std::int32_t ny, std::int32_t nx, double dx, double dy,
                               std::int32_t max_length, double max_cost, bool min_dist, double small_num,
                               bool fill_rest);
rust::Vec<std::int8_t> hydro_d8(rust::Slice<const double> z, std::int32_t ny, std::int32_t nx, double dx, double dy,
                                bool resolve_flats, std::int32_t threads);
rust::Vec<std::int8_t> hydro_flat_directions(rust::Slice<const double> z, rust::Slice<const std::int8_t> dir,
                                             std::int32_t ny, std::int32_t nx, double dx, double dy);
rust::Vec<double> hydro_d8_accumulation(rust::Slice<const std::int8_t> dir, rust::Slice<const double> weight,
                                        std::int32_t ny, std::int32_t nx);
rust::Vec<double> hydro_dinf(rust::Slice<const double> z, std::int32_t ny, std::int32_t nx, double dx, double dy,
                             rust::Slice<const std::int8_t> flats, std::int32_t threads);
rust::Vec<double> hydro_dinf_accumulation(rust::Slice<const double> angle, rust::Slice<const double> weight,
                                          std::int32_t ny, std::int32_t nx);
rust::Vec<double> hydro_mfd_accumulation(rust::Slice<const double> z, std::int32_t ny, std::int32_t nx, double dx,
                                         double dy, double exponent, bool quinn, rust::Slice<const std::int8_t> flats,
                                         rust::Slice<const double> weight);
rust::Vec<std::int32_t> hydro_mfd_directions(rust::Slice<const double> z, std::int32_t ny, std::int32_t nx,
                                             std::int32_t block);
rust::Vec<double> hydro_mfd_packed_accumulation(rust::Slice<const std::int32_t> dir, rust::Slice<const double> weight,
                                                std::int32_t ny, std::int32_t nx);
rust::Vec<double> hydro_mfd_distance_to_channel(rust::Slice<const std::int32_t> dir,
                                                rust::Slice<const std::uint8_t> channel,
                                                rust::Slice<const double> weight, std::int32_t ny, std::int32_t nx);
rust::Vec<std::uint8_t> hydro_mfd_streams(rust::Slice<const double> acc, rust::Slice<const std::int32_t> dir,
                                          std::int32_t ny, std::int32_t nx, double threshold, double trace,
                                          std::int32_t block);
rust::Vec<std::uint8_t> hydro_pgp_d8(rust::Slice<const double> z, std::int32_t ny, std::int32_t nx,
                                     std::int32_t block);
rust::Vec<double> hydro_pgp_d8_accumulation(rust::Slice<const std::uint8_t> dir, rust::Slice<const double> weight,
                                            std::int32_t ny, std::int32_t nx);
rust::Vec<double> hydro_pgp_d8_distance_to_channel(rust::Slice<const std::uint8_t> dir,
                                                   rust::Slice<const std::uint8_t> channel,
                                                   rust::Slice<const double> weight, std::int32_t ny, std::int32_t nx);
rust::Vec<double> hydro_hand(rust::Slice<const double> z, rust::Slice<const std::int8_t> dir,
                             rust::Slice<const std::uint8_t> streams, std::int32_t ny, std::int32_t nx);
HydroLinks hydro_stream_links(rust::Slice<const std::int8_t> dir, rust::Slice<const std::uint8_t> stream,
                              rust::Slice<const std::uint8_t> breaks, std::int32_t ny, std::int32_t nx);
rust::Vec<std::int32_t> hydro_watersheds(rust::Slice<const std::int8_t> dir, rust::Slice<const std::int32_t> seeds,
                                         std::int32_t ny, std::int32_t nx);

// ecosystem services (landschaft's serv: InVEST's models)
ServClasses serv_classify(rust::Slice<const std::int64_t> lulc, rust::Slice<const std::uint8_t> valid,
                          rust::Slice<const std::int64_t> codes, std::int32_t threads);
rust::Vec<float> serv_carbon_pools(rust::Slice<const std::int32_t> cls, rust::Slice<const float> pools,
                                   std::int32_t threads);
double serv_total(rust::Slice<const float> x, std::int32_t threads);
rust::Vec<float> serv_distance_transform(rust::Slice<const std::uint8_t> region, std::int32_t ny, std::int32_t nx,
                                         std::int32_t threads);
rust::Vec<float> serv_threat_degradation(rust::Slice<const float> dist, rust::Slice<const float> given,
                                         rust::Slice<const std::int32_t> cls, rust::Slice<const float> sens,
                                         rust::Slice<const float> access, double max_px, bool exponential,
                                         double weight, rust::Slice<float> total, std::int32_t threads);
rust::Vec<float> serv_quality(rust::Slice<const float> total, rust::Slice<const std::int32_t> cls,
                              rust::Slice<const float> habitat, double ksq, std::int32_t threads);
rust::Vec<float> serv_water_fraction(rust::Slice<const std::int32_t> cls, rust::Slice<const float> kc,
                                     rust::Slice<const float> root, rust::Slice<const float> veg,
                                     rust::Slice<const double> eto, rust::Slice<const double> precip,
                                     rust::Slice<const double> soil, rust::Slice<const double> pawc, double z,
                                     bool wide, std::int32_t threads);
rust::Vec<double> serv_water_yield(rust::Slice<const float> fractp, rust::Slice<const double> precip, bool wide,
                                   std::int32_t threads);
rust::Vec<double> serv_potential_et(rust::Slice<const std::int32_t> cls, rust::Slice<const float> kc,
                                    rust::Slice<const double> eto, bool wide, std::int32_t threads);
rust::Vec<float> serv_pgp_slope(rust::Slice<const double> z, std::int32_t ny, std::int32_t nx, double xcell,
                                double ycell, std::int32_t threads);
rust::Vec<float> serv_sediment_deposition(rust::Slice<const std::int32_t> mfd, rust::Slice<const std::uint8_t> d8,
                                          rust::Slice<const float> e_prime, rust::Slice<const float> sdr,
                                          std::int32_t ny, std::int32_t nx, std::int32_t block);

// surfaces: geomorphons, hilltops, gravity
GeomorphonsOut surf_geomorphons(rust::Slice<const float> z, std::int32_t ny, std::int32_t nx, double north,
                                double west, double ns_res, double ew_res, std::int32_t search_cells,
                                std::int32_t skip_cells, double search_distance, double flat_threshold,
                                double flat_distance, double flat_height, double max_resolution, bool extended,
                                std::int32_t comparison, std::uint32_t outputs);
HilltopsOut surf_hilltops(rust::Slice<const double> z, rust::Slice<const double> slope,
                          rust::Slice<const std::uint8_t> plain, std::int32_t ny, std::int32_t nx, double dx, double dy,
                          double min_height, double min_slope, bool line_slope, double fraction);
rust::Vec<double> surf_gravity(rust::Slice<const double> mass, rust::Slice<const double> mask, std::int32_t ny,
                               std::int32_t nx, double beta, double dx, double dy, double self_distance);
rust::Vec<double> surf_gravity_fft(rust::Slice<const double> mass, rust::Slice<const double> weights, std::int32_t ny,
                                   std::int32_t nx);
rust::Vec<double> surf_gravity_weights(std::int32_t ny, std::int32_t nx, double beta, double dx, double dy,
                                       double self_distance);

// simulation: the Patcher of a continuous state, endogenous roads
ContinuousOut sim_continuous_patches(rust::Slice<const double> stock, rust::Slice<const double> probability,
                                     std::int64_t ny, std::int64_t nx, double quota, double mean, double variance,
                                     double isometry, double prune_factor, std::int32_t win_rows,
                                     std::int32_t win_cols, std::uint64_t seed);
FrictionOut sim_friction_cost(rust::Slice<const std::uint8_t> source, rust::Slice<const double> friction,
                              std::int64_t ny, std::int64_t nx, double diagonal, bool directions);
rust::Vec<std::int64_t> sim_destinations(rust::Slice<const double> attraction, std::int64_t ny, std::int64_t nx,
                                         std::int32_t horizontal, std::int32_t vertical, std::int32_t per_quadrant,
                                         double min_distance, std::int32_t max_attempts, double dx, double dy,
                                         std::uint64_t seed);
rust::Vec<std::uint8_t> sim_pathways(rust::Slice<const std::uint8_t> network, rust::Slice<const std::uint8_t> sources,
                                     rust::Slice<const double> cost, rust::Slice<const std::uint8_t> dir,
                                     std::int64_t ny, std::int64_t nx, bool lottery, std::uint64_t seed);

}  // namespace tfx
