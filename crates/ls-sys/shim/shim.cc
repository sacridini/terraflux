// SPDX-License-Identifier: GPL-3.0-or-later
//
// The shim between Terraflux (Rust, cxx) and landschaft's C++ kernels: Rust
// slices and vectors in, landschaft's types and calls, Rust vectors out.
// Exceptions of the kernels reach Rust as errors (cxx's Result).

#include "terraflux-ls-sys/shim/shim.h"

#include <stdexcept>
#include <vector>

#include "lsm/rmath.hpp"
#include "nlm/models.hpp"
#include "sim/change.hpp"
#include "sim/clues.hpp"
#include "sim/demand.hpp"
#include "sim/observed.hpp"
#include "sim/ordered.hpp"
#include "sim/patches.hpp"
#include "sim/rglm.hpp"
#include "sim/rules.hpp"
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

}  // namespace

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

ObservedPatches observed_patches(rust::Slice<const double> a, rust::Slice<const double> b, std::int64_t ny,
                                 std::int64_t nx, double from, double to) {
    check(static_cast<i64>(a.size()) == ny * nx && b.size() == a.size(), "the maps differ in size from ny x nx");
    const auto r = sim::observed_patches(a.data(), b.data(), ny, nx, from, to);
    return ObservedPatches{r.cells, r.expansion, r.expansion_patches, r.expansion_mean, r.expansion_variance,
                           r.new_patches, r.new_mean, r.new_variance};
}

}  // namespace tfx
