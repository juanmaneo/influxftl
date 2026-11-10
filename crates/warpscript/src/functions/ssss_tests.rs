// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of `->SSSS` and `SSSS->`: the messages are those of Warp 10 3.5.0, the shares that
//! `SSSS->` recovers were made or recovered by it.

use super::ssss::tossss_with;
use crate::error::Error;
use crate::stack::Stack;
use crate::tests::{assert_fails, assert_top, top};
use crate::value::Value;

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

/// `->SSSS` of `secret` in `shares` shares of which `needed` are needed, with the given
/// source of randomness and the flag `ensure_randomness` when there is one.
fn split_with(
    secret: &[u8],
    shares: i64,
    needed: i64,
    ensure_randomness: Option<bool>,
    random: &mut dyn FnMut() -> Result<u8, getrandom::Error>,
) -> Result<Vec<Vec<u8>>, Error> {
    let mut stack = Stack::new();
    stack.push(Value::bytes(secret));
    stack.push(Value::Long(shares));
    stack.push(Value::Long(needed));
    if let Some(ensure) = ensure_randomness {
        stack.push(Value::Boolean(ensure));
    }
    tossss_with(&mut stack, random)?;
    let Value::List(list) = stack.pop()? else {
        panic!("->SSSS should leave a list");
    };
    let shares = list
        .borrow()
        .iter()
        .map(|share| match share {
            Value::Bytes(bytes) => bytes.to_vec(),
            other => panic!("{other:?} is not a byte array"),
        })
        .collect();
    assert_eq!(stack.depth(), 0);
    Ok(shares)
}

/// Randomness for a 1 byte secret in 3 shares of which 2 are needed: the leading coefficient 7
/// (after a refused 0), then the abscissae of the share (0 with the pair `(0, 9)`, 5), (6)
/// and (200).
const ONE_BYTE: [u8; 8] = [0, 7, 0, 9, 5, 5, 6, 200];

#[test]
fn zero_abscissae_are_kept_by_default() {
    let shares = split_with(&[0x42], 3, 2, None, &mut scripted(&ONE_BYTE)).expect("it splits");
    assert_eq!(
        shares,
        vec![vec![0, 9, 5, 0x59], vec![6, 0x50], vec![200, 0]]
    );
    let shares =
        split_with(&[0x42], 3, 2, Some(true), &mut scripted(&ONE_BYTE)).expect("it splits");
    assert_eq!(
        shares,
        vec![vec![0, 9, 5, 0x59], vec![6, 0x50], vec![200, 0]]
    );
}

#[test]
fn zero_abscissae_are_dropped_without_the_randomness_guarantee() {
    let shares =
        split_with(&[0x42], 3, 2, Some(false), &mut scripted(&ONE_BYTE)).expect("it splits");
    assert_eq!(shares, vec![vec![5, 0x59], vec![6, 0x50], vec![200, 0]]);
}

#[test]
fn entropy_failures_are_reported() {
    let err = split_with(&[0x42], 3, 2, None, &mut scripted(&[])).expect_err("no entropy");
    assert_eq!(
        err.to_string(),
        format!(
            "Unable to obtain random data from the operating system ({}).",
            getrandom::Error::UNSUPPORTED
        )
    );
}

#[test]
fn splits_with_the_system_randomness() {
    for flag in ["", "T", "F"] {
        let script = format!(
            "'The quick brown fox' 'UTF-8' ->BYTES 7 4 {flag} ->SSSS \
             <% DROP SIZE %> LMAP"
        );
        let Value::List(sizes) = top(&script) else {
            panic!("a list of sizes");
        };
        let sizes: Vec<_> = sizes.borrow().clone();
        assert_eq!(sizes.len(), 7);
        for size in sizes {
            let Value::Long(size) = size else {
                panic!("a size");
            };
            // 19 bytes: 38 bytes, and 2 more for each abscissa 0 drawn without the flag `F`.
            assert!(size >= 38 && size % 2 == 0, "{size}");
            if flag == "F" {
                assert_eq!(size, 38);
            }
        }
    }
}

#[test]
fn round_trips_through_the_script_functions() {
    assert_top(
        "'a secret' 'UTF-8' ->BYTES 5 3 ->SSSS 1 3 SUBLIST SSSS-> 'UTF-8' BYTES->",
        "a secret",
    );
    assert_top(
        "'a secret' 'UTF-8' ->BYTES 255 255 F ->SSSS SSSS-> 'UTF-8' BYTES->",
        "a secret",
    );
    assert_top("'' 'UTF-8' ->BYTES 2 2 ->SSSS SIZE", 2i64);
    assert_top("'' 'UTF-8' ->BYTES 2 2 ->SSSS SSSS-> SIZE", 0i64);
}

