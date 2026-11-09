// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the Argon2 port against BouncyCastle 1.78.1 (`Argon2BytesGenerator`) and of the
//! embedded BLAKE2b against RFC 7693.

use crate::argon2::{ARGON2_D, ARGON2_I, ARGON2_ID, Params, VERSION_10, blake2b, generate};
use crate::encoding::hex_encode;

#[test]
fn blake2b_matches_rfc7693() {
    // Appendix A of RFC 7693.
    assert_eq!(
        hex_encode(&blake2b(64, &[b"abc"])),
        "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923"
    );
    assert_eq!(
        hex_encode(&blake2b(64, &[b""])),
        "786a02f742015903c6c6fd852552d272912f4740e15847618a86e217f71f5419d25e1031afee585313896444934eb04b903a685b1448b755d56f701afe9be2ce"
    );
    assert_eq!(
        hex_encode(&blake2b(32, &[b"abc"])),
        "bddd813c634239723171ef3fee98579b94964e3bb1cb3e427262c8c068d52319"
    );
    // Block boundaries (BouncyCastle `Blake2bDigest` values): exactly one block is a single
    // final compression, one more byte needs two.
    let seq: Vec<u8> = (0..300u32).map(|i| i as u8).collect();
    assert_eq!(
        hex_encode(&blake2b(64, &[&seq[..128]])),
        "2319e3789c47e2daa5fe807f61bec2a1a6537fa03f19ff32e87eecbfd64b7e0e8ccff439ac333b040f19b0c4ddd11a61e24ac1fe0f10a039806c5dcc0da3d115"
    );
    assert_eq!(
        hex_encode(&blake2b(64, &[&seq[..129]])),
        "f59711d44a031d5f97a9413c065d1e614c417ede998590325f49bad2fd444d3e4418be19aec4e11449ac1a57207898bc57d76a1bcf3566292c20c683a5c4648f"
    );
    assert_eq!(
        hex_encode(&blake2b(64, &[&seq[..256]])),
        "1ecc896f34d3f9cac484c73f75f6a5fb58ee6784be41b35f46067b9c65c63a6794d3d744112c653f73dd7deb6666204c5a9bfa5b46081fc10fdbe7884fa5cbf8"
    );
    // The concatenation of the parts is what gets hashed.
    let (head, tail) = seq.split_at(129);
    assert_eq!(
        hex_encode(&blake2b(64, &[head, tail])),
        "d9cf5983dc6b34c0fa1f0226926855ad3eccd2bcdcd8f8053b9a80664d33b5afcc32fd21c70ea14f4ef50ca97c3203c4d1803159f0e01bb6cb1d1c83db52b63c"
    );
    assert_eq!(hex_encode(&blake2b(8, &[&seq])), "d325a9e186fa61cc");
    assert_eq!(
        hex_encode(&blake2b(36, &[&seq])),
        "55394a7b006078245229e4ea467d62ce16e7ad3419aa9bfb4716d2be4d0ceff69415587d"
    );
    // Output sizes are clamped to BLAKE2b's 1..=64 bytes.
    assert_eq!(blake2b(0, &[b"abc"]).len(), 1);
    assert_eq!(blake2b(100, &[b"abc"]).len(), 64);
}

fn params(kind: u32, iterations: i32, memory: i32, lanes: i32, salt: Option<&[u8]>) -> Params {
    Params {
        kind,
        iterations,
        memory,
        lanes,
        salt: salt.map(<[u8]>::to_vec),
        ..Params::default()
    }
}

const SALT: Option<&[u8]> = Some(b"saltsalt");

