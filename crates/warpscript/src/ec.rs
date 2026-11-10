// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Elliptic curve cryptography of the `EC*` functions: key pairs, ECDSA signatures, the
//! recovery of the public keys of a signature and the Diffie-Hellman agreement, on the named
//! curves `secp256k1`, `secp256r1`, `secp384r1` and `secp521r1` (BouncyCastle has many more).
//!
//! The curves come from the RustCrypto crates: every operation that involves a secret (the
//! public key of a private key, the `s` of a signature, the agreement) is theirs, in constant
//! time. What is built here is what BouncyCastle does around them: the numbers read as
//! `BigInteger`s, the truncation of the hash, the random nonce of a signature, the DER of a
//! signature and the candidates of the recovery.

use std::ops::RangeInclusive;

use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use num_traits::Zero;

use crate::error::crypto::{EcError, PointError};

/// A named curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    /// `secp256k1`.
    Secp256k1,
    /// `secp256r1`, `prime256v1`, `P-256`.
    Secp256r1,
    /// `secp384r1`, `P-384`.
    Secp384r1,
    /// `secp521r1`, `P-521`.
    Secp521r1,
}

/// The names of the curves, in the order the error messages list them.
pub const CURVE_NAMES: [&str; 8] = [
    "secp256k1",
    "secp256r1",
    "prime256v1",
    "P-256",
    "secp384r1",
    "P-384",
    "secp521r1",
    "P-521",
];

/// A source of random bytes, that a test replaces.
pub type Random<'a> = &'a mut dyn FnMut(&mut [u8]) -> Result<(), getrandom::Error>;

/// The random bytes of the operating system.
///
/// # Errors
///
/// The operating system cannot give entropy.
pub fn system_random(bytes: &mut [u8]) -> Result<(), getrandom::Error> {
    getrandom::fill(bytes)
}

/// The operations of a curve on its points and on numbers read modulo the order.
macro_rules! backend {
    ($module:ident, $krate:ident, $curve:ty, $size:expr) => {
        mod $module {
            use elliptic_curve::bigint::Reduce;
            use elliptic_curve::ff::{Field, PrimeField};
            use elliptic_curve::group::Group;
            use elliptic_curve::sec1::{FromSec1Point, ToSec1Point};
            use num_bigint::BigUint;
            use $krate::{AffinePoint, FieldBytes, ProjectivePoint, Scalar};

            /// The size of a coordinate or a scalar, in bytes.
            pub(super) const SIZE: usize = $size;

            /// The order of the group of points.
            pub(super) fn order() -> BigUint {
                BigUint::from_bytes_be(
                    <$curve as elliptic_curve::Curve>::ORDER
                        .as_ref()
                        .to_be_bytes()
                        .as_ref(),
                )
            }

            /// A number as a scalar, modulo the order.
            fn scalar(number: &BigUint) -> Scalar {
                let bytes = (number % order()).to_bytes_be();
                let mut repr = FieldBytes::default();
                // A number below the order has no more than `SIZE` bytes.
                repr.as_mut_slice()[SIZE - bytes.len()..].copy_from_slice(&bytes);
                <Scalar as Reduce<FieldBytes>>::reduce(&repr)
            }

            /// `k * G`.
            pub(super) fn base_mul(k: &BigUint) -> ProjectivePoint {
                ProjectivePoint::generator() * scalar(k)
            }

            /// `k * P`.
            pub(super) fn mul(point: &ProjectivePoint, k: &BigUint) -> ProjectivePoint {
                *point * scalar(k)
            }

            /// `base * G + k * P`.
            pub(super) fn combine(
                point: &ProjectivePoint,
                base: &BigUint,
                k: &BigUint,
            ) -> ProjectivePoint {
                base_mul(base) + mul(point, k)
            }

            /// The point of an encoding (compressed or not, hybrid ones are refused).
            ///
            /// The curve crates say nothing of why an encoding is refused (their error has no
            /// content): [`super::Point::decode`] tells why from the encoding.
            pub(super) fn decode(encoded: &[u8]) -> Option<ProjectivePoint> {
                AffinePoint::from_sec1_bytes(encoded)
                    .map(ProjectivePoint::from)
                    .ok()
            }

            /// The point encoded in full (`00` for the point at infinity).
            pub(super) fn encode(point: &ProjectivePoint) -> Vec<u8> {
                point.to_affine().to_sec1_point(false).as_bytes().to_vec()
            }

            /// The `s` of a signature, `k^-1 (e + r d)` modulo the order, for the nonce `k` and
            /// the private key `d`. The inverse is the power `k^(order - 2)`: the exponent is
            /// public, the nonce goes through constant time operations only.
            pub(super) fn sign_s(d: &BigUint, k: &BigUint, e: &BigUint, r: &BigUint) -> BigUint {
                let exponent = (order() - 2u8).to_u64_digits();
                let inverse = <Scalar as Field>::pow_vartime(&scalar(k), exponent);
                let s = inverse * (scalar(e) + scalar(r) * scalar(d));
                BigUint::from_bytes_be(s.to_repr().as_slice())
            }
        }
    };
}

