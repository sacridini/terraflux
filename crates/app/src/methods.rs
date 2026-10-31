//! The methods of a model as a paper tells them: a paragraph that says which methods it used, with the papers of
//! each and of the software Terraflux reproduces; the model step by step; and the references, as text, BibTeX
//! and RIS.
//!
//! The references and which operators use them are those of the website's "Cite Terraflux" page
//! (`website/content/about/citing.md`), each method's reference implementation as its operators' descriptions
//! name it. Nothing else is cited: a method without a paper here is said without one, and the papers a node's
//! `refs` give are listed as they are written.

use terraflux_graph::{Model, Node};
use terraflux_ops::Registry;

/// A paper (or a report) to cite.
#[derive(Debug, PartialEq, Eq)]
pub struct Reference {
    /// Its key in BibTeX.
    pub key: &'static str,
    /// "Family, I. N.", in the paper's order.
    pub authors: &'static [&'static str],
    /// Whether the authors listed are the first of more.
    pub more: bool,
    pub year: u16,
    pub title: &'static str,
    /// The journal (empty for a report).
    pub journal: &'static str,
    pub volume: &'static str,
    pub issue: &'static str,
    pub pages: &'static str,
    pub doi: &'static str,
    /// A report: its series and publisher.
    pub report: &'static str,
}

impl Reference {
    fn family(author: &str) -> &str {
        author.split(',').next().unwrap_or(author).trim()
    }

    /// How the text cites it: "Soille & Vogt, 2009", "Hesselbarth et al., 2019".
    pub fn cite(&self) -> String {
        let f: Vec<&str> = self.authors.iter().map(|a| Self::family(a)).collect();
        let who = match f.len() {
            1 if !self.more => f[0].to_string(),
            2 if !self.more => format!("{} & {}", f[0], f[1]),
            _ => format!("{} et al.", f[0]),
        };
        format!("{who}, {}", self.year)
    }

    /// The reference in the APA style, as the website lists it.
    pub fn apa(&self) -> String {
        let a = self.authors;
        let authors = if self.more {
            format!("{}, et al.", a.join(", "))
        } else if a.len() == 1 {
            a[0].to_string()
        } else {
            format!("{}, & {}", a[..a.len() - 1].join(", "), a[a.len() - 1])
        };
        if self.journal.is_empty() {
            return format!("{authors} ({}). {}. {}", self.year, self.title, self.report);
        }
        let issue = if self.issue.is_empty() { String::new() } else { format!("({})", self.issue) };
        format!(
            "{authors} ({}). {}. {}, {}{issue}, {}. https://doi.org/{}",
            self.year, self.title, self.journal, self.volume, self.pages, self.doi
        )
    }

    /// The reference as a BibTeX entry.
    pub fn bibtex(&self) -> String {
        let mut authors = self.authors.join(" and ");
        if self.more {
            authors.push_str(" and others");
        }
        let mut fields = vec![("author", authors), ("title", format!("{{{}}}", self.title))];
        if self.journal.is_empty() {
            fields.push(("institution", self.report.to_string()));
        } else {
            fields.push(("journal", self.journal.to_string()));
            fields.push(("volume", self.volume.to_string()));
            if !self.issue.is_empty() {
                fields.push(("number", self.issue.to_string()));
            }
            fields.push(("pages", self.pages.replace('–', "--")));
            fields.push(("doi", self.doi.to_string()));
        }
        fields.push(("year", self.year.to_string()));
        let kind = if self.journal.is_empty() { "techreport" } else { "article" };
        let body: Vec<String> = fields.into_iter().map(|(k, v)| format!("  {k:<11} = {{{v}}}")).collect();
        format!("@{kind}{{{},\n{}\n}}", self.key, body.join(",\n"))
    }

    /// The reference in RIS (Zotero, Mendeley, EndNote).
    pub fn ris(&self) -> String {
        let mut lines = vec![format!("TY  - {}", if self.journal.is_empty() { "RPRT" } else { "JOUR" })];
        lines.extend(self.authors.iter().map(|a| format!("AU  - {a}")));
        lines.push(format!("PY  - {}", self.year));
        lines.push(format!("TI  - {}", self.title));
        if self.journal.is_empty() {
            lines.push(format!("PB  - {}", self.report));
        } else {
            lines.push(format!("T2  - {}", self.journal));
            lines.push(format!("VL  - {}", self.volume));
            if !self.issue.is_empty() {
                lines.push(format!("IS  - {}", self.issue));
            }
            let mut pages = self.pages.split('–');
            lines.push(format!("SP  - {}", pages.next().unwrap_or_default()));
            if let Some(end) = pages.next() {
                lines.push(format!("EP  - {end}"));
            }
            lines.push(format!("DO  - {}", self.doi));
        }
        lines.push("ER  - ".into());
        lines.join("\n")
    }
}

