// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the secret sharing: the arithmetic of GF(256) against a bitwise multiplication,
//! hand-computed splits driven by scripted random bytes, and recoveries of shares that the
//! OSS implementation of Warp 10 3.5.0 made or recovered.

use crate::encoding::hex_decode;
use crate::error::crypto::{RecoverError, SplitError};
use crate::jrandom::JavaRandom;
use crate::ssss::{Field, MAX_SHARES, recover, split};

/// The product of two elements of GF(256) by shifts and reductions, without tables.
fn slow_mul(mut a: u8, mut b: u8) -> u8 {
    let mut product = 0;
    while b != 0 {
        if b & 1 != 0 {
            product ^= a;
        }
        let carry = a & 0x80 != 0;
        a <<= 1;
        if carry {
            a ^= 0x1d;
        }
        b >>= 1;
    }
    product
}

/// A source that delivers `bytes`, then fails.
fn scripted(bytes: &[u8]) -> impl FnMut() -> Result<u8, getrandom::Error> + '_ {
    let mut remaining = bytes.iter();
    move || {
        remaining
            .next()
            .copied()
            .ok_or(getrandom::Error::UNSUPPORTED)
    }
}

/// An endless reproducible source.
fn seeded(seed: i64) -> impl FnMut() -> Result<u8, getrandom::Error> {
    let mut generator = JavaRandom::new(seed);
    move || Ok(generator.next_int() as u8)
}

fn hex(text: &str) -> Vec<u8> {
    hex_decode(text).expect("the test vectors are hexadecimal")
}

#[test]
fn multiplication_matches_the_bitwise_product() {
    let field = Field::default();
    for a in 0..=255u8 {
        for b in 0..=255u8 {
            assert_eq!(field.mul(a, b), slow_mul(a, b), "{a} * {b}");
        }
    }
}

#[test]
fn inverse_gives_one() {
    let field = Field::new();
    for a in 1..=255u8 {
        let a = core::num::NonZeroU8::new(a).expect("a is not zero");
        assert_eq!(field.mul(a.get(), field.inverse(a)), 1, "{a}");
    }
}

#[test]
fn evaluation_follows_horner() {
    let field = Field::new();
    assert_eq!(field.evaluate(&[5, 3, 7], 2), 0x1f);
    assert_eq!(field.evaluate(&[5, 3, 7], 0), 5);
    assert_eq!(field.evaluate(&[], 9), 0);
}

#[test]
fn split_with_scripted_randomness() {
    // Degree 1, constant 0x42, leading coefficient 7 (0 is refused first); the first share
    // draws the abscissa 0 (a `(0, 9)` pair is inserted) then 5, the second draws 5 again
    // (taken) then 6, the third 200, whose ordinate is 0.
    let mut random = scripted(&[0, 7, 0, 9, 5, 5, 6, 200]);
    let shares = split(&[0x42], 3, 2, &mut random).expect("the split should succeed");
    assert_eq!(
        shares,
        vec![vec![0, 9, 5, 0x59], vec![6, 0x50], vec![200, 0],]
    );
    for pair in [[0, 1], [0, 2], [1, 2]] {
        let chosen: Vec<_> = pair.iter().map(|&i| shares[i].clone()).collect();
        assert_eq!(recover(&chosen).expect("two shares recover"), vec![0x42]);
    }
    assert_eq!(recover(&shares).expect("all shares recover"), vec![0x42]);
}

#[test]
fn split_with_a_random_inner_coefficient() {
    // Degree 2: the coefficients are 0x10, 11 and 13 (0 refused), then 0x20, 0xfe and 0x80.
    let mut random = scripted(&[11, 0, 13, 1, 2, 3, 0xfe, 0x80, 1, 2, 3]);
    let shares = split(&[0x10, 0x20], 3, 3, &mut random).expect("the split should succeed");
    assert_eq!(
        shares,
        vec![
            vec![1, 0x16, 1, 0x5e],
            vec![2, 0x32, 2, 0xfb],
            vec![3, 0x34, 3, 0x85],
        ]
    );
    assert_eq!(recover(&shares).expect("shares recover"), vec![0x10, 0x20]);
}

