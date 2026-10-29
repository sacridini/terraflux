//! Times of tracks: datetimes written as text (ISO 8601, read in UTC, as pandas' to_datetime(utc=True)) to and from
//! nanoseconds since 1970, and lubridate's floor_date of the intervals of one location per interval.

const NS: i64 = 1_000_000_000;
const DAY: i64 = 86_400 * NS;

/// Days since 1970-01-01 of a date of the proleptic Gregorian calendar (Hinnant's days_from_civil).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date (year, month, day) of days since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { yoe + era * 400 + 1 } else { yoe + era * 400 }, m, d)
}

fn digits(s: &[u8], at: &mut usize, n: usize) -> Option<i64> {
    if *at + n > s.len() || !s[*at..*at + n].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let v = s[*at..*at + n].iter().fold(0i64, |a, &c| a * 10 + i64::from(c - b'0'));
    *at += n;
    Some(v)
}

/// A datetime written as text in nanoseconds since 1970 (UTC): `2024-01-31`, `2024-01-31 12:00`,
/// `2024-01-31T12:00:00.250Z`, `2024/01/31 12:00:00+02` (GDAL's form), with or without an offset (`Z`, `+02`,
/// `+02:00`, `-0300`, ` UTC`: none is UTC). None for an empty or unreadable text.
pub fn parse(text: &str) -> Option<i64> {
    let s = text.trim().as_bytes();
    if s.is_empty() {
        return None;
    }
    let mut at = 0;
    let y = digits(s, &mut at, 4)?;
    let sep = *s.get(at)?;
    if sep != b'-' && sep != b'/' {
        return None;
    }
    at += 1;
    let m = digits(s, &mut at, 2)?;
    if s.get(at) != Some(&sep) {
        return None;
    }
    at += 1;
    let d = digits(s, &mut at, 2)?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let (mut hh, mut mm, mut ss, mut frac) = (0, 0, 0, 0i64);
    if at < s.len() && (s[at] == b'T' || s[at] == b' ') && s.get(at + 1).is_some_and(u8::is_ascii_digit) {
        at += 1;
        hh = digits(s, &mut at, 2)?;
        if s.get(at) == Some(&b':') {
            at += 1;
            mm = digits(s, &mut at, 2)?;
            if s.get(at) == Some(&b':') {
                at += 1;
                ss = digits(s, &mut at, 2)?;
                if matches!(s.get(at), Some(b'.' | b',')) {
                    at += 1;
                    let start = at;
                    while at < s.len() && s[at].is_ascii_digit() {
                        at += 1;
                    }
                    let f = &s[start..at];
                    if f.is_empty() {
                        return None;
                    }
                    // nanoseconds: the first nine digits
                    for k in 0..9 {
                        frac = frac * 10 + f.get(k).map_or(0, |&c| i64::from(c - b'0'));
                    }
                }
            }
        }
        if hh > 23 || mm > 59 || ss > 60 {
            return None;
        }
    }
    let rest = std::str::from_utf8(&s[at..]).ok()?.trim();
    let offset = match rest {
        "" | "Z" | "z" | "UTC" | "GMT" | "+00" | "+0000" | "+00:00" => 0,
        r => {
            let sign = match r.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let r = r[1..].replace(':', "");
            let b = r.as_bytes();
            let mut k = 0;
            let oh = digits(b, &mut k, 2)?;
            let om = if b.len() > 2 { digits(b, &mut k, 2)? } else { 0 };
            if k != b.len() {
                return None;
            }
            sign * (oh * 3600 + om * 60)
        }
    };
    let secs = days_from_civil(y, m, d) * 86_400 + hh * 3600 + mm * 60 + ss - offset;
    secs.checked_mul(NS)?.checked_add(frac)
}

/// A time in nanoseconds since 1970 as text: `2024-01-31 12:00:00`, with the fraction of a second when it has one
/// (milli-, micro- or nanoseconds).
pub fn format(ns: i64) -> String {
    let days = ns.div_euclid(DAY);
    let rest = ns.rem_euclid(DAY);
    let (y, m, d) = civil_from_days(days);
    let s = rest / NS;
    let frac = rest % NS;
    let mut out = format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60);
    if frac != 0 {
        let f = format!("{frac:09}");
        let keep = if frac % 1_000_000 == 0 {
            3
        } else if frac % 1000 == 0 {
            6
        } else {
            9
        };
        out.push('.');
        out.push_str(&f[..keep]);
    }
    out
}

/// A unit of lubridate's floor_date: a whole number of seconds, minutes, hours, days, weeks, months or years.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unit {
    Second(i64),
    Minute(i64),
    Hour(i64),
    Day(i64),
    Week,
    Month(i64),
    Year(i64),
}

/// A unit as lubridate writes it: `hour`, `15 mins`, `2 hours`, `day`, `week`, `month`, `year`...
pub fn unit(text: &str) -> Result<Unit, String> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    let (n, name) = match parts.as_slice() {
        [name] => (1, *name),
        [n, name] => {
            let v: f64 = n.parse().map_err(|_| format!("unknown unit {text:?}"))?;
            if v.fract() != 0.0 || v < 1.0 {
                return Err("unit: a whole number of units, e.g. '15 mins'".into());
            }
            (v as i64, *name)
        }
        _ => return Err(format!("unknown unit {text:?}")),
    };
    Ok(match name.to_lowercase().as_str() {
        "second" | "seconds" | "sec" | "secs" | "s" => Unit::Second(n),
        "minute" | "minutes" | "min" | "mins" => Unit::Minute(n),
        "hour" | "hours" | "h" => Unit::Hour(n),
        "day" | "days" | "d" => Unit::Day(n),
        "week" | "weeks" => {
            if n != 1 {
                return Err("unit: weeks one at a time".into());
            }
            Unit::Week
        }
        "month" | "months" => Unit::Month(n),
        "year" | "years" => Unit::Year(n),
        _ => return Err(format!("unknown unit {text:?}")),
    })
}

