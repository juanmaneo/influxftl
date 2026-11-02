#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the date and time functions against Joda-Time 2.13.1 and `java.time` values.

use super::runtime::current_time;
use super::time::{Period, Zone, add_period, parse_duration, parse_period, parse_timestamp};
use crate::tests::{assert_fails, assert_top, fail_msg, top};
use crate::value::Value;

fn longs(values: &[i64]) -> Value {
    Value::list(values.iter().map(|&v| Value::Long(v)).collect())
}

// -----------------------------------------------------------------------------------------
// ISO8601 / TSELEMENTS
// -----------------------------------------------------------------------------------------

/// `(timestamp, zone, ISO8601, TSELEMENTS)` from Joda-Time.
const INSTANTS: &[(i64, &str, &str, [i64; 10])] = &[
    (
        0,
        "UTC",
        "1970-01-01T00:00:00.000000Z",
        [1970, 1, 1, 0, 0, 0, 0, 1, 4, 1],
    ),
    (
        0,
        "Europe/Paris",
        "1970-01-01T01:00:00.000000+01:00",
        [1970, 1, 1, 1, 0, 0, 0, 1, 4, 1],
    ),
    (
        0,
        "America/New_York",
        "1969-12-31T19:00:00.000000-05:00",
        [1969, 12, 31, 19, 0, 0, 0, 365, 3, 1],
    ),
    (
        1,
        "UTC",
        "1970-01-01T00:00:00.000001Z",
        [1970, 1, 1, 0, 0, 0, 1, 1, 4, 1],
    ),
    (
        -1,
        "UTC",
        "1969-12-31T23:59:59.999999Z",
        [1969, 12, 31, 23, 59, 59, 999_999, 365, 3, 1],
    ),
    (
        -1,
        "Europe/Paris",
        "1970-01-01T00:59:59.999999+01:00",
        [1970, 1, 1, 0, 59, 59, 999_999, 1, 4, 1],
    ),
    (
        1_234_567_890_123_456,
        "UTC",
        "2009-02-13T23:31:30.123456Z",
        [2009, 2, 13, 23, 31, 30, 123_456, 44, 5, 7],
    ),
    (
        1_234_567_890_123_456,
        "Europe/Paris",
        "2009-02-14T00:31:30.123456+01:00",
        [2009, 2, 14, 0, 31, 30, 123_456, 45, 6, 7],
    ),
    (
        -1_234_567_890_123_456,
        "America/New_York",
        "1930-11-17T19:28:29.876544-05:00",
        [1930, 11, 17, 19, 28, 29, 876_544, 321, 1, 47],
    ),
    (
        -1_234_567_890_123_456,
        "Europe/Paris",
        "1930-11-18T00:28:29.876544Z",
        [1930, 11, 18, 0, 28, 29, 876_544, 322, 2, 47],
    ),
    (
        1_700_000_000_000_000,
        "Asia/Kolkata",
        "2023-11-15T03:43:20.000000+05:30",
        [2023, 11, 15, 3, 43, 20, 0, 319, 3, 46],
    ),
    (
        253_402_300_800_000_000,
        "UTC",
        "10000-01-01T00:00:00.000000Z",
        [10000, 1, 1, 0, 0, 0, 0, 1, 6, 52],
    ),
    (
        253_402_300_800_000_000,
        "America/New_York",
        "9999-12-31T19:00:00.000000-05:00",
        [9999, 12, 31, 19, 0, 0, 0, 365, 5, 52],
    ),
    (
        -62_135_596_800_000_000,
        "Europe/Paris",
        "0001-01-01T00:09:21.000000+00:09:21",
        [1, 1, 1, 0, 9, 21, 0, 1, 1, 1],
    ),
    (
        -62_135_596_800_000_000,
        "America/New_York",
        "0000-12-31T19:03:58.000000-04:56:02",
        [0, 12, 31, 19, 3, 58, 0, 366, 7, 52],
    ),
    (
        1_616_893_200_000_000,
        "Europe/Paris",
        "2021-03-28T03:00:00.000000+02:00",
        [2021, 3, 28, 3, 0, 0, 0, 87, 7, 12],
    ),
    (
        1_616_893_200_000_000,
        "America/Sao_Paulo",
        "2021-03-27T22:00:00.000000-03:00",
        [2021, 3, 27, 22, 0, 0, 0, 86, 6, 12],
    ),
    (
        0,
        "+02:00",
        "1970-01-01T02:00:00.000000+02:00",
        [1970, 1, 1, 2, 0, 0, 0, 1, 4, 1],
    ),
];

