# SPDX-License-Identifier: GPL-3.0-or-later
# What the R package segmetric (Costa et al.) gives, for Terraflux's tests of segment.evaluate: a few cases of a
# reference and a segmentation on a grid of 30 x 24 cells, each region the union of whole cells (so that the areas are
# exact and the raster and vector worlds agree), written as polygons (a GeoPackage of the reference and one of the
# segmentation per case, x to the east and y minus the row, as a Terraflux grid of cell size 1 has them) and as the
# labels of the cells, with the pairs and the values segmetric computes:
#
#     Rscript tools/make_segmetric_reference.R
#
# Needs segmetric, sf and jsonlite. The case "equal" is a segmentation equal to its reference, "over" one with many
# more regions, "under" one with fewer, "apart" a reference with cells in none and a region cut in two parts, and
# "mixed" a bit of everything. The values are those of sm_compute's rounding (10 digits).

suppressMessages({
  library(segmetric)
  library(sf)
  library(jsonlite)
})

here <- normalizePath(file.path(dirname(sub("--file=", "", grep("--file=", commandArgs(FALSE), value = TRUE))), ".."))
out <- file.path(here, "tests", "data", "reference", "segmetric")
dir.create(out, showWarnings = FALSE, recursive = TRUE)
W <- 30
H <- 24

# a Voronoi partition of the grid by `n` seeds (cells to the nearest, the first on a tie), numbered by the first cell
voronoi <- function(n, seed) {
  set.seed(seed)
  sx <- sample.int(W, n, replace = TRUE) - 0.5
  sy <- sample.int(H, n, replace = TRUE) - 0.5
  lab <- matrix(0L, H, W)
  for (r in seq_len(H)) for (c in seq_len(W)) {
    lab[r, c] <- which.min((sx - (c - 0.5))^2 + (sy - (r - 0.5))^2)
  }
  renumber(lab)
}

# the labels numbered 1.. in the order of their first cell, row by row (0 stays 0)
renumber <- function(lab) {
  flat <- as.vector(t(lab))
  ids <- unique(flat[flat > 0])
  new <- match(lab, ids)
  new[lab == 0] <- 0L
  matrix(as.integer(new), nrow(lab), ncol(lab))
}

# the polygons of a label matrix, a feature per label, the union of its cells
polygons <- function(lab) {
  ids <- sort(unique(lab[lab > 0]))
  geoms <- lapply(ids, function(id) {
    cells <- which(lab == id, arr.ind = TRUE)
    squares <- lapply(seq_len(nrow(cells)), function(k) {
      r <- cells[k, 1] - 1
      c <- cells[k, 2] - 1
      st_polygon(list(rbind(c(c, -r), c(c + 1, -r), c(c + 1, -r - 1), c(c, -r - 1), c(c, -r))))
    })
    st_union(st_sfc(squares))
  })
  st_sf(id = ids, geometry = do.call(c, geoms))
}

cases <- list()
ref <- voronoi(8, 11)
cases$equal <- list(ref = ref, seg = ref)
cases$over <- list(ref = ref, seg = voronoi(40, 12))
cases$under <- list(ref = ref, seg = voronoi(3, 13))
apart <- voronoi(10, 14)
apart[1:8, 1:12] <- 0L
apart <- renumber(apart)
cut <- apart
biggest <- as.integer(names(which.max(table(apart[apart > 0]))))
cut[cut == biggest & col(cut) > 15] <- max(cut) + 1L
cases$apart <- list(ref = apart, seg = renumber(cut))
cases$mixed <- list(ref = voronoi(12, 15), seg = voronoi(18, 16))

as_rows <- function(m) lapply(seq_len(nrow(m)), function(r) as.integer(m[r, ]))

result <- list(segmetric = as.character(packageVersion("segmetric")), width = W, height = H, cases = list())
for (name in names(cases)) {
  ref_sf <- polygons(cases[[name]]$ref)
  seg_sf <- polygons(cases[[name]]$seg)
  st_write(ref_sf, file.path(out, paste0(name, "_reference.gpkg")), delete_dsn = TRUE, quiet = TRUE)
  st_write(seg_sf, file.path(out, paste0(name, "_segmentation.gpkg")), delete_dsn = TRUE, quiet = TRUE)
  # (the ids of segmetric are the rows of the datasets: the labels, as they are numbered 1..)
  m <- sm_read(ref_sf = ref_sf, seg_sf = seg_sf)
  ids <- c("OS1", "US1", "D_index", "QR", "AFI", "precision", "recall", "F_measure", "Dice")
  m <- sm_compute(m, ids)
  pairs <- function(s) {
    d <- st_drop_geometry(s)[c("ref_id", "seg_id")]
    unname(lapply(seq_len(nrow(d)), function(i) c(d$ref_id[i], d$seg_id[i])))
  }
  star <- sm_ystar(m)
  prime <- sm_yprime(m)
  xprime <- sm_xprime(m)
  per_pair <- function(s, metric) {
    f <- get(metric, asNamespace("segmetric"))
    as.numeric(f(m, s))
  }
  result$cases[[name]] <- list(
    reference = as_rows(cases[[name]]$ref),
    segmentation = as_rows(cases[[name]]$seg),
    summary = as.list(setNames(as.numeric(summary(m)), names(m))),
    star = pairs(star), os = per_pair(star, "OS1"), us = per_pair(star, "US1"),
    d = per_pair(star, "D_index"), qr = per_pair(star, "QR"),
    prime = pairs(prime), afi = per_pair(prime, "AFI"),
    xprime = pairs(xprime)
  )
}
write(toJSON(result, auto_unbox = TRUE, digits = NA), file.path(out, "segmetric.json"))
cat("wrote", file.path(out, "segmetric.json"), "\n")