#[test]
fn split_of_nothing_makes_empty_shares() {
    let mut random = scripted(&[]);
    assert_eq!(
        split(&[], 3, 2, &mut random).expect("the split should succeed"),
        vec![Vec::<u8>::new(); 3]
    );
}

#[test]
fn split_refuses_what_cannot_be_split() {
    for (shares, needed) in [(1, 2), (256, 2), (3, 1), (3, 4), (0, 0)] {
        assert_eq!(
            split(&[1], shares, needed, &mut seeded(1)),
            Err(SplitError::InvalidParameters { shares, needed })
        );
    }
    assert_eq!(
        SplitError::InvalidParameters {
            shares: 1,
            needed: 2
        }
        .to_string(),
        "cannot split a secret in 1 shares of which 2 are needed: expected 2 <= needed <= shares <= 255"
    );
}

#[test]
fn split_reports_the_entropy_failures() {
    let entropy = SplitError::Entropy(getrandom::Error::UNSUPPORTED);
    let failure = Err(entropy.clone());
    // The inner coefficient, the leading coefficient, an abscissa, the ordinate of a zero pair.
    assert_eq!(split(&[1], 3, 3, &mut scripted(&[])), failure);
    assert_eq!(split(&[1], 3, 3, &mut scripted(&[5])), failure);
    assert_eq!(split(&[1], 3, 3, &mut scripted(&[5, 9])), failure);
    assert_eq!(split(&[1], 3, 3, &mut scripted(&[5, 9, 0])), failure);
    assert_eq!(
        entropy.to_string(),
        format!(
            "Unable to obtain random data from the operating system ({}).",
            getrandom::Error::UNSUPPORTED
        )
    );
}

#[test]
fn every_subset_recovers() {
    let secret: Vec<u8> = (0..=255).collect();
    for (shares, needed) in [(2, 2), (3, 2), (5, 3), (7, 7), (8, 4)] {
        let all = split(&secret, shares, needed, &mut seeded(7)).expect("the split succeeds");
        assert_eq!(all.len(), shares);
        // Every window of `needed` consecutive shares, wrapping around.
        for start in 0..shares {
            let chosen: Vec<_> = (0..needed)
                .map(|offset| all[(start + offset) % shares].clone())
                .collect();
            assert_eq!(recover(&chosen).expect("the subset recovers"), secret);
        }
    }
}

#[test]
fn too_few_shares_recover_something_else() {
    let secret = b"a secret that needs three shares".to_vec();
    let all = split(&secret, 4, 3, &mut seeded(3)).expect("the split succeeds");
    assert_ne!(recover(&all[..2]).expect("two shares combine"), secret);
}

#[test]
fn the_maximum_number_of_shares_uses_every_abscissa() {
    let secret = b"all the abscissae".to_vec();
    let all = split(&secret, MAX_SHARES, 200, &mut seeded(11)).expect("the split succeeds");
    assert_eq!(all.len(), 255);
    assert_eq!(recover(&all[10..210]).expect("the subset recovers"), secret);
    // Each byte used all the 255 non-zero abscissae once.
    let mut abscissae: Vec<u8> = Vec::new();
    for share in &all {
        let mut points = share.as_chunks::<2>().0.iter().filter(|pair| pair[0] != 0);
        abscissae.push(points.next().expect("one point per byte")[0]);
    }
    abscissae.sort_unstable();
    abscissae.dedup();
    assert_eq!(abscissae.len(), 255);
}