#[test]
fn iso8601_and_tselements_match_joda() {
    for (ts, zone, iso, elements) in INSTANTS {
        assert_top(&format!("{ts} '{zone}' ISO8601"), *iso);
        assert_top(&format!("{ts} '{zone}' TSELEMENTS"), longs(elements));
        assert_top(&format!("{ts} '{zone}' ->TSELEMENTS"), longs(elements));
        if *zone == "UTC" {
            assert_top(&format!("{ts} ISO8601"), *iso);
            assert_top(&format!("{ts} TSELEMENTS"), longs(elements));
        }
    }
    // Zone identifiers: fixed offsets, aliases, zero offsets print `Z`.
    assert_top("0 '-0530' ISO8601", "1969-12-31T18:30:00.000000-05:30");
    assert_top("0 '+05' ISO8601", "1970-01-01T05:00:00.000000+05:00");
    assert_top("0 'EST' ISO8601", "1969-12-31T19:00:00.000000-05:00");
    assert_top("0 '+00:00' ISO8601", "1970-01-01T00:00:00.000000Z");
    assert_top(
        "0 '+02:30:15' ISO8601",
        "1970-01-01T02:30:15.000000+02:30:15",
    );
    assert_top("0 '+023015' ISO8601", "1970-01-01T02:30:15.000000+02:30:15");
    assert_top(
        "1700000000000000 'Europe/London' ISO8601",
        "2023-11-14T22:13:20.000000Z",
    );
    assert_fails(
        "0 'Mars/Olympus' ISO8601",
        "The datetime zone id 'Mars/Olympus' is not recognised",
    );
    assert_fails(
        "0 'utc' ISO8601",
        "The datetime zone id 'utc' is not recognised",
    );
    assert_fails(
        "0 '+25:00' ISO8601",
        "Invalid format: \"+25:00\" is malformed at \"25:00\"",
    );
    assert_fails(
        "0 '+2:00' ISO8601",
        "Invalid format: \"+2:00\" is malformed at \"2:00\"",
    );
    assert_fails(
        "0 '+02:60' ISO8601",
        "Invalid format: \"+02:60\" is malformed at \"60\"",
    );
    assert_fails(
        "0 '+02:00x' ISO8601",
        "Invalid format: \"+02:00x\" is malformed at \"x\"",
    );
    assert_fails(
        "0 '+02:00:99' ISO8601",
        "Invalid format: \"+02:00:99\" is malformed at \"99\"",
    );
    assert_fails(
        "0 '+02:30:15x' ISO8601",
        "Invalid format: \"+02:30:15x\" is malformed at \"x\"",
    );
    assert_top("-62198755200000000 ISO8601", "-0001-01-01T00:00:00.000000Z");
    assert_top("[ 2021 1 1 ] '+02:00' TSELEMENTS->", 1_609_452_000_000_000);
    assert_fails(
        "'x' 'y' ISO8601",
        "ISO8601 operates on a timestamp or a timestamp + timezone.",
    );
    assert_fails(
        "1.5 ISO8601",
        "ISO8601 operates on a timestamp or a timestamp + timezone.",
    );
    assert_fails(
        "'x' 'UTC' TSELEMENTS",
        "TSELEMENTS operates on a timestamp or a timestamp + timezone.",
    );
    assert_fails(
        "1.5 ->TSELEMENTS",
        "->TSELEMENTS operates on a timestamp or a timestamp + timezone.",
    );
    assert_fails("9223372036854775807 ISO8601", "is out of range.");
    assert_fails("9223372036854775807 TSELEMENTS", "is out of range.");
    assert_fails(
        "-9223372036854775808 'Europe/Paris' ->TSELEMENTS",
        "is out of range.",
    );
    assert_fails("ISO8601", "Empty stack.");
    assert_fails("'UTC' ISO8601", "Empty stack.");
}

