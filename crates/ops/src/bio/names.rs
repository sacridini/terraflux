//! Element names as Biodiverse builds them (landschaft's bio/_names.py): axis values joined with `:` by
//! Text::CSV_XS (quote `'`), numbers as Perl stringifies a double (`%.15g`). Sorting the names gives the order
//! groups and labels are numbered in.

use std::sync::LazyLock;

use regex::Regex;

pub const JOIN_CHAR: char = ':';
pub const QUOTE_CHAR: char = '\'';

/// C's `%.{p}g` of a finite double.
pub fn format_g(x: f64, p: usize) -> String {
    if x == 0.0 {
        return if x.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let e = format!("{:.*e}", p - 1, x);
    let (mantissa, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let strip = |s: &str| -> String {
        if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s.to_string() }
    };
    if exp < -4 || exp >= p as i32 {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", strip(mantissa), exp.abs())
    } else {
        strip(&format!("{:.*}", (p as i32 - 1 - exp) as usize, x))
    }
}

/// A double as Perl stringifies it (`%.15g`; Inf, -Inf, NaN; never -0).
pub fn perl_number(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Inf".into() } else { "-Inf".into() };
    }
    let s = format_g(x, 15);
    if s == "-0" { "0".into() } else { s }
}

static BINARY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x00-\x08\x0a-\x1f\x7f-\u{a0}]").unwrap());

/// One field as Text::CSV_XS `combine` writes it for Biodiverse (spaces and tabs quoted too).
pub fn csv_field(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    if value.contains(JOIN_CHAR)
        || value.contains(QUOTE_CHAR)
        || value.contains(' ')
        || value.contains('\t')
        || BINARY.is_match(value)
    {
        let q = QUOTE_CHAR.to_string();
        return format!("{q}{}{q}", value.replace(QUOTE_CHAR, &q.repeat(2)));
    }
    value.to_string()
}

/// An element name from its axis values (`list2csv`).
pub fn join_element<S: AsRef<str>>(values: &[S]) -> String {
    values.iter().map(|v| csv_field(v.as_ref())).collect::<Vec<_>>().join(":")
}

/// Biodiverse's `dequote_element`: one level of quotes off simple names.
pub fn dequote_element(name: &str) -> String {
    let c: Vec<char> = name.chars().collect();
    if c.len() >= 3
        && c[0] == QUOTE_CHAR
        && c[c.len() - 1] == QUOTE_CHAR
        && c[1..c.len() - 1].iter().all(|ch| *ch != QUOTE_CHAR && !ch.is_whitespace())
    {
        return c[1..c.len() - 1].iter().collect();
    }
    name.to_string()
}

/// The axis values of an element name (`csv2list`).
pub fn split_element(name: &str) -> Vec<String> {
    let c: Vec<char> = name.chars().collect();
    let n = c.len();
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        if i < n && c[i] == QUOTE_CHAR {
            let mut j = i + 1;
            let mut buf = String::new();
            while j < n {
                if c[j] == QUOTE_CHAR {
                    if j + 1 < n && c[j + 1] == QUOTE_CHAR {
                        buf.push(QUOTE_CHAR);
                        j += 2;
                        continue;
                    }
                    break;
                }
                buf.push(c[j]);
                j += 1;
            }
            out.push(buf);
            i = j + 1;
        } else {
            let j = (i..n).find(|&k| c[k] == JOIN_CHAR).unwrap_or(n);
            out.push(c[i..j].iter().collect());
            i = j;
        }
        if i >= n {
            break;
        }
        if c[i] == JOIN_CHAR {
            i += 1;
            if i == n {
                out.push(String::new());
                break;
            }
        }
    }
    out
}

static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[+-]?(?:\d+\.?\d*(?:[eE][+-]?\d+)?|\.\d+(?:[eE][+-]?\d+)?|0[xX][0-9a-fA-F]+)\s*$").unwrap()
});
static SPECIAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[+-]?(?:inf(?:inity)?|nan)$").unwrap());

/// Perl's `looks_like_number` of a string (decimal, exponent, Inf, NaN).
pub fn looks_like_number(s: &str) -> bool {
    let s = if s.chars().next().is_some_and(char::is_whitespace) { s.trim() } else { s };
    if NUMBER.is_match(s) {
        let l = s.to_lowercase();
        return !(l.starts_with("0x") || l.starts_with("+0x") || l.starts_with("-0x"));
    }
    SPECIAL.is_match(s)
}

/// The number of a string that looks like one (Python's float()).
pub fn numify(s: &str) -> f64 {
    let t = s.trim();
    match t.to_lowercase().trim_start_matches(['+', '-']) {
        "inf" | "infinity" => {
            if t.starts_with('-') {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            }
        }
        "nan" => f64::NAN,
        _ => t.parse().unwrap_or(f64::NAN),
    }
}

/// The key of a natural sort (landschaft's _natural_key): runs of digits as numbers, before text.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn parts(s: &str) -> Vec<(u8, u128, String)> {
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut digits = false;
        for ch in s.chars() {
            let d = ch.is_ascii_digit();
            if !cur.is_empty() && d != digits {
                out.push(std::mem::take(&mut cur));
            }
            digits = d;
            cur.push(ch);
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out.into_iter()
            .map(|p| match p.parse::<u128>() {
                Ok(n) if p.chars().all(|c| c.is_ascii_digit()) => (0, n, String::new()),
                _ => (1, 0, p),
            })
            .collect()
    }
    parts(a).cmp(&parts(b))
}

/// Python's repr of a float.
pub fn py_float(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let s = format!("{v:?}");
    // Rust writes 1e20 as 1e20, Python as 1e+20
    if let Some((m, e)) = s.split_once('e') {
        let (sign, digits) = if let Some(d) = e.strip_prefix('-') { ('-', d) } else { ('+', e) };
        return format!("{m}e{sign}{digits:0>2}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perl_prints_doubles_as_percent_15g() {
        assert_eq!(perl_number(150.0), "150");
        assert_eq!(perl_number(0.1 + 0.2), "0.3");
        assert_eq!(perl_number(1e20), "1e+20");
        assert_eq!(perl_number(1.5e-5), "1.5e-05");
        assert_eq!(perl_number(-0.0), "0");
        assert_eq!(perl_number(123456789012345678.0), "1.23456789012346e+17");
        assert_eq!(perl_number(2550000.0), "2550000");
    }

    #[test]
    fn names_split_and_join() {
        assert_eq!(join_element(&["a b", "c"]), "'a b':c");
        assert_eq!(split_element("'a b':c"), vec!["a b", "c"]);
        assert_eq!(split_element("1:2:"), vec!["1", "2", ""]);
        assert_eq!(dequote_element("'ab'"), "ab");
        assert!(looks_like_number("1e5") && looks_like_number(" 3") && !looks_like_number("0x1f"));
    }
}
