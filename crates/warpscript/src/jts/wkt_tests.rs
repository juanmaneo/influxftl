// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! What the oracle tables of `GEO.WKT` and `->WKT` cannot say: the failures on numbers that
//! are not finite, the depth limit, the line numbers and the arithmetic of the number format.

use super::geometry::{Coord, Geometry, Line};
use super::wkt::{format_number, increment, parse_number, read, write};
use crate::error::fault::ResourceError;
use crate::error::geometry::{GeometryError, NumberError};
use crate::error::{Error, Result};
use crate::finite::NotFinite;
use crate::tests::on_big_stack;

fn at(x: f64, y: f64) -> Coord {
    Coord::new(x, y, None).expect("finite coordinate")
}

fn parse_error(text: &str) -> GeometryError {
    match read(text) {
        Err(Error::Geometry(err)) => err,
        other => panic!("{text:?} should fail to parse, got {other:?}"),
    }
}

#[test]
fn numbers_that_are_not_finite_are_refused() {
    for (text, error) in [
        ("POINT (NaN 1)", NotFinite::Nan),
        ("POINT (nan 1)", NotFinite::Nan),
        ("POINT (-NaN 1)", NotFinite::Nan),
        ("POINT (+NaN 1)", NotFinite::Nan),
        ("POINT (1 NaN)", NotFinite::Nan),
        ("POINT (1 2 NaN)", NotFinite::Nan),
        ("POINT (Infinity 1)", NotFinite::PositiveInfinity),
        ("POINT (+Infinity 1)", NotFinite::PositiveInfinity),
        ("POINT (1 Infinity)", NotFinite::PositiveInfinity),
        ("POINT (1e999 1)", NotFinite::PositiveInfinity),
        ("POINT (-Infinity 1)", NotFinite::NegativeInfinity),
        ("POINT (1 -Infinity)", NotFinite::NegativeInfinity),
        ("POINT (1 2 -Infinity)", NotFinite::NegativeInfinity),
        ("POINT (-1e999 1)", NotFinite::NegativeInfinity),
        ("LINESTRING (1 2, 3 Infinity)", NotFinite::PositiveInfinity),
        ("MULTIPOINT (1 2, NaN 4)", NotFinite::Nan),
    ] {
        assert_eq!(read(text), Err(Error::NotFinite(error)), "{text}");
    }
}

#[test]
fn only_the_decimal_forms_are_numbers() {
    for (word, expected) in [
        ("1", 1.0),
        ("-1.5", -1.5),
        ("+.5", 0.5),
        ("5.", 5.0),
        ("1e3", 1000.0),
        ("1E-3", 0.001),
        ("2.5f", 2.5),
        ("2.5D", 2.5),
    ] {
        assert_eq!(
            parse_number(word).expect("a decimal number"),
            expected,
            "{word}"
        );
    }
    for word in [
        "", "-", "e5", "0x10", "1_0", "abc", "1 2", "--1", "+-1", "inf", "-nan",
    ] {
        assert!(
            matches!(parse_number(word), Err(NumberError::NotDecimal)),
            "{word}"
        );
    }
    for word in [".", "1e", "1e+", "1.2.3"] {
        assert!(
            matches!(parse_number(word), Err(NumberError::Parse(_))),
            "{word}"
        );
    }
    assert!(parse_number("NaN").expect("NaN").is_nan());
    assert_eq!(
        parse_number("-Infinity").expect("-Infinity"),
        f64::NEG_INFINITY
    );
}

#[test]
fn a_text_without_a_word_is_no_geometry() {
    for text in ["", " \t\r\n", ";", "# only a comment", "(", ")", ",", "1 2"] {
        match read(text) {
            Ok(None) | Err(Error::Geometry(GeometryError::UnknownType { .. })) => {}
            other => panic!("{text:?}: {other:?}"),
        }
    }
    assert_eq!(read(""), Ok(None));
    assert_eq!(read(";"), Ok(None));
    assert_eq!(read("# only a comment"), Ok(None));
}

#[test]
fn the_grammar_words_are_not_geometry_types() {
    for (text, name) in [
        ("EMPTY", "EMPTY"),
        ("empty", "EMPTY"),
        (",", ","),
        ("(", "("),
        (")", ")"),
    ] {
        assert_eq!(
            parse_error(text),
            GeometryError::UnknownType {
                name: name.to_owned(),
                line: 1
            },
            "{text}"
        );
    }
}

#[test]
fn lines_and_comments_are_counted_like_a_stream_tokenizer() {
    let point = Geometry::Point(Some(at(1.0, 2.0)));
    for text in [
        "POINT\r(1\r2)",
        "POINT\r\n(1\r\n2)",
        "POINT\n(1\n2)",
        "# comment\nPOINT (1 2)",
        "# comment\r\nPOINT (1 2)",
        "POINT # c\r\n(1 2) # tail",
    ] {
        assert_eq!(read(text), Ok(Some(point.clone())), "{text:?}");
    }
    assert_eq!(
        parse_error("POINT\r\n(1\r2\r\nx)"),
        GeometryError::InvalidNumber {
            text: "x".to_owned(),
            line: 4,
            source: NumberError::NotDecimal,
        }
    );
    assert_eq!(
        parse_error("# c\r\n# d\nFOO (1 2)"),
        GeometryError::UnknownType {
            name: "FOO".to_owned(),
            line: 3
        }
    );
    assert_eq!(
        parse_error("POINT (1 2"),
        GeometryError::Expected {
            expected: "word",
            found: "End-of-Stream".to_owned(),
            line: 1
        }
    );
    assert_eq!(
        parse_error("POINT (1 ;)"),
        GeometryError::Expected {
            expected: "number",
            found: "';'".to_owned(),
            line: 1
        }
    );
}

