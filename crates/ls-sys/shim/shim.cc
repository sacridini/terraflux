// SPDX-License-Identifier: GPL-3.0-or-later
//
// The shim between Terraflux (Rust, cxx) and landschaft's C++ kernels: Rust
// slices and vectors in, landschaft's types and calls, Rust vectors out.
// Exceptions of the kernels reach Rust as errors (cxx's Result).

#include "terraflux-ls-sys/shim/shim.h"

#include <array>
#include <stdexcept>
#include <string>

#ifdef _OPENMP
#include <omp.h>
#endif
#include <vector>

#include "gtb/accounting.hpp"
#include "gtb/change.hpp"
#include "gtb/contagion.hpp"
#include "gtb/distance.hpp"
#include "gtb/entropy.hpp"
#include "gtb/fad.hpp"
#include "gtb/fos.hpp"
#include "gtb/grayspatcon.hpp"
#include "gtb/morphology.hpp"
#include "gtb/mosaic.hpp"
#include "gtb/mspa.hpp"
#include "gtb/spa.hpp"
#include "lsm/landscape.hpp"
#include "rest/habitat.hpp"
#include "rest/intactness.hpp"
#include "lsm/rmath.hpp"
#include "nlm/models.hpp"
#include "sim/change.hpp"
#include "sim/clues.hpp"
#include "sim/demand.hpp"
#include "sim/observed.hpp"
#include "sim/ordered.hpp"
#include "sim/patches.hpp"
#include "sim/plus.hpp"
#include "sim/rglm.hpp"
#include "sim/rules.hpp"
#include "sim/similarity.hpp"
#include "sim/woe.hpp"
#include "terraflux-ls-sys/src/lib.rs.h"

