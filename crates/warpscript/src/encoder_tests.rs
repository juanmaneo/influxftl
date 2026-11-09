// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::format_collect
)]

use std::rc::Rc;

use crate::encoder::{
    DEFAULT_COMP_RATIO_THRESHOLD, Decimal, EncodedValue, EncoderError, GtsDecoder, GtsEncoder,
    encoder_to_wrapper, gts_to_wrapper, wrapper_to_encoder, wrapper_to_gts,
};
use crate::error::Error;
use crate::geoxp::to_geoxp_point;
use crate::gts::{GeoTimeSerie, Metadata, NO_ELEVATION, NO_LOCATION};
use crate::thrift::{GtsWrapper, deserialize_wrapper, serialize_wrapper};
use crate::thrift_tests::hex;
use crate::value::Value;

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn dec(unscaled: i64, scale: i32) -> Decimal {
    Decimal { unscaled, scale }
}

fn series(points: &[(i64, i64, i64, Value)]) -> GeoTimeSerie {
    let mut gts = GeoTimeSerie::new();
    for (tick, location, elevation, value) in points {
        gts.set_value(*tick, *location, *elevation, value, false);
    }
    gts
}

fn simple(points: &[(i64, Value)]) -> GeoTimeSerie {
    let points: Vec<_> = points
        .iter()
        .map(|(t, v)| (*t, NO_LOCATION, NO_ELEVATION, v.clone()))
        .collect();
    series(&points)
}

fn wrap_hex(gts: &GeoTimeSerie, optimized: bool) -> String {
    let compratio = if optimized {
        1.0
    } else {
        DEFAULT_COMP_RATIO_THRESHOLD
    };
    to_hex(&serialize_wrapper(&gts_to_wrapper(
        gts, true, compratio, optimized, true,
    )))
}

/// Every datapoint of a decoder.
fn decode_all(decoder: &mut GtsDecoder) -> Vec<(i64, i64, i64, EncodedValue)> {
    let mut out = Vec::new();
    while decoder.next().unwrap() {
        out.push((
            decoder.timestamp(),
            decoder.location(),
            decoder.elevation(),
            decoder.binary_value(),
        ));
    }
    out
}

fn decoder(bytes: &str) -> GtsDecoder {
    GtsDecoder::new(0, Rc::from(hex(bytes)))
}

fn decode_err(bytes: &str) -> (String, &'static str) {
    let mut d = decoder(bytes);
    loop {
        match d.next() {
            Ok(true) => {}
            Ok(false) => panic!("no error for {bytes}"),
            Err(e) => return (e.message(), crate::error::compat::exception_class(&e)),
        }
    }
}

#[test]
fn decimals() {
    assert_eq!(dec(1200, 2).strip(), dec(12, 0));
    assert_eq!(dec(0, 5).strip(), dec(0, 0));
    assert_eq!(Decimal::from_double(1.5), dec(15, 1));
    assert_eq!(Decimal::from_double(-0.0), dec(0, 0));
    assert_eq!(Decimal::from_double(1.0e20), dec(1, -20));
    assert_eq!(Decimal::from_double(-1.25e-20), dec(-125, 22));
    assert_eq!(Decimal::from_double(100.0), dec(1, -2));
    assert_eq!(dec(125, 2).double_value(), 1.25);
    assert_eq!(dec(1, -20).double_value(), 1.0e20);
    assert_eq!(dec(-15, 1).long_value(), -1);
    assert_eq!(dec(7, -2).long_value(), 700);
    assert_eq!(dec(7, -70).long_value(), 0);
    assert_eq!(dec(i64::MAX, 19).long_value(), 0);
    assert_eq!(dec(0, 0).bit_length(), 0);
    assert_eq!(dec(-1, 0).bit_length(), 0);
    assert_eq!(dec(-129, 0).bit_length(), 8);
    assert_eq!(dec(1 << 46, 0).bit_length(), 47);
    for (d, text) in [
        (dec(42, 0), "42"),
        (dec(125, 2), "1.25"),
        (dec(-125, 2), "-1.25"),
        (dec(5, 3), "0.005"),
        (dec(5, 7), "5E-7"),
        (dec(15, 8), "1.5E-7"),
        (dec(1, -3), "1E+3"),
        (dec(-123, -1), "-1.23E+3"),
        (dec(0, -2), "0E+2"),
        (dec(123, 3), "0.123"),
    ] {
        assert_eq!(d.to_string(), text);
    }
}

