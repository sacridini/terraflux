//! Help > Tutorials with a model...: the website's tutorials by category, each to read (its page) and, when it has a
//! model, to open: a template of the application (as the Templates menu opens it) or the model files published with
//! the tutorial, downloaded from the website into a folder of the application's data (the data the models read, next
//! to them) and opened as File > Open does. The download is one thread per model; a failure is told in the row.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use egui::{RichText, Vec2};
use terraflux_graph::Model;

use crate::templates::{self, Template};
use crate::tutorials::{self, Tutorial};

/// What a tutorial's model is: a template of the application, or a file published on the website (its name).
pub enum Way {
    Template(&'static Template),
    File(String),
}

/// What the window asks of the application.
pub enum Act {
    /// A template's model, started as a new model.
    Start(Box<Model>),
    /// A model file downloaded, opened.
    Open(PathBuf),
}

/// What is done to get a published model: the files of the tutorial's folder to download (the path in the site and
/// where it goes) and the model to open once they are all there.
#[derive(Debug, PartialEq)]
pub struct Plan {
    pub fetch: Vec<(String, PathBuf)>,
    pub open: PathBuf,
}

/// The ways to open a tutorial's model: its template, then each model file published with it.
pub fn ways(t: &Tutorial) -> Vec<Way> {
    let template = t.template.as_deref().and_then(|n| templates::TEMPLATES.iter().find(|x| x.name == n));
    let files = t.models.iter().filter_map(|m| m.rsplit('/').next()).map(|n| Way::File(n.to_string()));
    template.map(Way::Template).into_iter().chain(files).collect()
}

/// Whether a tutorial is listed for the filter (empty: all): its title, its category or its template's name has
/// the text, in any case. `all` lists those without a model too.
pub fn listed(t: &Tutorial, category: &str, filter: &str, all: bool) -> bool {
    if !all && ways(t).is_empty() {
        return false;
    }
    let f = filter.trim().to_lowercase();
    f.is_empty()
        || [t.title.as_str(), category, t.template.as_deref().unwrap_or("")]
            .iter()
            .any(|s| s.to_lowercase().contains(&f))
}

/// The folder of the application's data the published models go to.
pub fn cache_dir() -> PathBuf {
    crate::settings::data_dir().join("tutorial models")
}

/// The files to download for a model of a tutorial and the model to open, under `root`: one folder per tutorial
/// (`root/{category}/{slug}`), the files of its published folder in it by their names.
pub fn plan(t: &Tutorial, model: &str, root: &Path) -> Plan {
    let folder = root.join(&t.category).join(&t.slug);
    let name = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
    let mut fetch: Vec<(String, PathBuf)> = t.files.iter().map(|p| (p.clone(), folder.join(name(p)))).collect();
    let path = format!("/data/tutorials/{}/{}/{model}", t.category, t.slug);
    if !fetch.iter().any(|(p, _)| *p == path) {
        fetch.push((path, folder.join(model)));
    }
    Plan { fetch, open: folder.join(model) }
}

/// Make the files of a plan, each got by `get` (the path in the site to its bytes): written beside their final name
/// and renamed, so a failure leaves no half file. The model to open.
pub fn download(plan: &Plan, get: &dyn Fn(&str) -> Result<Vec<u8>, String>) -> Result<PathBuf, String> {
    for (path, dest) in &plan.fetch {
        let name = dest.file_name().map_or(path.clone(), |n| n.to_string_lossy().into_owned());
        let bytes = get(path).map_err(|e| format!("{name}: {e}"))?;
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let part = dest.with_extension("part");
        std::fs::write(&part, bytes).map_err(|e| format!("{}: {e}", part.display()))?;
        let _ = std::fs::remove_file(dest);
        std::fs::rename(&part, dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    }
    Ok(plan.open.clone())
}

/// A file of the website, from the web.
fn get(path: &str) -> Result<Vec<u8>, String> {
    let url = crate::docs::page(path);
    let mut res = crate::update::agent().get(&url).call().map_err(|e| e.to_string())?;
    res.body_mut().with_config().limit(256 << 20).read_to_vec().map_err(|e| e.to_string())
}

type Done = Arc<Mutex<Option<Result<PathBuf, String>>>>;

/// A model being downloaded, or one that failed (what to tell).
enum Job {
    Running(Done),
    Failed(String),
}

/// The window's state: the filter, whether all tutorials are listed, and the downloads by model.
pub struct State {
    filter: String,
    all: bool,
    fresh: bool,
    jobs: HashMap<String, Job>,
}

impl State {
    pub fn new() -> Self {
        State { filter: String::new(), all: false, fresh: true, jobs: HashMap::new() }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

/// Start the download of a published model in a thread of its own (it wakes the window when done).
fn start(ctx: &egui::Context, t: &Tutorial, model: &str) -> Done {
    let (plan, done, ctx) = (plan(t, model, &cache_dir()), Done::default(), ctx.clone());
    let slot = done.clone();
    std::thread::spawn(move || {
        let r = download(&plan, &get);
        *slot.lock().unwrap() = Some(r);
        ctx.request_repaint();
    });
    done
}

/// The first line of a failure, for a row.
fn one_line(e: &str) -> &str {
    e.lines().next().unwrap_or(e)
}

/// Draw the window (when there is one); what was asked, if anything (the window closes when a model is opened).
pub fn window(ctx: &egui::Context, state: &mut Option<State>) -> Option<Act> {
    let mut st = state.take()?;
    let mut act = None;
    let mut open = true;
    // (the downloads that finished: the model opens, or the row tells why not)
    let mut finished = Vec::new();
    for (key, job) in &st.jobs {
        if let Job::Running(done) = job
            && let Some(r) = done.lock().unwrap().take()
        {
            finished.push((key.clone(), r));
        }
    }
    for (key, r) in finished {
        match r {
            Ok(p) => {
                st.jobs.remove(&key);
                act = Some(Act::Open(p));
            }
            Err(e) => {
                st.jobs.insert(key, Job::Failed(one_line(&e).to_string()));
            }
        }
    }
    let screen = ctx.content_rect();
    egui::Window::new("Tutorials with a model")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(560.0f32.min(screen.width() - 40.0).max(300.0), 520.0f32.min(screen.height() - 60.0)))
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(screen.center())
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "A tutorial's model opens here: a template of the application, or the files of the tutorial \
                     downloaded from the website (the data they read next to them).",
                )
                .weak()
                .size(11.0),
            );
            ui.horizontal(|ui| {
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut st.filter)
                        .hint_text("filter by title, category or template")
                        .desired_width(300.0),
                );
                if st.fresh {
                    edit.request_focus();
                    st.fresh = false;
                }
                ui.checkbox(&mut st.all, "all tutorials")
                    .on_hover_text("also the tutorials without a model to open (to read only)");
            });
            ui.separator();
            let (filter, all) = (st.filter.clone(), st.all);
            let mut any = false;
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for c in tutorials::index() {
                    let rows: Vec<&Tutorial> =
                        c.tutorials.iter().filter(|t| listed(t, &c.title, &filter, all)).collect();
                    if rows.is_empty() {
                        continue;
                    }
                    // (the first open; all of them while a filter is on)
                    let first = !any;
                    any = true;
                    let header = egui::CollapsingHeader::new(RichText::new(&c.title).strong())
                        .id_salt(("tutorial models", &c.slug))
                        .default_open(first);
                    let header = if filter.trim().is_empty() { header } else { header.open(Some(true)) };
                    header.show(ui, |ui| {
                        for t in rows {
                            if let Some(a) = row(ui, &mut st.jobs, t) {
                                act = Some(a);
                            }
                        }
                    });
                }
                if !any {
                    ui.label(RichText::new("No tutorial matches.").weak());
                }
            });
        });
    if open && act.is_none() {
        *state = Some(st);
    }
    act
}