#[test]
fn fromtselements_matches_joda() {
    assert_top("[ ] TSELEMENTS->", 0);
    assert_top("[ 2021 ] TSELEMENTS->", 1_609_459_200_000_000);
    assert_top("[ 2021 ] 'UTC' TSELEMENTS->", 1_609_459_200_000_000);
    assert_top(
        "[ 2021 2 28 23 59 59 1234567 ] TSELEMENTS->",
        1_614_556_799_234_567,
    );
    assert_top(
        "[ 2021 2 28 23 59 59 -5 ] TSELEMENTS->",
        1_614_556_798_999_995,
    );
    assert_top("[ -1 1 1 ] TSELEMENTS->", -62_198_755_200_000_000);
    assert_top("[ 1969 12 31 23 59 59 999999 ] TSELEMENTS->", -1);
    // Overlaps take the earlier instant, gaps are illegal.
    assert_top(
        "[ 2021 10 31 2 30 0 ] 'Europe/Paris' TSELEMENTS->",
        1_635_640_200_000_000,
    );
    assert_top(
        "[ 2021 11 7 1 30 0 ] 'America/New_York' TSELEMENTS->",
        1_636_263_000_000_000,
    );
    assert_fails(
        "[ 2021 3 28 2 30 0 ] 'Europe/Paris' TSELEMENTS->",
        "Illegal instant due to time zone offset transition (daylight savings time 'gap'): 2021-03-28T02:30:00.000 (Europe/Paris)",
    );
    assert_fails(
        "[ 2021 3 14 2 30 0 ] 'America/New_York' TSELEMENTS->",
        "Illegal instant due to time zone offset transition (daylight savings time 'gap'): 2021-03-14T02:30:00.000 (America/New_York)",
    );
    // Round trips.
    for (ts, zone, _, _) in INSTANTS {
        let whole_second = ts - ts.rem_euclid(1_000_000);
        assert_top(
            &format!("{whole_second} '{zone}' TSELEMENTS '{zone}' TSELEMENTS->"),
            whole_second,
        );
    }
    // Field validation.
    assert_fails(
        "[ 2021 13 1 ] TSELEMENTS->",
        "Value 13 for monthOfYear must be in the range [1,12]",
    );
    assert_fails(
        "[ 2021 2 30 ] TSELEMENTS->",
        "Value 30 for dayOfMonth must be in the range [1,28]: year: 2021 month: 2",
    );
    assert_fails(
        "[ 2024 2 30 ] TSELEMENTS->",
        "Value 30 for dayOfMonth must be in the range [1,29]: year: 2024 month: 2",
    );
    assert_fails(
        "[ 2021 2 28 24 ] TSELEMENTS->",
        "Value 24 for hourOfDay must be in the range [0,23]",
    );
    assert_fails(
        "[ 2021 2 28 23 60 ] TSELEMENTS->",
        "Value 60 for minuteOfHour must be in the range [0,59]",
    );
    assert_fails(
        "[ 2021 2 28 23 59 60 ] TSELEMENTS->",
        "Value 60 for secondOfMinute must be in the range [0,59]",
    );
    assert_fails(
        "[ 2021 'x' ] TSELEMENTS->",
        "class java.lang.String cannot be cast to class java.lang.Number",
    );
    assert_fails("[ 300000 1 1 ] TSELEMENTS->", "Value 300000 for year");
    assert_fails(
        "[ 2021 1 1 0 0 0 'x' ] TSELEMENTS->",
        "class java.lang.String cannot be cast to class java.lang.Number",
    );
    assert_fails(
        "1 TSELEMENTS->",
        "TSELEMENTS-> operates on a list of timestamp elements or a list of timestamp elements + timezone.",
    );
    assert_fails(
        "1 'UTC' TSELEMENTS->",
        "TSELEMENTS-> operates on a list of timestamp elements or a list of timestamp elements + timezone.",
    );
    assert_fails("TSELEMENTS->", "Empty stack.");
    assert_fails("'UTC' TSELEMENTS->", "Empty stack.");
    assert_fails("[ 2021 ] 'Mars/Olympus' TSELEMENTS->", "is not recognised");
}

// -----------------------------------------------------------------------------------------
// ADDDAYS / ADDMONTHS / ADDYEARS
// -----------------------------------------------------------------------------------------

#[test]
fn add_units_to_instants_match_joda() {
    let jan31 = 1_612_051_200_000_000i64;
    assert_top(&format!("{jan31} 1 ADDMONTHS"), 1_614_470_400_000_000);
    assert_top(
        &format!("{jan31} 'UTC' 13 ADDMONTHS"),
        1_646_006_400_000_000,
    );
    assert_top(&format!("{jan31} -11 ADDMONTHS"), 1_582_934_400_000_000);
    let feb29 = 1_582_934_400_000_000i64;
    assert_top(&format!("{feb29} 1 ADDYEARS"), 1_614_470_400_000_000);
    assert_top(&format!("{feb29} -4 ADDYEARS"), 1_456_704_000_000_000);
    // Days across daylight saving transitions (sub-millisecond part kept).
    assert_top(
        "1616889600000123 'Europe/Paris' 1 ADDDAYS",
        1_616_972_400_000_123,
    );
    assert_top(
        "1616808600000000 'Europe/Paris' 1 ADDDAYS",
        1_616_895_000_000_000,
    );
    assert_top(
        "1615685400000000 'America/New_York' 1 ADDDAYS",
        1_615_768_200_000_000,
    );
    assert_top(
        "1635036600000000 'Europe/Paris' 7 ADDDAYS",
        1_635_641_400_000_000,
    );
    assert_top(
        "1636331400000000 'Europe/Paris' -8 ADDDAYS",
        1_635_636_600_000_000,
    );
    assert_top(
        "1635744600000000 'America/New_York' 6 ADDDAYS",
        1_636_263_000_000_000,
    );
    assert_top(
        "1636353000000000 'America/New_York' -1 ADDDAYS",
        1_636_263_000_000_000,
    );
    assert_top("-1 1 ADDDAYS", 86_399_999_999);
    assert_top("-1000001 -1 ADDDAYS", -86_401_000_001);
    assert_fails(
        "9223372036854775807 1000000 ADDDAYS",
        "The instant 9223372036854775 is out of range.",
    );
    assert_fails(
        "8000000000000000000 2000000000 ADDDAYS",
        "The calculation caused an overflow.",
    );
    assert_fails(
        "0 'x' ADDDAYS",
        "ADDDAYS expects a number of days on top of the stack.",
    );
    assert_fails(
        "0 'x' ADDMONTHS",
        "ADDMONTHS expects a number of months on top of the stack.",
    );
    assert_fails(
        "0 'x' ADDYEARS",
        "ADDYEARS expects a number of years on top of the stack.",
    );
    assert_fails(
        "'x' 'UTC' 1 ADDDAYS",
        "ADDDAYS operates on a tselements list, timestamp, or timestamp and timezone.",
    );
    assert_fails(
        "1.5 1 ADDDAYS",
        "ADDDAYS operates on a tselements list, timestamp, or timestamp and timezone.",
    );
    assert_fails("0 'Mars/Olympus' 1 ADDDAYS", "is not recognised");
    assert_fails("ADDDAYS", "Empty stack.");
    assert_fails("1 ADDDAYS", "Empty stack.");
    assert_fails("'UTC' 1 ADDDAYS", "Empty stack.");
}

