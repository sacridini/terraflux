//! Any file a URL names, saved to disk like curl or wget (`data.download_file`): redirects followed, the file
//! streamed to `<name>.part` and renamed when whole, kept when already there, a zip extracted on request.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(DownloadFile));
}

/// The 16 hexadecimal digits that name the cache's folder of a URL.
pub fn url_hash(url: &str) -> String {
    Sha256::digest(url.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// A name a file can have on Windows too: no reserved characters or names, no trailing dots or spaces; `download`
/// when nothing is left.
pub fn sanitize(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
    let s: String = s.trim().trim_end_matches(['.', ' ']).chars().take(150).collect();
    if s.is_empty() || s.chars().all(|c| c == '_') {
        return "download".into();
    }
    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.ends_with(|c: char| c.is_ascii_digit()));
    if reserved { format!("_{s}") } else { s }
}

/// A text with its `%XX` sequences decoded (UTF-8; the bytes that are not kept as they are).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let (mut out, mut i) = (Vec::with_capacity(b.len()), 0);
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The last segment of the URL's path, decoded and sanitized (`download` when it has none).
pub fn name_from_url(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let path = rest.split_once('/').map_or("", |(_, p)| p);
    let last = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    sanitize(&percent_decode(last))
}

/// The file name a Content-Disposition header gives (`filename*=UTF-8''...` before `filename=`), sanitized.
pub fn name_from_disposition(h: &str) -> Option<String> {
    let mut plain = None;
    for part in h.split(';').skip(1) {
        let Some((k, v)) = part.split_once('=') else { continue };
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
        if k == "filename*" {
            // charset'language'value
            let value = v.splitn(3, '\'').nth(2).unwrap_or(v);
            let name = sanitize(base(&percent_decode(value.trim_matches('"'))));
            if name != "download" {
                return Some(name);
            }
        } else if k == "filename" && plain.is_none() {
            let name = sanitize(base(v.trim_matches('"')));
            if name != "download" {
                plain = Some(name);
            }
        }
    }
    plain
}

/// The last component of a name with folders in it.
fn base(s: &str) -> &str {
    s.rsplit(['/', '\\']).next().unwrap_or(s)
}

/// A byte count in words (`12.3 MB`).
fn size(n: u64) -> String {
    let n = n as f64;
    match n {
        n if n >= 1e9 => format!("{:.2} GB", n / 1e9),
        n if n >= 1e6 => format!("{:.1} MB", n / 1e6),
        n if n >= 1e3 => format!("{:.0} kB", n / 1e3),
        n => format!("{n:.0} B"),
    }
}

/// Where the file goes: exactly there, or in a folder under the name the server gives.
enum Target {
    File(PathBuf),
    /// The folder, and whether Terraflux made it for this URL alone (what is in it may be this download's).
    Dir(PathBuf, bool),
}

struct DownloadFile;

impl Operator for DownloadFile {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "data.download_file",
            "Any file a URL names saved to disk, like curl or wget (http or https; redirects followed; one \
             connection): for a raster GDAL reads and a grid to warp it onto, data.download. The file is named by \
             the server's Content-Disposition, else the last part of the URL's path; it is written as <name>.part \
             and renamed when whole, so a failure or a stop leaves no half file. Without a path it goes to \
             Terraflux's cache (TERRAFLUX_CACHE), in downloads/<16 hex digits of the url>/; a file already there is \
             kept without asking the server, unless refresh. The path it gives can feed a setting of another node \
             (raster.read's path, as { from = \"download.path\" }); with unzip, a .zip is extracted into a folder \
             beside it (named as the zip without .zip) and that folder is the path.",
            Access::Global,
        )
        .output(PortSpec::new("path", Type::Text, "the file saved (the folder, with unzip)"))
        .param(PortSpec::new("url", Type::Text, "the address to download: http:// or https://"))
        .param(PortSpec::optional(
            "path",
            Type::Text,
            "where to save it: a file path, or an existing folder (or one ending in / or \\) to save into under the \
             file's name (default Terraflux's cache, downloads/<hash of the url>/)",
        ))
        .param(PortSpec::optional("unzip", Type::Bool, "extract a .zip into a folder beside it (default false)"))
        .param(PortSpec::optional(
            "refresh",
            Type::Bool,
            "download again even if the file is there (default false)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let url: String = args.param("url", String::new())?;
        let url = url.trim().to_string();
        if url.is_empty() {
            return Err(fail("set the url: the address to download (http:// or https://)".into()));
        }
        let lower = url.to_ascii_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://")) {
            return Err(fail(format!(
                "the url must start with http:// or https:// ({url:?}): other schemes are not downloaded"
            )));
        }
        let unzip: bool = args.param("unzip", false)?;
        let refresh: bool = args.param("refresh", false)?;
        let given: Option<String> = args.opt_param("path")?;
        let target = match given.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
            Some(p) => {
                let pb = PathBuf::from(p);
                if p.ends_with(['/', '\\']) || pb.is_dir() { Target::Dir(pb, false) } else { Target::File(pb) }
            }
            None => Target::Dir(crate::cache::root().join("downloads").join(url_hash(&url)), true),
        };
        let from_url = name_from_url(&url);

        // already there: no network at all
        if !refresh && let Some(file) = kept(&target, &from_url) {
            ctx.progress(Some(1.0), format!("kept {} (already downloaded)", file.display()));
            return finish(ctx, args, &file, unzip, false);
        }

        let (file, got) = fetch(ctx, args, &url, &target, &from_url, refresh)?;
        finish(ctx, args, &file, unzip, got)
    }
}

