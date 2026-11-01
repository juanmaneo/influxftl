#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::format::{
    is_double, is_long, java_double_to_string, parse_double, parse_long, parse_radix_wrapping,
    snapshot_encode_readable, url_decode, url_encode,
};

#[test]
fn readable_encoding_only_escapes_quotes_percents_and_controls() {
    assert_eq!(snapshot_encode_readable("a b+c/dé"), "a b+c/dé");
    assert_eq!(
        snapshot_encode_readable("it's 100%\n\t"),
        "it%27s 100%25%0a%09"
    );
}

#[test]
fn double_to_string_matches_java() {
    let cases: &[(f64, &str)] = &[
        (0.0, "0.0"),
        (-0.0, "-0.0"),
        (1.0, "1.0"),
        (-1.5, "-1.5"),
        (100.0, "100.0"),
        (123_456.789, "123456.789"),
        (0.001, "0.001"),
        (0.000_1, "1.0E-4"),
        (1e7, "1.0E7"),
        (9_999_999.0, "9999999.0"),
        (12_345_678.9, "1.23456789E7"),
        (1e-7, "1.0E-7"),
        (1.5e300, "1.5E300"),
        (f64::NAN, "NaN"),
        (f64::INFINITY, "Infinity"),
        (f64::NEG_INFINITY, "-Infinity"),
        (0.1 + 0.2, "0.30000000000000004"),
        (1e21, "1.0E21"),
        (123.0, "123.0"),
        (0.5, "0.5"),
    ];
    for (value, expected) in cases {
        assert_eq!(java_double_to_string(*value), *expected, "{value}");
    }
}

#[test]
fn long_detection() {
    assert!(is_long("0"));
    assert!(is_long("-12"));
    assert!(is_long("+7"));
    assert!(is_long("  42"));
    assert!(!is_long(""));
    assert!(!is_long("   "));
    assert!(!is_long("-"));
    assert!(!is_long("1.0"));
    assert!(!is_long("12a"));
    assert!(!is_long("abc"));
}

#[test]
fn double_detection() {
    assert!(is_double("1.0"));
    assert!(is_double("-1.5"));
    assert!(is_double("+0.5"));
    assert!(is_double("1e3"));
    assert!(is_double("1E-3"));
    assert!(is_double("1.5e+10"));
    assert!(is_double("NaN"));
    assert!(is_double("-Infinity"));
    assert!(is_double("  2.5"));
    assert!(!is_double(""));
    assert!(!is_double("  "));
    assert!(!is_double("-"));
    assert!(!is_double("42"));
    assert!(!is_double(".5"));
    assert!(!is_double("1.2.3"));
    assert!(!is_double("1e2e3"));
    assert!(!is_double("1e+2e3"));
    assert!(!is_double("1e+2+3"));
    assert!(!is_double("1e+-2"));
    assert!(!is_double("1+2"));
    assert!(!is_double("1e2.0"));
    assert!(!is_double("1e+2.0"));
    assert!(!is_double("1.0x"));
    assert!(!is_double("Infinite"));
}

#[test]
fn parsing_numbers() {
    assert_eq!(parse_long("42").unwrap(), 42);
    assert_eq!(parse_long("+42").unwrap(), 42);
    assert_eq!(parse_long("  -42").unwrap(), -42);
    assert_eq!(
        parse_long("99999999999999999999").unwrap_err().to_string(),
        "For input string: \"99999999999999999999\""
    );
    assert_eq!(parse_double("1.5").unwrap(), 1.5);
    assert_eq!(parse_double("+1.5").unwrap(), 1.5);
    assert_eq!(parse_double(" -1.5e2").unwrap(), -150.0);
    assert!(parse_double("NaN").unwrap().is_nan());
    assert_eq!(parse_double("-Infinity").unwrap(), f64::NEG_INFINITY);
    assert_eq!(parse_double("Infinity").unwrap(), f64::INFINITY);
    assert!(
        parse_double("1.5.5")
            .unwrap_err()
            .to_string()
            .contains("For input string")
    );
}

#[test]
fn radix_parsing_wraps_like_biginteger() {
    assert_eq!(parse_radix_wrapping("ff", 16).unwrap(), 255);
    assert_eq!(parse_radix_wrapping("-ff", 16).unwrap(), -255);
    assert_eq!(parse_radix_wrapping("+101", 2).unwrap(), 5);
    assert_eq!(parse_radix_wrapping("ffffffffffffffff", 16).unwrap(), -1);
    assert_eq!(
        parse_radix_wrapping("1ffffffffffffffff", 16).unwrap(),
        -1,
        "17 hex digits wrap"
    );
    assert!(parse_radix_wrapping("", 16).is_err());
    assert!(parse_radix_wrapping("-", 16).is_err());
    assert!(parse_radix_wrapping("fg", 16).is_err());
    assert!(parse_radix_wrapping("102", 2).is_err());
}

#[test]
fn url_encoding() {
    assert_eq!(url_encode("abc-_.*09"), "abc-_.*09");
    assert_eq!(url_encode("a b+c/d"), "a%20b%2Bc%2Fd");
    assert_eq!(url_encode("é"), "%C3%A9");
    assert_eq!(url_encode("'"), "%27");
}

#[test]
fn url_decoding() {
    assert_eq!(url_decode("abc").unwrap(), "abc");
    assert_eq!(url_decode("a+b").unwrap(), "a+b");
    assert_eq!(url_decode("a%20b%2Bc").unwrap(), "a b+c");
    assert_eq!(url_decode("%C3%A9").unwrap(), "é");
    assert_eq!(url_decode("%FF").unwrap(), "\u{FFFD}");
    assert_eq!(
        url_decode("ab%2").unwrap_err().to_string(),
        "URLDecoder: Incomplete trailing escape (%) pattern"
    );
    assert_eq!(
        url_decode("%").unwrap_err().to_string(),
        "URLDecoder: Incomplete trailing escape (%) pattern"
    );
    assert!(
        url_decode("%zz")
            .unwrap_err()
            .to_string()
            .contains("Illegal hex characters in escape (%) pattern")
    );
}