#[test]
fn add_units_to_elements() {
    assert_top("[ 2021 1 31 ] 1 ADDDAYS", longs(&[2021, 2, 1]));
    assert_top("[ 2021 12 31 12 ] 1 ADDDAYS", longs(&[2022, 1, 1, 12]));
    assert_top("[ 2021 3 1 ] -1 ADDDAYS", longs(&[2021, 2, 28]));
    assert_top("[ 2021 3 15 ] -3 ADDDAYS", longs(&[2021, 3, 12]));
    assert_top("[ 2024 3 1 ] -1 ADDDAYS", longs(&[2024, 2, 29]));
    assert_top("[ 2021 1 1 ] -1 ADDDAYS", longs(&[2020, 12, 31]));
    assert_top("[ 2021 4 30 ] 1 ADDDAYS", longs(&[2021, 5, 1]));
    assert_top("[ 2021 5 1 ] -1 ADDDAYS", longs(&[2021, 4, 30]));
    assert_top("[ 2021 1 1 ] 365 ADDDAYS", longs(&[2022, 1, 1]));
    assert_top("[ 2021 1 31 ] 1 ADDMONTHS", longs(&[2021, 2, 28]));
    assert_top("[ 2024 1 31 ] 1 ADDMONTHS", longs(&[2024, 2, 29]));
    assert_top("[ 2021 3 31 ] 1 ADDMONTHS", longs(&[2021, 4, 30]));
    assert_top("[ 2021 12 15 ] 1 ADDMONTHS", longs(&[2022, 1, 15]));
    assert_top("[ 2021 1 15 ] -13 ADDMONTHS", longs(&[2019, 12, 15]));
    assert_top("[ 2021 1 ] 2 ADDMONTHS", longs(&[2021, 3]));
    assert_top("[ 2020 2 29 ] 1 ADDYEARS", longs(&[2021, 2, 28]));
    assert_top("[ 2020 2 29 ] 4 ADDYEARS", longs(&[2024, 2, 29]));
    assert_top("[ 2100 1 31 ] 1 ADDMONTHS", longs(&[2100, 2, 28]));
    assert_top("[ 2000 1 31 ] 1 ADDMONTHS", longs(&[2000, 2, 29]));
    assert_fails("[ ] 1 ADDDAYS", "Index 0 out of bounds for length 0");
    assert_fails("[ 2021 ] 1 ADDDAYS", "Index 1 out of bounds for length 1");
    assert_fails(
        "[ 2021 1 'x' ] 1 ADDMONTHS",
        "class java.lang.String cannot be cast to class java.lang.Number",
    );
    assert_fails("[ 2021 1 ] 1 ADDDAYS", "Index 2 out of bounds for length 2");
    assert_fails(
        "[ 2021 1 true ] 1 ADDDAYS",
        "class java.lang.Boolean cannot be cast to class java.lang.Number",
    );
    assert_fails(
        "[ 2021 [ ] ] 1 ADDMONTHS",
        "class java.util.ArrayList cannot be cast to class java.lang.Number",
    );
    assert_fails(
        "[ 2021 { } ] 1 ADDYEARS",
        "class java.util.LinkedHashMap cannot be cast to class java.lang.Number",
    );
    assert_fails(
        "[ 2021 NULL ] 1 ADDYEARS",
        "class java.lang.Object cannot be cast to class java.lang.Number",
    );
}

// -----------------------------------------------------------------------------------------
// Durations
// -----------------------------------------------------------------------------------------

