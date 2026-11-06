//! *Go to...*: a place typed as text and found on the map. Degrees, as decimals or with minutes and seconds
//! (`-12.34, 122.5`, `12.5S 122.5E`, `12°20'30"S 122°30'E`), or the map's own coordinates (`x y`). The text is
//! parsed by a plain function with errors that say what is wrong; the view centres on the place and keeps a mark
//! there.
//!
//! How the two numbers of degrees are told apart: a hemisphere letter (N, S, E, W) says which is which, in any
//! order; without letters a comma between them means latitude first (as Google Maps copies it), and with none the
//! one beyond 90 degrees must be the longitude, else longitude first (`x y`, as GIS tools write it).

use terraflux_raster::Grid;

use crate::measure::Frame;

/// What the text of the box asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    /// A longitude and a latitude, in degrees.
    Degrees { lon: f64, lat: f64 },
    /// A point in the map's own coordinates.
    Map { x: f64, y: f64 },
}

/// What the hint of the box says.
pub const HINT: &str = "lat, lon (a comma, as Google Maps copies it)  ·  lon lat (a space)  ·  12°20'30\"S 122°30'E  ·  \
                        N S E W tell which is which  ·  add @ 1:50000 or @ 5 km for the zoom";

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Deg,
    Min,
    Sec,
    Hemi(char),
    Comma,
}

fn tokens(text: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() || c == ';' => i += 1,
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            '°' | 'º' | '˚' => {
                out.push(Tok::Deg);
                i += 1;
            }
            '\'' | '′' | '’' | '‘' => {
                out.push(Tok::Min);
                i += 1;
            }
            '"' | '″' | '”' | '“' => {
                out.push(Tok::Sec);
                i += 1;
            }
            c if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == '−' => {
                let start = i;
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let t: String = chars[start..i].iter().map(|&c| if c == '−' { '-' } else { c }).collect();
                let v = t.parse::<f64>().map_err(|_| format!("\"{t}\" is not a number"))?;
                out.push(Tok::Num(v));
            }
            c if matches!(c.to_ascii_uppercase(), 'N' | 'S' | 'E' | 'W') => {
                out.push(Tok::Hemi(c.to_ascii_uppercase()));
                i += 1;
            }
            c => return Err(format!("unexpected \"{c}\": numbers, ° ' \" and N S E W only")),
        }
    }
    // (two single quotes written for seconds)
    let mut merged = Vec::new();
    for t in out {
        if t == Tok::Min && merged.last() == Some(&Tok::Min) {
            merged.pop();
            merged.push(Tok::Sec);
        } else {
            merged.push(t);
        }
    }
    Ok(merged)
}

/// Which axis a coordinate of degrees is on, as its hemisphere letter says.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Axis {
    Lat,
    Lon,
}

/// One coordinate: its signed degrees, and the axis its hemisphere gave it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Coord {
    value: f64,
    axis: Option<Axis>,
}

/// A coordinate from the tokens at `at`: `[hemisphere] degrees[°] [minutes'] [seconds"] [hemisphere]`.
fn coord(t: &[Tok], at: &mut usize) -> Result<Coord, String> {
    let hemi = |c: char| match c {
        'N' => (Axis::Lat, 1.0),
        'S' => (Axis::Lat, -1.0),
        'E' => (Axis::Lon, 1.0),
        _ => (Axis::Lon, -1.0),
    };
    let mut first = None;
    if let Some(Tok::Hemi(c)) = t.get(*at) {
        first = Some(hemi(*c));
        *at += 1;
    }
    let Some(Tok::Num(d)) = t.get(*at) else {
        return Err("a coordinate is a number of degrees (12.5, 12°30', 12°30'15\")".into());
    };
    let mut value = d.abs();
    let negative = d.is_sign_negative() && *d != 0.0;
    *at += 1;
    // the parts after the degrees, each with its mark
    let marked = matches!(t.get(*at), Some(Tok::Deg));
    if marked {
        *at += 1;
        let mut scale = 60.0;
        for (mark, name) in [(Tok::Min, "minutes"), (Tok::Sec, "seconds")] {
            if let Some(Tok::Num(m)) = t.get(*at)
                && t.get(*at + 1) == Some(&mark)
            {
                if *m < 0.0 || *m >= 60.0 {
                    return Err(format!("{name} go from 0 to 60, not {m}"));
                }
                value += m / scale;
                scale *= 60.0;
                *at += 2;
            } else if mark == Tok::Min && matches!(t.get(*at), Some(Tok::Num(_))) && t.get(*at + 1) != Some(&Tok::Deg) {
                return Err("minutes need their ' mark (12°20'30\")".into());
            } else {
                break;
            }
        }
    }
    let mut last = None;
    if let Some(Tok::Hemi(c)) = t.get(*at) {
        // (a hemisphere after the number: but a following coordinate may start with its own prefix letter, so only
        // one letter is taken, and not when the first had one)
        if first.is_none() {
            last = Some(hemi(*c));
            *at += 1;
        }
    }
    let letter = first.or(last);
    if letter.is_some() && negative {
        return Err("a sign and a hemisphere letter together: use one of them".into());
    }
    let (axis, sign) = match letter {
        Some((a, s)) => (Some(a), s),
        None => (None, if negative { -1.0 } else { 1.0 }),
    };
    Ok(Coord { value: sign * value, axis })
}