backend!(secp256k1, k256, k256::Secp256k1, 32);
backend!(secp256r1, p256, p256::NistP256, 32);
backend!(secp384r1, p384, p384::NistP384, 48);
backend!(secp521r1, p521, p521::NistP521, 66);

/// A point of a curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Point {
    /// A point of `secp256k1`.
    Secp256k1(k256::ProjectivePoint),
    /// A point of `secp256r1`.
    Secp256r1(p256::ProjectivePoint),
    /// A point of `secp384r1`.
    Secp384r1(p384::ProjectivePoint),
    /// A point of `secp521r1`.
    Secp521r1(p521::ProjectivePoint),
}

/// A point made from the point of a curve and an operation of its backend.
macro_rules! point_of {
    ($curve:expr, $function:ident($($argument:expr),*)) => {
        match $curve {
            Curve::Secp256k1 => Point::Secp256k1(secp256k1::$function($($argument),*)),
            Curve::Secp256r1 => Point::Secp256r1(secp256r1::$function($($argument),*)),
            Curve::Secp384r1 => Point::Secp384r1(secp384r1::$function($($argument),*)),
            Curve::Secp521r1 => Point::Secp521r1(secp521r1::$function($($argument),*)),
        }
    };
}

/// The result of an operation of the backend of the curve of a point, on that point.
macro_rules! on_point {
    ($point:expr, $function:ident($($argument:expr),*) $(=> $wrap:ident)?) => {
        match $point {
            Point::Secp256k1(point) => on_point!(@wrap $($wrap)? Secp256k1, secp256k1::$function(point, $($argument),*)),
            Point::Secp256r1(point) => on_point!(@wrap $($wrap)? Secp256r1, secp256r1::$function(point, $($argument),*)),
            Point::Secp384r1(point) => on_point!(@wrap $($wrap)? Secp384r1, secp384r1::$function(point, $($argument),*)),
            Point::Secp521r1(point) => on_point!(@wrap $($wrap)? Secp521r1, secp521r1::$function(point, $($argument),*)),
        }
    };
    (@wrap point $variant:ident, $value:expr) => { Point::$variant($value) };
    (@wrap $variant:ident, $value:expr) => { $value };
}

impl Point {
    /// The point `k * G` of a curve.
    pub fn base_mul(curve: Curve, k: &BigUint) -> Point {
        point_of!(curve, base_mul(k))
    }