#[test]
fn duration_matches_joda() {
    let cases = [
        ("PT1S", 1_000_000),
        ("P1D", 86_400_000_000),
        ("P1W1DT1H1M1.5S", 694_861_500_000),
        ("-PT1S", -1_000_000),
        ("PT-1S", -1_000_000),
        ("PT-0.5S", -500_000),
        ("PT0.5S", 500_000),
        ("PT1.000001S", 1_000_001),
        ("PT1.5", 1_000_000),
        ("PT1,5S", 1_500_000),
        ("PT1S ", 1_000_000),
        ("P-1D", -86_400_000_000),
        ("PT-1H30M", -1_800_000_000),
        ("PT", 0),
        ("P", 0),
        ("X", 0),
        ("P1DT", 86_400_000_000),
        ("PT-00.25S", -250_000),
        ("PT00.25S", 250_000),
        ("PT1.5S", 1_500_000),
    ];
    for (text, expected) in cases {
        assert_top(&format!("'{text}' DURATION"), expected);
    }
    assert_fails(
        "'P1M' DURATION",
        "DURATION encountered an error while parsing duration. (no support for ambiguous durations containing years or months, please convert those to days.)",
    );
    assert_fails(
        "'P1Y' DURATION",
        "no support for ambiguous durations containing years or months",
    );
    assert_fails(
        "'PT1.2.3S' DURATION",
        "DURATION encountered an error while parsing duration. (invalid ISO8601 duration.)",
    );
    assert_fails("'PT1.xS' DURATION", "For input string: \"0.x\"");
    assert_fails(
        "1 DURATION",
        "DURATION expects an ISO8601 duration (a string) on top of the stack. See http://en.wikipedia.org/wiki/ISO_8601#Durations",
    );
    assert_fails("DURATION", "Empty stack.");
    // The period parser returns Joda's positions.
    let mut period = Period::default();
    assert_eq!(parse_period("PT", &mut period), -3);
    assert_eq!(parse_period("P1DT", &mut period), -5);
    assert_eq!(period.days, 1);
    assert_eq!(parse_period("X", &mut period), -1);
    assert_eq!(parse_period("P1Y2M3W4DT5H6M7.890S", &mut period), 20);
    assert_eq!(
        period,
        Period {
            years: 1,
            months: 2,
            weeks: 3,
            days: 4,
            hours: 5,
            minutes: 6,
            seconds: 7,
            millis: 890,
        }
    );
    let mut period = Period::default();
    assert_eq!(parse_period("PT+1H-2.5S", &mut period), 10);
    assert_eq!((period.hours, period.seconds, period.millis), (1, -2, -500));
    let mut period = Period::default();
    assert_eq!(parse_period("PT-0.5S", &mut period), 7);
    assert_eq!((period.seconds, period.millis), (0, -500));
    let mut period = Period::default();
    assert_eq!(parse_period("PT1.S", &mut period), 5);
    assert_eq!((period.seconds, period.millis), (1, 0));
    assert_eq!(parse_period("PT1.1234S", &mut period), 9);
    assert_eq!((period.seconds, period.millis), (1, 123));
    assert_eq!(parse_period("PT0.0005S", &mut period), 9);
    assert_eq!((period.seconds, period.millis), (0, 0));
    assert_eq!(parse_period("PT99999999999999999999S", &mut period), -3);
    assert_eq!(parse_period("PT1.5", &mut period), -3);
    assert_eq!(parse_period("PT1H1", &mut period), 4);
    assert_eq!(parse_period("P1X", &mut period), 1);
    assert_eq!(parse_period("P-0D", &mut period), 4);
    assert_eq!(
        parse_duration("-P1DT-1H").unwrap(),
        -(86_400 - 3600) * 1_000_000
    );
}

#[test]
fn isoduration_matches_joda() {
    assert_top("0 ISODURATION", "PT0S");
    assert_top("1000000 ISODURATION", "PT1S");
    assert_top("90061001000 ISODURATION", "PT25H1M1.001S");
    assert_top("694861001000 ISODURATION", "PT193H1M1.001S");
    assert_top("-90061001000 ISODURATION", "PT-25H-1M-1.001S");
    assert_top("604800000000 ISODURATION", "PT168H");
    assert_top("1234 ISODURATION", "PT0.001S");
    assert_top("999 ISODURATION", "PT0S");
    assert_top("-1 ISODURATION", "PT0S");
    assert_top("-1000 ISODURATION", "PT-0.001S");
    assert_top("60000000 ISODURATION", "PT1M");
    assert_top("3600000000 ISODURATION", "PT1H");
    assert_top("3601000000 ISODURATION", "PT1H1S");
    assert_fails(
        "'x' ISODURATION",
        "ISODURATION expects a number of time units (LONG) on top of the stack.",
    );
    assert_fails("ISODURATION", "Empty stack.");
}

#[test]
fn humanduration() {
    assert_top("0 HUMANDURATION", "0.000000s");
    assert_top("1 HUMANDURATION", "0.000001s");
    assert_top("1500000 HUMANDURATION", "1.500000s");
    assert_top("61000000 HUMANDURATION", "1m1.000000s");
    assert_top("3661000000 HUMANDURATION", "1h1m1.000000s");
    assert_top("90061001000 HUMANDURATION", "1d1h1m1.001000s");
    assert_top("-90061001000 HUMANDURATION", "-1d1h1m1.001000s");
    assert_top("-1 HUMANDURATION", "-0h0m0.000001s");
    assert_top("86400000000 HUMANDURATION", "1d0h0m0.000000s");
    assert_fails(
        "'x' HUMANDURATION",
        "HUMANDURATION expects a number of time units (LONG) on top of the stack.",
    );
    assert_fails("HUMANDURATION", "Empty stack.");
}

