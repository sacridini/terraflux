//! The application's way to its documentation: the website's pages by their address (an operator's page and anchor
//! by the same rule as `website/build.py`), the Help menu's pages, the report of a problem as a new GitHub issue
//! already filled in (opened in the browser for the person to read and send: nothing leaves the application by
//! itself) and the About window.

use std::cell::RefCell;

use egui::{RichText, Vec2};

use crate::builder::{IO, SUBJECTS};

/// The website's address.
pub const SITE: &str = "https://sacridini.github.io/terraflux/";

/// The Help menu's pages: what it says, the page, and what the page is.
pub const PAGES: [(&str, &str, &str); 4] = [
    ("Quickstart", "quickstart/", "the first model, step by step"),
    ("Tutorials", "tutorials/", "the tutorials by subject"),
    ("The application", "app/", "the window, the menus, the canvas, the Viewer and the settings"),
    ("Operator reference", "operators/", "every operator by subject: its ports, its settings and its method"),
];

/// The citation file, read when the application is built.
const CITATION: &str = include_str!("../../../CITATION.cff");

/// The page of the website at a path from its root (`app/`, or `/app/`).
pub fn page(path: &str) -> String {
    format!("{SITE}{}", path.trim_start_matches('/'))
}

/// A tutorial's page: its category's slug and its own (`habitat-patterns`, `mspa`).
pub fn tutorial_url(category: &str, page_name: &str) -> String {
    page(&format!("tutorials/{category}/{page_name}/"))
}

/// What the website makes of a title for an address (`Distance & restoration` is `distance-restoration`): the
/// letters, digits and `_` kept in lower case, the spaces and hyphens in a run made one hyphen.
pub fn slug(title: &str) -> String {
    let kept: String =
        title.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || c.is_whitespace()).collect();
    let mut out = String::new();
    let mut dash = false;
    for c in kept.trim().to_lowercase().chars() {
        if c == '-' || c.is_whitespace() {
            dash = true;
        } else {
            if dash {
                out.push('-');
                dash = false;
            }
            out.push(c);
        }
    }
    out
}

/// The subject of an operator in the library, which is the section of the operator reference: the file operators
/// are IO, then a subject that names the operator in full, then the first whose prefix is its (as `website/build.py`).
pub fn subject_of(op: &str) -> &'static str {
    if IO.contains(&op) {
        return "IO";
    }
    let prefix = op.split('.').next().unwrap_or("");
    let named = SUBJECTS.iter().find(|(_, p)| p.contains(&op));
    named.or_else(|| SUBJECTS.iter().find(|(_, p)| p.contains(&prefix))).map_or("Other", |(s, _)| s)
}

/// The anchor of an operator in its page.
pub fn anchor(op: &str) -> String {
    format!("op-{}", op.replace(['.', '_'], "-"))
}

/// The page of an operator in the operator reference, from the website's root: `/operators/{subject}/#op-{id}`
/// (the whole address with [`page`]).
pub fn op_url(op: &str) -> String {
    format!("/operators/{}/#{}", slug(subject_of(op)), anchor(op))
}

/// Open an address in the browser: [`crate::update::open_page`], or, in the tests that asked ([`record`]), noted.
pub fn open(url: &str) {
    let noted = OPENED.with(|o| match &mut *o.borrow_mut() {
        Some(list) => {
            list.push(url.to_string());
            true
        }
        None => false,
    });
    if !noted {
        crate::update::open_page(url);
    }
}

