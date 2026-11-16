// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

#![allow(clippy::format_collect)]

use indexmap::IndexMap;

use crate::error::Error;
use crate::gts::Metadata;
use crate::thrift::{
    GtsWrapper, MetaSet, ThriftError, deserialize_wrapper, serialize_metaset, serialize_wrapper,
};

pub(crate) fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("two hexadecimal digits"))
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn err(bytes: &[u8]) -> (String, &'static str) {
    let e = Error::from(deserialize_wrapper(bytes).expect_err("the payload is malformed"));
    (e.message(), crate::error::compat::exception_class(&e))
}

const EOF1: &str = "Cannot read. Remote side has closed. Tried to read 1 bytes, but only got 0 bytes. \
                    (This is often indicative of an internal error on the server side. Please check \
                    your server logs.)";
const TRANSPORT: &str = "org.apache.thrift.transport.TTransportException";
const PROTOCOL: &str = "org.apache.thrift.protocol.TProtocolException";

#[test]
fn round_trip() {
    let mut labels = IndexMap::new();
    labels.insert("a".to_owned(), "b".to_owned());
    let mut attributes = IndexMap::new();
    attributes.insert("c".to_owned(), "d".to_owned());
    let wrapper = GtsWrapper {
        metadata: Some(Metadata {
            name: Some("n".to_owned()),
            labels,
            attributes,
            class_id: -3,
            labels_id: 1 << 40,
            last_activity: 42,
            class_id_set: true,
            labels_id_set: true,
            last_activity_set: true,
            source: Some("s".to_owned()),
        }),
        lastbucket: Some(10),
        bucketspan: Some(1),
        bucketcount: Some(10),
        base: Some(-1),
        encoded: Some(vec![1, 2, 3]),
        count: Some(3),
        compressed: Some(false),
        compression_passes: Some(-2),
    };
    let bytes = serialize_wrapper(&wrapper);
    assert_eq!(
        deserialize_wrapper(&bytes).expect("the payload is a valid wrapper"),
        wrapper
    );
    let mut compressed = wrapper.clone();
    compressed.compressed = Some(true);
    compressed.metadata = Some(Metadata::default());
    let bytes = serialize_wrapper(&compressed);
    let mut back = deserialize_wrapper(&bytes).expect("the payload is a valid wrapper");
    assert!(back.is_compressed());
    assert_eq!(back.compression_passes(), -2);
    assert_eq!((back.base(), back.count()), (-1, 3));
    let meta = back
        .metadata
        .take()
        .expect("the payload is a valid wrapper");
    assert_eq!(meta.name, None);
    assert!(meta.class_id_set && !meta.last_activity_set);
}

#[test]
fn metadata_equality() {
    let a = Metadata {
        last_activity_set: true,
        last_activity: 1,
        class_id_set: true,
        ..Metadata::default()
    };
    let mut b = a.clone();
    b.class_id_set = false;
    assert_eq!(a, b);
    b.last_activity = 2;
    assert_ne!(a, b);
    b.last_activity_set = false;
    assert_ne!(a, b);
    assert_eq!(b, Metadata::default());
    assert_eq!(serialize_wrapper(&GtsWrapper::default()), vec![0]);
}

#[test]
fn oracle_bytes() {
    // `NEWGTS WRAPRAW` and a named, labelled series (Warp 10 3.5.0).
    let empty = GtsWrapper {
        encoded: Some(Vec::new()),
        count: Some(0),
        ..GtsWrapper::default()
    };
    assert_eq!(to_hex(&serialize_wrapper(&empty)), "7800160000");
    let s = hex("1c1801731b018801610162160016001b000068094c02020e04060c0104160600");
    let wrapper = deserialize_wrapper(&s).expect("the payload is a valid wrapper");
    let meta = wrapper
        .metadata
        .clone()
        .expect("the payload is a valid wrapper");
    assert_eq!(meta.name.as_deref(), Some("s"));
    assert_eq!(meta.labels.get("a").map(String::as_str), Some("b"));
    assert_eq!(wrapper.encoded.as_ref().map(Vec::len), Some(9));
    assert_eq!(wrapper.count, Some(3));
    assert_eq!(serialize_wrapper(&wrapper), s);
    assert_eq!(wrapper.compression_passes(), 1);
    assert!(!wrapper.is_compressed());
}

