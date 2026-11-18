//! The website's tutorials as the application knows them: the categories in the site's order, each with its
//! tutorials (the page, the title, the template it is about and the models published with it). The index is
//! `tutorials.json`, made from `website/content` by `website/tools/tutorial_index.py` and read once; a test fails
//! when it is out of date.

use std::sync::LazyLock;

use serde::Deserialize;

/// The index, as made by the script.
const INDEX: &str = include_str!("tutorials.json");

/// A tutorial of the website.
#[derive(Debug, Deserialize)]
pub struct Tutorial {
    /// Its page's name in the category's address (`mspa`).
    pub slug: String,
    pub title: String,
    /// The name of the template of the application it is about (the page's `template:`), if any.
    pub template: Option<String>,
    /// The models published with it, as paths of the site (`/data/tutorials/{category}/{slug}/model.toml`).
    pub models: Vec<String>,
    /// Its category's slug (filled when the index is read).
    #[serde(skip)]
    pub category: String,
}

/// A category of tutorials.
#[derive(Debug, Deserialize)]
pub struct Category {
    pub slug: String,
    pub title: String,
    pub tutorials: Vec<Tutorial>,
}

#[derive(Deserialize)]
struct Index {
    categories: Vec<Category>,
}

static CATEGORIES: LazyLock<Vec<Category>> = LazyLock::new(|| {
    let mut index: Index = serde_json::from_str(INDEX).expect("tutorials.json is the script's index");
    for c in &mut index.categories {
        for t in &mut c.tutorials {
            t.category = c.slug.clone();
        }
    }
    index.categories
});

/// The categories in the site's order, each with its tutorials in the order of the site.
pub fn index() -> &'static [Category] {
    &CATEGORIES
}

/// The first tutorial (in the site's order) about a template, by the template's name.
pub fn for_template(name: &str) -> Option<&'static Tutorial> {
    index().iter().flat_map(|c| &c.tutorials).find(|t| t.template.as_deref() == Some(name))
}

/// The address of a tutorial's page.
pub fn url(t: &Tutorial) -> String {
    crate::docs::tutorial_url(&t.category, &t.slug)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::path::{Path, PathBuf};

    fn website() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website")
    }

    /// A value of a front matter line (`title: "A: b"`), without its quotes.
    fn unquoted(v: &str) -> String {
        let v = v.trim();
        let quoted =
            v.len() >= 2 && (v.starts_with('"') && v.ends_with('"') || v.starts_with('\'') && v.ends_with('\''));
        if quoted { v[1..v.len() - 1].to_string() } else { v.to_string() }
    }

    /// The front matter's fields of a page, one per line (all the pages' are).
    fn front(text: &str) -> Vec<(String, String)> {
        let Some(rest) = text.strip_prefix("---") else { return Vec::new() };
        let end = rest.find("\n---").expect("a front matter is closed");
        rest[..end].lines().filter_map(|l| l.split_once(": ").map(|(k, v)| (k.to_string(), unquoted(v)))).collect()
    }

    /// The categories of `site.yaml`: slug and title, in order.
    fn site_categories(yaml: &str) -> Vec<(String, String)> {
        let block = yaml.split("\ntutorial_categories:\n").nth(1).expect("the site has tutorial categories");
        block
            .lines()
            .take_while(|l| l.starts_with("  - {"))
            .map(|l| {
                let field = |k: &str| {
                    let at = l.find(&format!("{k}: ")).expect("a category has a slug and a title") + k.len() + 2;
                    let end = l[at..].find(", ").map_or(l.len(), |e| at + e);
                    unquoted(&l[at..end])
                };
                (field("slug"), field("title"))
            })
            .collect()
    }

    /// What the script makes, made again here from the content of the site.
    fn derived() -> Value {
        let site = website();
        let yaml = std::fs::read_to_string(site.join("site.yaml")).unwrap().replace("\r\n", "\n");
        let mut categories = Vec::new();
        for (slug, title) in site_categories(&yaml) {
            let mut pages = Vec::new();
            for e in std::fs::read_dir(site.join("content/tutorials").join(&slug)).unwrap() {
                let path = e.unwrap().path();
                if path.extension().is_none_or(|x| x != "md") || path.file_name().unwrap() == "index.md" {
                    continue;
                }
                let fields = front(&std::fs::read_to_string(&path).unwrap());
                let get = |k: &str| fields.iter().find(|(f, _)| f == k).map(|(_, v)| v.clone());
                let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
                let order: i64 = get("order").map_or(999, |o| o.parse().expect("an order is a number"));
                pages.push((order, get("title").unwrap_or_else(|| stem.clone()), stem, get("template")));
            }
            pages.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
            let tutorials: Vec<Value> = pages
                .into_iter()
                .map(|(_, title, page, template)| {
                    let dir = site.join("static/data/tutorials").join(&slug).join(&page);
                    let mut models: Vec<String> = std::fs::read_dir(dir)
                        .into_iter()
                        .flatten()
                        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                        .filter(|n| n.ends_with(".toml"))
                        .collect();
                    models.sort();
                    let models: Vec<String> =
                        models.into_iter().map(|m| format!("/data/tutorials/{slug}/{page}/{m}")).collect();
                    json!({ "slug": page, "title": title, "template": template, "models": models })
                })
                .collect();
            categories.push(json!({ "slug": slug, "title": title, "tutorials": tutorials }));
        }
        json!({ "categories": categories })
    }

    #[test]
    fn the_index_is_the_one_of_the_content() {
        let written: Value = serde_json::from_str(INDEX).unwrap();
        assert!(
            written == derived(),
            "crates/app/src/tutorials.json is out of date: run `python website/tools/tutorial_index.py`"
        );
    }

    #[test]
    fn every_template_a_tutorial_names_exists() {
        let names: Vec<&str> = crate::templates::TEMPLATES.iter().map(|t| t.name).collect();
        let lost: Vec<String> = index()
            .iter()
            .flat_map(|c| &c.tutorials)
            .filter_map(|t| t.template.as_ref().map(|n| format!("{}/{}: {n}", t.category, t.slug)))
            .filter(|s| !names.contains(&s.split_once(": ").unwrap().1))
            .collect();
        assert!(lost.is_empty(), "tutorials of a template that does not exist: {lost:?}");
    }

    #[test]
    fn a_template_finds_its_tutorial() {
        assert!(!index().is_empty() && index().iter().all(|c| !c.tutorials.is_empty()));
        let t = for_template("MSPA of a forest").expect("MSPA of a forest has a tutorial");
        assert!(t.category == "getting-started" || t.category == "habitat-patterns", "{}", t.category);
        assert_eq!(url(t), crate::docs::tutorial_url(&t.category, &t.slug));
        assert!(url(t).starts_with(crate::docs::SITE) && url(t).ends_with('/'));
        assert!(for_template("No such template").is_none());
        // a template of the application with a tutorial is found in the models' list too
        let with_model = index().iter().flat_map(|c| &c.tutorials).find(|t| !t.models.is_empty()).unwrap();
        assert!(
            with_model.models[0].starts_with(&format!("/data/tutorials/{}/{}/", with_model.category, with_model.slug))
        );
    }
}