#[test]
fn values() {
    let list = Value::list(vec![]);
    let e = EncodedValue::from_value(&list).unwrap_err();
    assert_eq!(
        e.message(),
        "Unsuported value type 'class java.util.ArrayList'"
    );
    assert_eq!(
        crate::error::compat::exception_class(&e),
        "java.lang.RuntimeException"
    );
    for v in [
        Value::Null,
        Value::Boolean(true),
        Value::Long(3),
        Value::Double(2.5),
        Value::from("s"),
        Value::bytes([1u8, 2]),
    ] {
        assert_eq!(EncodedValue::from_value(&v).unwrap().to_value(), v);
    }
    assert_eq!(
        EncodedValue::Decimal(dec(25, 1)).to_value(),
        Value::Double(2.5)
    );
}

#[test]
fn oracle_wraps() {
    let longs = simple(&[
        (1, Value::Long(42)),
        (2, Value::Long(42)),
        (3, Value::Long(43)),
        (100, Value::Long(-5)),
    ]);
    assert_eq!(
        wrap_hex(&longs, false),
        "780c4c025409020e02020cc20109160800"
    );
    let doubles = simple(&[
        (1, Value::Double(1.5)),
        (2, Value::Double(1.5)),
        (3, Value::Double(f64::NAN)),
        (4, Value::Double(f64::NAN)),
    ]);
    assert_eq!(
        wrap_hex(&doubles, false),
        "781854023ff8000000000000110214027ff80000000000001102160800"
    );
    let opt = simple(&[
        (1, Value::Double(1.5)),
        (2, Value::Double(1.5)),
        (3, Value::Double(0.1)),
        (4, Value::Double(1e300)),
    ]);
    assert_eq!(
        wrap_hex(&opt, true),
        "78145002011e11021002010214027e37e43c8800759c160800"
    );
    let opt = simple(&[
        (1, Value::Double(1.25)),
        (2, Value::Double(1.0e20)),
        (3, Value::Double(1.0e-20)),
        (4, Value::Double(0.0)),
        (5, Value::Double(-0.0)),
    ]);
    assert_eq!(
        wrap_hex(&opt, true),
        "7813500202fa011002ec0210021402100200001102160a00"
    );
    let strings = simple(&[
        (1, Value::from("a")),
        (2, Value::from("a")),
        (3, Value::from("\u{c3}\u{a9}")),
    ]);
    assert_eq!(
        wrap_hex(&strings, false),
        "780d580201611902180204c383c2a9160600"
    );
    let booleans = simple(&[(1, Value::Boolean(true)), (2, Value::Boolean(false))]);
    assert_eq!(wrap_hex(&booleans, false), "780444020202160400");
    let paris = to_geoxp_point(48.0, 2.0);
    let geo = series(&[
        (1, paris, 100, Value::Long(1)),
        (2, paris, 100, Value::Long(1)),
        (3, to_geoxp_point(48.1, 2.0), -100, Value::Long(1)),
        (4, NO_LOCATION, 5, Value::Long(1)),
    ]);
    assert_eq!(
        wrap_hex(&geo, false),
        "7822cc4c02e021347021347021c80102895902896e02c4a784888d98048f03890e02d201160800"
    );
    let mut bucketized = simple(&[(1, Value::Long(2)), (10, Value::Long(1))]);
    bucketized.lastbucket = 10;
    bucketized.bucketspan = 1;
    bucketized.bucketcount = 10;
    bucketized.sort(true);
    assert_eq!(
        wrap_hex(&bucketized, false),
        "26141602161438064c14024e0202160400"
    );
}

