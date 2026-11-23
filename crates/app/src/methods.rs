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
        "achanta2012",
        ["Achanta, R.", "Shaji, A.", "Smith, K.", "Lucchi, A.", "Fua, P.", "Süsstrunk, S."],
        false,
        2012,
        "SLIC superpixels compared to state-of-the-art superpixel methods",
        "IEEE Transactions on Pattern Analysis and Machine Intelligence",
        "34",
        "11",
        "2274–2282",
        "10.1109/TPAMI.2012.120"
    ),
    Reference {
        key: "buckner2001",
        authors: &["Bückner, J.", "Pahl, M.", "Stahlhut, O.", "Liedtke, C.-E."],
        more: false,
        year: 2001,
        title: "GeoAIDA: a knowledge based automatic image data analyser for remote sensing data",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "Proceedings of the Second International ICSC Symposium on Advances in Intelligent Data Analysis (CIMA 2001).",
    },
    Reference {
        key: "costa2010",
        authors: &["Costa, G. A. O. P."],
        more: true,
        year: 2010,
        title: "Knowledge-based interpretation of remote sensing data with the InterIMAGE system: major characteristics and recent developments",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "Proceedings of the 3rd International Conference on Geographic Object-Based Image Analysis (GEOBIA 2010).",
    },
    Reference {
        key: "feitosa2009",
        authors: &["Feitosa, R. Q.", "Costa, G. A. O. P.", "Mota, G. L. A.", "Pakzad, K.", "Costa, M. C. O."],
        more: false,
        year: 2009,
        title: "Cascade multitemporal classification based on fuzzy Markov chains",
        journal: "ISPRS Journal of Photogrammetry and Remote Sensing",
        volume: "64",
        issue: "2",
        pages: "159-170",
        doi: "",
        report: "",
    },
    Reference {
        key: "kirillov2023",
        authors: &["Kirillov, A."],
        more: true,
        year: 2023,
        title: "Segment anything",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "Proceedings of the IEEE/CVF International Conference on Computer Vision (ICCV), 4015–4026.",
    },
    paper!(
        "olofsson2014",
        ["Olofsson, P.", "Foody, G. M.", "Herold, M.", "Stehman, S. V.", "Woodcock, C. E.", "Wulder, M. A."],
        false,
        2014,
        "Good practices for estimating area and assessing accuracy of land change",
        "Remote Sensing of Environment",
        "148",
        "",
        "42–57",
        "10.1016/j.rse.2014.02.015"
    ),
    paper!(
        "radoux2017",
        ["Radoux, J.", "Bogaert, P."],
        false,
        2017,
        "Good practices for object-based accuracy assessment",
        "Remote Sensing",
        "9",
        "7",
        "646",
        "10.3390/rs9070646"
    ),
    paper!(
        "stehman2014",
        ["Stehman, S. V."],
        false,
        2014,
        "Estimating area and map accuracy for stratified random sampling when the strata are different from the map classes",
        "International Journal of Remote Sensing",
        "35",
        "13",
        "4923–4939",
        "10.1080/01431161.2014.930207"
    ),
    paper!(
        "breiman2001",
        ["Breiman, L."],
        false,
        2001,
        "Random forests",
        "Machine Learning",
        "45",
        "1",
        "5–32",
        "10.1023/A:1010933404324"
    ),
    paper!(
        "elith2010",
        ["Elith, J.", "Kearney, M.", "Phillips, S."],
        false,
        2010,
        "The art of modelling range-shifting species",
        "Methods in Ecology and Evolution",
        "1",
        "4",
        "330–342",
        "10.1111/j.2041-210X.2010.00036.x"
    ),
    paper!(
        "owens2013",
        [
            "Owens, H. L.",
            "Campbell, L. P.",
            "Dornak, L. L.",
            "Saupe, E. E.",
            "Barve, N.",
            "Soberón, J.",
            "Ingenloff, K.",
            "Lira-Noriega, A.",
            "Hensz, C. M.",
            "Myers, C. E.",
            "Peterson, A. T."
        ],
        false,
        2013,
        "Constraints on interpretation of ecological niche models by limited environmental ranges on calibration areas",
        "Ecological Modelling",
        "263",
        "",
        "10–18",
        "10.1016/j.ecolmodel.2013.04.011"
    ),
    paper!(
        "mesgaran2014",
        ["Mesgaran, M. B.", "Cousens, R. D.", "Webber, B. L."],
        false,
        2014,
        "Here be dragons: a tool for quantifying novelty due to covariate range and correlation change when projecting species distribution models",
        "Diversity and Distributions",
        "20",
        "10",
        "1147–1159",
        "10.1111/ddi.12209"
    ),
    paper!(
        "vanderwalt2014",
        [
            "van der Walt, S.",
            "Schönberger, J. L.",
            "Nunez-Iglesias, J.",
            "Boulogne, F.",
            "Warner, J. D.",
            "Yager, N.",
            "Gouillart, E.",
            "Yu, T."
        ],
        false,
        2014,
        "scikit-image: image processing in Python",
        "PeerJ",
        "2",
        "",
        "e453",
        "10.7717/peerj.453"
    ),
    paper!(
        "friedman2001",
        ["Friedman, J. H."],
        false,
        2001,
        "Greedy function approximation: a gradient boosting machine",
        "The Annals of Statistics",
        "29",
        "5",
        "1189–1232",
        "10.1214/aos/1013203451"
    ),
    Reference {
        key: "achanta2017",
        authors: &["Achanta, R.", "Süsstrunk, S."],
        more: false,
        year: 2017,
        title: "Superpixels and polygons using simple non-iterative clustering",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "Proceedings of the IEEE Conference on Computer Vision and Pattern Recognition (CVPR), 4651–4660.",
    },
    Reference {
        key: "baatz2000",
        authors: &["Baatz, M.", "Schäpe, A."],
        more: false,
        year: 2000,
        title: "Multiresolution segmentation: an optimization approach for high quality multi-scale image segmentation",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "In J. Strobl, T. Blaschke & G. Griesebner (Eds.), Angewandte Geographische Informationsverarbeitung XII (pp. 12–23). Wichmann.",
    },
    paper!(
        "benz2004",
        ["Benz, U. C.", "Hofmann, P.", "Willhauck, G.", "Lingenfelder, I.", "Heynen, M."],
        false,
        2004,
        "Multi-resolution, object-oriented fuzzy analysis of remote sensing data for GIS-ready information",
        "ISPRS Journal of Photogrammetry and Remote Sensing",
        "58",
        "3–4",
        "239–258",
        "10.1016/j.isprsjprs.2003.10.002"
    ),
    paper!(
        "zadeh1965",
        ["Zadeh, L. A."],
        false,
        1965,
        "Fuzzy sets",
        "Information and Control",
        "8",
        "3",
        "338–353",
        "10.1016/S0019-9958(65)90241-X"
    ),
    paper!(
        "comaniciu2002",
        ["Comaniciu, D.", "Meer, P."],
        false,
        2002,
        "Mean shift: a robust approach toward feature space analysis",
        "IEEE Transactions on Pattern Analysis and Machine Intelligence",
        "24",
        "5",
        "603–619",
        "10.1109/34.1000236"
    ),
    paper!(
        "dragut2010",
        ["Drăguţ, L.", "Tiede, D.", "Levick, S. R."],
        false,
        2010,
        "ESP: a tool to estimate scale parameter for multiresolution image segmentation of remotely sensed data",
        "International Journal of Geographical Information Science",
        "24",
        "6",
        "859–871",
        "10.1080/13658810903174803"
    ),
    paper!(
        "dragut2014",
        ["Drăguţ, L.", "Csillik, O.", "Eisank, C.", "Tiede, D."],
        false,
        2014,
        "Automated parameterisation for multi-scale image segmentation on multiple layers",
        "ISPRS Journal of Photogrammetry and Remote Sensing",
        "88",
        "",
        "119–127",
        "10.1016/j.isprsjprs.2013.11.018"
    ),
    paper!(
        "espindola2006",
        ["Espindola, G. M.", "Câmara, G.", "Reis, I. A.", "Bins, L. S.", "Monteiro, A. M."],
        false,
        2006,
        "Parameter selection for region-growing image segmentation algorithms using spatial autocorrelation",
        "International Journal of Remote Sensing",
        "27",
        "14",
        "3035–3040",
        "10.1080/01431160600617194"
    ),
    paper!(
        "johnson2011",
        ["Johnson, B.", "Xie, Z."],
        false,
        2011,
        "Unsupervised image segmentation evaluation and refinement using a multi-scale approach",
        "ISPRS Journal of Photogrammetry and Remote Sensing",
        "66",
        "4",
        "473–483",
        "10.1016/j.isprsjprs.2011.02.006"
    ),
    paper!(
        "felzenszwalb2004",
        ["Felzenszwalb, P. F.", "Huttenlocher, D. P."],
        false,
        2004,
        "Efficient graph-based image segmentation",
        "International Journal of Computer Vision",
        "59",
        "2",
        "167–181",
        "10.1023/B:VISI.0000022288.19776.77"
    ),
    Reference {
        key: "otsu1979",
        authors: &["Otsu, N."],
        more: false,
        year: 1979,
        title: "A threshold selection method from gray-level histograms",
        journal: "IEEE Transactions on Systems, Man, and Cybernetics",
        volume: "9",
        issue: "1",
        pages: "62–66",
        doi: "10.1109/TSMC.1979.4310076",
        report: "",
    },
    Reference {
        key: "liao2001",
        authors: &["Liao, P.-S.", "Chen, T.-S.", "Chung, P.-C."],
        more: false,
        year: 2001,
        title: "A fast algorithm for multilevel thresholding",
        journal: "Journal of Information Science and Engineering",
        volume: "17",
        issue: "5",
        pages: "713–727",
        doi: "",
        report: "",
    },
    Reference {
        key: "pearson1901",
        authors: &["Pearson, K."],
        more: false,
        year: 1901,
        title: "On lines and planes of closest fit to systems of points in space",
        journal: "The London, Edinburgh, and Dublin Philosophical Magazine and Journal of Science",
        volume: "2",
        issue: "11",
        pages: "559-572",
        doi: "10.1080/14786440109462720",
        report: "",
    },
    Reference {
        key: "hotelling1933",
        authors: &["Hotelling, H."],
        more: false,
        year: 1933,
        title: "Analysis of a complex of statistical variables into principal components",
        journal: "Journal of Educational Psychology",
        volume: "24",
        issue: "6",
        pages: "417-441",
        doi: "10.1037/h0071325",
        report: "",
    },
    Reference {
        key: "green1988",
        authors: &["Green, A. A.", "Berman, M.", "Switzer, P.", "Craig, M. D."],
        more: false,
        year: 1988,
        title: "A transformation for ordering multispectral data in terms of image quality with implications for noise removal",
        journal: "IEEE Transactions on Geoscience and Remote Sensing",
        volume: "26",
        issue: "1",
        pages: "65-74",
        doi: "10.1109/36.3001",
        report: "",
    },
    Reference {
        key: "lee1990",
        authors: &["Lee, J. B.", "Woodyatt, A. S.", "Berman, M."],
        more: false,
        year: 1990,
        title: "Enhancement of high spectral resolution remote-sensing data by a noise-adjusted principal components transform",
        journal: "IEEE Transactions on Geoscience and Remote Sensing",
        volume: "28",
        issue: "3",
        pages: "295-304",
        doi: "10.1109/36.54356",
        report: "",
    },
    Reference {
        key: "clinton2010",
        authors: &["Clinton, N.", "Holt, A.", "Scarborough, J.", "Yan, L.", "Gong, P."],
        more: false,
        year: 2010,
        title: "Accuracy assessment measures for object-based image segmentation goodness",
        journal: "Photogrammetric Engineering & Remote Sensing",
        volume: "76",
        issue: "3",
        pages: "289-299",
        doi: "10.14358/pers.76.3.289",
        report: "",
    },
    Reference {
        key: "lucieer2002",
        authors: &["Lucieer, A.", "Stein, A."],
        more: false,
        year: 2002,
        title: "Existential uncertainty of spatial objects segmented from satellite sensor imagery",
        journal: "IEEE Transactions on Geoscience and Remote Sensing",
        volume: "40",
        issue: "11",
        pages: "2518-2521",
        doi: "10.1109/tgrs.2002.805072",
        report: "",
    },
    Reference {
        key: "weidner2008",
        authors: &["Weidner, U."],
        more: false,
        year: 2008,
        title: "Contribution to the assessment of segmentation quality for remote sensing applications",
        journal: "International Archives of the Photogrammetry, Remote Sensing and Spatial Information Sciences",
        volume: "37",
        issue: "B7",
        pages: "479-484",
        doi: "",
        report: "",
    },
    Reference {
        key: "liu2012",
        authors: &["Liu, Y.", "Bian, L.", "Meng, Y.", "Wang, H.", "Zhang, S.", "Yang, Y.", "Shao, X.", "Wang, B."],
        more: false,
        year: 2012,
        title: "Discrepancy measures for selecting optimal combination of parameter values in object-based image analysis",
        journal: "ISPRS Journal of Photogrammetry and Remote Sensing",
        volume: "68",
        issue: "",
        pages: "144-156",
        doi: "10.1016/j.isprsjprs.2012.01.007",
        report: "",
    },
    Reference {
        key: "zhang2015",
        authors: &["Zhang, X.", "Feng, X.", "Xiao, P.", "He, G.", "Zhu, L."],
        more: false,
        year: 2015,
        title: "Segmentation quality evaluation using region-based precision and recall measures for remote sensing images",
        journal: "ISPRS Journal of Photogrammetry and Remote Sensing",
        volume: "102",
        issue: "",
        pages: "73-84",
        doi: "10.1016/j.isprsjprs.2015.01.009",
        report: "",
    },
    Reference {
        key: "costa2018",
        authors: &["Costa, H.", "Foody, G. M.", "Boyd, D. S."],
        more: false,
        year: 2018,
        title: "Supervised methods of image segmentation accuracy assessment in land cover mapping",
        journal: "Remote Sensing of Environment",
        volume: "205",
        issue: "",
        pages: "338-351",
        doi: "10.1016/j.rse.2017.11.024",
        report: "",
    },
    Reference {
        key: "jonsson2004",
        authors: &["Jönsson, P.", "Eklundh, L."],
        more: false,
        year: 2004,
        title: "TIMESAT: a program for analyzing time-series of satellite sensor data",
        journal: "Computers & Geosciences",
        volume: "30",
        issue: "8",
        pages: "833-845",
        doi: "10.1016/j.cageo.2004.05.006",
        report: "",
    },
    Reference {
        key: "savitzky1964",
        authors: &["Savitzky, A.", "Golay, M. J. E."],
        more: false,
        year: 1964,
        title: "Smoothing and differentiation of data by simplified least squares procedures",
        journal: "Analytical Chemistry",
        volume: "36",
        issue: "8",
        pages: "1627-1639",
        doi: "10.1021/ac60214a047",
        report: "",
    },
    Reference {
        key: "jakubauskas2001",
        authors: &["Jakubauskas, M. E.", "Legates, D. R.", "Kastens, J. H."],
        more: false,
        year: 2001,
        title: "Harmonic analysis of time-series AVHRR NDVI data",
        journal: "Photogrammetric Engineering & Remote Sensing",
        volume: "67",
        issue: "4",
        pages: "461-470",
        doi: "",
        report: "",
    },
    Reference {
        key: "zhu2014",
        authors: &["Zhu, Z.", "Woodcock, C. E."],
        more: false,
        year: 2014,
        title: "Continuous change detection and classification of land cover using all available Landsat data",
        journal: "Remote Sensing of Environment",
        volume: "144",
        issue: "",
        pages: "152-171",
        doi: "10.1016/j.rse.2014.01.011",
        report: "",
    },
    Reference {
        key: "ploton2020",
        authors: &["Ploton, P.", "Mortier, F.", "Réjou-Méchain, M.", "Barbier, N.", "Picard, N.", "Rossi, V.", "Dormann, C.", "Cornu, G.", "Viennois, G.", "Bayol, N.", "Lyapustin, A.", "Gourlet-Fleury, S.", "Pélissier, R."],
        more: false,
        year: 2020,
        title: "Spatial validation reveals poor predictive performance of large-scale ecological mapping models",
        journal: "Nature Communications",
        volume: "11",
        issue: "1",
        pages: "4540",
        doi: "10.1038/s41467-020-18321-y",
        report: "",
    },
    Reference {
        key: "meyer2022",
        authors: &["Meyer, H.", "Pebesma, E."],
        more: false,
        year: 2022,
        title: "Machine learning-based global maps of ecological variables and the challenge of assessing them",
        journal: "Nature Communications",
        volume: "13",
        issue: "1",
        pages: "2208",
        doi: "10.1038/s41467-022-29838-9",
        report: "",
    },
    Reference {
        key: "roberts2017",
        authors: &["Roberts, D. R.", "Bahn, V.", "Ciuti, S.", "Boyce, M. S.", "Elith, J.", "Guillera-Arroita, G.", "Hauenstein, S.", "Lahoz-Monfort, J. J.", "Schröder, B.", "Thuiller, W.", "Warton, D. I.", "Wintle, B. A.", "Hartig, F.", "Dormann, C. F."],
        more: false,
        year: 2017,
        title: "Cross-validation strategies for data with temporal, spatial, hierarchical, or phylogenetic structure",
        journal: "Ecography",
        volume: "40",
        issue: "8",
        pages: "913-929",
        doi: "10.1111/ecog.02881",
        report: "",
    },
    Reference {
        key: "byrne1980",
        authors: &["Byrne, G. F.", "Crapper, P. F.", "Mayo, K. K."],
        more: false,
        year: 1980,
        title: "Monitoring land-cover change by principal component analysis of multitemporal Landsat data",
        journal: "Remote Sensing of Environment",
        volume: "10",
        issue: "3",
        pages: "175-184",
        doi: "10.1016/0034-4257(80)90021-8",
        report: "",
    },
    Reference {
        key: "fung1987",
        authors: &["Fung, T.", "LeDrew, E."],
        more: false,
        year: 1987,
        title: "Application of principal components analysis to change detection",
        journal: "Photogrammetric Engineering and Remote Sensing",
        volume: "53",
        issue: "12",
        pages: "1649-1658",
        doi: "",
        report: "",
    },
    Reference {
        key: "chavez1989",
        authors: &["Chavez, P. S.", "Kwarteng, A. Y."],
        more: false,
        year: 1989,
        title: "Extracting spectral contrast in Landsat Thematic Mapper image data using selective principal component analysis",
        journal: "Photogrammetric Engineering and Remote Sensing",
        volume: "55",
        issue: "3",
        pages: "339-348",
        doi: "",
        report: "",
    },
    Reference {
        key: "lloyd1982",
        authors: &["Lloyd, S. P."],
        more: false,
        year: 1982,
        title: "Least squares quantization in PCM",
        journal: "IEEE Transactions on Information Theory",
        volume: "28",
        issue: "2",
        pages: "129-137",
        doi: "10.1109/TIT.1982.1056489",
        report: "",
    },
    Reference {
        key: "arthur2007",
        authors: &["Arthur, D.", "Vassilvitskii, S."],
        more: false,
        year: 2007,
        title: "k-means++: the advantages of careful seeding",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "Proceedings of the Eighteenth Annual ACM-SIAM Symposium on Discrete Algorithms (SODA '07), 1027–1035.",
    },
    Reference {
        key: "rousseeuw1987",
        authors: &["Rousseeuw, P. J."],
        more: false,
        year: 1987,
        title: "Silhouettes: a graphical aid to the interpretation and validation of cluster analysis",
        journal: "Journal of Computational and Applied Mathematics",
        volume: "20",
        issue: "",
        pages: "53-65",
        doi: "10.1016/0377-0427(87)90125-7",
        report: "",
    },
    Reference {
        key: "kohonen1982",
        authors: &["Kohonen, T."],
        more: false,
        year: 1982,
        title: "Self-organized formation of topologically correct feature maps",
        journal: "Biological Cybernetics",
        volume: "43",
        issue: "1",
        pages: "59–69",
        doi: "10.1007/BF00337288",
        report: "",
    },
    Reference {
        key: "kohonen2001",
        authors: &["Kohonen, T."],
        more: false,
        year: 2001,
        title: "Self-Organizing Maps (3rd ed.)",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "Springer Series in Information Sciences 30. Springer, Berlin.",
    },
    Reference {
        key: "vesanto2000",
        authors: &["Vesanto, J.", "Alhoniemi, E."],
        more: false,
        year: 2000,
        title: "Clustering of the self-organizing map",
        journal: "IEEE Transactions on Neural Networks",
        volume: "11",
        issue: "3",
        pages: "586–600",
        doi: "10.1109/72.846731",
        report: "",
    },
    Reference {
        key: "momsen2012",
        authors: &["Momsen, E.", "Metz, M."],
        more: false,
        year: 2012,
        title: "i.segment: identifies segments (objects) from imagery data (region growing and merging)",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "GRASS GIS manual (first developed in the Google Summer of Code 2012), Open Source Geospatial Foundation.",
    },
    Reference {
        key: "vedaldi2008",
        authors: &["Vedaldi, A.", "Soatto, S."],
        more: false,
        year: 2008,
        title: "Quick shift and kernel methods for mode seeking",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "10.1007/978-3-540-88693-8_52",
        report: "In D. Forsyth, P. Torr & A. Zisserman (Eds.), Computer Vision – ECCV 2008, Lecture Notes in Computer Science 5305 (pp. 705–718). Springer.",
    },
    Reference {
        key: "trimble2022",
        authors: &["Trimble Germany GmbH"],
        more: false,
        year: 2022,
        title: "eCognition Developer reference book: spectral difference segmentation",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "Trimble Germany GmbH, Munich.",
    },
    Reference {
        key: "neubert2014",
        authors: &["Neubert, P.", "Protzel, P."],
        more: false,
        year: 2014,
        title: "Compact watershed and preemptive SLIC: on improving trade-offs of superpixel segmentation algorithms",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "22nd International Conference on Pattern Recognition (ICPR), 996–1001.",
    },
    paper!(
        "vincent1991",
        ["Vincent, L.", "Soille, P."],
        false,
        1991,
        "Watersheds in digital spaces: an efficient algorithm based on immersion simulations",
        "IEEE Transactions on Pattern Analysis and Machine Intelligence",
        "13",
        "6",
        "583–598",
        "10.1109/34.87344"
    ),
    paper!(
        "carpenter2006",
        ["Carpenter, A. E.", "Jones, T. R.", "Lamprecht, M. R.", "Clarke, C.", "Kang, I. H.", "Friman, O."],
        true,
        2006,
        "CellProfiler: image analysis software for identifying and quantifying cell phenotypes",
        "Genome Biology",
        "7",
        "10",
        "R100",
        "10.1186/gb-2006-7-10-r100"
    ),
    Reference {
        key: "serra1982",
        authors: &["Serra, J."],
        more: false,
        year: 1982,
        title: "Image Analysis and Mathematical Morphology",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "Academic Press, London.",
    },
    Reference {
        key: "soille2004",
        authors: &["Soille, P."],
        more: false,
        year: 2004,
        title: "Morphological Image Analysis: Principles and Applications (2nd ed.)",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "10.1007/978-3-662-05088-0",
        report: "Springer, Berlin, Heidelberg.",
    },
    paper!(
        "felzenszwalb2012",
        ["Felzenszwalb, P. F.", "Huttenlocher, D. P."],
        false,
        2012,
        "Distance transforms of sampled functions",
        "Theory of Computing",
        "8",
        "1",
        "415–428",
        "10.4086/toc.2012.v008a019"
    ),
    paper!(
        "avgar2016",
        ["Avgar, T.", "Potts, J. R.", "Lewis, M. A.", "Boyce, M. S."],
        false,
        2016,
        "Integrated step selection analysis: bridging the gap between resource selection and animal movement",
        "Methods in Ecology and Evolution",
        "7",
        "5",
        "619–630",
        "10.1111/2041-210X.12528"
    ),
    paper!(
        "avgar2017",
        ["Avgar, T.", "Lele, S. R.", "Keim, J. L.", "Boyce, M. S."],
        false,
        2017,
        "Relative selection strength: quantifying effect size in habitat- and step-selection inference",
        "Ecology and Evolution",
        "7",
        "14",
        "5322–5330",
        "10.1002/ece3.3122"
    ),
    paper!(
        "berti2022",
        [
            "Berti, E.", "Davoli, M.", "Buitenwerf, R.", "Dyer, A.", "Hansen, O. L. P.", "Hirt, M.", "Svenning, J.-C.",
            "Terlau, J. F.", "Brose, U.", "Vollrath, F."
        ],
        false,
        2022,
        "The r package enerscape: a general energy landscape framework for terrestrial movement ecology",
        "Methods in Ecology and Evolution",
        "13",
        "1",
        "60–67",
        "10.1111/2041-210X.13734"
    ),
    paper!(
        "campolongo2007",
        ["Campolongo, F.", "Cariboni, J.", "Saltelli, A."],
        false,
        2007,
        "An effective screening design for sensitivity analysis of large models",
        "Environmental Modelling & Software",
        "22",
        "10",
        "1509–1518",
        "10.1016/j.envsoft.2006.10.004"
    ),
    Reference {
        key: "cuturi2013",
        authors: &["Cuturi, M."],
        more: false,
        year: 2013,
        title: "Sinkhorn distances: lightspeed computation of optimal transport",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "Advances in Neural Information Processing Systems, 26, 2292–2300. https://arxiv.org/abs/1306.0895",
    },
    paper!(
        "fieberg2005",
        ["Fieberg, J.", "Kochanny, C. O."],
        false,
        2005,
        "Quantifying home-range overlap: the importance of the utilization distribution",
        "Journal of Wildlife Management",
        "69",
        "4",
        "1346–1359",
        "10.2193/0022-541X(2005)69[1346:QHOTIO]2.0.CO;2"
    ),
    paper!(
        "fortin2005",
        ["Fortin, D.", "Beyer, H. L.", "Boyce, M. S.", "Smith, D. W.", "Duchesne, T.", "Mao, J. S."],
        false,
        2005,
        "Wolves influence elk movements: behavior shapes a trophic cascade in Yellowstone National Park",
        "Ecology",
        "86",
        "5",
        "1320–1330",
        "10.1890/04-0953"
    ),
    paper!(
        "gail1981",
        ["Gail, M. H.", "Lubin, J. H.", "Rubinstein, L. V."],
        false,
        1981,
        "Likelihood calculations for matched case-control studies and survival studies with tied death times",
        "Biometrika",
        "68",
        "3",
        "703–707",
        "10.1093/biomet/68.3.703"
    ),
    paper!(
        "getz2007",
        ["Getz, W. M.", "Fortmann-Roe, S.", "Cross, P. C.", "Lyons, A. J.", "Ryan, S. J.", "Wilmers, C. C."],
        false,
        2007,
        "LoCoH: nonparameteric kernel methods for constructing home ranges and utilization distributions",
        "PLoS ONE",
        "2",
        "2",
        "e207",
        "10.1371/journal.pone.0000207"
    ),
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
        "herman2017",
        ["Herman, J.", "Usher, W."],
        false,
        2017,
        "SALib: An open-source Python library for sensitivity analysis",
        "Journal of Open Source Software",
        "2",
        "9",
        "97",
        "10.21105/joss.00097"
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
        "hirt2017",
        ["Hirt, M. R.", "Jetz, W.", "Rall, B. C.", "Brose, U."],
        false,
        2017,
        "A general scaling law reveals why the largest animals are not the fastest",
        "Nature Ecology & Evolution",
        "1",
        "8",
        "1116–1122",
        "10.1038/s41559-017-0241-4"
    ),
    paper!(
        "horne2007",
        ["Horne, J. S.", "Garton, E. O.", "Krone, S. M.", "Lewis, J. S."],
        false,
        2007,
        "Analyzing animal movements using Brownian bridges",
        "Ecology",
        "88",
        "9",
        "2354–2363",
        "10.1890/06-0957.1"
    ),
    paper!(
        "iwanaga2022",
        ["Iwanaga, T.", "Usher, W.", "Herman, J."],
        false,
        2022,
        "Toward SALib 2.0: Advancing the accessibility and interpretability of global sensitivity analyses",
        "Socio-Environmental Systems Modelling",
        "4",
        "",
        "18155",
        "10.18174/sesmo.18155"
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
        "kranstauber2012",
        ["Kranstauber, B.", "Kays, R.", "LaPoint, S. D.", "Wikelski, M.", "Safi, K."],
        false,
        2012,
        "A dynamic Brownian bridge movement model to estimate utilization distributions for heterogeneous animal \
         movement",
        "Journal of Animal Ecology",
        "81",
        "4",
        "738–746",
        "10.1111/j.1365-2656.2012.01955.x"
    ),
    paper!(
        "kranstauber2014",
        ["Kranstauber, B.", "Safi, K.", "Bartumeus, F."],
        false,
        2014,
        "Bivariate Gaussian bridges: directional factorization of diffusion in Brownian bridge models",
        "Movement Ecology",
        "2",
        "1",
        "5",
        "10.1186/2051-3933-2-5"
    ),
    Reference {
        key: "kranstauber2026",
        authors: &["Kranstauber, B.", "Safi, K.", "Scharf, A. K."],
        more: false,
        year: 2026,
        title: "move2utils: a utility toolkit for the move2 ecosystem",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "10.64898/2026.07.07.736908",
        report: "bioRxiv preprint, https://doi.org/10.64898/2026.07.07.736908",
    },
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
        "looney2019",
        ["Looney, D. P.", "Santee, W. R.", "Hansen, E. O.", "Bonventre, P. J.", "Chalmers, C. R.", "Potter, A. W."],
        false,
        2019,
        "Estimating energy expenditure during level, uphill, and downhill walking",
        "Medicine & Science in Sports & Exercise",
        "51",
        "9",
        "1954–1960",
        "10.1249/mss.0000000000002002"
    ),
    paper!(
        "lapoint2013",
        ["LaPoint, S.", "Gallery, P.", "Wikelski, M.", "Kays, R."],
        false,
        2013,
        "Animal behavior, cost-based corridor models, and real corridors",
        "Landscape Ecology",
        "28",
        "8",
        "1615–1630",
        "10.1007/s10980-013-9910-0"
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
        "mcrae2012",
        ["McRae, B. H.", "Hall, S. A.", "Beier, P.", "Theobald, D. M."],
        false,
        2012,
        "Where to restore ecological connectivity? Detecting barriers and quantifying restoration benefits",
        "PLoS ONE",
        "7",
        "12",
        "e52604",
        "10.1371/journal.pone.0052604"
    ),
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
        "morris1991",
        ["Morris, M. D."],
        false,
        1991,
        "Factorial sampling plans for preliminary computational experiments",
        "Technometrics",
        "33",
        "2",
        "161–174",
        "10.1080/00401706.1991.10484804"
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
        "pontzer2016",
        ["Pontzer, H."],
        false,
        2016,
        "A unified theory for the energy cost of legged locomotion",
        "Biology Letters",
        "12",
        "2",
        "20150935",
        "10.1098/rsbl.2015.0935"
    ),
    paper!(
        "rubner2000",
        ["Rubner, Y.", "Tomasi, C.", "Guibas, L. J."],
        false,
        2000,
        "The earth mover's distance as a metric for image retrieval",
        "International Journal of Computer Vision",
        "40",
        "2",
        "99–121",
        "10.1023/A:1026543900054"
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
        "saltelli2002",
        ["Saltelli, A."],
        false,
        2002,
        "Making best use of model evaluations to compute sensitivity indices",
        "Computer Physics Communications",
        "145",
        "2",
        "280–297",
        "10.1016/S0010-4655(02)00280-1"
    ),
    paper!(
        "saltelli2010",
        ["Saltelli, A.", "Annoni, P.", "Azzini, I.", "Campolongo, F.", "Ratto, M.", "Tarantola, S."],
        false,
        2010,
        "Variance based sensitivity analysis of model output. Design and estimator for the total sensitivity index",
        "Computer Physics Communications",
        "181",
        "2",
        "259–270",
        "10.1016/j.cpc.2009.09.018"
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
        "sheather1991",
        ["Sheather, S. J.", "Jones, M. C."],
        false,
        1991,
        "A reliable data-based bandwidth selection method for kernel density estimation",
        "Journal of the Royal Statistical Society: Series B (Methodological)",
        "53",
        "3",
        "683–690",
        "10.1111/j.2517-6161.1991.tb01857.x"
    ),
    paper!(
        "signer2019",
        ["Signer, J.", "Fieberg, J.", "Avgar, T."],
        false,
        2019,
        "Animal movement tools (amt): R package for managing tracking data and conducting habitat selection analyses",
        "Ecology and Evolution",
        "9",
        "2",
        "880–890",
        "10.1002/ece3.4823"
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
        "sobol2001",
        ["Sobol', I. M."],
        false,
        2001,
        "Global sensitivity indices for nonlinear mathematical models and their Monte Carlo estimates",
        "Mathematics and Computers in Simulation",
        "55",
        "1–3",
        "271–280",
        "10.1016/S0378-4754(00)00270-6"
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
    Reference {
        key: "wand1995",
        authors: &["Wand, M. P.", "Jones, M. C."],
        more: false,
        year: 1995,
        title: "Kernel smoothing",
        journal: "",
        volume: "",
        issue: "",
        pages: "",
        doi: "",
        report: "Chapman & Hall/CRC. https://doi.org/10.1201/b14876",
    },
    paper!(
        "worton1989",
        ["Worton, B. J."],
        false,
        1989,
        "Kernel methods for estimating the utilization distribution in home-range studies",
        "Ecology",
        "70",
        "1",
        "164–168",
        "10.2307/1938423"
    ),
    paper!(
        "zeller2018",
        ["Zeller, K. A.", "Jennings, M. K.", "Vickers, T. W.", "Ernest, H. B.", "Cushman, S. A.", "Boyce, W. M."],
        false,
        2018,
        "Are all data types and connectivity models created equal? Validating common connectivity approaches with \
         dispersal data",
        "Diversity and Distributions",
        "24",
        "7",
        "868–879",
        "10.1111/ddi.12742"
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
        ops: &["linkage."],
        name: "least-cost corridors, pinch points and barriers ({ops})",
        refs: &["mcrae2012", "mcrae2008"],
        software: "Linkage Mapper",
        software_refs: &[],
    },
    // before the family's: the first method that matches an operator is its own
    Method {
        ops: &["cost.energy_landscape"],
        name: "the energy cost of walking over a terrain (animals: Pontzer's allometric cost of locomotion; people: \
               Looney et al.'s walking equation)",
        refs: &["pontzer2016", "looney2019"],
        software: "enerscape",
        software_refs: &["berti2022"],
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
    Method {
        ops: &["ml.pca"],
        name: "principal component analysis",
        refs: &["pearson1901", "hotelling1933", "byrne1980", "fung1987", "chavez1989", "green1988", "lee1990"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["ml.kmeans"],
        name: "k-means clustering",
        refs: &["lloyd1982", "arthur2007", "rousseeuw1987"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["ml.som"],
        name: "self-organizing map",
        refs: &["kohonen1982", "kohonen2001", "vesanto2000"],
        software: "",
        software_refs: &[],
    },
    // ml.fit fits a random forest unless its method is gradient_boosting (see `method_for`)
    Method {
        ops: &["ml.fit"],
        name: "random forest learning",
        refs: &["breiman2001"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["objects.classify", "ml.predict", "sdm.mess", "sdm.mop", "sdm.exdet"],
        name: "the novelty of new records against what a model was trained on (MESS, MOP and ExDet)",
        refs: &["elith2010", "owens2013", "mesgaran2014"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["raster.match_histogram"],
        name: "the matching of a scene's histograms to another's (scikit-image's match_histograms)",
        refs: &["vanderwalt2014"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["segment.multiresolution"],
        name: "multiresolution segmentation into objects",
        refs: &["baatz2000"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["segment.slic"],
        name: "SLIC superpixels",
        refs: &["achanta2012"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["segment.snic"],
        name: "SNIC superpixels",
        refs: &["achanta2017"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["segment.felzenszwalb"],
        name: "graph-based segmentation into objects",
        refs: &["felzenszwalb2004"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["segment.threshold"],
        name: "multi-threshold segmentation into objects",
        refs: &["otsu1979", "liao2001"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["segment.spectral_difference"],
        name: "spectral difference segmentation (merging neighbouring objects of close means)",
        refs: &["trimble2022"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["segment.region_growing"],
        name: "region growing and merging segmentation into objects (GRASS GIS's i.segment)",
        refs: &["momsen2012"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["segment.quickshift"],
        name: "quick shift segmentation into objects",
        refs: &["vedaldi2008"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["segment.watershed"],
        name: "watershed segmentation into objects",
        refs: &["vincent1991", "neubert2014"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["objects.split"],
        name: "splitting of touching objects by the watershed of the distance to their border",
        refs: &["vincent1991", "carpenter2006"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["objects.resize"],
        name: "pixel-based resizing, morphological opening and closing, filling and border optimization of objects",
        refs: &["serra1982", "soille2004", "felzenszwalb2012"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["segment.meanshift"],
        name: "mean shift segmentation into objects",
        refs: &["comaniciu2002"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["segment.scale"],
        name: "the estimation of the scale parameter of multiresolution segmentation (ESP)",
        refs: &["dragut2010", "dragut2014"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["objects.series", "raster.phenology"],
        name: "the seasons of a time series (the amplitude method of TIMESAT, Savitzky-Golay smoothing, harmonic regression)",
        refs: &["jonsson2004", "savitzky1964", "jakubauskas2001", "zhu2014"],
        software: "TIMESAT",
        software_refs: &[],
    },
    Method {
        ops: &["segment.evaluate"],
        name: "the accuracy of a segmentation against reference objects (over- and under-segmentation, D, quality rate, area fit index, precision and recall, discrepancy ED2)",
        refs: &["clinton2010", "lucieer2002", "weidner2008", "liu2012", "zhang2015", "costa2018"],
        software: "segmetric",
        software_refs: &[],
    },
    Method {
        ops: &["segment.optimize"],
        name: "unsupervised optimisation of a segmentation's parameter (intra-object variance and Moran's I), or by a reference (D, ED2 or the F of the overlap)",
        refs: &["espindola2006", "johnson2011", "clinton2010", "liu2012", "zhang2015"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["objects.rules"],
        name: "classification of objects by a hierarchy of fuzzy rules",
        refs: &["zadeh1965", "benz2004"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["objects.folds"],
        name: "spatial cross-validation of objects (folds of neighbouring objects, of blocks or of clusters of the centroids)",
        refs: &["ploton2020", "meyer2022", "roberts2017"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["objects.nearest"],
        name: "classification of objects by their nearest samples (as eCognition's Standard Nearest Neighbour)",
        refs: &[],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["objects.resolve"],
        name: "the arbitration of the hypotheses of each class (as GeoAIDA and InterIMAGE)",
        refs: &["buckner2001", "costa2010"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["objects.temporal", "objects.transition_matrix"],
        name: "multitemporal classification bound by a fuzzy transition matrix (as InterIMAGE)",
        refs: &["feitosa2009", "costa2010"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["segment.sam"],
        name: "segmentation by the Segment Anything Model",
        refs: &["kirillov2023"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["objects.accuracy", "objects.sample"],
        name: "the accuracy of a classification of objects and the sample to assess it",
        refs: &["olofsson2014", "radoux2017", "stehman2014"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["move.max_speed"],
        name: "the allometric maximum speed of an animal",
        refs: &["hirt2017"],
        software: "move2utils",
        software_refs: &["kranstauber2026"],
    },
    Method {
        ops: &["move.flag_speed"],
        name: "the speed cap on the steps of tracks",
        refs: &[],
        software: "move2utils",
        software_refs: &["kranstauber2026"],
    },
    Method {
        ops: &["move.filter_gps_quality", "move.thin_time", "move.thin_distance"],
        name: "the cleaning and thinning of tracks ({ops})",
        refs: &[],
        software: "move2utils",
        software_refs: &["kranstauber2026"],
    },
    Method {
        ops: &["move.corridor"],
        name: "the detection of corridor segments in animal tracks",
        refs: &["lapoint2013"],
        software: "move2utils",
        software_refs: &["kranstauber2026"],
    },
    Method {
        ops: &["move.ud_distance"],
        name: "the earth mover's distance between utilisation distributions",
        refs: &["rubner2000", "cuturi2013"],
        software: "move2utils",
        software_refs: &["kranstauber2026"],
    },
    Method {
        ops: &["move.validate"],
        name: "the validation of a connectivity surface with the use of the animals (rank correlation, share of the \
               use inside the corridors, use against area)",
        refs: &["zeller2018"],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["move.dbbmm_variance", "move.dbbmm_ud"],
        name: "the utilization distribution of the dynamic Brownian bridge movement model ({ops})",
        refs: &["kranstauber2012", "horne2007"],
        software: "move2utils",
        software_refs: &["kranstauber2026"],
    },
    Method {
        ops: &["move.dbgb_variance", "move.dbgb_ud"],
        name: "the utilization distribution of the dynamic bivariate Gaussian bridge ({ops})",
        refs: &["kranstauber2014", "kranstauber2012"],
        software: "move2utils",
        software_refs: &["kranstauber2026"],
    },
    Method {
        ops: &["move.suggest_window", "move.mask_segments"],
        name: "the window of the motion variance of a Brownian bridge and the masking of its segments ({ops})",
        refs: &["kranstauber2012"],
        software: "move2utils",
        software_refs: &["kranstauber2026"],
    },
    Method {
        ops: &["move.ud_volume", "move.ud_outer_probability"],
        name: "the volume of a utilization distribution and the outer probability of locations ({ops})",
        refs: &[],
        software: "move2utils",
        software_refs: &["kranstauber2026"],
    },
    Method {
        ops: &["move.home_range"],
        name: "the home range of animal tracks (minimum convex polygons, kernel density estimates and local convex hulls)",
        refs: &["worton1989", "getz2007", "sheather1991", "wand1995"],
        software: "amt",
        software_refs: &["signer2019"],
    },
    Method {
        ops: &["move.home_range_overlap"],
        name: "the overlap between home ranges",
        refs: &["fieberg2005"],
        software: "amt",
        software_refs: &["signer2019"],
    },
    Method {
        ops: &["move.intensity_use"],
        name: "the intensity of use of a track",
        refs: &[],
        software: "amt",
        software_refs: &["signer2019"],
    },
    Method {
        ops: &["move.fit_issf", "move.log_rss"],
        name: "a step selection function (conditional logistic regression with the exact partial likelihood, and the \
               log relative selection strength of covariates)",
        refs: &["fortin2005", "avgar2016", "gail1981", "avgar2017"],
        software: "amt and survival",
        software_refs: &["signer2019"],
    },
    Method {
        ops: &["move.surface_distance"],
        name: "the length of animal steps measured on the relief (the sum of the 3D chords of each step over a DEM, \
               with bilinear heights along it)",
        refs: &[],
        software: "",
        software_refs: &[],
    },
    Method {
        ops: &["sensitivity.design", "sensitivity.analyze"],
        name: "a global sensitivity analysis ({ops})",
        refs: &["morris1991", "campolongo2007", "sobol2001", "saltelli2002", "saltelli2010"],
        software: "SALib",
        software_refs: &["herman2017", "iwanaga2022"],
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

pub(crate) fn reference(key: &str) -> Option<&'static Reference> {
    REFERENCES.iter().find(|r| r.key == key)
}

/// What `ml.fit` fits when its method is gradient_boosting.
static BOOSTING: Method =
    Method { ops: &[], name: "gradient boosting of trees", refs: &["friedman2001"], software: "", software_refs: &[] };

/// What `ml.fit` fits when its method is a model input ($name), unknown until the run: no paper is cited.
static TREES: Method = Method {
    ops: &[],
    name: "tree ensemble learning (random forest or gradient boosting)",
    refs: &[],
    software: "",
    software_refs: &[],
};

/// The method a node is part of: [`method_of`], but `ml.fit` by its `method` setting (the forest by default).
fn method_for(n: &Node) -> Option<&'static Method> {
    if n.op == "ml.fit" {
        match n.params.get("method").and_then(|v| v.as_str()) {
            Some("gradient_boosting") => return Some(&BOOSTING),
            Some(s) if s.starts_with('$') || s.starts_with('@') => return Some(&TREES),
            _ => {}
        }
    }
    method_of(&n.op)
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
    let ops: Vec<(&str, Option<&'static Method>)> =
        nodes.iter().filter(|(_, n)| !n.bypass).map(|(_, n)| (n.op.as_str(), method_for(n))).collect();
    // the methods used, grouped by the software reproduced, in the order they come
    let mut groups: Vec<Group> = Vec::new();
    for &(op, me) in &ops {
        let Some(me) = me else { continue };
        let g = match groups.iter().position(|(s, sr, _)| *s == me.software && *sr == me.software_refs) {
            Some(g) => g,
            None => {
                groups.push((me.software, me.software_refs, Vec::new()));
                groups.len() - 1
            }
        };
        let used = &mut groups[g].2;
        match used.iter_mut().find(|(x, _)| std::ptr::eq(*x, me)) {
            Some((_, o)) if !o.contains(&op) => o.push(op),
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
    let file = file.map(|f| format!(" ({f})")).unwrap_or_default();
    let mut paragraph = format!(
        "The analysis was run in Terraflux {version} (Lacerda, 2026), as a model of {steps} step{}{file}; the model's file, with every setting{}, is given with this paper.",
        if steps == 1 { "" } else { "s" },
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
        let mut s = format!("We computed {} with Terraflux", and(&names));
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
        let h = self.headings();
        let mut out = format!("{}\n\n{}\n\n{}\n\n", h[0], self.paragraph, h[1]);
        let mut k = 0;
        for s in &self.steps {
            if s.starts_with(' ') {
                out.push_str(&format!("{s}\n"));
            } else {
                k += 1;
                out.push_str(&format!("{k}. {s}\n"));
            }
        }
        out.push_str(&format!("\n{}\n\n", h[2]));
        let mut refs: Vec<String> = self.references.iter().map(|r| r.apa()).collect();
        refs.push(self.software.clone());
        refs.sort_by_key(|r| r.to_lowercase());
        for r in refs {
            out.push_str(&format!("{r}\n"));
        }
        if !self.own.is_empty() {
            out.push_str(&format!("\n{}\n\n", h[3]));
            for r in &self.own {
                out.push_str(&format!("{r}\n"));
            }
        }
        out
    }

    /// The headings: the methods, the steps, the references, those of the model's notes.
    pub fn headings(&self) -> [&'static str; 4] {
        ["Methods", "The model, step by step", "References", "Given in the model's notes"]
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
    let h = m.headings();
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
            ui.horizontal(|ui| {
                ui.hyperlink_to(RichText::new("What to cite, on the website").size(11.5), CITING);
            });
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
                crate::theme::section(ui, h[0]);
                ui.add(egui::Label::new(&m.paragraph).selectable(true).wrap());
                ui.add_space(10.0);
                crate::theme::section(ui, h[1]);
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
                crate::theme::section(ui, h[2]);
                let mut refs: Vec<String> = m.references.iter().map(|r| r.apa()).collect();
                refs.push(m.software.clone());
                refs.sort_by_key(|r| r.to_lowercase());
                for x in &refs {
                    ui.add(egui::Label::new(RichText::new(x).size(12.5)).selectable(true).wrap());
                    ui.add_space(2.0);
                }
                if !m.own.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("{} (refs)", h[3])).size(11.5).color(weak));
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
        assert_eq!(method_of("segment.watershed").unwrap().refs, &["vincent1991", "neubert2014"]);
        assert_eq!(method_of("objects.split").unwrap().refs, &["vincent1991", "carpenter2006"]);
        assert_eq!(method_of("objects.resize").unwrap().refs, &["serra1982", "soille2004", "felzenszwalb2012"]);
        assert_eq!(method_of("segment.scale").unwrap().refs, &["dragut2010", "dragut2014"]);
        assert_eq!(
            method_of("segment.optimize").unwrap().refs,
            &["espindola2006", "johnson2011", "clinton2010", "liu2012", "zhang2015"]
        );
        assert_eq!(
            method_of("ml.pca").unwrap().refs,
            &["pearson1901", "hotelling1933", "byrne1980", "fung1987", "chavez1989", "green1988", "lee1990"]
        );
        assert_eq!(method_of("ml.kmeans").unwrap().refs, &["lloyd1982", "arthur2007", "rousseeuw1987"]);
        assert_eq!(method_of("ml.som").unwrap().refs, &["kohonen1982", "kohonen2001", "vesanto2000"]);
        assert_eq!(method_of("ml.fit").unwrap().refs, &["breiman2001"]);
        assert!(method_of("raster.reclassify").is_none());
    }

    #[test]
    fn ml_fit_cites_the_algorithm_it_fits() {
        let r = Registry::standard();
        let paragraph = |params: &str| {
            let m = Model::from_toml(&format!(
                "format = \"terraflux/1\"

[[nodes]]
id = \"fit\"
op = \"ml.fit\"
params = {{ {params} }}
"
            ))
            .unwrap();
            of(&m, &r, "1.0.0", None).paragraph
        };
        let forest = paragraph("");
        assert!(forest.contains("random forest learning (Breiman, 2001)"), "{forest}");
        assert_eq!(paragraph("method = \"random_forest\""), forest);
        let boosting = paragraph("method = \"gradient_boosting\"");
        assert!(
            boosting.contains("gradient boosting of trees (Friedman, 2001)") && !boosting.contains("Breiman"),
            "{boosting}"
        );
        let open = paragraph("method = \"$m\"");
        assert!(
            open.contains("tree ensemble learning") && !open.contains("Breiman") && !open.contains("Friedman"),
            "{open}"
        );
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