fn degrees(a: Coord, b: Coord, comma: bool) -> Result<Target, String> {
    let (lat, lon) = match (a.axis, b.axis) {
        (Some(Axis::Lat), Some(Axis::Lat)) => return Err("both coordinates are latitudes (N or S)".into()),
        (Some(Axis::Lon), Some(Axis::Lon)) => return Err("both coordinates are longitudes (E or W)".into()),
        (Some(Axis::Lat), _) | (_, Some(Axis::Lon)) => (a.value, b.value),
        (Some(Axis::Lon), _) | (_, Some(Axis::Lat)) => (b.value, a.value),
        (None, None) if comma => (a.value, b.value),
        (None, None) => {
            let (big_a, big_b) = (a.value.abs() > 90.0, b.value.abs() > 90.0);
            match (big_a, big_b) {
                (true, true) => {
                    return Err(format!("{} and {} are both beyond 90: a latitude is within 90", a.value, b.value));
                }
                (true, false) => (b.value, a.value),
                (false, true) => (a.value, b.value),
                // longitude first, as x y
                (false, false) => (b.value, a.value),
            }
        }
    };
    if lat.abs() > 90.0 {
        let hint = if comma && a.axis.is_none() { " (with a comma, latitude comes first)" } else { "" };
        return Err(format!("latitude {} is beyond 90 degrees{hint}", plain(lat)));
    }
    if lon.abs() > 180.0 {
        return Err(format!("longitude {} is beyond 180 degrees", plain(lon)));
    }
    Ok(Target::Degrees { lon, lat })
}

