//! Several rasters dropped at once, a layer each, joined as one stack (a time series played with *Play*) when
//! asked: one `raster.read` of a pattern that names exactly those files when they are in one folder (the nodes
//! that read them one by one go, unless another node reads them), else a `stack.of` they feed.

use std::path::PathBuf;

use terraflux_ops::Registry;

use crate::app::Session;

/// A pattern of file names (`folder/landuse_*.tif`) that names exactly these files: in one folder, the start and
/// the end their names share around a `*`; none when they are in several folders or the pattern names others too.
pub fn pattern_of(files: &[PathBuf]) -> Option<String> {
    let dir = files.first()?.parent()?;
    if files.len() < 2 || files.iter().any(|f| f.parent() != Some(dir)) {
        return None;
    }
    let names: Vec<Vec<char>> = files
        .iter()
        .map(|f| f.file_name().map(|n| n.to_string_lossy().chars().collect()).unwrap_or_default())
        .collect();
    let shortest = names.iter().map(Vec::len).min()?;
    let mut start = 0;
    while start < shortest && names.iter().all(|n| n[start] == names[0][start]) {
        start += 1;
    }
    let mut end = 0;
    while end < shortest - start && names.iter().all(|n| n[n.len() - 1 - end] == names[0][names[0].len() - 1 - end]) {
        end += 1;
    }
    // (a pattern with nothing around the star names the whole folder)
    if start + end == 0 {
        return None;
    }
    let head: String = names[0][..start].iter().collect();
    let tail: String = names[0][names[0].len() - end..].iter().collect();
    let pattern = dir.join(format!("{head}*{tail}")).to_string_lossy().into_owned();
    let mut found = terraflux_ops::files::files_of(&pattern).ok()?;
    let mut wanted = files.to_vec();
    found.sort();
    wanted.sort();
    (found == wanted).then_some(pattern)
}

/// The rasters of nodes (`raster.read` of a file, at the model's top level) joined as one stack: a `raster.read`
/// of a pattern in place of them, else a `stack.of` they feed (below the first); the stack kept as a layer of the
/// Viewer in their place and shown. Its id.
pub fn join(s: &mut Session, r: &Registry, ids: &[String]) -> Result<String, String> {
    let read: Vec<(String, PathBuf, Option<[f32; 2]>)> = ids
        .iter()
        .filter_map(|id| {
            let n = s.source.nodes.iter().find(|n| &n.id == id && n.op == "raster.read")?;
            let path = n.params.get("path")?.as_str()?;
            Some((id.clone(), PathBuf::from(path), n.pos))
        })
        .collect();
    if read.len() < 2 {
        return Err("fewer than two rasters to join".into());
    }
    let files: Vec<PathBuf> = read.iter().map(|(_, p, _)| p.clone()).collect();
    let at = read[0].2;
    let id = match pattern_of(&files) {
        Some(pattern) => {
            // (the nodes no other node reads go: the new one reads their files)
            let reads = |id: &str| {
                let from = format!("{id}.");
                s.source.nodes.iter().any(|n| {
                    n.inputs.values().any(|v| matches!(v, terraflux_graph::Source::Ref(r) if r.starts_with(&from)))
                })
            };
            let gone: Vec<String> = read.iter().map(|(id, _, _)| id.clone()).filter(|id| !reads(id)).collect();
            for g in &gone {
                crate::editor::remove_node(&mut s.source, &[], g)?;
            }
            s.preview_layers.retain(|l| !gone.contains(&l.node));
            crate::builder::add_at(s, r, "raster.read", &[("path", pattern.into())], at)?
        }
        None => {
            let below = at.map(|[x, y]| [x + 260.0, y]);
            let id = crate::builder::add_at(s, r, "stack.of", &[], below)?;
            if let Some(n) = s.source.nodes.iter_mut().find(|n| n.id == id) {
                for (from, _, _) in &read {
                    n.inputs.insert(from.clone(), terraflux_graph::Source::Ref(format!("{from}.raster")));
                }
            }
            let joined: Vec<&String> = read.iter().map(|(id, _, _)| id).collect();
            s.preview_layers.retain(|l| !joined.contains(&&l.node));
            s.edited();
            id
        }
    };
    let port = s
        .source
        .nodes
        .iter()
        .find(|n| n.id == id)
        .and_then(|n| crate::builder::first_output(n, r))
        .unwrap_or_else(|| "raster".into());
    crate::builder::pin(&mut s.preview_layers, &id, &port);
    crate::builder::select(s, r, &id);
    s.preview = Some((id.clone(), port));
    s.show_queue.push(id.clone());
    s.dropped_rasters.clear();
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_names_exactly_the_files_dropped() {
        let dir = std::env::temp_dir().join(format!("terraflux-series-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["landuse_1997.tif", "landuse_2011.tif", "landuse_2020.tif", "roads.tif"] {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        let two = [dir.join("landuse_1997.tif"), dir.join("landuse_2011.tif")];
        let three = [dir.join("landuse_2011.tif"), dir.join("landuse_1997.tif"), dir.join("landuse_2020.tif")];
        // (two of the three years: "landuse_*.tif" would name the third too)
        assert_eq!(pattern_of(&two), None);
        let want = dir.join("landuse_*.tif").to_string_lossy().into_owned();
        assert_eq!(pattern_of(&three), Some(want));
        // nothing shared around the star: the whole folder, not a pattern of them
        assert_eq!(pattern_of(&[dir.join("roads.tif"), dir.join("landuse_1997.tif")]), None);
        // in two folders: none
        let other = dir.join("b");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("landuse_2030.tif"), b"").unwrap();
        assert_eq!(pattern_of(&[dir.join("landuse_1997.tif"), other.join("landuse_2030.tif")]), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
