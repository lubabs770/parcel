//! RFC 3339 → epoch seconds, and friendly relative dates. No date crate needed.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// "2026-10-25T18:03:56Z", fractional seconds and ±hh:mm offsets allowed.
pub fn parse(s: &str) -> Option<i64> {
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (h, mi, se) = (n(11..13)?, n(14..16)?, n(17..19)?);
    let mut rest = s.get(19..)?;
    if let Some(r) = rest.strip_prefix('.') {
        rest = r.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    let off = match rest.as_bytes().first() {
        None | Some(b'Z') | Some(b'z') => 0,
        Some(&sign @ (b'+' | b'-')) => {
            let o = rest.get(1..3)?.parse::<i64>().ok()? * 3600 + rest.get(4..6)?.parse::<i64>().ok()? * 60;
            if sign == b'+' { o } else { -o }
        }
        _ => return None,
    };
    Some(days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + se - off)
}

/// "now", "5m", "2h", "Tue", "Oct 3", "2024"; unparseable dates are shown as-is.
pub fn relative(s: &str, now: i64) -> String {
    const MON: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const DOW: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"]; // 1970-01-01 was a Thursday
    let Some(t) = parse(s) else { return s.chars().take(10).collect() };
    match now - t {
        a if a < 60 => "now".into(),
        a if a < 3600 => format!("{}m", a / 60),
        a if a < 86400 => format!("{}h", a / 3600),
        a if a < 6 * 86400 => DOW[t.div_euclid(86400).rem_euclid(7) as usize].into(),
        _ => {
            let (y, m, d) = civil_from_days(t.div_euclid(86400));
            if y == civil_from_days(now.div_euclid(86400)).0 { format!("{} {d}", MON[m as usize - 1]) } else { y.to_string() }
        }
    }
}

// Howard Hinnant's civil-date algorithms.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc3339() {
        assert_eq!(parse("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse("2000-03-01T00:00:00Z"), Some(951_868_800));
        assert_eq!(parse("2000-03-01T02:00:00+02:00"), Some(951_868_800));
        assert_eq!(parse("2000-03-01T00:00:00.123Z"), Some(951_868_800));
        assert_eq!(parse("Tue, 3 Oct"), None);
    }

    #[test]
    fn relative_dates() {
        let now = parse("2026-10-25T18:00:00Z").unwrap(); // a Sunday
        assert_eq!(relative("2026-10-25T17:59:30Z", now), "now");
        assert_eq!(relative("2026-10-25T17:58:00Z", now), "2m");
        assert_eq!(relative("2026-10-25T15:00:00Z", now), "3h");
        assert_eq!(relative("2026-10-23T09:00:00Z", now), "Fri");
        assert_eq!(relative("2026-10-03T09:00:00Z", now), "Oct 3");
        assert_eq!(relative("2024-05-01T09:00:00Z", now), "2024");
        assert_eq!(relative("garbage", now), "garbage");
    }
}