    /// The point of an encoding, which must be on the curve, as BouncyCastle decodes it:
    /// the point at infinity (`00`), a compressed point (`02`, `03`), an uncompressed one
    /// (`04`) or a hybrid one (`06`, `07`).
    ///
    /// # Errors
    ///
    /// An encoding that is not a point of the curve.
    pub fn decode(curve: Curve, encoded: &[u8]) -> Result<Point, PointError> {
        let size = curve.size();
        let (&tag, body) = encoded.split_first().ok_or(PointError::Empty)?;
        let canonical = |encoded: &[u8], error: PointError| {
            match curve {
                Curve::Secp256k1 => secp256k1::decode(encoded).map(Point::Secp256k1),
                Curve::Secp256r1 => secp256r1::decode(encoded).map(Point::Secp256r1),
                Curve::Secp384r1 => secp384r1::decode(encoded).map(Point::Secp384r1),
                Curve::Secp521r1 => secp521r1::decode(encoded).map(Point::Secp521r1),
            }
            .ok_or(error)
        };
        // A coordinate must be below the prime of the field.
        let below_prime = |coordinate: &[u8]| {
            if BigUint::from_bytes_be(coordinate) < curve.prime() {
                Ok(())
            } else {
                Err(PointError::FieldElement(curve.field_element()))
            }
        };
        match tag {
            0 if body.is_empty() => canonical(encoded, PointError::Coordinates),
            0 => Err(PointError::InfinityLength),
            2 | 3 if body.len() == size => {
                below_prime(body)?;
                canonical(encoded, PointError::Compression)
            }
            2 | 3 => Err(PointError::CompressedLength),
            4 | 6 | 7 if body.len() == 2 * size => {
                let (x, y) = body.split_at(size);
                // The tag of a hybrid point is the parity of its ordinate.
                if tag != 4 && (y[size - 1] & 1 == 1) != (tag == 7) {
                    return Err(PointError::HybridParity);
                }
                below_prime(x)?;
                below_prime(y)?;
                let mut uncompressed = encoded.to_vec();
                uncompressed[0] = 4;
                canonical(&uncompressed, PointError::Coordinates)
            }
            4 => Err(PointError::UncompressedLength),
            6 | 7 => Err(PointError::HybridLength),
            other => Err(PointError::Encoding(other)),
        }
    }

    /// The point encoded in full: `04`, the abscissa and the ordinate (`00` for the point at
    /// infinity).
    pub fn encode(&self) -> Vec<u8> {
        on_point!(self, encode())
    }

    /// `k * P`.
    #[must_use]
    pub fn mul(&self, k: &BigUint) -> Point {
        on_point!(self, mul(k) => point)
    }

    /// `base * G + k * P`.
    #[must_use]
    pub fn combine(&self, base: &BigUint, k: &BigUint) -> Point {
        on_point!(self, combine(base, k) => point)
    }

    /// The point opposite to this one.
    #[must_use]
    pub fn negate(&self) -> Point {
        match self {
            Point::Secp256k1(point) => Point::Secp256k1(-*point),
            Point::Secp256r1(point) => Point::Secp256r1(-*point),
            Point::Secp384r1(point) => Point::Secp384r1(-*point),
            Point::Secp521r1(point) => Point::Secp521r1(-*point),
        }
    }

    /// The abscissa and the ordinate (zero for the point at infinity).
    pub fn coordinates(&self) -> (BigUint, BigUint) {
        let encoded = self.encode();
        let (x, y) = encoded[1..].split_at((encoded.len() - 1) / 2);
        (BigUint::from_bytes_be(x), BigUint::from_bytes_be(y))
    }
}

/// A number as big-endian bytes.
fn bytes(number: &BigUint) -> Vec<u8> {
    number.to_bytes_be()
}

/// `number mod order`, as a non-negative number.
fn reduce(number: &BigInt, order: &BigUint) -> BigUint {
    number
        .mod_floor(&BigInt::from(order.clone()))
        .magnitude()
        .clone()
}

/// A signature of ECDSA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    /// The `r` of the signature.
    pub r: BigUint,
    /// The `s` of the signature.
    pub s: BigUint,
}

/// A candidate of the recovery of the public key of a signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The public key.
    pub key: Point,
    /// The abscissa of the point `R` of the signature the key comes from.
    pub x: BigUint,
    /// The ordinate of the point `R`.
    pub y: BigUint,
}

impl Curve {
    /// The curve a name designates (without regard to case), if this runtime has it.
    pub fn lookup(name: &str) -> Option<Curve> {
        match name.to_ascii_lowercase().as_str() {
            "secp256k1" => Some(Curve::Secp256k1),
            "secp256r1" | "prime256v1" | "p-256" => Some(Curve::Secp256r1),
            "secp384r1" | "p-384" => Some(Curve::Secp384r1),
            "secp521r1" | "p-521" => Some(Curve::Secp521r1),
            _ => None,
        }
    }

    /// The order of the group of points.
    pub fn order(self) -> BigUint {
        match self {
            Curve::Secp256k1 => secp256k1::order(),
            Curve::Secp256r1 => secp256r1::order(),
            Curve::Secp384r1 => secp384r1::order(),
            Curve::Secp521r1 => secp521r1::order(),
        }
    }