macro_rules! paper {
    ($key:literal, [$($a:literal),+], $more:literal, $year:literal, $title:literal, $journal:literal, $volume:literal,
     $issue:literal, $pages:literal, $doi:literal) => {
        Reference {
            key: $key,
            authors: &[$($a),+],
            more: $more,
            year: $year,
            title: $title,
            journal: $journal,
            volume: $volume,
            issue: $issue,
            pages: $pages,
            doi: $doi,
            report: "",
        }
    };
}

/// The papers of the methods Terraflux reproduces (the website's "Cite Terraflux").
pub const REFERENCES: &[Reference] = &[
    paper!(
        "hanson2025",
        [
            "Hanson, J. O.",
            "Schuster, R.",
            "Strimas-Mackey, M.",
            "Morrell, N.",
            "Edwards, B. P. M.",
            "Arcese, P.",
            "Bennett, J. R.",
            "Possingham, H. P."
        ],
        false,
        2025,
        "Systematic conservation prioritization with the prioritizr R package",
        "Conservation Biology",
        "39",
        "1",
        "e14376",
        "10.1111/cobi.14376"
    ),
    paper!(
        "hesselbarth2019",
        ["Hesselbarth, M. H. K.", "Sciaini, M.", "With, K. A.", "Wiegand, K.", "Nowosad, J."],
        false,
        2019,
        "landscapemetrics: an open-source R tool to calculate landscape metrics",
        "Ecography",
        "42",
        "10",
        "1648–1657",
        "10.1111/ecog.04617"
    ),
    paper!(
        "kass2021",
        [
            "Kass, J. M.",
            "Muscarella, R.",
            "Galante, P. J.",
            "Bohl, C. L.",
            "Pinilla-Buitrago, G. E.",
            "Boria, R. A.",
            "Soley-Guardia, M.",
            "Anderson, R. P."
        ],
        false,
        2021,
        "ENMeval 2.0: Redesigned for customizable and reproducible modeling of species' niches and distributions",
        "Methods in Ecology and Evolution",
        "12",
        "9",
        "1602–1608",
        "10.1111/2041-210X.13628"
    ),
    paper!(
        "laffan2010",
        ["Laffan, S. W.", "Lubarsky, E.", "Rosauer, D. F."],
        false,
        2010,
        "Biodiverse, a tool for the spatial analysis of biological and related diversity",
        "Ecography",
        "33",
        "4",
        "643–647",
        "10.1111/j.1600-0587.2010.06237.x"
    ),
    paper!(
        "landau2021",
        ["Landau, V. A.", "Shah, V. B.", "Anantharaman, R.", "Hall, K. R."],
        false,
        2021,
        "Omniscape.jl: Software to compute omnidirectional landscape connectivity",
        "Journal of Open Source Software",
        "6",
        "57",
        "2829",
        "10.21105/joss.02829"
    ),
    paper!(
        "liang2021",
        ["Liang, X.", "Guan, Q.", "Clarke, K. C.", "Liu, S.", "Wang, B.", "Yao, Y."],
        false,
        2021,
        "Understanding the drivers of sustainable land expansion using a patch-generating land use simulation \
         (PLUS) model: A case study in Wuhan, China",
        "Computers, Environment and Urban Systems",
        "85",
        "",
        "101569",
        "10.1016/j.compenvurbsys.2020.101569"
    ),
    Reference {
        key: "mcgarigal1995",
        authors: &["McGarigal, K.", "Marks, B. J."],
        more: false,
        year: 1995,
        title: "FRAGSTATS: spatial pattern analysis program for quantifying landscape structure",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "General Technical Report PNW-GTR-351. USDA Forest Service, Pacific Northwest Research Station.",
    },
    paper!(
        "mcrae2008",
        ["McRae, B. H.", "Dickson, B. G.", "Keitt, T. H.", "Shah, V. B."],
        false,
        2008,
        "Using circuit theory to model connectivity in ecology, evolution, and conservation",
        "Ecology",
        "89",
        "10",
        "2712–2724",
        "10.1890/07-1861.1"
    ),
    paper!(
        "meentemeyer2013",
        ["Meentemeyer, R. K.", "Tang, W.", "Dorning, M. A.", "Vogler, J. B.", "Cunniffe, N. J.", "Shoemaker, D. A."],
        false,
        2013,
        "FUTURES: Multilevel simulations of emerging urban–rural landscape structure using a stochastic \
         patch-growing algorithm",
        "Annals of the Association of American Geographers",
        "103",
        "4",
        "785–807",
        "10.1080/00045608.2012.707591"
    ),
    paper!(
        "moilanen2022",
        ["Moilanen, A.", "Lehtinen, P.", "Kohonen, I.", "Jalkanen, J.", "Virtanen, E. A.", "Kujala, H."],
        false,
        2022,
        "Novel methods for spatial prioritization with applications in conservation, land use planning and \
         ecological impact avoidance",
        "Methods in Ecology and Evolution",
        "13",
        "5",
        "1062–1072",
        "10.1111/2041-210X.13819"
    ),
    paper!(
        "moulds2015",
        ["Moulds, S.", "Buytaert, W.", "Mijic, A."],
        false,
        2015,
        "An open and extensible framework for spatially explicit land use change modelling: the lulcc R package",
        "Geoscientific Model Development",
        "8",
        "10",
        "3215–3229",
        "10.5194/gmd-8-3215-2015"
    ),
    paper!(
        "murrayrust2014",
        [
            "Murray-Rust, D.",
            "Brown, C.",
            "van Vliet, J.",
            "Alam, S. J.",
            "Robinson, D. T.",
            "Verburg, P. H.",
            "Rounsevell, M."
        ],
        false,
        2014,
        "Combining agent functional types, capitals and services to model land use dynamics",
        "Environmental Modelling & Software",
        "59",
        "",
        "187–201",
        "10.1016/j.envsoft.2014.05.019"
    ),
    paper!(
        "phillips2017",
        ["Phillips, S. J.", "Anderson, R. P.", "Dudík, M.", "Schapire, R. E.", "Blair, M. E."],
        false,
        2017,
        "Opening the black box: an open-source release of Maxent",
        "Ecography",
        "40",
        "7",
        "887–893",
        "10.1111/ecog.03049"
    ),
    paper!(
        "salgadorojas2020",
        ["Salgado-Rojas, J.", "Álvarez-Miranda, E.", "Hermoso, V.", "Garcia-Gonzalo, J.", "Weintraub, A."],
        false,
        2020,
        "A mixed integer programming approach for multi-action planning for threat management",
        "Ecological Modelling",
        "418",
        "",
        "108901",
        "10.1016/j.ecolmodel.2019.108901"
    ),
    paper!(
        "saura2007",
        ["Saura, S.", "Pascual-Hortal, L."],
        false,
        2007,
        "A new habitat availability index to integrate connectivity in landscape conservation planning: \
         Comparison with existing indices and application to a case study",
        "Landscape and Urban Planning",
        "83",
        "2–3",
        "91–103",
        "10.1016/j.landurbplan.2007.03.005"
    ),
    paper!(
        "saura2009",
        ["Saura, S.", "Torné, J."],
        false,
        2009,
        "Conefor Sensinode 2.2: A software package for quantifying the importance of habitat patches for \
         landscape connectivity",
        "Environmental Modelling & Software",
        "24",
        "1",
        "135–139",
        "10.1016/j.envsoft.2008.05.005"
    ),
    paper!(
        "soaresfilho2002",
        ["Soares-Filho, B. S.", "Coutinho Cerqueira, G.", "Lopes Pennachin, C."],
        false,
        2002,
        "DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an \
         Amazonian colonization frontier",
        "Ecological Modelling",
        "154",
        "3",
        "217–235",
        "10.1016/S0304-3800(02)00059-5"
    ),
    paper!(
        "soille2009",
        ["Soille, P.", "Vogt, P."],
        false,
        2009,
        "Morphological segmentation of binary patterns",
        "Pattern Recognition Letters",
        "30",
        "4",
        "456–459",
        "10.1016/j.patrec.2008.10.015"
    ),
    paper!(
        "strassburg2020",
        [
            "Strassburg, B. B. N.",
            "Iribarrem, A.",
            "Beyer, H. L.",
            "Cordeiro, C. L.",
            "Crouzeilles, R.",
            "Jakovac, C. C."
        ],
        true,
        2020,
        "Global priority areas for ecosystem restoration",
        "Nature",
        "586",
        "7831",
        "724–729",
        "10.1038/s41586-020-2784-9"
    ),
    paper!(
        "vanetten2017",
        ["van Etten, J."],
        false,
        2017,
        "R package gdistance: Distances and routes on geographical grids",
        "Journal of Statistical Software",
        "76",
        "13",
        "1–21",
        "10.18637/jss.v076.i13"
    ),
    paper!(
        "verburg2002",
        ["Verburg, P. H.", "Soepboer, W.", "Veldkamp, A.", "Limpiada, R.", "Espaldon, V.", "Mastura, S. S. A."],
        false,
        2002,
        "Modeling the spatial dynamics of regional land use: the CLUE-S model",
        "Environmental Management",
        "30",
        "3",
        "391–405",
        "10.1007/s00267-002-2630-x"
    ),
    paper!(
        "vogt2017",
        ["Vogt, P.", "Riitters, K."],
        false,
        2017,
        "GuidosToolbox: universal digital image object analysis",
        "European Journal of Remote Sensing",
        "50",
        "1",
        "352–361",
        "10.1080/22797254.2017.1330650"
    ),
    paper!(
        "zizka2019",
        ["Zizka, A.", "Silvestro, D.", "Andermann, T.", "Azevedo, J.", "Duarte Ritter, C.", "Edler, D."],
        true,
        2019,
        "CoordinateCleaner: Standardized cleaning of occurrence records from biological collection databases",
        "Methods in Ecology and Evolution",
        "10",
        "5",
        "744–751",
        "10.1111/2041-210X.13152"
    ),
];

