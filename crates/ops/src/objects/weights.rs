//! The weights of SAM for `segment.sam` when no ONNX files are given: an export of SAM ViT-B (Apache-2.0, from
//! https://huggingface.co/vietanhdev/segment-anything-onnx-models) downloaded once, as a zip, to the cache folder, its
//! SHA-256 checked, and its encoder and decoder extracted there. A later run reuses the files, after checking their
//! sizes against what was recorded when they were extracted. The cache logic takes the download as a function, so it
//! is tested without the network.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// One of the zips of the export.
pub struct Source {
    /// The name in the cache (a folder) and in the messages.
    pub name: &'static str,
    pub url: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

/// The quantised ViT-B (75 MB): the default.
pub const QUANT: Source = Source {
    name: "vit_b_quant",
    url: "https://huggingface.co/vietanhdev/segment-anything-onnx-models/resolve/main/sam_vit_b_01ec64_quant.zip",
    bytes: 75_377_417,
    sha256: "9d59db12affc009f71c9ddeab8fc833507510883e7f6bc4c0cc6c45e0d4f5862",
};

/// The full-precision ViT-B (348 MB).
pub const FULL: Source = Source {
    name: "vit_b",
    url: "https://huggingface.co/vietanhdev/segment-anything-onnx-models/resolve/main/sam_vit_b_01ec64.zip",
    bytes: 348_403_199,
    sha256: "e41630c522ac51309e1fedf6c7050c042a1823baf852c99a01abf677ebba430e",
};

/// The source a `weights` setting names ("quant" or "full").
pub fn source(weights: &str) -> Result<&'static Source> {
    match weights {
        "quant" => Ok(&QUANT),
        "full" => Ok(&FULL),
        w => Err(Error::msg(format!("weights {w:?}: quant (75 MB) or full (348 MB)"))),
    }
}

/// How a download reports: the bytes so far and the total (when known); false stops it.
pub type Report<'a> = &'a mut dyn FnMut(u64, Option<u64>) -> bool;

/// A download: the source's file written to the path (`report` told as it goes).
pub type Download<'a> = &'a mut dyn FnMut(&Source, &Path, Report) -> Result<()>;

/// The encoder and the decoder of a source, extracted in `<cache>/sam/<name>/` (downloaded and extracted when they are
/// not there, or their sizes are not what was recorded).
pub fn ensure(cache: &Path, src: &Source, download: Download, report: Report) -> Result<(PathBuf, PathBuf)> {
    let dir = cache.join("sam").join(src.name);
    let (enc, dec, ok) = (dir.join("encoder.onnx"), dir.join("decoder.onnx"), dir.join("weights.ok"));
    if recorded(&ok, src, &enc, &dec) {
        return Ok((enc, dec));
    }
    std::fs::create_dir_all(&dir).map_err(|e| Error::msg(format!("{}: {e}", dir.display())))?;
    let pid = std::process::id();
    let zip = dir.join(format!(".download-{pid}.zip"));
    let made = (|| -> Result<()> {
        download(src, &zip, report)?;
        let (size, sha) = file_sha(&zip)?;
        if size != src.bytes || sha != src.sha256 {
            return Err(Error::msg(format!(
                "{}: the download is not the expected file ({size} bytes, SHA-256 {sha}; expected {} bytes, {})",
                src.url, src.bytes, src.sha256
            )));
        }
        let mut z = std::fs::File::open(&zip).map_err(|e| Error::msg(format!("{}: {e}", zip.display())))?;
        let entries = entries(&mut z)?;
        let mut sizes = Vec::new();
        for (part, to) in [("encoder", &enc), ("decoder", &dec)] {
            let e = entries
                .iter()
                .find(|e| e.name.contains(&format!(".{part}")) && e.name.ends_with(".onnx"))
                .ok_or_else(|| Error::msg(format!("{}: the zip holds no {part} (.onnx)", src.url)))?;
            let tmp = dir.join(format!(".{part}-{pid}.onnx"));
            let n = extract(&mut z, e, &tmp)?;
            std::fs::rename(&tmp, to).map_err(|e| Error::msg(format!("{}: {e}", to.display())))?;
            sizes.push(n);
        }
        // (last: the files count as there only once both are)
        let note = format!("{}\n{}\n{}\n", src.sha256, sizes[0], sizes[1]);
        let tmp = dir.join(format!(".ok-{pid}"));
        std::fs::write(&tmp, note).map_err(|e| Error::msg(format!("{}: {e}", tmp.display())))?;
        std::fs::rename(&tmp, &ok).map_err(|e| Error::msg(format!("{}: {e}", ok.display())))
    })();
    let _ = std::fs::remove_file(&zip);
    made?;
    Ok((enc, dec))
}