#[test]
fn oracle_unwraps() {
    // A named series whose content Warp 10 gzipped.
    let bytes = hex(concat!(
        "1c18016e1b018801610162160016001b000068271f8b0800000000000000f369e8ac62f2fbdf51c594",
        "53ff1f0d30fa21988c00e4a36f102900000016081100"
    ));
    let wrapper = deserialize_wrapper(&bytes).unwrap();
    let gts = wrapper_to_gts(&wrapper, false).unwrap();
    assert_eq!(gts.name(), Some("n"));
    assert_eq!(gts.ticks, vec![1_000_000, -1_000_000, i64::MAX, i64::MIN]);
    assert_eq!(gts.value_at(2), Value::Long(i64::MIN));
    assert_eq!(gts.value_at(3), Value::Long(i64::MAX));
    // Optimized doubles come back as doubles.
    let wrapper = gts_to_wrapper(
        &simple(&[
            (1, Value::Double(1.25)),
            (2, Value::Double(1.0e20)),
            (3, Value::Double(1.0e-20)),
            (4, Value::Double(123_456_789.125)),
        ]),
        false,
        1.0,
        true,
        true,
    );
    let gts = wrapper_to_gts(&wrapper, false).unwrap();
    assert_eq!(gts.value_at(3), Value::Double(123_456_789.125));
    let wrapper = deserialize_wrapper(&hex(
        "781b1f8b08000000000000008b606a674c1c1c000026dc61218b00000016021100",
    ))
    .unwrap();
    let gts = wrapper_to_gts(&wrapper, false).unwrap();
    assert_eq!(gts.value_at(0), Value::from("a".repeat(135)));
}

#[test]
fn decimal_conversions() {
    // `BigDecimal` values stored into series of every type.
    let mut gts = GeoTimeSerie::new();
    gts.set_decimal(1, NO_LOCATION, NO_ELEVATION, dec(25, 1));
    assert_eq!(gts.value_at(0), Value::Double(2.5));
    let mut longs = simple(&[(0, Value::Long(1))]);
    longs.set_decimal(1, NO_LOCATION, NO_ELEVATION, dec(25, 1));
    assert_eq!(longs.value_at(1), Value::Long(2));
    let mut strings = simple(&[(0, Value::from("x"))]);
    strings.set_decimal(1, NO_LOCATION, NO_ELEVATION, dec(25, 8));
    assert_eq!(strings.value_at(1), Value::from("2.5E-7"));
    let mut booleans = simple(&[(0, Value::Boolean(false))]);
    booleans.set_decimal(1, NO_LOCATION, NO_ELEVATION, dec(25, 1));
    booleans.set_decimal(2, NO_LOCATION, NO_ELEVATION, dec(0, 1));
    assert_eq!(booleans.value_at(1), Value::Boolean(true));
    assert_eq!(booleans.value_at(2), Value::Boolean(false));
}