/// A method, by the operators that compute it: its name in a sentence ("{ops}": the operators used), the papers
/// of the method, the software whose results Terraflux reproduces (none: the paper's own) and that software's
/// papers. An operator is the first method whose patterns take it: a name, or its beginning (ending in `.` or
/// `_`).
pub struct Method {
    pub ops: &'static [&'static str],
    pub name: &'static str,
    pub refs: &'static [&'static str],
    pub software: &'static str,
    pub software_refs: &'static [&'static str],
}

const GTB: &str = "GuidosToolbox";

pub const METHODS: &[Method] = &[
    Method {
        ops: &["landscape.mspa", "landscape.mspa_tiled"],
        name: "morphological spatial pattern analysis (MSPA)",
        refs: &["soille2009"],
        software: GTB,
        software_refs: &["vogt2017"],
    },
    Method {
        ops: &["fragmentation.fad", "fragmentation.fad_multiscale", "fragmentation.fos"],
        name: "fragmentation (FAD, FOS)",
        refs: &[],
        software: GTB,
        software_refs: &["vogt2017"],
    },
    Method {
        ops: &["landscape.", "fragmentation."],
        name: "pattern analyses ({ops})",
        refs: &[],
        software: GTB,
        software_refs: &["vogt2017"],
    },
    Method {
        ops: &["networks.gtn_"],
        name: "the habitat network's components and importance",
        refs: &[],
        software: GTB,
        software_refs: &["vogt2017"],
    },
    Method {
        ops: &["networks.conefor"],
        name: "the habitat network's inputs for Conefor",
        refs: &["saura2009"],
        software: GTB,
        software_refs: &["vogt2017"],
    },
    Method {
        ops: &[
            "restoration.corridor",
            "restoration.cost_distance",
            "restoration.isochrone",
            "restoration.least_cost_path",
            "restoration.resistance",
            "restoration.status",
        ],
        name: "the Restoration Planner's analyses ({ops})",
        refs: &[],
        software: GTB,
        software_refs: &["vogt2017"],
    },
    Method {
        ops: &["restoration."],
        name: "restoration planning ({ops})",
        refs: &["strassburg2020"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["metrics."],
        name: "landscape metrics",
        refs: &["mcgarigal1995"],
        software: "landscapemetrics",
        software_refs: &["hesselbarth2019"],
    },
    Method {
        ops: &["conn."],
        name: "habitat availability and connectivity indices",
        refs: &["saura2007"],
        software: "Conefor Sensinode",
        software_refs: &["saura2009"],
    },
    Method {
        ops: &["circuit.omniscape", "circuit.omniscape_ini"],
        name: "omnidirectional connectivity",
        refs: &["mcrae2008"],
        software: "Omniscape.jl",
        software_refs: &["landau2021"],
    },
    Method {
        ops: &["circuit."],
        name: "circuit-theory connectivity",
        refs: &["mcrae2008"],
        software: "Circuitscape",
        software_refs: &[],
    },
    Method {
        ops: &["cost."],
        name: "least-cost and random-walk distances",
        refs: &[],
        software: "gdistance",
        software_refs: &["vanetten2017"],
    },
    Method {
        ops: &["prioritize.zonation", "prioritize.zonation_project"],
        name: "spatial prioritization",
        refs: &[],
        software: "Zonation 5",
        software_refs: &["moilanen2022"],
    },
    Method {
        ops: &["prioritize.actions", "prioritize.actions_raster"],
        name: "multi-action planning",
        refs: &["salgadorojas2020"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["prioritize."],
        name: "systematic conservation planning by integer linear programming",
        refs: &[],
        software: "prioritizr",
        software_refs: &["hanson2025"],
    },
    Method {
        ops: &["sdm.maxnet", "sdm.maxnet_predict", "sdm.fit"],
        name: "species distribution models (Maxent)",
        refs: &[],
        software: "maxnet",
        software_refs: &["phillips2017"],
    },
    Method {
        ops: &[
            "sdm.checkerboard_folds",
            "sdm.jackknife_folds",
            "sdm.quadrant_folds",
            "sdm.random_folds",
            "sdm.omission_rate",
        ],
        name: "the models' partitions and evaluation",
        refs: &[],
        software: "ENMeval",
        software_refs: &["kass2021"],
    },
    Method {
        ops: &["sdm.clean_occurrences", "sdm.reference_data", "sdm.prepare"],
        name: "the cleaning of occurrence records",
        refs: &[],
        software: "CoordinateCleaner",
        software_refs: &["zizka2019"],
    },
    Method {
        ops: &["bio."],
        name: "biodiversity analyses",
        refs: &[],
        software: "Biodiverse",
        software_refs: &["laffan2010"],
    },
    Method {
        ops: &["woe.", "patches.", "dinamica."],
        name: "land change simulation by weights of evidence, the Expander and the Patcher",
        refs: &["soaresfilho2002"],
        software: "Dinamica EGO",
        software_refs: &[],
    },
    Method {
        ops: &["glm.", "lulcc.", "demand."],
        name: "land use allocation (CLUE-S)",
        refs: &["verburg2002"],
        software: "lulcc",
        software_refs: &["moulds2015"],
    },
    Method {
        ops: &["plus.", "cars."],
        name: "land use simulation (PLUS)",
        refs: &["liang2021"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["futures."],
        name: "urban growth simulation (FUTURES)",
        refs: &["meentemeyer2013"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["crafty."],
        name: "competition for land between agent functional types (CRAFTY)",
        refs: &["murrayrust2014"],
        software: "",
        software_refs: &[],
    },
];

/// Terraflux itself (the version running).
pub fn terraflux() -> Reference {
    Reference {
        key: "terraflux",
        authors: &["Lacerda, E."],
        more: false,
        year: 2026,
        title: "Terraflux: landscape ecology and land change modelling from a laptop to a cluster",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "",
    }
}

/// Where the website says what to cite.
pub const CITING: &str = "https://sacridini.github.io/terraflux/about/citing/";
const URL: &str = "https://github.com/sacridini/terraflux";

fn reference(key: &str) -> Option<&'static Reference> {
    REFERENCES.iter().find(|r| r.key == key)
}

/// The method an operator is part of.
pub fn method_of(op: &str) -> Option<&'static Method> {
    METHODS.iter().find(|m| m.ops.iter().any(|p| if p.ends_with(['.', '_']) { op.starts_with(p) } else { op == *p }))
}

/// The methods of a model, told as a paper's.
#[derive(Debug, Clone, PartialEq)]
pub struct Methods {
    /// The paragraph: Terraflux and its version, then a sentence for the methods of each software reproduced.
    pub paragraph: String,
    /// What each step does, in the order a reader follows them.
    pub steps: Vec<String>,
    /// The papers cited, in alphabetical order (Terraflux's own reference is [`Methods::software`]).
    pub references: Vec<&'static Reference>,
    /// Terraflux's reference, in the APA style.
    pub software: String,
    /// The references the model's nodes give (`refs`), as they are written.
    pub own: Vec<String>,
}

/// Every node of a model, those in containers' bodies after their container, each scope in reading order; with
/// its depth.
fn all_nodes(nodes: &[Node], depth: usize, out: &mut Vec<(usize, Node)>) {
    for n in crate::document::reading_order(nodes) {
        out.push((depth, n.clone()));
        if let Some(b) = &n.body {
            all_nodes(&b.nodes, depth + 1, out);
        }
    }
}

/// "a", "a and b", "a, b and c".
fn and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// What a step does: its operator's phrase with its settings, else the first words of its operator's
/// description and the settings given; its note after.
fn step(n: &Node, r: &Registry) -> String {
    let sig = r.sig(&n.op).map(|s| s.as_ref());
    let said = crate::phrases::sentence(n, sig).unwrap_or_else(|| {
        let doc = sig.map(|s| s.doc.as_str()).unwrap_or_default();
        let end = [" (", ": ", ". ", "; "].iter().filter_map(|p| doc.find(p)).min().unwrap_or(doc.len());
        let what = doc[..end].trim_end_matches('.').trim();
        let mut s = if what.is_empty() { n.op.clone() } else { what.to_string() };
        let settings: Vec<String> = n
            .params
            .iter()
            .filter(|(k, _)| !matches!(k.as_str(), "expression" | "script"))
            .map(|(k, v)| {
                let doc = sig.and_then(|s| s.params.iter().find(|p| &p.name == k)).map(|p| p.doc.as_str());
                let label = doc.map_or_else(|| k.clone(), |d| crate::document::setting_label(k, d));
                let mut w = crate::document::words(v);
                if w.chars().count() > 60 {
                    w = format!("{}...", w.chars().take(57).collect::<String>());
                }
                format!("{label} {w}")
            })
            .collect();
        if !settings.is_empty() {
            s.push_str(&format!(": {}", settings.join(", ")));
        }
        s.push('.');
        s
    });
    let mut out = format!("{} ({}): {said}", n.id, n.op);
    if let Some(note) = n.note.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        out.push_str(&format!(" {}", note.trim_end_matches('.')));
        out.push('.');
    }
    out
}

/// The methods of one software reproduced: the software, its papers, each method used with its operators.
type Group<'a> = (&'static str, &'static [&'static str], Vec<(&'static Method, Vec<&'a str>)>);