#[test]
fn recovers_the_shares_of_warp_10() {
    let recover = |shares: &str| top(&format!("[ {shares} ] <% DROP HEX-> %> LMAP SSSS-> ->HEX"));
    assert_eq!(recover("'0650' 'c800'"), Value::string("42"));
    assert_eq!(recover("'00090559' '0650'"), Value::string("42"));
    assert_eq!(
        recover("'0116015e' '023202fb' '03340385'"),
        Value::string("1020")
    );
    assert_eq!(recover("'0116015e' '03340385'"), Value::string("07bd"));
    assert_eq!(recover("'0116015e' '0650'"), Value::string("0c"));
    assert_eq!(recover("'0116015e' '02'"), Value::string(""));
    assert_eq!(recover("''"), Value::string(""));
}

#[test]
fn shares_that_cannot_combine_give_null() {
    assert_top(
        "[ '0116015e' '0116015e' ] <% DROP HEX-> %> LMAP SSSS->",
        Value::Null,
    );
    assert_top(
        "[ '0650' '00000650' ] <% DROP HEX-> %> LMAP SSSS->",
        Value::Null,
    );
}

#[test]
fn no_share_is_a_null_pointer() {
    assert_fails("[ ] SSSS->", "(NullPointerException)");
}

#[test]
fn ssss_arguments_are_checked() {
    let bytes = "'abc' 'UTF-8' ->BYTES";
    let cases = [
        (
            format!("{bytes} 3 1 ->SSSS"),
            "->SSSS expects the number of needed splits to be between 2 and 255.",
        ),
        (
            format!("{bytes} 3 256 ->SSSS"),
            "->SSSS expects the number of needed splits to be between 2 and 255.",
        ),
        (
            format!("{bytes} 3 0 ->SSSS"),
            "->SSSS expects the number of needed splits to be between 2 and 255.",
        ),
        (
            format!("{bytes} 3 -3 ->SSSS"),
            "->SSSS expects the number of needed splits to be between 2 and 255.",
        ),
        (
            format!("{bytes} 256 3 ->SSSS"),
            "->SSSS the number of splits must be strictly less than 256.",
        ),
        (
            format!("{bytes} 2 3 ->SSSS"),
            "->SSSS the number of splits to generate must be at least equal to the number of needed splits.",
        ),
        (
            format!("{bytes} -5 2 ->SSSS"),
            "->SSSS the number of splits to generate must be at least equal to the number of needed splits.",
        ),
        (
            format!("{bytes} 3 'a' ->SSSS"),
            "->SSSS expects the number of splits needed to reconstruct the input.",
        ),
        (
            format!("{bytes} 3 2 'x' ->SSSS"),
            "->SSSS expects the number of splits needed to reconstruct the input.",
        ),
        (
            format!("{bytes} 3 'x' T ->SSSS"),
            "->SSSS expects the number of splits needed to reconstruct the input.",
        ),
        (
            format!("{bytes} 3 2 T F ->SSSS"),
            "->SSSS expects the number of splits needed to reconstruct the input.",
        ),
        (
            format!("{bytes} 'x' 2 ->SSSS"),
            "->SSSS expects the number of splits to generate.",
        ),
        ("3 2 ->SSSS".to_owned(), "Empty stack."),
        (
            "'abc' 3 2 ->SSSS".to_owned(),
            "->SSSS operates on a byte array.",
        ),
        (format!("{bytes} 5 4294967296 ->SSSS"), "integer overflow"),
        (format!("{bytes} 4294967296 2 ->SSSS"), "integer overflow"),
        ("2 ->SSSS".to_owned(), "Empty stack."),
        ("T ->SSSS".to_owned(), "Empty stack."),
        ("->SSSS".to_owned(), "Empty stack."),
        (
            "[ 1 ] SSSS->".to_owned(),
            "SSSS-> operates on a LIST of BYTES (byte arrays).",
        ),
        (
            "1 SSSS->".to_owned(),
            "SSSS-> operates on a LIST of BYTES (byte arrays).",
        ),
        ("SSSS->".to_owned(), "Empty stack."),
    ];
    for (script, message) in cases {
        assert_fails(&script, &format!("({message})"));
    }
    assert_fails("[ 'a' ] SSSS->", "SSSS-> operates on a LIST of BYTES");
}