#[test]
fn encode_decode() {
    let big = 1i64 << 50;
    let points = vec![
        (0, NO_LOCATION, NO_ELEVATION, EncodedValue::Long(1)), // equals base
        (big, 7, 1, EncodedValue::Long(big)),                  // raw everything
        (big + 1, 7, 1, EncodedValue::Long(big + 1)),          // identical loc/elev, delta long
        (big + 2, 9, 3, EncodedValue::Long(big + 1)),          // zigzag deltas, identical long
        (-big, big * 4, big * 4, EncodedValue::Long(-big)),    // raw again
        (-big + 1, -big * 4, -big * 4, EncodedValue::Null),    // delete marker, no location
        (2, 1, NO_ELEVATION, EncodedValue::Boolean(false)),
        (3, NO_LOCATION, 5, EncodedValue::Boolean(true)),
        (4, NO_LOCATION, 6, EncodedValue::String(Rc::from("é"))),
        (5, NO_LOCATION, big * 4, EncodedValue::String(Rc::from("é"))),
        (
            6,
            NO_LOCATION,
            1,
            EncodedValue::Bytes(Rc::from(vec![0xe9u8])),
        ),
        (
            7,
            NO_LOCATION,
            NO_ELEVATION,
            EncodedValue::Bytes(Rc::from(vec![0x41u8])),
        ),
        (
            7,
            NO_LOCATION,
            NO_ELEVATION,
            EncodedValue::Bytes(Rc::from(vec![0x41u8])),
        ),
        (8, NO_LOCATION, NO_ELEVATION, EncodedValue::Double(-0.0)),
        (9, NO_LOCATION, NO_ELEVATION, EncodedValue::Double(0.0)),
        (
            10,
            NO_LOCATION,
            NO_ELEVATION,
            EncodedValue::Decimal(dec(150, 2)),
        ),
        (
            11,
            NO_LOCATION,
            NO_ELEVATION,
            EncodedValue::Decimal(dec(15, 1)),
        ),
        (
            12,
            NO_LOCATION,
            NO_ELEVATION,
            EncodedValue::Decimal(dec(1, 200)),
        ),
        (
            13,
            NO_LOCATION,
            NO_ELEVATION,
            EncodedValue::Decimal(dec(1 << 50, 0)),
        ),
        (14, NO_LOCATION, NO_ELEVATION, EncodedValue::Double(1.0)),
        (
            15,
            NO_LOCATION,
            NO_ELEVATION,
            EncodedValue::Decimal(dec(7, 0)),
        ),
        (16, NO_LOCATION, NO_ELEVATION, EncodedValue::Double(7.0)),
        (16, NO_LOCATION, NO_ELEVATION, EncodedValue::Long(i64::MIN)),
        (1 << 60, NO_LOCATION, NO_ELEVATION, EncodedValue::Long(0)),
    ];
    let mut encoder = GtsEncoder::new(0);
    for (t, l, e, v) in &points {
        encoder.add_value(*t, *l, *e, v);
    }
    assert_eq!(encoder.count(), points.len() as i64);
    assert_eq!(encoder.size(), encoder.bytes().len());
    let decoded = decode_all(&mut encoder.decoder());
    let expected: Vec<_> = points
        .iter()
        .map(|(t, l, e, v)| match v {
            EncodedValue::Null => (*t, NO_LOCATION, NO_ELEVATION, EncodedValue::Null),
            EncodedValue::Decimal(d) if d.scale > 127 || d.bit_length() > 46 => {
                (*t, *l, *e, EncodedValue::Double(d.double_value()))
            }
            EncodedValue::Decimal(d) => (*t, *l, *e, EncodedValue::Decimal(d.strip())),
            other => (*t, *l, *e, other.clone()),
        })
        .collect();
    assert_eq!(decoded, expected);
    // `getValue` keeps binary values as ISO-8859-1 strings.
    let mut d = encoder.decoder();
    for _ in 0..12 {
        d.next().unwrap();
    }
    assert_eq!(d.value(), EncodedValue::String(Rc::from("A")));
}

#[test]
fn decoder_quirks() {
    // A string identical to a previous non Latin-1 one, flagged binary.
    let mut d = decoder("3802c4803b");
    d.next().unwrap();
    d.next().unwrap();
    assert_eq!(d.binary_value(), EncodedValue::Bytes(Rc::from(vec![b'?'])));
    // An identical string without a previous one is null.
    let mut d = decoder("39");
    d.next().unwrap();
    assert_eq!(d.value(), EncodedValue::Null);
    // Encrypted blocks are skipped; a continuation without its byte ends the stream.
    let mut d = decoder("000201022480");
    assert!(d.next().unwrap());
    assert_eq!(d.value(), EncodedValue::Boolean(true));
    assert!(!d.next().unwrap());
    // Nine continuation bytes are fine, the shift wraps.
    let mut d = decoder("44ffffffffffffffffff01");
    d.next().unwrap();
    assert_eq!(d.timestamp(), i64::MIN);
    for (bytes, message, class) in [
        (
            "44ffffffffffffffffffff01",
            "Variable length quantity is too long",
            "java.lang.IllegalArgumentException",
        ),
        (
            "6000",
            "BufferUnderflowException",
            "java.nio.BufferUnderflowException",
        ),
        (
            "00",
            "BufferUnderflowException",
            "java.nio.BufferUnderflowException",
        ),
        (
            "0002",
            "newPosition > limit: (4 > 2)",
            "java.lang.IllegalArgumentException",
        ),
        (
            "00f0ffffff0f",
            "newPosition < 0: (-10 < 0)",
            "java.lang.IllegalArgumentException",
        ),
        (
            "3805",
            "Invalid string length.",
            "java.lang.RuntimeException",
        ),
        (
            "38ffffffffffffffffff01",
            "-1",
            "java.lang.NegativeArraySizeException",
        ),
        (
            "3885808080808080808001",
            "BufferUnderflowException",
            "java.nio.BufferUnderflowException",
        ),
        ("21", "Invalid boolean value.", "java.lang.RuntimeException"),
    ] {
        assert_eq!(decode_err(bytes), (message.to_owned(), class), "{bytes}");
    }
    // Every field of a datapoint can be truncated.
    for bytes in [
        "40", "04", "a440", "a460", "a408", "a40c", "28", "2c", "34", "30", "3001", "38",
    ] {
        assert!(decoder(bytes).next().is_err(), "{bytes}");
    }
}

