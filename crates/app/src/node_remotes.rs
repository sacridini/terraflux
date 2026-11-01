//! Libraries of nodes from elsewhere, and who saves in a shared one.
//!
//! A remote library is a git repository (cloned, then pulled; `git` on the PATH) or a folder on the web (read by
//! its `index.txt`, a node's file a line, which Terraflux writes in every library it saves in, so that a shared
//! folder put on a web server is one as it is). A copy is kept in the application's data, pulled when the
//! application starts and when asked. It is read, never written: a node is added to a repository in a clone of it
//! (a shared library of the settings), pushed.
//!
//! A library may have a `library.toml`: its `name`, and `writers`, the user names of those who save in it and take
//! nodes out of it (none: anyone who can write its folder). The user is this computer's login
//! (`TERRAFLUX_USER` when set).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::Deserialize;

/// A library's own file: its name and who saves in it.
pub const ABOUT: &str = "library.toml";

/// The list of a library's files, a line each: what a library on the web is read by.
pub const INDEX: &str = "index.txt";

/// What a library's `library.toml` says.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct About {
    pub name: Option<String>,
    /// The user names of those who save in it (none: anyone).
    pub writers: Vec<String>,
}

/// What a library's folder's `library.toml` says (nothing when it has none, or it does not read).
pub fn about(dir: &Path) -> About {
    std::fs::read_to_string(dir.join(ABOUT)).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
}

/// Who the user is to a library's writers: `TERRAFLUX_USER`, else this computer's login.
pub fn user() -> String {
    ["TERRAFLUX_USER", "USERNAME", "USER"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.trim().is_empty()))
        .unwrap_or_default()
}

/// Whether the user saves in a library's folder (and takes nodes out of it): refused, with why, when its
/// `library.toml` names its writers and the user is not one of them.
pub fn may_save(dir: &Path) -> Result<(), String> {
    let a = about(dir);
    if a.writers.is_empty() {
        return Ok(());
    }
    let me = user();
    if a.writers.iter().any(|w| w.trim().eq_ignore_ascii_case(me.trim())) {
        return Ok(());
    }
    Err(format!("only {} save in this library (its {ABOUT}), and you are {me:?}", a.writers.join(", ")))
}

/// Whether a file's name is a version of a node (`name@3.toml`): nothing else is listed or fetched.
pub fn node_file(f: &str) -> bool {
    f.strip_suffix(".toml")
        .and_then(|s| s.rsplit_once('@'))
        .is_some_and(|(n, v)| terraflux_graph::valid_id(n) && v.parse::<u32>().is_ok())
}

/// Write the index of a library's folder ([`INDEX`]): the file of each version of its nodes, a line each.
pub fn write_index(dir: &Path) -> std::io::Result<()> {
    let mut files: Vec<String> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| node_file(n))
        .collect();
    files.sort();
    std::fs::write(dir.join(INDEX), files.iter().map(|f| format!("{f}\n")).collect::<String>())
}

/// Whether an address is a git repository's (else a folder on the web): `.git` at its end, `git@`, `ssh://`,
/// `git://`, or a repository's page of GitHub, GitLab, Codeberg or Bitbucket (`https://github.com/lab/nodes`).
pub fn is_git(url: &str) -> bool {
    let u = url.trim().trim_end_matches('/');
    if u.ends_with(".git") || u.starts_with("git@") || u.starts_with("ssh://") || u.starts_with("git://") {
        return true;
    }
    let rest = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://"));
    rest.is_some_and(|r| {
        let parts: Vec<&str> = r.split('/').collect();
        parts.len() == 3 && ["github.com", "gitlab.com", "codeberg.org", "bitbucket.org"].contains(&parts[0])
    })
}

/// A folder on the web's address, ending in `/` (given as its index or its `library.toml` too).
fn folder(url: &str) -> String {
    let u = url.trim();
    let u = u.strip_suffix(INDEX).or_else(|| u.strip_suffix(ABOUT)).unwrap_or(u);
    if u.ends_with('/') { u.to_string() } else { format!("{u}/") }
}

/// Where the copy of a remote library is kept on this computer: a folder of the application's data named by its
/// address.
pub fn dir_of(url: &str) -> PathBuf {
    crate::settings::data_dir().join("remote nodes").join(slug(url))
}

