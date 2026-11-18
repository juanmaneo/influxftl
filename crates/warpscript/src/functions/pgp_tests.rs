// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of the key IDs that PGPPRIVATE lists: each key ID once, even when a ring repeats it.

use super::pgp::key_ids;
use crate::openpgp::ring::{Ring, read_rings};

fn one_key_ring() -> Ring {
    // A v4 RSA public key packet (tag 6) with a 16-bit modulus and exponent 3.
    let packet = [0x98, 13, 4, 0, 0, 0, 1, 1, 0, 16, 0xAB, 0xCD, 0, 2, 3];
    let Ok(mut rings) = read_rings(&packet) else {
        panic!("the key packet must parse");
    };
    let Some(ring) = rings.pop() else {
        panic!("one ring is expected");
    };
    ring
}

#[test]
fn a_repeated_key_id_is_listed_once() {
    let single = one_key_ring();
    let Some(key) = single.keys.first().cloned() else {
        panic!("the ring has a primary key");
    };
    let repeated = Ring {
        secret: true,
        keys: vec![key.clone(), key],
        extras: Vec::new(),
    };
    assert_eq!(key_ids(&repeated), key_ids(&single));
}