    /// The prime of the field the coordinates are in.
    pub fn prime(self) -> BigUint {
        let power = |exponent: u32| BigUint::from(1u8) << exponent;
        match self {
            Curve::Secp256k1 => power(256) - power(32) - 977u32,
            Curve::Secp256r1 => power(256) - power(224) + power(192) + power(96) - 1u8,
            Curve::Secp384r1 => power(384) - power(128) - power(96) + power(32) - 1u8,
            Curve::Secp521r1 => power(521) - 1u8,
        }
    }

    /// The name of the class of the elements of the field in BouncyCastle, which its errors name.
    fn field_element(self) -> &'static str {
        match self {
            Curve::Secp256k1 => "SecP256K1FieldElement",
            Curve::Secp256r1 => "SecP256R1FieldElement",
            Curve::Secp384r1 => "SecP384R1FieldElement",
            Curve::Secp521r1 => "SecP521R1FieldElement",
        }
    }

    /// The size of a coordinate, in bytes.
    pub fn size(self) -> usize {
        match self {
            Curve::Secp256k1 => secp256k1::SIZE,
            Curve::Secp256r1 => secp256r1::SIZE,
            Curve::Secp384r1 => secp384r1::SIZE,
            Curve::Secp521r1 => secp521r1::SIZE,
        }
    }

    /// The point `d * G` of a private key `d`: the point at infinity for the multiples of the
    /// order.
    pub fn public_key(self, d: &BigInt) -> Point {
        Point::base_mul(self, &reduce(d, &self.order()))
    }

    /// The private number of a private key, which must be in `[1, order - 1]`.
    ///
    /// # Errors
    ///
    /// A number outside of the interval.
    pub fn private_scalar(self, d: &BigInt) -> Result<BigUint, EcError> {
        if *d < BigInt::from(1) || *d >= BigInt::from(self.order()) {
            return Err(EcError::ScalarInterval);
        }
        Ok(d.magnitude().clone())
    }

    /// A random number of as many bits as the order, until it is in `[min, order)`.
    fn random_below_order(self, min: u8, random: Random<'_>) -> Result<BigUint, EcError> {
        let order = self.order();
        let bits = order.bits() as usize;
        let mut buffer = vec![0u8; bits.div_ceil(8)];
        loop {
            random(&mut buffer)?;
            // Only the low bits of the first byte count: the number has as many bits as the order.
            buffer[0] &= 0xff >> (buffer.len() * 8 - bits);
            let number = BigUint::from_bytes_be(&buffer);
            if number >= BigUint::from(min) && number < order {
                return Ok(number);
            }
        }
    }

    /// A random private key, in `[2, order)`.
    ///
    /// # Errors
    ///
    /// The source of random bytes fails.
    pub fn generate(self, random: Random<'_>) -> Result<BigUint, EcError> {
        self.random_below_order(2, random)
    }

    /// The x coordinate of `d * Q`, the secret the owners of `d` and `Q` agree on.
    pub fn agree(self, d: &BigUint, q: &Point) -> BigUint {
        q.mul(d).coordinates().0
    }

    /// The number a hash stands for: its leftmost bits, as many as the order has.
    fn hash_number(self, hash: &[u8]) -> BigUint {
        let excess = (hash.len() * 8).saturating_sub(self.order().bits() as usize);
        BigUint::from_bytes_be(hash) >> excess
    }

    /// Sign a hash with the private number `d` (see [`Curve::private_scalar`]).
    ///
    /// # Errors
    ///
    /// The source of random bytes fails.
    pub fn sign(
        self,
        secret: &BigUint,
        hash: &[u8],
        random: Random<'_>,
    ) -> Result<Signature, EcError> {
        let order = self.order();
        let digest = self.hash_number(hash) % &order;
        loop {
            let nonce = self.random_below_order(1, random)?;
            let r = Point::base_mul(self, &nonce).coordinates().0 % &order;
            let s = match self {
                Curve::Secp256k1 => secp256k1::sign_s(secret, &nonce, &digest, &r),
                Curve::Secp256r1 => secp256r1::sign_s(secret, &nonce, &digest, &r),
                Curve::Secp384r1 => secp384r1::sign_s(secret, &nonce, &digest, &r),
                Curve::Secp521r1 => secp521r1::sign_s(secret, &nonce, &digest, &r),
            };
            // Another nonce for the signatures that would give nothing away of the key.
            if r.is_zero() || s.is_zero() {
                continue;
            }
            return Ok(Signature { r, s });
        }
    }

    /// `number^-1 mod order`, for a number that is not a multiple of the (prime) order.
    fn invert(self, number: &BigUint) -> BigUint {
        let order = self.order();
        number.modpow(&(&order - 2u8), &order)
    }

    /// Whether a signature of a hash is the one of the owner of a public key.
    pub fn verify(self, q: &Point, hash: &[u8], signature: &Signature) -> bool {
        let order = self.order();
        let in_range = |number: &BigUint| !number.is_zero() && *number < order;
        if !in_range(&signature.r) || !in_range(&signature.s) {
            return false;
        }
        let inverse = self.invert(&signature.s);
        let u1 = (self.hash_number(hash) * &inverse) % &order;
        let u2 = (&signature.r * &inverse) % &order;
        q.combine(&u1, &u2).coordinates().0 % &order == signature.r
    }

    /// The public keys a signature of a hash can come from: for each multiple of the order
    /// added to `r` (the `cofactors`) and each tag of the compressed point `R` the signature
    /// stands for (`parities`: `2` for an even ordinate, `3` for an odd one), the key
    /// `r^-1 (s R - e G)`, with the point `R` and the one opposite to it. Candidates are given
    /// once, in the order they are found.
    ///
    /// # Errors
    ///
    /// An `r` that is a multiple of the order, which has no inverse.
    pub fn recover(
        self,
        r: &BigInt,
        s: &BigInt,
        hash: &[u8],
        cofactors: RangeInclusive<i64>,
        parities: &[u8],
    ) -> Result<Vec<Candidate>, EcError> {
        let order = self.order();
        let reduced = reduce(r, &order);
        if reduced.is_zero() {
            return Err(EcError::NotInvertible);
        }
        let inverse = self.invert(&reduced);
        let digest = self.hash_number(hash) % &order;
        let g_scalar = (&order - digest) * &inverse % &order;
        let r_scalar = reduce(s, &order) * &inverse % &order;
        let mut candidates: Vec<Candidate> = Vec::new();
        for cofactor in cofactors {
            for &parity in parities {
                // The abscissa must be a coordinate, and the point must exist.
                let abscissa = r + BigInt::from(cofactor) * BigInt::from(order.clone());
                let Some(abscissa) = abscissa.to_biguint() else {
                    continue;
                };
                let Some(padding) = self.size().checked_sub(bytes(&abscissa).len()) else {
                    continue;
                };
                let mut encoded = vec![parity];
                encoded.extend(vec![0u8; padding]);
                encoded.extend(bytes(&abscissa));
                let point = match Point::decode(self, &encoded) {
                    Ok(point) => point,
                    // Whatever the reason, there is no point for this abscissa (one of the two
                    // parities, or a multiple of the order that is too large): no candidate.
                    Err(_no_point) => continue,
                };
                for point in [point, point.negate()] {
                    let key = point.combine(&g_scalar, &r_scalar);
                    let (x, y) = point.coordinates();
                    let candidate = Candidate { key, x, y };
                    if !candidates.iter().any(|known| known.key == candidate.key) {
                        candidates.push(candidate);
                    }
                }
            }
        }
        Ok(candidates)
    }
}