#[test]
fn argon2_matches_bouncycastle() {
    let cases: [(Params, i64, &str); 9] = [
        (
            Params::default(),
            32,
            "46fa7bf33c1a3f1329d381bcb8c12d526f0971717297df36e58d73177b935f87",
        ),
        (
            params(ARGON2_I, 3, 32, 1, SALT),
            32,
            "002b3a1d0d35f03eec61552964d4e6fca26b0e770397d42e9f7b358264ed6210",
        ),
        (
            params(ARGON2_D, 3, 32, 1, SALT),
            32,
            "71b6fbb2d5f73f84390d47577f78caee4f48b0c51accf4467631153850318373",
        ),
        (
            params(ARGON2_ID, 3, 32, 1, SALT),
            32,
            "207e0af68739ae80a597ba07aff5cbb27403abedabba6a7828423e71d600c93a",
        ),
        (
            params(ARGON2_ID, 3, 32, 1, Some(b"abc")),
            16,
            "a7688792942f660b490f1eaadc2450db",
        ),
        (
            Params {
                secret: Some(b"secret".to_vec()),
                additional: Some(b"additional".to_vec()),
                ..params(ARGON2_ID, 3, 32, 1, SALT)
            },
            32,
            "b890e8df6d4d89a6dbb247989ca9f2dedecb15e9a14d9c1b9ee6f2753252402c",
        ),
        (params(ARGON2_ID, 1, 64, 2, SALT), 4, "78b6a127"),
        (
            params(ARGON2_ID, 2, 8, 4, SALT),
            100,
            "501be8074ea5786a54254685263ccb10cdcf2a2c330fcbb660749277a23a245245a7accb04e07d700705445ca0d1d82168d302a349e3b2b5b9bdcd1034c7a4d580390fa0ca0cf25781c346c5e08b7c8c6e9591d03bd9def06acdb282fe91959c9ea3f7e8",
        ),
        (
            params(ARGON2_ID, 3, 32, 3, SALT),
            32,
            "73c486f82788495bd84a405fdf2ceb933a9defa5b94163185288ec041de30c26",
        ),
    ];
    for (params, size, expected) in &cases {
        let out = generate(params, b"password", *size).expect("generate should succeed");
        assert_eq!(hex_encode(&out), *expected, "{params:?}");
    }
    // Memory below 8 blocks is raised to 8 and an empty salt is the same as none.
    let tiny = params(ARGON2_I, 1, 2, 1, Some(b""));
    assert_eq!(
        hex_encode(&generate(&tiny, b"password", 64).expect("generate should succeed")),
        "caeb0932a019d47800a32fd9ff74389e0379281c6ae175643e4a7718b15f7f5b2dcf702a6fe95f217b1fbaf016b041a6cca8e29730f004afcbd68728cdc66bb5"
    );
    assert_eq!(
        generate(&params(ARGON2_I, 1, 2, 1, None), b"password", 64),
        generate(&tiny, b"password", 64)
    );
}

#[test]
fn argon2_version_10_differs() {
    let v13 = params(ARGON2_ID, 2, 16, 2, SALT);
    let v10 = Params {
        version: VERSION_10,
        ..v13.clone()
    };
    assert_ne!(
        generate(&v13, b"password", 32).expect("generate should succeed"),
        generate(&v10, b"password", 32).expect("generate should succeed")
    );
    // Passwords and salts matter, outputs have the requested size.
    assert_ne!(
        generate(&v13, b"password", 32).expect("generate should succeed"),
        generate(&v13, b"Password", 32).expect("generate should succeed")
    );
    assert_eq!(
        generate(&v13, b"password", 65)
            .expect("generate should succeed")
            .len(),
        65
    );
    assert_eq!(
        generate(&v13, b"password", 96)
            .expect("generate should succeed")
            .len(),
        96
    );
}

#[test]
fn argon2_rejects_invalid_parameters_like_bouncycastle() {
    let err = |params: &Params, size: i64| {
        generate(params, b"password", size)
            .expect_err("generate should fail")
            .message()
    };
    let ok = params(ARGON2_ID, 3, 32, 1, SALT);
    assert_eq!(err(&ok, 3), "output length less than 4");
    assert_eq!(err(&ok, 0), "output length less than 4");
    assert_eq!(err(&ok, -1), "-1");
    assert_eq!(
        err(&params(ARGON2_ID, 0, 32, 1, SALT), 32),
        "iterations is less than: 1"
    );
    assert_eq!(
        err(&params(ARGON2_ID, 3, 1, 1, SALT), 32),
        "memory is less than: 2 expected 2"
    );
    assert_eq!(
        err(&params(ARGON2_ID, 3, -1, 1, SALT), 32),
        "memory is less than: 2 expected 2"
    );
    assert_eq!(
        err(&params(ARGON2_ID, 3, 32, 0, SALT), 32),
        "lanes must be greater than 1"
    );
    assert_eq!(
        err(&params(ARGON2_ID, 3, 32, 16_777_217, SALT), 32),
        "lanes must be less than 16777216"
    );
    assert_eq!(
        err(&params(ARGON2_ID, 3, 32, 20, SALT), 32),
        "memory is less than: 40 expected 40"
    );
    // The size is checked before the other parameters, the iterations after the lanes.
    assert_eq!(err(&params(ARGON2_ID, 0, 32, 0, SALT), -5), "-5");
    assert_eq!(
        err(&params(ARGON2_ID, 0, 32, 0, SALT), 3),
        "lanes must be greater than 1"
    );
    assert_eq!(
        err(&params(ARGON2_ID, 0, 32, 1, SALT), 3),
        "iterations is less than: 1"
    );
    // Memory that cannot be allocated is reported like Java's `OutOfMemoryError`.
    assert_eq!(
        err(&params(ARGON2_ID, 1, i32::MAX, 1, SALT), 32),
        "Java heap space"
    );
}
