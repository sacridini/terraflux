//! data.download_file against a web server on 127.0.0.1 (a port the system chose): the name of the file, the
//! redirects, the errors, a zip extracted, the file kept without a request, and refresh.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use terraflux_ops::{Args, Ctx, Registry, Value};

/// A zip of stored entries (name, data).
fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let (mut out, mut dir) = (Vec::new(), Vec::new());
    for (name, data) in files {
        let mut crc = flate2::Crc::new();
        crc.update(data);
        let at = out.len() as u32;
        let common = |v: &mut Vec<u8>| {
            v.extend(0u16.to_le_bytes());
            v.extend([0u8; 4]);
            v.extend(crc.sum().to_le_bytes());
            v.extend((data.len() as u32).to_le_bytes());
            v.extend((data.len() as u32).to_le_bytes());
            v.extend((name.len() as u16).to_le_bytes());
            v.extend(0u16.to_le_bytes());
        };
        out.extend([0x50, 0x4b, 3, 4, 20, 0, 0, 0]);
        common(&mut out);
        out.extend(name.as_bytes());
        out.extend(*data);
        dir.extend([0x50, 0x4b, 1, 2, 20, 0, 20, 0, 0, 0]);
        common(&mut dir);
        dir.extend([0u8; 6]); // comment length, disk, internal attributes
        dir.extend(0u32.to_le_bytes());
        dir.extend(at.to_le_bytes());
        dir.extend(name.as_bytes());
    }
    let at = out.len() as u32;
    out.extend(&dir);
    out.extend([0x50, 0x4b, 5, 6, 0, 0, 0, 0]);
    out.extend((files.len() as u16).to_le_bytes());
    out.extend((files.len() as u16).to_le_bytes());
    out.extend((dir.len() as u32).to_le_bytes());
    out.extend(at.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

/// The server's port and the requests it has had.
fn serve() -> (u16, Arc<AtomicUsize>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let seen = seen.clone();
            std::thread::spawn(move || {
                let mut out = stream.try_clone().unwrap();
                let mut input = BufReader::new(stream);
                let mut first = String::new();
                if input.read_line(&mut first).unwrap_or(0) == 0 {
                    return;
                }
                loop {
                    let mut l = String::new();
                    if input.read_line(&mut l).unwrap_or(0) == 0 || l == "\r\n" {
                        break;
                    }
                }
                seen.fetch_add(1, Ordering::SeqCst);
                let target = first.split_whitespace().nth(1).unwrap_or("/").to_string();
                let zip = zip_of(&[("a.txt", b"alpha"), ("sub/", b""), ("sub/b.txt", b"beta")]);
                let (status, extra, body): (&str, String, Vec<u8>) = match target.as_str() {
                    "/files/data%20set.txt" => ("200 OK", String::new(), b"hello".to_vec()),
                    "/cd" => (
                        "200 OK",
                        "Content-Disposition: attachment; filename=\"named.csv\"\r\n".into(),
                        b"a,b\n1,2\n".to_vec(),
                    ),
                    "/redir" => ("302 Found", "Location: /files/data%20set.txt\r\n".into(), Vec::new()),
                    "/pack.zip" => ("200 OK", String::new(), zip),
                    "/new" => ("200 OK", String::new(), b"v2".to_vec()),
                    _ => ("404 Not Found", String::new(), b"nothing here".to_vec()),
                };
                let head =
                    format!("HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = out.write_all(head.as_bytes());
                let _ = out.write_all(&body);
                let _ = out.flush();
            });
        }
    });
    (port, count)
}

fn work(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("terraflux-dlfile-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(params: serde_json::Value) -> Result<String, String> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "dl", inputs: Default::default(), params: &params };
    match r.get("data.download_file").unwrap().run(&mut Ctx::default(), &args) {
        Ok(o) => match o.get("path") {
            Some(Value::Text(t)) => Ok(t.clone()),
            v => panic!("{v:?}"),
        },
        Err(e) => Err(e.to_string()),
    }
}

fn no_part(dir: &Path) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        assert!(!e.file_name().to_string_lossy().ends_with(".part"), "{:?} is left", e.path());
    }
}