#[test]
fn recover_oss_shares() {
    let hex_shares = [
        "74b80810d054601762d1466667df631d9f8b34ef84022bb8161de1b347b53656c768bc86cbe64b7a23c0f12776c4be46b7171345a034da7cc171aa2d1088d25f7f2b91dbdb7f62aa00aa15f968823eee10d1ff17c851762a",
        "e1c3960059d1af635649770d1002fdea2104e7e160479a8612af6737629a9e76e452750f3ddb4318e1f40b0bc8255d50c621e40e8fad22931ac6b42bdd2cd69a6dff252998308f56c9fd6bbae3a9e59793bec7bd478b",
        "bda35f1c8ad4ef380c098140d66fb8f0ee7fe9bd20737f4c6dd2899b9324063dd4997c2ce8da6811df24858cc10a1985efd52760d6e44935ce82cdd6e19c769c4126f359209490e321f8c726a624bc002a7a485f9d04",
        "1ab3c055680cbe37b7231a634aea00c7e69a6ff01ab03ab163505599596abecf22f14ffacc6db3d8aa05087ad03deb6f3d83675b3921080f9425330872e23c903f31e21dd3fdc1fb1a5fba7cba9ec5a12b7e0a978ea58c29",
    ];
    let shares: Vec<Vec<u8>> = hex_shares.iter().map(|share| hex(share)).collect();
    // Two of them carry a `(0, random)` pair: 88 bytes for a 43 bytes secret.
    assert_eq!(
        shares.iter().map(Vec::len).collect::<Vec<_>>(),
        [88, 86, 86, 88]
    );
    for subset in [[0, 1, 2], [1, 2, 3], [3, 0, 1], [2, 3, 0], [0, 1, 2]] {
        let chosen: Vec<_> = subset.iter().map(|&i| shares[i].clone()).collect();
        assert_eq!(
            recover(&chosen).expect("three shares recover"),
            b"The quick brown fox jumps over the lazy dog"
        );
    }
    let all = recover(&shares).expect("four shares recover");
    assert_eq!(all, b"The quick brown fox jumps over the lazy dog");
}

#[test]
fn recover_matches_the_oss_reference() {
    let cases: &[(&[&str], &str)] = &[
        (&["00090559", "0650"], "42"),
        (&["00090559", "c800"], "42"),
        (&["0650", "c800"], "42"),
        (&["00090559", "0650", "c800"], "42"),
        (&["0116015e", "023202fb"], "0a3d"),
        (&["0116015e", "023202fb", "03340385"], "1020"),
        (&["0116015e", "03340385", "023202fb"], "1020"),
        (&["0116015e", "03340385"], "07bd"),
        // One point from the first share, the second one has a single one: it stops there.
        (&["0116015e", "0650"], "0c"),
        (&["0116015e", "02"], ""),
        (&["0116", "023202fb"], "0a"),
        (&["0116015e", ""], ""),
        (&["", "0650"], ""),
        // The pairs whose abscissa is 0 are skipped.
        (&["0000", "0650"], ""),
        (&["00", "0650"], ""),
        (&["000005"], ""),
        (&["0000"], ""),
        // One share: its ordinates.
        (&["01"], ""),
        (&["0102"], "02"),
        (&[""], ""),
    ];
    for (shares, expected) in cases {
        let shares: Vec<Vec<u8>> = shares.iter().map(|share| hex(share)).collect();
        assert_eq!(recover(&shares), Ok(hex(expected)), "{shares:?}");
    }
}

#[test]
fn recover_refuses_what_cannot_combine() {
    assert_eq!(recover::<Vec<u8>>(&[]), Err(RecoverError::NoShares));
    assert_eq!(
        recover(&[hex("0116015e"), hex("0116015e")]),
        Err(RecoverError::DuplicateAbscissa)
    );
    // The abscissa is shared once the pairs of abscissa 0 are skipped.
    assert_eq!(
        recover(&[hex("0650"), hex("00000650")]),
        Err(RecoverError::DuplicateAbscissa)
    );
    // The second byte is the one that repeats.
    assert_eq!(
        recover(&[hex("01020304"), hex("05060307")]),
        Err(RecoverError::DuplicateAbscissa)
    );
    assert_eq!(
        RecoverError::NoShares.to_string(),
        "no share to recover the secret from"
    );
    assert_eq!(
        RecoverError::DuplicateAbscissa.to_string(),
        "duplicate abscissa: the shares cannot be combined"
    );
}