#[test]
fn skipped_fields() {
    // A struct ends on a type nibble 0; a metadata name of type MAP is skipped.
    assert_eq!(
        deserialize_wrapper(&[0x10]).expect("the payload is a valid wrapper"),
        GtsWrapper::default()
    );
    let w =
        deserialize_wrapper(&hex("1c1b0188016101620000")).expect("the payload is a valid wrapper");
    assert_eq!(
        w.metadata.expect("the payload is a valid wrapper").name,
        None
    );
    // Unknown metadata field, then every skippable type in the wrapper (field 11+).
    let w = deserialize_wrapper(&hex(concat!(
        "1cb60200",           // metadata { 11: i64 }
        "b1",                 // 12: bool true
        "11",                 // 13: bool true
        "12",                 // 14: bool false
        "1305",               // 15: byte
        "1402",               // 16: i16
        "1502",               // 17: i32
        "1602",               // 18: i64
        "170000000000000000", // 19: double
        "180161",             // 20: binary
        "1900",               // 21: empty list
        "19f8010161",         // 22: list of one string, long size
        "1a1101",             // 23: set of one bool
        "1b018801610162",     // 24: map {a: b}
        "1c00",               // 25: empty struct
        "00",
    )))
    .expect("the payload is a valid wrapper");
    assert!(w.metadata.is_some());
    // Long field headers: `lastbucket` (id 2) and `encoded` (id 7).
    let w = deserialize_wrapper(&hex("0604020800")).expect_err("the binary is cut short");
    assert_eq!(w.to_string(), EOF1);
    let w = deserialize_wrapper(&hex("060414080e010000")).expect("the payload is a valid wrapper");
    assert_eq!((w.lastbucket, w.encoded), (Some(10), Some(vec![0])));
    // Booleans and compression passes.
    let w = deserialize_wrapper(&hex("91150400")).expect("the payload is a valid wrapper");
    assert_eq!((w.compressed, w.compression_passes), (Some(true), Some(2)));
    let w = deserialize_wrapper(&hex("9200")).expect("the payload is a valid wrapper");
    assert_eq!(w.compressed, Some(false));
}

#[test]
fn errors() {
    assert_eq!(err(&[]), (EOF1.to_owned(), TRANSPORT));
    // Fast varint path running off the array, slow path short read.
    assert_eq!(
        err(&hex("a5ffffffffff")),
        (
            "Index 6 out of bounds for length 6".to_owned(),
            "java.lang.ArrayIndexOutOfBoundsException"
        )
    );
    assert_eq!(err(&hex("a5ff")).0, EOF1);
    assert_eq!(err(&hex("1c18ffffffff0f")).0, "Negative length: -1");
    assert_eq!(
        err(&hex("78ffffffff0f")),
        (
            "IndexOutOfBoundsException".to_owned(),
            "java.lang.IndexOutOfBoundsException"
        )
    );
    assert!(
        err(&hex("7805aa"))
            .0
            .contains("Tried to read 5 bytes, but only got 1 bytes.")
    );
    assert!(
        err(&hex("b700"))
            .0
            .contains("Tried to read 8 bytes, but only got 1 bytes.")
    );
    // A double consumes the message size budget; a binary of the remaining size is refused.
    assert_eq!(
        err(&hex("b70000000000000000080ef8ffffff07")).0,
        "MaxMessageSize reached"
    );
    assert_eq!(err(&hex("1c2b01ff")).0, "don't know what type: 15");
    assert_eq!(err(&hex("1d")).0, "don't know what type: 13");
    assert_eq!(err(&hex("b9f8ffffffff0f")).0, "Negative length: -1");
    assert_eq!(err(&hex("b91000")).0, "Unrecognized type 0");
    assert_eq!(err(&hex("b9")).0, EOF1);
    assert_eq!(err(&hex("1cbb")).0, EOF1);
    let mut nested = vec![0xb9];
    nested.extend(vec![0x19; 600]);
    assert_eq!(
        err(&nested),
        (
            "StackOverflowError".to_owned(),
            "java.lang.StackOverflowError"
        )
    );
    for (bytes, message) in [
        ("bbffffffff0f", "Negative length: -1"),
        ("bbffffffff0788", "MaxMessageSize reached"),
        ("b9f7ffffffff07", "MaxMessageSize reached"),
        ("bb01f8", "don't know what type: 15"),
        ("bb018f", "don't know what type: 15"),
        ("b91d", "don't know what type: 13"),
    ] {
        assert_eq!(err(&hex(bytes)).0, message, "{bytes}");
    }
    // Short reads inside skipped values.
    for bytes in [
        "b911",
        "b9f8",
        "bb01",
        "bb01880161",
        "06",
        "1c180561",
        "1c2b0188",
        "1c2b01880161",
        "1c36",
        "1c46",
        "1c5b",
        "1c68",
        "1c76",
        "26",
        "36",
        "46",
        "86",
    ] {
        assert_eq!(err(&hex(bytes)).1, TRANSPORT, "{bytes}");
    }
    for bytes in [
        "b3",
        "b4",
        "b6",
        "b8",
        "bc",
        "bcb3",
        "1cb1",
        "1c18",
        "1c1b0188",
        "1c1b018801",
        "a5",
        "66",
        "96",
    ] {
        assert!(deserialize_wrapper(&hex(bytes)).is_err(), "{bytes}");
    }
}

