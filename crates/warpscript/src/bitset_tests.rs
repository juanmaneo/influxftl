// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::bitset::BitSet;

#[test]
fn from_bytes_trims_trailing_zero_bytes() {
    let set = BitSet::from_bytes(&[0x05, 0x00, 0x80, 0x00, 0x00]);
    assert_eq!(set.to_bytes(), vec![0x05, 0x00, 0x80]);
    assert_eq!(BitSet::from_bytes(&[0, 0]), BitSet::default());
    assert_eq!(BitSet::from_bytes(&[]).to_bytes(), Vec::<u8>::new());
}

#[test]
fn get_length_cardinality() {
    // Reference values from java.util.BitSet.valueOf(new byte[]{0x05, 0x00, (byte) 0x80}).
    let set = BitSet::from_bytes(&[0x05, 0x00, 0x80]);
    assert!(set.get(0).unwrap());
    assert!(!set.get(1).unwrap());
    assert!(set.get(2).unwrap());
    assert!(set.get(23).unwrap());
    assert!(!set.get(100).unwrap());
    assert_eq!(set.get(-1).unwrap_err().to_string(), "bitIndex < 0: -1");
    assert_eq!(set.length(), 24);
    assert_eq!(set.cardinality(), 3);

    let empty = BitSet::default();
    assert_eq!(empty.length(), 0);
    assert_eq!(empty.cardinality(), 0);
    assert!(!empty.get(0).unwrap());

    let big = BitSet::from_bytes(&[0, 0, 0, 0, 0, 0, 0, 0, 1]);
    assert_eq!(big.length(), 65);
    assert_eq!(big.cardinality(), 1);
    assert_eq!(big.set_bits().collect::<Vec<_>>(), vec![64]);
}

#[test]
fn display_like_java() {
    assert_eq!(
        BitSet::from_bytes(&[0x05, 0x00, 0x80]).to_string(),
        "{0, 2, 23}"
    );
    assert_eq!(BitSet::default().to_string(), "{}");
    assert_eq!(
        BitSet::from_bytes(&[0xFF]).to_string(),
        "{0, 1, 2, 3, 4, 5, 6, 7}"
    );
}

#[test]
fn display_propagates_writer_failures() {
    use std::fmt::Write;

    use super::error_tests::FailAfter;

    let set = BitSet::from_bytes(&[0x05]);
    let outcomes: Vec<bool> = (0..6)
        .map(|n| write!(FailAfter(n), "{set}").is_ok())
        .collect();
    assert_eq!(outcomes, [false, false, false, false, false, true]);
}