#[test]
fn every_part_of_the_grammar_reports_what_it_expected() {
    for text in [
        "POLYGON x",
        "POLYGON (",
        "POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0), x)",
        "POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0) x)",
        "MULTIPOINT (",
        "MULTIPOINT (;",
        "MULTIPOINT ((1 2), x)",
        "MULTIPOINT ((x))",
        "MULTIPOINT (1 2, x)",
        "MULTILINESTRING x",
        "MULTILINESTRING ((1 2, 3 4) x)",
        "MULTILINESTRING (x)",
        "MULTIPOLYGON x",
        "MULTIPOLYGON (x)",
        "MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0)) x)",
        "GEOMETRYCOLLECTION x",
        "GEOMETRYCOLLECTION (POINT (1 2) x)",
        "LINESTRING (1 2, x)",
        "LINEARRING (0 0, 1 1, x)",
        "POINT (1 2 x)",
    ] {
        assert!(
            matches!(read(text), Err(Error::Geometry(_))),
            "{text}: {:?}",
            read(text)
        );
    }
}

#[test]
fn a_collection_needs_every_element_to_be_a_geometry() {
    assert_eq!(
        read("GEOMETRYCOLLECTION (;)"),
        Err(GeometryError::NullElement.into())
    );
    assert_eq!(
        read("GEOMETRYCOLLECTION (POINT (1 2), ;)"),
        Err(GeometryError::NullElement.into())
    );
}

fn nested(depth: usize) -> String {
    format!(
        "{}POINT (1 2){}",
        "GEOMETRYCOLLECTION (".repeat(depth),
        ")".repeat(depth)
    )
}

fn the_depth_of_collections_is_limited() {
    assert!(matches!(
        read(&nested(512)),
        Ok(Some(Geometry::Collection(_)))
    ));
    assert_eq!(read(&nested(513)), Err(ResourceError::StackOverflow.into()));
}

#[test]
fn deep_collections_end_in_a_stack_overflow() {
    on_big_stack(the_depth_of_collections_is_limited);
}

#[test]
fn numbers_are_written_with_at_most_16_fraction_digits() {
    for (value, expected) in [
        (0.0, "0"),
        (-0.0, "-0"),
        (1.0, "1"),
        (-1.5, "-1.5"),
        (0.5, "0.5"),
        (100.0, "100"),
        (1e21, "1000000000000000000000"),
        (1.5e-7, "0.00000015"),
        (123_456_789.123_456_79, "123456789.12345679"),
        (0.1 + 0.2, "0.3"),
        (1e-20, "0"),
        (5e-17, "0"),
        (6e-17, "0.0000000000000001"),
        (0.123_456_789_012_345_67, "0.1234567890123457"),
        (0.999_999_999_999_999_9, "0.9999999999999999"),
        (0.000_099_999_999_999_999_99, "0.0001"),
        // Exactly half of the last digit kept: the even digit wins, as `HALF_EVEN` says.
        (0.000_007_629_394_531_25, "0.0000076293945312"),
        (0.000_022_888_183_593_750, "0.0000228881835938"),
    ] {
        assert_eq!(format_number(value), expected, "{value:e}");
    }
}

#[test]
fn one_unit_carries_through_the_nines() {
    let carry = |whole: &str, fraction: &str| {
        let mut whole = whole.as_bytes().to_vec();
        let mut fraction = fraction.as_bytes().to_vec();
        increment(&mut whole, &mut fraction);
        (
            String::from_utf8(whole).expect("digits"),
            String::from_utf8(fraction).expect("digits"),
        )
    };
    assert_eq!(carry("0", "123"), ("0".to_owned(), "124".to_owned()));
    assert_eq!(carry("0", "199"), ("0".to_owned(), "200".to_owned()));
    assert_eq!(carry("0", "99"), ("1".to_owned(), "00".to_owned()));
    assert_eq!(carry("19", "9"), ("20".to_owned(), "0".to_owned()));
    assert_eq!(carry("9", "9"), ("10".to_owned(), "0".to_owned()));
}

#[test]
fn a_multi_point_with_an_empty_point_has_no_text() {
    let points = Geometry::MultiPoint(vec![Some(at(1.0, 2.0)), None]);
    assert_eq!(write(&points), Err(Error::NullPointer));
    let wrapped = Geometry::Collection(vec![points]);
    assert_eq!(write(&wrapped), Err(Error::NullPointer));
    let all_empty = Geometry::MultiPoint(vec![None, None]);
    assert_eq!(write(&all_empty).as_deref(), Ok("MULTIPOINT EMPTY"));
}

#[test]
fn geometries_are_written_as_jts_writes_them() {
    let line = Line::new(vec![at(1.0, 2.0), at(3.0, 4.5)]).expect("line");
    let texts: Vec<Result<String>> = vec![
        write(&Geometry::LineString(line.clone())),
        write(&Geometry::MultiLineString(vec![line.clone(), line])),
        write(&Geometry::Collection(Vec::new())),
        write(&Geometry::Point(None)),
    ];
    assert_eq!(
        texts,
        vec![
            Ok("LINESTRING (1 2, 3 4.5)".to_owned()),
            Ok("MULTILINESTRING ((1 2, 3 4.5), (1 2, 3 4.5))".to_owned()),
            Ok("GEOMETRYCOLLECTION EMPTY".to_owned()),
            Ok("POINT EMPTY".to_owned()),
        ]
    );
}