/// The methods of a model, for Terraflux `version`; `file` the model's file name, if it has one.
pub fn of(m: &Model, r: &Registry, version: &str, file: Option<&str>) -> Methods {
    let mut nodes = Vec::new();
    all_nodes(&m.nodes, 0, &mut nodes);
    let ops: Vec<&str> = nodes.iter().filter(|(_, n)| !n.bypass).map(|(_, n)| n.op.as_str()).collect();
    // the methods used, grouped by the software reproduced, in the order they come
    let mut groups: Vec<Group> = Vec::new();
    for op in &ops {
        let Some(me) = method_of(op) else { continue };
        let g = match groups.iter().position(|(s, sr, _)| *s == me.software && *sr == me.software_refs) {
            Some(g) => g,
            None => {
                groups.push((me.software, me.software_refs, Vec::new()));
                groups.len() - 1
            }
        };
        let used = &mut groups[g].2;
        match used.iter_mut().find(|(x, _)| std::ptr::eq(*x, me)) {
            Some((_, o)) if !o.contains(op) => o.push(op),
            Some(_) => {}
            None => used.push((me, vec![op])),
        }
    }
    let mut cited: Vec<&'static str> = Vec::new();
    let mut cite = |keys: &[&'static str]| -> Vec<String> {
        keys.iter()
            .filter_map(|k| {
                let r = reference(k)?;
                if !cited.contains(k) {
                    cited.push(k);
                }
                Some(r.cite())
            })
            .collect()
    };
    let seeds = nodes.iter().any(|(_, n)| n.params.contains_key("seed") || n.op.starts_with("rng."));
    let steps = nodes.iter().filter(|(_, n)| n.body.is_none()).count();
    let mut paragraph = format!(
        "The analysis was run in Terraflux {version} (Lacerda, 2026), as a model of {steps} step{}{}; the model's \
         file, with every setting{}, is given with this paper.",
        if steps == 1 { "" } else { "s" },
        file.map(|f| format!(" ({f})")).unwrap_or_default(),
        if seeds { " and random seed" } else { "" },
    );
    for (software, software_refs, used) in &groups {
        // each method with its papers: "name (Author, year)", or inside the name's own brackets
        let mut keys: Vec<&'static str> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        for (me, o) in used {
            let name = me.name.replace("{ops}", &o.join(", "));
            keys.extend(me.refs.iter().filter(|k| !keys.contains(k)).copied().collect::<Vec<_>>());
            let cites = cite(me.refs).join("; ");
            names.push(match (cites.is_empty(), name.strip_suffix(')')) {
                (true, _) => name,
                (false, Some(open)) => format!("{open}; {cites})"),
                (false, None) => format!("{name} ({cites})"),
            });
        }
        let mut s = format!("We computed {}", and(&names));
        s.push_str(" with Terraflux");
        if !software.is_empty() {
            s.push_str(&format!(", which reproduces {software}"));
            let own: Vec<&'static str> = software_refs.iter().copied().filter(|k| !keys.contains(k)).collect();
            let software_cites = cite(&own);
            if !software_cites.is_empty() {
                s.push_str(&format!(" ({})", software_cites.join("; ")));
            }
        }
        s.push('.');
        paragraph.push(' ');
        paragraph.push_str(&s);
    }
    let mut references: Vec<&'static Reference> = cited.iter().filter_map(|k| reference(k)).collect();
    references.sort_by_key(|r| (r.authors[0].to_lowercase(), r.year));
    let mut own: Vec<String> = Vec::new();
    for (_, n) in &nodes {
        for x in n.refs.iter().map(|x| x.trim()).filter(|x| !x.is_empty()) {
            if !own.iter().any(|o| o == x) {
                own.push(x.to_string());
            }
        }
    }
    let steps = nodes.iter().map(|(depth, n)| format!("{}{}", "    ".repeat(*depth), step(n, r))).collect::<Vec<_>>();
    Methods { paragraph, steps, references, software: software_apa(version), own }
}