#[test]
fn encoders_from_decoders() {
    let mut encoder = GtsEncoder::new(5);
    encoder.metadata.name = Some("n".to_owned());
    for (t, v) in [(5, 1), (6, 2), (7, 2)] {
        encoder.add_value(t, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(v));
    }
    let mut d = encoder.decoder();
    let e = d.encoder().unwrap_err();
    assert_eq!(
        e.message(),
        "Can only get an encoder for a decoder on which 'next' was called at least once."
    );
    d.next().unwrap();
    d.next().unwrap();
    let tail = d.encoder().unwrap();
    assert_eq!(tail.count(), 2);
    assert_eq!(tail.base(), 5);
    assert_eq!(tail.metadata.name.as_deref(), Some("n"));
    let values: Vec<_> = decode_all(&mut tail.decoder())
        .into_iter()
        .map(|p| p.3)
        .collect();
    assert_eq!(values, vec![EncodedValue::Long(2), EncodedValue::Long(2)]);
    // Appending to it does not rely on the previous deltas.
    let mut tail = tail;
    tail.add_value(8, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(2));
    let values: Vec<_> = decode_all(&mut tail.decoder())
        .into_iter()
        .map(|p| p.0)
        .collect();
    assert_eq!(values, vec![6, 7, 8]);
    tail.set_count(9);
    assert_eq!(tail.count(), 9);
}

#[test]
fn merges() {
    let mut a = GtsEncoder::new(0);
    a.add_value(1, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(1));
    let mut b = GtsEncoder::new(0);
    b.metadata.name = Some("b".to_owned());
    b.add_value(2, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(2));
    // Into an empty encoder of the same base: a copy.
    let mut empty = GtsEncoder::new(0);
    empty.merge(&b).unwrap();
    assert_eq!(empty.metadata.name.as_deref(), Some("b"));
    assert_eq!(empty.bytes(), b.bytes());
    // Same base: the bytes are appended.
    let mut same = a.clone();
    same.merge(&b).unwrap();
    same.add_value(3, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(2));
    assert_eq!(same.count(), 3);
    let ticks: Vec<_> = decode_all(&mut same.decoder())
        .into_iter()
        .map(|p| p.0)
        .collect();
    assert_eq!(ticks, vec![1, 2, 3]);
    // Different bases: re-encoded.
    let mut other = GtsEncoder::new(100);
    other.merge(&a).unwrap();
    other.merge(&b).unwrap();
    let ticks: Vec<_> = decode_all(&mut other.decoder())
        .into_iter()
        .map(|p| p.0)
        .collect();
    assert_eq!(ticks, vec![1, 2]);
    let clone = other.clone_empty();
    assert_eq!((clone.base(), clone.count()), (100, 0));
    // A corrupt encoder cannot be merged into one of another base.
    let wrapper = GtsWrapper {
        encoded: Some(hex("442121")),
        ..GtsWrapper::default()
    };
    let corrupt = wrapper_to_encoder(&wrapper).unwrap();
    let mut target = GtsEncoder::new(1);
    assert!(target.merge(&corrupt).is_err());
}

