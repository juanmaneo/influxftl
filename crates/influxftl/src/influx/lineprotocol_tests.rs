// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of line protocol.

use indexmap::IndexMap;
use warpscript::Value;
use warpscript::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION};

use super::{
    InfluxError, escape, measurement, meta_line, series_key, series_lines, string_field, tag,
};

fn labels(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

fn series(pairs: &[(&str, &str)], points: &[(i64, i64, i64, Value)]) -> GeoTimeSerie {
    let mut gts = GeoTimeSerie::new();
    gts.metadata.labels = labels(pairs);
    for (tick, location, elevation, value) in points {
        gts.set_value(*tick, *location, *elevation, value, false);
    }
    gts
}

#[test]
fn the_special_characters_are_escaped() {
    assert_eq!(escape("a b,c=d\\e", &[',', ' ']), "a\\ b\\,c=d\\\\e");
    assert_eq!(tag("a b,c=d").expect("a tag"), "a\\ b\\,c\\=d");
    assert_eq!(string_field("say \"hi\"\\"), "\"say \\\"hi\\\"\\\\\"");
    assert_eq!(string_field("a\nb"), "\"a\nb\"");
}

#[test]
fn a_name_or_a_value_without_a_way_to_write_it_is_refused() {
    for text in ["", "a\nb", "a\rb"] {
        assert!(
            matches!(tag(text), Err(InfluxError::Unencodable(_))),
            "{text:?}"
        );
        assert!(
            matches!(measurement(text), Err(InfluxError::Unencodable(_))),
            "{text:?}"
        );
    }
    assert!(matches!(
        measurement("#comment"),
        Err(InfluxError::Unencodable(_))
    ));
    assert_eq!(measurement("a#b").expect("a measurement"), "a#b");
    assert_eq!(
        measurement("a b,c=d").expect("a measurement"),
        "a\\ b\\,c=d"
    );
}

#[test]
fn the_table_of_the_attributes_is_not_a_class() {
    let err = measurement("_influxftl_series").expect_err("the table of the attributes");
    assert!(matches!(&err, InfluxError::ReservedClass(class) if class == "_influxftl_series"));
    assert!(err.to_string().contains("table of the attributes"), "{err}");
}

#[test]
fn a_series_key_sorts_its_tags() {
    let labels = labels(&[("zone", "b c"), ("host", "a,b")]);
    assert_eq!(
        series_key("cpu load", &labels).expect("a key"),
        "cpu\\ load,host=a\\,b,zone=b\\ c"
    );
    assert_eq!(series_key("cpu", &IndexMap::new()).expect("a key"), "cpu");
}

#[test]
fn a_series_key_refuses_what_cannot_be_stored() {
    for name in [
        "time",
        "value_type",
        "value_l",
        "value_d",
        "value_b",
        "value_s",
        "hhcode",
        "elevation",
    ] {
        let err = series_key("cpu", &labels(&[(name, "x")])).expect_err("a reserved label");
        assert!(
            matches!(&err, InfluxError::ReservedLabel(label) if label == name),
            "{err}"
        );
    }
    let err = series_key("cpu", &labels(&[("host", "")])).expect_err("an empty value");
    assert!(matches!(&err, InfluxError::EmptyLabelValue(label) if label == "host"));
    assert!(matches!(
        series_key("cpu", &labels(&[("a\nb", "x")])),
        Err(InfluxError::Unencodable(_))
    ));
    assert!(matches!(
        series_key("cpu", &labels(&[("a", "x\ny")])),
        Err(InfluxError::Unencodable(_))
    ));
    assert!(matches!(
        series_key("", &labels(&[])),
        Err(InfluxError::Unencodable(_))
    ));
}

#[test]
fn the_datapoints_of_every_type_are_lines() {
    let location = 0x1234;
    let gts = series(&[("h", "a")], &[(2, location, 7, Value::Long(-5))]);
    assert_eq!(
        series_lines("c", &gts).expect("lines"),
        [format!(
            "c,h=a value_type=\"l\",value_l=-5i,hhcode={location}i,elevation=7i 2000"
        )]
    );
    let gts = series(
        &[],
        &[
            (1, NO_LOCATION, NO_ELEVATION, Value::Double(1.5)),
            (2, NO_LOCATION, NO_ELEVATION, Value::Double(1e21)),
            (3, NO_LOCATION, NO_ELEVATION, Value::Double(-0.0)),
        ],
    );
    let lines = series_lines("c", &gts).expect("lines");
    assert_eq!(lines.len(), 3);
    assert!(
        lines[0].starts_with("c value_type=\"d\",value_d=1.5,hhcode="),
        "{lines:?}"
    );
    assert!(lines[1].contains("value_d=1e21,"), "{lines:?}");
    assert!(lines[2].contains("value_d=-0.0,"), "{lines:?}");
    let gts = series(
        &[],
        &[(-1, NO_LOCATION, NO_ELEVATION, Value::Boolean(true))],
    );
    assert_eq!(
        series_lines("c", &gts).expect("lines"),
        [format!(
            "c value_type=\"b\",value_b=true,hhcode={NO_LOCATION}i,elevation={NO_ELEVATION}i -1000"
        )]
    );
    let gts = series(
        &[],
        &[(0, NO_LOCATION, NO_ELEVATION, Value::from("a \"b\"\n"))],
    );
    assert!(
        series_lines("c", &gts).expect("lines")[0]
            .starts_with("c value_type=\"s\",value_s=\"a \\\"b\\\"\n\","),
    );
}

#[test]
fn a_series_without_datapoints_has_no_line() {
    assert_eq!(
        series_lines("c", &GeoTimeSerie::new())
            .expect("no lines")
            .len(),
        0
    );
}

#[test]
fn what_cannot_be_written_is_refused() {
    for double in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let gts = series(
            &[],
            &[(1, NO_LOCATION, NO_ELEVATION, Value::Double(double))],
        );
        assert!(
            matches!(series_lines("c", &gts), Err(InfluxError::NotFinite(_))),
            "{double}"
        );
    }
    let gts = series(
        &[],
        &[(i64::MAX, NO_LOCATION, NO_ELEVATION, Value::Long(1))],
    );
    assert!(matches!(
        series_lines("c", &gts),
        Err(InfluxError::TickOutOfRange(i64::MAX))
    ));
    let gts = series(
        &[("time", "x")],
        &[(1, NO_LOCATION, NO_ELEVATION, Value::Long(1))],
    );
    assert!(matches!(
        series_lines("c", &gts),
        Err(InfluxError::ReservedLabel(_))
    ));
}

#[test]
fn the_attributes_of_a_series_are_a_line() {
    assert_eq!(
        meta_line(
            "cpu",
            "{\"host\":\"a b\"}",
            "{\"k\":\"v \\\"q\\\"\"}",
            false,
            42
        )
        .expect("a line"),
        "_influxftl_series,class=cpu,labels={\"host\":\"a\\ b\"} attributes=\"{\\\"k\\\":\\\"v \\\\\\\"q\\\\\\\"\\\"}\",deleted=false 42"
    );
    assert_eq!(
        meta_line("cpu", "{}", "{}", true, 1).expect("a line"),
        "_influxftl_series,class=cpu,labels={} attributes=\"{}\",deleted=true 1"
    );
}

#[test]
fn the_line_of_attributes_refuses_what_cannot_be_written() {
    assert!(matches!(
        meta_line("", "{}", "{}", false, 1),
        Err(InfluxError::Unencodable(_))
    ));
    assert!(matches!(
        meta_line("c", "", "{}", false, 1),
        Err(InfluxError::Unencodable(_))
    ));
}
