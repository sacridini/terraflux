// SPDX-License-Identifier: GPL-3.0-or-later
//
// The shim between Terraflux (Rust, cxx) and landschaft's C++ kernels: Rust
// slices and vectors in, landschaft's types and calls, Rust vectors out.
// Exceptions of the kernels reach Rust as errors (cxx's Result).

#include "terraflux-ls-sys/shim/shim.h"

#include <algorithm>
#include <array>
#include <cmath>
#include <limits>
#include <stdexcept>
#include <string>

#ifdef _OPENMP
#include <omp.h>
#endif
#include <mutex>
#include <vector>

#include "circuit/circuitscape.hpp"
#include "circuit/grid.hpp"
#include "circuit/omniscape.hpp"
#include "bio/cluster.hpp"
#include "bio/conditions.hpp"
#include "bio/indices.hpp"
#include "bio/perlmath.hpp"
#include "bio/phylocom.hpp"
#include "bio/randomise.hpp"
#include "cost/randomwalk.hpp"
#include "cost/rsp.hpp"
#include "lsm/extended.hpp"
#include "cost/shortest.hpp"
#include "surf/surface.hpp"
#include "sim/clumondo.hpp"
#include "sim/futures.hpp"
#include "cost/transition.hpp"
#include "gtb/accounting.hpp"
#include "gtb/change.hpp"
#include "gtb/contagion.hpp"
#include "gtb/distance.hpp"
#include "gtb/entropy.hpp"
#include "gtb/fad.hpp"
#include "gtb/fos.hpp"
#include "gtb/influence.hpp"
#include "gtb/isochrone.hpp"
#include "gtb/primitives.hpp"
#include "gtb/restoration.hpp"
#include "gtb/grayspatcon.hpp"
#include "gtb/morphology.hpp"
#include "gtb/mosaic.hpp"
#include "gtb/mspa.hpp"
#include "gtb/spa.hpp"
#include "lsm/landscape.hpp"
#include "opt/actions.hpp"
#include "opt/cover.hpp"
#include "opt/eval.hpp"
#include "opt/features.hpp"
#include "opt/marxan.hpp"
#include "opt/milp.hpp"
#include "opt/proxirr.hpp"
#include "opt/wide.hpp"
#include "opt/zones.hpp"
#include "opt/zones_wide.hpp"
#include "io/exact.hpp"
#include "io/io.hpp"
#include "prio/prio.hpp"
#include "rest/habitat.hpp"
#include "rest/intactness.hpp"
#include "lsm/rmath.hpp"
#include "nlm/models.hpp"
#include <Eigen/SparseCore>
#include <Eigen/SparseLU>

#include "conn/cost.hpp"
#include "sdm/extrapolation.hpp"
#include "sdm/maxnet.hpp"
#include "sdm/rstats.hpp"
#include "sdm/thin.hpp"
#include "sdm/clean.hpp"
#include "sdm/geocircle/geocircle.hpp"
#include "sdm/kmeans.hpp"
#include "sdm/collinear.hpp"
#include "sdm/variogram.hpp"
#include "cost/geodesic.hpp"
#include "conn/graph.hpp"
#include "conn/nodes.hpp"
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
#include "sim/validation.hpp"
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

rust::Vec<std::int64_t> rrng_sample_int(nlm::RRng& rng, std::int64_t n, std::int64_t size, bool replace,
                                        rust::Slice<const double> prob) {
    check(n >= 0 && size >= 0 && (replace || size <= n), "cannot take a sample larger than the population");
    check(prob.empty() || prob.size() == static_cast<std::size_t>(n), "a probability per element");
    const std::vector<double> p = to_std(prob);
    return to_rust<std::int64_t>(rng.sample_int(n, size, replace, prob.empty() ? nullptr : &p));
}

rust::Vec<std::int32_t> sdm_kmeans(nlm::RRng& rng, rust::Slice<const double> x, std::int64_t m, std::int64_t p,
                                   std::int32_t k, std::int32_t iter_max, std::int32_t nstart, std::int32_t algorithm) {
    check(m > 0 && p > 0 && x.size() == static_cast<std::size_t>(m * p), "expected an m x p table");
    check(algorithm >= 1 && algorithm <= 3, "the algorithm is 1 Hartigan-Wong, 2 Lloyd or 3 MacQueen");
    std::vector<double> col(static_cast<std::size_t>(m * p));  // column-major, as R
    for (std::int64_t i = 0; i < m; ++i)
        for (std::int64_t j = 0; j < p; ++j) col[static_cast<std::size_t>(j * m + i)] = x[static_cast<std::size_t>(i * p + j)];
    const sdm::KmeansResult r =
        sdm::kmeans(col.data(), m, p, k, iter_max, nstart, static_cast<sdm::KmeansAlgorithm>(algorithm), rng);
    rust::Vec<std::int32_t> out;
    for (const int c : r.cluster) out.push_back(c);
    return out;
}

rust::Vec<double> sdm_vif(rust::Slice<const double> table, std::int64_t n, std::int64_t m,
                          rust::Slice<const std::int64_t> which) {
    check(n > 0 && m > 0 && table.size() == static_cast<std::size_t>(n * m), "expected an n x m table");
    for (const std::int64_t w : which) check(w >= 0 && w < m, "column out of range");
    std::vector<double> col(static_cast<std::size_t>(n * m));  // column-major, as R
    for (std::int64_t i = 0; i < n; ++i)
        for (std::int64_t j = 0; j < m; ++j) col[static_cast<std::size_t>(j * n + i)] = table[static_cast<std::size_t>(i * m + j)];
    return to_rust<double>(sdm::vif(col.data(), n, m, to_std(which)));
}

rust::Vec<double> sdm_sample_variogram(rust::Slice<const double> x, rust::Slice<const double> y,
                                       rust::Slice<const double> z, rust::Slice<const double> bounds, bool lonlat) {
    check(x.size() == y.size() && x.size() == z.size(), "lengths differ");
    const sdm::SampleVariogram v =
        sdm::sample_variogram(x.data(), y.data(), z.data(), static_cast<std::int64_t>(x.size()), to_std(bounds), lonlat);
    rust::Vec<double> out;
    for (const auto* part : {&v.np, &v.dist, &v.gamma})
        for (const double d : *part) out.push_back(d);
    return out;
}

rust::Vec<double> sdm_fit_variogram(rust::Str model, double kappa, rust::Slice<const double> dist,
                                    rust::Slice<const double> gamma, rust::Slice<const double> np, double nugget,
                                    double psill, double range) {
    check(dist.size() == gamma.size() && dist.size() == np.size(), "lengths differ");
    const std::string m(model);
    const sdm::VariogramFit f = sdm::fit_variogram(m.c_str(), kappa, dist.data(), gamma.data(), np.data(),
                                                   static_cast<std::int64_t>(dist.size()), nugget, psill, range);
    rust::Vec<double> out;
    if (f.ok)
        for (const double v : {f.nugget, f.psill, f.range, f.sserr}) out.push_back(v);
    return out;
}

double sdm_r_sd(rust::Slice<const double> x) {
    const auto n = static_cast<std::int64_t>(x.size());
    return lsm::r_sd(x.data(), n, lsm::r_mean(x.data(), n));
}

ThinOut sdm_thin_spthin(rust::Slice<const double> lon, rust::Slice<const double> lat, double max_km, std::int32_t reps,
                        std::int32_t seed) {
    check(lon.size() == lat.size(), "lon and lat differ in length");
    check(reps >= 1, "reps must be >= 1");
    sdm::ThinResult r = sdm::thin_spthin(lon.data(), lat.data(), static_cast<std::int64_t>(lon.size()), max_km, reps, seed);
    ThinOut out;
    out.keep = to_rust<std::uint8_t>(r.keep);
    out.kept = to_rust<std::int64_t>(r.kept);
    return out;
}

namespace {

using ExactGridT = exactextract::Grid<exactextract::bounded_extent>;

// lsio's grid_of and feature_box (io/exact.cpp)
ExactGridT exact_grid(rust::Slice<const double> g, std::int64_t ny, std::int64_t nx) {
    check(g.size() == 6, "a grid of 6");
    ExactGridT grid({g[0], g[1], g[2], g[3]}, g[4], g[5]);
    check(static_cast<std::int64_t>(grid.rows()) == ny && static_cast<std::int64_t>(grid.cols()) == nx,
          "the grid's extent and resolution do not give its rows and columns");
    return grid;
}

std::vector<exactextract::Polygon> exact_feature(rust::Slice<const double> xy, rust::Slice<const std::int64_t> ring_offsets,
                                                 rust::Slice<const std::int64_t> polygon_offsets) {
    std::vector<exactextract::Polygon> feature;
    for (std::size_t p = 0; p + 1 < polygon_offsets.size(); ++p) {
        exactextract::Polygon poly;
        for (std::int64_t r = polygon_offsets[p]; r < polygon_offsets[p + 1]; ++r) {
            exactextract::Ring ring;
            for (std::int64_t k = ring_offsets[r]; k < ring_offsets[r + 1]; ++k) ring.push_back({xy[2 * k], xy[2 * k + 1]});
            poly.rings.push_back(std::move(ring));
        }
        feature.push_back(std::move(poly));
    }
    return feature;
}

std::vector<ExactGridT> exact_subgrids(const ExactGridT& grid, const std::vector<exactextract::Polygon>& feature,
                                       std::int64_t max_cells) {
    exactextract::Box b = exactextract::Box::make_empty();
    bool first = true;
    for (const auto& p : feature)
        for (const auto& ring : p.rings) {
            if (ring.empty()) continue;
            const exactextract::Box r = exactextract::ring_box(ring);
            b = first ? r : exactextract::Box{std::min(b.xmin, r.xmin), std::min(b.ymin, r.ymin), std::max(b.xmax, r.xmax),
                                              std::max(b.ymax, r.ymax)};
            first = false;
        }
    if (feature.empty() || !b.intersects(grid.extent())) return {};
    return exactextract::subdivide(grid.crop(b), static_cast<std::size_t>(max_cells));
}

}  // namespace

std::int64_t io_exact_piece_count(rust::Slice<const double> grid, std::int64_t ny, std::int64_t nx,
                                  rust::Slice<const double> xy, rust::Slice<const std::int64_t> ring_offsets,
                                  rust::Slice<const std::int64_t> polygon_offsets, std::int64_t max_cells) {
    const ExactGridT g = exact_grid(grid, ny, nx);
    return static_cast<std::int64_t>(exact_subgrids(g, exact_feature(xy, ring_offsets, polygon_offsets), max_cells).size());
}

ExactPiece io_exact_piece(rust::Slice<const double> grid, std::int64_t ny, std::int64_t nx, rust::Slice<const double> xy,
                          rust::Slice<const std::int64_t> ring_offsets, rust::Slice<const std::int64_t> polygon_offsets,
                          std::int64_t max_cells, std::int64_t k) {
    const ExactGridT g = exact_grid(grid, ny, nx);
    const auto feature = exact_feature(xy, ring_offsets, polygon_offsets);
    const auto subgrids = exact_subgrids(g, feature, max_cells);
    check(k >= 0 && k < static_cast<std::int64_t>(subgrids.size()), "no such piece");
    ExactPiece out{};
    out.empty = true;
    const exactextract::Coverage cov = exactextract::raster_cell_intersection(subgrids[static_cast<std::size_t>(k)], feature);
    if (cov.grid.empty()) return out;
    // as lsio::exact_sums: the window of the grid under the coverage, and the coverage's offset in it
    const ExactGridT window = g.crop(cov.grid.extent());
    if (window.empty()) return out;
    check(window.dx() == cov.grid.dx() && window.dy() == cov.grid.dy(),
          "coverage grid at another resolution than the raster");
    if (!(window == cov.grid)) {
        out.x_off = static_cast<std::int64_t>(std::round((cov.grid.xmin() - window.xmin()) / cov.grid.dx()));
        out.y_off = static_cast<std::int64_t>(std::round((window.ymax() - cov.grid.ymax()) / cov.grid.dy()));
    }
    out.empty = false;
    out.i0 = static_cast<std::int64_t>(window.row_offset(g));
    out.j0 = static_cast<std::int64_t>(window.col_offset(g));
    out.wrows = static_cast<std::int64_t>(window.rows());
    out.wcols = static_cast<std::int64_t>(window.cols());
    out.rows = static_cast<std::int64_t>(cov.fraction.rows());
    out.cols = static_cast<std::int64_t>(cov.fraction.cols());
    out.fraction.reserve(static_cast<std::size_t>(out.rows * out.cols));
    for (std::int64_t i = 0; i < out.rows; ++i)
        for (std::int64_t j = 0; j < out.cols; ++j)
            out.fraction.push_back(cov.fraction(static_cast<std::size_t>(i), static_cast<std::size_t>(j)));
    return out;
}

rust::Vec<double> sim_tmc_table(std::int32_t n, rust::Slice<const double> x, rust::Slice<const double> x1,
                                rust::Slice<const double> y1, rust::Slice<const double> w) {
    check(n >= 2 && n <= 64, "the comparison takes 2 to 64 classes");
    const std::int64_t blocks = static_cast<std::int64_t>(w.size());
    const std::size_t cells = static_cast<std::size_t>(n) * static_cast<std::size_t>(blocks);
    check(x.size() == cells && x1.size() == cells && y1.size() == cells, "the counts and weights differ in size");
    return to_rust<double>(sim::tmc_table(n, blocks, x.data(), x1.data(), y1.data(), w.data()));
}

CirclesOut sdm_circles(rust::Slice<const double> lon, rust::Slice<const double> lat, double d, rust::Str kind) {
    check(lon.size() == lat.size(), "lon and lat differ in length");
    const std::string k(kind);
    check(k == "geosphere" || k == "terra" || k == "planar", "kind must be geosphere, terra or planar");
    CirclesOut out;
    for (std::size_t i = 0; i < lon.size(); ++i) {
        sdm::GeoParts p;
        if (k == "geosphere") {
            p = sdm::geod_polys_fixed(lon[i], lat[i], d);
        } else if (k == "terra") {
            p = sdm::terra_point_buffer(lon[i], lat[i], d);
        } else {
            p = {sdm::geos_circle(lon[i], lat[i], d)};
        }
        out.parts.push_back(static_cast<std::int64_t>(p.size()));
        for (const sdm::GeoRing& r : p) {
            out.vertices.push_back(static_cast<std::int64_t>(r.size() / 2));
            for (const double v : r) out.xy.push_back(v);
        }
    }
    return out;
}

rust::Vec<double> sdm_outlier_statistic(rust::Slice<const double> lon, rust::Slice<const double> lat, bool use_min) {
    check(lon.size() == lat.size(), "lon and lat differ in length");
    return to_rust<double>(sdm::outlier_statistic(lon.data(), lat.data(), static_cast<std::int64_t>(lon.size()), use_min));
}

rust::Vec<double> sdm_geod_direct(rust::Slice<const double> lon, rust::Slice<const double> lat,
                                  rust::Slice<const double> azi, rust::Slice<const double> s) {
    const std::size_t n = lon.size();
    check(lat.size() == n && azi.size() == n && s.size() == n, "coordinates of different lengths");
    rust::Vec<double> out;
    out.reserve(2 * n);
    for (std::size_t i = 0; i < n; ++i) {
        double x = 0.0, y = 0.0;
        sdm::terra_direct(lon[i], lat[i], azi[i], s[i], x, y);
        out.push_back(x);
        out.push_back(y);
    }
    return out;
}

rust::Vec<double> sdm_outlier_statistic_weighted(rust::Slice<const double> lon, rust::Slice<const double> lat,
                                                 rust::Slice<const double> count, double within, bool use_min) {
    check(lon.size() == lat.size() && lon.size() == count.size(), "lengths differ");
    return to_rust<double>(sdm::outlier_statistic_weighted(lon.data(), lat.data(), count.data(),
                                                           static_cast<std::int64_t>(lon.size()), within, use_min));
}