/// The file already saved for this target, if there is one.
fn kept(target: &Target, from_url: &str) -> Option<PathBuf> {
    match target {
        Target::File(f) => f.is_file().then(|| f.clone()),
        Target::Dir(d, own) => {
            let f = d.join(from_url);
            if f.is_file() {
                return Some(f);
            }
            if *own {
                // a name the server gave: the only file of the folder (not a half one, not a hidden one)
                return std::fs::read_dir(d).ok()?.flatten().map(|e| e.path()).find(|p| {
                    let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    p.is_file() && !n.starts_with('.') && !n.ends_with(".part")
                });
            }
            None
        }
    }
}

/// The request, its answer checked, and the body written to the file: the file and whether it was downloaded now
/// (not kept after learning its name).
fn fetch(
    ctx: &mut Ctx,
    args: &Args,
    url: &str,
    target: &Target,
    from_url: &str,
    refresh: bool,
) -> Result<(PathBuf, bool)> {
    use ureq::tls::{RootCerts, TlsConfig};
    let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .timeout_global(None)
        .http_status_as_error(false)
        .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
        .user_agent(format!("Terraflux/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    if !ctx.progress(None, format!("connecting to {url}")) {
        return Err(fail("stopped".into()));
    }
    let r = agent.get(url).call().map_err(|e| match e {
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed | ureq::Error::Timeout(_) => {
            fail(format!("{url} cannot be reached ({e}): is the computer online, and the address right?"))
        }
        e => fail(format!("{url}: {e}")),
    })?;
    let status = r.status();
    if !status.is_success() {
        let why = status.canonical_reason().unwrap_or("");
        return Err(fail(format!("the server answered {} {why} for {url}", status.as_u16())));
    }
    let name = r
        .headers()
        .get("content-disposition")
        .and_then(|h| h.to_str().ok())
        .and_then(name_from_disposition)
        .unwrap_or_else(|| from_url.to_string());
    let file = match target {
        Target::File(f) => f.clone(),
        Target::Dir(d, _) => d.join(&name),
    };
    // the server's name found a file already there: the body is not read
    if !refresh && file.is_file() {
        ctx.progress(Some(1.0), format!("kept {} (already downloaded)", file.display()));
        return Ok((file, false));
    }
    if let Some(dir) = file.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| fail(format!("{}: {e}", dir.display())))?;
    }
    let part = PathBuf::from(format!("{}.part", file.display()));
    let total = r.body().content_length();
    let mut body = r.into_body().into_with_config().limit(u64::MAX).reader();
    let written = (|| -> Result<()> {
        let e = |e: std::io::Error| fail(format!("{}: {e}", part.display()));
        let mut f = std::fs::File::create(&part).map_err(e)?;
        let (mut buf, mut got, mut last) = (vec![0u8; 1 << 20], 0u64, Instant::now());
        loop {
            let k = body.read(&mut buf).map_err(|x| fail(format!("{url}: the download broke off ({x})")))?;
            if k == 0 {
                break;
            }
            f.write_all(&buf[..k]).map_err(e)?;
            got += k as u64;
            if last.elapsed() >= Duration::from_millis(200) {
                last = Instant::now();
                let text = match total {
                    Some(t) => format!("downloading {name}: {} of {}", size(got), size(t)),
                    None => format!("downloading {name}: {}", size(got)),
                };
                if !ctx.progress(total.map(|t| got as f64 / t.max(1) as f64), text) {
                    return Err(fail("stopped".into()));
                }
            }
        }
        f.flush().map_err(e)?;
        if let Some(t) = total
            && got != t
        {
            return Err(fail(format!("{url}: the server sent {got} bytes of the {t} it announced")));
        }
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, &file).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        fail(format!("{}: {e}", file.display()))
    })?;
    Ok((file, true))
}