/// Whether the sidecar records this source and the two files have the sizes it records.
fn recorded(ok: &Path, src: &Source, enc: &Path, dec: &Path) -> bool {
    let Ok(note) = std::fs::read_to_string(ok) else { return false };
    let l: Vec<&str> = note.lines().collect();
    let size = |p: &Path| std::fs::metadata(p).ok().map(|m| m.len());
    l.len() == 3 && l[0] == src.sha256 && size(enc) == l[1].parse().ok() && size(dec) == l[2].parse().ok()
}

/// A file's size and SHA-256.
fn file_sha(p: &Path) -> Result<(u64, String)> {
    let e = |e: std::io::Error| Error::msg(format!("{}: {e}", p.display()));
    let mut f = std::fs::File::open(p).map_err(e)?;
    let (mut h, mut n, mut buf) = (Sha256::new(), 0u64, vec![0u8; 1 << 20]);
    loop {
        let k = f.read(&mut buf).map_err(e)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        n += k as u64;
    }
    Ok((n, h.finalize().iter().map(|b| format!("{b:02x}")).collect()))
}

/// The download over HTTPS, written to `to` and reported (the zip's bytes are hashed by the caller afterwards).
pub fn download(src: &Source, to: &Path, report: Report) -> Result<()> {
    use ureq::tls::{RootCerts, TlsConfig};
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(30)))
        .timeout_global(None)
        .timeout_recv_body(Some(std::time::Duration::from_secs(120)))
        .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
        .user_agent(format!("Terraflux/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let r = agent.get(src.url).call().map_err(|e| {
        Error::msg(format!(
            "{}: {e} (the SAM weights are downloaded on first use: is the computer online? Or give `encoder` and \
             `decoder` the paths of ONNX files)",
            src.url
        ))
    })?;
    let total = r.body().content_length().or(Some(src.bytes));
    let mut body = r.into_body().into_with_config().limit(src.bytes.saturating_mul(2)).reader();
    let e = |e: std::io::Error| Error::msg(format!("{}: {e}", to.display()));
    let mut f = std::fs::File::create(to).map_err(e)?;
    let (mut buf, mut got) = (vec![0u8; 1 << 20], 0u64);
    loop {
        let k = body.read(&mut buf).map_err(|e| Error::msg(format!("{}: {e}", src.url)))?;
        if k == 0 {
            break;
        }
        f.write_all(&buf[..k]).map_err(e)?;
        got += k as u64;
        if !report(got, total) {
            return Err(Error::msg("the download was stopped"));
        }
    }
    f.flush().map_err(e)
}

// ---- a minimal zip reader (stored and deflate entries, as the export's zips are)

