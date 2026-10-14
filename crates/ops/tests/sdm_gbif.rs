//! The download of GBIF's records with the API replaced by a fake one (no network), as landschaft's tests: the
//! pages and the query (the accepted name of a synonym, the country, the years), the issues, the limit and the
//! uncertainty, an unknown species.

use std::sync::Mutex;

use serde_json::{Value as Json, json};
use terraflux_ops::sdm::gbif::{Query, Species, download};
use terraflux_raster::vector::Column;

type Calls = Mutex<Vec<(String, Vec<(String, String)>)>>;

fn fake_api(n: usize, calls: &Calls) -> impl Fn(&str, &[(String, String)]) -> Result<Json, String> + Sync + '_ {
    move |path: &str, q: &[(String, String)]| {
        calls.lock().unwrap().push((path.to_string(), q.to_vec()));
        let get = |k: &str| q.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap();
        if path == "species/match" {
            if get("name") == "Nonexistent" {
                return Ok(json!({"matchType": "NONE"}));
            }
            return Ok(json!({"usageKey": 10, "acceptedUsageKey": 7, "matchType": "EXACT", "status": "SYNONYM"}));
        }
        let (offset, limit): (usize, usize) = (get("offset").parse().unwrap(), get("limit").parse().unwrap());
        let results: Vec<Json> = (offset..(offset + limit).min(n))
            .map(|k| {
                json!({"key": k, "species": "A b", "decimalLongitude": -50.0 + k as f64 / 1000.0,
                       "decimalLatitude": -10.0,
                       "coordinateUncertaintyInMeters": if k % 2 == 1 { json!(10 * k) } else { Json::Null },
                       "issues": if k == 0 { json!(["A", "B"]) } else { json!([]) }})
            })
            .collect();
        Ok(json!({"count": n, "results": results}))
    }
}

fn column<'a>(v: &'a terraflux_raster::vector::Vector, name: &str) -> &'a Column {
    &v.attributes.iter().find(|(n, _)| n == name).unwrap().1
}

#[test]
fn pages_synonym_and_filters() {
    let calls = Calls::default();
    let q = Query { country: vec!["BR".into()], year: Some("1990,2020".into()), workers: 3, ..Default::default() };
    let (v, info) = download(&Species::Name("A b".into()), &q, &fake_api(1000, &calls)).unwrap();
    assert_eq!(v.len(), 1000);
    let Column::Numbers(ids) = column(&v, "gbifID") else { panic!() };
    assert_eq!(ids, &(0..1000).map(|k| k as f64).collect::<Vec<_>>());
    let Column::Texts(issue) = column(&v, "issue") else { panic!() };
    assert_eq!((issue[0].as_str(), issue[1].as_str()), ("A;B", ""));
    let calls = calls.lock().unwrap();
    let searches: Vec<&Vec<(String, String)>> =
        calls.iter().filter(|(p, _)| p == "occurrence/search").map(|(_, q)| q).collect();
    let value = |q: &[(String, String)], k: &str| q.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap();
    let mut offsets: Vec<usize> = searches.iter().map(|q| value(q, "offset").parse().unwrap()).collect();
    offsets.sort_unstable();
    assert_eq!(offsets, [0, 300, 600, 900]);
    assert!(
        searches
            .iter()
            .all(|q| value(q, "taxonKey") == "7" && value(q, "country") == "BR" && value(q, "year") == "1990,2020")
    );
    assert_eq!(info["taxonKey"], 7);
    assert_eq!(info["truncated"], false);
    assert_eq!(v.crs.as_deref(), Some("EPSG:4326"));
}

#[test]
fn limit_and_uncertainty() {
    let calls = Calls::default();
    let q = Query { limit: Some(400), max_uncertainty: Some(1000.0), workers: 4, ..Default::default() };
    let (v, info) = download(&Species::Key(7), &q, &fake_api(5000, &calls)).unwrap();
    // 400 records, the odd ones above 1000 m dropped (k >= 101), the even ones have none
    assert_eq!(v.len(), 200 + 50);
    assert_eq!(info["truncated"], true);
}

#[test]
fn unknown_species() {
    let calls = Calls::default();
    let e = download(&Species::Name("Nonexistent".into()), &Query::default(), &fake_api(0, &calls)).unwrap_err();
    assert!(e.contains("no taxon"), "{e}");
}

/// GBIF itself (network): a few records of the lowland tapir in Brazil, through the node.
#[test]
#[ignore]
fn downloads_from_gbif() {
    let r = terraflux_ops::Registry::standard();
    let params = json!({"species": "Tapirus terrestris", "country": "BR", "limit": 650});
    let args = terraflux_ops::Args { node: "n", inputs: Default::default(), params: params.as_object().unwrap() };
    let out = r.get("sdm.download_gbif").unwrap().run(&mut terraflux_ops::Ctx::default(), &args).unwrap();
    let terraflux_ops::Value::Vector(v) = &out["occurrences"] else { panic!() };
    assert!(v.len() > 300 && v.len() <= 650, "{}", v.len());
    let Column::Texts(cc) = column(v, "countryCode") else { panic!() };
    assert!(cc.iter().all(|c| c == "BR"));
}