#[test]
fn addduration() {
    assert_top("0 'PT1H' ADDDURATION", 3_600_000_000);
    assert_top("0 'PT1H' 3 ADDDURATION", 10_800_000_000);
    assert_top("0 'P1D' -1 ADDDURATION", -86_400_000_000);
    assert_top("0 'PT1.5S' ADDDURATION", 1_500_000);
    assert_top("0 'PT1.5S' 2 ADDDURATION", 3_000_000);
    assert_top("0 'PT-1.5S' ADDDURATION", -1_500_000);
    assert_top("123 'PT0.000001S' ADDDURATION", 124);
    assert_top("-123 'PT0S' ADDDURATION", -123);
    // Months and years follow the calendar, in the given time zone.
    assert_top("1612051200000000 'P1M' ADDDURATION", 1_614_470_400_000_000);
    assert_top("1582934400000000 'P1Y' ADDDURATION", 1_614_470_400_000_000);
    assert_top(
        "1616808600000000 'Europe/Paris' 'P1D' ADDDURATION",
        1_616_895_000_000_000,
    );
    // Hours are exact: one day minus one hour across the spring transition.
    assert_top(
        "1616808600000000 'Europe/Paris' 'P1DT-1H' ADDDURATION",
        1_616_891_400_000_000,
    );
    // On a tselements list.
    assert_top(
        "[ 2021 1 31 12 0 0 ] 'P1M' ADDDURATION",
        longs(&[2021, 2, 28, 12, 0, 0, 0, 59, 7, 8]),
    );
    // A time zone requires a timestamp below it, like Warp 10.
    assert_fails(
        "[ 2021 1 31 ] 'Europe/Paris' 'PT1H' ADDDURATION",
        "ADDDURATION operates on a tselements list, timestamp, or timestamp and timezone.",
    );
    assert_top(
        "[ 2021 1 31 ] 'PT1H' ADDDURATION",
        longs(&[2021, 1, 31, 1, 0, 0, 0, 31, 7, 4]),
    );
    // Years and months are applied one after the other (Feb 29 + 1 year + 1 month).
    assert_top(
        "[ 2020 2 29 ] 'P1Y1M' ADDDURATION [ 0 2 ] SUBLIST",
        longs(&[2021, 3, 28]),
    );
    assert_fails(
        "0 'P1M' 9223372036854775807 ADDDURATION",
        "The calculation caused an overflow.",
    );
    assert_fails(
        "0 'P1D' 9223372036854775807 ADDDURATION",
        "The calculation caused an overflow.",
    );
    assert_fails(
        "0 'PT1H' 9223372036854775807 ADDDURATION",
        "The calculation caused an overflow.",
    );
    assert_fails(
        "0 'P1Y' 300000 ADDDURATION",
        "The calculation caused an overflow.",
    );
    assert_fails(
        "0 'PT1S' 9223372036854775807 ADDDURATION",
        "The calculation caused an overflow.",
    );
    for huge in [
        "P9223372036854775807Y",
        "P9223372036854775807W",
        "PT9223372036854775807H",
        "PT9223372036854775807M",
        "PT9223372036854775807S",
        "PT2000000000000H",
        "PT-9223372036854775807H",
    ] {
        assert_fails(
            &format!("0 '{huge}' ADDDURATION"),
            "The calculation caused an overflow.",
        );
    }
    assert_fails("9223372036854775807 'PT1S' ADDDURATION", "is out of range.");
    assert_fails("[ 2021 'x' ] 'PT1S' ADDDURATION", "cannot be cast");
    assert_fails(
        "[ 2021 ] 'P9223372036854775807Y' ADDDURATION",
        "The calculation caused an overflow.",
    );
    // Sub-millisecond offsets carry into the elements.
    assert_top(
        "[ 2021 1 1 0 0 0 999999 ] 'PT0.000002S' ADDDURATION 6 GET",
        1,
    );
    assert_fails(
        "0 'PT1.2.3S' ADDDURATION",
        "ADDDURATION encountered an exception. (Invalid ISO8601 duration)",
    );
    assert_fails(
        "0 'PT1.xS' ADDDURATION",
        "Parsing of sub second precision part of duration has failed. tried to parse: x",
    );
    assert_fails(
        "0 'PT' ADDDURATION",
        "Parsing of duration without sub second precision has failed. Tried to parse: PT",
    );
    assert_fails(
        "0 1.5 ADDDURATION",
        "ADDDURATION expects an ISO8601 duration (a string) on top of the stack (see http://en.wikipedia.org/wiki/ISO_8601#Durations), or a number of durations (a Long).",
    );
    assert_fails(
        "0 1 2 ADDDURATION",
        "ADDDURATION expects an ISO8601 duration (a string) in the second level of the stack (see http://en.wikipedia.org/wiki/ISO_8601#Durations).",
    );
    assert_fails(
        "'x' 'UTC' 'PT1S' ADDDURATION",
        "ADDDURATION operates on a tselements list, timestamp, or timestamp and timezone.",
    );
    assert_fails(
        "1.5 'PT1S' ADDDURATION",
        "ADDDURATION operates on a tselements list, timestamp, or timestamp and timezone.",
    );
    assert_fails("0 'Mars/Olympus' 'PT1S' ADDDURATION", "is not recognised");
    assert_fails("ADDDURATION", "Empty stack.");
    assert_fails("'PT1S' ADDDURATION", "Empty stack.");
    assert_fails("'UTC' 'PT1S' ADDDURATION", "Empty stack.");
    assert_fails("1 ADDDURATION", "Empty stack.");
    // `add_period` with a zone and a period containing everything.
    let mut period = Period::default();
    assert!(parse_period("P1Y1M1W1DT1H1M1S", &mut period) > 0);
    assert_eq!(
        add_period(0, &period, 500_000, Zone::for_id(None).unwrap(), 2).unwrap(),
        // 2 years, 2 months, 16 days (1972 is a leap year), 2 hours, 2 minutes, 2 seconds and
        // 1 second of offset
        ((((365 + 365 + 31 + 29 + 16) * 24 + 2) * 60 + 2) * 60 + 2) * 1_000_000 + 1_000_000,
    );
}