/// A private key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateKey {
    /// The curve.
    pub curve: Curve,
    /// The name of the curve the key was made with, which `ECPRIVATE` gives back.
    pub name: String,
    /// The private number.
    pub d: BigInt,
}

/// A public key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicKey {
    /// The curve.
    pub curve: Curve,
    /// The name of the curve the key was made with, which `ECPUBLIC` gives back.
    pub name: String,
    /// The encoding of the point as it was given (compressed or not).
    pub encoded: Vec<u8>,
    /// The point.
    pub point: Point,
}

/// A key of the `EC*` functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    /// A private key (`ECPRIVATE`).
    Private(PrivateKey),
    /// A public key (`ECPUBLIC`).
    Public(Box<PublicKey>),
}

impl PublicKey {
    /// The public key of an encoded point of a curve.
    ///
    /// # Errors
    ///
    /// An encoding that is not a point of the curve.
    pub fn new(curve: Curve, name: &str, encoded: Vec<u8>) -> Result<PublicKey, PointError> {
        let point = Point::decode(curve, &encoded)?;
        Ok(PublicKey {
            curve,
            name: name.to_owned(),
            encoded,
            point,
        })
    }
}

impl PrivateKey {
    /// The public key of this private key.
    pub fn public(&self) -> PublicKey {
        let point = self.curve.public_key(&self.d);
        PublicKey {
            curve: self.curve,
            name: self.name.clone(),
            encoded: point.encode(),
            point,
        }
    }
}

