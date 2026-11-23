# bargy_ibex_2013.csv

GPS locations of four Alpine ibex (*Capra ibex*) of the Bargy massif (Haute-Savoie, France), 31 May to 28 August
2013: 2,643 locations, in four tracks (`I9`, `I10`, `I18`, `I27`: the ibex 9, 10, 18 and 27 of the source).

Columns, with the names of a Movebank export, so that `move.read_tracks` (Terraflux) and `tf.load_tracks` (the library)
read the file as it is: `individual-local-identifier` (the animal), `timestamp` (UTC) and `location-long` and
`location-lat` (WGS 84, EPSG:4326).

**Source.** Marchand, P., Risser, N., Petit, E., & Garel, M. (2025). Data from: Disturbance by massive sporting events
in mountain areas: when and where matters for the protected Alpine ibex *Capra ibex* [Data set]. Zenodo.
https://doi.org/10.5281/zenodo.17052197 (the paper: *People and Nature*, 7(11), 3002-3016,
https://doi.org/10.1002/pan3.70151).

**Licence.** Creative Commons Attribution 4.0 (CC-BY 4.0), as the record's. Cite the paper and the record.

**What was changed.** From the record's `locs_export` table: the locations with a position, outside the periods of the
massive sporting events (`MSE_or_reference == "without_MSE"`), the first record of each animal and time, in 2013, of the
animals 9, 10, 18 and 27; the columns renamed and the time written in UTC. Nothing else (no smoothing, no thinning).
The script that made the file is `write.R` in the same folder.