/// Terraflux's reference in the APA style, for a version.
pub fn software_apa(version: &str) -> String {
    let t = terraflux();
    format!("{} ({}). {} (Version {version}) [Computer software]. {URL}", t.authors[0], t.year, t.title)
}

impl Methods {
    /// All of it as plain text: the paragraph, the steps numbered (a container's body under it), the references.
    pub fn text(&self) -> String {
        let mut out = format!("Methods\n\n{}\n\nThe model, step by step\n\n", self.paragraph);
        let mut k = 0;
        for s in &self.steps {
            if s.starts_with(' ') {
                out.push_str(&format!("{s}\n"));
            } else {
                k += 1;
                out.push_str(&format!("{k}. {s}\n"));
            }
        }
        out.push_str("\nReferences\n\n");
        let mut refs: Vec<String> = self.references.iter().map(|r| r.apa()).collect();
        refs.push(self.software.clone());
        refs.sort_by_key(|r| r.to_lowercase());
        for r in refs {
            out.push_str(&format!("{r}\n"));
        }
        if !self.own.is_empty() {
            out.push_str("\nGiven in the model's notes\n\n");
            for r in &self.own {
                out.push_str(&format!("{r}\n"));
            }
        }
        out
    }

    /// The references as BibTeX, Terraflux first.
    pub fn bibtex(&self, version: &str) -> String {
        let t = terraflux();
        let mut out = format!(
            "@software{{terraflux,\n  author      = {{{}}},\n  title       = {{{{{}}}}},\n  version     = {{{version}}},\n  \
             year        = {{{}}},\n  url         = {{{URL}}},\n  license     = {{GPL-3.0-or-later}}\n}}\n",
            t.authors[0], t.title, t.year
        );
        for r in &self.references {
            out.push('\n');
            out.push_str(&r.bibtex());
            out.push('\n');
        }
        out
    }

