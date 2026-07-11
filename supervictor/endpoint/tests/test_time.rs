use supervictor_endpoint::time::{format_rfc3339, now_rfc3339};

/// Reference vectors generated with `chrono 0.4.45`'s
/// `DateTime::<Utc>::from_timestamp(secs, nanos).to_rfc3339()` before the
/// dep was removed. `format_rfc3339` must stay byte-identical to these.
const CHRONO_VECTORS: &[(u64, u32, &str)] = &[
    (0, 0, "1970-01-01T00:00:00+00:00"),
    (1, 1, "1970-01-01T00:00:01.000000001+00:00"),
    // leap day, year divisible by 400
    (951_782_400, 0, "2000-02-29T00:00:00+00:00"),
    // leap day with millisecond fraction
    (1_709_164_800, 500_000_000, "2024-02-29T00:00:00.500+00:00"),
    (
        1_719_791_999,
        999_999_999,
        "2024-06-30T23:59:59.999999999+00:00",
    ),
    // year-end boundary
    (1_735_689_599, 0, "2024-12-31T23:59:59+00:00"),
    (1_735_689_600, 123_000_000, "2025-01-01T00:00:00.123+00:00"),
    // microsecond vs nanosecond precision selection
    (
        1_752_192_000,
        123_456_000,
        "2025-07-11T00:00:00.123456+00:00",
    ),
    (
        1_752_192_000,
        123_456_789,
        "2025-07-11T00:00:00.123456789+00:00",
    ),
    (4_102_444_800, 0, "2100-01-01T00:00:00+00:00"),
    // 2100 is NOT a leap year (divisible by 100, not 400): +28d from Feb 1 lands on Mar 1
    (4_107_542_400, 42_000_000, "2100-03-01T00:00:00.042+00:00"),
    (
        253_402_300_799,
        999_999_999,
        "9999-12-31T23:59:59.999999999+00:00",
    ),
    (86_399, 1_000, "1970-01-01T23:59:59.000001+00:00"),
    (2_147_483_647, 0, "2038-01-19T03:14:07+00:00"),
];

#[test]
fn matches_chrono_reference_vectors() {
    for (secs, nanos, expected) in CHRONO_VECTORS {
        assert_eq!(
            format_rfc3339(*secs, *nanos),
            *expected,
            "mismatch for ({secs}, {nanos})"
        );
    }
}

/// Every day from 1970 through 2106 must produce a valid calendar date and
/// midnight must roll over exactly one day at a time (catches off-by-one and
/// leap-year drift anywhere in the range, not just at hand-picked vectors).
#[test]
fn day_rollover_is_continuous_for_137_years() {
    let mut prev_date = String::new();
    for day in 0u64..50_000 {
        let ts = format_rfc3339(day * 86_400, 0);
        let (date, rest) = ts.split_at(10);
        assert_eq!(rest, "T00:00:00+00:00", "bad time part at day {day}");
        assert!(
            date > prev_date.as_str(),
            "date did not advance at day {day}"
        );
        // month/day stay in calendar range
        let month: u32 = date[5..7].parse().unwrap();
        let dom: u32 = date[8..10].parse().unwrap();
        assert!((1..=12).contains(&month), "bad month at day {day}: {date}");
        assert!((1..=31).contains(&dom), "bad day at day {day}: {date}");
        prev_date = ts;
    }
}

/// Feb 29 appears exactly on leap years within 2000..2104.
#[test]
fn leap_days_land_on_leap_years_only() {
    let mut leap_years = Vec::new();
    for day in 10_957u64..49_000 {
        // 2000-01-01 onward
        let ts = format_rfc3339(day * 86_400, 0);
        if &ts[5..10] == "02-29" {
            leap_years.push(ts[0..4].parse::<u32>().unwrap());
        }
    }
    for y in &leap_years {
        assert!(
            y % 4 == 0 && (y % 100 != 0 || y % 400 == 0),
            "Feb 29 on non-leap year {y}"
        );
    }
    assert!(leap_years.contains(&2000), "2000 must be a leap year");
    assert!(leap_years.contains(&2024), "2024 must be a leap year");
    assert!(!leap_years.contains(&2100), "2100 must not be a leap year");
}

#[test]
fn now_has_rfc3339_shape() {
    let ts = now_rfc3339();
    assert!(ts.ends_with("+00:00"), "missing UTC offset: {ts}");
    assert_eq!(&ts[4..5], "-");
    assert_eq!(&ts[10..11], "T");
    // sanity: we're past 2026 and before year 3000
    let year: u32 = ts[0..4].parse().unwrap();
    assert!((2026..3000).contains(&year), "implausible year: {ts}");
}