// -----------------------------------------------------------------------------------------
// TOTIMESTAMP / NOTBEFORE / NOTAFTER
// -----------------------------------------------------------------------------------------

#[test]
fn totimestamp_matches_java_time() {
    let cases = [
        ("2021-03-28T01:00:00Z", 1_616_893_200_000_000),
        ("2021-03-28T01:00:00.123456789Z", 1_616_893_200_123_456),
        ("2021-03-28T01:00Z", 1_616_893_200_000_000),
        ("2021-03-28T01:00:00+02:00", 1_616_886_000_000_000),
        ("2021-03-28T01:00:00-05:30", 1_616_913_000_000_000),
        ("2021-03-28T01:00:00+02:00:30", 1_616_885_970_000_000),
        ("2021-03-28T01:00:00+02", 1_616_886_000_000_000),
        ("2021-03-28T01:00:00.5Z", 1_616_893_200_500_000),
        ("2021-03-28T01:00:00.Z", 1_616_893_200_000_000),
        ("2021-03-28T01:00:00Z[Europe/Paris]", 1_616_893_200_000_000),
        (
            "2021-10-31T02:30:00+01:00[Europe/Paris]",
            1_635_643_800_000_000,
        ),
        (
            "2021-10-31T02:30:00+02:00[Europe/Paris]",
            1_635_640_200_000_000,
        ),
        (
            "2021-10-31T02:30:00+05:00[Europe/Paris]",
            1_635_629_400_000_000,
        ),
        (
            "2021-03-28T02:30:00+01:00[Europe/Paris]",
            1_616_895_000_000_000,
        ),
        ("2021-03-28T01:00:00Z[UTC]", 1_616_893_200_000_000),
        ("+10000-01-01T00:00:00Z", 253_402_300_800_000_000),
        ("-0001-01-01T00:00:00Z", -62_198_755_200_000_000),
        ("0000-01-01T00:00:00Z", -62_167_219_200_000_000),
        ("1969-12-31T23:59:59.999999Z", -1),
        ("2021-03-28T01:00:00z", 1_616_893_200_000_000),
        ("2021-03-28t01:00:00Z", 1_616_893_200_000_000),
        ("2021-03-28T01:00:00+18:00", 1_616_828_400_000_000),
    ];
    for (text, expected) in cases {
        assert_top(&format!("'{text}' TOTIMESTAMP"), expected);
        assert_eq!(parse_timestamp(text).unwrap(), expected, "{text}");
    }
    let errors = [
        (
            "2021-03-28T01:00:00+0200",
            "Text '2021-03-28T01:00:00+0200' could not be parsed, unparsed text found at index 22",
        ),
        (
            "2021-03-28T01:00:00",
            "Text '2021-03-28T01:00:00' could not be parsed at index 19",
        ),
        (
            "2021-03-28",
            "Text '2021-03-28' could not be parsed at index 10",
        ),
        (
            "2021-03-28T25:00:00Z",
            "Invalid value for HourOfDay (valid values 0 - 23): 25",
        ),
        ("2021-02-30T01:00:00Z", "Invalid date 'FEBRUARY 30'"),
        (
            "10000-01-01T00:00:00Z",
            "Text '10000-01-01T00:00:00Z' could not be parsed at index 0",
        ),
        (
            "2021-3-28T01:00:00Z",
            "Text '2021-3-28T01:00:00Z' could not be parsed at index 5",
        ),
        (
            "2021-03-28T01:00:60Z",
            "Invalid value for SecondOfMinute (valid values 0 - 59): 60",
        ),
        (
            "2021-03-28T01:60:00Z",
            "Invalid value for MinuteOfHour (valid values 0 - 59): 60",
        ),
        (
            "2021-13-28T01:00:00Z",
            "Invalid value for MonthOfYear (valid values 1 - 12): 13",
        ),
        (
            "2021-03-32T01:00:00Z",
            "Invalid value for DayOfMonth (valid values 1 - 28/31): 32",
        ),
        (
            "2021-03-28T01:00:00.1234567890Z",
            "Text '2021-03-28T01:00:00.1234567890Z' could not be parsed at index 29",
        ),
        (
            " 2021-03-28T01:00:00Z",
            "Text ' 2021-03-28T01:00:00Z' could not be parsed at index 0",
        ),
        (
            "2021-03-28T01:00:00Z ",
            "Text '2021-03-28T01:00:00Z ' could not be parsed, unparsed text found at index 20",
        ),
        (
            "2021-03-28T01:00:00+24:00",
            "Value out of range: Hour[0-23], Minute[0-59], Second[0-59]",
        ),
        (
            "2021-03-28T01:00:00+18:01",
            "Zone offset not in valid range: -18:00 to +18:00",
        ),
        (
            "2021-03-28T01:00:00Z[Mars/Olympus]",
            "Text '2021-03-28T01:00:00Z[Mars/Olympus]' could not be parsed, unparsed text found at index 20",
        ),
        ("", "Text '' could not be parsed at index 0"),
        ("+21", "Text '+21' could not be parsed at index 1"),
        ("2021", "Text '2021' could not be parsed at index 4"),
        ("2021-0", "Text '2021-0' could not be parsed at index 5"),
        ("2021-03", "Text '2021-03' could not be parsed at index 7"),
        (
            "2021-03-28T01",
            "Text '2021-03-28T01' could not be parsed at index 13",
        ),
        (
            "+99999999999999999999-01-01T00:00:00Z",
            "could not be parsed at index 1",
        ),
        ("2021-0x-28T01:00:00Z", "could not be parsed at index 5"),
        ("2021-03-2xT01:00:00Z", "could not be parsed at index 8"),
        ("2021-03-28T0x:00:00Z", "could not be parsed at index 11"),
        ("2021-03-28T01:0x:00Z", "could not be parsed at index 14"),
        ("2021-03-28T01:00:0xZ", "could not be parsed at index 17"),
        (
            "2021-03-28T01:00:00+0x:00",
            "could not be parsed at index 20",
        ),
        (
            "+99999999999-01-01T00:00:00Z",
            "could not be parsed at index 1",
        ),
        (
            "+262144-01-01T00:00:00Z",
            "Invalid value for Year (valid values -262143 - 262142): 262144",
        ),
    ];
    for (text, needle) in errors {
        let message = fail_msg(&format!("'{text}' TOTIMESTAMP"));
        assert!(
            message.contains("(Can only parse an ISO8601 timestamp (YYYY-MM-DDThh:mm:ss.sssssssss) with a specified time zone. (")
                && message.contains(needle),
            "{text}: {message}"
        );
    }
    assert_fails(
        "1 TOTIMESTAMP",
        "TOTIMESTAMP expects an ISO8601 timestamp or a Date instance on top of the stack.",
    );
    assert_fails("TOTIMESTAMP", "Empty stack.");
}

