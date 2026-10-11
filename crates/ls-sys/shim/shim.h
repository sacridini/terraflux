// SPDX-License-Identifier: GPL-3.0-or-later
//
// The shim between Terraflux (Rust, cxx) and landschaft's C++ kernels.
#pragma once

#include <cstdint>
#include <memory>

#include "nlm/rng.hpp"
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
DistanceOut euclidean_distance(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx);

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