thread_local! {
    /// The addresses opened, while a test records them (none: they are opened).
    static OPENED: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// From now on, in this thread, [`open`] notes the address instead of opening a browser (for the tests).
pub fn record() {
    OPENED.with(|o| *o.borrow_mut() = Some(Vec::new()));
}

/// The addresses noted since [`record`] or the last call.
pub fn opened() -> Vec<String> {
    OPENED.with(|o| o.borrow_mut().as_mut().map(std::mem::take).unwrap_or_default())
}

/// The most the encoded body of a report of a problem takes (GitHub refuses addresses past about 8 kB).
const BODY_MAX: usize = 6000;

/// A text percent-encoded for the query of an address.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The body of a report: the version, the system, the last failure if any, and the headings to fill in.
fn body(version: &str, os: &str, failure: Option<&str>) -> String {
    let mut b = format!("**Terraflux** {version}\n**System** {os}\n");
    if let Some(f) = failure {
        b.push_str(&format!("\n**The last failure**\n```\n{f}\n```\n"));
    }
    b.push_str("\n## What I did\n\n\n## What I expected\n\n\n## What happened\n\n");
    b
}

/// The address of a new issue on GitHub filled in with the version, the system and the last failure (cut when it is
/// long: the address stays within what GitHub takes). The browser shows the form; the person reads it and sends it.
pub fn issue_url(version: &str, os: &str, failure: Option<&str>) -> String {
    let encoded = |f: Option<&str>| encode(&body(version, os, f));
    let fits = |f: &str| encoded(Some(f)).len() <= BODY_MAX;
    let text = match failure.map(str::trim).filter(|f| !f.is_empty()) {
        None => encoded(None),
        Some(f) if fits(f) => encoded(Some(f)),
        Some(f) => {
            // the most characters of it that fit, with the mark that it was cut
            let chars: Vec<char> = f.chars().collect();
            let cut = |n: usize| format!("{} [...cut]", chars[..n].iter().collect::<String>());
            let (mut lo, mut hi) = (0, chars.len());
            while lo < hi {
                let mid = (lo + hi).div_ceil(2);
                if fits(&cut(mid)) {
                    lo = mid;
                } else {
                    hi = mid - 1;
                }
            }
            encoded(Some(&cut(lo)))
        }
    };
    format!("https://github.com/{}/issues/new?title={}&body={}", crate::update::REPO, encode("Problem: "), text)
}

/// The system as the report names it: `windows x86_64`.
pub fn system() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// The value of a top-level field of the citation file (`title: "..."`), without quotes.
fn field(cff: &str, key: &str) -> Option<String> {
    let line = cff.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix(':'))?;
    Some(line.trim().trim_matches('"').to_string())
}

/// The authors of the citation file as `Lacerda, E.` (the family name, the initial of the given name).
fn authors(cff: &str) -> Vec<String> {
    let (mut out, mut family) = (Vec::new(), None::<String>);
    let mut in_authors = false;
    for l in cff.lines() {
        if !l.starts_with(' ') && !l.starts_with('-') {
            in_authors = l.starts_with("authors:");
            continue;
        }
        if !in_authors {
            continue;
        }
        let l = l.trim().trim_start_matches("- ");
        if let Some(f) = l.strip_prefix("family-names:") {
            family = Some(f.trim().trim_matches('"').to_string());
        } else if let Some(g) = l.strip_prefix("given-names:") {
            let initial = g.trim().trim_matches('"').chars().next().map(|c| format!(", {c}.")).unwrap_or_default();
            out.push(format!("{}{initial}", family.take().unwrap_or_default()));
        }
    }
    out
}

/// How to cite Terraflux, from the citation file: `Lacerda, E. (2026). Title (version 1.54.0) [Computer software].
/// https://...`. The version is the running one (the file's is written at a release).
pub fn citation(cff: &str, version: &str) -> String {
    let year = field(cff, "date-released").and_then(|d| d.get(..4).map(str::to_string)).unwrap_or_default();
    let title = field(cff, "title").unwrap_or_else(|| "Terraflux".into());
    let url = field(cff, "url").unwrap_or_else(|| SITE.into());
    format!("{} ({year}). {title} (version {version}) [Computer software]. {url}", authors(cff).join(", "))
}

/// How to cite this version of Terraflux.
pub fn how_to_cite() -> String {
    citation(CITATION, env!("CARGO_PKG_VERSION"))
}