/// A number as short text.
fn plain(v: f64) -> String {
    let t = format!("{v:.6}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The place a text asks for: degrees (`degrees`) or two numbers in the map's units.
pub fn parse(text: &str, degrees_mode: bool) -> Result<Target, String> {
    let t = tokens(text.trim())?;
    if t.is_empty() {
        return Err("type a coordinate".into());
    }
    if !degrees_mode {
        let nums: Vec<f64> = t
            .iter()
            .filter(|k| !matches!(k, Tok::Comma))
            .map(|k| match k {
                Tok::Num(v) => Ok(*v),
                _ => Err("map units are two plain numbers: x y".to_string()),
            })
            .collect::<Result<_, _>>()?;
        return match nums[..] {
            [x, y] => Ok(Target::Map { x, y }),
            _ => Err(format!("two numbers (x y) are needed, not {}", nums.len())),
        };
    }
    let mut at = 0;
    let a = coord(&t, &mut at)?;
    let comma = t.get(at) == Some(&Tok::Comma);
    if comma {
        at += 1;
    }
    if at >= t.len() {
        return Err("a second coordinate is missing (lat, lon)".into());
    }
    let b = coord(&t, &mut at)?;
    if at < t.len() {
        return Err("more than two coordinates".into());
    }
    degrees(a, b, comma)
}

/// How far in the view is after going: the zoom as it is, a map scale, or the ground the view's width shows.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Zoom {
    #[default]
    Keep,
    /// 1:n, a screen point taken as 1/96 inch (as CSS and the scale bars of GIS tools do).
    Scale(f64),
    /// Metres across the width of the view.
    Width(f64),
}

/// Metres in a screen point (egui's points are 1/96 inch).
const POINT_METRES: f64 = 0.0254 / 96.0;

/// The scales the box offers.
pub const SCALES: [f64; 4] = [5_000.0, 25_000.0, 100_000.0, 1_000_000.0];

impl Zoom {
    /// What the box shows for it.
    pub fn label(self) -> String {
        match self {
            Zoom::Keep => "keep the zoom".into(),
            Zoom::Scale(n) => format!("1:{}", spaced(n)),
            Zoom::Width(m) if m >= 1000.0 => format!("{} km wide", plain(m / 1000.0)),
            Zoom::Width(m) => format!("{} m wide", plain(m)),
        }
    }

    /// Metres on the ground per screen point this asks for, in a view `width` points wide (none: keep the zoom).
    pub fn metres_per_point(self, width: f32) -> Option<f64> {
        match self {
            Zoom::Keep => None,
            Zoom::Scale(n) => Some(n * POINT_METRES),
            Zoom::Width(m) => Some(m / f64::from(width.max(1.0))),
        }
    }
}

/// 25000 as `25 000`.
fn spaced(n: f64) -> String {
    let t = format!("{n:.0}");
    let mut out = String::new();
    for (i, c) in t.chars().enumerate() {
        if i > 0 && (t.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// A zoom as typed: `1:50000` (spaces allowed in the number), `keep`, or a width of view: `5 km`, `800 m`.
pub fn parse_zoom(text: &str) -> Result<Zoom, String> {
    let t = text.trim().to_lowercase();
    if t.is_empty() || t == "keep" {
        return Ok(Zoom::Keep);
    }
    let number = |s: &str| {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        match s.parse::<f64>() {
            Ok(v) if v.is_finite() && v > 0.0 => Ok(v),
            _ => Err(format!("\"{s}\" is not a size above zero")),
        }
    };
    if let Some(n) = t.strip_prefix("1:").or_else(|| t.strip_prefix("1/")) {
        return Ok(Zoom::Scale(number(n)?));
    }
    if let Some(n) = t.strip_suffix("km") {
        return Ok(Zoom::Width(number(n)? * 1000.0));
    }
    if let Some(n) = t.strip_suffix('m') {
        return Ok(Zoom::Width(number(n)?));
    }
    Err("a zoom is a scale (1:50000) or a width of view (5 km, 800 m)".into())
}

/// A place and, after an `@`, the zoom to go there at: `-15.8, -47.9 @ 1:50000`, `450000 1300000 @ 2 km`.
pub fn parse_with_zoom(text: &str, degrees_mode: bool) -> Result<(Target, Zoom), String> {
    match text.split_once('@') {
        None => Ok((parse(text, degrees_mode)?, Zoom::Keep)),
        Some((place, zoom)) => Ok((parse(place, degrees_mode)?, parse_zoom(zoom)?)),
    }
}

/// A place as text in the map's own system: degrees with hemispheres on a geographic map, else x and y.
pub fn place_text(g: &Grid, c: [f64; 2]) -> String {
    let t = &g.transform;
    let (x, y) = (t[0] + c[0] * t[1] + c[1] * t[2], t[3] + c[0] * t[4] + c[1] * t[5]);
    match Frame::of(g) {
        Frame::Geographic => degrees_text(x, y),
        Frame::Projected(_) => format!("x {x:.2}  y {y:.2}"),
        Frame::Plain => format!("x {} y {}", plain(x), plain(y)),
    }
}

/// Longitude and latitude as `12.50000°S 122.50000°E`.
pub fn degrees_text(lon: f64, lat: f64) -> String {
    format!(
        "{:.5}°{} {:.5}°{}",
        lat.abs(),
        if lat < 0.0 { 'S' } else { 'N' },
        lon.abs(),
        if lon < 0.0 { 'W' } else { 'E' }
    )
}

/// The cell (column, row, fractional) of a place on a grid, and whether it is inside the map. Degrees go to the
/// map's system through GDAL (EPSG:4326 to the map's, x first) when it is projected, and are taken as they are on a
/// geographic one; a map with no coordinate system takes only `Map` places.
pub fn locate(g: &Grid, target: Target) -> Result<([f64; 2], bool), String> {
    let (x, y) = match target {
        Target::Map { x, y } => (x, y),
        Target::Degrees { lon, lat } => match Frame::of(g) {
            Frame::Geographic => (lon, lat),
            Frame::Plain => return Err("the map has no coordinate system: type map units".into()),
            Frame::Projected(_) => {
                let crs = crate::map_view::crs_of(g).ok_or("the map has no coordinate system: type map units")?;
                let tr = terraflux_gdal::Transform::new("EPSG:4326", &crs)?;
                let (mut xs, mut ys) = ([lon], [lat]);
                tr.points(&mut xs, &mut ys);
                if !xs[0].is_finite() || !ys[0].is_finite() {
                    return Err("this place has no coordinates in the map's system".into());
                }
                (xs[0], ys[0])
            }
        },
    };
    let t = &g.transform;
    let det = t[1] * t[5] - t[2] * t[4];
    if det == 0.0 || !det.is_finite() {
        return Err("the map has no cells' size".into());
    }
    let (dx, dy) = (x - t[0], y - t[3]);
    let cell = [(t[5] * dx - t[2] * dy) / det, (t[1] * dy - t[4] * dx) / det];
    let inside = cell[0] >= 0.0 && cell[1] >= 0.0 && cell[0] < g.width as f64 && cell[1] < g.height as f64;
    Ok((cell, inside))
}

/// Views side by side share the place found: the one that just went somewhere (`mark_new`) keeps its mark and the
/// others lose theirs; escape over any (`mark_stopped`) lets it go everywhere; the views are told one is marked.
pub fn together(views: &mut [&mut crate::map_view::MapView]) {
    let stopped = views.iter().any(|v| v.mark_stopped);
    let newest = views.iter().rposition(|v| v.mark_new);
    for (i, v) in views.iter_mut().enumerate() {
        v.mark_stopped = false;
        v.mark_new = false;
        if stopped || newest.is_some_and(|n| n != i) {
            v.mark = None;
        }
    }
    let any = views.iter().any(|v| v.mark.is_some());
    for v in views.iter_mut() {
        v.mark_beside = any;
    }
}

/// The box of *Go to*: what is typed, in which units, and what came of the last try.
#[derive(Debug, Clone)]
pub struct GoTo {
    pub text: String,
    /// Degrees (true) or the map's own units.
    pub degrees: bool,
    pub error: Option<String>,
    /// A word that is not an error ("outside the map": it went there all the same).
    pub note: Option<String>,
    pub focus: bool,
    /// The zoom chosen in the box (an `@` in the text overrides it).
    pub zoom: Zoom,
}

impl GoTo {
    /// A box open on degrees when the map has a coordinate system to carry them, else on map units.
    pub fn new(g: Option<&Grid>) -> GoTo {
        let degrees = g.is_none_or(|g| Frame::of(g) != Frame::Plain);
        GoTo { text: String::new(), degrees, error: None, note: None, focus: true, zoom: Zoom::Keep }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deg(lon: f64, lat: f64) -> Result<Target, String> {
        Ok(Target::Degrees { lon, lat })
    }

    fn close(t: Result<Target, String>, lon: f64, lat: f64) {
        match t {
            Ok(Target::Degrees { lon: a, lat: b }) => {
                assert!((a - lon).abs() < 1e-9 && (b - lat).abs() < 1e-9, "got {a} {b}, wanted {lon} {lat}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn decimal_degrees_by_comma_or_space() {
        // a comma: latitude first (Google Maps)
        assert_eq!(parse("12.5, 122.25", true), deg(122.25, 12.5));
        assert_eq!(parse("-12.5,122.25", true), deg(122.25, -12.5));
        assert_eq!(parse("  12.5 ,  -122.25 ", true), deg(-122.25, 12.5));
        // a space: the one beyond 90 is the longitude, whichever comes first
        assert_eq!(parse("122.25 12.5", true), deg(122.25, 12.5));
        assert_eq!(parse("12.5 122.25", true), deg(122.25, 12.5));
        assert_eq!(parse("-122.25 -12.5", true), deg(-122.25, -12.5));
        // both within 90 and no comma: longitude first (x y)
        assert_eq!(parse("30 40", true), deg(30.0, 40.0));
        assert_eq!(parse("40.5 -20.5", true), deg(40.5, -20.5));
        // (the minus sign of the unicode)
        assert_eq!(parse("\u{2212}12.5, 122.25", true), deg(122.25, -12.5));
    }

    #[test]
    fn a_zoom_after_an_at_sign() {
        assert_eq!(parse_zoom("1:50000"), Ok(Zoom::Scale(50_000.0)));
        assert_eq!(parse_zoom(" 1:25 000 "), Ok(Zoom::Scale(25_000.0)));
        assert_eq!(parse_zoom("5 km"), Ok(Zoom::Width(5000.0)));
        assert_eq!(parse_zoom("800m"), Ok(Zoom::Width(800.0)));
        assert_eq!(parse_zoom("keep"), Ok(Zoom::Keep));
        assert!(parse_zoom("50000").unwrap_err().contains("scale"));
        assert!(parse_zoom("1:0").is_err() && parse_zoom("-2 km").is_err());
        assert_eq!(
            parse_with_zoom("-15.8, -47.9 @ 1:50000", true),
            Ok((Target::Degrees { lon: -47.9, lat: -15.8 }, Zoom::Scale(50_000.0)))
        );
        assert_eq!(
            parse_with_zoom("450000 1300000 @ 2 km", false),
            Ok((Target::Map { x: 450_000.0, y: 1_300_000.0 }, Zoom::Width(2000.0)))
        );
        assert_eq!(parse_with_zoom("12.5, 122.25", true), Ok((Target::Degrees { lon: 122.25, lat: 12.5 }, Zoom::Keep)));
        // (the place's own errors stay)
        assert!(parse_with_zoom("@ 1:5000", true).is_err());
        // metres per screen point: a point is 1/96 inch
        let m = Zoom::Scale(1000.0).metres_per_point(800.0).unwrap();
        assert!((m - 0.264583).abs() < 1e-5, "{m}");
        assert_eq!(Zoom::Width(800.0).metres_per_point(400.0), Some(2.0));
        assert_eq!(Zoom::Keep.metres_per_point(400.0), None);
        assert_eq!(Zoom::Scale(25_000.0).label(), "1:25 000");
        assert_eq!(Zoom::Width(5000.0).label(), "5 km wide");
    }

    #[test]
    fn hemispheres() {
        assert_eq!(parse("12.5S 122.5E", true), deg(122.5, -12.5));
        assert_eq!(parse("122.5E 12.5S", true), deg(122.5, -12.5));
        assert_eq!(parse("S 12.5, E 122.5", true), deg(122.5, -12.5));
        assert_eq!(parse("12.5 N, 45 W", true), deg(-45.0, 12.5));
        assert_eq!(parse("45w 12.5n", true), deg(-45.0, 12.5));
        // one letter fixes it, the other is the other axis (30 40 alone would be lon 30, lat 40)
        assert_eq!(parse("30 40N", true), deg(30.0, 40.0));
        assert_eq!(parse("30N 40", true), deg(40.0, 30.0));
        assert_eq!(parse("30 40E", true), deg(40.0, 30.0));
    }

    #[test]
    fn minutes_and_seconds() {
        close(parse("12°20'30\"S 122°30'E", true), 122.5, -(12.0 + 20.0 / 60.0 + 30.0 / 3600.0));
        close(parse("12° 20' 30\" S, 122° 30' E", true), 122.5, -(12.0 + 20.0 / 60.0 + 30.0 / 3600.0));
        close(parse("N12°30' E122°15'", true), 122.25, 12.5);
        close(parse("12.5°N 122.5°E", true), 122.5, 12.5);
        close(parse("12°30'N, 45°W", true), -45.0, 12.5);
        // typographic marks, two quotes for seconds
        close(parse("12°20′30″N 122°30′E", true), 122.5, 12.0 + 20.0 / 60.0 + 30.0 / 3600.0);
        close(parse("12°20'30''N 122°E", true), 122.0, 12.0 + 20.0 / 60.0 + 30.0 / 3600.0);
        // degrees with a sign and marks
        close(parse("-12°30', 122°", true), 122.0, -12.5);
    }

    #[test]
    fn map_units() {
        assert_eq!(parse("450000 1300000", false), Ok(Target::Map { x: 450_000.0, y: 1_300_000.0 }));
        assert_eq!(parse("450000.5, -1300000", false), Ok(Target::Map { x: 450_000.5, y: -1_300_000.0 }));
        // (beyond the ranges of degrees: fine here)
        assert!(parse("1 2 3", false).unwrap_err().contains("two numbers"));
        assert!(parse("12N 5", false).is_err());
    }

    #[test]
    fn errors_say_what_is_wrong() {
        let e = |t: &str| parse(t, true).unwrap_err();
        assert!(e("").contains("type a coordinate"));
        assert!(e("12.5").contains("second coordinate"), "{}", e("12.5"));
        assert!(e("12.5, 122.5, 3").contains("more than two"), "{}", e("12.5, 122.5, 3"));
        assert!(e("abc").contains("unexpected"), "{}", e("abc"));
        assert!(e("12.5.5, 3").contains("not a number"), "{}", e("12.5.5, 3"));
        // a latitude beyond 90 with a comma: latitude first, said
        let m = e("122.5, 12.5");
        assert!(m.contains("latitude 122.5") && m.contains("comma"), "{m}");
        assert!(e("12.5, 200").contains("longitude 200"), "{}", e("12.5, 200"));
        assert!(e("100 200").contains("beyond 90"), "{}", e("100 200"));
        assert!(e("12N 20N").contains("both coordinates are latitudes"), "{}", e("12N 20N"));
        assert!(e("12E 20W").contains("both coordinates are longitudes"));
        assert!(e("-12S 20E").contains("sign and a hemisphere"));
        assert!(e("12°70'N 20E").contains("minutes"), "{}", e("12°70'N 20E"));
        assert!(e("12°20'61\"N 20E").contains("seconds"), "{}", e("12°20'61\"N 20E"));
        assert!(e("12° 20 N 20E").contains("minutes need"), "{}", e("12° 20 N 20E"));
        assert!(e("91N 20E").contains("latitude 91"));
        assert!(e("N").contains("number of degrees"));
    }

    fn utm() -> Grid {
        let mut g = Grid::new(400, 300);
        g.transform = [450_000.0, 10.0, 0.0, 1_300_000.0, 0.0, -10.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 3, 1024, 0, 1, 1, 3072, 0, 1, 32651, 3076, 0, 1, 9001]);
        g
    }

    fn geographic() -> Grid {
        let mut g = Grid::new(1000, 800);
        g.transform = [122.5, 0.0001, 0.0, 11.8, 0.0, -0.0001];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 2, 2048, 0, 1, 4326]);
        g
    }

    #[test]
    fn a_place_is_a_cell_of_the_map() {
        // degrees on a geographic map: direct
        let g = geographic();
        let (cell, inside) = locate(&g, Target::Degrees { lon: 122.55, lat: 11.75 }).unwrap();
        assert!((cell[0] - 500.0).abs() < 1e-6 && (cell[1] - 500.0).abs() < 1e-6, "{cell:?}");
        assert!(inside);
        let (_, inside) = locate(&g, Target::Degrees { lon: 123.0, lat: 11.75 }).unwrap();
        assert!(!inside, "outside the map");
        // map units
        let (cell, inside) = locate(&utm(), Target::Map { x: 452_005.0, y: 1_299_015.0 }).unwrap();
        assert!((cell[0] - 200.5).abs() < 1e-9 && (cell[1] - 98.5).abs() < 1e-9 && inside);
        // no coordinate system: only map units
        let plain = Grid::new(10, 10);
        assert!(locate(&plain, Target::Degrees { lon: 1.0, lat: 1.0 }).is_err());
        assert!(locate(&plain, Target::Map { x: 2.5, y: -3.5 }).unwrap().1);
    }

    #[test]
    fn degrees_reach_a_projected_map_through_gdal() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        // UTM 51N, central meridian 123 E: the central meridian at the equator is (500000, 0)
        let mut g = Grid::new(100, 100);
        g.transform = [499_000.0, 10.0, 0.0, 1_000.0, 0.0, -10.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 3, 1024, 0, 1, 1, 3072, 0, 1, 32651, 3076, 0, 1, 9001]);
        let (cell, _) = locate(&g, Target::Degrees { lon: 123.0, lat: 0.0 }).unwrap();
        assert!((cell[0] - 100.0).abs() < 1e-3 && (cell[1] - 100.0).abs() < 1e-3, "{cell:?}");
        // 111 m west and 55 m north of it: 11 cells left, 5.5 up
        let (cell, inside) = locate(&g, Target::Degrees { lon: 122.999, lat: 0.0005 }).unwrap();
        assert!((cell[0] - 88.9).abs() < 0.1 && (cell[1] - 94.5).abs() < 0.1 && inside, "{cell:?}");
    }

    #[test]
    fn places_as_text() {
        assert_eq!(degrees_text(122.5, -12.5), "12.50000°S 122.50000°E");
        assert_eq!(place_text(&geographic(), [0.0, 0.0]), "11.80000°N 122.50000°E");
        assert_eq!(place_text(&utm(), [0.0, 0.0]), "x 450000.00  y 1300000.00");
    }
}