/// The output: the file, or with `unzip` the folder its zip was extracted to (left as it is when it is there and
/// the file was not downloaded now).
fn finish(ctx: &mut Ctx, args: &Args, file: &Path, unzip: bool, downloaded: bool) -> Result<Outputs> {
    let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
    if !unzip {
        return Ok(outputs([("path", Value::Text(file.to_string_lossy().into_owned()))]));
    }
    let s = file.to_string_lossy().into_owned();
    let dir = match s.strip_suffix(".zip").or_else(|| s.strip_suffix(".ZIP")) {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => PathBuf::from(format!("{s}_unzipped")),
    };
    if dir.is_dir() && !downloaded {
        return Ok(outputs([("path", Value::Text(dir.to_string_lossy().into_owned()))]));
    }
    ctx.progress(None, format!("extracting {}", file.display()));
    let part = PathBuf::from(format!("{}.part", dir.display()));
    let _ = std::fs::remove_dir_all(&part);
    let done = extract_all(ctx, file, &part);
    if let Err(e) = done {
        let _ = std::fs::remove_dir_all(&part);
        return Err(fail(e));
    }
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir).map_err(|e| fail(format!("{}: {e}", dir.display())))?;
    }
    std::fs::rename(&part, &dir).map_err(|e| {
        let _ = std::fs::remove_dir_all(&part);
        fail(format!("{}: {e}", dir.display()))
    })?;
    Ok(outputs([("path", Value::Text(dir.to_string_lossy().into_owned()))]))
}

/// Every entry of the zip written under `to` (those that would leave it refused).
fn extract_all(ctx: &mut Ctx, zip: &Path, to: &Path) -> std::result::Result<(), String> {
    let mut f = std::fs::File::open(zip).map_err(|e| format!("{}: {e}", zip.display()))?;
    let entries = crate::objects::weights::entries(&mut f).map_err(|e| format!("{}: {e}", zip.display()))?;
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for (k, e) in entries.iter().enumerate() {
        if !ctx.progress(Some(k as f64 / entries.len() as f64), format!("extracting {}", e.name)) {
            return Err("stopped".into());
        }
        let parts: Vec<&str> = e.name.split(['/', '\\']).filter(|p| !p.is_empty() && *p != ".").collect();
        if parts.iter().any(|p| *p == ".." || p.contains(':')) || e.name.starts_with(['/', '\\']) {
            return Err(format!("the zip holds {:?}, a path out of its folder: not extracted", e.name));
        }
        let out = parts.iter().fold(to.to_path_buf(), |p, c| p.join(c));
        if e.name.ends_with(['/', '\\']) {
            std::fs::create_dir_all(&out).map_err(|x| format!("{}: {x}", out.display()))?;
            continue;
        }
        if let Some(d) = out.parent() {
            std::fs::create_dir_all(d).map_err(|x| format!("{}: {x}", d.display()))?;
        }
        crate::objects::weights::extract(&mut f, e, &out).map_err(|x| x.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(name_from_url("https://x.org/a/b/data%20file.tif?v=1#top"), "data file.tif");
        assert_eq!(name_from_url("https://x.org/"), "download");
        assert_eq!(name_from_url("https://x.org"), "download");
        assert_eq!(name_from_url("http://x.org/a/b/"), "b");
        assert_eq!(sanitize("a<b>:c?.txt"), "a_b__c_.txt");
        assert_eq!(sanitize("CON.txt"), "_CON.txt");
        assert_eq!(sanitize("name. "), "name");
        assert_eq!(sanitize("???"), "download");
        assert_eq!(url_hash("https://x.org/a").len(), 16);
    }

    #[test]
    fn disposition() {
        let n = |h| name_from_disposition(h);
        assert_eq!(n("attachment; filename=\"report.csv\""), Some("report.csv".into()));
        assert_eq!(n("attachment; filename=plain.zip"), Some("plain.zip".into()));
        assert_eq!(n("attachment; filename=\"a.txt\"; filename*=UTF-8''caf%C3%A9.txt"), Some("café.txt".into()));
        assert_eq!(n("attachment; filename=\"../../evil.sh\""), Some("evil.sh".into()));
        assert_eq!(n("inline"), None);
    }
}