namespace tfx {

namespace {

using i64 = std::int64_t;

template <class T, class U = T>
rust::Vec<T> to_rust(const std::vector<U>& v) {
    rust::Vec<T> out;
    out.reserve(v.size());
    for (const auto& x : v) out.push_back(static_cast<T>(x));
    return out;
}

template <class T>
std::vector<T> to_std(rust::Slice<const T> s) {
    return std::vector<T>(s.begin(), s.end());
}

sim::MapView view(rust::Slice<const double> values) {
    sim::MapView m;
    m.data = values.data();
    m.type = sim::DType::f64;
    m.n = static_cast<i64>(values.size());
    return m;
}

void check(bool ok, const char* what) {
    if (!ok) throw std::invalid_argument(what);
}

// a map of bytes (row-major ny x nx) as GTB's image
gtb::ImageU8 image(rust::Slice<const std::uint8_t> input, i64 ny, i64 nx) {
    check(ny >= 0 && nx >= 0 && static_cast<i64>(input.size()) == ny * nx, "the map differs in size from ny x nx");
    gtb::ImageU8 im(nx, ny);
    if (im.size() > 0) std::copy(input.begin(), input.end(), im.data());
    return im;
}

template <class T>
rust::Vec<T> pixels(const gtb::Image<T>& im) {
    rust::Vec<T> v;
    v.reserve(static_cast<std::size_t>(im.size()));
    for (i64 i = 0; i < im.size(); ++i) v.push_back(im[i]);
    return v;
}

void check_window(std::int32_t w) { check(w >= 3 && w % 2 == 1, "the window's side is an odd number of cells, 3 or more"); }

}  // namespace

void set_threads(std::int32_t n) {
#ifdef _OPENMP
    omp_set_num_threads(n < 1 ? 1 : n);
#else
    (void)n;
#endif
}

std::int32_t max_threads() {
#ifdef _OPENMP
    return omp_get_max_threads();
#else
    return 1;
#endif
}

std::unique_ptr<nlm::RRng> rrng_new(std::int32_t seed) { return std::make_unique<nlm::RRng>(seed); }

double rrng_unif_rand(nlm::RRng& rng) { return rng.unif_rand(); }

rust::Vec<double> rrng_runif(nlm::RRng& rng, std::size_t n) {
    std::vector<double> v(n);
    rng.runif(v.data(), static_cast<i64>(n));
    return to_rust<double>(v);
}

rust::Vec<double> map_classes(rust::Slice<const double> values) { return to_rust<double>(sim::map_classes(view(values))); }

rust::Vec<std::int64_t> crosstab(rust::Slice<const double> a, rust::Slice<const double> b,
                                 rust::Slice<const double> classes_a, rust::Slice<const double> classes_b) {
    check(a.size() == b.size(), "the maps differ in size");
    return to_rust<std::int64_t>(sim::crosstab(view(a), view(b), to_std(classes_a), to_std(classes_b)));
}

double r_sum(rust::Slice<const double> x) { return lsm::r_sum(x.data(), static_cast<i64>(x.size())); }

rust::Vec<double> focal_mean(rust::Slice<const double> x, std::int64_t ny, std::int64_t nx,
                             rust::Slice<const double> w, std::int32_t wy, std::int32_t wx) {
    check(static_cast<i64>(x.size()) == ny * nx, "the map differs in size from ny x nx");
    check(wy % 2 == 1 && wx % 2 == 1 && static_cast<i64>(w.size()) == i64{wy} * wx, "the window needs odd sides");
    return to_rust<double>(sim::focal_mean(x.data(), ny, nx, w.data(), wy, wx));
}

rust::Vec<double> distance_exact(rust::Slice<const std::uint8_t> origin, std::int64_t ny, std::int64_t nx, double res) {
    check(static_cast<i64>(origin.size()) == ny * nx, "the map differs in size from ny x nx");
    return to_rust<double>(nlm::distance_exact(to_std(origin), ny, nx, res));
}

rust::Vec<double> nlm_random_cluster(std::int32_t seed, std::int64_t ny, std::int64_t nx, double p,
                                     rust::Slice<const double> ai, std::int32_t neighbourhood) {
    check(ny > 0 && nx > 0, "the map needs rows and columns");
    check(p > 0.0 && p <= 1.0, "p is a share of the cells, in (0, 1]");
    check(!ai.empty(), "give the types' weights");
    check(neighbourhood == 4 || neighbourhood == 8, "the neighbourhood is 4 or 8");
    nlm::RRng rng(seed);
    return to_rust<double>(nlm::random_cluster(rng, ny, nx, p, to_std(ai), neighbourhood));
}

rust::Vec<double> nlm_gaussian_field(std::int32_t seed, std::int64_t ny, std::int64_t nx, double autocorr_range,
                                     double mag_var, double nug, double mean) {
    check(ny > 0 && nx > 0, "the map needs rows and columns");
    check(autocorr_range > 0.0, "the range of autocorrelation is above 0");
    nlm::RRng rng(seed);
    return to_rust<double>(nlm::gaussian_field(rng, ny, nx, autocorr_range, mag_var, nug, mean));
}

rust::Vec<double> nlm_fbm(std::int32_t seed, std::int64_t ny, std::int64_t nx, double fract_dim) {
    check(ny > 0 && nx > 0, "the map needs rows and columns");
    check(fract_dim > 0.0 && fract_dim <= 2.0, "the fractal dimension is in (0, 2]");
    nlm::RRng rng(seed);
    return to_rust<double>(nlm::fbm(rng, ny, nx, fract_dim));
}

rust::Vec<std::uint8_t> mspa(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                             std::int32_t connectivity, std::int32_t edge_width, bool internal) {
    const gtb::ImageU8 im = image(input, ny, nx);
    gtb::MspaParams p;
    p.connectivity = connectivity;
    p.edge_width = edge_width;
    p.internal = internal;
    p.threads = 0;  // the stages in parallel, the kernels with the calling thread's OpenMP threads
    return pixels(gtb::mspa(im, p));
}

FadOut fad(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx, std::int32_t window) {
    check_window(window);
    const gtb::FADSingleResultCpp r = gtb::compute_fad(image(input, ny, nx), window);
    FadOut out;
    out.density = pixels(r.density);
    out.raw = pixels(r.raw);
    out.classes = pixels(r.classes);
    out.foreground = r.foreground;
    for (i64 n : r.class_counts) out.class_counts.push_back(n);
    out.density_sum = r.density_sum;
    return out;
}

FadScalesOut fad_multiscale(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                            rust::Slice<const std::int32_t> windows) {
    check(!windows.empty(), "give one window or more");
    for (std::int32_t w : windows) check_window(w);
    const gtb::FADMultiscaleResultCpp r =
        gtb::compute_fad_multiscale(image(input, ny, nx), std::vector<int>(windows.begin(), windows.end()));
    FadScalesOut out;
    out.maps.reserve(static_cast<std::size_t>(ny * nx) * r.single_scale_maps.size());
    for (const auto& m : r.single_scale_maps)
        for (i64 i = 0; i < m.size(); ++i) out.maps.push_back(m[i]);
    out.multiscale = pixels(r.multiscale_map);
    out.classes = pixels(r.multiscale_classes);
    for (const auto& s : r.stats) {
        for (double v : {double(s.scale), s.intact_pct, s.interior_pct, s.dominant_pct, s.transitional_pct, s.patchy_pct,
                         s.rare_pct, s.average_fad})
            out.stats.push_back(v);
    }
    return out;
}

rust::Vec<std::uint8_t> fad_density(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                    std::int32_t window, bool gtb_codes) {
    check_window(window);
    return pixels(gtb::compute_fad_density(image(input, ny, nx), window, gtb_codes));
}

rust::Vec<std::uint8_t> clustering_fac(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                       std::int32_t window, bool gtb_codes) {
    check_window(window);
    return pixels(gtb::compute_clustering_fac(image(input, ny, nx), window, gtb_codes));
}

rust::Vec<std::uint8_t> grayspatcon_fos(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                        std::int32_t metric, std::int32_t window) {
    check_window(window);
    check(metric == 1 || metric == 52, "the metric is 1 (the mean) or 52 (clustering)");
    return pixels(gtb::grayspatcon_fos(image(input, ny, nx), metric, window));
}

SpaOut spa(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx, std::int32_t variant) {
    check(variant == 2 || variant == 3 || variant == 5 || variant == 6, "SPA's variant is 2, 3, 5 or 6");
    const gtb::SPAResult r = gtb::compute_spa(image(input, ny, nx), variant);
    SpaOut out;
    out.map = pixels(r.map);
    for (i64 n : {r.nr_fg, r.nr_cores, r.nr_margin, r.nr_holes, r.nr_edge, r.nr_perf, r.nr_islet, r.nr_contiguous})
        out.counts.push_back(n);
    return out;
}

AccountingOut accounting(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                         std::int32_t connectivity, rust::Slice<const std::int64_t> thresholds, bool highlight_largest) {
    check(connectivity == 4 || connectivity == 8, "the connectivity is 4 or 8");
    const gtb::AccountingResult r = gtb::compute_accounting(
        image(input, ny, nx), connectivity, std::vector<i64>(thresholds.begin(), thresholds.end()), highlight_largest);
    AccountingOut out;
    out.thresholds = to_rust<std::int64_t>(r.thresholds);
    out.classes = pixels(r.classes);
    out.ids = pixels(r.ids);
    out.area = pixels(r.area);
    // object k's area at k - 1
    for (std::size_t k = 1; k < r.object_area.size(); ++k) out.object_area.push_back(r.object_area[k]);
    return out;
}

rust::Vec<std::uint8_t> contagion_pff(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                      std::int32_t window) {
    check_window(window);
    return pixels(gtb::compute_contagion_pff(image(input, ny, nx), window, true));
}

EntropyOut spatial_entropy(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx) {
    const gtb::SpatialEntropyResultCpp r = gtb::compute_spatial_entropy(image(input, ny, nx));
    EntropyOut out;
    out.index = r.entropy_index;
    out.histogram = to_rust<std::uint64_t>(r.histogram);
    return out;
}

rust::Vec<float> spatial_entropy_map(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                     std::int32_t window) {
    check(window >= 3, "the window's side is 3 cells or more");
    return pixels(gtb::compute_spatial_entropy_map(image(input, ny, nx), window));
}

rust::Vec<std::uint8_t> landscape_mosaic(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                         std::int32_t window, std::int32_t classes) {
    check_window(window);
    check(classes == 19 || classes == 103, "the landscape mosaic has 19 or 103 classes");
    return pixels(gtb::compute_landscape_mosaic(image(input, ny, nx), window, classes));
}

MosaicStatsOut landscape_mosaic_stats(rust::Slice<const std::uint8_t> lm103, std::int64_t ny, std::int64_t nx) {
    const gtb::LandscapeMosaicStats r = gtb::compute_landscape_mosaic_stats(image(lm103, ny, nx));
    MosaicStatsOut out;
    out.codes = to_rust<std::int32_t>(r.class_codes);
    out.counts = to_rust<std::uint64_t>(r.pixel_counts);
    out.percentages = to_rust<double>(r.percentages);
    return out;
}

rust::Vec<float> gray_convolution(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                  rust::Str metric, std::int32_t window, bool mask_missing) {
    check_window(window);
    return pixels(gtb::compute_gray_convolution(image(input, ny, nx), std::string(metric), window, mask_missing, 0));
}

ParcellationOut parcellation(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                             std::int32_t connectivity) {
    check(connectivity == 4 || connectivity == 8, "the connectivity is 4 or 8");
    const gtb::ParcellationResult r = gtb::compute_parcellation(image(input, ny, nx), connectivity, true);
    ParcellationOut out;
    for (const auto& c : r.class_stats) {
        for (double v : {double(c.class_id), double(c.count), double(c.area), c.aps, c.awaps_rel, c.awaps_data,
                         c.division, c.parc})
            out.classes.push_back(v);
    }
    const auto& g = r.global_stats;
    for (double v : {double(g.total_parcels), double(g.data_area), g.aps, g.awaps, g.division, g.parc})
        out.global.push_back(v);
    out.labels = pixels(r.parcel_labels);
    return out;
}

rust::Vec<std::uint32_t> contortion(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx,
                                    std::int64_t min_area) {
    return pixels(gtb::compute_contortion(image(input, ny, nx), min_area));
}

ChangeOut simple_change(rust::Slice<const std::uint8_t> a, rust::Slice<const std::uint8_t> b, std::int64_t ny,
                        std::int64_t nx) {
    const gtb::SimpleChangeResult r = gtb::compute_simple_change(image(a, ny, nx), image(b, ny, nx));
    ChangeOut out;
    out.map = pixels(r.change_map);
    for (i64 n : {r.stable_bg, r.gain, r.loss, r.stable_fg, r.missing, r.total_valid}) out.counts.push_back(n);
    return out;
}

ChangeOut morphological_change(rust::Slice<const std::uint8_t> a, rust::Slice<const std::uint8_t> b, std::int64_t ny,
                               std::int64_t nx) {
    const gtb::MorphologicalChangeResult r = gtb::compute_morphological_change(image(a, ny, nx), image(b, ny, nx));
    ChangeOut out;
    out.map = pixels(r.change_map);
    for (i64 n : {r.fg_area_t1, r.fg_area_t2, r.net_change, r.stable_bg, r.stable_fg, r.mcd_gain, r.mcd_loss,
                  r.spurious, r.missing})
        out.counts.push_back(n);
    out.values.push_back(r.elasticity);
    return out;
}

ChangeOut fos_change(rust::Slice<const std::uint8_t> a, rust::Slice<const std::uint8_t> b, std::int64_t ny,
                     std::int64_t nx) {
    const gtb::FOSChangeResult r = gtb::compute_fos_change(image(a, ny, nx), image(b, ny, nx));
    ChangeOut out;
    out.map = pixels(r.change_map);
    for (i64 n : {r.fg_gain, r.fg_loss, r.stable_bg, r.forcom, r.diag_same_class, r.diff_class}) out.counts.push_back(n);
    out.values.push_back(r.frag_decrease_pct);
    out.values.push_back(r.frag_increase_pct);
    for (const auto& row : r.matrix_6x6)
        for (i64 n : row) out.matrix.push_back(n);
    return out;
}

namespace {

// a slice, or null when it is empty
template <class T>
const T* or_null(rust::Slice<const T> s, std::size_t n, const char* what) {
    if (s.empty()) return nullptr;
    check(s.size() == n, what);
    return s.data();
}

rest::IntactnessParams intactness_params(double gamma, double beta, double z, std::int32_t radius, double dx,
                                         double dy) {
    check(radius >= 0, "the radius is 0 cells or more");
    check(z > 0, "z is above 0");
    return rest::IntactnessParams{gamma, beta, z, radius, dx, dy};
}

}  // namespace

rust::Vec<double> rest_footprint(rust::Slice<const double> hfi, rust::Slice<const double> reduction,
                                 rust::Slice<const double> restored, std::int64_t ny, std::int64_t nx) {
    const std::size_t n = static_cast<std::size_t>(ny * nx);
    check(hfi.size() == n, "the footprint differs in size from ny x nx");
    return to_rust<double>(rest::restored_footprint(hfi.data(), or_null(reduction, n, "the reduction's grid"),
                                                    or_null(restored, n, "the restored shares' grid"), ny, nx,
                                                    max_threads()));
}

rust::Vec<double> rest_intactness(rust::Slice<const double> hfi, std::int64_t ny, std::int64_t nx, double gamma,
                                  double beta, double z, std::int32_t radius, double dx, double dy) {
    check(static_cast<i64>(hfi.size()) == ny * nx, "the footprint differs in size from ny x nx");
    return to_rust<double>(
        rest::intactness(hfi.data(), ny, nx, intactness_params(gamma, beta, z, radius, dx, dy), max_threads()));
}

rust::Vec<double> rest_intactness_gradient(rust::Slice<const double> hfi, rust::Slice<const double> reduction,
                                           std::int64_t ny, std::int64_t nx, double gamma, double beta, double z,
                                           std::int32_t radius, double dx, double dy) {
    check(static_cast<i64>(hfi.size()) == ny * nx && hfi.size() == reduction.size(),
          "the footprint and the reduction differ in size from ny x nx");
    return to_rust<double>(rest::intactness_gradient(hfi.data(), reduction.data(), ny, nx,
                                                     intactness_params(gamma, beta, z, radius, dx, dy),
                                                     max_threads()));
}

VegetationOut rest_original_vegetation(rust::Slice<const double> natural, std::int32_t k,
                                       rust::Slice<const double> anthropic, std::int32_t m, std::int64_t n,
                                       rust::Slice<const double> past, rust::Slice<const double> ecoregion,
                                       rust::Slice<const std::uint8_t> domain, rust::Slice<const std::uint8_t> fill,
                                       double threshold, bool restore_current) {
    const std::size_t kn = static_cast<std::size_t>(k) * static_cast<std::size_t>(n);
    check(k > 0 && m > 0 && n >= 0, "natural and anthropic classes are needed");
    check(natural.size() == kn, "the natural shares are k x n");
    check(anthropic.size() == static_cast<std::size_t>(m) * static_cast<std::size_t>(n), "the anthropic shares are m x n");
    check(fill.size() == static_cast<std::size_t>(k), "a failsafe flag per natural class");
    const rest::Vegetation v = rest::original_vegetation(
        natural.data(), k, anthropic.data(), m, n, or_null(past, kn, "the past shares are k x n"),
        or_null(ecoregion, kn, "the ecoregions' shares are k x n"),
        or_null(domain, static_cast<std::size_t>(n), "a domain flag per pixel"), fill.data(), threshold,
        restore_current, max_threads());
    VegetationOut out;
    out.original = to_rust<double>(v.original);
    out.restore = to_rust<double>(v.restore);
    out.source = to_rust<std::uint8_t>(v.source);
    out.fill = to_rust<double>(v.fill);
    return out;
}

namespace {

void check_ranges(rust::Slice<const std::int64_t> range_ptr, rust::Slice<const std::int64_t> range_idx,
                  rust::Slice<const std::uint8_t> suitable, std::int64_t n, std::int32_t k, std::int64_t ns) {
    check(range_ptr.size() == static_cast<std::size_t>(ns + 1) && range_ptr[0] == 0 &&
              range_ptr[static_cast<std::size_t>(ns)] == static_cast<i64>(range_idx.size()),
          "the ranges are ns + 1 offsets into the pixels");
    for (i64 p : range_idx) check(p >= 0 && p < n, "a range's pixel is outside the map");
    check(suitable.size() == static_cast<std::size_t>(ns) * static_cast<std::size_t>(k), "suitability is ns x k");
}

}  // namespace

HabitatOut rest_habitat(rust::Slice<const double> current, rust::Slice<const double> original,
                        rust::Slice<const double> area, std::int64_t n, std::int32_t k,
                        rust::Slice<const std::int64_t> range_ptr, rust::Slice<const std::int64_t> range_idx,
                        rust::Slice<const std::uint8_t> suitable, std::int64_t ns) {
    const std::size_t kn = static_cast<std::size_t>(k) * static_cast<std::size_t>(n);
    check(current.size() == kn && original.size() == kn, "the shares are k x n");
    check(area.size() == static_cast<std::size_t>(n), "an area per pixel");
    check_ranges(range_ptr, range_idx, suitable, n, k, ns);
    std::vector<double> now(static_cast<std::size_t>(ns)), pot(static_cast<std::size_t>(ns));
    rest::habitat_areas(current.data(), original.data(), area.data(), n, k, range_ptr.data(), range_idx.data(),
                        suitable.data(), ns, now.data(), pot.data(), max_threads());
    HabitatOut out;
    out.now = to_rust<double>(now);
    out.potential = to_rust<double>(pot);
    return out;
}

GainOut rest_habitat_gain(rust::Slice<const double> restore, rust::Slice<const double> area, std::int64_t n,
                          std::int32_t k, rust::Slice<const std::int64_t> range_ptr,
                          rust::Slice<const std::int64_t> range_idx, rust::Slice<const std::uint8_t> suitable,
                          std::int64_t ns, rust::Slice<const std::uint8_t> domain) {
    check(restore.size() == static_cast<std::size_t>(k) * static_cast<std::size_t>(n), "the shares are k x n");
    check(area.size() == static_cast<std::size_t>(n), "an area per pixel");
    check_ranges(range_ptr, range_idx, suitable, n, k, ns);
    std::vector<i64> indptr, index;
    std::vector<double> gain;
    rest::habitat_gain(restore.data(), area.data(), n, k, range_ptr.data(), range_idx.data(), suitable.data(), ns,
                       or_null(domain, static_cast<std::size_t>(n), "a domain flag per pixel"), indptr, index, gain,
                       max_threads());
    GainOut out;
    out.indptr = to_rust<std::int64_t>(indptr);
    out.index = to_rust<std::int64_t>(index);
    out.gain = to_rust<double>(gain);
    return out;
}

namespace {

lsm::Options lsm_options(const LsmOptions& o) {
    lsm::Options opt;
    check(o.directions == 4 || o.directions == 8, "directions is 4 or 8");
    check(o.neighbourhood == 4 || o.neighbourhood == 8, "neighbourhood is 4 or 8");
    check(o.edge_depth >= 0, "edge_depth is 0 or more");
    opt.directions = o.directions;
    opt.count_boundary = o.count_boundary;
    opt.consider_boundary = o.consider_boundary;
    opt.edge_depth = o.edge_depth;
    opt.cell_center = o.cell_center;
    opt.classes_max = o.classes_max;
    opt.neighbourhood = o.neighbourhood;
    opt.ordered = o.ordered;
    opt.base = std::string(o.base);
    opt.fragstats = o.fragstats;
    opt.search_radius = o.search_radius;
    opt.connect_threshold = o.connect_threshold;
    opt.boundary_contrast = o.boundary_contrast;
    auto table = [](const rust::Vec<double>& v, std::vector<std::array<double, 3>>& out) {
        check(v.size() % 3 == 0, "weights come as (class a, class b, weight)");
        for (std::size_t i = 0; i + 2 < v.size(); i += 3) out.push_back({v[i], v[i + 1], v[i + 2]});
    };
    table(o.contrast, opt.contrast);
    table(o.similarity, opt.similarity);
    return opt;
}

std::vector<std::string> strings(rust::Slice<const rust::String> names) {
    std::vector<std::string> out;
    for (const auto& n : names) out.emplace_back(n);
    return out;
}

const std::uint8_t* na_mask(rust::Slice<const std::uint8_t> na, i64 n) {
    if (na.empty()) return nullptr;
    check(static_cast<i64>(na.size()) == n, "the NA mask differs in size from the map");
    return na.data();
}

}  // namespace

LsmTable lsm_table(rust::Slice<const std::int32_t> values, rust::Slice<const std::uint8_t> na, std::int64_t ny,
                   std::int64_t nx, double resx, double resy, const LsmOptions& options,
                   rust::Slice<const std::int32_t> levels, rust::Slice<const rust::String> names, bool labels) {
    check(ny >= 0 && nx >= 0 && static_cast<i64>(values.size()) == ny * nx, "the map differs in size from ny x nx");
    check(levels.size() == names.size(), "a level per metric");
    std::vector<lsm::MetricSpec> specs;
    for (std::size_t i = 0; i < levels.size(); ++i) {
        check(levels[i] >= 0 && levels[i] <= 2, "the level is 0 (patch), 1 (class) or 2 (landscape)");
        specs.push_back({static_cast<lsm::Level>(levels[i]), std::string(names[i])});
    }
    lsm::Landscape l(values.data(), na_mask(na, ny * nx), nx, ny, resx, resy, lsm_options(options));
    lsm::LongTable t;
    lsm::append_long_table(l, specs, 0, t);
    LsmTable out;
    out.class_values = to_rust<std::int32_t>(l.class_values());
    out.metric = to_rust<std::int16_t>(t.metric);
    out.cls = to_rust<std::int32_t>(t.cls);
    out.cls_na = to_rust<std::uint8_t>(t.cls_na);
    out.id = to_rust<std::int32_t>(t.id);
    out.id_na = to_rust<std::uint8_t>(t.id_na);
    out.value = to_rust<double>(t.value);
    if (labels) {
        out.labels = to_rust<std::int32_t>(l.labels());
        out.patch_class = to_rust<std::int32_t>(l.patches().cls);
    }
    return out;
}

rust::Vec<double> lsm_window(rust::Slice<const std::int32_t> values, rust::Slice<const std::uint8_t> na, std::int64_t ny,
                             std::int64_t nx, double resx, double resy, rust::Slice<const std::uint8_t> window,
                             std::int64_t wy, std::int64_t wx, rust::Slice<const rust::String> names,
                             const LsmOptions& options) {
    check(ny >= 0 && nx >= 0 && static_cast<i64>(values.size()) == ny * nx, "the map differs in size from ny x nx");
    check(wy > 0 && wx > 0 && wy % 2 == 1 && wx % 2 == 1 && static_cast<i64>(window.size()) == wy * wx,
          "the window has odd sides");
    const lsm::RasterView r{values.data(), na_mask(na, ny * nx), nx, ny, resx, resy};
    return to_rust<double>(lsm::window_metrics(r, window.data(), wx, wy, strings(names), lsm_options(options)));
}

DistanceOut euclidean_distance(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx) {
    const gtb::EuclideanDistanceResultCpp r = gtb::compute_euclidean_distance(image(input, ny, nx));
    DistanceOut out;
    out.distance = to_rust<float>(r.signed_distance);
    out.bytes = pixels(r.byte_map);
    const auto& s = r.stats;
    for (double v : {double(s.fg_pixel_count), s.mean_fg_distance, s.max_fg_distance, double(s.bg_pixel_count),
                     s.mean_bg_distance, s.max_bg_distance})
        out.stats.push_back(v);
    return out;
}

rust::Vec<double> allow_matrix(rust::Slice<const double> x, rust::Slice<const std::int64_t> hist,
                               rust::Slice<const double> classes, rust::Slice<const double> cd,
                               rust::Slice<const std::int64_t> rules) {
    const int ncode = static_cast<int>(classes.size());
    check(hist.size() == x.size(), "one history value per cell");
    check(cd.size() == classes.size(), "one demand change per class");
    check(rules.size() == classes.size() * classes.size(), "the rules must be classes x classes");
    return to_rust<double>(sim::allow_matrix(x.data(), hist.data(), static_cast<i64>(x.size()), classes.data(), ncode,
                                             cd.data(), rules.data()));
}

rust::Vec<double> auto_convert(rust::Slice<const double> x, rust::Slice<const double> prob,
                               rust::Slice<const double> classes, rust::Slice<const std::int64_t> mask) {
    check(prob.size() == x.size() * classes.size(), "prob must be cells x classes");
    check(mask.size() == x.size(), "one mask value per cell");
    return to_rust<double>(sim::auto_convert(x.data(), prob.data(), static_cast<i64>(x.size()), classes.data(),
                                             static_cast<int>(classes.size()), mask.data()));
}

rust::Vec<double> neighbourhood_allow(rust::Slice<const double> landscape, std::int64_t ny, std::int64_t nx,
                                      rust::Slice<const double> classes, rust::Slice<const NbRule> rules) {
    check(static_cast<i64>(landscape.size()) == ny * nx, "the map differs in size from ny x nx");
    std::vector<sim::NeighbourhoodRule> r;
    for (const auto& x : rules) {
        sim::NeighbourhoodRule q;
        q.cls = x.cls;
        q.threshold = x.threshold;
        q.wy = x.wy;
        q.wx = x.wx;
        q.weights.assign(x.weights.begin(), x.weights.end());
        r.push_back(std::move(q));
    }
    return to_rust<double>(sim::neighbourhood_allow(landscape.data(), ny, nx, classes.data(),
                                                    static_cast<int>(classes.size()), r));
}

CluesOut clues(rust::Slice<const double> tprob, rust::Slice<const double> landuse, rust::Slice<const double> demand,
               rust::Slice<const double> codes, double jitter, double scale, double max_iter, double max_diff,
               double mean_diff, nlm::RRng& rng) {
    const int ncode = static_cast<int>(codes.size());
    check(tprob.size() == landuse.size() * codes.size(), "tprob must be classes x cells");
    check(demand.size() == codes.size(), "one demand per class");
    const auto r = sim::clues_allocate(tprob.data(), landuse.data(), static_cast<i64>(landuse.size()), demand.data(),
                                       codes.data(), ncode, jitter, scale, max_iter, max_diff, mean_diff, rng);
    return CluesOut{to_rust<double>(r.allocation), r.iterations, r.converged, r.max_diff, r.mean_diff};
}

OrderedOut ordered(rust::Slice<const double> tprob, rust::Slice<const double> x, rust::Slice<const double> demand,
                   rust::Slice<const double> classes, rust::Slice<const double> order, bool stochastic,
                   nlm::RRng& rng) {
    check(tprob.size() == x.size() * classes.size(), "tprob must be cells x classes");
    check(demand.size() == classes.size() && order.size() == classes.size(), "one demand and order per class");
    const auto r = sim::ordered_allocate(tprob.data(), x.data(), static_cast<i64>(x.size()), demand.data(),
                                         classes.data(), static_cast<int>(classes.size()), order.data(), stochastic,
                                         rng);
    return OrderedOut{to_rust<double>(r.allocation), r.iterations, r.converged, r.short_cells};
}

rust::Vec<double> approx_extrap(rust::Slice<const double> x, rust::Slice<const double> y,
                                rust::Slice<const double> xout) {
    return to_rust<double>(sim::approx_extrap(to_std(x), to_std(y), to_std(xout)));
}

rust::Vec<double> round_sum(rust::Slice<const double> x, std::int64_t nrow, std::int64_t ncol, std::int64_t ncell) {
    return to_rust<double>(sim::round_sum(to_std(x), nrow, ncol, ncell));
}

rust::Vec<double> markov_from_counts(rust::Slice<const double> counts, std::int32_t k, rust::Vec<std::int32_t>& kept) {
    std::vector<int> kv;
    auto p = sim::markov_from_counts(to_std(counts), k, kv);
    kept.clear();
    for (int v : kv) kept.push_back(v);
    return to_rust<double>(p);
}

rust::Vec<double> markov_root(rust::Slice<const double> p, std::int32_t k, std::int32_t n) {
    return to_rust<double>(sim::markov_root(to_std(p), k, n));
}

rust::Vec<double> markov_project(rust::Slice<const double> areas, rust::Slice<const double> p, std::int32_t k,
                                 std::int32_t steps) {
    return to_rust<double>(sim::markov_project(to_std(areas), to_std(p), k, steps));
}

rust::Vec<std::int32_t> woe_buckets(rust::Slice<const double> var, bool categorical, rust::Slice<const double> classes,
                                    double start, double inc, std::int64_t n_buckets) {
    return to_rust<std::int32_t>(sim::woe_buckets(view(var), categorical, to_std(classes), start, inc, n_buckets));
}

rust::Vec<std::int64_t> woe_counts(rust::Slice<const double> t0, rust::Slice<const double> t1,
                                   rust::Slice<const std::int32_t> buckets, std::int64_t n_buckets, double from,
                                   rust::Slice<const double> final_classes) {
    check(t0.size() == t1.size() && buckets.size() == t0.size(), "the maps and buckets differ in size");
    return to_rust<std::int64_t>(sim::woe_counts(view(t0), view(t1), to_std(buckets), n_buckets, from,
                                                 to_std(final_classes)));
}

rust::Vec<double> woe_probability(rust::Slice<const double> landscape, double from,
                                  rust::Slice<const std::int32_t> buckets, rust::Slice<const double> weights,
                                  rust::Slice<const std::int64_t> offsets) {
    // buckets: one map per variable, one after the other; weights: the variables' tables, from offsets[v]
    const std::size_t n = landscape.size();
    check(offsets.size() >= 1, "offsets must start at 0 and end at the weights' length");
    const std::size_t nvar = offsets.size() - 1;
    check(buckets.size() == n * nvar, "one bucket map per variable");
    std::vector<const std::int32_t*> b;
    std::vector<std::vector<double>> w;
    for (std::size_t v = 0; v < nvar; ++v) {
        b.push_back(buckets.data() + v * n);
        w.emplace_back(weights.data() + offsets[v], weights.data() + offsets[v + 1]);
    }
    return to_rust<double>(sim::woe_probability(view(landscape), from, b, w, static_cast<i64>(n)));
}

WoeWeights woe_weights(double nb, double b, double n, double a, bool fix) {
    const auto w = sim::woe_weights(nb, b, n, a, fix);
    return WoeWeights{w.plus, w.contrast, w.significant};
}

rust::Vec<std::int32_t> woe_generalise(rust::Slice<const std::int64_t> possible,
                                       rust::Slice<const std::int64_t> executed, double min_delta, double max_delta,
                                       double tolerance, bool fix) {
    return to_rust<std::int32_t>(
        sim::woe_generalise(to_std(possible), to_std(executed), min_delta, max_delta, tolerance, fix));
}

void woe_normalise(rust::Slice<double> sums, std::int32_t k, std::int64_t n) {
    check(sums.size() == static_cast<std::size_t>(k) * static_cast<std::size_t>(n), "sums must be k x n");
    sim::woe_normalise(sums.data(), k, n);
}

GlmFit glm_logit(rust::Slice<const double> x, rust::Slice<const double> y, std::int32_t n, std::int32_t p,
                 std::int32_t maxit, double epsilon) {
    check(x.size() == static_cast<std::size_t>(n) * static_cast<std::size_t>(p) && y.size() == static_cast<std::size_t>(n),
          "x must be n x p (column-major) and y of n");
    const auto f = sim::glm_logit(to_std(x), to_std(y), n, p, maxit, epsilon);
    return GlmFit{to_rust<double>(f.coefficients), to_rust<std::int32_t>(f.pivot), f.rank, f.iterations, f.converged,
                  f.boundary, f.deviance, f.null_deviance};
}

rust::Vec<double> glm_predict(rust::Slice<const double> x, std::int64_t n, std::int32_t p, const GlmFit& fit) {
    check(x.size() == static_cast<std::size_t>(n) * static_cast<std::size_t>(p), "x must be n x p (column-major)");
    sim::GlmFit f;
    f.coefficients.assign(fit.coefficients.begin(), fit.coefficients.end());
    f.pivot.assign(fit.pivot.begin(), fit.pivot.end());
    f.rank = fit.rank;
    return to_rust<double>(sim::glm_logit_predict(x.data(), n, p, f));
}

PatchOut allocate_patches(std::int64_t ny, std::int64_t nx, rust::Slice<const std::int32_t> landscape,
                          rust::Slice<const PatchSpec> specs, double prune_factor, std::int32_t win_rows,
                          std::int32_t win_cols, std::uint64_t seed) {
    check(static_cast<i64>(landscape.size()) == ny * nx, "the map differs in size from ny x nx");
    std::vector<sim::PatchTransition> ts;
    for (const auto& s : specs) {
        check(static_cast<i64>(s.probability.size()) == ny * nx, "a probability map differs in size from the landscape");
        sim::PatchTransition t;
        t.from = s.from;
        t.to = s.to;
        t.probability.assign(s.probability.begin(), s.probability.end());
        t.expand = s.expand;
        t.patch = s.patch;
        t.e_mean = s.e_mean;
        t.e_var = s.e_var;
        t.e_iso = s.e_iso;
        t.p_mean = s.p_mean;
        t.p_var = s.p_var;
        t.p_iso = s.p_iso;
        ts.push_back(std::move(t));
    }
    std::vector<int> land(landscape.begin(), landscape.end());
    const auto r = sim::allocate_patches(ny, nx, land.data(), ts, prune_factor, win_rows, win_cols, seed);
    return PatchOut{to_rust<std::int32_t>(r.landscape), to_rust<std::int64_t>(r.expanded),
                    to_rust<std::int64_t>(r.patched), to_rust<std::int64_t>(r.patches)};
}

CarsOut cars(std::int64_t ny, std::int64_t nx, std::int32_t k, rust::Slice<const std::int32_t> landscape,
             rust::Slice<const std::uint8_t> fixed, rust::Slice<const std::int32_t> region, std::int32_t nreg,
             rust::Slice<const double> probability, rust::Slice<const double> weights,
             rust::Slice<const double> seeds, rust::Slice<const std::uint8_t> allowed,
             rust::Slice<const std::int64_t> demand, std::int32_t window, double decay, double step,
             std::int64_t max_iter, std::uint64_t seed) {
    // fixed and region: empty for none
    const i64 n = ny * nx;
    check(static_cast<i64>(landscape.size()) == n, "the map differs in size from ny x nx");
    check(fixed.empty() || static_cast<i64>(fixed.size()) == n, "the fixed cells differ in size from the landscape");
    check(region.empty() || static_cast<i64>(region.size()) == n, "the regions differ in size from the landscape");
    check(static_cast<i64>(probability.size()) == i64{k} * n, "the probabilities must be classes x cells");
    check(static_cast<i64>(weights.size()) == k && static_cast<i64>(seeds.size()) == k, "one weight and seed per class");
    check(static_cast<i64>(allowed.size()) == i64{k} * k, "the conversion matrix must be classes x classes");
    check(static_cast<i64>(demand.size()) == i64{nreg} * k, "the demand must be regions x classes");
    sim::CarsSettings s;
    s.window = window;
    s.decay = decay;
    s.step = step;
    s.max_iter = max_iter;
    s.seed = seed;
    const auto r = sim::cars_run(ny, nx, k, landscape.data(), fixed.empty() ? nullptr : fixed.data(),
                                 region.empty() ? nullptr : region.data(), nreg, probability.data(), weights.data(),
                                 seeds.data(), allowed.data(), demand.data(), s);
    const std::size_t m = static_cast<std::size_t>(nreg) * static_cast<std::size_t>(k);
    std::vector<std::int64_t> last(r.trace.end() - static_cast<std::ptrdiff_t>(m), r.trace.end());
    return CarsOut{to_rust<std::int32_t>(r.landscape), r.iterations, r.converged, r.decays,
                   to_rust<std::int64_t>(last)};
}

SimilarityOut similarity(rust::Slice<const std::int32_t> a, rust::Slice<const std::int32_t> b, std::int64_t ny,
                         std::int64_t nx, std::int32_t window, bool exponential) {
    check(static_cast<i64>(a.size()) == ny * nx && b.size() == a.size(), "the maps differ in size from ny x nx");
    check(window >= 1 && window % 2 == 1, "the window must be odd");
    const auto r = sim::reciprocal_similarity(ny, nx, a.data(), b.data(), window, exponential);
    return SimilarityOut{to_rust<float>(r.first), to_rust<float>(r.second), r.first_mean, r.second_mean};
}

ObservedPatches observed_patches(rust::Slice<const double> a, rust::Slice<const double> b, std::int64_t ny,
                                 std::int64_t nx, double from, double to) {
    check(static_cast<i64>(a.size()) == ny * nx && b.size() == a.size(), "the maps differ in size from ny x nx");
    const auto r = sim::observed_patches(a.data(), b.data(), ny, nx, from, to);
    return ObservedPatches{r.cells, r.expansion, r.expansion_patches, r.expansion_mean, r.expansion_variance,
                           r.new_patches, r.new_mean, r.new_variance};
}

}  // namespace tfx
