//! Minimal UTC RFC 3339 timestamp formatting.
//!
//! Hand-rolled replacement for `chrono::Utc::now().to_rfc3339()` — the only
//! chrono surface this crate used. Output is byte-identical to chrono's
//! (`+00:00` offset, 0/3/6/9 fractional digits via trailing-zero trimming).
//! Date conversion follows Howard Hinnant's `civil_from_days` algorithm
//! (https://howardhinnant.github.io/date_algorithms.html), the same one
//! chrono uses internally. Reintroduce chrono if we ever need parsing,
//! arithmetic, or non-UTC time zones.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current UTC time as an RFC 3339 string, e.g. `2026-07-10T18:22:03.123456+00:00`.
pub fn now_rfc3339() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format_rfc3339(now.as_secs(), now.subsec_nanos())
}

/// Format a Unix timestamp (seconds + nanoseconds since epoch) as RFC 3339 UTC.
pub fn format_rfc3339(unix_secs: u64, nanos: u32) -> String {
    let days = unix_secs / 86_400;
    let secs_of_day = unix_secs % 86_400;
    let (hour, min, sec) = (
        secs_of_day / 3_600,
        (secs_of_day % 3_600) / 60,
        secs_of_day % 60,
    );

    // civil_from_days (Hinnant): days since 1970-01-01 -> (y, m, d).
    // Shift epoch to 0000-03-01 so leap days land at the end of the cycle.
    let z = days as i64 + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097; // day of 400-year era
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // year of era
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of March-based year
    let mp = (5 * doy + 2) / 153; // March-based month
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);

    // Fraction digits match chrono's SecondsFormat::AutoSi: trim trailing
    // zeros in whole groups of three.
    let mut out = format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}");
    if nanos != 0 {
        if nanos.is_multiple_of(1_000_000) {
            out.push_str(&format!(".{:03}", nanos / 1_000_000));
        } else if nanos.is_multiple_of(1_000) {
            out.push_str(&format!(".{:06}", nanos / 1_000));
        } else {
            out.push_str(&format!(".{nanos:09}"));
        }
    }
    out.push_str("+00:00");
    out
}