/// A folder's name for an address: its last part and a hash of it all (two of the same last part apart).
fn slug(url: &str) -> String {
    let u = url.trim().trim_end_matches('/');
    let last: String = short(u).chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    // FNV-1a: the same address, the same folder, from one version to the next
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in u.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    format!("{last}-{:08x}", h as u32)
}

/// The last part of an address, without `.git`.
fn short(url: &str) -> &str {
    let u = url.trim().trim_end_matches('/');
    let u = u.strip_suffix(INDEX).or_else(|| u.strip_suffix(ABOUT)).unwrap_or(u).trim_end_matches('/');
    let last = u.rsplit(['/', ':']).next().unwrap_or(u);
    last.strip_suffix(".git").unwrap_or(last)
}

/// What a remote library is called: its `library.toml`'s name, else the last part of its address.
pub fn name_of(url: &str) -> String {
    about(&dir_of(url)).name.unwrap_or_else(|| short(url).to_string())
}

/// Fetch a remote library (clone or pull it, or read its index and the versions not here yet) into its copy
/// ([`dir_of`]): what it has then.
pub fn pull(url: &str) -> Result<String, String> {
    pull_into(url, &dir_of(url))
}

/// [`pull`] into a folder.
pub fn pull_into(url: &str, dir: &Path) -> Result<String, String> {
    let new = if is_git(url) { pull_git(url, dir)? } else { pull_web(url, dir)? };
    let n = terraflux_graph::user_node::Library::new(dir).list().len();
    let nodes = if n == 1 { "1 node".to_string() } else { format!("{n} nodes") };
    Ok(match new {
        Some(k) if k > 0 => format!("{nodes}, {k} new version(s)"),
        _ => nodes,
    })
}

fn git(args: &[&str]) -> Result<(), String> {
    let mut c = std::process::Command::new("git");
    c.args(args).stdin(Stdio::null()).env("GIT_TERMINAL_PROMPT", "0");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    let out = c.output().map_err(|e| {
        format!("git did not run ({e}): install git, or clone the repository and add its folder as a shared library")
    })?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("git {}: {}", args.iter().find(|a| !a.starts_with('-') && **a != "-C").unwrap_or(&""), {
            String::from_utf8_lossy(&out.stderr).trim().to_string()
        }))
    }
}