/// The DER of a signature: a sequence of the integers `r` and `s`.
pub fn der_encode(signature: &Signature) -> Vec<u8> {
    fn integer(number: &BigUint, out: &mut Vec<u8>) {
        let mut content = bytes(number);
        if content.first().is_none_or(|byte| byte & 0x80 != 0) {
            content.insert(0, 0);
        }
        out.push(0x02);
        push_length(content.len(), out);
        out.extend(content);
    }
    fn push_length(length: usize, out: &mut Vec<u8>) {
        if length < 0x80 {
            out.push(length as u8);
        } else {
            let significant: Vec<u8> = length
                .to_be_bytes()
                .into_iter()
                .skip_while(|byte| *byte == 0)
                .collect();
            out.push(0x80 | significant.len() as u8);
            out.extend(significant);
        }
    }
    let mut content = Vec::new();
    integer(&signature.r, &mut content);
    integer(&signature.s, &mut content);
    let mut out = vec![0x30];
    push_length(content.len(), &mut out);
    out.extend(content);
    out
}

/// The length of a DER element and where its content starts, from the bytes after its tag.
fn der_length(input: &[u8]) -> Result<(usize, usize), EcError> {
    let first = *input.first().ok_or(EcError::MalformedSignature)?;
    if first & 0x80 == 0 {
        return Ok((usize::from(first), 1));
    }
    let count = usize::from(first & 0x7f);
    let digits = input.get(1..=count).ok_or(EcError::MalformedSignature)?;
    // The shortest form: no length below 128 in the long form, no leading zero.
    if count == 0 || count > 4 || digits[0] == 0 {
        return Err(EcError::MalformedSignature);
    }
    let length = digits
        .iter()
        .fold(0usize, |length, digit| length << 8 | usize::from(*digit));
    if length < 0x80 {
        return Err(EcError::MalformedSignature);
    }
    Ok((length, 1 + count))
}

/// An element of DER with the given tag, its content and what follows it.
fn der_element(input: &[u8], tag: u8) -> Result<(&[u8], &[u8]), EcError> {
    let (first, rest) = input.split_first().ok_or(EcError::MalformedSignature)?;
    if *first != tag {
        return Err(EcError::MalformedSignature);
    }
    let (length, offset) = der_length(rest)?;
    let content = rest
        .get(offset..)
        .and_then(|content| content.get(..length))
        .ok_or(EcError::MalformedSignature)?;
    Ok((content, &rest[offset + length..]))
}

/// A positive DER integer, in its shortest form.
fn der_integer(input: &[u8]) -> Result<(BigUint, &[u8]), EcError> {
    let (content, rest) = der_element(input, 0x02)?;
    let padded = match content {
        [] => return Err(EcError::MalformedSignature),
        [0, next, ..] if next & 0x80 == 0 => return Err(EcError::MalformedSignature),
        [first, ..] if first & 0x80 != 0 => return Err(EcError::MalformedSignature),
        [0, tail @ ..] => tail,
        all => all,
    };
    Ok((BigUint::from_bytes_be(padded), rest))
}

/// Read the DER of a signature.
///
/// # Errors
///
/// Bytes that are not the DER of a sequence of two positive integers.
pub fn der_decode(input: &[u8]) -> Result<Signature, EcError> {
    let (content, trailing) = der_element(input, 0x30)?;
    let (r, rest) = der_integer(content)?;
    let (s, rest) = der_integer(rest)?;
    if !rest.is_empty() || !trailing.is_empty() {
        return Err(EcError::MalformedSignature);
    }
    Ok(Signature { r, s })
}

#[cfg(test)]
#[path = "ec_tests.rs"]
mod ec_tests;