    /// The references as RIS, Terraflux first.
    pub fn ris(&self, version: &str) -> String {
        let t = terraflux();
        let mut out = format!(
            "TY  - COMP\nAU  - {}\nPY  - {}\nTI  - {}\nET  - {version}\nUR  - {URL}\nER  - \n",
            t.authors[0], t.year, t.title
        );
        for r in &self.references {
            out.push('\n');
            out.push_str(&r.ris());
            out.push('\n');
        }
        out
    }
}

/// The window of the model's methods and citations, while the session asks for it: the paragraph, the steps,
/// the references; copied, or saved as BibTeX or RIS.
pub fn window(ctx: &egui::Context, s: &mut crate::app::Session, r: &Registry) {
    use egui::RichText;
    if !s.methods_open {
        return;
    }
    let version = env!("CARGO_PKG_VERSION");
    let file = s.path.as_ref().and_then(|p| p.file_name()).map(|f| f.to_string_lossy().into_owned());
    let m = of(&s.source, r, version, file.as_deref());
    let said_id = egui::Id::new("methods said");
    let mut open = true;
    egui::Window::new("Methods and citations")
        .open(&mut open)
        .default_size([660.0, 560.0])
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            let weak = ui.visuals().weak_text_color();
            ui.label(
                RichText::new(
                    "A draft of the methods section, written from the model: check it, and add why each step was \
                     chosen. The papers are those of the methods Terraflux reproduces.",
                )
                .size(11.5)
                .color(weak),
            );
            ui.hyperlink_to(RichText::new("What to cite, on the website").size(11.5), CITING);
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                let mut said = None;
                if ui.button("Copy all").on_hover_text("the paragraph, the steps and the references, as text").clicked()
                {
                    ui.ctx().copy_text(m.text());
                    said = Some("copied".to_string());
                }
                if ui.button("Copy the paragraph").clicked() {
                    ui.ctx().copy_text(m.paragraph.clone());
                    said = Some("copied".to_string());
                }
                if ui.button("Copy BibTeX").on_hover_text("for LaTeX, Zotero, Mendeley...").clicked() {
                    ui.ctx().copy_text(m.bibtex(version));
                    said = Some("copied".to_string());
                }
                let save = |filter: &str, ext: &str, text: String| -> Option<String> {
                    let path = rfd::FileDialog::new()
                        .set_title("Save the references")
                        .add_filter(filter, &[ext])
                        .set_file_name(format!("references.{ext}"))
                        .save_file()?;
                    Some(match std::fs::write(&path, text) {
                        Ok(()) => format!("saved: {}", path.display()),
                        Err(e) => format!("not saved: {e}"),
                    })
                };
                if ui.button("Save BibTeX...").clicked() {
                    said = save("BibTeX", "bib", m.bibtex(version));
                }
                if ui.button("Save RIS...").on_hover_text("for EndNote, Zotero, Mendeley...").clicked() {
                    said = save("RIS", "ris", m.ris(version));
                }
                if let Some(t) = said {
                    ui.data_mut(|d| d.insert_temp(said_id, t));
                }
                if let Some(t) = ui.data(|d| d.get_temp::<String>(said_id)) {
                    ui.label(RichText::new(t).size(11.0).color(weak));
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                crate::theme::section(ui, "Methods");
                ui.add(egui::Label::new(&m.paragraph).selectable(true).wrap());
                ui.add_space(10.0);
                crate::theme::section(ui, "The model, step by step");
                let mut k = 0;
                for st in &m.steps {
                    let depth = st.len() - st.trim_start().len();
                    let text = if depth > 0 {
                        st.trim_start().to_string()
                    } else {
                        k += 1;
                        format!("{k}. {st}")
                    };
                    ui.horizontal_top(|ui| {
                        ui.add_space(depth as f32 * 4.0);
                        ui.add(egui::Label::new(RichText::new(text).size(12.5)).selectable(true).wrap());
                    });
                }
                ui.add_space(10.0);
                crate::theme::section(ui, "References");
                let mut refs: Vec<String> = m.references.iter().map(|r| r.apa()).collect();
                refs.push(m.software.clone());
                refs.sort_by_key(|r| r.to_lowercase());
                for x in &refs {
                    ui.add(egui::Label::new(RichText::new(x).size(12.5)).selectable(true).wrap());
                    ui.add_space(2.0);
                }
                if !m.own.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Given in the model's notes (refs)").size(11.5).color(weak));
                    for x in &m.own {
                        ui.add(egui::Label::new(RichText::new(x).size(12.5)).selectable(true).wrap());
                    }
                }
            });
        });
    s.methods_open = open;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_are_cited_and_written() {
        let r = |k| reference(k).unwrap();
        assert_eq!(r("soille2009").cite(), "Soille & Vogt, 2009");
        assert_eq!(r("vanetten2017").cite(), "van Etten, 2017");
        assert_eq!(r("hesselbarth2019").cite(), "Hesselbarth et al., 2019");
        assert_eq!(r("strassburg2020").cite(), "Strassburg et al., 2020");
        // as the website writes them
        assert_eq!(
            r("soille2009").apa(),
            "Soille, P., & Vogt, P. (2009). Morphological segmentation of binary patterns. Pattern Recognition \
             Letters, 30(4), 456–459. https://doi.org/10.1016/j.patrec.2008.10.015"
        );
        assert_eq!(
            r("zizka2019").apa(),
            "Zizka, A., Silvestro, D., Andermann, T., Azevedo, J., Duarte Ritter, C., Edler, D., et al. (2019). \
             CoordinateCleaner: Standardized cleaning of occurrence records from biological collection databases. \
             Methods in Ecology and Evolution, 10(5), 744–751. https://doi.org/10.1111/2041-210X.13152"
        );
        let b = r("soille2009").bibtex();
        assert!(b.starts_with("@article{soille2009,") && b.contains("pages       = {456--459}"), "{b}");
        assert!(r("mcgarigal1995").bibtex().starts_with("@techreport{mcgarigal1995,"));
        let ris = r("soille2009").ris();
        assert!(ris.contains("SP  - 456\nEP  - 459") && ris.ends_with("ER  - "), "{ris}");
        // every key is unique, every method's papers are listed
        for (i, a) in REFERENCES.iter().enumerate() {
            assert!(REFERENCES[i + 1..].iter().all(|b| b.key != a.key), "{}", a.key);
        }
        for m in METHODS {
            for k in m.refs.iter().chain(m.software_refs) {
                assert!(reference(k).is_some(), "{k}");
            }
        }
    }

    #[test]
    fn every_method_names_operators_that_exist() {
        let r = Registry::standard();
        let names: Vec<String> = r.signatures().into_iter().map(|s| s.name).collect();
        for m in METHODS {
            for p in m.ops {
                let ok = if p.ends_with(['.', '_']) {
                    names.iter().any(|n| n.starts_with(p))
                } else {
                    names.iter().any(|n| n == p)
                };
                assert!(ok, "no operator {p}");
            }
        }
        assert_eq!(method_of("landscape.mspa").unwrap().refs, &["soille2009"]);
        assert_eq!(method_of("landscape.spa").unwrap().name, "pattern analyses ({ops})");
        assert_eq!(method_of("restoration.status").unwrap().software, GTB);
        assert_eq!(method_of("restoration.plan").unwrap().refs, &["strassburg2020"]);
        assert_eq!(method_of("prioritize.zonation_project").unwrap().software, "Zonation 5");
        assert_eq!(method_of("prioritize.actions_raster").unwrap().refs, &["salgadorojas2020"]);
        assert!(method_of("raster.reclassify").is_none());
    }

    #[test]
    fn a_model_told_as_a_paper() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "raster.read"