#[test]
fn wrappers() {
    // Tiny content is not worth compressing.
    let mut encoder = GtsEncoder::new(0);
    encoder.add_value(1, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(1));
    let wrapper = encoder_to_wrapper(&encoder, true, 0.5, false);
    assert_eq!(wrapper.compressed, None);
    assert_eq!(wrapper.count, None);
    assert_eq!(wrapper.metadata, None);
    // Highly redundant content is gzipped several times.
    let mut big = GtsEncoder::new(7);
    let text: Rc<str> = Rc::from("ab".repeat(1 << 16));
    for t in 0..64 {
        big.add_value(
            t,
            NO_LOCATION,
            NO_ELEVATION,
            &EncodedValue::String(Rc::clone(&text)),
        );
        big.add_value(t, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(t));
    }
    big.metadata.labels.insert("k".to_owned(), "v".to_owned());
    let wrapper = encoder_to_wrapper(&big, true, 1.0, true);
    assert_eq!(wrapper.compressed, Some(true));
    assert!(wrapper.compression_passes.is_some_and(|p| p > 1));
    assert_eq!(wrapper.base, Some(7));
    let gts = wrapper_to_gts(&wrapper, false).unwrap();
    assert_eq!(gts.size(), 128);
    assert_eq!(gts.metadata.labels.len(), 1);
    let back = wrapper_to_encoder(&wrapper).unwrap();
    assert_eq!(back.count(), 128);
    let wrapper = encoder_to_wrapper(&big, true, DEFAULT_COMP_RATIO_THRESHOLD, true);
    assert_eq!(wrapper.compressed, Some(true));
    let mut once = GtsEncoder::new(0);
    once.add_value(
        1,
        NO_LOCATION,
        NO_ELEVATION,
        &EncodedValue::String(Rc::from("a".repeat(200))),
    );
    let wrapper = encoder_to_wrapper(&once, true, DEFAULT_COMP_RATIO_THRESHOLD, true);
    assert_eq!(
        (wrapper.compressed, wrapper.compression_passes),
        (Some(true), None)
    );
    // Metadata worth wrapping.
    for change in 0..6 {
        let mut e = GtsEncoder::new(0);
        let m = &mut e.metadata;
        match change {
            0 => m.name = Some("n".to_owned()),
            1 => {
                m.attributes.insert("a".to_owned(), "b".to_owned());
            }
            2 => m.class_id_set = true,
            3 => m.labels_id_set = true,
            4 => m.source = Some(String::new()),
            _ => m.last_activity_set = true,
        }
        assert!(encoder_to_wrapper(&e, false, 1.0, true).metadata.is_some());
    }
    let mut e = GtsEncoder::new(0);
    e.metadata.name = Some(String::new());
    assert!(encoder_to_wrapper(&e, false, 1.0, true).metadata.is_none());
}

#[test]
fn unwrapping() {
    let invalid = |w: &GtsWrapper| {
        let e = wrapper_to_gts(w, false).unwrap_err();
        assert_eq!(e.message(), "Invalid compressed content.");
    };
    invalid(&GtsWrapper {
        encoded: Some(vec![1, 2, 3]),
        compressed: Some(true),
        ..GtsWrapper::default()
    });
    invalid(&GtsWrapper {
        encoded: Some(Vec::new()),
        compressed: Some(true),
        ..GtsWrapper::default()
    });
    // No pass at all: the content is used as is.
    let w = GtsWrapper {
        encoded: Some(hex("24")),
        compressed: Some(true),
        compression_passes: Some(0),
        bucketcount: Some((1 << 32) + 3),
        bucketspan: Some(2),
        lastbucket: Some(9),
        ..GtsWrapper::default()
    };
    let gts = wrapper_to_gts(&w, false).unwrap();
    assert_eq!(
        (gts.size(), gts.bucketcount, gts.bucketspan, gts.lastbucket),
        (1, 3, 2, 9)
    );
    assert_eq!(gts.name(), None);
    assert_eq!(wrapper_to_gts(&w, true).unwrap().size(), 0);
    assert_eq!(
        wrapper_to_gts(&GtsWrapper::default(), false)
            .unwrap()
            .size(),
        0
    );
    // Encoders.
    let e = wrapper_to_encoder(&GtsWrapper::default()).unwrap_err();
    assert_eq!(
        crate::error::compat::exception_class(&e),
        "java.lang.NullPointerException"
    );
    let w = GtsWrapper {
        encoded: Some(Vec::new()),
        base: Some(4),
        metadata: Some(Metadata {
            name: Some("x".to_owned()),
            ..Metadata::default()
        }),
        ..GtsWrapper::default()
    };
    let e = wrapper_to_encoder(&w).unwrap();
    assert_eq!((e.base(), e.metadata.name.as_deref()), (4, Some("x")));
    let w = GtsWrapper {
        encoded: Some(vec![1]),
        compressed: Some(true),
        ..GtsWrapper::default()
    };
    assert!(wrapper_to_encoder(&w).is_err());
    let w = GtsWrapper {
        encoded: Some(hex("21")),
        ..GtsWrapper::default()
    };
    assert!(wrapper_to_gts(&w, false).is_err());
    assert!(wrapper_to_encoder(&w).is_err());
}