/// A tutorial's row: its title, *Read*, and a button for each model it has.
fn row(ui: &mut egui::Ui, jobs: &mut HashMap<String, Job>, t: &'static Tutorial) -> Option<Act> {
    let mut act = None;
    let ways = ways(t);
    ui.horizontal_wrapped(|ui| {
        ui.label(&t.title);
        if ui.small_button("Read").on_hover_text("the tutorial's page on the website").clicked() {
            crate::docs::open(&tutorials::url(t));
        }
        for way in &ways {
            let (label, hover, key) = match way {
                Way::Template(x) => (
                    if ways.len() == 1 { "Open the model".to_string() } else { "Open the template".to_string() },
                    format!("the template {}, as the Templates menu opens it", x.name),
                    format!("template {}/{}", t.category, t.slug),
                ),
                Way::File(f) => (
                    if ways.len() == 1 { "Open the model".to_string() } else { format!("Open {f}") },
                    "downloaded from the website with the files it reads, then opened".to_string(),
                    format!("/data/tutorials/{}/{}/{f}", t.category, t.slug),
                ),
            };
            if matches!(jobs.get(&key), Some(Job::Running(_))) {
                ui.spinner();
                ui.label(RichText::new("downloading...").weak().size(11.0));
                continue;
            }
            if !ui.small_button(label).on_hover_text(hover).clicked() {
                continue;
            }
            match way {
                Way::Template(x) => act = Some(Act::Start(Box::new(x.model()))),
                Way::File(f) => {
                    jobs.insert(key, Job::Running(start(ui.ctx(), t, f)));
                }
            }
        }
    });
    for way in &ways {
        if let Way::File(f) = way
            && let Some(Job::Failed(e)) = jobs.get(&format!("/data/tutorials/{}/{}/{f}", t.category, t.slug))
        {
            ui.label(RichText::new(e).small().color(ui.visuals().error_fg_color));
        }
    }
    act
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tutorial(slug: &str, title: &str, template: Option<&str>, files: &[&str]) -> Tutorial {
        let models: Vec<String> =
            files.iter().filter(|f| f.ends_with(".toml")).map(|f| format!("/data/tutorials/cat/{slug}/{f}")).collect();
        Tutorial {
            slug: slug.into(),
            title: title.into(),
            template: template.map(String::from),
            models,
            files: files.iter().map(|f| format!("/data/tutorials/cat/{slug}/{f}")).collect(),
            category: "cat".into(),
        }
    }

    #[test]
    fn the_filter_matches_title_category_and_template_in_any_case() {
        let t = tutorial("a", "Habitat Patterns", Some("MSPA of a forest"), &[]);
        for f in ["", "  ", "habitat", "PATTERNS", "mspa", "Forest", "landscape"] {
            assert!(listed(&t, "Landscape metrics", f, false), "{f:?}");
        }
        assert!(!listed(&t, "Landscape metrics", "corridor", false));
    }

    #[test]
    fn the_list_has_only_the_tutorials_with_a_model_unless_all() {
        let none = tutorial("a", "Plain", None, &["data.csv"]);
        let file = tutorial("b", "Files", None, &["data.csv", "m.toml"]);
        let tmpl = tutorial("c", "Template", Some("MSPA of a forest"), &[]);
        assert!(!listed(&none, "C", "", false) && listed(&none, "C", "", true));
        assert!(listed(&file, "C", "", false) && listed(&tmpl, "C", "", false));
        assert_eq!(ways(&none).len(), 0);
        assert!(matches!(ways(&tmpl)[0], Way::Template(x) if x.name == "MSPA of a forest"));
        assert!(matches!(&ways(&file)[0], Way::File(n) if n == "m.toml"));
        // a template that does not exist is no way (a test elsewhere keeps the index to templates that exist)
        assert!(ways(&tutorial("d", "X", Some("No such template"), &[])).is_empty());
    }

    #[test]
    fn the_plan_fetches_every_file_of_the_folder_and_opens_the_model() {
        let t = tutorial("s", "S", None, &["a.csv", "m1.toml", "m2.toml", "r.tif"]);
        let root = Path::new("cache");
        let p = plan(&t, "m2.toml", root);
        let folder = root.join("cat").join("s");
        assert_eq!(p.open, folder.join("m2.toml"));
        assert_eq!(p.fetch.len(), 4);
        assert_eq!(p.fetch[0], ("/data/tutorials/cat/s/a.csv".to_string(), folder.join("a.csv")));
        assert!(p.fetch.iter().any(|(u, d)| u == "/data/tutorials/cat/s/r.tif" && *d == folder.join("r.tif")));
        // a model the index lacks in the files is fetched too
        let lone = tutorial("l", "L", None, &[]);
        let p = plan(&lone, "x.toml", root);
        assert_eq!(p.fetch, vec![("/data/tutorials/cat/l/x.toml".to_string(), root.join("cat/l/x.toml"))]);
    }

    #[test]
    fn the_index_gives_every_model_its_files_and_its_template_exists() {
        for c in tutorials::index() {
            for t in &c.tutorials {
                for m in &t.models {
                    assert!(t.files.contains(m), "{m} is in the files of {}", t.slug);
                }
                assert!(t.files.iter().all(|f| f.starts_with(&format!("/data/tutorials/{}/{}/", c.slug, t.slug))));
            }
        }
        // the tutorials listed by default are those with a way to open
        let listed_n = tutorials::index()
            .iter()
            .flat_map(|c| c.tutorials.iter().map(move |t| (c, t)))
            .filter(|(c, t)| listed(t, &c.title, "", false))
            .count();
        assert!(listed_n > 20, "{listed_n}");
    }

    #[test]
    fn a_download_writes_all_the_files_and_names_the_one_that_fails() {
        let dir = std::env::temp_dir().join(format!("terraflux-tutorial-models-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let t = tutorial("s", "S", None, &["a.csv", "m.toml"]);
        let p = plan(&t, "m.toml", &dir);
        let got = download(&p, &|path| Ok(format!("bytes of {path}").into_bytes())).unwrap();
        assert_eq!(got, dir.join("cat/s/m.toml"));
        assert_eq!(std::fs::read_to_string(dir.join("cat/s/a.csv")).unwrap(), "bytes of /data/tutorials/cat/s/a.csv");
        // again: the files are replaced, no part file is left
        download(&p, &|_| Ok(b"new".to_vec())).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("cat/s/m.toml")).unwrap(), "new");
        assert!(!dir.join("cat/s/m.part").exists());
        // a failure names the file, and the model is not opened
        let e = download(&p, &|path| if path.ends_with("m.toml") { Err("HTTP 404".into()) } else { Ok(vec![]) });
        assert_eq!(e, Err("m.toml: HTTP 404".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