params = { path = "forest.tif" }

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.raster" }
note = "The forest's pattern"
refs = ["Vogt, P. (2024). My own paper."]

[[nodes]]
id = "spa"
op = "landscape.spa"
inputs = { map = "forest.raster" }

[[nodes]]
id = "metrics"
op = "metrics.landscape"
inputs = { map = "mspa.map" }
"#,
        )
        .unwrap();
        let x = of(&m, &r, "1.28.0", Some("forest.toml"));
        assert!(
            x.paragraph.starts_with(
                "The analysis was run in Terraflux 1.28.0 (Lacerda, 2026), as a model of 4 steps (forest.toml)"
            ),
            "{}",
            x.paragraph
        );
        // GuidosToolbox's methods in one sentence, the metrics in another
        assert!(
            x.paragraph.contains(
                "We computed morphological spatial pattern analysis (MSPA; Soille & Vogt, 2009) and pattern \
                 analyses (landscape.spa) with Terraflux, which reproduces GuidosToolbox (Vogt & Riitters, 2017)."
            ),
            "{}",
            x.paragraph
        );
        assert!(
            x.paragraph.contains(
                "We computed landscape metrics (McGarigal & Marks, 1995) with Terraflux, which reproduces \
                 landscapemetrics (Hesselbarth et al., 2019)."
            ),
            "{}",
            x.paragraph
        );
        let keys: Vec<&str> = x.references.iter().map(|r| r.key).collect();
        assert_eq!(keys, ["hesselbarth2019", "mcgarigal1995", "soille2009", "vogt2017"]);
        assert_eq!(x.steps.len(), 4);
        assert!(x.steps[0].starts_with("forest (raster.read): Reads the rasters of forest.tif"), "{}", x.steps[0]);
        assert!(
            x.steps.iter().any(|s| s.starts_with("mspa (landscape.mspa): ") && s.ends_with("The forest's pattern."))
        );
        assert_eq!(x.own, ["Vogt, P. (2024). My own paper."]);
        let text = x.text();
        assert!(text.contains("\nReferences\n\n") && text.contains("Lacerda, E. (2026). Terraflux"), "{text}");
        assert!(text.contains("Given in the model's notes"));
        let bib = x.bibtex("1.28.0");
        assert!(bib.starts_with("@software{terraflux,") && bib.contains("version     = {1.28.0}"), "{bib}");
        assert_eq!(bib.matches("\n@article{").count(), 3);
        assert!(x.ris("1.28.0").starts_with("TY  - COMP"));
    }
}