#[test]
fn failures_have_the_wording_and_class_of_warp10() {
    use crate::error::compat::exception_class;
    use crate::value::ValueKind;

    let io = std::io::Error::new(std::io::ErrorKind::InvalidData, "corrupt deflate stream");
    for (error, message, class, runtime) in [
        (
            EncoderError::UnsupportedValue(ValueKind::List),
            "Unsuported value type 'class java.util.ArrayList'",
            "java.lang.RuntimeException",
            true,
        ),
        (
            EncoderError::BufferUnderflow,
            "BufferUnderflowException",
            "java.nio.BufferUnderflowException",
            true,
        ),
        (
            EncoderError::VarintTooLong,
            "Variable length quantity is too long",
            "java.lang.IllegalArgumentException",
            true,
        ),
        (
            EncoderError::PositionBeforeStart { target: -1 },
            "newPosition < 0: (-1 < 0)",
            "java.lang.IllegalArgumentException",
            true,
        ),
        (
            EncoderError::PositionAfterLimit {
                target: 9,
                limit: 4,
            },
            "newPosition > limit: (9 > 4)",
            "java.lang.IllegalArgumentException",
            true,
        ),
        (
            EncoderError::InvalidStringLength,
            "Invalid string length.",
            "java.lang.RuntimeException",
            true,
        ),
        (
            EncoderError::NegativeStringLength(-5),
            "-5",
            "java.lang.NegativeArraySizeException",
            true,
        ),
        (
            EncoderError::InvalidBoolean,
            "Invalid boolean value.",
            "java.lang.RuntimeException",
            true,
        ),
        (
            EncoderError::InvalidCompressedContent(io.into()),
            "Invalid compressed content.",
            "java.lang.RuntimeException",
            true,
        ),
        (
            EncoderError::DecoderNotStarted,
            "Can only get an encoder for a decoder on which 'next' was called at least once.",
            "java.io.IOException",
            false,
        ),
    ] {
        let error = Error::from(error);
        assert_eq!(error.to_string(), message);
        assert_eq!(exception_class(&error), class, "{message}");
        assert_eq!(error.is_runtime(), runtime, "{message}");
    }
}

#[test]
fn a_corrupt_deflate_stream_is_the_source_of_the_invalid_content() {
    use std::error::Error as _;

    let wrapper = GtsWrapper {
        encoded: Some(vec![1, 2, 3]),
        compressed: Some(true),
        compression_passes: Some(1),
        ..GtsWrapper::default()
    };
    let err = wrapper_to_encoder(&wrapper).expect_err("the content is not gzip");
    assert_eq!(err.to_string(), "Invalid compressed content.");
    assert!(matches!(
        err,
        Error::Encoder(EncoderError::InvalidCompressedContent(_))
    ));
    let source = err
        .source()
        .expect("the failure of the inflater is the source");
    assert_ne!(source.to_string(), "");
}

#[test]
fn series_values_are_encoded_by_type() {
    let mut gts = GeoTimeSerie::default();
    assert_eq!(EncodedValue::of_series(&gts, 0), EncodedValue::Null);
    gts.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::Long(7), false);
    assert_eq!(EncodedValue::of_series(&gts, 0), EncodedValue::Long(7));
    assert_eq!(EncodedValue::of_series(&gts, 1), EncodedValue::Null);
    let mut doubles = GeoTimeSerie::default();
    doubles.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::Double(1.5), false);
    assert_eq!(
        EncodedValue::of_series(&doubles, 0),
        EncodedValue::Double(1.5)
    );
    assert_eq!(EncodedValue::of_series(&doubles, 1), EncodedValue::Null);
    let mut booleans = GeoTimeSerie::default();
    booleans.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::Boolean(true), false);
    assert_eq!(
        EncodedValue::of_series(&booleans, 0),
        EncodedValue::Boolean(true)
    );
    assert_eq!(EncodedValue::of_series(&booleans, 1), EncodedValue::Null);
    let mut strings = GeoTimeSerie::default();
    strings.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::string("s"), false);
    assert_eq!(
        EncodedValue::of_series(&strings, 0),
        EncodedValue::String(Rc::from("s"))
    );
    assert_eq!(EncodedValue::of_series(&strings, 1), EncodedValue::Null);
}