rust::Vec<double> cost_geodesic(rust::Slice<const double> lon1, rust::Slice<const double> lat1,
                                rust::Slice<const double> lon2, rust::Slice<const double> lat2) {
    const std::size_t n = lon1.size();
    check(lat1.size() == n && lon2.size() == n && lat2.size() == n, "coordinates of different lengths");
    const cost::Geodesic geod;
    rust::Vec<double> out;
    out.reserve(n);
    for (std::size_t k = 0; k < n; ++k) out.push_back(geod.distance(lon1[k], lat1[k], lon2[k], lat2[k]));
    return out;
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

namespace {

conn::Scenario conn_scenario(std::int32_t kind, double threshold, double probability, double pij_min, bool conefor) {
    check(kind >= 0 && kind <= 2, "the kind is 0 distance, 1 probability or 2 link");
    conn::Scenario s;
    s.kind = kind == 0 ? conn::Kind::Distance : kind == 1 ? conn::Kind::Probability : conn::Kind::Link;
    s.threshold = threshold;
    s.probability = probability;
    s.pij_min = pij_min;
    s.conefor = conefor;
    return s;
}

conn::Pairs conn_pairs(rust::Slice<const std::int32_t> pi, rust::Slice<const std::int32_t> pj,
                       rust::Slice<const double> value, std::int32_t n) {
    check(pi.size() == pj.size() && pi.size() == value.size(), "pi, pj and value differ in length");
    for (std::size_t k = 0; k < pi.size(); ++k)
        check(pi[k] >= 0 && pi[k] < n && pj[k] >= 0 && pj[k] < n && pi[k] != pj[k], "a pair joins no two nodes");
    conn::Pairs p;
    p.i = to_std(pi);
    p.j = to_std(pj);
    p.value = to_std(value);
    return p;
}

ConnGains gains_out(conn::Gains&& g) {
    ConnGains out;
    out.num = to_rust<double>(g.num);
    out.h = to_rust<double>(g.h);
    return out;
}

}  // namespace

ConnAnalysis conn_analyse(rust::Slice<const std::int32_t> pi, rust::Slice<const std::int32_t> pj,
                          rust::Slice<const double> value, rust::Slice<const double> area, std::int32_t kind,
                          double threshold, double probability, double pij_min, double expand_limit, bool conefor,
                          bool links, bool probabilities, bool importance, bool edges, bool betweenness) {
    const auto n = static_cast<std::int32_t>(area.size());
    const conn::Pairs pairs = conn_pairs(pi, pj, value, n);
    const conn::Scenario s = conn_scenario(kind, threshold, probability, pij_min, conefor);
    const std::vector<double> a = to_std(area);
    ConnAnalysis out{};
    if (links) {
        const conn::Csr g = conn::make_link_graph(n, pairs, s);
        conn::LinkResult r = conn::analyse_links(g, a.data(), importance, edges, betweenness);
        out.nl = r.nl;
        out.nc = r.nc;
        out.h = r.h;
        out.iic_reach = r.iic_reach;
        out.iic_direct = r.iic_direct;
        out.unreach_aa = r.unreach_aa;
        out.comp_sq = r.comp_sq;
        out.component = to_rust<std::int32_t>(r.component);
        out.iic_row = to_rust<double>(r.iic_row);
        out.unreach_a = to_rust<double>(r.unreach_a);
        out.h_row = to_rust<double>(r.h_row);
        out.iic_conn = to_rust<double>(r.iic_conn);
        out.h_conn = to_rust<double>(r.h_conn);
        out.degree = to_rust<std::int64_t>(r.degree);
        out.pieces = to_rust<std::int32_t>(r.pieces);
        out.pieces_sq = to_rust<double>(r.pieces_sq);
        out.bc = to_rust<double>(r.bc);
        out.bc_iic = to_rust<double>(r.bc_iic);
        if (edges) {
            out.link_i = to_rust<std::int32_t>(g.eu);
            out.link_j = to_rust<std::int32_t>(g.ev);
        }
        out.edge_iic = to_rust<double>(r.edge_iic);
        out.edge_h = to_rust<double>(r.edge_h);
    }
    if (probabilities) {
        std::vector<double> f_row, awf_row;
        const conn::Csr g = conn::make_prob_graph(n, pairs, s, a.data(), &f_row, &awf_row);
        conn::ProbResult r = conn::analyse_probabilities(g, a.data(), expand_limit, importance, edges, betweenness);
        out.pc_num = r.pc_num;
        out.pc_direct = r.pc_direct;
        out.f_row = to_rust<double>(f_row);
        out.awf_row = to_rust<double>(awf_row);
        out.pc_row = to_rust<double>(r.pc_row);
        out.pc_conn = to_rust<double>(r.pc_conn);
        out.bc_pc = to_rust<double>(r.bc_pc);
        if (edges) {
            out.edge_i = to_rust<std::int32_t>(g.eu);
            out.edge_j = to_rust<std::int32_t>(g.ev);
        }
        out.edge_pc = to_rust<double>(r.edge_pc);
    }
    return out;
}

rust::Vec<double> conn_all_pairs(rust::Slice<const std::int32_t> pi, rust::Slice<const std::int32_t> pj,
                                 rust::Slice<const double> value, std::int32_t n, std::int32_t kind, double threshold,
                                 double probability, double pij_min, double expand_limit, bool conefor, bool links) {
    check(n > 0 && static_cast<double>(n) * n <= 4e8, "all-pairs lengths of so many nodes do not fit in memory");
    const conn::Pairs pairs = conn_pairs(pi, pj, value, n);
    const conn::Scenario s = conn_scenario(kind, threshold, probability, pij_min, conefor);
    std::vector<double> zero(static_cast<std::size_t>(n), 0.0);
    const conn::Csr g = links ? conn::make_link_graph(n, pairs, s)
                              : conn::make_prob_graph(n, pairs, s, zero.data(), nullptr, nullptr);
    return to_rust<double>(conn::all_pairs(g, links, expand_limit));
}

rust::Vec<double> conn_pair_lengths(rust::Slice<const double> value, std::int32_t kind, double threshold,
                                    double probability, double pij_min, bool conefor, bool links, bool cut) {
    const conn::Scenario s = conn_scenario(kind, threshold, probability, pij_min, conefor);
    rust::Vec<double> out;
    out.reserve(value.size());
    for (const double v : value) out.push_back(conn::pair_length(v, s, links, cut));
    return out;
}

ConnGains conn_addition_gains(rust::Slice<const double> d, rust::Slice<const double> area,
                              rust::Slice<const std::int64_t> cand_start, rust::Slice<const std::int32_t> cand_node,
                              rust::Slice<const double> cand_len, rust::Slice<const double> cand_area, bool hops) {
    const auto n = static_cast<std::int32_t>(area.size());
    check(d.size() == static_cast<std::size_t>(n) * n, "D must be n x n");
    const auto nc = static_cast<std::int32_t>(cand_area.size());
    check(cand_start.size() == static_cast<std::size_t>(nc) + 1 && cand_node.size() == cand_len.size(),
          "the candidates' edges do not match");
    const std::vector<double> dv = to_std(d);
    return gains_out(conn::addition_gains(dv, n, area.data(), cand_start.data(), cand_node.data(), cand_len.data(), nc,
                                          cand_area.data(), hops));
}

ConnGains conn_edge_gains(rust::Slice<const double> d, rust::Slice<const double> area,
                          rust::Slice<const std::int32_t> ea, rust::Slice<const std::int32_t> eb,
                          rust::Slice<const double> elen, bool hops) {
    const auto n = static_cast<std::int32_t>(area.size());
    check(d.size() == static_cast<std::size_t>(n) * n, "D must be n x n");
    check(ea.size() == eb.size() && ea.size() == elen.size(), "ea, eb and elen differ in length");
    const std::vector<double> dv = to_std(d);
    return gains_out(conn::edge_gains(dv, n, area.data(), ea.data(), eb.data(), elen.data(),
                                      static_cast<std::int32_t>(ea.size()), hops));
}

namespace {

ConnPairs pairs_out(conn::PairList&& p) {
    ConnPairs out;
    out.i = to_rust<std::int32_t>(p.i);
    out.j = to_rust<std::int32_t>(p.j);
    out.distance = to_rust<double>(p.distance);
    return out;
}

conn::CostFunction cost_function(std::int32_t f) {
    check(f == 0 || f == 1, "the cost function is 0 (1 / destination) or 1 (the mean)");
    return f == 0 ? conn::CostFunction::Destination : conn::CostFunction::Mean;
}

conn::CostGrid cost_grid(std::int64_t ny, std::int64_t nx, const std::array<double, 4>& extent, std::int32_t directions,
                         std::int32_t function) {
    check(ny > 0 && nx > 0, "the raster needs rows and columns");
    check(directions == 4 || directions == 8, "the directions are 4 or 8");
    conn::CostGrid g;
    g.ny = ny;
    g.nx = nx;
    g.xmin = extent[0];
    g.xmax = extent[1];
    g.ymin = extent[2];
    g.ymax = extent[3];
    g.directions = directions;
    g.function = cost_function(function);
    return g;
}

}  // namespace

ConnRasterNodes conn_raster_nodes(rust::Slice<const std::int32_t> values, rust::Slice<const std::uint8_t> na,
                                  std::int64_t ny, std::int64_t nx, std::array<double, 4> extent, bool centroids,
                                  bool keep_index) {
    const auto n = static_cast<std::size_t>(ny * nx);
    check(ny > 0 && nx > 0 && values.size() == n, "the raster must be ny x nx");
    check(na.empty() || na.size() == n, "the mask does not match the raster");
    conn::RasterGrid grid;
    grid.ny = ny;
    grid.nx = nx;
    grid.xmin = extent[0];
    grid.xmax = extent[1];
    grid.ymin = extent[2];
    grid.ymax = extent[3];
    conn::RasterNodes r = conn::raster_nodes(values.data(), na.empty() ? nullptr : na.data(), grid, centroids, keep_index);
    ConnRasterNodes out;
    out.ids = to_rust<std::int32_t>(r.ids);
    out.cells = to_rust<std::int64_t>(r.cells);
    out.cx = to_rust<double>(r.cx);
    out.cy = to_rust<double>(r.cy);
    out.node = to_rust<std::int32_t>(r.node);
    return out;
}

ConnPairs conn_point_distances(rust::Slice<const double> x, rust::Slice<const double> y, double max_distance) {
    check(x.size() == y.size(), "x and y differ in length");
    return pairs_out(conn::point_distances(x.data(), y.data(), static_cast<std::int32_t>(x.size()), max_distance));
}

ConnPairs conn_raster_edge_distances(rust::Slice<const std::int32_t> node, std::int32_t n_nodes, std::int64_t ny,
                                     std::int64_t nx, double resx, double resy, double max_distance) {
    check(ny > 0 && nx > 0 && node.size() == static_cast<std::size_t>(ny * nx), "the node raster must be ny x nx");
    return pairs_out(conn::raster_edge_distances(node.data(), n_nodes, nx, ny, resx, resy, max_distance));
}

rust::Vec<double> conn_gdal_areas(rust::Slice<const double> x, rust::Slice<const double> y,
                                  rust::Slice<const std::int64_t> ring_start, rust::Slice<const std::int64_t> poly_start,
                                  rust::Slice<const std::int64_t> geom_start) {
    check(x.size() == y.size() && !geom_start.empty() && !poly_start.empty() && !ring_start.empty(),
          "malformed geometries");
    return to_rust<double>(conn::gdal_areas(x.data(), y.data(), ring_start.data(), poly_start.data(), geom_start.data(),
                                            static_cast<std::int64_t>(geom_start.size()) - 1));
}

ConnPairs conn_least_cost(rust::Slice<const double> resistance, std::int64_t ny, std::int64_t nx,
                          std::array<double, 4> extent, rust::Slice<const std::int64_t> node_cell, std::int32_t directions,
                          std::int32_t function, double max_cost, std::int32_t max_threads) {
    const conn::CostGrid g = cost_grid(ny, nx, extent, directions, function);
    check(resistance.size() == static_cast<std::size_t>(ny * nx), "the resistance must be ny x nx");
    return pairs_out(conn::least_cost_distances(resistance.data(), g, node_cell.data(),
                                                static_cast<std::int32_t>(node_cell.size()), max_cost, max_threads));
}

ConnConductance conn_conductance(rust::Slice<const double> resistance, std::int64_t ny, std::int64_t nx,
                                 std::array<double, 4> extent, std::int32_t directions, std::int32_t function) {
    const conn::CostGrid g = cost_grid(ny, nx, extent, directions, function);
    check(resistance.size() == static_cast<std::size_t>(ny * nx), "the resistance must be ny x nx");
    conn::ConductanceEdges e = conn::conductance_edges(resistance.data(), g);
    ConnConductance out;
    out.a = to_rust<std::int64_t>(e.a);
    out.b = to_rust<std::int64_t>(e.b);
    out.conductance = to_rust<double>(e.conductance);
    return out;
}

ConnSamc conn_samc_transitions(rust::Slice<const double> resistance, rust::Slice<const double> absorption,
                               std::int64_t ny, std::int64_t nx, std::int32_t directions, std::int32_t function) {
    const auto n = static_cast<std::size_t>(ny * nx);
    check(ny > 0 && nx > 0 && resistance.size() == n && absorption.size() == n,
          "expected a resistance and an absorption raster of the same shape");
    conn::SamcTransitions t =
        conn::samc_transitions(resistance.data(), absorption.data(), nx, ny, directions, cost_function(function));
    ConnSamc out;
    out.cell = to_rust<std::int64_t>(t.cell);
    out.from = to_rust<std::int32_t>(t.from);
    out.to = to_rust<std::int32_t>(t.to);
    out.q = to_rust<double>(t.q);
    return out;
}

rust::Vec<double> samc_inverse_block(std::int64_t m, rust::Slice<const std::int32_t> from,
                                     rust::Slice<const std::int32_t> to, rust::Slice<const double> q,
                                     rust::Slice<const std::int64_t> targets) {
    check(m > 0 && from.size() == to.size() && from.size() == q.size(), "from, to and q differ in length");
    using Sparse = Eigen::SparseMatrix<double, Eigen::ColMajor, int>;
    std::vector<Eigen::Triplet<double, int>> t;
    t.reserve(from.size() + static_cast<std::size_t>(m));
    for (std::int64_t k = 0; k < m; ++k) t.emplace_back(static_cast<int>(k), static_cast<int>(k), 1.0);
    for (std::size_t k = 0; k < from.size(); ++k) {
        check(from[k] >= 0 && from[k] < m && to[k] >= 0 && to[k] < m, "a state outside the chain");
        t.emplace_back(from[k], to[k], -q[k]);
    }
    Sparse a(static_cast<int>(m), static_cast<int>(m));
    a.setFromTriplets(t.begin(), t.end());
    a.makeCompressed();
    Eigen::SparseLU<Sparse, Eigen::COLAMDOrdering<int>> lu;
    lu.compute(a);
    check(lu.info() == Eigen::Success, "the chain's matrix cannot be factorised");
    const auto k = static_cast<std::int64_t>(targets.size());
    rust::Vec<double> out;
    out.reserve(static_cast<std::size_t>(k * k));
    for (std::int64_t i = 0; i < k * k; ++i) out.push_back(0.0);
    const std::int64_t block = 64;
    for (std::int64_t s = 0; s < k; s += block) {
        const std::int64_t w = std::min(block, k - s);
        Eigen::MatrixXd rhs = Eigen::MatrixXd::Zero(m, w);
        for (std::int64_t c = 0; c < w; ++c) rhs(targets[s + c], c) = 1.0;
        const Eigen::MatrixXd x = lu.solve(rhs);
        for (std::int64_t r = 0; r < k; ++r)
            for (std::int64_t c = 0; c < w; ++c) out[static_cast<std::size_t>(r * k + s + c)] = x(targets[r], c);
    }
    return out;
}

ConnCentrality conn_centrality(rust::Slice<const std::int32_t> pi, rust::Slice<const std::int32_t> pj,
                               rust::Slice<const double> length, std::int32_t n, bool unit) {
    check(pi.size() == pj.size() && pi.size() == length.size(), "pi, pj and length differ in length");
    const conn::Csr g = conn::make_length_graph(n, to_std(pi), to_std(pj), to_std(length));
    conn::Centrality c = conn::centrality(g, unit);
    ConnCentrality out;
    out.betweenness = to_rust<double>(c.betweenness);
    out.closeness_sum = to_rust<double>(c.closeness_sum);
    out.reached = to_rust<std::int32_t>(c.reached);
    return out;
}

rust::Vec<double> sdm_knn_mean_distance(rust::Slice<const double> m, std::int64_t nm, rust::Slice<const double> g,
                                        std::int64_t ng, std::int64_t p, std::int64_t k, bool squared) {
    check(nm > 0 && p > 0 && m.size() == static_cast<std::size_t>(nm * p) && g.size() == static_cast<std::size_t>(ng * p),
          "expected two tables with the same columns");
    check(k >= 1, "k must be at least 1");
    return to_rust<double>(sdm::knn_mean_distance(m.data(), nm, g.data(), ng, p, k, squared));
}

namespace {

sdm::maxnet::Table maxnet_table(rust::Slice<const double> cont, rust::Slice<const std::int32_t> cat, std::int64_t n,
                                rust::Slice<const std::int32_t> n_levels) {
    check(n > 0, "no rows");
    check(cont.size() % static_cast<std::size_t>(n) == 0 && cat.size() % static_cast<std::size_t>(n) == 0,
          "cont and cat must have n rows");
    sdm::maxnet::Table t;
    t.n = n;
    t.n_cont = static_cast<std::int32_t>(cont.size() / static_cast<std::size_t>(n));
    t.n_cat = static_cast<std::int32_t>(cat.size() / static_cast<std::size_t>(n));
    t.cont = cont.data();
    t.cat = cat.data();
    t.n_levels.assign(n_levels.begin(), n_levels.end());
    check(static_cast<std::int32_t>(t.n_levels.size()) == t.n_cat, "one n_levels per factor");
    return t;
}

}  // namespace

MaxnetFit sdm_maxnet_fit(rust::Slice<const double> cont, rust::Slice<const std::int32_t> cat, std::int64_t n,
                         rust::Slice<const std::int32_t> n_levels, rust::Slice<const double> p, rust::Str classes,
                         double regmult, bool add_samples_to_background) {
    const sdm::maxnet::Table t = maxnet_table(cont, cat, n, n_levels);
    check(p.size() == static_cast<std::size_t>(n), "p must have one value per row");
    sdm::maxnet::Settings s;
    s.classes = std::string(classes);
    s.regmult = regmult;
    s.add_samples_to_background = add_samples_to_background;
    const sdm::maxnet::Model m = sdm::maxnet::fit(t, p.data(), s);
    MaxnetFit out;
    out.classes = m.classes;
    for (const sdm::maxnet::Feature& f : m.features) {
        out.kind.push_back(static_cast<std::int32_t>(f.kind));
        out.var.push_back(f.var);
        out.var2.push_back(f.var2);
        out.k1.push_back(f.k1);
        out.k2.push_back(f.k2);
        out.pk1.push_back(f.pk1);
        out.pk2.push_back(f.pk2);
        out.s1.push_back(f.s1);
        out.s2.push_back(f.s2);
    }
    out.penalty = to_rust<double>(m.penalty);
    out.lambda = to_rust<double>(m.lambda);
    out.feature_min = to_rust<double>(m.feature_min);
    out.feature_max = to_rust<double>(m.feature_max);
    out.var_min = to_rust<double>(m.var_min);
    out.var_max = to_rust<double>(m.var_max);
    out.sample_means = to_rust<double>(m.sample_means);
    out.majority = to_rust<std::int32_t>(m.majority);
    out.beta_index = to_rust<std::int32_t>(m.beta_index);
    out.betas = to_rust<double>(m.betas);
    out.alpha = m.alpha;
    out.entropy = m.entropy;
    out.n_passes = m.n_passes;
    out.n_added = m.n_added;
    out.n_rows = m.n_rows;
    return out;
}

rust::Vec<double> sdm_maxnet_predict(rust::Slice<const std::int32_t> kind, rust::Slice<const std::int32_t> var,
                                     rust::Slice<const std::int32_t> var2, rust::Slice<const double> pk1,
                                     rust::Slice<const double> pk2, rust::Slice<const double> betas,
                                     rust::Slice<const double> feature_min, rust::Slice<const double> feature_max,
                                     rust::Slice<const double> var_min, rust::Slice<const double> var_max, double alpha,
                                     double entropy, rust::Slice<const double> cont, rust::Slice<const std::int32_t> cat,
                                     std::int64_t n, rust::Slice<const std::int32_t> n_levels, bool clamp,
                                     std::int32_t output) {
    const std::size_t nb = betas.size();
    check(kind.size() == nb && var.size() == nb && var2.size() == nb && pk1.size() == nb && pk2.size() == nb &&
              feature_min.size() == nb && feature_max.size() == nb,
          "one value per coefficient expected");
    check(output >= 0 && output <= 3, "the output is 0 link, 1 exponential, 2 cloglog or 3 logistic");
    const sdm::maxnet::Table t = maxnet_table(cont, cat, n, n_levels);
    sdm::maxnet::Predictor m;
    for (std::size_t j = 0; j < nb; ++j) {
        sdm::maxnet::Feature f;
        f.kind = static_cast<sdm::maxnet::Kind>(kind[j]);
        f.var = var[j];
        f.var2 = var2[j];
        f.pk1 = pk1[j];
        f.pk2 = pk2[j];
        const bool categorical = f.kind == sdm::maxnet::Kind::Categorical;
        check(f.var >= 0 && f.var < (categorical ? t.n_cat : t.n_cont) &&
                  (f.kind != sdm::maxnet::Kind::Product || (f.var2 >= 0 && f.var2 < t.n_cont)),
              "a feature refers to a variable that is not in the data");
        m.features.push_back(f);
    }
    m.betas = to_std(betas);
    m.feature_min = to_std(feature_min);
    m.feature_max = to_std(feature_max);
    m.var_min = to_std(var_min);
    m.var_max = to_std(var_max);
    m.alpha = alpha;
    m.entropy = entropy;
    std::vector<double> out(static_cast<std::size_t>(n));
    sdm::maxnet::predict(m, t, clamp, static_cast<sdm::maxnet::Output>(output), out.data());
    return to_rust<double>(out);
}

rust::String sdm_r_as_character(double x) { return rust::String(sdm::maxnet::r_as_character(x)); }

double sdm_r_cor2(rust::Slice<const double> x, rust::Slice<const double> y, bool kendall) {
    check(x.size() == y.size(), "x and y differ in length");
    return sdm::r_cor2(x.data(), y.data(), static_cast<std::int64_t>(x.size()), kendall);
}

rust::Vec<double> sdm_r_colmeans(rust::Slice<const double> x, std::int64_t n, std::int64_t p) {
    check(n > 0 && p > 0 && x.size() == static_cast<std::size_t>(n * p), "expected an n x p table");
    rust::Vec<double> out;
    for (std::int64_t j = 0; j < p; ++j) {  // colMeans: one 80-bit pass, divided by n in 80 bits
        lsm::Extended s = 0.0;
        for (std::int64_t i = 0; i < n; ++i) s += x[static_cast<std::size_t>(i * p + j)];
        out.push_back(static_cast<double>(s / lsm::Extended(n)));
    }
    return out;
}

rust::Vec<double> nlm_rescale(rust::Slice<const double> x) {
    std::vector<double> v = to_std(x);
    nlm::rescale(v.data(), static_cast<std::int64_t>(v.size()));
    return to_rust<double>(v);
}

rust::Vec<double> nlm_random(std::int32_t seed, std::int64_t ny, std::int64_t nx) {
    check(ny > 0 && nx > 0, "the map needs rows and columns");
    nlm::RRng rng(seed);
    return to_rust<double>(nlm::random_field(rng, ny, nx));
}

rust::Vec<double> nlm_percolation(std::int32_t seed, std::int64_t ny, std::int64_t nx, double prob) {
    check(ny > 0 && nx > 0, "the map needs rows and columns");
    check(prob >= 0.0 && prob <= 1.0, "the probability is in [0, 1]");
    nlm::RRng rng(seed);
    return to_rust<double>(nlm::percolation(rng, ny, nx, prob));
}

rust::Vec<double> nlm_planar_gradient(std::int64_t ny, std::int64_t nx, double direction, bool rescale, bool edge) {
    check(ny > 0 && nx > 0, "the map needs rows and columns");
    std::vector<double> g = nlm::planar_gradient(ny, nx, direction);
    const std::int64_t n = ny * nx;
    if (edge) {
        // nlm_edgegradient(): a rescaled planar gradient folded around 0.5
        nlm::rescale(g.data(), n);
        nlm::edge_from_planar(g.data(), n);
    }
    if (rescale) nlm::rescale(g.data(), n);
    return to_rust<double>(g);
}

rust::Vec<double> nlm_distance(rust::Slice<const std::uint8_t> origin, std::int64_t ny, std::int64_t nx,
                               std::int32_t method, double res, double xmin, double ymax, double xres, double yres) {
    check(ny > 0 && nx > 0 && origin.size() == static_cast<std::size_t>(ny * nx), "the origin must be ny x nx");
    const std::vector<std::uint8_t> o(origin.begin(), origin.end());
    switch (method) {
        case 0: return to_rust<double>(nlm::distance_gdal(o, ny, nx, res));
        case 1: return to_rust<double>(nlm::distance_haversine(o, ny, nx, xmin, ymax, xres, yres));
        case 2: return to_rust<double>(nlm::distance_exact(o, ny, nx, res));
        default: throw std::invalid_argument("the distance method is 0 to 2");
    }
}

rust::Vec<double> nlm_r_cumsum(rust::Slice<const double> x) {
    rust::Vec<double> out;
    out.reserve(x.size());
    lsm::Extended s = 0.0;
    for (const double v : x) {
        s += v;
        out.push_back(static_cast<double>(s));
    }
    return out;
}

rust::Vec<double> nlm_cut(rust::Slice<const double> x, rust::Slice<const double> breaks) {
    std::vector<double> out(x.size());
    nlm::cut_include_lowest(x.data(), static_cast<std::int64_t>(x.size()), to_std(breaks), out.data());
    return to_rust<double>(out);
}

rust::Vec<double> nlm_jenks_breaks(rust::Slice<const double> sorted, std::int32_t k) {
    check(k >= 1, "k must be >= 1");
    return to_rust<double>(nlm::jenks_breaks(to_std(sorted), k));
}

rust::Vec<double> nlm_curds(std::int32_t seed, rust::Slice<const double> curds, rust::Slice<const std::int64_t> steps,
                            rust::Slice<const double> wheyes, bool with_wheyes) {
    check(curds.size() == steps.size() && (!with_wheyes || wheyes.size() == steps.size()),
          "curds, recursion_steps and wheyes must have the same length");
    const std::vector<double> p = to_std(curds), w = to_std(wheyes);
    const std::vector<std::int64_t> s = to_std(steps);
    nlm::RRng rng(seed);
    std::int64_t size = 0;
    return to_rust<double>(nlm::curds(rng, p, s, with_wheyes ? &w : nullptr, size));
}

rust::Vec<double> nlm_mpd(std::int64_t size, rust::Slice<const double> rand_dev, std::uint32_t seed, bool torus) {
    check(size >= 3, "the map's side is 3 or more");
    return to_rust<double>(nlm::mpd_square(size, to_std(rand_dev), seed, torus));
}

rust::Vec<double> nlm_neigh(std::int64_t ny, std::int64_t nx, std::int32_t n_categories,
                            rust::Slice<const std::int64_t> cells_per_cat, std::int32_t neighbourhood, float p_neigh,
                            float p_empty, std::uint32_t seed) {
    check(ny > 0 && nx > 0, "the map needs rows and columns");
    check(static_cast<std::int64_t>(cells_per_cat.size()) == n_categories + 1,
          "cells_per_cat must have n_categories + 1 values");
    return to_rust<double>(nlm::neigh(ny, nx, n_categories, to_std(cells_per_cat), neighbourhood, p_neigh, p_empty, seed));
}

rust::Vec<double> nlm_random_rectangular_cluster(std::int64_t ny, std::int64_t nx, std::int32_t minl, std::int32_t maxl,
                                                 std::uint32_t seed) {
    check(ny > 0 && nx > 0, "the map needs rows and columns");
    return to_rust<double>(nlm::random_rectangular_cluster(ny, nx, minl, maxl, seed));
}

NlmMosaicOut nlm_mosaic_field(std::int32_t seed, std::int64_t ny, std::int64_t nx, std::int32_t n, bool nlmr_compat,
                              double mean, double sd, bool collect, bool infinit) {
    check(ny > 0 && nx > 0, "the map needs rows and columns");
    nlm::RRng rng(seed);
    nlm::MosaicFieldResult r = nlm::mosaic_field(rng, ny, nx, n, nlmr_compat, mean, sd, collect, infinit);
    NlmMosaicOut out;
    out.sum = to_rust<double>(r.sum);
    out.n_steps = static_cast<std::int64_t>(r.steps.size());
    for (const auto& s : r.steps)
        for (const double v : s) out.steps.push_back(v);
    out.infinit = to_rust<double>(r.inf);
    return out;
}

namespace {

NlmGermsOut germs_out(nlm::GermDraws&& g) {
    NlmGermsOut out;
    out.x = to_rust<double>(g.x);
    out.y = to_rust<double>(g.y);
    out.v = to_rust<double>(g.v);
    return out;
}

}  // namespace

NlmGermsOut nlm_tess_germs(std::int32_t seed, std::int64_t germs, double width, double height) {
    check(germs >= 1, "germs must be a positive integer");
    nlm::RRng rng(seed);
    return germs_out(nlm::tess_germs(rng, germs, width, height));
}

NlmGermsOut nlm_gibbs_germs(std::int32_t seed, std::int64_t germs, double r, std::int32_t patch_classes, double width,
                            double height, std::int32_t giveup) {
    check(germs >= 1 && patch_classes >= 1, "germs and patch_classes must be positive integers");
    nlm::RRng rng(seed);
    return germs_out(nlm::gibbs_germs(rng, germs, r, patch_classes, width, height, giveup));
}

rust::Vec<double> nlm_nearest_germ_raster(rust::Slice<const double> x, rust::Slice<const double> y,
                                          rust::Slice<const double> v, std::int64_t ny, std::int64_t nx) {
    check(x.size() == y.size() && x.size() == v.size(), "x, y and values differ in length");
    check(ny > 0 && nx > 0, "the map needs rows and columns");
    return to_rust<double>(nlm::nearest_germ_raster(to_std(x), to_std(y), to_std(v), ny, nx));
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
    for (const auto& ids : r.largest) {
        out.largest.push_back(static_cast<std::int64_t>(ids.size()));
        for (std::int32_t id : ids) out.largest.push_back(id);
    }
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

SecantOut rest_intactness_secant(rust::Slice<const double> h0, rust::Slice<const double> tlo,
                                 rust::Slice<const double> thi, std::int64_t ny, std::int64_t nx, double gamma,
                                 double beta, double z, std::int32_t radius, double dx, double dy) {
    check(static_cast<i64>(h0.size()) == ny * nx && h0.size() == tlo.size() && h0.size() == thi.size(),
          "the footprint and the bounds of its reduction differ in size from ny x nx");
    std::vector<double> sigma;
    SecantOut out;
    out.u0 = rest::intactness_secant(h0.data(), tlo.data(), thi.data(), ny, nx,
                                     intactness_params(gamma, beta, z, radius, dx, dy), max_threads(), sigma);
    out.sigma = to_rust<double>(sigma);
    return out;
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

prio::ReadRules prio_rules(std::int32_t layers, rust::Slice<const float> nodata, rust::Slice<const float> extract,
                           rust::Slice<const float> thresholds) {
    const std::size_t n = static_cast<std::size_t>(layers);
    check(layers > 0 && nodata.size() == n && extract.size() == n && thresholds.size() == n,
          "a nodata, an extracted value and a threshold per layer");
    prio::ReadRules r;
    r.layers = layers;
    r.nodata.assign(nodata.begin(), nodata.end());
    r.extract.assign(extract.begin(), extract.end());
    r.thresholds.assign(thresholds.begin(), thresholds.end());
    return r;
}

void prio_check(rust::Slice<const float> values, std::int32_t layers, std::int64_t ny, std::int64_t nx,
                rust::Slice<const std::uint8_t> area) {
    check(ny > 0 && nx > 0 && static_cast<i64>(values.size()) == i64{layers} * ny * nx, "the layers are layers x ny x nx");
    check(area.empty() || static_cast<i64>(area.size()) == ny * nx, "the analysis area differs from the layers' grid");
}

// Zonation's values of a map at the data cells (readImageMatchingMapDataIndexes): NoData 0, clipped to [lo, hi],
// rounded first when `round`
std::vector<double> gather(rust::Slice<const double> map, const prio::Buffer<i64>& cells, i64 n, double lo, double hi,
                           bool round) {
    std::vector<double> out;
    if (map.empty()) return out;
    check(static_cast<i64>(map.size()) == n, "a map of the features' grid");
    out.resize(cells.size());
    for (std::size_t i = 0; i < cells.size(); ++i) {
        double v = map[static_cast<std::size_t>(cells[i])];
        if (std::isnan(v)) {
            out[i] = 0.0;
            continue;
        }
        if (round) v = std::nearbyint(v);
        out[i] = std::min(std::max(v, lo), hi);
    }
    return out;
}

prio::Rule prio_rule(const rust::String& s) {
    const std::string r(s);
    if (r == "ABF") return prio::Rule::ABF;
    if (r == "CAZ1") return prio::Rule::CAZ1;
    if (r == "CAZ2") return prio::Rule::CAZ2;
    if (r == "CAZMAX") return prio::Rule::CAZMAX;
    if (r == "CAZP") return prio::Rule::CAZP;
    if (r == "CAZPW") return prio::Rule::CAZPW;
    if (r == "RAND") return prio::Rule::RAND;
    if (r == "LOAD") return prio::Rule::LOAD;
    throw std::invalid_argument("unknown marginal loss rule: " + r);
}

}  // namespace

PrioStatsOut prio_stats(rust::Slice<const float> values, std::int32_t layers, std::int64_t ny, std::int64_t nx,
                        rust::Slice<const float> nodata, rust::Slice<const float> extract,
                        rust::Slice<const float> thresholds, rust::Slice<const std::uint8_t> area) {
    prio_check(values, layers, ny, nx, area);
    prio::Stats s(prio_rules(layers, nodata, extract, thresholds));
    s.add_block(values.data(), ny, nx, area.empty() ? nullptr : area.data());
    PrioStatsOut out;
    out.pixels = s.pixels;
    out.zeros = to_rust<std::int64_t>(s.zeros);
    out.negative = to_rust<std::int64_t>(s.negative);
    out.nodata = to_rust<std::int64_t>(s.nodata);
    out.max_values = to_rust<float>(s.max_values);
    out.orig_max_values = to_rust<float>(s.orig_max_values);
    return out;
}

std::unique_ptr<prio::Data> prio_data_new(rust::Slice<const float> nodata, rust::Slice<const float> extract,
                                          rust::Slice<const float> thresholds, rust::Slice<const float> max_values,
                                          rust::Slice<const std::uint8_t> keep_zeros, std::int32_t bits) {
    check(bits == 8 || bits == 16, "bits is 8 or 16");
    const std::int32_t layers = static_cast<std::int32_t>(nodata.size());
    const std::size_t L = static_cast<std::size_t>(layers);
    check(max_values.size() == L && keep_zeros.size() == L, "a maximum and a zero mode per layer");
    return std::make_unique<prio::Data>(prio_rules(layers, nodata, extract, thresholds),
                                        std::vector<float>(max_values.begin(), max_values.end()),
                                        std::vector<std::uint8_t>(keep_zeros.begin(), keep_zeros.end()), bits);
}

void prio_data_set_conditions(prio::Data& data, rust::Slice<const std::int32_t> condition,
                              rust::Slice<const std::uint8_t> renormalize) {
    const std::size_t L = static_cast<std::size_t>(data.rules.layers);
    check(condition.size() == L && renormalize.size() == L, "one condition and renormalize flag per layer");
    data.condition.assign(condition.begin(), condition.end());
    data.renormalize.assign(renormalize.begin(), renormalize.end());
}

void prio_data_add(prio::Data& data, rust::Slice<const float> values, std::int64_t ny, std::int64_t nx,
                   rust::Slice<const std::uint8_t> area, rust::Slice<const float> conditions, std::int64_t block_rows) {
    prio_check(values, data.rules.layers, ny, nx, area);
    check(conditions.size() % static_cast<std::size_t>(ny * nx) == 0, "the conditions are (conditions, ny, nx)");
    data.add_block(values.data(), ny, nx, area.empty() ? nullptr : area.data(),
                   conditions.empty() ? nullptr : conditions.data(), block_rows > 0 ? block_rows : ny);
    data.finish();
    check(data.cells() > 0, "no cells with data");
}

rust::Vec<std::int64_t> prio_data_cells(const prio::Data& data) {
    rust::Vec<std::int64_t> out;
    out.reserve(data.cell_index.size());
    for (i64 c : data.cell_index) out.push_back(c);
    return out;
}

PrioDataInfo prio_data_info(const prio::Data& data) {
    PrioDataInfo out;
    out.bits = data.bits;
    out.max_norm_sums = to_rust<double>(data.max_norm_sums);
    out.orig_abs_sums = to_rust<double>(data.orig_abs_sums);
    out.layer_scale = to_rust<double>(data.layer_scale);
    out.initial_levels = to_rust<double>(data.initial_levels);
    out.quantization_errors = to_rust<double>(data.quantization_errors);
    return out;
}

rust::Vec<double> prio_data_layer(const prio::Data& data, std::int32_t layer, bool scaled, float missing) {
    check(layer >= 0 && layer < data.rules.layers, "no such layer");
    std::vector<double> out(static_cast<std::size_t>(data.cells()), static_cast<double>(missing));
    const double sc = data.layer_scale[static_cast<std::size_t>(layer)];
#pragma omp parallel for schedule(static)
    for (i64 c = 0; c < data.cells(); ++c) {
        const long q = data.value_at(c, layer);
        if (q >= 0) out[static_cast<std::size_t>(c)] = scaled ? static_cast<double>(q) * sc : static_cast<double>(q);
    }
    return to_rust<double>(out);
}

double prio_data_replace_layer(prio::Data& data, std::int32_t layer, rust::Slice<const float> values, double level) {
    return data.replace_layer(layer, std::vector<float>(values.begin(), values.end()), level);
}

std::int64_t prio_data_retain(prio::Data& data, std::int32_t layer, std::int32_t kind, rust::Slice<const float> values,
                              float fraction) {
    check(kind == 2 || static_cast<i64>(values.size()) == data.cells(), "retention: one value per data cell");
    return data.retain(layer, kind, values.empty() ? nullptr : values.data(), fraction);
}

rust::Vec<float> prio_data_preprocessing(const prio::Data& data, rust::Slice<const float> mult,
                                         rust::Slice<const float> weights, bool unscaled) {
    const std::size_t L = static_cast<std::size_t>(data.rules.layers);
    check(mult.size() == L && (weights.empty() || weights.size() == L), "one multiplier (and weight) per layer");
    return to_rust<float>(data.preprocessing(mult.data(), weights.empty() ? nullptr : weights.data(), unscaled));
}

void prio_data_scale_level(prio::Data& data, std::int32_t layer, double factor) {
    check(layer >= 0 && layer < data.rules.layers, "no such layer");
    data.initial_levels[static_cast<std::size_t>(layer)] *= factor;
}

void prio_data_set_layer_values(prio::Data& data, std::int32_t layer, rust::Slice<const float> values) {
    check(layer >= 0 && layer < data.rules.layers, "no such layer");
    check(static_cast<i64>(values.size()) == data.cells(), "one value per data cell");
    const float* v = values.data();
    data.update_layer(layer, [v](i64 c, unsigned) { return static_cast<unsigned>(static_cast<std::uint16_t>(v[c])); });
}

namespace {

prio::RankSettings rank_settings(const prio::Data& d, const PrioSettings& settings, rust::Slice<const double> cost,
                                 rust::Slice<const double> hmask, rust::Slice<const double> solution, i64 n) {
    const std::size_t L = static_cast<std::size_t>(d.rules.layers);
    check(settings.weights.size() == L && settings.abf_z.size() == L, "a weight and an ABF z per layer");
    prio::RankSettings s;
    s.rule = prio_rule(settings.rule);
    s.param = settings.param;
    s.weights.assign(settings.weights.begin(), settings.weights.end());
    s.abf_z.assign(settings.abf_z.begin(), settings.abf_z.end());
    for (double v : gather(cost, d.cell_index, n, 0.0, std::numeric_limits<double>::infinity(), false))
        s.cost.push_back(static_cast<float>(v));
    s.cost_in_ranking = settings.cost_in_ranking;
    for (double v : gather(hmask, d.cell_index, n, 0.0, 255.0, true)) s.hmask.push_back(static_cast<std::uint8_t>(v));
    for (double v : gather(solution, d.cell_index, n, -std::numeric_limits<double>::infinity(),
                           std::numeric_limits<double>::infinity(), false))
        s.solution.push_back(static_cast<float>(v));
    check(s.rule != prio::Rule::LOAD || !s.solution.empty(), "the LOAD rule needs a solution");
    s.max_iterations = settings.max_iterations;
    s.ranking_error = settings.ranking_error;
    s.stop_on_plateau = settings.stop_on_plateau;
    s.smoothing = settings.smoothing;
    s.curves_interval = settings.curves_interval;
    s.seed = settings.seed;
    return s;
}

}  // namespace

PrioRankOut prio_rank(const prio::Data& d, const PrioSettings& settings, std::int64_t ny, std::int64_t nx,
                      rust::Slice<const double> cost, rust::Slice<const double> hmask,
                      rust::Slice<const double> solution) {
    const prio::RankResult r = prio::rank(d, rank_settings(d, settings, cost, hmask, solution, ny * nx));
    PrioRankOut out;
    out.cell_index = prio_data_cells(d);
    out.rank = to_rust<float>(r.rank);
    out.order = to_rust<std::uint32_t>(r.order);
    out.iterations = r.iterations;
    out.ranking_error = r.ranking_error;
    out.history_error = to_rust<double>(r.history_error);
    out.curve_rank = to_rust<float>(r.curve_rank);
    out.curve_levels = to_rust<float>(r.curve_levels);
    out.curve_cost = to_rust<double>(r.curve_cost);
    out.auc = r.auc;
    out.total_cost = r.total_cost;
    out.curve_interval = r.curve_interval;
    out.orig_abs_sums = to_rust<double>(d.orig_abs_sums);
    out.quantization_errors = to_rust<double>(d.quantization_errors);
    return out;
}

PrioFlexOut prio_flexibility(const prio::Data& d, const PrioSettings& settings, std::int64_t ny, std::int64_t nx,
                             rust::Slice<const double> cost, rust::Slice<const double> hmask,
                             rust::Slice<const std::uint32_t> order, rust::Slice<const float> curve_rank,
                             rust::Slice<const float> curve_levels, std::int64_t curve_interval, double max_error,
                             bool up, bool down) {
    const prio::RankSettings s = rank_settings(d, settings, cost, hmask, {}, ny * nx);
    prio::RankResult r;
    r.order.assign(order.begin(), order.end());
    r.curve_rank.assign(curve_rank.begin(), curve_rank.end());
    r.curve_levels.assign(curve_levels.begin(), curve_levels.end());
    r.curve_interval = curve_interval;
    check(static_cast<i64>(r.order.size()) == d.cells(), "one position per cell");
    check(r.curve_levels.size() == r.curve_rank.size() * static_cast<std::size_t>(d.rules.layers),
          "curve levels: (samples, layers)");
    std::vector<float> pos, neg;
    prio::flexibility(d, s, r, max_error, up, down, pos, neg);
    PrioFlexOut out;
    out.up = to_rust<float>(pos);
    out.down = to_rust<float>(neg);
    return out;
}

namespace {

lsio::Runs runs_of(const RunsOut& r) {
    lsio::Runs out;
    out.offsets.assign(r.offsets.begin(), r.offsets.end());
    out.y.assign(r.y.begin(), r.y.end());
    out.x0.assign(r.x0.begin(), r.x0.end());
    out.x1.assign(r.x1.begin(), r.x1.end());
    check(!out.offsets.empty() && out.y.size() == out.x0.size() && out.y.size() == out.x1.size() &&
              static_cast<std::size_t>(out.offsets.back()) == out.y.size(),
          "inconsistent runs");
    return out;
}

}  // namespace

RunsOut io_polygon_runs(rust::Slice<const double> xy, rust::Slice<const std::int64_t> ring_offsets,
                        rust::Slice<const std::int64_t> geom_rings, rust::Slice<const double> gt, std::int32_t nx,
                        std::int32_t ny, bool all_touched) {
    check(gt.size() == 6 && xy.size() % 2 == 0 && !geom_rings.empty() && !ring_offsets.empty(),
          "polygons: xy pairs, ring and geometry offsets, a geotransform of 6");
    check(static_cast<std::size_t>(ring_offsets.back()) == xy.size() / 2 &&
              static_cast<std::size_t>(geom_rings.back()) + 1 == ring_offsets.size(),
          "polygons: inconsistent offsets");
    const lsio::Runs r = lsio::polygon_runs(xy.data(), ring_offsets.data(), geom_rings.data(),
                                            static_cast<i64>(geom_rings.size()) - 1, gt.data(), nx, ny, all_touched);
    RunsOut out;
    out.offsets = to_rust<std::int64_t>(r.offsets);
    out.y = to_rust<std::int32_t>(r.y);
    out.x0 = to_rust<std::int32_t>(r.x0);
    out.x1 = to_rust<std::int32_t>(r.x1);
    return out;
}

void io_burn_runs(rust::Slice<double> out, std::int32_t ny, std::int32_t nx, const RunsOut& runs,
                  rust::Slice<const double> value, bool add) {
    const lsio::Runs r = runs_of(runs);
    check(static_cast<i64>(out.size()) == i64{ny} * nx, "out differs from ny x nx");
    check(static_cast<i64>(value.size()) == r.zones(), "one value per zone");
    for (std::size_t k = 0; k < r.y.size(); ++k) check(r.y[k] < ny && r.x1[k] <= nx, "zones larger than the grid");
    lsio::burn_runs(out.data(), ny, nx, r, value.data(), add);
}

rust::Vec<double> io_zonal_stats(rust::Slice<const double> values, std::int32_t bands, std::int32_t ny,
                                 std::int32_t nx, const RunsOut& runs, rust::Slice<const std::int32_t> stats,
                                 rust::Slice<const double> percentiles) {
    const lsio::Runs r = runs_of(runs);
    check(static_cast<i64>(values.size()) == i64{bands} * ny * nx, "the values are bands x ny x nx");
    check(percentiles.size() == stats.size(), "one percentile per statistic");
    for (std::size_t k = 0; k < r.y.size(); ++k) check(r.y[k] < ny && r.x1[k] <= nx, "zones larger than the raster");
    return to_rust<double>(lsio::zonal_stats(values.data(), bands, ny, nx, false, 0.0, r,
                                             std::vector<int>(stats.begin(), stats.end()),
                                             std::vector<double>(percentiles.begin(), percentiles.end())));
}

rust::Vec<double> io_exact_sums(rust::Slice<const double> values, std::int64_t layers, std::int64_t ny,
                                std::int64_t nx, rust::Slice<const double> grid, rust::Slice<const double> xy,
                                rust::Slice<const std::int64_t> ring_offsets,
                                rust::Slice<const std::int64_t> polygon_offsets,
                                rust::Slice<const std::int64_t> feature_offsets) {
    check(static_cast<i64>(values.size()) == layers * ny * nx, "the values are layers x ny x nx");
    check(grid.size() == 6 && xy.size() % 2 == 0 && !feature_offsets.empty(), "a grid of 6, xy pairs, offsets");
    const i64 nf = static_cast<i64>(feature_offsets.size()) - 1;
    std::vector<std::vector<exactextract::Polygon>> features(static_cast<std::size_t>(nf));
    for (i64 f = 0; f < nf; ++f) {
        for (i64 p = feature_offsets[f]; p < feature_offsets[f + 1]; ++p) {
            exactextract::Polygon poly;
            for (i64 r = polygon_offsets[p]; r < polygon_offsets[p + 1]; ++r) {
                exactextract::Ring ring;
                for (i64 k = ring_offsets[r]; k < ring_offsets[r + 1]; ++k) ring.push_back({xy[2 * k], xy[2 * k + 1]});
                poly.rings.push_back(std::move(ring));
            }
            features[static_cast<std::size_t>(f)].push_back(std::move(poly));
        }
    }
    const lsio::ExactGrid g{grid[0], grid[1], grid[2], grid[3], grid[4], grid[5], ny, nx};
    return to_rust<double>(lsio::exact_sums(values.data(), layers, g, features, 30000000, max_threads()));
}

double prio_eigen_sum(rust::Slice<const double> values) {
    return prio::eigen_avx_sum(values.data(), static_cast<i64>(values.size()), 0);
}

float prio_eigen_sum_f32(rust::Slice<const float> values, std::int64_t offset) {
    return prio::eigen_avx_sum(values.data(), static_cast<i64>(values.size()), offset);
}

MarxanOut opt_marxan(const MarxanData& data, const MarxanOptions& options) {
    opt::marxan::Data d;
    d.npu = static_cast<int>(data.cost.size());
    d.nsp = static_cast<int>(data.target.size());
    d.cost.assign(data.cost.begin(), data.cost.end());
    d.status.assign(data.status.begin(), data.status.end());
    d.offset.assign(data.offset.begin(), data.offset.end());
    d.spindex.assign(data.spindex.begin(), data.spindex.end());
    d.amount.assign(data.amount.begin(), data.amount.end());
    d.target.assign(data.target.begin(), data.target.end());
    d.prop.assign(data.prop.begin(), data.prop.end());
    d.spf.assign(data.spf.begin(), data.spf.end());
    d.targetocc.assign(data.targetocc.begin(), data.targetocc.end());
    d.fixedcost.assign(data.fixedcost.begin(), data.fixedcost.end());
    d.nb_start.assign(data.nb_start.begin(), data.nb_start.end());
    d.nb.assign(data.nb.begin(), data.nb.end());
    d.nb_cost.assign(data.nb_cost.begin(), data.nb_cost.end());
    const std::size_t npu = static_cast<std::size_t>(d.npu), nsp = static_cast<std::size_t>(d.nsp);
    check(d.status.size() == npu && d.offset.size() == npu + 1 && d.fixedcost.size() == npu &&
              d.nb_start.size() == npu + 1,
          "inconsistent planning-unit arrays");
    check(d.prop.size() == nsp && d.spf.size() == nsp && d.targetocc.size() == nsp, "inconsistent feature arrays");
    check(d.spindex.size() == d.amount.size() && static_cast<std::int64_t>(d.amount.size()) == d.offset[npu],
          "inconsistent amounts");
    check(d.nb.size() == d.nb_cost.size() && static_cast<std::int64_t>(d.nb.size()) == d.nb_start[npu],
          "inconsistent boundaries");
    opt::marxan::Options o;
    o.blm = options.blm;
    o.prop = options.prop;
    o.iterations = options.iterations;
    o.tinit = options.tinit;
    o.tcool = options.tcool;
    o.titns = static_cast<long>(options.titns);
    o.repeats = static_cast<long>(options.repeats);
    o.costthresh = options.costthresh;
    o.tpf1 = options.tpf1;
    o.tpf2 = options.tpf2;
    o.misslevel = options.misslevel;
    o.runmode = options.runmode;
    o.heurtype = options.heurtype;
    o.itimptype = options.itimptype;
    o.connectivity_in = options.connectivity_in ? 1 : 0;
    o.seed = options.seed;
    o.threads = max_threads();
    const opt::marxan::Result r = opt::marxan::solve(d, o);
    MarxanOut out;
    out.penalty = to_rust<double>(r.penalty);
    out.target = to_rust<double>(r.target);
    out.unmet = r.unmet;
    out.best = r.best;
    for (const auto& run : r.runs) {
        for (int x : run.R) out.solutions.push_back(x);
        out.score.push_back(run.score.total);
        out.cost.push_back(run.incremental.cost);
        out.penalty_total.push_back(run.incremental.penalty);
        out.incremental_score.push_back(run.incremental.total);
        out.pus.push_back(run.score.pus);
        out.connectivity.push_back(run.connectivity);
        out.shortfall.push_back(run.shortfall);
        out.missing.push_back(run.missing);
        out.mpm.push_back(run.min_prop_met);
    }
    return out;
}

rust::Vec<double> proxirr_alpha(rust::Slice<const double> local, rust::Slice<const double> global,
                                rust::Slice<const double> target, bool triage) {
    check(local.size() == global.size() && local.size() == target.size(), "a global amount and a target per entry");
    return to_rust<double>(opt::proxirr::alpha(to_std(local), to_std(global), to_std(target), triage, max_threads()));
}

ProxirrOut proxirr_beta_gamma(rust::Slice<const std::int64_t> start, rust::Slice<const double> alphas) {
    check(!start.empty() && start[0] == 0 && start[start.size() - 1] == static_cast<i64>(alphas.size()),
          "the units' alphas are offsets into them");
    std::vector<double> beta, gamma;
    opt::proxirr::beta_gamma(to_std(start), to_std(alphas), max_threads(), beta, gamma);
    ProxirrOut out;
    out.beta = to_rust<double>(beta);
    out.gamma = to_rust<double>(gamma);
    return out;
}

FeaturesOut opt_features(rust::Slice<const double> values, std::int64_t layers, rust::Slice<const std::int64_t> unit) {
    check(layers > 0 && values.size() == static_cast<std::size_t>(layers) * unit.size(), "the values are layers x cells");
    opt::FeatureMatrix m(layers);
    m.add(values.data(), static_cast<i64>(unit.size()), unit.data(), max_threads());
    std::vector<i64> start, index;
    std::vector<double> value;
    m.csr(start, index, value);
    FeaturesOut out;
    out.totals = to_rust<double>(m.totals());
    out.start = to_rust<std::int64_t>(start);
    out.index = to_rust<std::int64_t>(index);
    out.value = to_rust<double>(value);
    return out;
}

SolveOut opt_solve(const MilpModel& model, const SolveSettings& settings, rust::Str method, std::int64_t n_units) {
    opt::Model m;
    m.start.assign(model.start.begin(), model.start.end());
    m.index.assign(model.index.begin(), model.index.end());
    m.value.assign(model.value.begin(), model.value.end());
    m.obj.assign(model.obj.begin(), model.obj.end());
    m.lb.assign(model.lb.begin(), model.lb.end());
    m.ub.assign(model.ub.begin(), model.ub.end());
    m.row_lo.assign(model.row_lo.begin(), model.row_lo.end());
    m.row_hi.assign(model.row_hi.begin(), model.row_hi.end());
    m.integer.assign(model.integer.begin(), model.integer.end());
    m.nrow = static_cast<i64>(m.row_lo.size());
    m.ncol = static_cast<i64>(m.obj.size());
    m.maximize = model.maximize;
    m.offset = model.offset;
    m.check();
    opt::SolveOptions o;
    o.gap = settings.gap;
    o.time_limit = settings.time_limit;
    o.threads = max_threads();
    const std::string p(settings.presolve);
    o.presolve = p == "off" ? opt::Presolve::off : p == "on" ? opt::Presolve::on : opt::Presolve::choose;
    o.relaxation = settings.relaxation;
    o.start.assign(settings.start.begin(), settings.start.end());
    const std::string how(method);
    opt::SolveResult r;
    if (how == "covering") {
        check(opt::is_covering(m), "not a covering model (a minimisation with only >= rows and finite bounds)");
        r = opt::solve_covering(m, o);
    } else if (how == "wide") {
        check(opt::is_wide(m, n_units), "not a wide model (units with finite bounds, few other columns)");
        r = opt::solve_wide(m, n_units, o);
    } else if (how == "zones") {
        check(opt::is_zone_covering(m, n_units),
              "not a covering model with zones (n_units units, target rows and one row per unit)");
        r = opt::solve_zone_covering(m, n_units, o);
    } else if (how == "zones_wide") {
        check(opt::is_zone_wide(m, n_units),
              "not a wide model with zones (n_units units with a GUB row each, few other columns)");
        r = opt::solve_zone_wide(m, n_units, o);
    } else {
        check(how == "highs", "the method is highs, covering, wide, zones or zones_wide");
        r = opt::solve_highs(m, o);
    }
    SolveOut out;
    out.status = r.status;
    out.has_solution = r.has_solution;
    out.x = to_rust<double>(r.x);
    out.objective = r.objective;
    out.bound = r.bound;
    out.gap = r.gap;
    return out;
}

namespace {

opt::eval::Csc csc_of(rust::Slice<const std::int64_t> start, rust::Slice<const std::int64_t> index,
                      rust::Slice<const double> value, i64 nrow) {
    check(!start.empty() && index.size() == value.size() &&
              start[start.size() - 1] == static_cast<i64>(value.size()),
          "a matrix in compressed columns");
    opt::eval::Csc m;
    m.nrow = nrow;
    m.ncol = static_cast<i64>(start.size()) - 1;
    m.start = to_std(start);
    m.index = to_std(index);
    m.value = to_std(value);
    return m;
}

}  // namespace

double eval_boundary(double edge_factor, rust::Slice<const std::int64_t> start, rust::Slice<const std::int64_t> index,
                     rust::Slice<const double> value, rust::Slice<const double> exposed,
                     rust::Slice<const double> total, rust::Slice<const double> solution) {
    const i64 n = static_cast<i64>(start.size()) - 1;
    check(static_cast<i64>(exposed.size()) == n && static_cast<i64>(total.size()) == n &&
              static_cast<i64>(solution.size()) == n,
          "an exposed and a total boundary and a value per unit");
    return opt::eval::boundary(edge_factor, csc_of(start, index, value, n), to_std(exposed), to_std(total),
                               to_std(solution));
}

double eval_connectivity(rust::Slice<const std::int64_t> start, rust::Slice<const std::int64_t> index,
                         rust::Slice<const double> value, rust::Slice<const double> solution, bool asym) {
    const i64 n = static_cast<i64>(start.size()) - 1;
    check(static_cast<i64>(solution.size()) == n, "a value per unit");
    const opt::eval::Csc m = csc_of(start, index, value, n);
    for (const i64 r : m.index) check(r >= 0 && r < n, "rows within the units");
    return asym ? opt::eval::asym_connectivity(m, to_std(solution)) : opt::eval::connectivity(m, to_std(solution));
}

namespace {

template <typename T, typename U>
std::vector<T> std_of(const rust::Vec<U>& v) {
    return std::vector<T>(v.begin(), v.end());
}

opt::actions::Input actions_input(const ActionsInput& in) {
    opt::actions::Input a;
    a.n_units = in.n_units;
    a.n_features = in.n_features;
    a.n_threats = in.n_threats;
    a.monitoring_cost = std_of<double>(in.monitoring_cost);
    a.unit_status = std_of<double>(in.unit_status);
    a.target_recovery = std_of<double>(in.target_recovery);
    a.target_conservation = std_of<double>(in.target_conservation);
    a.df_unit = std_of<i64>(in.df_unit);
    a.df_feature = std_of<i64>(in.df_feature);
    a.df_amount = std_of<double>(in.df_amount);
    a.blm_actions = std_of<double>(in.blm_actions);
    a.act_unit = std_of<i64>(in.act_unit);
    a.act_threat = std_of<i64>(in.act_threat);
    a.act_amount = std_of<double>(in.act_amount);
    a.act_cost = std_of<double>(in.act_cost);
    a.act_status = std_of<double>(in.act_status);
    a.sens_feature = std_of<i64>(in.sens_feature);
    a.sens_threat = std_of<i64>(in.sens_threat);
    a.sens_delta = std_of<double>(in.sens_delta);
    a.has_boundary = in.has_boundary;
    if (a.has_boundary) {
        a.bnd_id1 = std_of<i64>(in.bnd_id1);
        a.bnd_id2 = std_of<i64>(in.bnd_id2);
        a.bnd_value = std_of<double>(in.bnd_value);
    }
    a.check();
    return a;
}

opt::actions::Settings actions_settings(const ActionsSettings& s) {
    opt::actions::Settings o;
    o.maximize = s.maximize;
    o.blm = s.blm;
    o.curve = s.curve;
    o.segments = s.segments;
    o.step = s.step;
    o.budget = s.budget;
    return o;
}

}  // namespace

ActionsModel actions_compile(const ActionsInput& input, const ActionsSettings& settings) {
    const opt::actions::Input in = actions_input(input);
    const opt::actions::Pairs p = opt::actions::pairs(in);
    opt::actions::Compiled c = opt::actions::compile(in, p, actions_settings(settings), false, false);
    ActionsModel out;
    out.start = to_rust<std::int64_t>(c.model.start);
    out.index = to_rust<std::int64_t>(c.model.index);
    out.value = to_rust<double>(c.model.value);
    out.obj = to_rust<double>(c.model.obj);
    out.lb = to_rust<double>(c.model.lb);
    out.ub = to_rust<double>(c.model.ub);
    out.rhs = to_rust<double>(c.rhs);
    out.sense = to_rust<std::int32_t>(c.sense);
    out.integer = to_rust<std::uint8_t>(c.model.integer);
    return out;
}

SolveOut actions_solve(const ActionsInput& input, const ActionsSettings& settings, double gap, double time_limit,
                       rust::Slice<const double> start) {
    const opt::actions::Input in = actions_input(input);
    const opt::actions::Pairs p = opt::actions::pairs(in);
    opt::SolveOptions o;
    o.gap = gap;
    o.time_limit = time_limit;
    o.threads = max_threads();
    o.start = to_std(start);
    opt::SolveResult r = opt::actions::solve(in, p, actions_settings(settings), o, true);
    SolveOut out;
    out.status = r.status;
    out.has_solution = r.has_solution;
    out.x = to_rust<double>(r.x);
    out.objective = r.objective;
    out.bound = r.bound;
    out.gap = r.gap;
    return out;
}

ActionsBenefit actions_benefit(const ActionsInput& input, rust::Slice<const double> solution) {
    const opt::actions::Input in = actions_input(input);
    const opt::actions::Pairs p = opt::actions::pairs(in);
    opt::actions::Benefit b = opt::actions::benefit(in, p, to_std(solution));
    ActionsBenefit out;
    out.dist = to_rust<double>(b.dist);
    out.dist_threatened = to_rust<double>(b.dist_threatened);
    out.conservation = to_rust<double>(b.conservation);
    out.recovery = to_rust<double>(b.recovery);
    return out;
}

rust::Vec<double> actions_costs(const ActionsInput& input, rust::Slice<const double> units,
                                rust::Slice<const double> actions) {
    return to_rust<double>(opt::actions::costs(actions_input(input), to_std(units), to_std(actions)));
}

rust::Vec<double> actions_connectivity(const ActionsInput& input, rust::Slice<const double> units,
                                       rust::Slice<const double> actions) {
    return to_rust<double>(opt::actions::connectivity(actions_input(input), to_std(units), to_std(actions)));
}

ActionsPairs actions_pairs(const ActionsInput& input) {
    const opt::actions::Pairs p = opt::actions::pairs(actions_input(input));
    ActionsPairs out;
    out.feature = to_rust<std::int64_t>(p.feature);
    out.unit = to_rust<std::int64_t>(p.unit);
    out.threatened = to_rust<std::uint8_t>(p.threatened);
    return out;
}

double eval_boundary_zones(rust::Slice<const double> edge_factor, rust::Slice<const double> zones,
                           rust::Slice<const std::int64_t> start, rust::Slice<const std::int64_t> index,
                           rust::Slice<const double> value, rust::Slice<const double> exposed,
                           rust::Slice<const double> total, rust::Slice<const double> solution) {
    const i64 n = static_cast<i64>(start.size()) - 1;
    const opt::eval::Csc lower = csc_of(start, index, value, n);
    for (const i64 r : lower.index) check(r >= 0 && r < n, "rows within the units");
    return opt::eval::boundary_zones(to_std(edge_factor), to_std(zones), lower, to_std(exposed), to_std(total),
                                     to_std(solution));
}

double eval_connectivity_zones(rust::Slice<const std::int64_t> start, rust::Slice<const std::int64_t> index,
                               rust::Slice<const double> value, rust::Slice<const double> zones,
                               rust::Slice<const double> solution, bool asym) {
    const i64 n = static_cast<i64>(start.size()) - 1;
    const opt::eval::Csc m = csc_of(start, index, value, n);
    for (const i64 r : m.index) check(r >= 0 && r < n, "rows within the units");
    return asym ? opt::eval::asym_connectivity_zones(m, to_std(zones), to_std(solution))
                : opt::eval::connectivity_zones(m, to_std(zones), to_std(solution));
}

CscOut eval_ferrier(rust::Slice<const std::int64_t> start, rust::Slice<const std::int64_t> index,
                    rust::Slice<const double> value, std::int64_t nrow, rust::Slice<const double> targets,
                    rust::Slice<const std::uint8_t> selected) {
    const opt::eval::Csc rij = csc_of(start, index, value, nrow);
    check(static_cast<i64>(targets.size()) == nrow, "a target per feature");
    check(static_cast<i64>(selected.size()) == rij.ncol, "a flag per unit");
    const opt::eval::Csc s = opt::eval::ferrier_score(rij, to_std(targets), to_std(selected));
    CscOut out;
    out.start = to_rust<std::int64_t>(s.start);
    out.index = to_rust<std::int64_t>(s.index);
    out.value = to_rust<double>(s.value);
    return out;
}

double eval_accumulate(rust::Slice<const double> x) {
    return opt::eval::accumulate(x.data(), static_cast<i64>(x.size()));
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

rust::Vec<std::uint8_t> lsm_rasterize(rust::Slice<const std::int64_t> sizes, rust::Slice<const double> xs,
                                      rust::Slice<const double> ys, rust::Slice<const double> gt, std::int32_t nx,
                                      std::int32_t ny) {
    check(xs.size() == ys.size() && gt.size() == 6 && nx > 0 && ny > 0, "rings, a geotransform and a grid");
    std::vector<std::vector<std::pair<double, double>>> rings;
    std::size_t at = 0;
    for (std::int64_t n : sizes) {
        check(n >= 0 && at + static_cast<std::size_t>(n) <= xs.size(), "the rings' sizes exceed their points");
        std::vector<std::pair<double, double>> ring;
        for (std::int64_t k = 0; k < n; ++k, ++at) ring.emplace_back(xs[at], ys[at]);
        rings.push_back(std::move(ring));
    }
    return to_rust<std::uint8_t>(lsm::rasterize_all_touched(rings, gt.data(), nx, ny));
}

rust::Vec<double> lsm_r_sincos(rust::Slice<const double> x) {
    rust::Vec<double> out;
    for (double v : x) out.push_back(lsm::r_sin(v));
    for (double v : x) out.push_back(lsm::r_cos(v));
    return out;
}

rust::Vec<double> lsm_r_cor(rust::Slice<const double> x, std::int64_t n, std::int64_t m, rust::Str method) {
    check(n >= 0 && m >= 0 && static_cast<i64>(x.size()) == n * m, "x differs in size from n x m");
    return to_rust<double>(lsm::r_cor(std::vector<double>(x.begin(), x.end()), n, m, std::string(method)));
}

ChangeOut fad_change_counts(rust::Slice<const std::uint8_t> a, rust::Slice<const std::uint8_t> b, std::int64_t ny,
                            std::int64_t nx) {
    const gtb::FADChangeCounts r = gtb::compute_fad_change_counts(image(a, ny, nx), image(b, ny, nx));
    ChangeOut out;
    for (i64 n : r.matrix) out.matrix.push_back(n);
    for (i64 n : {r.fg_area_a, r.fg_area_b, r.forcom, r.fg_sum_a, r.fg_sum_b, r.ru_area_a, r.ru_area_b})
        out.counts.push_back(n);
    return out;
}

InfluenceOut influence(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx, std::int64_t min_area,
                       std::int32_t core_buffer, std::int32_t buffer, bool proximity) {
    check(min_area >= 1 && core_buffer >= 0 && buffer >= 0, "min_area >= 1, core_buffer and buffer >= 0");
    const gtb::InfluenceResult r = gtb::compute_influence(image(input, ny, nx), min_area, core_buffer, buffer,
                                                          proximity, gtb::DistanceMetric::QuasiEuclidean);
    InfluenceOut out;
    out.classes = pixels(r.classes);
    for (i64 k = 0; k < r.ids.size(); ++k) out.ids.push_back(r.ids[k]);
    for (i64 k = 0; k < r.zones.size(); ++k) out.zones.push_back(r.zones[k]);
    for (i64 k = 0; k < r.distance.size(); ++k) out.distance.push_back(r.distance[k]);
    for (i64 k = 0; k < r.cag.size(); ++k) out.cag.push_back(r.cag[k]);
    out.object_area = to_rust<std::int64_t>(r.object_area);
    out.n_objects = r.n_objects;
    for (const auto& l : r.lines) {
        for (i64 v : {l.x, l.y, l.nbr, l.clength, l.cag, static_cast<i64>(l.ids.size())}) out.lines.push_back(v);
        for (std::int32_t id : l.ids) out.lines.push_back(id);
        for (i64 a : l.areas) out.lines.push_back(a);
    }
    out.prox_min = r.prox_min;
    out.prox_min_count = r.prox_min_count;
    return out;
}

rust::Vec<double> restoration_status(rust::Slice<const std::uint8_t> input, std::int64_t ny, std::int64_t nx) {
    const gtb::RestorationStatusResult r = gtb::compute_restoration_status(image(input, ny, nx));
    rust::Vec<double> out;
    for (double v : {double(r.data_area), double(r.nr_comp), double(r.eca_max), double(r.larg_obj), r.aps, r.cnoa,
                     r.eca, r.doc, r.rest_pot, r.rac})
        out.push_back(v);
    return out;
}

rust::Vec<float> cost_distance(rust::Slice<const std::uint8_t> resistance, rust::Slice<const std::uint8_t> sources,
                               std::int64_t ny, std::int64_t nx) {
    return pixels(gtb::compute_cost_distance(image(resistance, ny, nx), image(sources, ny, nx)));
}

LcpOut least_cost_path(rust::Slice<const std::uint8_t> resistance, rust::Slice<const std::uint8_t> a,
                       rust::Slice<const std::uint8_t> b, rust::Slice<const float> distance, std::int64_t ny,
                       std::int64_t nx) {
    std::unique_ptr<gtb::Image<float>> d;
    if (!distance.empty()) {
        check(static_cast<i64>(distance.size()) == ny * nx, "the distance map differs from ny x nx");
        d = std::make_unique<gtb::Image<float>>(nx, ny);
        std::copy(distance.begin(), distance.end(), d->data());
    }
    gtb::LeastCostPathResult r =
        gtb::compute_least_cost_path(image(resistance, ny, nx), image(a, ny, nx), image(b, ny, nx), d.get());
    LcpOut out;
    out.cost = pixels(r.combined_cost);
    out.path = pixels(r.lcp_mask);
    out.path_length = r.path_length;
    out.rest_pixels = r.rest_pixels;
    out.expense = r.expense;
    out.avg_distance = r.avg_distance;
    return out;
}

rust::Vec<std::uint32_t> geodesic_time(rust::Slice<const std::uint8_t> mask, rust::Slice<const std::uint8_t> reference,
                                       std::int64_t ny, std::int64_t nx) {
    return pixels(gtb::geodesic_time(image(mask, ny, nx), image(reference, ny, nx)));
}

rust::Vec<std::uint8_t> thin_anchored(rust::Slice<const std::uint8_t> input, std::int32_t stype,
                                      rust::Slice<const std::uint8_t> anchor, std::int64_t ny, std::int64_t nx) {
    gtb::ImageU8 im = image(input, ny, nx);
    gtb::ImageU8 an = image(anchor, ny, nx);
    gtb::thin_anchored(im, stype, an);
    return pixels(im);
}

rust::Vec<float> morph_distance(rust::Slice<const std::uint8_t> mask, std::int64_t ny, std::int64_t nx, bool background) {
    return to_rust<float>(gtb::morph_distance(image(mask, ny, nx), background, gtb::DistanceMetric::QuasiEuclidean));
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

// simulate bridge (landschaft-bb)

rust::Vec<double> cost_accumulated(rust::Slice<const double> values, std::int64_t ny, std::int64_t nx, double xmin,
                                   double xmax, double ymin, double ymax, bool wrap, std::int32_t directions,
                                   bool symmetric, rust::Slice<const std::int64_t> sources) {
    check(directions == 4 || directions == 8 || directions == 16, "directions must be 4, 8 or 16");
    check(ny > 0 && nx > 0, "the grid is empty");
    cost::Grid g;
    g.ny = ny;
    g.nx = nx;
    g.xmin = xmin;
    g.xmax = xmax;
    g.ymin = ymin;
    g.ymax = ymax;
    g.wrap = wrap;
    const auto n_dirs = static_cast<std::int64_t>(cost::offsets(directions, symmetric).size());
    check(static_cast<std::int64_t>(values.size()) == n_dirs * g.ncell(),
          "the transition's values do not match the grid and directions");
    std::vector<std::int64_t> src(sources.begin(), sources.end());
    for (const auto c : src) check(c >= 0 && c < g.ncell(), "a source cell is outside the grid");
    const cost::Layout t{&g, directions, symmetric, values.data()};
    return to_rust<double>(cost::accumulated_cost(t, src));
}

std::unique_ptr<sim::NumpyRandomState> numpy_rng_new(std::uint32_t seed) {
    return std::make_unique<sim::NumpyRandomState>(seed);
}

rust::Vec<double> numpy_rng_random_sample(sim::NumpyRandomState& rng, std::size_t n) {
    rust::Vec<double> out;
    out.reserve(n);
    for (std::size_t i = 0; i < n; ++i) out.push_back(rng.random_sample());
    return out;
}

rust::Vec<double> molusce_categories(rust::Slice<const double> values, rust::Slice<const double> bounds) {
    std::vector<double> b(bounds.begin(), bounds.end());
    check(std::is_sorted(b.begin(), b.end()), "the bounds must be sorted");
    return to_rust<double>(sim::molusce_categories(values.data(), static_cast<std::int64_t>(values.size()), b));
}

rust::Vec<double> molusce_total(rust::Slice<const double> factors, std::int32_t nf, std::int64_t n,
                                rust::Slice<const MolusceFactor> weights) {
    check(nf > 0 && n >= 0 && static_cast<std::int64_t>(factors.size()) == std::int64_t{nf} * n,
          "the factors must be factors x cells");
    check(weights.size() % static_cast<std::size_t>(nf) == 0, "the weights must be codes x factors");
    std::vector<std::vector<sim::MolusceFactorWeights>> w(weights.size() / static_cast<std::size_t>(nf));
    for (std::size_t k = 0; k < weights.size(); ++k) {
        const auto& x = weights[k];
        check(x.categories.size() == x.weights.size(), "one weight per category");
        w[k / static_cast<std::size_t>(nf)].push_back(
            {std::vector<double>(x.categories.begin(), x.categories.end()),
             std::vector<double>(x.weights.begin(), x.weights.end())});
    }
    return to_rust<double>(sim::molusce_total(factors.data(), nf, n, w));
}

MolusceForecastOut molusce_prediction_woe(rust::Slice<const double> state, rust::Slice<const double> classes,
                                          rust::Slice<const std::int64_t> codes, rust::Slice<const double> totals) {
    const auto n = static_cast<std::int64_t>(state.size());
    check(static_cast<std::int64_t>(totals.size()) == static_cast<std::int64_t>(codes.size()) * n,
          "the totals must be codes x cells");
    const auto f = sim::molusce_prediction_woe(state.data(), n, std::vector<double>(classes.begin(), classes.end()),
                                               std::vector<std::int64_t>(codes.begin(), codes.end()), totals.data());
    return MolusceForecastOut{to_rust<double>(f.prediction), to_rust<double>(f.confidence)};
}

MolusceForecastOut molusce_prediction_classes(rust::Slice<const double> state, rust::Slice<const double> layers,
                                              rust::Slice<const double> classes) {
    const auto n = static_cast<std::int64_t>(state.size());
    const auto m = static_cast<int>(classes.size());
    check(static_cast<std::int64_t>(layers.size()) == std::int64_t{m} * n, "the layers must be classes x cells");
    const auto f = sim::molusce_prediction_classes(state.data(), n, m, layers.data(),
                                                   std::vector<double>(classes.begin(), classes.end()));
    return MolusceForecastOut{to_rust<double>(f.prediction), to_rust<double>(f.confidence)};
}

MolusceStepOut molusce_ca_step(rust::Slice<const double> state, rust::Slice<const double> prediction,
                               rust::Slice<const double> confidence, rust::Slice<const MolusceChange> counts,
                               sim::NumpyRandomState& rng) {
    check(prediction.size() == state.size() && confidence.size() == state.size(),
          "the prediction and confidence differ in size from the state");
    std::vector<sim::MolusceTransition> c;
    for (const auto& x : counts) c.push_back({x.from, x.to, x.cells});
    const auto r = sim::molusce_ca_step(state.data(), static_cast<std::int64_t>(state.size()), prediction.data(),
                                        confidence.data(), c, rng);
    MolusceStepOut out{to_rust<double>(r.state), {}};
    for (const auto& x : r.changed) out.changed.push_back(MolusceChange{x.from, x.to, x.cells});
    return out;
}

ClumondoYearOut clumondo_year(const ClumondoSettingsIn& in, rust::Slice<const std::int32_t> oldco,
                              rust::Slice<const std::int32_t> region, rust::Slice<const std::int32_t> age,
                              rust::Slice<const float> suit, rust::Slice<const float> allow_maps,
                              rust::Slice<const double> demand, rust::Slice<const float> dem_elas) {
    const auto ncell = static_cast<std::int64_t>(oldco.size());
    const int nsys = in.nsys, ndem = in.ndem;
    const auto ns = static_cast<std::size_t>(nsys), nd = static_cast<std::size_t>(ndem);
    check(nsys > 0 && ndem > 0, "no land systems or no demands");
    check(region.size() == oldco.size() && age.size() == oldco.size(), "region and age must match the map");
    check(static_cast<std::int64_t>(suit.size()) == std::int64_t{nsys} * ncell, "suit must be systems x cells");
    check(in.allowed.size() == ns * ns, "allowed must be systems x systems");
    check(in.lusconv.size() == ns * nd && in.lusmatrix.size() == ns * nd, "lusconv and lusmatrix: systems x demands");
    check(in.weight.size() == nd && in.resistance.size() == ns, "a weight per demand and a resistance per system");
    check(demand.size() == nd && dem_elas.size() == nd, "one demand per demand type");
    check(ncell == 0 || allow_maps.size() % static_cast<std::size_t>(ncell) == 0, "allow maps: maps x cells");
    sim::ClumondoSettings st;
    st.nsys = nsys;
    st.ndem = ndem;
    st.allowed.assign(in.allowed.begin(), in.allowed.end());
    st.lusconv.assign(in.lusconv.begin(), in.lusconv.end());
    st.lusmatrix.assign(in.lusmatrix.begin(), in.lusmatrix.end());
    st.weight.assign(in.weight.begin(), in.weight.end());
    st.resistance.assign(in.resistance.begin(), in.resistance.end());
    st.itermode = in.itermode;
    st.mean_dev = in.mean_dev;
    st.max_dev = in.max_dev;
    st.speed = in.speed;
    st.max_loops = in.max_loops;
    if (!in.kind.empty()) {
        check(in.kind.size() == nd, "one demand type per demand");
        st.kind.assign(in.kind.begin(), in.kind.end());
    }
    if (!in.dev_limit.empty()) {
        check(in.dev_limit.size() == nd, "one deviation per demand");
        st.dev_limit.assign(in.dev_limit.begin(), in.dev_limit.end());
    }
    st.nx = in.nx;
    if (!in.patch_radius.empty()) {
        check(in.patch_radius.size() == ns && in.patch_threshold.size() == ns && in.patch_min.size() == ns,
              "patch settings: one per land system");
        st.patch_radius.assign(in.patch_radius.begin(), in.patch_radius.end());
        st.patch_threshold.assign(in.patch_threshold.begin(), in.patch_threshold.end());
        st.patch_min.assign(in.patch_min.begin(), in.patch_min.end());
    }
    if (!in.pair_resistance.empty()) {
        check(in.pair_resistance.size() == ns * ns, "pair_resistance must be systems x systems");
        st.pair_resistance.assign(in.pair_resistance.begin(), in.pair_resistance.end());
    }
    check(in.cap_pairs.size() == in.cap_cells.size() * ns * ns, "cap_pairs must be caps x systems x systems");
    for (std::size_t k = 0; k < in.cap_cells.size(); ++k) {
        st.cap_pairs.emplace_back(in.cap_pairs.begin() + static_cast<std::ptrdiff_t>(k * ns * ns),
                                  in.cap_pairs.begin() + static_cast<std::ptrdiff_t>((k + 1) * ns * ns));
        st.cap_cells.push_back(in.cap_cells[k]);
    }
    std::vector<float> elas(dem_elas.begin(), dem_elas.end());
    const int nmap = ncell ? static_cast<int>(allow_maps.size() / static_cast<std::size_t>(ncell)) : 0;
    const auto r = sim::clumondo_year(st, ncell, oldco.data(), region.data(), age.data(), suit.data(),
                                      allow_maps.data(), nmap, demand.data(), elas.data());
    return ClumondoYearOut{to_rust<std::int32_t>(r.newco), to_rust<std::int32_t>(r.age), to_rust<double>(r.supply),
                           to_rust<std::int64_t>(r.cells), r.loops, r.converged, r.max_diff, r.mean_diff,
                           to_rust<float>(elas)};
}

rust::Vec<float> clumondo_logit(rust::Slice<const std::int32_t> region, rust::Slice<const float> x, double constant,
                                rust::Slice<const float> coef) {
    const auto ncell = static_cast<std::int64_t>(region.size());
    check(static_cast<std::int64_t>(x.size()) == static_cast<std::int64_t>(coef.size()) * ncell,
          "x must be drivers x cells");
    return to_rust<float>(sim::clumondo_logit(ncell, region.data(), x.data(), static_cast<int>(coef.size()),
                                              constant, coef.data()));
}

rust::Vec<float> clumondo_add_neighbourhood(rust::Slice<const float> suit, std::int64_t ny, std::int64_t nx,
                                            rust::Slice<const std::int32_t> oldco,
                                            rust::Slice<const std::int32_t> region,
                                            rust::Slice<const ClumondoNbIn> parts) {
    const std::int64_t ncell = ny * nx;
    check(static_cast<std::int64_t>(oldco.size()) == ncell && region.size() == oldco.size(),
          "oldco and region must be ny x nx");
    check(ncell > 0 && static_cast<std::int64_t>(suit.size()) % ncell == 0, "suit must be systems x cells");
    const int nsys = static_cast<int>(static_cast<std::int64_t>(suit.size()) / ncell);
    std::vector<sim::ClumondoNeighbourhood> nb;
    for (const auto& p : parts) {
        check(p.coef_systems.size() == p.coef_values.size(), "a coefficient per system");
        sim::ClumondoNeighbourhood x;
        x.system = p.system;
        x.weight = p.weight;
        x.kernel.assign(p.kernel.begin(), p.kernel.end());
        x.radius = p.radius;
        x.from.assign(p.from.begin(), p.from.end());
        x.constant = p.constant;
        for (std::size_t k = 0; k < p.coef_systems.size(); ++k) x.coefficients.emplace_back(p.coef_systems[k], p.coef_values[k]);
        nb.push_back(std::move(x));
    }
    std::vector<float> out(suit.begin(), suit.end());
    sim::clumondo_add_neighbourhood(out.data(), nsys, ny, nx, oldco.data(), region.data(), nb);
    return to_rust<float>(out);
}

rust::Vec<float> clumondo_add_preference(rust::Slice<const float> row, rust::Slice<const std::int32_t> region,
                                         rust::Slice<const float> values, float weight) {
    check(region.size() == row.size() && values.size() == row.size(), "the preference differs in size from the map");
    std::vector<float> out(row.begin(), row.end());
    sim::clumondo_add_preference(out.data(), static_cast<std::int64_t>(out.size()), region.data(), values.data(),
                                 weight);
    return to_rust<float>(out);
}

rust::Vec<std::int32_t> clumondo_random_age(std::int64_t ncell, std::int32_t max_age, std::uint32_t seed) {
    return to_rust<std::int32_t>(sim::clumondo_random_age(ncell, max_age, seed));
}

namespace {

template <typename T, typename U>
std::vector<T> to_std(const rust::Vec<U>& v) {
    return std::vector<T>(v.begin(), v.end());
}

// k maps of n cells from a flat vector
std::vector<std::vector<float>> split_maps(const rust::Vec<float>& v, std::size_t n) {
    std::vector<std::vector<float>> out;
    for (std::size_t i = 0; n && i + n <= v.size(); i += n) out.emplace_back(v.begin() + i, v.begin() + i + n);
    return out;
}

}  // namespace

FuturesOut futures_run(const FuturesIn& in, const FuturesClimateIn& c, rust::Slice<const std::int32_t> developed,
                       rust::Slice<const std::int32_t> region, rust::Slice<const std::int32_t> pot_region,
                       rust::Slice<const float> devpressure, rust::Slice<const float> aggregated,
                       rust::Slice<const float> weight, rust::Slice<const std::int32_t> zone) {
    const std::int64_t n = in.rows * in.cols;
    const auto un = static_cast<std::size_t>(n);
    check(n > 0 && developed.size() == un && region.size() == un && devpressure.size() == un &&
              aggregated.size() == un,
          "the maps differ in size from rows x cols");
    check(pot_region.empty() || pot_region.size() == un, "the potential subregions differ in size");
    check(weight.empty() || weight.size() == un, "the weight differs in size");
    check(zone.empty() || zone.size() == un, "the zoning differs in size");
    check(!in.patch_offsets.empty() && in.patch_offsets.back() == static_cast<std::int32_t>(in.patch_sizes.size()),
          "the patch offsets must end at the number of patch sizes");
    sim::FuturesSettings s;
    s.rows = in.rows;
    s.cols = in.cols;
    s.nregions = in.nregions;
    s.npot = in.npot;
    s.intercept = to_std<double>(in.intercept);
    s.devpressure_coef = to_std<double>(in.devpressure_coef);
    s.neighborhood = in.neighborhood;
    s.approach = in.approach;
    s.gamma = in.gamma;
    s.scaling = in.scaling;
    for (std::size_t k = 0; k + 1 < in.patch_offsets.size(); ++k)
        s.patch_sizes.emplace_back(in.patch_sizes.begin() + in.patch_offsets[k],
                                   in.patch_sizes.begin() + in.patch_offsets[k + 1]);
    s.single_column = in.single_column;
    s.max_patch_size = 1;
    for (const auto& v : s.patch_sizes)
        for (int x : v) s.max_patch_size = std::max(s.max_patch_size, x);
    s.compactness_mean = in.compactness_mean;
    s.compactness_range = in.compactness_range;
    s.num_neighbors = in.num_neighbors;
    s.seed_probability = in.seed_probability;
    s.incentive = to_std<float>(in.incentive);
    s.use_zone = !zone.empty();
    s.user_effects = in.user_effects;
    s.zone_ids = to_std<int>(in.zone_ids);
    s.zone_regions = to_std<int>(in.zone_regions);
    s.zone_effects = to_std<float>(in.zone_effects);
    s.stringency = to_std<float>(in.stringency);
    s.use_weight = !weight.empty();
    s.demand = to_std<float>(in.demand);
    s.nsteps = in.nsteps;
    s.seed = in.seed;
    sim::FuturesClimate cl;
    if (c.present) {
        cl.depth_mode = c.depth_mode;
        cl.huc = to_std<int>(c.huc);
        cl.nhuc = c.nhuc;
        cl.huc_ids = to_std<int>(c.huc_ids);
        cl.flood_steps = to_std<int>(c.flood_steps);
        if (cl.depth_mode) {
            cl.probabilities = to_std<float>(c.probabilities);
            const std::size_t per = cl.probabilities.size() * un;
            check(per > 0 && c.depths.size() == per * cl.flood_steps.size(), "depths: steps x probabilities x cells");
            for (std::size_t k = 0; k < cl.flood_steps.size(); ++k) {
                rust::Vec<float> part;
                for (std::size_t i = k * per; i < (k + 1) * per; ++i) part.push_back(c.depths[i]);
                cl.depths.push_back(split_maps(part, un));
            }
        } else {
            cl.flood_probability = split_maps(c.flood_probability, un);
            cl.hand = to_std<float>(c.hand);
            cl.percentile = c.percentile;
        }
        cl.adaptive_capacity = to_std<float>(c.adaptive_capacity);
        cl.adaptation = to_std<int>(c.adaptation);
        cl.levels = to_std<double>(c.levels);
        const std::size_t nl = cl.levels.size();
        check(nl > 0 && c.damage.size() % nl == 0, "damage: regions x levels");
        for (std::size_t i = 0; i < c.damage.size(); i += nl)
            cl.damage.emplace_back(c.damage.begin() + i, c.damage.begin() + i + nl);
        cl.ddf_region = to_std<int>(c.ddf_region);
        check(c.response.size() == 4, "the response is four numbers");
        cl.vulnerability_a = c.response[0];
        cl.vulnerability_b = c.response[1];
        cl.resilience_a = c.response[2];
        cl.resilience_b = c.response[3];
        cl.stddev = c.stddev;
        cl.has_matrix = c.has_matrix;
        if (cl.has_matrix) {
            cl.from_ids = to_std<int>(c.from_ids);
            cl.to_ids = to_std<int>(c.to_ids);
            const std::size_t nt = cl.to_ids.size();
            check(c.matrix.size() == cl.from_ids.size() * nt, "the redistribution matrix: from x to");
            for (std::size_t i = 0; i < cl.from_ids.size(); ++i)
                cl.matrix.emplace_back(c.matrix.begin() + i * nt, c.matrix.begin() + (i + 1) * nt);
            cl.population = to_std<float>(c.population);
        }
        cl.region_ids = to_std<int>(c.region_ids);
        check(cl.huc.size() == un && cl.ddf_region.size() == un && cl.adaptive_capacity.size() == un,
              "the climate maps differ in size");
        s.climate = &cl;
    }
    const auto r = sim::futures_run(s, developed.data(), region.data(), pot_region.empty() ? nullptr : pot_region.data(),
                                    devpressure.data(), aggregated.data(), weight.empty() ? nullptr : weight.data(),
                                    zone.empty() ? nullptr : zone.data());
    FuturesOut out;
    for (const auto& m : r.series)
        for (const int x : m) out.series.push_back(x);
    for (const auto& m : r.adaptation)
        for (const int x : m) out.adaptation.push_back(x);
    out.converted = to_rust<std::int64_t>(r.converted);
    out.overflow = to_rust<std::int64_t>(r.overflow);
    out.warnings = to_rust<std::int32_t>(r.warnings);
    out.flood_steps = to_rust<std::int32_t>(r.flood_steps);
    out.flood_hucs = to_rust<std::int32_t>(r.flood_hucs);
    out.flood_probabilities = to_rust<float>(r.flood_probabilities);
    for (const auto& m : r.moved)
        for (const float x : m) out.moved.push_back(x);
    out.demand = to_rust<float>(r.demand);
    return out;
}

}  // namespace tfx

// out-of-core landscapes (tiles)

namespace tfx {

namespace {

void check_block(const lsm::TiledLandscape& t, rust::Slice<const std::int32_t> values,
                 rust::Slice<const std::uint8_t> na, std::int64_t rows, std::int64_t above, std::int64_t below) {
    const i64 cells = (rows + above + below) * t.nx();
    check(rows >= 0 && above >= 0 && below >= 0 && static_cast<i64>(values.size()) == cells && na.size() == values.size(),
          "a block differs in size from its rows x the map's width, or has no NA mask");
}

}  // namespace

std::unique_ptr<lsm::TiledLandscape> lsm_tiled_new(std::int64_t nx, std::int64_t ny, double resx, double resy,
                                                   const LsmOptions& options,
                                                   rust::Slice<const std::int32_t> class_values) {
    check(nx > 0 && ny > 0 && resx > 0 && resy > 0, "a map of cells");
    return std::make_unique<lsm::TiledLandscape>(nx, ny, resx, resy, lsm_options(options),
                                                 std::vector<std::int32_t>(class_values.begin(), class_values.end()));
}

rust::Vec<std::int32_t> lsm_unique_values(rust::Slice<const std::int32_t> values, rust::Slice<const std::uint8_t> na) {
    return to_rust<std::int32_t>(
        lsm::unique_values(values.data(), na_mask(na, static_cast<i64>(values.size())), static_cast<i64>(values.size())));
}

void lsm_tiled_add(lsm::TiledLandscape& t, rust::Slice<const std::int32_t> values, rust::Slice<const std::uint8_t> na,
                   std::int64_t y0, std::int64_t rows, std::int64_t above, std::int64_t below) {
    check_block(t, values, na, rows, above, below);
    t.add_block(values.data(), na.data(), y0, rows, above, below);
}

void lsm_tiled_finish(lsm::TiledLandscape& t) { t.finish(); }

std::int64_t lsm_tiled_detail_halo(const lsm::TiledLandscape& t, bool core, bool enn) {
    return t.detail_halo(core, enn);
}

void lsm_tiled_begin_detail(lsm::TiledLandscape& t, bool core, bool gyrate, bool circle, bool enn) {
    t.begin_detail(core, gyrate, circle, enn);
}

void lsm_tiled_detail(lsm::TiledLandscape& t, rust::Slice<const std::int32_t> values,
                      rust::Slice<const std::uint8_t> na, std::int64_t y0, std::int64_t rows, std::int64_t above,
                      std::int64_t below) {
    check_block(t, values, na, rows, above, below);
    t.detail_block(values.data(), na.data(), y0, rows, above, below);
}

bool lsm_tiled_end_detail(lsm::TiledLandscape& t) { return t.end_detail(); }

void lsm_tiled_tie(lsm::TiledLandscape& t, rust::Slice<const std::int32_t> values, rust::Slice<const std::uint8_t> na,
                   std::int64_t y0, std::int64_t rows, std::int64_t above, std::int64_t below) {
    check_block(t, values, na, rows, above, below);
    t.tie_block(values.data(), na.data(), y0, rows, above, below);
}

void lsm_tiled_end_ties(lsm::TiledLandscape& t) { t.end_ties(); }

LsmTable lsm_tiled_table(lsm::TiledLandscape& t, rust::Slice<const std::int32_t> levels,
                         rust::Slice<const rust::String> names) {
    check(levels.size() == names.size(), "a level per metric");
    std::vector<lsm::MetricSpec> specs;
    for (std::size_t i = 0; i < levels.size(); ++i) {
        check(levels[i] >= 0 && levels[i] <= 2, "the level is 0 (patch), 1 (class) or 2 (landscape)");
        specs.push_back({static_cast<lsm::Level>(levels[i]), std::string(names[i])});
    }
    lsm::Landscape& l = t.landscape();
    lsm::LongTable table;
    lsm::append_long_table(l, specs, 0, table);
    LsmTable out;
    out.class_values = to_rust<std::int32_t>(l.class_values());
    out.metric = to_rust<std::int16_t>(table.metric);
    out.cls = to_rust<std::int32_t>(table.cls);
    out.cls_na = to_rust<std::uint8_t>(table.cls_na);
    out.id = to_rust<std::int32_t>(table.id);
    out.id_na = to_rust<std::uint8_t>(table.id_na);
    out.value = to_rust<double>(table.value);
    out.patch_class = to_rust<std::int32_t>(l.patches().cls);
    return out;
}

rust::Vec<std::int32_t> lsm_tiled_labels(const lsm::TiledLandscape& t, rust::Slice<const std::int32_t> values,
                                         rust::Slice<const std::uint8_t> na, std::int64_t y0, std::int64_t rows) {
    check_block(t, values, na, rows, 0, 0);
    return to_rust<std::int32_t>(t.block_labels(values.data(), na.data(), y0, rows, 0, 0));
}

OmniscapeOut circuit_omniscape(rust::Slice<const double> resistance, rust::Slice<const double> source, std::int64_t nx,
                               std::int64_t ny, std::int32_t radius, std::int32_t buffer, std::int32_t block_size,
                               bool four_neighbors, bool resistance_is_conductance, bool correct_artifacts,
                               bool flow_potential, rust::Slice<const double> condition1,
                               rust::Slice<const double> condition1_future, rust::Slice<const double> condition2,
                               rust::Slice<const double> condition2_future, rust::Slice<const double> comparisons,
                               double memory_limit) {
    const auto n = static_cast<std::size_t>(nx * ny);
    check(nx > 0 && ny > 0 && resistance.size() == n && source.size() == n,
          "the resistance and the source differ in size from nx x ny");
    for (auto c : {condition1, condition1_future, condition2, condition2_future})
        check(c.empty() || c.size() == n, "a condition differs in size from the resistance");
    check(comparisons.size() == 6, "two comparisons: equal, lower, upper");
    check(radius >= 0 && buffer >= 0 && block_size >= 1 && block_size % 2 == 1,
          "radius and buffer >= 0, block_size odd and >= 1");
    circuit::OmniscapeParams p;
    p.radius = radius;
    p.buffer = buffer;
    p.block_size = block_size;
    p.four_neighbors = four_neighbors;
    p.resistance_is_conductance = resistance_is_conductance;
    p.correct_artifacts = correct_artifacts;
    p.flow_potential = flow_potential;
    p.memory_limit = memory_limit;
    auto condition = [&](rust::Slice<const double> present, rust::Slice<const double> future, std::size_t k) {
        circuit::OmniscapeCondition c;
        if (present.empty()) return c;
        c.present = present.data();
        c.future = future.empty() ? nullptr : future.data();
        c.equal = comparisons[3 * k] != 0.0;
        c.lower = comparisons[3 * k + 1];
        c.upper = comparisons[3 * k + 2];
        return c;
    };
    p.condition1 = condition(condition1, condition1_future, 0);
    p.condition2 = condition(condition2, condition2_future, 1);
    int threads = 1;
#if defined(_OPENMP)
    threads = omp_get_max_threads();
#endif
    circuit::OmniscapeResult r = circuit::omniscape(resistance.data(), source.data(), nx, ny, p, threads);
    OmniscapeOut out;
    out.current = to_rust<double>(std::move(r.current));
    out.flow_potential = to_rust<double>(std::move(r.flow_potential));
    out.n_targets = r.n_targets;
    out.threads = r.threads;
    return out;
}

CircuitOut circuit_advanced(rust::Slice<const double> conductance, rust::Slice<const double> source,
                            rust::Slice<const double> ground, std::int64_t nx, std::int64_t ny, bool four_neighbors) {
    const auto n = static_cast<std::size_t>(nx * ny);
    check(nx > 0 && ny > 0 && conductance.size() == n && source.size() == n && ground.size() == n,
          "the conductance, source and ground differ in size from nx x ny");
    int threads = 1;
#if defined(_OPENMP)
    threads = omp_get_max_threads();
#endif
    circuit::AdvancedResult r =
        circuit::advanced(conductance.data(), source.data(), ground.data(), nx, ny, four_neighbors, threads);
    CircuitOut out;
    out.current = to_rust<double>(std::move(r.current));
    out.voltage = to_rust<double>(std::move(r.voltage));
    return out;
}

PairwiseOut circuit_pairwise(rust::Slice<const double> gmap, rust::Slice<const std::int64_t> polymap, std::int64_t nx,
                             std::int64_t ny, rust::Slice<const std::int64_t> point_cells,
                             rust::Slice<const std::int64_t> region_ids, rust::Slice<const std::uint8_t> excluded,
                             bool avg_resistances, bool four_neighbors, rust::Slice<const std::uint8_t> flags) {
    const auto n = static_cast<std::size_t>(nx * ny);
    check(nx > 0 && ny > 0 && gmap.size() == n, "the habitat differs in size from nx x ny");
    check(polymap.empty() || polymap.size() == n, "the polygon raster must have the habitat's shape");
    check(flags.size() == 8, "eight options");
    check(region_ids.empty() || region_ids.size() == point_cells.size(), "an id per focal cell");
    const std::int64_t* pm = polymap.empty() ? nullptr : polymap.data();
    circuit::PairwiseOptions o;
    o.cum = flags[0] != 0;
    o.max = flags[1] != 0;
    o.focal_zero = flags[2] != 0;
    o.null_currents = flags[3] != 0;
    o.null_voltages = flags[4] != 0;
    o.log_transform = flags[5] != 0;
    o.volt_maps = flags[6] != 0;
    o.cur_maps = flags[7] != 0;
    std::vector<std::uint8_t> ex(excluded.begin(), excluded.end());
    std::mutex lock;
    std::vector<circuit::PairMaps> pairs;
    std::function<void(circuit::PairMaps&&)> ignore = [&](circuit::PairMaps&& pm) {
        std::lock_guard<std::mutex> guard(lock);
        pairs.push_back(std::move(pm));
    };
    int threads = 1;
#if defined(_OPENMP)
    threads = omp_get_max_threads();
#endif
    circuit::PairwiseResult r;
    if (!region_ids.empty()) {
        std::vector<circuit::i64> cells(point_cells.begin(), point_cells.end());
        std::vector<std::int64_t> ids(region_ids.begin(), region_ids.end());
        r = circuit::raster_pairwise_regions(gmap.data(), pm, nx, ny, avg_resistances, four_neighbors, cells, ids, ex, o,
                                             ignore, threads);
    } else {
        circuit::RasterGraph g = circuit::raster_graph(gmap.data(), pm, nx, ny, avg_resistances, four_neighbors);
        std::vector<std::int32_t> nodes(point_cells.size());
        for (std::size_t i = 0; i < point_cells.size(); ++i) {
            const std::int64_t c = point_cells[i];
            nodes[i] = c >= 0 && c < nx * ny ? g.nodemap[static_cast<std::size_t>(c)] : -1;
        }
        r = circuit::raster_pairwise(g, gmap.data(), nodes, ex, o, ignore, threads);
    }
    PairwiseOut out;
    out.resistances = to_rust<double>(std::move(r.resistances));
    out.cum = to_rust<double>(std::move(r.cum));
    out.max = to_rust<double>(std::move(r.max));
    std::sort(pairs.begin(), pairs.end(), [](const circuit::PairMaps& x, const circuit::PairMaps& y) {
        return std::pair(x.ci, x.cj) < std::pair(y.ci, y.cj);
    });
    for (const circuit::PairMaps& pm : pairs) {
        out.pair_ci.push_back(pm.ci);
        out.pair_cj.push_back(pm.cj);
        for (const double v : pm.voltage) out.pair_volt.push_back(v);
        for (const double v : pm.current) out.pair_cur.push_back(v);
    }
    return out;
}

CsAdvancedOut circuit_cs_advanced(rust::Slice<const double> gmap, rust::Slice<const std::int64_t> polymap, std::int64_t nx,
                                  std::int64_t ny, rust::Slice<const double> source, rust::Slice<const double> ground,
                                  bool avg_resistances, bool four_neighbors, std::int32_t policy) {
    const auto n = static_cast<std::size_t>(nx * ny);
    check(nx > 0 && ny > 0 && gmap.size() == n && source.size() == n && ground.size() == n,
          "expected habitat, source and ground rasters of the same shape");
    check(polymap.empty() || polymap.size() == n, "the polygon raster must have the habitat's shape");
    check(policy >= 0 && policy <= 3, "the policy is 0 to 3");
    const std::int64_t* pm = polymap.empty() ? nullptr : polymap.data();
    int threads = 1;
#if defined(_OPENMP)
    threads = omp_get_max_threads();
#endif
    const circuit::RasterGraph g = circuit::raster_graph(gmap.data(), pm, nx, ny, avg_resistances, four_neighbors);
    circuit::NodeSolution sol = circuit::raster_advanced(g, circuit::node_sums(g, source.data()),
                                                         circuit::node_sums(g, ground.data()),
                                                         static_cast<circuit::ConflictPolicy>(policy), threads);
    CsAdvancedOut out;
    out.nodemap = to_rust<std::int32_t>(g.nodemap);
    out.voltage = to_rust<double>(std::move(sol.voltage));
    out.current = to_rust<double>(std::move(sol.current));
    out.solved = sol.solved;
    return out;
}

namespace {
circuit::RasterGraph network_of(rust::Slice<const std::int32_t> a, rust::Slice<const std::int32_t> b,
                                rust::Slice<const double> w, std::int32_t m) {
    check(a.size() == b.size() && a.size() == w.size(), "edge arrays of different lengths");
    return circuit::network_graph(std::vector<std::int32_t>(a.begin(), a.end()), std::vector<std::int32_t>(b.begin(), b.end()),
                                  std::vector<double>(w.begin(), w.end()), m);
}

// the node of the adjacency list holding each entry
std::vector<std::int32_t> entry_owner(const circuit::RasterGraph& g) {
    std::vector<std::int32_t> owner(g.adj.size());
    for (std::int32_t c = 0; c < g.n_nodes; ++c)
        for (std::int64_t e = g.ptr[c]; e < g.ptr[c + 1]; ++e) owner[static_cast<std::size_t>(e)] = c;
    return owner;
}

void push_branches(const circuit::RasterGraph& g, const std::vector<std::int32_t>& owner,
                   const std::vector<std::pair<std::int64_t, double>>& branch, rust::Vec<std::int32_t>& r,
                   rust::Vec<std::int32_t>& c, rust::Vec<double>& v) {
    for (const auto& [e, val] : branch) {
        r.push_back(g.adj[static_cast<std::size_t>(e)]);
        c.push_back(owner[static_cast<std::size_t>(e)]);
        v.push_back(val);
    }
}

int network_threads() {
#if defined(_OPENMP)
    return omp_get_max_threads();
#else
    return 1;
#endif
}
}  // namespace

NetworkOut circuit_network_advanced(rust::Slice<const std::int32_t> a, rust::Slice<const std::int32_t> b,
                                    rust::Slice<const double> w, std::int32_t m, rust::Slice<const double> source,
                                    rust::Slice<const double> ground, std::int32_t policy) {
    check(static_cast<std::int32_t>(source.size()) == m && static_cast<std::int32_t>(ground.size()) == m,
          "sources and grounds must have one value per node");
    check(policy >= 0 && policy <= 3, "the policy is 0 to 3");
    const circuit::RasterGraph g = network_of(a, b, w, m);
    std::vector<double> src(source.begin(), source.end()), gnd(ground.begin(), ground.end());
    std::vector<double> finite(static_cast<std::size_t>(m), 0.0);
    bool any_finite = false;
    for (std::int32_t v = 0; v < m; ++v) {
        finite[v] = gnd[v] < std::numeric_limits<double>::infinity() ? gnd[v] : 0.0;
        any_finite = any_finite || finite[v] != 0;
    }
    circuit::NodeSolution sol = circuit::raster_advanced(g, std::move(src), std::move(gnd),
                                                         static_cast<circuit::ConflictPolicy>(policy), network_threads());
    std::vector<std::int32_t> all(static_cast<std::size_t>(m));
    for (std::int32_t v = 0; v < m; ++v) all[v] = v;
    std::vector<double> current(static_cast<std::size_t>(m), 0.0);
    std::vector<std::pair<std::int64_t, double>> branch;
    circuit::network_currents(g, all.data(), m, sol.voltage.data(), any_finite ? finite.data() : nullptr, current.data(),
                              branch);
    NetworkOut out;
    out.voltage = to_rust<double>(std::move(sol.voltage));
    out.current = to_rust<double>(std::move(current));
    push_branches(g, entry_owner(g), branch, out.branch_r, out.branch_c, out.branch_v);
    out.solved = sol.solved;
    out.n_components = g.n_comp;
    return out;
}

NetworkOut circuit_network_pairwise(rust::Slice<const std::int32_t> a, rust::Slice<const std::int32_t> b,
                                    rust::Slice<const double> w, std::int32_t m, rust::Slice<const std::int32_t> points,
                                    rust::Slice<const std::uint8_t> excluded, bool volt_maps, bool cur_maps, bool cum) {
    check(excluded.size() == points.size() * points.size(), "excluded: n x n");
    circuit::PairwiseOptions o;
    o.volt_maps = volt_maps;
    o.cur_maps = cur_maps;
    o.cum = cum;
    o.network = true;
    std::vector<std::uint8_t> ex(excluded.begin(), excluded.end());
    std::vector<std::int32_t> pts(points.begin(), points.end());
    for (std::int32_t& v : pts)
        if (v < 0 || v >= m) v = -1;
    const circuit::RasterGraph g = network_of(a, b, w, m);
    const std::vector<std::int32_t> owner = entry_owner(g);
    std::mutex lock;
    std::vector<circuit::PairMaps> pairs;
    std::function<void(circuit::PairMaps&&)> keep = [&](circuit::PairMaps&& pm) {
        std::lock_guard<std::mutex> guard(lock);
        pairs.push_back(std::move(pm));
    };
    circuit::PairwiseResult r = circuit::raster_pairwise(g, nullptr, pts, ex, o, keep, network_threads());
    std::sort(pairs.begin(), pairs.end(),
              [](const circuit::PairMaps& x, const circuit::PairMaps& y) { return std::pair(x.ci, x.cj) < std::pair(y.ci, y.cj); });
    NetworkOut out;
    out.resistances = to_rust<double>(std::move(r.resistances));
    out.cum = to_rust<double>(std::move(r.cum));
    if (!r.cum_branch.empty()) {
        std::vector<std::pair<std::int64_t, double>> cb;
        for (std::int32_t c = 0; c < g.n_nodes; ++c)
            for (std::int64_t e = g.ptr[c]; e < g.ptr[c + 1]; ++e)
                if (g.adj[static_cast<std::size_t>(e)] < c) cb.emplace_back(e, r.cum_branch[static_cast<std::size_t>(e)]);
        push_branches(g, owner, cb, out.cum_r, out.cum_c, out.cum_v);
    }
    out.n_components = g.n_comp;
    out.pair_node_off.push_back(0);
    out.pair_branch_off.push_back(0);
    for (const circuit::PairMaps& pm : pairs) {
        out.pair_ci.push_back(pm.ci);
        out.pair_cj.push_back(pm.cj);
        for (const std::int32_t v : pm.nodes) out.pair_nodes.push_back(v);
        for (const double v : pm.voltage) out.pair_voltage.push_back(v);
        for (const double v : pm.current) out.pair_current.push_back(v);
        out.pair_node_off.push_back(static_cast<std::int64_t>(out.pair_nodes.size()));
        push_branches(g, owner, pm.branch, out.pair_branch_r, out.pair_branch_c, out.pair_branch_v);
        out.pair_branch_off.push_back(static_cast<std::int64_t>(out.pair_branch_r.size()));
    }
    return out;
}

MultiOut circuit_multi_advanced(rust::Slice<const double> gmap, rust::Slice<const std::int64_t> polymap, std::int64_t nx,
                                std::int64_t ny, bool avg_resistances, bool four_neighbors,
                                rust::Slice<const std::int64_t> src_cells, rust::Slice<const double> src_vals,
                                rust::Slice<const std::int64_t> src_offsets, rust::Slice<const std::int64_t> gnd_cells,
                                rust::Slice<const double> gnd_vals, rust::Slice<const std::int64_t> gnd_offsets,
                                rust::Slice<const std::int64_t> checks, std::int32_t policy) {
    const auto n = static_cast<std::size_t>(nx * ny);
    check(nx > 0 && ny > 0 && gmap.size() == n, "the habitat differs in size from nx x ny");
    check(polymap.empty() || polymap.size() == n, "the polygon raster must have the habitat's shape");
    const std::size_t np = checks.size();
    check(src_offsets.size() == np + 1 && gnd_offsets.size() == np + 1, "offsets: a problem and one more");
    check(src_cells.size() == src_vals.size() && gnd_cells.size() == gnd_vals.size(), "a value per cell");
    check(policy >= 0 && policy <= 3, "the policy is 0 to 3");
    const std::int64_t* pm = polymap.empty() ? nullptr : polymap.data();
    const circuit::ConflictPolicy pol = static_cast<circuit::ConflictPolicy>(policy);
    int threads = 1;
#if defined(_OPENMP)
    threads = omp_get_max_threads();
#endif
    circuit::RasterGraph g = circuit::raster_graph(gmap.data(), pm, nx, ny, avg_resistances, four_neighbors);
    // per node sums in column-major cell order (as landschaft's binding)
    auto sums = [&](rust::Slice<const std::int64_t> cells, rust::Slice<const double> vals, std::int64_t a,
                    std::int64_t b) {
        std::vector<std::size_t> order;
        for (std::int64_t k = a; k < b; ++k) order.push_back(static_cast<std::size_t>(k));
        std::stable_sort(order.begin(), order.end(), [&](std::size_t x, std::size_t y) {
            const std::int64_t cx = cells[x], cy = cells[y];
            return std::make_pair(cx % nx, cx / nx) < std::make_pair(cy % nx, cy / nx);
        });
        std::vector<double> sv(static_cast<std::size_t>(g.n_nodes), 0.0);
        for (std::size_t k : order) {
            const std::int32_t v = g.nodemap[static_cast<std::size_t>(cells[k])];
            if (v >= 0 && vals[k] != 0) sv[static_cast<std::size_t>(v)] += vals[k];
        }
        return sv;
    };
    std::vector<std::vector<double>> srcs, gnds;
    std::vector<std::int32_t> only;
    for (std::size_t q = 0; q < np; ++q) {
        srcs.push_back(sums(src_cells, src_vals, src_offsets[q], src_offsets[q + 1]));
        gnds.push_back(sums(gnd_cells, gnd_vals, gnd_offsets[q], gnd_offsets[q + 1]));
        std::int32_t o = -1;
        if (checks[q] >= 0) {
            o = g.nodemap[static_cast<std::size_t>(checks[q])];
            if (o < 0) o = -2;
        }
        only.push_back(o);
    }
    std::vector<circuit::NodeSolution> sols = circuit::raster_advanced_many(g, srcs, gnds, only, pol, threads);
    MultiOut out;
    out.nodemap = to_rust<std::int32_t>(g.nodemap);
    out.node_offsets.push_back(0);
    out.active_offsets.push_back(0);
    for (auto& sol : sols) {
        for (double v : sol.voltage) out.voltage.push_back(v);
        for (double v : sol.current) out.current.push_back(v);
        out.node_offsets.push_back(static_cast<std::int64_t>(out.voltage.size()));
        out.solved.push_back(sol.solved ? 1 : 0);
        for (std::int32_t a : sol.active) out.active.push_back(a);
        out.active_offsets.push_back(static_cast<std::int64_t>(out.active.size()));
    }
    return out;
}

namespace {
surf::Grid surf_grid(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny, rust::Slice<const double> extent) {
    check(nx > 0 && ny > 0 && z.size() == static_cast<std::size_t>(nx) * ny, "the values differ in size from nx x ny");
    check(extent.size() == 4, "an extent: xmin, xmax, ymin, ymax");
    surf::Grid g;
    g.z = z.data();
    g.nx = nx;
    g.ny = ny;
    g.xmin = extent[0];
    g.xmax = extent[1];
    g.ymin = extent[2];
    g.ymax = extent[3];
    return g;
}
}  // namespace

rust::Vec<double> surf_metrics(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                               rust::Slice<const double> extent, bool lonlat, rust::Slice<const std::int32_t> ids) {
    const surf::Grid g = surf_grid(z, nx, ny, extent);
    const surf::i64 n = g.size();
    auto wants = [&](int lo, int hi) {
        for (std::int32_t id : ids)
            if (id >= lo && id <= hi) return true;
        return false;
    };
    for (std::int32_t id : ids) check(id >= 0 && id <= surf::kWSfd, "an unknown surface metric");
    std::vector<double> v(static_cast<std::size_t>(surf::kWSfd) + 1, std::nan(""));
    // as landschaft's groups: each computation once
    if (wants(surf::kWSa, surf::kWSmean)) {
        const surf::Amplitude a = surf::amplitude(z.data(), n);
        const double av[] = {a.sa, a.sq, a.ssk, a.sku, a.sv, a.sph, a.smean};
        for (int k = 0; k < 7; ++k) v[static_cast<std::size_t>(surf::kWSa + k)] = av[k];
    }
    if (wants(surf::kWSdq, surf::kWSdr)) {
        v[surf::kWSdq] = surf::sdq(g);
        v[surf::kWSdq6] = surf::sdq6(g);
        const double area = surf::surface_area(g), flat = surf::flat_area(g);
        v[surf::kWSdr] = ((area - flat) / flat) * 100;
    }
    if (wants(surf::kWSds, surf::kWS10z)) {
        v[surf::kWSds] = surf::sds(g);
        v[surf::kWSsc] = surf::ssc(g);
        v[surf::kWS10z] = surf::s10z(g);
    }
    if (wants(surf::kWSbi, surf::kWSvk)) {
        const double sq = surf::amplitude(z.data(), n, 2).sq;
        const surf::BearingParameters p = surf::bearing_parameters(surf::bearing_curve(z.data(), n), sq);
        v[surf::kWSbi] = p.sbi;
        v[surf::kWSci] = p.sci;
        v[surf::kWSvi] = p.svi;
        v[surf::kWSk] = p.sk;
        v[surf::kWSpk] = p.spk;
        v[surf::kWSvk] = p.svk;
    }
    if (wants(surf::kWSal, surf::kWSfd)) {
        const std::vector<double> l = surf::scl(g, lonlat, {0.2, 1 / std::exp(1.0)});
        v[surf::kWSal] = l[0];
        v[surf::kWStr] = l[0] / l[2];
        const std::vector<double> d = surf::texture_direction(g, lonlat);
        v[surf::kWStd] = d[0];
        v[surf::kWStdi] = d[1];
        const std::vector<double> w = surf::radial_wavelength(g, lonlat);
        v[surf::kWSrw] = w[0];
        v[surf::kWSrwi] = w[1];
        v[surf::kWShw] = w[2];
        v[surf::kWSfd] = surf::sfd(g);
    }
    rust::Vec<double> out;
    for (std::int32_t id : ids) out.push_back(v[static_cast<std::size_t>(id)]);
    return out;
}

rust::Vec<double> surf_window(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                              rust::Slice<const double> extent, bool lonlat, std::int32_t size, bool circle,
                              rust::Slice<const std::int32_t> ids) {
    const surf::Grid g = surf_grid(z, nx, ny, extent);
    check(size >= 3 && size % 2 == 1, "the window is an odd number of cells, 3 or more");
    std::vector<int> want(ids.begin(), ids.end());
    for (int id : want) check(id >= 0 && id <= surf::kWSfd, "an unknown surface metric");
    const std::vector<std::vector<double>> maps = surf::window_metrics(g, lonlat, size, circle, want);
    rust::Vec<double> out;
    for (const auto& m : maps)
        for (double x : m) out.push_back(x);
    return out;
}

namespace {
std::vector<char> chars(rust::Slice<const std::uint8_t> m) { return std::vector<char>(m.begin(), m.end()); }
}  // namespace

rust::Vec<double> surf_terrain(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                               rust::Slice<const double> extent, bool lonlat, std::int32_t neighbors, bool degrees,
                               bool aspect) {
    check(neighbors == 4 || neighbors == 8, "neighbors: 4 or 8");
    return to_rust<double>(surf::terrain(surf_grid(z, nx, ny, extent), lonlat, neighbors, degrees, aspect));
}

rust::Vec<double> surf_focal(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                             rust::Slice<const double> extent, std::int32_t wr, std::int32_t wc,
                             rust::Slice<const std::uint8_t> mask, std::int32_t stat) {
    check(stat >= 0 && stat <= 4 && mask.size() == static_cast<std::size_t>(wr) * wc, "a statistic 0-4 and a mask");
    return to_rust<double>(
        surf::focal(surf_grid(z, nx, ny, extent), wr, wc, chars(mask), static_cast<surf::FocalStat>(stat)));
}

rust::Vec<double> surf_tri(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                           rust::Slice<const double> extent, std::int32_t wr, std::int32_t wc) {
    return to_rust<double>(surf::tri(surf_grid(z, nx, ny, extent), wr, wc));
}

rust::Vec<double> surf_curvature(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                                 rust::Slice<const double> extent, std::int32_t kind) {
    check(kind >= 0 && kind <= 4, "a curvature 0-4");
    return to_rust<double>(surf::curvature(surf_grid(z, nx, ny, extent), kind));
}

rust::Vec<double> surf_tpi(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                           rust::Slice<const double> extent, std::int32_t wr, std::int32_t wc,
                           rust::Slice<const std::uint8_t> mask, bool normalize) {
    check(mask.size() == static_cast<std::size_t>(wr) * wc, "a mask of wr x wc");
    return to_rust<double>(surf::tpi(surf_grid(z, nx, ny, extent), wr, wc, chars(mask), normalize));
}

rust::Vec<double> surf_hsp(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                           rust::Slice<const double> extent, rust::Slice<const std::int32_t> rows,
                           rust::Slice<const std::int32_t> cols, rust::Slice<const std::uint8_t> masks,
                           bool normalize) {
    check(rows.size() == cols.size(), "a row and a column count per scale");
    std::vector<std::vector<char>> m;
    std::size_t at = 0;
    for (std::size_t k = 0; k < rows.size(); ++k) {
        const auto n = static_cast<std::size_t>(rows[k]) * cols[k];
        check(at + n <= masks.size(), "the masks of the scales");
        m.emplace_back(masks.begin() + at, masks.begin() + at + n);
        at += n;
    }
    return to_rust<double>(surf::hsp(surf_grid(z, nx, ny, extent), std::vector<int>(rows.begin(), rows.end()),
                                     std::vector<int>(cols.begin(), cols.end()), m, normalize));
}

PlaneOut surf_fit_plane(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                        rust::Slice<const double> extent, std::int32_t order) {
    check(order >= 0 && order <= 6, "an order 0-6");
    int ifail = 0;
    std::vector<double> v = surf::fit_plane(surf_grid(z, nx, ny, extent), order, &ifail);
    PlaneOut out;
    out.values = to_rust<double>(std::move(v));
    out.code = ifail;
    return out;
}

PlaneOut surf_best_fit_plane(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                             rust::Slice<const double> extent) {
    int order = 0;
    std::vector<double> v = surf::best_fit_plane(surf_grid(z, nx, ny, extent), &order);
    PlaneOut out;
    out.values = to_rust<double>(std::move(v));
    out.code = order;
    return out;
}

rust::Vec<std::int64_t> surf_peaks(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny,
                                   rust::Slice<const double> extent, bool valleys) {
    const surf::Grid g = surf_grid(z, nx, ny, extent);
    return to_rust<std::int64_t>(valleys ? surf::find_valleys(g) : surf::find_peaks(g));
}

BearingOut surf_bearing(rust::Slice<const double> z, rust::Slice<const double> probs) {
    const surf::Bearing b = surf::bearing_curve(z.data(), static_cast<surf::i64>(z.size()));
    BearingOut out;
    out.bearing = to_rust<double>(b.bearing);
    out.height = to_rust<double>(b.heights);
    for (double p : probs) out.heights.push_back(b.height(p));
    return out;
}

AacfOut surf_aacf(rust::Slice<const double> z, std::int32_t nx, std::int32_t ny, rust::Slice<const double> extent,
                  bool lonlat) {
    surf::Raster r = surf::aacf_image(surf_grid(z, nx, ny, extent), lonlat);
    AacfOut out;
    out.values = to_rust<double>(std::move(r.v));
    out.nx = r.nx;
    out.ny = r.ny;
    for (double v : {r.xmin, r.xmax, r.ymin, r.ymax}) out.extent.push_back(v);
    return out;
}

namespace {
cost::Grid cost_grid(rust::Slice<const double> grid) {
    check(grid.size() == 7, "a grid: ny, nx, xmin, xmax, ymin, ymax, wrap");
    cost::Grid g;
    g.ny = static_cast<std::int64_t>(grid[0]);
    g.nx = static_cast<std::int64_t>(grid[1]);
    g.xmin = grid[2];
    g.xmax = grid[3];
    g.ymin = grid[4];
    g.ymax = grid[5];
    g.wrap = grid[6] != 0.0;
    check(g.ny > 0 && g.nx > 0, "the grid is empty");
    return g;
}

void cost_check(const cost::Grid& g, std::int32_t directions, bool symmetric, std::size_t n) {
    check(directions == 4 || directions == 8 || directions == 16, "directions must be 4, 8 or 16");
    const auto nd = static_cast<std::int64_t>(cost::offsets(directions, symmetric).size());
    check(static_cast<std::int64_t>(n) == nd * g.ncell(), "the transition's values do not match the grid");
}

cost::Builtin cost_builtin_of(const std::string& name) {
    if (name == "mean") return cost::Builtin::Mean;
    if (name == "min") return cost::Builtin::Min;
    if (name == "max") return cost::Builtin::Max;
    if (name == "sum") return cost::Builtin::Sum;
    if (name == "from") return cost::Builtin::From;
    if (name == "to") return cost::Builtin::To;
    if (name == "diff") return cost::Builtin::Diff;
    if (name == "harmonic") return cost::Builtin::Harmonic;
    if (name == "geometric") return cost::Builtin::Geometric;
    if (name == "prod") return cost::Builtin::Product;
    throw std::invalid_argument("unknown transition function '" + name + "'");
}
}  // namespace

rust::Vec<std::int32_t> cost_offsets(std::int32_t directions, bool symmetric) {
    check(directions == 4 || directions == 8 || directions == 16, "directions must be 4, 8 or 16");
    rust::Vec<std::int32_t> out;
    for (const cost::Offset o : cost::offsets(directions, symmetric)) {
        out.push_back(o.dr);
        out.push_back(o.dc);
    }
    return out;
}

rust::Vec<double> cost_builtin(rust::Slice<const double> x, rust::Slice<const double> grid, std::int32_t directions,
                               bool symmetric, rust::Str function) {
    const cost::Grid g = cost_grid(grid);
    check(static_cast<std::int64_t>(x.size()) == g.ncell(), "the values do not match the grid");
    check(directions == 4 || directions == 8 || directions == 16, "directions must be 4, 8 or 16");
    const cost::Builtin fn = cost_builtin_of(std::string(function));
    const auto nd = static_cast<std::size_t>(cost::offsets(directions, symmetric).size());
    std::vector<double> out(nd * static_cast<std::size_t>(g.ncell()));
    cost::builtin_values(x.data(), g, directions, symmetric, fn, out.data());
    return to_rust<double>(std::move(out));
}

rust::Vec<double> cost_geo_correction(rust::Slice<const double> values, rust::Slice<const double> grid,
                                      std::int32_t directions, bool symmetric, bool lonlat, rust::Str kind,
                                      bool resistance, bool scale, bool factors) {
    const cost::Grid g = cost_grid(grid);
    cost_check(g, directions, symmetric, values.size());
    const std::string type(kind);
    check(type == "c" || type == "r", "type can only be c or r");
    std::vector<double> out(values.begin(), values.end());
    cost::geo_correction(g, directions, symmetric, lonlat, type[0], resistance, scale, factors, out.data());
    return to_rust<double>(std::move(out));
}

rust::Vec<double> cost_matrix(rust::Slice<const double> values, rust::Slice<const double> grid,
                              std::int32_t directions, bool symmetric, rust::Slice<const std::int64_t> from,
                              rust::Slice<const std::int64_t> to) {
    const cost::Grid g = cost_grid(grid);
    cost_check(g, directions, symmetric, values.size());
    const cost::Layout t{&g, directions, symmetric, values.data()};
    return to_rust<double>(cost::cost_matrix(t, std::vector<std::int64_t>(from.begin(), from.end()),
                                             std::vector<std::int64_t>(to.begin(), to.end())));
}

rust::Vec<std::int64_t> cost_neighbours(rust::Slice<const double> grid, std::int32_t directions, bool symmetric) {
    const cost::Grid g = cost_grid(grid);
    check(directions == 4 || directions == 8 || directions == 16, "directions must be 4, 8 or 16");
    const auto nd = static_cast<std::size_t>(cost::offsets(directions, symmetric).size());
    std::vector<std::int64_t> out(nd * static_cast<std::size_t>(g.ncell()));
    cost::neighbours(g, directions, symmetric, out.data());
    return to_rust<std::int64_t>(std::move(out));
}

rust::Vec<double> cost_margin_sums(rust::Slice<const double> values, rust::Slice<const double> grid,
                                   std::int32_t directions, bool symmetric, std::int32_t margin, bool count) {
    const cost::Grid g = cost_grid(grid);
    cost_check(g, directions, symmetric, values.size());
    std::vector<double> out(static_cast<std::size_t>(g.ncell()));
    cost::margin_sums(g, directions, symmetric, values.data(), margin, count, out.data());
    return to_rust<double>(std::move(out));
}

rust::Vec<double> cost_rmath(rust::Str name, rust::Slice<const double> x, double y) {
    const std::string n(name);
    const int f = n == "sqrt" ? 0 : n == "exp" ? 1 : n == "log" ? 2 : n == "pow" ? 3 : -1;
    check(f >= 0, "unknown function: sqrt, exp, log or pow");
    std::vector<double> out(x.size());
    const auto len = static_cast<std::int64_t>(x.size());
#pragma omp parallel for schedule(static)
    for (std::int64_t k = 0; k < len; ++k) {
        const double v = x[static_cast<std::size_t>(k)];
        switch (f) {
            case 0: out[k] = lsm::r_sqrt(v); break;
            case 1: out[k] = lsm::r_exp(v); break;
            case 2: out[k] = lsm::r_log(v); break;
            default: out[k] = lsm::r_pow(v, y); break;
        }
    }
    return to_rust<double>(std::move(out));
}

namespace {
std::vector<std::int32_t> walk_nodes(const cost::WalkGraph& wg, const cost::Grid& g,
                                     rust::Slice<const std::int64_t> cells, bool strict) {
    std::vector<std::int32_t> v;
    for (const std::int64_t c : cells) {
        const std::int32_t n = (c >= 0 && c < g.ncell()) ? wg.node_of[static_cast<std::size_t>(c)] : -1;
        check(!strict || n >= 0, "an origin or goal is not on the transition's graph");
        v.push_back(n);
    }
    return v;
}
}  // namespace

CommuteOut cost_commute(rust::Slice<const double> values, rust::Slice<const double> grid, std::int32_t directions,
                        bool symmetric, rust::Slice<const std::int64_t> cells) {
    const cost::Grid g = cost_grid(grid);
    cost_check(g, directions, symmetric, values.size());
    const cost::Layout t{&g, directions, symmetric, values.data()};
    const bool geometric = directions != 16 && !g.wrap;
    const cost::WalkGraph wg = cost::walk_graph(t);
    const std::vector<std::int32_t> nodes = walk_nodes(wg, g, cells, false);
    CommuteOut out;
    for (const std::int32_t n : nodes) out.part.push_back(n >= 0 ? wg.component[n] : -1);
    out.resistance = to_rust<double>(cost::effective_resistance(wg, nodes, geometric, g.nx));
    out.volume = wg.volume;
    for (const double v : wg.component_volume) out.part_volume.push_back(v);
    return out;
}

rust::Vec<double> cost_rw_passages(rust::Slice<const double> values, rust::Slice<const double> grid,
                                   std::int32_t directions, bool symmetric, rust::Slice<const std::int64_t> origins,
                                   rust::Slice<const std::int64_t> goals, bool quirk) {
    const cost::Grid g = cost_grid(grid);
    cost_check(g, directions, symmetric, values.size());
    const cost::Layout t{&g, directions, symmetric, values.data()};
    const bool geometric = directions != 16 && !g.wrap;
    const cost::WalkGraph wg = cost::walk_graph(t);
    return to_rust<double>(cost::random_walk_passages(wg, walk_nodes(wg, g, origins, true),
                                                      walk_nodes(wg, g, goals, true), quirk, geometric, g.nx,
                                                      g.ncell()));
}

rust::Vec<double> cost_rsp_passages(rust::Slice<const double> values, rust::Slice<const double> grid,
                                    std::int32_t directions, bool symmetric, rust::Slice<const std::int64_t> origins,
                                    rust::Slice<const std::int64_t> goals, double theta, bool net, bool edges) {
    const cost::Grid g = cost_grid(grid);
    cost_check(g, directions, symmetric, values.size());
    const cost::Layout t{&g, directions, symmetric, values.data()};
    cost::RspOptions o;
    o.theta = theta;
    o.net = net;
    o.solidify = true;
    return to_rust<double>(cost::rsp_passages(t, std::vector<std::int64_t>(origins.begin(), origins.end()),
                                              std::vector<std::int64_t>(goals.begin(), goals.end()), o, edges));
}

rust::Vec<double> cost_rsp_distances(rust::Slice<const double> values, rust::Slice<const double> grid,
                                     std::int32_t directions, bool symmetric, rust::Slice<const std::int64_t> from,
                                     rust::Slice<const std::int64_t> to, double theta, bool net, std::int32_t method) {
    const cost::Grid g = cost_grid(grid);
    cost_check(g, directions, symmetric, values.size());
    const cost::Layout t{&g, directions, symmetric, values.data()};
    cost::RspOptions o;
    o.theta = theta;
    o.net = net;
    o.method = method;
    o.solidify = false;
    return to_rust<double>(cost::rsp_distances(t, std::vector<std::int64_t>(from.begin(), from.end()),
                                               std::vector<std::int64_t>(to.begin(), to.end()), o));
}

EdgesOut cost_graph_edges(rust::Slice<const double> values, rust::Slice<const double> grid, std::int32_t directions,
                          bool symmetric) {
    const cost::Grid g = cost_grid(grid);
    cost_check(g, directions, symmetric, values.size());
    const cost::Layout t{&g, directions, symmetric, values.data()};
    cost::GraphEdges e = cost::rsp_edges(t);
    EdgesOut out;
    out.from = to_rust<std::int64_t>(e.from);
    out.to = to_rust<std::int64_t>(e.to);
    out.value = to_rust<double>(e.t);
    return out;
}

rust::Vec<double> cost_path_flows(rust::Slice<const double> values, rust::Slice<const double> grid,
                                  std::int32_t directions, bool symmetric, std::int64_t origin,
                                  rust::Slice<const std::int64_t> locs, double theta, bool quirk) {
    const cost::Grid g = cost_grid(grid);
    cost_check(g, directions, symmetric, values.size());
    const cost::Layout t{&g, directions, symmetric, values.data()};
    const std::vector<std::int64_t> cells(locs.begin(), locs.end());
    if (std::isnan(theta)) {  // random walk
        const cost::WalkGraph wg = cost::walk_graph(t);
        const std::int32_t o = origin >= 0 && origin < g.ncell() ? wg.node_of[static_cast<std::size_t>(origin)] : -1;
        check(o >= 0, "the origin is not on the transition's graph");
        std::vector<std::int32_t> nodes;
        for (const std::int64_t c : cells)
            nodes.push_back(c >= 0 && c < g.ncell() ? wg.node_of[static_cast<std::size_t>(c)] : -1);
        return to_rust<double>(cost::walk_flows(wg, o, nodes, quirk, directions != 16 && !g.wrap, g.nx));
    }
    return to_rust<double>(cost::rsp_flows(t, origin, cells, theta));
}

rust::Vec<double> cost_path_overlap(rust::Slice<const double> fa, std::int64_t na, rust::Slice<const double> fb,
                                    std::int64_t nb, std::int64_t ne, rust::Slice<const double> r, std::int64_t nr,
                                    rust::Slice<const std::int32_t> functions, bool upper) {
    check(static_cast<std::int64_t>(fa.size()) == na * ne && static_cast<std::int64_t>(fb.size()) == nb * ne &&
              static_cast<std::int64_t>(r.size()) == nr * ne,
          "flows (locations x connections) and weights (layers x connections)");
    const std::vector<int> fns(functions.begin(), functions.end());
    const auto nf = static_cast<std::int64_t>(fns.size());
    std::vector<double> out(static_cast<std::size_t>(nf * nr * na * nb), std::numeric_limits<double>::quiet_NaN());
    cost::path_overlap(fa.data(), na, fb.data(), nb, ne, r.data(), nr, fns, upper, out.data());
    return to_rust<double>(std::move(out));
}

rust::Vec<double> cost_r_cov(rust::Slice<const double> x, std::int64_t n, std::int64_t k) {
    // R's cov(x) (cov.c, cov_na_1, no missing values): column means in two passes, sums of products of
    // deviations, all in LDOUBLE (landschaft's cost_r_cov)
    check(static_cast<std::int64_t>(x.size()) == n * k, "x must be n x k");
    using E = lsm::Extended;
    const double* px = x.data();
    std::vector<double> mean(static_cast<std::size_t>(k));
    for (std::int64_t j = 0; j < k; ++j) {
        E s = 0.0;
        for (std::int64_t r = 0; r < n; ++r) s += px[r * k + j];
        E t = s / E(static_cast<double>(n));
        if (std::isfinite(static_cast<double>(t))) {
            s = 0.0;
            for (std::int64_t r = 0; r < n; ++r) s += E(px[r * k + j]) - t;
            t = t + s / E(static_cast<double>(n));
        }
        mean[j] = static_cast<double>(t);
    }
    std::vector<double> out(static_cast<std::size_t>(k * k));
    for (std::int64_t i = 0; i < k; ++i) {
        for (std::int64_t j = 0; j <= i; ++j) {
            E s = 0.0;
            const E xm = mean[i], ym = mean[j];
            for (std::int64_t r = 0; r < n; ++r) s += (E(px[r * k + i]) - xm) * (E(px[r * k + j]) - ym);
            out[i * k + j] = out[j * k + i] = static_cast<double>(s / E(static_cast<double>(n - 1)));
        }
    }
    return to_rust<double>(std::move(out));
}

double cost_r_mean(rust::Slice<const double> x) { return lsm::r_mean(x.data(), static_cast<std::int64_t>(x.size())); }

namespace {
template <class T, class R>
std::vector<T> std_vec(const R& v) {
    return std::vector<T>(v.begin(), v.end());
}

bio::Groups bio_groups(const BioGroups& in) {
    bio::Groups g;
    g.n = in.n;
    g.ndim = in.ndim;
    g.coords = std_vec<double>(in.coords);
    g.numeric = std_vec<std::uint8_t>(in.numeric);
    g.axis_str = std_vec<std::int32_t>(in.axis_str);
    g.name_str = std_vec<std::int32_t>(in.name_str);
    g.cell_sizes = std_vec<double>(in.cell_sizes);
    for (const auto& s : in.strings) g.strings.emplace_back(std::string(s));
    check(static_cast<std::int64_t>(g.coords.size()) == g.n * g.ndim, "coords must be n x ndim");
    return g;
}

bio::Csr bio_csr(const BioCsr& in) {
    bio::Csr c;
    c.indptr = std_vec<std::int64_t>(in.indptr);
    c.indices = std_vec<std::int32_t>(in.indices);
    return c;
}

bio::Program bio_program(const BioProgram& in, std::int64_t n_groups) {
    bio::Program p;
    const std::size_t n = in.ops.size();
    check(in.abc.size() == 3 * n && in.v.size() == n, "nodes: ops, abc (3 each) and v");
    p.nodes.resize(n);
    for (std::size_t i = 0; i < n; ++i) {
        bio::Node& node = p.nodes[i];
        node.op = static_cast<bio::Op>(in.ops[i]);
        node.a = in.abc[3 * i];
        node.b = in.abc[3 * i + 1];
        node.c = in.abc[3 * i + 2];
        node.v = in.v[i];
    }
    p.seq = std_vec<std::int32_t>(in.seq);
    p.root = in.root;
    p.n_locals = in.n_locals;
    for (const BioCall& c : in.calls) {
        bio::Call call;
        call.kind = static_cast<bio::CallKind>(c.kind);
        call.p = std_vec<double>(c.p);
        call.axes = std_vec<std::int32_t>(c.axes);
        call.sizes = std_vec<double>(c.sizes);
        call.origin = std_vec<double>(c.origin);
        call.which = c.which;
        call.index = c.index;
        call.text = c.text;
        call.axis = c.axis;
        p.calls.push_back(std::move(call));
    }
    const auto ng = static_cast<std::size_t>(n_groups);
    check(in.masks.size() == static_cast<std::size_t>(in.n_masks) * ng, "masks: one value per group each");
    for (std::int32_t k = 0; k < in.n_masks; ++k)
        p.masks.emplace_back(in.masks.begin() + k * ng, in.masks.begin() + (k + 1) * ng);
    check(in.rotations.size() == static_cast<std::size_t>(in.n_rotations) * ng, "rotations: one value per group each");
    for (std::int32_t k = 0; k < in.n_rotations; ++k)
        p.rotations.emplace_back(in.rotations.begin() + k * ng, in.rotations.begin() + (k + 1) * ng);
    check(in.set_indptr_off.size() == in.set_indices_off.size(), "sets: offsets of indptr and indices");
    for (std::size_t k = 0; k + 1 < in.set_indptr_off.size(); ++k) {
        bio::Csr c;
        c.indptr.assign(in.set_indptr.begin() + in.set_indptr_off[k], in.set_indptr.begin() + in.set_indptr_off[k + 1]);
        c.indices.assign(in.set_indices.begin() + in.set_indices_off[k],
                         in.set_indices.begin() + in.set_indices_off[k + 1]);
        p.sets.push_back(std::move(c));
    }
    p.use_euc = in.use_euc;
    p.use_cell = in.use_cell;
    p.cell_lists = in.cell_lists;
    p.iter_axes = std_vec<std::int32_t>(in.iter_axes);
    p.bounds = std_vec<double>(in.bounds);
    p.self_only = in.self_only;
    p.always_true = in.always_true;
    p.always_false = in.always_false;
    return p;
}
}  // namespace

BioCsr bio_find_neighbours(const BioGroups& groups, const BioProgram& program, rust::Slice<const std::int32_t> procs,
                           bool parallel) {
    const bio::Groups g = bio_groups(groups);
    const bio::Program p = bio_program(program, g.n);
    const bio::Csr c = bio::find_neighbours(g, p, std::vector<std::int32_t>(procs.begin(), procs.end()), nullptr,
                                            parallel);
    BioCsr out;
    out.indptr = to_rust<std::int64_t>(c.indptr);
    out.indices = to_rust<std::int32_t>(c.indices);
    return out;
}

rust::Vec<std::uint8_t> bio_definition_query(const BioGroups& groups, const BioProgram& program, bool parallel) {
    const bio::Groups g = bio_groups(groups);
    const bio::Program p = bio_program(program, g.n);
    return to_rust<std::uint8_t>(bio::definition_query(g, p, parallel));
}

rust::Vec<double> bio_side_rotation(rust::Slice<const double> coords, std::int32_t ndim, std::int32_t axis0,
                                    std::int32_t axis1, double angle) {
    check(ndim > 0 && coords.size() % static_cast<std::size_t>(ndim) == 0, "coords must be n x ndim");
    bio::Groups g;
    g.ndim = ndim;
    g.n = static_cast<std::int64_t>(coords.size()) / ndim;
    g.coords.assign(coords.begin(), coords.end());
    return to_rust<double>(bio::side_rotation(g, axis0, axis1, angle));
}

rust::Vec<BioCalcInfo> bio_calculations() {
    rust::Vec<BioCalcInfo> out;
    for (const bio::CalcInfo& c : bio::calculation_registry()) {
        BioCalcInfo i;
        i.name = c.name;
        for (const auto& s : c.scalars) i.scalars.push_back(rust::String(s));
        for (const auto& l : c.lists) {
            i.list_names.push_back(rust::String(l.first));
            i.list_kinds.push_back(static_cast<std::int32_t>(l.second));
        }
        i.uses_nbr_lists = c.uses_nbr_lists;
        i.uses_processing_element = c.uses_processing_element;
        i.uses_tree = c.uses_tree;
        out.push_back(std::move(i));
    }
    return out;
}

namespace {
bio::TreeView bio_tree(const BioTree& in) {
    bio::TreeView t;
    t.n = in.n;
    t.parent = std_vec<std::int32_t>(in.parent);
    t.length = std_vec<double>(in.length);
    t.child_ptr = std_vec<std::int64_t>(in.child_ptr);
    t.child_idx = std_vec<std::int32_t>(in.child_idx);
    t.depth = std_vec<std::int32_t>(in.depth);
    t.terminal = std_vec<std::uint8_t>(in.terminal);
    t.in_trimmed = std_vec<std::uint8_t>(in.in_trimmed);
    t.trimmed_child_count = std_vec<std::int32_t>(in.trimmed_child_count);
    t.total_length = in.total_length;
    t.trimmed_total_length = in.trimmed_total_length;
    t.root = in.root;
    t.label_node = std_vec<std::int32_t>(in.label_node);
    t.node_label = std_vec<std::int32_t>(in.node_label);
    t.n_terminals = in.n_terminals;
    t.n_trimmed_terminals = in.n_trimmed_terminals;
    t.zero_length_count = in.zero_length_count;
    t.eq_full_total = in.eq_full_total;
    t.eq_trimmed_total = in.eq_trimmed_total;
    t.eq_trimmed_length = in.eq_trimmed_length;
    t.default_eq_len = in.default_eq_len;
    t.pc_steps = std_vec<std::int32_t>(in.pc_steps);
    t.pc_vpd = in.pc_vpd;
    t.pc_seed = in.pc_seed;
    return t;
}
}  // namespace

rust::Vec<std::uint8_t> bio_ultrametric(const BioTree& tree) {
    const bio::TreeView tv = bio_tree(tree);
    bio::PhyloCom pc(tv, false, 0);
    rust::Vec<std::uint8_t> out;
    out.push_back(pc.ultrametric ? 1 : 0);
    out.push_back(pc.multifurcating ? 1 : 0);
    return out;
}

std::unique_ptr<BioRng> bio_rng_new(std::uint64_t seed) { return std::make_unique<BioRng>(seed); }

rust::Vec<std::int64_t> bio_rng_shuffle(BioRng& rng, std::int64_t n) {
    std::vector<std::int64_t> v(static_cast<std::size_t>(std::max<std::int64_t>(n, 0)));
    for (std::int64_t i = 0; i < n; ++i) v[static_cast<std::size_t>(i)] = i;
    rng.mt.shuffle(v);
    return to_rust<std::int64_t>(v);
}

std::unique_ptr<BioAlloc> bio_alloc_none() { return std::make_unique<BioAlloc>(); }

std::unique_ptr<BioAlloc> bio_alloc_new(rust::Str order, rust::Str backtracking, double reseed_prob, std::int64_t n_groups,
                                        std::int32_t n_sets, rust::Slice<const std::int64_t> nbr_ptr,
                                        rust::Slice<const std::int32_t> nbr_idx, bool same,
                                        rust::Slice<const double> coords, std::int32_t ndim, bool have_seeds,
                                        rust::Slice<const std::int32_t> seeds) {
    check(static_cast<std::int64_t>(nbr_ptr.size()) == n_groups * n_sets + 1, "nbr_ptr: a row per group and set");
    check(static_cast<std::int64_t>(coords.size()) == n_groups * ndim, "coords: n_groups x ndim");
    std::vector<std::vector<std::vector<std::int32_t>>> nbrs(static_cast<std::size_t>(n_groups));
    std::vector<std::vector<double>> xy(static_cast<std::size_t>(n_groups));
    for (std::int64_t g = 0; g < n_groups; ++g) {
        for (std::int32_t s = 0; s < n_sets; ++s) {
            const std::int64_t r = g * n_sets + s;
            nbrs[g].emplace_back(nbr_idx.begin() + nbr_ptr[r], nbr_idx.begin() + nbr_ptr[r + 1]);
        }
        xy[g].assign(coords.begin() + g * ndim, coords.begin() + (g + 1) * ndim);
    }
    auto out = std::make_unique<BioAlloc>();
    out->a = std::make_unique<bio::SpatialAlloc>(std::string(order), std::string(backtracking), reseed_prob,
                                                 std::move(nbrs), same, std::move(xy), have_seeds,
                                                 std::vector<std::int32_t>(seeds.begin(), seeds.end()));
    return out;
}

namespace {
bio::CountsCsr counts_csr(rust::Slice<const std::int64_t> indptr, rust::Slice<const std::int32_t> indices,
                          rust::Slice<const double> counts, std::int64_t n_groups, std::int64_t n_labels) {
    bio::CountsCsr bd;
    bd.n_groups = n_groups;
    bd.n_labels = n_labels;
    bd.indptr.assign(indptr.begin(), indptr.end());
    bd.indices.assign(indices.begin(), indices.end());
    bd.counts.assign(counts.begin(), counts.end());
    check(static_cast<std::int64_t>(bd.indptr.size()) == n_groups + 1 && bd.indices.size() == bd.counts.size(),
          "counts: a groups x labels CSR");
    return bd;
}

BioCounts bio_counts(const bio::CountsCsr& c) {
    BioCounts out;
    out.indptr = to_rust<std::int64_t>(c.indptr);
    out.indices = to_rust<std::int32_t>(c.indices);
    out.counts = to_rust<double>(c.counts);
    return out;
}
}  // namespace

BioCounts bio_rand_structured(rust::Slice<const std::int64_t> indptr, rust::Slice<const std::int32_t> indices,
                              rust::Slice<const double> counts, std::int64_t n_groups, std::int64_t n_labels,
                              BioRng& rng, double multiplier, double addition, BioAlloc& alloc) {
    const bio::CountsCsr bd = counts_csr(indptr, indices, counts, n_groups, n_labels);
    return bio_counts(bio::rand_structured(bd, rng.mt, multiplier, addition, alloc.a.get()));
}

BioCounts bio_rand_swaps(rust::Slice<const std::int64_t> indptr, rust::Slice<const std::int32_t> indices,
                         rust::Slice<const double> counts, std::int64_t n_groups, std::int64_t n_labels, BioRng& rng,
                         rust::Slice<const double> label_range, rust::Str kind, std::int64_t swap_count,
                         std::int64_t max_swap_attempts, bool stop_on_all_swapped, bool use_max_swap) {
    const bio::CountsCsr bd = counts_csr(indptr, indices, counts, n_groups, n_labels);
    const std::vector<double> ranges(label_range.begin(), label_range.end());
    bio::SwapOptions opt;
    opt.swap_count = swap_count;
    opt.max_swap_attempts = max_swap_attempts;
    opt.stop_on_all_swapped = stop_on_all_swapped;
    opt.use_max_swap = use_max_swap;
    const std::string k(kind);
    if (k == "curveball") return bio_counts(bio::rand_curveball(bd, rng.mt, opt));
    if (k == "independent_swaps") return bio_counts(bio::rand_independent_swaps(bd, rng.mt, ranges, opt, false));
    if (k == "independent_swaps_modified") return bio_counts(bio::rand_independent_swaps(bd, rng.mt, ranges, opt, true));
    throw std::invalid_argument("unknown swap randomisation " + k);
}

BioClusterOut bio_cluster(const rust::Vec<rust::String>& names, std::int32_t n_sets, rust::Slice<const std::int64_t> nbr_ptr,
                          rust::Slice<const std::int32_t> nbr_idx, bool trim_processed, rust::Str direct,
                          rust::Slice<const std::int64_t> label_ptr, rust::Slice<const std::int32_t> label_idx,
                          rust::Slice<const std::int64_t> pair_keys, rust::Slice<const double> pair_values,
                          rust::Str linkage, bool can_lump_zeroes, double min_poss, bool have_tie_breaker,
                          const rust::Vec<rust::String>& breaker_keys, const rust::Vec<rust::String>& breaker_minmax,
                          bool objective_max, double max_poss, BioRng& rng, bool use_recalc, ClusterRecalc& recalc) {
    bio::ClusterInput in;
    for (const auto& s : names) in.names.emplace_back(std::string(s));
    const auto n = static_cast<std::int64_t>(in.names.size());
    check(static_cast<std::int64_t>(nbr_ptr.size()) == n_sets * n + 1, "nbr_ptr: a row per condition and element");
    for (std::int32_t k = 0; k < n_sets; ++k) {
        std::vector<std::vector<std::int32_t>> per;
        for (std::int64_t e = 0; e < n; ++e) {
            const std::int64_t r = k * n + e;
            per.emplace_back(nbr_idx.begin() + nbr_ptr[r], nbr_idx.begin() + nbr_ptr[r + 1]);
        }
        in.nbrs.push_back(std::move(per));
    }
    in.trim_processed = trim_processed;
    in.direct = std::string(direct);
    if (!in.direct.empty()) {
        check(static_cast<std::int64_t>(label_ptr.size()) == n + 1, "label_ptr: a row per element");
        for (std::int64_t e = 0; e < n; ++e)
            in.labels.emplace_back(label_idx.begin() + label_ptr[e], label_idx.begin() + label_ptr[e + 1]);
    }
    in.pair_keys.assign(pair_keys.begin(), pair_keys.end());
    in.pair_values.assign(pair_values.begin(), pair_values.end());
    in.linkage = std::string(linkage);
    in.can_lump_zeroes = can_lump_zeroes;
    in.min_poss = min_poss;
    in.have_tie_breaker = have_tie_breaker;
    for (const auto& s : breaker_keys) in.breaker_keys.emplace_back(std::string(s));
    for (const auto& s : breaker_minmax) in.breaker_minmax.emplace_back(std::string(s));
    in.objective_max = objective_max;
    in.max_poss = max_poss;
    if (use_recalc) {
        ClusterRecalc* r = &recalc;
        in.recalculate = [r](const std::string& a, const std::string& b, const std::string& nn,
                             const std::vector<std::string>& checks) {
            rust::Vec<rust::String> c;
            for (const auto& s : checks) c.push_back(rust::String(s));
            const rust::Vec<double> v = bio_recalculate(*r, rust::Str(a), rust::Str(b), rust::Str(nn), c);
            return std::vector<double>(v.begin(), v.end());
        };
    }
    const bio::ClusterOutput out = bio::cluster_groups(in, rng.mt);
    BioClusterOut res;
    for (const auto& t : out.terminals) res.terminals.push_back(rust::String(t));
    for (const auto& j : out.joins) {
        BioJoin bj;
        bj.join_number = j.join_number;
        bj.name = rust::String(j.name);
        bj.child1 = rust::String(j.child1);
        bj.child2 = rust::String(j.child2);
        bj.value = j.value;
        bj.matrix_iter = j.matrix_iter;
        res.joins.push_back(std::move(bj));
    }
    res.join_number = out.join_number;
    res.min_value = out.min_value;
    return res;
}

rust::Vec<double> bio_perl_sqrt(rust::Slice<const double> x) {
    rust::Vec<double> out;
    for (const double v : x) out.push_back(bio::perl_sqrt(v));
    return out;
}

BioIndexOut bio_run_indices(const BioView& view, rust::Slice<const std::int32_t> procs, const BioCsr& set1, bool two,
                            const BioCsr& set2, const rust::Vec<rust::String>& calcs, bool parallel, bool with_tree,
                            const BioTree& tree) {
    bio::BaseDataView b;
    b.n_groups = view.n_groups;
    b.n_labels = view.n_labels;
    b.indptr = std_vec<std::int64_t>(view.indptr);
    b.indices = std_vec<std::int32_t>(view.indices);
    b.counts = std_vec<double>(view.counts);
    b.label_range = std_vec<double>(view.label_range);
    b.label_abundance = std_vec<double>(view.label_abundance);
    b.label_sample_count = std_vec<double>(view.label_sample_count);
    b.abc_any_variant = view.abc_any_variant;
    b.n_label_levels = view.n_label_levels;
    b.label_prefix = std_vec<std::int32_t>(view.label_prefix);
    for (const auto& s : view.prefix_names) b.prefix_names.emplace_back(std::string(s));
    b.labels_numeric = view.labels_numeric;
    b.label_values = std_vec<double>(view.label_values);
    b.exec_order = std_vec<std::int32_t>(view.exec_order);
    b.pairwise_mode = view.pairwise_mode;
    auto props = [](const rust::Vec<rust::String>& names, const rust::Vec<std::int64_t>& key_off,
                    const rust::Vec<rust::String>& keys, const rust::Vec<double>& values,
                    const rust::Vec<std::int32_t>& elem, std::size_t n_elems,
                    std::vector<bio::BaseDataView::Property>& out) {
        check(key_off.size() == names.size() + 1 && elem.size() == names.size() * n_elems,
              "properties: offsets and a value per property and element");
        for (std::size_t k = 0; k < names.size(); ++k) {
            bio::BaseDataView::Property p;
            p.name = std::string(names[k]);
            for (std::int64_t i = key_off[k]; i < key_off[k + 1]; ++i) {
                p.keys.emplace_back(std::string(keys[static_cast<std::size_t>(i)]));
                p.values.push_back(values[static_cast<std::size_t>(i)]);
            }
            p.element_key.assign(elem.begin() + static_cast<std::ptrdiff_t>(k * n_elems),
                                 elem.begin() + static_cast<std::ptrdiff_t>((k + 1) * n_elems));
            out.push_back(std::move(p));
        }
    };
    props(view.label_prop_names, view.label_prop_key_off, view.label_prop_keys, view.label_prop_values,
          view.label_prop_elem, static_cast<std::size_t>(view.n_labels), b.label_props);
    props(view.group_prop_names, view.group_prop_key_off, view.group_prop_keys, view.group_prop_values,
          view.group_prop_elem, static_cast<std::size_t>(view.n_groups), b.group_props);
    if (view.has_matrix) {
        b.has_matrix = true;
        b.label_mx = std_vec<std::int32_t>(view.label_mx);
        b.mx_n = view.mx_n;
        b.mx_values = std_vec<double>(view.mx_values);
        b.mx_self_similarity = view.mx_self_similarity;
    }
    if (view.has_row_labels) {
        b.has_row_labels = true;
        b.row_indptr = std_vec<std::int64_t>(view.row_indptr);
        b.row_labels = std_vec<std::int32_t>(view.row_labels);
        b.row_counts = std_vec<double>(view.row_counts);
        b.row_samples = std_vec<double>(view.row_samples);
    }
    std::vector<std::string> names;
    for (const auto& c : calcs) names.emplace_back(std::string(c));
    const bio::Csr s1 = bio_csr(set1);
    const bio::Csr s2 = bio_csr(set2);
    bio::TreeView tv;
    if (with_tree) tv = bio_tree(tree);
    bio::IndexResult r = bio::run_indices(b, std::vector<std::int32_t>(procs.begin(), procs.end()), s1,
                                          two ? &s2 : nullptr, names, parallel, with_tree ? &tv : nullptr);
    BioIndexOut out;
    for (const auto& s : r.scalar_names) out.scalar_names.push_back(rust::String(s));
    out.scalars = to_rust<double>(r.scalars);
    out.list_indptr_off.push_back(0);
    out.list_entry_off.push_back(0);
    for (std::size_t k = 0; k < r.lists.size(); ++k) {
        out.list_names.push_back(rust::String(r.list_names[k]));
        out.list_kinds.push_back(static_cast<std::int32_t>(r.list_kinds[k]));
        for (const std::int64_t v : r.lists[k].indptr) out.list_indptr.push_back(v);
        for (const double v : r.lists[k].keys) out.list_keys.push_back(v);
        for (const double v : r.lists[k].values) out.list_values.push_back(v);
        out.list_indptr_off.push_back(static_cast<std::int64_t>(out.list_indptr.size()));
        out.list_entry_off.push_back(static_cast<std::int64_t>(out.list_keys.size()));
    }
    for (const auto& s : r.texts) out.texts.push_back(rust::String(s));
    return out;
}

PathsOut cost_paths(rust::Slice<const double> values, rust::Slice<const double> grid, std::int32_t directions,
                    bool symmetric, std::int64_t origin, rust::Slice<const std::int64_t> goals) {
    const cost::Grid g = cost_grid(grid);
    cost_check(g, directions, symmetric, values.size());
    check(origin >= 0 && origin < g.ncell(), "the origin is outside the grid");
    const cost::Layout t{&g, directions, symmetric, values.data()};
    const std::vector<std::vector<std::int64_t>> paths =
        cost::shortest_paths(t, origin, std::vector<std::int64_t>(goals.begin(), goals.end()));
    PathsOut out;
    out.offsets.push_back(0);
    for (const auto& p : paths) {
        for (std::int64_t c : p) out.cells.push_back(c);
        out.offsets.push_back(static_cast<std::int64_t>(out.cells.size()));
    }
    return out;
}

rust::Vec<double> io_extract(rust::Slice<const double> values, std::int32_t bands, std::int32_t ny, std::int32_t nx,
                             rust::Slice<const double> gt, rust::Slice<const double> x, rust::Slice<const double> y,
                             bool bilinear, std::int32_t row0, std::int32_t ny_total) {
    check(bands > 0 && ny > 0 && nx > 0 && values.size() == static_cast<std::size_t>(bands) * ny * nx,
          "the values differ in size from bands x ny x nx");
    check(gt.size() == 6 && x.size() == y.size(), "a geotransform and the points' x and y");
    check(row0 >= 0 && row0 + ny <= ny_total, "the block's rows lie in the grid");
    return to_rust<double>(lsio::extract(values.data(), bands, ny, nx, false, 0.0, gt.data(), x.data(), y.data(),
                                         static_cast<i64>(x.size()), bilinear, row0, ny_total));
}

}  // namespace tfx