#[test]
fn not_before_and_not_after() {
    assert_top("0 NOTBEFORE 1", 1);
    assert_top("'1970-01-01T00:00:00Z' NOTBEFORE 1", 1);
    assert_top("4102444800000000 NOTAFTER 1", 1);
    assert_top("'2100-01-01T00:00:00Z' NOTAFTER 1", 1);
    assert_fails(
        "4102444800000000 NOTBEFORE",
        "Current time is before '4102444800000000'",
    );
    assert_fails(
        "'2100-01-01T00:00:00Z' NOTBEFORE",
        "Current time is before '2100-01-01T00:00:00Z'",
    );
    assert_fails("0 NOTAFTER", "Current time is after '0'");
    assert_fails(
        "'1970-01-01T00:00:00Z' NOTAFTER",
        "Current time is after '1970-01-01T00:00:00Z'",
    );
    assert_fails(
        "'nope' NOTBEFORE",
        "Can only parse an ISO8601 timestamp (YYYY-MM-DDThh:mm:ss.sssssssss) with a specified time zone.",
    );
    assert_fails(
        "1.5 NOTBEFORE",
        "NOTBEFORE expects a timestamp or ISO8601 datetime string on top of the stack.",
    );
    assert_fails(
        "1.5 NOTAFTER",
        "NOTAFTER expects a timestamp or ISO8601 datetime string on top of the stack.",
    );
    assert_fails("NOTBEFORE", "Empty stack.");
    assert_fails("NOTAFTER", "Empty stack.");
    let now = top("NOW").as_i64().unwrap();
    assert!(now <= current_time());
}

#[test]
fn zones() {
    assert_eq!(
        Zone::for_id(None).unwrap(),
        Zone::for_id(Some("UTC")).unwrap()
    );
    assert_eq!(
        Zone::for_id(Some("+00:00")).unwrap(),
        Zone::for_id(Some("UTC")).unwrap()
    );
    assert!(Zone::for_id(Some("Europe/Paris")).is_ok());
    assert!(Zone::for_id(Some("")).is_err());
    assert!(Zone::for_id(Some("+")).is_err());
    assert!(Zone::for_id(Some("+0")).is_err());
    assert!(Zone::for_id(Some("-23:59:59")).is_ok());
}
