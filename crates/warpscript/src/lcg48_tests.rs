// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::float_cmp)]

//! Reference sequences below were produced with OpenJDK 27 (`java.util.Random`).

use crate::lcg48::Lcg48;

#[test]
fn seed_42_sequence() {
    let mut r = Lcg48::new(42);
    assert_eq!(r.next_int(), -1_170_105_035);
    assert_eq!(r.next_double(), 0.054_665_262_747_160_77);
    assert_eq!(r.next_long(), 884_324_181_205_335_268);
    assert_eq!(r.next_int_bounded(10), 5);
    assert_eq!(r.next_int_bounded(16), 4);
    assert!(r.next_boolean());
}

#[test]
fn seed_0_sequence() {
    let mut r = Lcg48::new(0);
    assert_eq!(r.next_double(), 0.730_967_787_376_657);
    assert_eq!(r.next_double(), 0.240_536_415_671_485_87);
    assert_eq!(r.next_int_bounded(7), 4);
    assert_eq!(r.next_int_bounded(3), 2);
}

#[test]
fn bounded_ints_with_rejection() {
    let mut r = Lcg48::new(-1);
    let values: Vec<i32> = (0..5).map(|_| r.next_int_bounded(1000)).collect();
    assert_eq!(values, [913, 225, 579, 439, 604]);
    // With a bound of 2/3 of 2^31 one draw in three is rejected: seed 0 needs three draws
    // (`new Random(0).nextInt(1431655766)` is 516548029).
    let mut r = Lcg48::new(0);
    assert_eq!(r.next_int_bounded(1_431_655_766), 516_548_029);
    // Every bounded draw stays in range, including after rejections.
    let mut r = Lcg48::new(7);
    for _ in 0..10_000 {
        let v = r.next_int_bounded(3);
        assert!((0..3).contains(&v));
    }
}

#[test]
fn state_round_trips() {
    let mut r = Lcg48::new(42);
    let first = r.next_double();
    let state = r.state();
    let mut restored = Lcg48::from_state(state);
    assert_eq!(restored, r);
    assert_eq!(restored.next_double(), r.next_double());
    assert_ne!(first, restored.next_double());
    // Only the low 48 bits are state.
    assert_eq!(Lcg48::from_state(-1).state(), (1 << 48) - 1);
}

#[test]
fn doubles_are_in_unit_interval() {
    let mut r = Lcg48::new(123_456_789);
    for _ in 0..10_000 {
        let d = r.next_double();
        assert!((0.0..1.0).contains(&d), "{d}");
    }
}

#[test]
fn gaussian_numbers_come_in_pairs_like_the_jdk_makes_them() {
    // `new Random(42).nextGaussian()` three times (the second one is the other of the pair).
    let mut random = Lcg48::new(42);
    for expected in [
        1.141_905_315_473_054_7,
        0.919_407_948_982_787_9,
        -0.949_866_636_890_895_9,
    ] {
        let actual = random.next_gaussian();
        assert!(
            (actual - expected).abs() < 1e-14,
            "{actual} instead of {expected}"
        );
    }
}