#[test]
fn a_file_named_by_the_url_kept_and_refreshed() {
    let (port, count) = serve();
    let dir = work("plain");
    let url = format!("http://127.0.0.1:{port}/files/data%20set.txt");
    let p = run(serde_json::json!({ "url": url, "path": dir.to_string_lossy() })).unwrap();
    assert_eq!(PathBuf::from(&p), dir.join("data set.txt"));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "hello");
    assert_eq!(count.load(Ordering::SeqCst), 1);
    no_part(&dir);
    // the second run: kept, no request
    let q = run(serde_json::json!({ "url": url, "path": dir.to_string_lossy() })).unwrap();
    assert_eq!(q, p);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    // refresh: asked again
    std::fs::write(&p, "old").unwrap();
    run(serde_json::json!({ "url": url, "path": dir.to_string_lossy(), "refresh": true })).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "hello");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_name_of_the_content_disposition_and_a_redirect() {
    let (port, _) = serve();
    let dir = work("cd");
    let p = run(serde_json::json!({ "url": format!("http://127.0.0.1:{port}/cd"), "path": dir.to_string_lossy() }))
        .unwrap();
    assert_eq!(PathBuf::from(&p), dir.join("named.csv"));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "a,b\n1,2\n");
    let p = run(serde_json::json!({ "url": format!("http://127.0.0.1:{port}/redir"), "path": dir.to_string_lossy() }))
        .unwrap();
    assert_eq!(PathBuf::from(&p), dir.join("redir"));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "hello");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_path_that_is_a_file_and_the_default_in_the_cache() {
    let (port, count) = serve();
    let dir = work("file");
    let to = dir.join("deep").join("mine.bin");
    let url = format!("http://127.0.0.1:{port}/new");
    let p = run(serde_json::json!({ "url": url, "path": to.to_string_lossy() })).unwrap();
    assert_eq!(PathBuf::from(&p), to);
    assert_eq!(std::fs::read_to_string(&to).unwrap(), "v2");
    // no path: the cache's downloads/<hash>/new, kept the second time (a url of this test's port alone)
    let a = run(serde_json::json!({ "url": url })).unwrap();
    let hash = terraflux_ops::download_file::url_hash(&url);
    assert!(a.contains("downloads") && a.contains(&hash) && a.ends_with("new"), "{a}");
    assert_eq!(count.load(Ordering::SeqCst), 2);
    let b = run(serde_json::json!({ "url": url })).unwrap();
    assert_eq!(a, b);
    assert_eq!(count.load(Ordering::SeqCst), 2);
    std::fs::remove_dir_all(PathBuf::from(&a).parent().unwrap()).ok();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_error_of_the_server_leaves_no_half_file() {
    let (port, _) = serve();
    let dir = work("e404");
    let url = format!("http://127.0.0.1:{port}/gone.txt");
    let e = run(serde_json::json!({ "url": url, "path": dir.to_string_lossy() })).unwrap_err();
    assert!(e.contains("node \"dl\"") && e.contains("the server answered 404 Not Found for"), "{e}");
    assert!(e.contains(&url), "{e}");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    let e = run(serde_json::json!({ "url": "ftp://example.org/x" })).unwrap_err();
    assert!(e.contains("http:// or https://"), "{e}");
    assert!(run(serde_json::json!({})).unwrap_err().contains("set the url"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_zip_is_extracted_beside_itself() {
    let (port, count) = serve();
    let dir = work("zip");
    let url = format!("http://127.0.0.1:{port}/pack.zip");
    let p = run(serde_json::json!({ "url": url, "path": dir.to_string_lossy(), "unzip": true })).unwrap();
    assert_eq!(PathBuf::from(&p), dir.join("pack"));
    assert_eq!(std::fs::read_to_string(dir.join("pack").join("a.txt")).unwrap(), "alpha");
    assert_eq!(std::fs::read_to_string(dir.join("pack").join("sub").join("b.txt")).unwrap(), "beta");
    assert!(dir.join("pack.zip").is_file());
    no_part(&dir);
    // again: the folder kept, no request
    let q = run(serde_json::json!({ "url": url, "path": dir.to_string_lossy(), "unzip": true })).unwrap();
    assert_eq!(q, p);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    // without unzip: the zip itself
    let z = run(serde_json::json!({ "url": url, "path": dir.to_string_lossy() })).unwrap();
    assert_eq!(PathBuf::from(&z), dir.join("pack.zip"));
    std::fs::remove_dir_all(&dir).ok();
}