#[test]
fn failures_are_typed() {
    let typed =
        |bytes: &str| deserialize_wrapper(&hex(bytes)).expect_err("the payload is malformed");
    assert_eq!(typed(""), ThriftError::EndOfInput { wanted: 1, got: 0 });
    assert_eq!(typed("1c18ffffffff0f"), ThriftError::NegativeLength(-1));
    assert_eq!(typed("1c2b01ff"), ThriftError::UnknownCompactType(15));
    assert_eq!(typed("b91000"), ThriftError::UnrecognizedType(0));
    assert_eq!(
        typed("b70000000000000000080ef8ffffff07"),
        ThriftError::MessageTooLarge
    );
    assert_eq!(
        typed("a5ffffffffff"),
        ThriftError::VarintOverrun {
            index: 6,
            length: 6
        }
    );
    assert_eq!(typed("78ffffffff0f"), ThriftError::NegativeBinaryLength);
    assert_eq!(
        typed(&format!("b9{}", "19".repeat(600))),
        ThriftError::SkipTooDeep
    );
}

#[test]
fn malformed_input_is_told_from_reader_defects() {
    for (error, malformed, class) in [
        (
            ThriftError::EndOfInput { wanted: 1, got: 0 },
            true,
            TRANSPORT,
        ),
        (ThriftError::MessageTooLarge, true, TRANSPORT),
        (ThriftError::NegativeLength(-1), true, PROTOCOL),
        (ThriftError::UnrecognizedType(0), true, PROTOCOL),
        (ThriftError::UnknownCompactType(15), true, PROTOCOL),
        (
            ThriftError::VarintOverrun {
                index: 1,
                length: 1,
            },
            false,
            "java.lang.ArrayIndexOutOfBoundsException",
        ),
        (
            ThriftError::NegativeBinaryLength,
            false,
            "java.lang.IndexOutOfBoundsException",
        ),
        (
            ThriftError::SkipTooDeep,
            false,
            "java.lang.StackOverflowError",
        ),
    ] {
        assert_eq!(error.is_malformed_input(), malformed, "{error}");
        let error = Error::from(error);
        assert_eq!(error.is_runtime(), !malformed, "{error}");
        assert_eq!(crate::error::compat::exception_class(&error), class);
    }
}

#[test]
fn metaset_is_laid_out_like_libthrift() {
    // Hand-encoded from the compact protocol: the field headers (delta and type), the zigzag
    // varints, the binary token and the struct of the one series, then the list of the sets.
    let set = MetaSet {
        expiry: 1,
        token: "t".to_owned(),
        metadatas: vec![Metadata {
            name: Some("c".to_owned()),
            ..Metadata::default()
        }],
        maxduration: Some(5),
        notbefore: None,
        notafter: Some(-1),
    };
    assert_eq!(
        serialize_metaset(&set),
        vec![
            0x16, 0x02, // expiry 1 (i64, zigzag)
            0x18, 0x01, b't', // token (binary)
            0x19, 0x1c, // metadatas: a list of one struct
            0x18, 0x01, b'c', // name
            0x1b, 0x00, // labels: an empty map
            0x16, 0x00, // classId
            0x16, 0x00, // labelsId
            0x1b, 0x00, // attributes: an empty map
            0x00, // end of the Metadata
            0x16, 0x0a, // maxduration 5
            0x26, 0x01, // notafter -1 (the field 5 is skipped, unset)
            0x00, // end of the MetaSet
        ]
    );
}

#[test]
fn metaset_lists_of_fifteen_series_or_more_have_a_size_varint() {
    let set = MetaSet {
        metadatas: vec![Metadata::default(); 15],
        ..MetaSet::default()
    };
    let bytes = serialize_metaset(&set);
    // The list header is 0xf0 | struct (12) followed by the size as a varint (15).
    assert_eq!(bytes[..8], [0x16, 0x00, 0x18, 0x00, 0x19, 0xfc, 0x0f, 0x2b]);
}
