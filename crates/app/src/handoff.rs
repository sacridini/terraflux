//! A model handed to the application already open (open_in_app of `terraflux mcp`), not opened in a new window:
//! each application listens on a port of this computer only, written with a secret in a file of the application's
//! data (`instances/<process>`). The asker gives the secret and the model's file; the application opens it, unless
//! the model it has open has edits not saved.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

/// A model to open, and where the answer goes: opened, or why not.
pub struct Request {
    pub path: PathBuf,
    pub answer: mpsc::Sender<Result<(), String>>,
}

/// The application listening: the models asked for come here. Its file is removed when it goes.
pub struct Listener {
    pub requests: mpsc::Receiver<Request>,
    file: PathBuf,
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.file);
    }
}

/// The folder of the files of the applications open.
fn dir() -> PathBuf {
    crate::settings::data_dir().join("instances")
}

/// Listen for models to open (`wake` called when one comes); None when the port or the file cannot be had.
pub fn listen(wake: impl Fn() + Send + 'static) -> Option<Listener> {
    listen_in(&dir(), wake)
}

fn listen_in(dir: &Path, wake: impl Fn() + Send + 'static) -> Option<Listener> {
    let socket = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
    let port = socket.local_addr().ok()?.port();
    let secret = terraflux_dist::secure::secret();
    std::fs::create_dir_all(dir).ok()?;
    let file = dir.join(std::process::id().to_string());
    std::fs::write(&file, format!("{port} {secret}")).ok()?;
    let (tx, requests) = mpsc::channel();
    std::thread::Builder::new()
        .name("handoff".into())
        .spawn(move || {
            for conn in socket.incoming().flatten() {
                let _ = serve(conn, &secret, &tx, &wake);
            }
        })
        .ok()?;
    Some(Listener { requests, file })
}

/// One asker: a line of JSON (`secret`, `open`), a line of JSON answered (`opened`, else `why`).
fn serve(conn: TcpStream, secret: &str, tx: &mpsc::Sender<Request>, wake: &dyn Fn()) -> std::io::Result<()> {
    conn.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut line = String::new();
    BufReader::new((&conn).take(64 * 1024)).read_line(&mut line)?;
    let asked: Value = serde_json::from_str(&line).unwrap_or_default();
    let given = asked["secret"].as_str().unwrap_or_default();
    let answer = if !terraflux_dist::secure::same(given.as_bytes(), secret.as_bytes()) {
        Err("not this application's secret".to_string())
    } else if let Some(path) = asked["open"].as_str() {
        let (answer, answered) = mpsc::channel();
        let _ = tx.send(Request { path: PathBuf::from(path), answer });
        wake();
        answered.recv_timeout(Duration::from_secs(10)).unwrap_or_else(|_| Err("the application did not answer".into()))
    } else {
        Err("no model to open".to_string())
    };
    let reply = match answer {
        Ok(()) => json!({ "opened": true }),
        Err(why) => json!({ "opened": false, "why": why }),
    };
    writeln!(&conn, "{reply}")
}

/// Open the model in an application already open, the one started last first: Ok(process) where it was opened,
/// Err(why not) when none did (none open, or each with edits not saved). The files of those gone are removed.
pub fn open_in_running(model: &Path) -> Result<u32, String> {
    open_in(&dir(), model)
}

fn open_in(dir: &Path, model: &Path) -> Result<u32, String> {
    let mut found: Vec<(SystemTime, u32, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let pid = e.file_name().to_str()?.parse().ok()?;
            let at = e.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
            Some((at, pid, e.path()))
        })
        .collect();
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    let mut whys = Vec::new();
    for (_, pid, file) in found {
        match ask(&file, model) {
            Some(Ok(())) => return Ok(pid),
            Some(Err(why)) => whys.push(why),
            None => {
                let _ = std::fs::remove_file(&file);
            }
        }
    }
    Err(if whys.is_empty() { "no Terraflux open".to_string() } else { whys.join("; ") })
}

/// Ask the application of a file to open the model: None when it is not there.
fn ask(file: &Path, model: &Path) -> Option<Result<(), String>> {
    let text = std::fs::read_to_string(file).ok()?;
    let (port, secret) = text.trim().split_once(' ')?;
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port.parse().ok()?));
    let conn = TcpStream::connect_timeout(&addr, Duration::from_secs(1)).ok()?;
    conn.set_read_timeout(Some(Duration::from_secs(15))).ok()?;
    writeln!(&conn, "{}", json!({ "secret": secret, "open": model })).ok()?;
    let mut line = String::new();
    BufReader::new(&conn).read_line(&mut line).ok()?;
    let reply: Value = serde_json::from_str(&line).ok()?;
    Some(if reply["opened"].as_bool() == Some(true) {
        Ok(())
    } else {
        Err(reply["why"].as_str().unwrap_or("not opened").to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_reaches_the_application_listening_and_a_wrong_secret_does_not() {
        let dir = std::env::temp_dir().join(format!("terraflux-handoff-{}", std::process::id()));
        let listener = listen_in(&dir, || {}).unwrap();
        let model = dir.join("m.toml");
        let asker = {
            let (dir, model) = (dir.clone(), model.clone());
            std::thread::spawn(move || (open_in(&dir, &model), open_in(&dir, &model)))
        };
        let r = listener.requests.recv().unwrap();
        assert_eq!(r.path, model);
        r.answer.send(Ok(())).unwrap();
        listener.requests.recv().unwrap().answer.send(Err("edits not saved".into())).unwrap();
        let (first, second) = asker.join().unwrap();
        assert_eq!(first, Ok(std::process::id()));
        assert_eq!(second, Err("edits not saved".to_string()));
        // a wrong secret is refused; the file of an application gone is removed
        let file = dir.join(std::process::id().to_string());
        let port = std::fs::read_to_string(&file).unwrap().split_once(' ').unwrap().0.to_string();
        std::fs::write(&file, format!("{port} wrong")).unwrap();
        assert_eq!(ask(&file, &model), Some(Err("not this application's secret".to_string())));
        std::fs::write(dir.join("1"), "1 x").unwrap();
        assert!(open_in(&dir, &model).is_err());
        assert!(!dir.join("1").exists());
        drop(listener);
        assert!(!file.exists());
        assert_eq!(open_in(&dir, &model), Err("no Terraflux open".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
