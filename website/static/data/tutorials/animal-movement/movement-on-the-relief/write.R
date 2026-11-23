d <- commandArgs(TRUE)[1]
load(file.path(d, "marchand2025", "d.Rdata"))
x <- locs_export
x <- x[!is.na(x$long) & !is.na(x$lat) & x$MSE_or_reference == "without_MSE", ]
x <- x[!duplicated(x[c("ibex_id", "date")]), ]
x <- x[format(x$date, "%Y", tz = "UTC") == "2013" & x$ibex_id %in% c("9", "10", "18", "27"), ]
x <- x[order(x$ibex_id, x$date), ]
# Movebank's column names, so that move.read_tracks (and tf.load_tracks) read it as it is
out <-data.frame("individual-local-identifier" = paste0("I", x$ibex_id), timestamp = format(x$date, "%Y-%m-%d %H:%M:%S.000", tz = "UTC"),
                  "location-long" = sprintf("%.7f", x$long), "location-lat" = sprintf("%.7f", x$lat), check.names = FALSE)
write.csv(out, file.path(d, "bargy_ibex_2013.csv"), row.names = FALSE, quote = FALSE)
cat(nrow(out), "rows;", range(x$long), range(x$lat), "\n")