/// An entry of the zip's central directory.
#[derive(Debug)]
pub struct Entry {
    pub name: String,
    method: u16,
    crc: u32,
    packed: u64,
    size: u64,
    at: u64,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// The entries of a zip (not zip64: under 4 GB).
pub fn entries<R: Read + Seek>(z: &mut R) -> Result<Vec<Entry>> {
    let bad = |m: &str| Error::msg(format!("not a usable zip: {m}"));
    let io = |e: std::io::Error| Error::msg(format!("zip: {e}"));
    let len = z.seek(SeekFrom::End(0)).map_err(io)?;
    // the end record is in the last 64 KB + 22 bytes
    let tail = len.min(65_557);
    z.seek(SeekFrom::Start(len - tail)).map_err(io)?;
    let mut t = vec![0u8; tail as usize];
    z.read_exact(&mut t).map_err(io)?;
    let end = (0..t.len().saturating_sub(21))
        .rev()
        .find(|&i| t[i..i + 4] == [0x50, 0x4b, 5, 6])
        .ok_or_else(|| bad("no end of central directory"))?;
    let (n, size, off) = (u16_at(&t, end + 10) as usize, u32_at(&t, end + 12) as u64, u32_at(&t, end + 16) as u64);
    if n == 0xffff || off == 0xffff_ffff || off + size > len {
        return Err(bad("zip64 or damaged"));
    }
    z.seek(SeekFrom::Start(off)).map_err(io)?;
    let mut c = vec![0u8; size as usize];
    z.read_exact(&mut c).map_err(io)?;
    let (mut out, mut i) = (Vec::new(), 0usize);
    for _ in 0..n {
        if i + 46 > c.len() || c[i..i + 4] != [0x50, 0x4b, 1, 2] {
            return Err(bad("damaged central directory"));
        }
        let (nl, xl, cl) = (u16_at(&c, i + 28) as usize, u16_at(&c, i + 30) as usize, u16_at(&c, i + 32) as usize);
        if i + 46 + nl > c.len() {
            return Err(bad("damaged central directory"));
        }
        out.push(Entry {
            name: String::from_utf8_lossy(&c[i + 46..i + 46 + nl]).into_owned(),
            method: u16_at(&c, i + 10),
            crc: u32_at(&c, i + 16),
            packed: u32_at(&c, i + 20) as u64,
            size: u32_at(&c, i + 24) as u64,
            at: u32_at(&c, i + 42) as u64,
        });
        i += 46 + nl + xl + cl;
    }
    Ok(out)
}

/// An entry's data written to `to`, its CRC-32 checked; the size written.
pub fn extract<R: Read + Seek>(z: &mut R, e: &Entry, to: &Path) -> Result<u64> {
    let io = |x: std::io::Error| Error::msg(format!("{}: {x}", to.display()));
    z.seek(SeekFrom::Start(e.at)).map_err(io)?;
    let mut h = [0u8; 30];
    z.read_exact(&mut h).map_err(io)?;
    if h[..4] != [0x50, 0x4b, 3, 4] {
        return Err(Error::msg(format!("{}: damaged zip entry", e.name)));
    }
    let skip = u16_at(&h, 26) as i64 + u16_at(&h, 28) as i64;
    z.seek(SeekFrom::Current(skip)).map_err(io)?;
    let packed = z.take(e.packed);
    let mut src: Box<dyn Read> = match e.method {
        0 => Box::new(packed),
        8 => Box::new(flate2::read::DeflateDecoder::new(packed)),
        m => return Err(Error::msg(format!("{}: zip method {m} is not supported", e.name))),
    };
    let mut f = std::fs::File::create(to).map_err(io)?;
    let (mut crc, mut buf, mut n) = (flate2::Crc::new(), vec![0u8; 1 << 20], 0u64);
    loop {
        let k = src.read(&mut buf).map_err(|x| Error::msg(format!("{}: {x}", e.name)))?;
        if k == 0 {
            break;
        }
        crc.update(&buf[..k]);
        f.write_all(&buf[..k]).map_err(io)?;
        n += k as u64;
    }
    f.flush().map_err(io)?;
    if n != e.size || crc.sum() != e.crc {
        drop(f);
        let _ = std::fs::remove_file(to);
        return Err(Error::msg(format!("{}: the zip entry is damaged (size or CRC-32 differs)", e.name)));
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A zip of (name, data, deflate?) entries.
    fn make_zip(files: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let (mut out, mut dir) = (Vec::new(), Vec::new());
        for (name, data, deflate) in files {
            let mut crc = flate2::Crc::new();
            crc.update(data);
            let packed = if *deflate {
                let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(data).unwrap();
                e.finish().unwrap()
            } else {
                data.to_vec()
            };
            let method: u16 = if *deflate { 8 } else { 0 };
            let at = out.len() as u32;
            let common = |v: &mut Vec<u8>| {
                v.extend(method.to_le_bytes());
                v.extend([0u8; 4]); // time, date
                v.extend(crc.sum().to_le_bytes());
                v.extend((packed.len() as u32).to_le_bytes());
                v.extend((data.len() as u32).to_le_bytes());
                v.extend((name.len() as u16).to_le_bytes());
                v.extend(0u16.to_le_bytes());
            };
            out.extend([0x50, 0x4b, 3, 4, 20, 0, 0, 0]);
            common(&mut out);
            out.extend(name.as_bytes());
            out.extend(&packed);
            dir.extend([0x50, 0x4b, 1, 2, 20, 0, 20, 0, 0, 0]);
            common(&mut dir);
            dir.extend([0u8; 10]); // comment length, disk, attributes
            dir.extend(at.to_le_bytes());
            dir.extend(name.as_bytes());
        }
        let off = out.len() as u32;
        out.extend(&dir);
        out.extend([0x50, 0x4b, 5, 6, 0, 0, 0, 0]);
        out.extend((files.len() as u16).to_le_bytes());
        out.extend((files.len() as u16).to_le_bytes());
        out.extend((dir.len() as u32).to_le_bytes());
        out.extend(off.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out
    }

    fn sha(b: &[u8]) -> &'static str {
        Box::leak(Sha256::digest(b).iter().map(|b| format!("{b:02x}")).collect::<String>().into_boxed_str())
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tf_sam_weights_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn source_of(zip: &[u8]) -> Source {
        Source { name: "t", url: "test://zip", bytes: zip.len() as u64, sha256: sha(zip) }
    }

    fn files() -> Vec<(&'static str, Vec<u8>, bool)> {
        vec![
            ("m.encoder.quant.onnx", (0..5000u32).flat_map(|i| (i % 251).to_le_bytes()).collect(), true),
            ("m.decoder.quant.onnx", b"decoder bytes".to_vec(), false),
            ("config.yaml", b"x: 1".to_vec(), true),
        ]
    }

    #[test]
    fn extracts_downloads_once_and_reuses() {
        let f = files();
        let zip = make_zip(&f.iter().map(|(n, d, c)| (*n, d.as_slice(), *c)).collect::<Vec<_>>());
        let src = source_of(&zip);
        let cache = temp("reuse");
        let calls = std::cell::Cell::new(0);
        let mut dl = |_: &Source, to: &Path, r: Report| -> Result<()> {
            calls.set(calls.get() + 1);
            r(zip.len() as u64, Some(zip.len() as u64));
            std::fs::write(to, &zip).map_err(|e| Error::msg(e.to_string()))
        };
        let mut told = 0;
        let (enc, dec) = ensure(&cache, &src, &mut dl, &mut |_, _| {
            told += 1;
            true
        })
        .unwrap();
        assert_eq!(told, 1);
        assert_eq!(std::fs::read(&enc).unwrap(), f[0].1);
        assert_eq!(std::fs::read(&dec).unwrap(), f[1].1);
        // no leftovers of the download
        assert!(
            std::fs::read_dir(enc.parent().unwrap()).unwrap().all(|e| !e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with('.'))
        );
        ensure(&cache, &src, &mut dl, &mut |_, _| true).unwrap();
        assert_eq!(calls.get(), 1, "the second run must reuse the files");
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn a_damaged_file_is_downloaded_again() {
        let f = files();
        let zip = make_zip(&f.iter().map(|(n, d, c)| (*n, d.as_slice(), *c)).collect::<Vec<_>>());
        let src = source_of(&zip);
        let cache = temp("damaged");
        let calls = std::cell::Cell::new(0);
        let mut dl = |_: &Source, to: &Path, _: Report| -> Result<()> {
            calls.set(calls.get() + 1);
            std::fs::write(to, &zip).map_err(|e| Error::msg(e.to_string()))
        };
        let (enc, _) = ensure(&cache, &src, &mut dl, &mut |_, _| true).unwrap();
        std::fs::write(&enc, b"cut").unwrap();
        let (enc, _) = ensure(&cache, &src, &mut dl, &mut |_, _| true).unwrap();
        assert_eq!(calls.get(), 2);
        assert_eq!(std::fs::read(&enc).unwrap(), f[0].1);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn a_wrong_hash_is_reported_and_nothing_is_kept() {
        let f = files();
        let zip = make_zip(&f.iter().map(|(n, d, c)| (*n, d.as_slice(), *c)).collect::<Vec<_>>());
        let mut src = source_of(&zip);
        src.sha256 = "0".repeat(64).leak();
        let cache = temp("hash");
        let mut dl = |_: &Source, to: &Path, _: Report| -> Result<()> {
            std::fs::write(to, &zip).map_err(|e| Error::msg(e.to_string()))
        };
        let e = ensure(&cache, &src, &mut dl, &mut |_, _| true).unwrap_err().to_string();
        assert!(e.contains("not the expected file"), "{e}");
        let dir = cache.join("sam").join("t");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "leftovers in {}", dir.display());
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn a_zip_without_the_decoder_or_with_a_bad_crc_is_refused() {
        let cache = temp("shape");
        let zip = make_zip(&[("m.encoder.onnx", b"abc", false)]);
        let src = source_of(&zip);
        let mut dl = |_: &Source, to: &Path, _: Report| -> Result<()> {
            std::fs::write(to, &zip).map_err(|e| Error::msg(e.to_string()))
        };
        let e = ensure(&cache, &src, &mut dl, &mut |_, _| true).unwrap_err().to_string();
        assert!(e.contains("no decoder"), "{e}");
        // a flipped byte of the stored data
        let mut bad = make_zip(&[("m.encoder.onnx", b"abc", false), ("m.decoder.onnx", b"def", false)]);
        let at = bad.windows(3).position(|w| w == b"abc").unwrap();
        bad[at] = b'x';
        let src = source_of(&bad);
        let mut dl = |_: &Source, to: &Path, _: Report| -> Result<()> {
            std::fs::write(to, &bad).map_err(|e| Error::msg(e.to_string()))
        };
        let e = ensure(&cache, &src, &mut dl, &mut |_, _| true).unwrap_err().to_string();
        assert!(e.contains("damaged"), "{e}");
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn the_weights_setting_picks_the_source() {
        assert_eq!(source("quant").unwrap().bytes, 75_377_417);
        assert_eq!(source("full").unwrap().bytes, 348_403_199);
        assert!(source("big").is_err());
    }
}