/// lubridate::floor_date(t, unit) in UTC: multiples count from the start of the next larger unit (minutes within
/// the hour, hours within the day, days within the month), weeks from Sunday.
pub fn floor(ns: i64, u: Unit) -> i64 {
    let days = ns.div_euclid(DAY);
    let day = days * DAY;
    let rest = ns - day;
    let (y, m, d) = civil_from_days(days);
    match u {
        Unit::Second(n) => {
            let minute = day + rest / (60 * NS) * 60 * NS;
            // pandas: the seconds as a float (second + microsecond / 1e6 + nanosecond / 1e9)
            let within = rest - (minute - day);
            let sec = (within / NS) as f64 + (within % NS / 1000) as f64 / 1e6 + (within % 1000) as f64 / 1e9;
            minute + ((sec / n as f64).floor() * n as f64) as i64 * NS
        }
        Unit::Minute(n) => {
            let hour = day + rest / (3600 * NS) * 3600 * NS;
            let minute = (rest / (60 * NS)) % 60;
            hour + (minute / n) * n * 60 * NS
        }
        Unit::Hour(n) => {
            let hour = rest / (3600 * NS);
            day + (hour / n) * n * 3600 * NS
        }
        Unit::Day(n) => day - ((d - 1) - ((d - 1) / n) * n) * DAY,
        Unit::Week => {
            // 1970-01-01 was a Thursday: Monday 0 ... Sunday 6
            let dow = (days + 3).rem_euclid(7);
            day - ((dow + 1) % 7) * DAY
        }
        Unit::Month(n) => days_from_civil(y, ((m - 1) / n) * n + 1, 1) * DAY,
        Unit::Year(n) => days_from_civil(y.div_euclid(n) * n, 1, 1) * DAY,
    }
}

/// `t` moved on by a unit (months and years by the calendar).
pub fn add(ns: i64, u: Unit) -> i64 {
    match u {
        Unit::Second(n) => ns + n * NS,
        Unit::Minute(n) => ns + n * 60 * NS,
        Unit::Hour(n) => ns + n * 3600 * NS,
        Unit::Day(n) => ns + n * DAY,
        Unit::Week => ns + 7 * DAY,
        Unit::Month(n) | Unit::Year(n) => {
            let months = if matches!(u, Unit::Month(_)) { n } else { 12 * n };
            let days = ns.div_euclid(DAY);
            let (y, m, d) = civil_from_days(days);
            let k = y * 12 + (m - 1) + months;
            let (y2, m2) = (k.div_euclid(12), k.rem_euclid(12) + 1);
            // pandas' DateOffset: the day kept, or the month's last
            let last = days_from_civil(if m2 == 12 { y2 + 1 } else { y2 }, if m2 == 12 { 1 } else { m2 + 1 }, 1)
                - days_from_civil(y2, m2, 1);
            days_from_civil(y2, m2, d.min(last)) * DAY + (ns - days * DAY)
        }
    }
}

/// The step of a regular sequence of times (`interpolate`'s times given as a unit), as landschaft's _step reads
/// it: `1 hour`, `15 mins`, `day`, `2 weeks`, `month`, `year`.
pub fn step(text: &str) -> Result<Unit, String> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    let (n, name) = match parts.as_slice() {
        [n, name] => (n.parse::<f64>().map_err(|_| format!("unknown unit {text:?}"))? as i64, name.to_string()),
        [name] => (1, name.to_string()),
        _ => return Err(format!("unknown unit {text:?}")),
    };
    let lower = name.to_lowercase();
    let name = if lower != "s" && lower != "secs" { lower.trim_end_matches('s').to_string() } else { "sec".into() };
    Ok(match name.as_str() {
        "sec" | "second" => Unit::Second(n),
        "min" | "minute" => Unit::Minute(n),
        "hour" | "h" => Unit::Hour(n),
        "day" | "d" => Unit::Day(n),
        "week" => Unit::Day(7 * n),
        "month" => Unit::Month(n),
        "year" => Unit::Year(n),
        _ => return Err(format!("unknown unit {text:?}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        let t = parse("2024-03-01 10:20:30.5").unwrap();
        assert_eq!(format(t), "2024-03-01 10:20:30.500");
        assert_eq!(parse("2024/03/01 12:20:30.5+02"), Some(t));
        assert_eq!(parse("2024-03-01T10:20:30.500Z"), Some(t));
        assert_eq!(parse("1969-12-31"), Some(-DAY));
        assert_eq!(format(floor(t, unit("month").unwrap())), "2024-03-01 00:00:00");
        assert_eq!(format(floor(t, unit("2 hours").unwrap())), "2024-03-01 10:00:00");
        assert_eq!(format(floor(t, unit("15 mins").unwrap())), "2024-03-01 10:15:00");
        // a Friday: the week from Sunday the 25th of February
        assert_eq!(format(floor(t, unit("week").unwrap())), "2024-02-25 00:00:00");
        assert_eq!(format(add(parse("2024-01-31").unwrap(), Unit::Month(1))), "2024-02-29 00:00:00");
        assert!(parse("31/01/2024").is_none());
    }
}