/// The links of the About window: what it says and the address.
fn links() -> [(&'static str, String); 4] {
    let repo = format!("https://github.com/{}", crate::update::REPO);
    [
        ("Website", SITE.to_string()),
        ("Source code", repo.clone()),
        ("Releases", format!("{repo}/releases")),
        ("Changelog", format!("{repo}/blob/main/CHANGELOG.md")),
    ]
}

/// The About window: the name, the version, the licence, how to cite and the links.
pub fn about_window(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    let screen = ctx.content_rect();
    egui::Window::new("About Terraflux")
        .open(open)
        .resizable(false)
        .collapsible(false)
        .fixed_size(Vec2::new(440.0f32.min(screen.width() - 40.0).max(260.0), 0.0))
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(screen.center())
        .show(ctx, |ui| {
            ui.label(RichText::new("Terraflux").strong().size(18.0));
            ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
            ui.label(RichText::new("GPL-3.0-or-later").weak());
            ui.add_space(6.0);
            crate::theme::section(ui, "How to cite");
            let cite = how_to_cite();
            ui.label(&cite);
            if ui.button("Copy").on_hover_text("the citation, to paste into a paper").clicked() {
                ui.ctx().copy_text(cite);
            }
            ui.add_space(6.0);
            crate::theme::section(ui, "Links");
            for (name, url) in links() {
                if ui.link(name).on_hover_text(&url).clicked() {
                    open_url(&url);
                }
            }
        });
}

/// Open an address (the name apart so the window's closure reads as it is).
fn open_url(url: &str) {
    open(url);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_operator_is_in_the_page_and_anchor_of_the_site() {
        assert_eq!(op_url("landscape.mspa"), "/operators/habitat-patterns/#op-landscape-mspa");
        // the full name wins over the prefix: raster.stats is Raster analysis, not Data preparation
        assert_eq!(op_url("raster.stats"), "/operators/raster-analysis/#op-raster-stats");
        assert_eq!(op_url("raster.local_correlation"), "/operators/raster-analysis/#op-raster-local-correlation");
        assert_eq!(op_url("raster.calc"), "/operators/data-preparation/#op-raster-calc");
        assert_eq!(op_url("raster.read"), "/operators/io/#op-raster-read");
        assert_eq!(page(&op_url("landscape.mspa")), format!("{SITE}operators/habitat-patterns/#op-landscape-mspa"));
    }

    #[test]
    fn a_title_is_an_address_as_the_site_makes_it() {
        assert_eq!(slug("Distance & restoration"), "distance-restoration");
        assert_eq!(slug("Forest Code (Brazil)"), "forest-code-brazil");
        assert_eq!(slug("Carbon and REDD+"), "carbon-and-redd");
        assert_eq!(slug("Large data & batch"), "large-data-batch");
    }

    #[test]
    fn the_pages_are_made_from_the_site() {
        assert_eq!(page("/app/"), "https://sacridini.github.io/terraflux/app/");
        assert_eq!(page("quickstart/"), "https://sacridini.github.io/terraflux/quickstart/");
        assert_eq!(tutorial_url("habitat-patterns", "mspa"), format!("{SITE}tutorials/habitat-patterns/mspa/"));
    }

    #[test]
    fn every_operator_has_a_subject() {
        let r = terraflux_ops::Registry::standard();
        let lost: Vec<String> =
            r.signatures().into_iter().map(|g| g.name).filter(|n| subject_of(n) == "Other").collect();
        assert!(lost.is_empty(), "operators without a subject: {lost:?}");
    }

    /// The site reads the subjects from `website/data/subjects.json` (made from builder.rs by
    /// `website/tools/refresh_data.py`): the two must be the same.
    #[test]
    fn the_subjects_are_the_ones_the_site_reads() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/data/subjects.json");
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let strings = |v: &serde_json::Value| -> Vec<String> {
            v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
        };
        let site: Vec<(String, Vec<String>)> = json["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| (s["title"].as_str().unwrap().to_string(), strings(&s["prefixes"])))
            .collect();
        let app: Vec<(String, Vec<String>)> =
            SUBJECTS.iter().map(|(t, p)| (t.to_string(), p.iter().map(|x| x.to_string()).collect())).collect();
        assert_eq!(app, site, "run website/tools/refresh_data.py");
        assert_eq!(IO.iter().map(|x| x.to_string()).collect::<Vec<_>>(), strings(&json["io"]));
    }

    #[test]
    fn a_report_is_encoded_and_cut_to_fit() {
        assert_eq!(encode("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
        let u = issue_url("1.54.0", "windows x86_64", Some("it failed: `x` & y"));
        assert!(u.starts_with("https://github.com/sacridini/terraflux/issues/new?title="));
        assert!(u.contains("&body=%2A%2ATerraflux%2A%2A%201.54.0"), "{u}");
        assert!(u.contains("windows%20x86_64") && u.contains("it%20failed%3A%20%60x%60%20%26%20y"), "{u}");
        assert!(u.contains("What%20I%20did") && u.contains("What%20I%20expected"));
        // no failure, no failure section
        assert!(!issue_url("1.0.0", "linux x86_64", None).contains("last%20failure"));
        // a long failure is cut, and the address stays short; the headings are kept
        let long = "é".repeat(20_000);
        let u = issue_url("1.54.0", "windows x86_64", Some(&long));
        let body = u.split("&body=").nth(1).unwrap();
        assert!(body.len() <= BODY_MAX, "{}", body.len());
        assert!(body.contains("cut%5D") && body.contains("What%20I%20expected"));
    }

    #[test]
    fn the_citation_is_made_from_the_citation_file() {
        let c = how_to_cite();
        assert!(c.starts_with("Lacerda, E. (20"), "{c}");
        assert!(c.contains(&format!("(version {})", env!("CARGO_PKG_VERSION"))) && c.ends_with(SITE), "{c}");
        assert!(c.contains("Terraflux: landscape ecology"), "{c}");
    }

    #[test]
    fn opening_is_noted_when_a_test_asks() {
        record();
        open("https://example.org/a");
        open(&page("app/"));
        assert_eq!(opened(), vec!["https://example.org/a".to_string(), page("app/")]);
        assert!(opened().is_empty());
    }
}
