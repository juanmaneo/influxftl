// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the elliptic curves, against what Warp 10 3.5.0 (BouncyCastle) gives for the same
//! keys: the public keys, the secrets of an agreement, the signatures it makes (which must
//! verify here) and the keys its recovery finds.

use std::ops::RangeInclusive;

use num_bigint::{BigInt, BigUint};
use num_traits::Zero;
use sha2::{Digest, Sha256};

use crate::ec::{
    CURVE_NAMES, Curve, Point, PrivateKey, PublicKey, Signature, der_decode, der_encode,
    system_random,
};
use crate::encoding::hex_decode;
use crate::error::crypto::{EcError, PointError};

/// What Warp 10 gives for the private keys `d1 = 123456789012345678901234567890` and `d2 =
/// 987654321098765432109876543210`, and a signature of `SHA256` of `hello` by `d1`.
struct Vector {
    curve: Curve,
    /// The public key of `d1`.
    q: &'static str,
    /// The DER of the signature of `d1` (`SHA256WITHECDSA` of `hello`).
    signature: &'static str,
    /// The keys `ECRECOVER` finds for that signature.
    recovered: [&'static str; 2],
    /// The public key of `d2`.
    q2: &'static str,
    /// The secret of the agreement of `d1` and the public key of `d2`.
    ecdh: &'static str,
}

const VECTORS: [Vector; 4] = [
    Vector {
        curve: Curve::Secp256k1,
        q: "04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629",
        signature: "3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a",
        recovered: [
            "04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0",
            "04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629",
        ],
        q2: "047144f652502b58cfc688b7ef74de6399cf40e2f40849f9731312a0fa98507a4e02653c9fd6cbd019792a27ed4d2c32b2f2eb8b0f2aff8379f2f434858f86ad46",
        ecdh: "62ba2e7769e6f9d1eab9a5bce81b45e2a71018ea1fa00a7f57ae882be154499f",
    },
    Vector {
        curve: Curve::Secp256r1,
        q: "04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec",
        signature: "3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d",
        recovered: [
            "04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec",
            "04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d",
        ],
        q2: "048a47f31750420e5a720006ce3cd585207456942ff9ba404c6bb7d68694ea41b8e53688531cf5a6d8c94ce7efc49cf701369b02af70912e5e34eb4f33673ad711",
        ecdh: "b3ce002fa7b7bed224867b93c362f16226964e73d2419262d70058e606768798",
    },
    Vector {
        curve: Curve::Secp384r1,
        q: "04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a",
        signature: "3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176",
        recovered: [
            "04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a",
            "04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf",
        ],
        q2: "04d0d21d6e6da69877837f03c4529004fd6b7025de40508e9f44352d7a1f54551e3763f33617489f1a5fc5d0f6a18d4178a96dc3a232616f59e43adc9dc2b4801e50a238fcd4614fddfc82482593976f806bd8a29f6ef49ab7793a2638ae795d23",
        ecdh: "2aa0df998ecace4f5d77ea87c2cb9f1ab01a352811a3ce11e43bd5dbb353adeff38e488489262b9f7947298d2a12592b",
    },
    Vector {
        curve: Curve::Secp521r1,
        q: "0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d",
        signature: "3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61",
        recovered: [
            "0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f",
            "0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d",
        ],
        q2: "04003bf3a5e0ab8a7074b9720070bf4840f10e4a2bdcf5fc319370914d659c54a315e5ed37b6b29a72d17650d76570d33b535dfa5354d97f998c41194639479e25252c013d5e1370beb877cd04ac471e8265413c730e3b1f094c784dfa87ba1ca19501a9d56c961d2f5c02d00fb91507911705d320d854b03ab66c77e9f6b1569afe7b975e",
        ecdh: "54c9b52ba9ecd5fa88bbf33e0529adc0dfa0595c87c9405d64cfc30a1e85bf699df5b9968fd748128c481f11ebf9e25191acef682456b17ffd4b04732fd5e25084",
    },
];

fn d1() -> BigInt {
    "123456789012345678901234567890".parse().expect("a number")
}

fn d2() -> BigInt {
    "987654321098765432109876543210".parse().expect("a number")
}

/// The private number of a key.
fn scalar(d: &BigInt) -> BigUint {
    d.magnitude().clone()
}

fn bytes(hex: &str) -> Vec<u8> {
    hex_decode(hex).expect("hexadecimal")
}

fn hello() -> Vec<u8> {
    Sha256::digest(b"hello").as_slice().to_vec()
}

fn point(curve: Curve, hex: &str) -> Point {
    Point::decode(curve, &bytes(hex)).expect("a point of the curve")
}

/// A source of random bytes that gives `bytes` in turn, then fails.
fn scripted(bytes: Vec<u8>) -> impl FnMut(&mut [u8]) -> Result<(), getrandom::Error> {
    let mut offset = 0;
    move |buffer: &mut [u8]| {
        let chunk = bytes
            .get(offset..offset + buffer.len())
            .ok_or(getrandom::Error::UNSUPPORTED)?;
        buffer.copy_from_slice(chunk);
        offset += buffer.len();
        Ok(())
    }
}

/// `number` as `size` big-endian bytes.
fn padded(number: &BigUint, size: usize) -> Vec<u8> {
    let digits = number.to_bytes_be();
    let mut out = vec![0u8; size - digits.len()];
    out.extend(digits);
    out
}

#[test]
fn curves_are_found_by_name_without_regard_to_case() {
    for name in CURVE_NAMES {
        assert!(Curve::lookup(name).is_some(), "{name}");
        assert_eq!(Curve::lookup(name), Curve::lookup(&name.to_uppercase()));
    }
    assert_eq!(Curve::lookup("prime256v1"), Some(Curve::Secp256r1));
    assert_eq!(Curve::lookup("P-256"), Curve::lookup("secp256r1"));
    assert_eq!(Curve::lookup("p-384"), Some(Curve::Secp384r1));
    assert_eq!(Curve::lookup("P-521"), Some(Curve::Secp521r1));
    assert_eq!(Curve::lookup("curve25519"), None);
    assert_eq!(Curve::lookup(""), None);
}

#[test]
fn a_curve_has_its_size_and_its_order() {
    assert_eq!(
        Curve::Secp256k1.order(),
        BigUint::parse_bytes(
            b"FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141",
            16
        )
        .expect("a number")
    );
    for (curve, size, bits) in [
        (Curve::Secp256k1, 32, 256),
        (Curve::Secp256r1, 32, 256),
        (Curve::Secp384r1, 48, 384),
        (Curve::Secp521r1, 66, 521),
    ] {
        assert_eq!(curve.size(), size);
        assert_eq!(curve.order().bits(), bits);
    }
}

#[test]
fn public_keys_are_those_of_warp_10() {
    for vector in &VECTORS {
        let curve = vector.curve;
        assert_eq!(
            curve.public_key(&d1()).encode(),
            bytes(vector.q),
            "{curve:?}"
        );
        assert_eq!(
            curve.public_key(&d2()).encode(),
            bytes(vector.q2),
            "{curve:?}"
        );
    }
}

#[test]
fn the_multiples_of_the_order_have_no_public_key() {
    for vector in &VECTORS {
        let curve = vector.curve;
        let order = BigInt::from(curve.order());
        for d in [BigInt::from(0), order.clone(), -order.clone(), order * 3] {
            assert_eq!(curve.public_key(&d).encode(), [0], "{curve:?} {d}");
        }
        // `-1` is the largest private number.
        assert_eq!(
            curve.public_key(&BigInt::from(-1)),
            curve.public_key(&BigInt::from(1)).negate(),
            "{curve:?}"
        );
    }
}

#[test]
fn points_are_decoded_whatever_their_encoding() {
    for vector in &VECTORS {
        let curve = vector.curve;
        let full = bytes(vector.q);
        let size = curve.size();
        let mut compressed = vec![2 + (full[full.len() - 1] & 1)];
        compressed.extend(&full[1..=size]);
        let decoded = Point::decode(curve, &full).expect("a point");
        assert_eq!(Point::decode(curve, &compressed), Ok(decoded));
        assert_eq!(decoded.encode(), full);
        let (x, y) = decoded.coordinates();
        assert_eq!(padded(&x, size), full[1..=size]);
        assert_eq!(padded(&y, size), full[size + 1..]);
        // Not on the curve, not a point at all.
        let mut off_curve = full.clone();
        off_curve[full.len() - 1] ^= 1;
        assert_eq!(
            Point::decode(curve, &off_curve),
            Err(PointError::Coordinates)
        );
        assert_eq!(Point::decode(curve, &[]), Err(PointError::Empty));
        assert_eq!(
            Point::decode(curve, &full[..full.len() - 1]),
            Err(PointError::UncompressedLength)
        );
        // The point at infinity.
        let infinity = Point::decode(curve, &[0]).expect("the point at infinity");
        assert_eq!(infinity.encode(), [0]);
        assert_eq!(infinity.coordinates(), (BigUint::zero(), BigUint::zero()));
    }
}

#[test]
fn secrets_agree_as_they_do_in_warp_10() {
    for vector in &VECTORS {
        let curve = vector.curve;
        let expected = BigUint::from_bytes_be(&bytes(vector.ecdh));
        assert_eq!(
            curve.agree(&scalar(&d1()), &point(curve, vector.q2)),
            expected,
            "{curve:?}"
        );
        assert_eq!(
            curve.agree(&scalar(&d2()), &point(curve, vector.q)),
            expected,
            "{curve:?}"
        );
    }
}

fn signature(vector: &Vector) -> Signature {
    der_decode(&bytes(vector.signature)).expect("a signature")
}

#[test]
fn signatures_of_warp_10_verify() {
    for vector in &VECTORS {
        let curve = vector.curve;
        let q = point(curve, vector.q);
        let signature = signature(vector);
        assert!(curve.verify(&q, &hello(), &signature), "{curve:?}");
        assert_eq!(der_encode(&signature), bytes(vector.signature), "{curve:?}");
        // Another hash, another key.
        assert!(!curve.verify(&q, &Sha256::digest(b"hellp"), &signature));
        assert!(!curve.verify(&point(curve, vector.q2), &hello(), &signature));
    }
}

#[test]
fn signatures_out_of_range_do_not_verify() {
    let vector = &VECTORS[0];
    let curve = vector.curve;
    let q = point(curve, vector.q);
    let good = signature(vector);
    let order = curve.order();
    for bad in [BigUint::zero(), order.clone(), &order + 1u8] {
        let r = Signature {
            r: bad.clone(),
            s: good.s.clone(),
        };
        let s = Signature {
            r: good.r.clone(),
            s: bad,
        };
        assert!(!curve.verify(&q, &hello(), &r));
        assert!(!curve.verify(&q, &hello(), &s));
    }
}

#[test]
fn what_is_signed_verifies_on_every_curve() {
    for vector in &VECTORS {
        let curve = vector.curve;
        let q = curve.public_key(&d1());
        for hash in [hello(), vec![0xff; 20], vec![0x01; 64], vec![7]] {
            let mut random = system_random;
            let signature = curve
                .sign(&scalar(&d1()), &hash, &mut random)
                .expect("a signature");
            assert!(curve.verify(&q, &hash, &signature), "{curve:?} {hash:?}");
            assert_eq!(der_decode(&der_encode(&signature)), Ok(signature.clone()));
            assert!(!curve.verify(&q, &Sha256::digest(&hash), &signature));
        }
    }
}

#[test]
fn a_nonce_that_gives_a_null_s_is_replaced() {
    // With the nonce `1`, `r` is the abscissa of the generator; `s` is null for the private
    // number that makes `e + r d` a multiple of the order.
    let curve = Curve::Secp256k1;
    let order = curve.order();
    let hash = hello();
    let e = BigUint::from_bytes_be(&hash) % &order;
    let r = Point::base_mul(curve, &BigUint::from(1u8)).coordinates().0 % &order;
    let inverse = r.modpow(&(&order - 2u8), &order);
    let d = BigInt::from((&order - e) * inverse % &order);
    let mut random = scripted(
        [
            padded(&BigUint::from(1u8), 32),
            padded(&BigUint::from(2u8), 32),
        ]
        .concat(),
    );
    let signature = curve
        .sign(&scalar(&d), &hash, &mut random)
        .expect("a signature");
    let expected_r = Point::base_mul(curve, &BigUint::from(2u8)).coordinates().0 % &order;
    assert_eq!(signature.r, expected_r);
    assert!(curve.verify(&curve.public_key(&d), &hash, &signature));
}

#[test]
fn a_failing_source_of_random_bytes_fails_the_signature_and_the_generation() {
    let mut random = scripted(Vec::new());
    assert_eq!(
        Curve::Secp256k1.sign(&scalar(&d1()), &hello(), &mut random),
        Err(EcError::Entropy(getrandom::Error::UNSUPPORTED))
    );
    let mut random = scripted(Vec::new());
    assert_eq!(
        Curve::Secp256r1.generate(&mut random),
        Err(EcError::Entropy(getrandom::Error::UNSUPPORTED))
    );
}

#[test]
fn random_numbers_out_of_range_are_rejected() {
    for curve in [Curve::Secp256k1, Curve::Secp521r1] {
        let size = curve.size();
        // Nothing, one, and a number that is not below the order are drawn again.
        let mut valid = vec![0u8; size];
        valid[size - 1] = 5;
        let script = [
            vec![0; size],
            padded(&BigUint::from(1u8), size),
            vec![0xff; size],
            valid,
        ]
        .concat();
        let mut random = scripted(script);
        assert_eq!(
            curve.generate(&mut random),
            Ok(BigUint::from(5u8)),
            "{curve:?}"
        );
    }
}

#[test]
fn a_nonce_of_one_is_not_too_small() {
    // A private key of 1 is refused by `generate`; a nonce of 1 is not by `sign`.
    let curve = Curve::Secp256k1;
    let mut random = scripted(padded(&BigUint::from(1u8), 32));
    let signature = curve
        .sign(&scalar(&d1()), &hello(), &mut random)
        .expect("a signature");
    assert!(curve.verify(&curve.public_key(&d1()), &hello(), &signature));
    let mut random = scripted(
        [
            padded(&BigUint::from(1u8), 32),
            padded(&BigUint::from(1u8), 32),
        ]
        .concat(),
    );
    assert_eq!(
        curve.generate(&mut random),
        Err(EcError::Entropy(getrandom::Error::UNSUPPORTED)),
        "the number 1 is drawn again, and nothing is left"
    );
}

#[test]
fn the_first_byte_of_a_random_number_is_cut_to_the_bits_of_the_order() {
    // The order of `secp521r1` has 521 bits: 66 bytes, of which the first has one bit.
    let mut script = vec![0u8; 66];
    script[0] = 0xff;
    let mut random = scripted(script);
    let number = Curve::Secp521r1.generate(&mut random).expect("a number");
    assert_eq!(number, BigUint::from(1u8) << 520);
}

fn recovered(
    vector: &Vector,
    signature: &Signature,
    cofactors: RangeInclusive<i64>,
    parities: &[u8],
) -> Vec<Vec<u8>> {
    vector
        .curve
        .recover(
            &BigInt::from(signature.r.clone()),
            &BigInt::from(signature.s.clone()),
            &hello(),
            cofactors,
            parities,
        )
        .expect("candidates")
        .iter()
        .map(|candidate| candidate.key.encode())
        .collect()
}

#[test]
fn the_keys_of_a_signature_are_those_warp_10_recovers() {
    for vector in &VECTORS {
        let signature = signature(vector);
        let expected: Vec<Vec<u8>> = vector.recovered.iter().map(|hex| bytes(hex)).collect();
        // The cofactor is 1: `0` and `1` are the multiples of the order that are tried.
        assert_eq!(
            recovered(vector, &signature, 0..=1, &[2, 3]),
            expected,
            "{:?}",
            vector.curve
        );
        assert!(expected.contains(&bytes(vector.q)));
        let candidates = vector
            .curve
            .recover(
                &BigInt::from(signature.r.clone()),
                &BigInt::from(signature.s.clone()),
                &hello(),
                0..=0,
                &[2, 3],
            )
            .expect("candidates");
        assert_eq!(candidates.len(), 2);
        // `R` and its opposite: the same abscissa, opposite ordinates.
        assert_eq!(candidates[0].x, signature.r);
        assert_eq!(candidates[1].x, signature.r);
        // One parity gives both candidates, as the opposite of `R` has the other parity.
        assert_eq!(recovered(vector, &signature, 0..=0, &[2]).len(), 2);
        assert_eq!(recovered(vector, &signature, 0..=0, &[3]).len(), 2);
        assert_eq!(recovered(vector, &signature, 1..=1, &[2, 3]).len(), 0);
        assert_eq!(recovered(vector, &signature, 0..=1, &[]).len(), 0);
        assert_eq!(
            recovered(vector, &signature, RangeInclusive::new(1, 0), &[2]).len(),
            0
        );
    }
}

#[test]
fn an_abscissa_that_is_no_point_gives_no_candidate() {
    let vector = &VECTORS[0];
    let signature = signature(vector);
    // About half the abscissae are those of a point.
    let none = (0u8..32).any(|offset| {
        let r = BigInt::from(&signature.r + offset);
        vector
            .curve
            .recover(
                &r,
                &BigInt::from(signature.s.clone()),
                &hello(),
                0..=0,
                &[2],
            )
            .expect("candidates")
            .is_empty()
    });
    assert!(none);
    // A negative number is no abscissa.
    assert_eq!(
        vector
            .curve
            .recover(&BigInt::from(-5), &BigInt::from(5), &hello(), 0..=0, &[2])
            .expect("candidates")
            .len(),
        0
    );
}

#[test]
fn a_number_without_inverse_cannot_be_recovered() {
    let curve = Curve::Secp256r1;
    let order = BigInt::from(curve.order());
    for r in [BigInt::from(0), order.clone(), -order] {
        assert_eq!(
            curve.recover(&r, &BigInt::from(1), &hello(), 0..=1, &[2, 3]),
            Err(EcError::NotInvertible)
        );
    }
}

#[test]
fn keys_know_their_encoding_and_their_curve() {
    let vector = &VECTORS[1];
    let private = PrivateKey {
        curve: vector.curve,
        name: "P-256".to_owned(),
        d: d1(),
    };
    let public = private.public();
    assert_eq!(public.name, "P-256");
    assert_eq!(public.encoded, bytes(vector.q));
    assert_eq!(public.point.encode(), bytes(vector.q));
    let full = bytes(vector.q);
    let mut compressed = vec![2 + (full[full.len() - 1] & 1)];
    compressed.extend(&full[1..=32]);
    let from_compressed =
        PublicKey::new(vector.curve, "prime256v1", compressed.clone()).expect("a key");
    assert_eq!(from_compressed.encoded, compressed);
    assert_eq!(from_compressed.point, public.point);
    assert_eq!(
        PublicKey::new(vector.curve, "prime256v1", vec![4, 1, 2]),
        Err(PointError::UncompressedLength)
    );
}

fn malformed(der: &[u8]) {
    assert_eq!(
        der_decode(der),
        Err(EcError::MalformedSignature),
        "{der:02x?}"
    );
}

#[test]
fn a_signature_is_a_sequence_of_two_integers_in_der() {
    let signature = Signature {
        r: BigUint::from(1u8),
        s: BigUint::from(0x80u8),
    };
    assert_eq!(der_encode(&signature), [0x30, 7, 2, 1, 1, 2, 2, 0, 0x80]);
    assert_eq!(der_decode(&der_encode(&signature)), Ok(signature));
    // Long forms of the length: the sequence is longer than 127 bytes.
    let long = Signature {
        r: BigUint::from(1u8) << 520,
        s: BigUint::from(1u8) << 520,
    };
    let der = der_encode(&long);
    assert_eq!(&der[..3], [0x30, 0x81, 0x88]);
    assert_eq!(der_decode(&der), Ok(long));
    let longer = Signature {
        r: BigUint::from(1u8) << 2100,
        s: BigUint::from(3u8),
    };
    let der = der_encode(&longer);
    assert_eq!(&der[..2], [0x30, 0x82]);
    assert_eq!(der_decode(&der), Ok(longer));
    let zero = Signature {
        r: BigUint::zero(),
        s: BigUint::zero(),
    };
    assert_eq!(der_encode(&zero), [0x30, 6, 2, 1, 0, 2, 1, 0]);
}

#[test]
fn what_is_not_the_der_of_a_signature_is_refused() {
    malformed(&[]);
    malformed(&[0x30]);
    malformed(&[0x31, 6, 2, 1, 1, 2, 1, 1]);
    malformed(&[0x30, 6, 2, 1, 1, 2, 1]);
    malformed(&[0x30, 6, 2, 1, 1, 2, 1, 1, 0]);
    malformed(&[0x30, 5, 2, 1, 1, 2, 1, 1]);
    // Inside the sequence: something else than two integers.
    malformed(&[0x30, 3, 2, 1, 1]);
    malformed(&[0x30, 9, 2, 1, 1, 2, 1, 1, 2, 1, 1]);
    malformed(&[0x30, 6, 2, 1, 1, 3, 1, 1]);
    malformed(&[0x30, 6, 3, 1, 1, 2, 1, 1]);
    // Integers: empty, negative, not in the shortest form.
    malformed(&[0x30, 5, 2, 0, 2, 1, 1]);
    malformed(&[0x30, 6, 2, 1, 0x80, 2, 1, 1]);
    malformed(&[0x30, 7, 2, 2, 0, 1, 2, 1, 1]);
    malformed(&[0x30, 8, 2, 2, 0, 0x80, 2, 1, 1]);
    // Lengths: the shortest form only, at most four bytes, all of them there.
    malformed(&[0x30, 0x80, 6, 2, 1, 1, 2, 1, 1]);
    malformed(&[0x30, 0x81]);
    malformed(&[0x30, 0x81, 6, 2, 1, 1, 2, 1, 1]);
    malformed(&[0x30, 0x82, 0, 6, 2, 1, 1, 2, 1, 1]);
    malformed(&[0x30, 0x85, 0, 0, 0, 0, 6, 2, 1, 1, 2, 1, 1]);
}

#[test]
fn encodings_that_are_no_points_are_refused_as_bouncycastle_does() {
    let curve = Curve::Secp256k1;
    let generator = bytes(
        "0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8",
    );
    let with_tag = |tag: u8| {
        let mut encoded = generator.clone();
        encoded[0] = tag;
        encoded
    };
    let refused = |encoded: &[u8]| Point::decode(curve, encoded).expect_err("no point");
    assert_eq!(refused(&[]), PointError::Empty);
    assert_eq!(refused(&[0, 0]), PointError::InfinityLength);
    assert_eq!(refused(&[2]), PointError::CompressedLength);
    assert_eq!(refused(&[3; 34]), PointError::CompressedLength);
    assert_eq!(refused(&[4, 1]), PointError::UncompressedLength);
    assert_eq!(refused(&with_tag(6)[..64]), PointError::HybridLength);
    assert_eq!(refused(&with_tag(7)[..64]), PointError::HybridLength);
    for tag in [1u8, 5, 8, 0xff] {
        assert_eq!(refused(&with_tag(tag)), PointError::Encoding(tag));
        assert_eq!(refused(&[tag]), PointError::Encoding(tag));
    }
    assert_eq!(refused(&[5]).to_string(), "Invalid point encoding 0x5");
    assert_eq!(refused(&[0xff]).to_string(), "Invalid point encoding 0xff");
    // The ordinate of the generator is even: a hybrid point says so with the tag `06`.
    assert_eq!(
        Point::decode(curve, &with_tag(6)),
        Point::decode(curve, &generator)
    );
    assert_eq!(refused(&with_tag(7)), PointError::HybridParity);
    // An abscissa that is not below the prime, an abscissa that has no point, an ordinate that
    // does not go with its abscissa.
    let mut high_x = vec![2];
    high_x.extend([0xff; 32]);
    assert_eq!(
        refused(&high_x),
        PointError::FieldElement("SecP256K1FieldElement")
    );
    let mut no_point = vec![2];
    no_point.extend([0; 31]);
    no_point.push(5);
    assert_eq!(refused(&no_point), PointError::Compression);
    let mut high_xy = vec![4];
    high_xy.extend([0xff; 64]);
    assert_eq!(
        refused(&high_xy),
        PointError::FieldElement("SecP256K1FieldElement")
    );
    let mut high_y = generator.clone();
    high_y[33..].fill(0xff);
    assert_eq!(
        refused(&high_y),
        PointError::FieldElement("SecP256K1FieldElement")
    );
    let mut off_curve = generator;
    off_curve[64] = 0x99;
    assert_eq!(refused(&off_curve), PointError::Coordinates);
    assert_eq!(refused(&off_curve).to_string(), "Invalid point coordinates");
    // The classes of the fields of the other curves.
    for (curve, class, size) in [
        (Curve::Secp256r1, "SecP256R1FieldElement", 32),
        (Curve::Secp384r1, "SecP384R1FieldElement", 48),
        (Curve::Secp521r1, "SecP521R1FieldElement", 66),
    ] {
        let mut encoded = vec![2];
        encoded.extend(vec![0xff; size]);
        assert_eq!(
            Point::decode(curve, &encoded),
            Err(PointError::FieldElement(class))
        );
    }
}

#[test]
fn the_prime_of_a_field_is_the_modulus_of_its_coordinates() {
    // The largest coordinate below the prime is accepted as an abscissa, the prime is not.
    for curve in [
        Curve::Secp256k1,
        Curve::Secp256r1,
        Curve::Secp384r1,
        Curve::Secp521r1,
    ] {
        let prime = curve.prime();
        let mut encoded = vec![2];
        encoded.extend(padded(&prime, curve.size()));
        assert!(matches!(
            Point::decode(curve, &encoded),
            Err(PointError::FieldElement(_))
        ));
        encoded.truncate(1);
        encoded.extend(padded(&(prime - 1u8), curve.size()));
        assert!(!matches!(
            Point::decode(curve, &encoded),
            Err(PointError::FieldElement(_))
        ));
    }
}

#[test]
fn a_private_number_is_in_one_to_the_order_minus_one() {
    for vector in &VECTORS {
        let curve = vector.curve;
        let order = BigInt::from(curve.order());
        for d in [
            BigInt::from(0),
            BigInt::from(-1),
            order.clone(),
            order.clone() + 1,
        ] {
            assert_eq!(
                curve.private_scalar(&d),
                Err(EcError::ScalarInterval),
                "{curve:?} {d}"
            );
        }
        for d in [BigInt::from(1), order - 1, d1()] {
            assert_eq!(
                curve.private_scalar(&d),
                Ok(d.magnitude().clone()),
                "{curve:?} {d}"
            );
        }
    }
    assert_eq!(
        EcError::ScalarInterval.to_string(),
        "Scalar is not in the interval [1, n - 1]"
    );
}