/// Clone a repository, or pull it when it is cloned (only forward: the copy is never changed here).
fn pull_git(url: &str, dir: &Path) -> Result<Option<usize>, String> {
    let d = dir.to_string_lossy().into_owned();
    if dir.join(".git").is_dir() {
        git(&["-C", &d, "pull", "--ff-only", "--quiet"])?;
    } else {
        // what a clone that failed left
        if dir.exists() {
            std::fs::remove_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        if let Some(p) = dir.parent() {
            std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        git(&["clone", "--depth", "1", "--quiet", url.trim(), &d])?;
    }
    Ok(None)
}

fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig};
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
        .user_agent(format!("Terraflux/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn get(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    let mut res = agent.get(url).call().map_err(|e| format!("{url}: {e}"))?;
    res.body_mut().read_to_string().map_err(|e| format!("{url}: {e}"))
}

/// Read a folder on the web: its index, and each version listed not here yet (a version never changes once
/// saved); its `library.toml` when it has one. How many versions were new.
fn pull_web(url: &str, dir: &Path) -> Result<Option<usize>, String> {
    let base = folder(url);
    if !(base.starts_with("https://") || base.starts_with("http://")) {
        return Err(format!("{url}: not a web address (https://...) nor a git repository's"));
    }
    let a = agent();
    let index = get(&a, &format!("{base}{INDEX}"))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut new = 0;
    for line in index.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        // a node's file, nothing else (no folder, no other file)
        if !node_file(line) {
            continue;
        }
        let to = dir.join(line);
        if to.is_file() {
            continue;
        }
        let text = get(&a, &format!("{base}{line}"))?;
        terraflux_graph::UserNode::from_toml(&text).map_err(|e| format!("{base}{line}: {e}"))?;
        let part = dir.join(format!("{line}.part"));
        std::fs::write(&part, text).and_then(|_| std::fs::rename(&part, &to)).map_err(|e| format!("{line}: {e}"))?;
        new += 1;
    }
    // its name (and writers, which say nothing here: it is only read)
    if let Ok(t) = get(&a, &format!("{base}{ABOUT}"))
        && toml::from_str::<About>(&t).is_ok()
    {
        let _ = std::fs::write(dir.join(ABOUT), t);
    }
    Ok(Some(new))
}

/// Where the fetching of a remote library is: under way, or what it gave.
#[derive(Debug, Clone, PartialEq)]
pub enum Pulled {
    Pulling,
    Done(Result<String, String>),
}

fn states() -> &'static Mutex<BTreeMap<String, Pulled>> {
    static S: OnceLock<Mutex<BTreeMap<String, Pulled>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Where the fetching of a remote library is, since the application started (none: not fetched yet).
pub fn state(url: &str) -> Option<Pulled> {
    states().lock().unwrap().get(url).cloned()
}

/// Fetch a remote library in the background (unless it is being fetched): its nodes read again when it is done.
pub fn pull_later(url: &str, wake: impl Fn() + Send + 'static) {
    {
        let mut s = states().lock().unwrap();
        if s.get(url) == Some(&Pulled::Pulling) {
            return;
        }
        s.insert(url.to_string(), Pulled::Pulling);
    }
    let url = url.to_string();
    std::thread::spawn(move || {
        let r = pull(&url);
        states().lock().unwrap().insert(url, Pulled::Done(r));
        crate::my_nodes::changed();
        wake();
    });
}

/// Fetch every remote library of the settings in the background (when the application starts).
pub fn pull_all(wake: impl Fn() + Clone + Send + 'static) {
    for url in crate::settings::get().node_remotes {
        pull_later(&url, wake.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_told_apart() {
        for g in [
            "https://github.com/lab/nodes",
            "https://github.com/lab/nodes/",
            "https://gitlab.example.org/lab/nodes.git",
            "git@github.com:lab/nodes.git",
            "ssh://git@host/lab/nodes",
        ] {
            assert!(is_git(g), "{g}");
        }
        for w in ["https://lab.example.org/nodes/", "https://lab.github.io/nodes/index.txt", "https://github.com/lab"] {
            assert!(!is_git(w), "{w}");
        }
        assert_eq!(folder("https://x.org/n/index.txt"), "https://x.org/n/");
        assert_eq!(folder("https://x.org/n"), "https://x.org/n/");
        assert_eq!(short("git@github.com:lab/nodes.git"), "nodes");
        assert_eq!(short("https://x.org/lab-nodes/library.toml"), "lab-nodes");
        // the same address the same folder; another, another
        assert_eq!(slug("https://github.com/lab/nodes"), slug("https://github.com/lab/nodes/"));
        assert_ne!(slug("https://github.com/lab/nodes"), slug("https://github.com/other/nodes"));
        assert!(slug("https://github.com/lab/nodes").starts_with("nodes-"));
        assert!(node_file("ndvi@3.toml") && !node_file("../ndvi@3.toml") && !node_file("library.toml"));
        assert!(!node_file("ndvi@x.toml") && !node_file("a/b@1.toml"));
    }

    #[test]
    fn writers_say_who_saves() {
        let dir = std::env::temp_dir().join(format!("terraflux-writers-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // no library.toml, or no writers: anyone
        assert!(may_save(&dir).is_ok());
        std::fs::write(dir.join(ABOUT), "name = \"Lab\"\n").unwrap();
        assert!(may_save(&dir).is_ok());
        assert_eq!(about(&dir).name.as_deref(), Some("Lab"));
        std::fs::write(dir.join(ABOUT), format!("writers = [\"{}\", \"ana\"]\n", user().to_uppercase())).unwrap();
        assert!(may_save(&dir).is_ok());
        std::fs::write(dir.join(ABOUT), "writers = [\"someone-else-entirely\"]\n").unwrap();
        let e = may_save(&dir).unwrap_err();
        assert!(e.contains("someone-else-entirely"), "{e}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn twice(name: &str) -> terraflux_graph::UserNode {
        let mut uses = terraflux_graph::Uses {
            name: name.into(),
            version: 0,
            description: None,
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
        };
        uses.inputs.insert("x".into(), terraflux_graph::UserInput::new(terraflux_graph::Type::Map));
        uses.outputs.insert("out".into(), terraflux_graph::UserOutput::new(terraflux_graph::Type::Map));
        let body = terraflux_graph::Body {
            nodes: vec![
                terraflux_graph::Node::new("c", "raster.calc").input("x", "@x").param("expression", "out y = x * 2"),
            ],
            outputs: [("out".to_string(), "c.y".to_string())].into(),
            ..Default::default()
        };
        terraflux_graph::UserNode::new(uses, body)
    }

    #[test]
    fn a_repository_is_cloned_then_pulled() {
        let run = |dir: &Path, args: &[&str]| {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@example.org", "-c", "commit.gpgsign=false"])
                .args(args)
                .output()
                .is_ok_and(|o| o.status.success());
            assert!(ok, "git {args:?}");
        };
        if std::process::Command::new("git").arg("--version").output().is_err() {
            eprintln!("no git: not tried");
            return;
        }
        let root = std::env::temp_dir().join(format!("terraflux-git-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // a repository of nodes (its folder named as a repository's address is, `.git` at its end)
        let repo = root.join("nodes.git");
        let copy = root.join("copy");
        let l = terraflux_graph::user_node::Library::new(&repo);
        l.save(&twice("twice")).unwrap();
        run(&repo, &["init", "--quiet"]);
        run(&repo, &["add", "."]);
        run(&repo, &["commit", "--quiet", "-m", "twice"]);
        let url = repo.to_string_lossy().into_owned();
        assert!(is_git(&url));
        assert_eq!(pull_into(&url, &copy).unwrap(), "1 node");
        // a node pushed to it: pulled
        l.save(&twice("thrice")).unwrap();
        run(&repo, &["add", "."]);
        run(&repo, &["commit", "--quiet", "-m", "thrice"]);
        assert_eq!(pull_into(&url, &copy).unwrap(), "2 nodes");
        let names: Vec<String> =
            terraflux_graph::user_node::Library::new(&copy).list().into_iter().map(|u| u.name).collect();
        assert_eq!(names, ["thrice", "twice"]);
        // an address that is not a repository: git's own words
        let e = pull_into(&root.join("none.git").to_string_lossy(), &root.join("copy2")).unwrap_err();
        assert!(e.starts_with("git clone:"), "{e}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_on_the_web_is_read_by_its_index() {
        // a library's folder, its index written, served over http (on this computer)
        let lib = std::env::temp_dir().join(format!("terraflux-web-lib-{}", std::process::id()));
        let copy = std::env::temp_dir().join(format!("terraflux-web-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&lib);
        let _ = std::fs::remove_dir_all(&copy);
        terraflux_graph::user_node::Library::new(&lib).save(&twice("twice")).unwrap();
        std::fs::write(lib.join(ABOUT), "name = \"Twice lab\"\n").unwrap();
        write_index(&lib).unwrap();
        assert_eq!(std::fs::read_to_string(lib.join(INDEX)).unwrap(), "twice@1.toml\n");
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = server.local_addr().unwrap().port();
        let served = lib.clone();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader, Write};
            for s in server.incoming().flatten() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut first = String::new();
                r.read_line(&mut first).unwrap();
                loop {
                    let mut l = String::new();
                    if r.read_line(&mut l).unwrap() == 0 || l == "\r\n" {
                        break;
                    }
                }
                let path = first.split(' ').nth(1).unwrap_or("/").trim_start_matches("/lib/").to_string();
                let mut s = s;
                match std::fs::read(served.join(&path)) {
                    Ok(b) => {
                        let _ =
                            write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", b.len());
                        let _ = s.write_all(&b);
                    }
                    Err(_) => {
                        let _ = write!(s, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    }
                }
            }
        });
        let url = format!("http://127.0.0.1:{port}/lib/");
        assert!(!is_git(&url));
        assert_eq!(pull_into(&url, &copy).unwrap(), "1 node, 1 new version(s)");
        let got = terraflux_graph::user_node::Library::new(&copy).list();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "twice");
        assert_eq!(about(&copy).name.as_deref(), Some("Twice lab"));
        // again: nothing new
        assert_eq!(pull_into(&url, &copy).unwrap(), "1 node");
        std::fs::remove_dir_all(&lib).unwrap();
        std::fs::remove_dir_all(&copy).unwrap();
    }
}
