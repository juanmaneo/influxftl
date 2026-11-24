// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The keys of OpenPGP: the public part of the public and secret key packets, with its
//! fingerprint, key ID, strength and flags, as BouncyCastle's `PGPPublicKey` exposes them.

use digest::Digest;
use num_bigint::BigUint;

use crate::error::pgp::PgpError;

/// The public part of a key packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey {
    /// The version of the packet: 3 or 4 (2 is read like 3).
    pub version: u8,
    /// The creation time, in seconds since the epoch.
    pub created: u32,
    /// The validity in days, which only version 3 keys carry.
    pub valid_days: u16,
    /// The public-key algorithm (RFC 4880 §9.1).
    pub algorithm: u8,
    /// The strength of the key, in bits (`getBitStrength`).
    pub bits: u32,
    /// The fingerprint: 16 bytes for version 3, 20 for version 4.
    pub fingerprint: Vec<u8>,
    /// The key ID: the low 64 bits of the fingerprint, or of the modulus for version 3.
    pub key_id: u64,
}

/// A multi-precision integer of a key: its bit length and its bytes, as written in the packet.
struct Mpi<'a> {
    bits: u32,
    bytes: &'a [u8],
}

/// The curve of an elliptic key, by its OID, with the bit strength BouncyCastle gives it.
const CURVES: &[(&[u8], u32)] = &[
    (&[0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07], 256), // NIST P-256
    (&[0x2B, 0x81, 0x04, 0x00, 0x22], 384),                   // NIST P-384
    (&[0x2B, 0x81, 0x04, 0x00, 0x23], 521),                   // NIST P-521
    (&[0x2B, 0x81, 0x04, 0x00, 0x0A], 256),                   // secp256k1
    (&[0x2B, 0x24, 0x03, 0x03, 0x02, 0x08, 0x01, 0x01, 0x07], 256), // brainpoolP256r1
    (&[0x2B, 0x24, 0x03, 0x03, 0x02, 0x08, 0x01, 0x01, 0x0B], 384), // brainpoolP384r1
    (&[0x2B, 0x24, 0x03, 0x03, 0x02, 0x08, 0x01, 0x01, 0x0D], 512), // brainpoolP512r1
    (&[0x2B, 0x06, 0x01, 0x04, 0x01, 0xDA, 0x47, 0x0F, 0x01], 256), // Ed25519
    (
        &[0x2B, 0x06, 0x01, 0x04, 0x01, 0x97, 0x55, 0x01, 0x05, 0x01],
        256,
    ), // Curve25519
];

impl PublicKey {
    /// The public part of the body of a key packet, and the length of that part, so that the
    /// secret part of a secret key packet can follow it (`PublicKeyPacket` reading).
    pub fn parse(body: &[u8]) -> Result<(Self, usize), PgpError> {
        let (key, rest, _) = Self::parse_material(body)?;
        Ok((key, body.len() - rest.len()))
    }

    /// `parse`, also giving the rest of the body after the public part and the key material.
    fn parse_material(body: &[u8]) -> Result<(Self, &[u8], Material<'_>), PgpError> {
        let mut rest = body;
        let version = lenient_byte(&mut rest);
        let created = u32::from_be_bytes([
            lenient_byte(&mut rest),
            lenient_byte(&mut rest),
            lenient_byte(&mut rest),
            lenient_byte(&mut rest),
        ]);
        let valid_days = if matches!(version, 2 | 3) {
            u16::from_be_bytes([lenient_byte(&mut rest), lenient_byte(&mut rest)])
        } else {
            0
        };
        let algorithm = lenient_byte(&mut rest);
        let material = read_material(algorithm, &mut rest)?;
        if !matches!(version, 2..=4) {
            return Err(PgpError::UnsupportedVersion(version));
        }
        let consumed = body.len() - rest.len();
        let (fingerprint, key_id) = if matches!(version, 2 | 3) {
            let (modulus, exponent) = material.rsa_parts(algorithm)?;
            let mut input = modulus.to_vec();
            input.extend_from_slice(exponent);
            (md5::Md5::digest(&input).to_vec(), low_u64(modulus))
        } else {
            let mut input = vec![0x99];
            input.extend_from_slice(&(consumed as u16).to_be_bytes());
            input.extend_from_slice(&body[..consumed]);
            let fingerprint = sha1::Sha1::digest(&input).to_vec();
            let key_id = low_u64(&fingerprint);
            (fingerprint, key_id)
        };
        let key = Self {
            version,
            created,
            valid_days,
            algorithm,
            bits: material.bits(),
            fingerprint,
            key_id,
        };
        Ok((key, rest, material))
    }

    /// The modulus and the public exponent of an RSA key packet body of version 4, and the rest
    /// of the body after the public part, where a secret part follows.
    pub fn rsa_numbers(body: &[u8]) -> Result<(BigUint, BigUint, &[u8]), PgpError> {
        let version = body.first().copied().ok_or(PgpError::PrematureEnd)?;
        if version != 4 {
            return Err(PgpError::UnsupportedVersion(version));
        }
        let (key, rest, material) = Self::parse_material(body)?;
        match material {
            Material::Rsa {
                modulus, exponent, ..
            } => Ok((
                BigUint::from_bytes_be(modulus.bytes),
                BigUint::from_bytes_be(exponent.bytes),
                rest,
            )),
            Material::Other { .. } => Err(PgpError::UnsupportedAlgorithm(key.algorithm)),
        }
    }

    /// The creation time, in milliseconds since the epoch (`getCreationTime`).
    pub fn creation_ms(&self) -> i64 {
        i64::from(self.created) * 1000
    }

    /// The validity of a version 3 key in seconds (`getValidSeconds`).
    pub fn valid_seconds_v3(&self) -> i64 {
        i64::from(self.valid_days) * 86400
    }

    /// Whether the algorithm encrypts (`isEncryptionKey`).
    pub fn is_encryption(&self) -> bool {
        matches!(self.algorithm, 1 | 2 | 16 | 18 | 20 | 21 | 25 | 26)
    }

    /// Whether the algorithm signs, for a secret key (`isSigningKey`).
    pub fn is_signing(&self) -> bool {
        matches!(self.algorithm, 1 | 3 | 17 | 19 | 20 | 22 | 27 | 28)
    }

    /// The name of the algorithm, as `PGPINFO` reports it.
    pub fn algorithm_name(&self) -> String {
        match self.algorithm {
            1 => "RSA_GENERAL".into(),
            2 => "RSA_ENCRYPT".into(),
            3 => "RSA_SIGN".into(),
            16 => "ELGAMAL_ENCRYPT".into(),
            17 => "DSA".into(),
            18 => "ECDH".into(),
            19 => "ECDSA".into(),
            20 => "ELGAMAL_GENERAL".into(),
            21 => "DIFFIE_HELLMAN".into(),
            22 => "EDDSA".into(),
            other => format!("UNKNOWN_{}", other as i8),
        }
    }

    /// The fingerprint as lower-case hexadecimal (`PGPINFO`'s `fingerprint`).
    pub fn fingerprint_hex(&self) -> String {
        to_hex(&self.fingerprint)
    }
}

/// The lower-case hexadecimal of some bytes.
pub fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
            out.push_str(&format!("{b:02x}"));
            out
        })
}

/// The key ID as a script reads it: sixteen upper-case hexadecimal digits.
pub fn key_id_hex(key_id: u64) -> String {
    format!("{key_id:016X}")
}

/// The next byte of `rest`, or `-1` at its end (`InputStream.read`), as the header of a key
/// reads them: a truncated header carries on, and fails on its algorithm.
fn lenient_byte(rest: &mut &[u8]) -> u8 {
    match rest.split_first() {
        Some((&byte, tail)) => {
            *rest = tail;
            byte
        }
        None => 0xFF,
    }
}

/// The key material that follows the algorithm: the two integers of an RSA key, or the strength
/// of a key of another algorithm (whose integers are read, and not kept).
enum Material<'a> {
    Rsa {
        bits: u32,
        modulus: Mpi<'a>,
        exponent: Mpi<'a>,
    },
    Other {
        bits: u32,
    },
}

impl<'a> Material<'a> {
    fn bits(&self) -> u32 {
        match self {
            Material::Rsa { bits, .. } | Material::Other { bits, .. } => *bits,
        }
    }

    /// The modulus and the exponent of an RSA key: the bytes of the integers. A version 2 or 3
    /// key of another algorithm is refused, as Warp 10 refuses it (`PGPPublicKeyRing` casts its
    /// key to an RSA key to compute the key ID).
    fn rsa_parts(&self, algorithm: u8) -> Result<(&'a [u8], &'a [u8]), PgpError> {
        match self {
            Material::Rsa {
                modulus, exponent, ..
            } => Ok((modulus.bytes, exponent.bytes)),
            Material::Other { .. } => Err(PgpError::UnsupportedAlgorithm(algorithm)),
        }
    }
}

/// Reads the key material of `algorithm` from `rest` (`PublicKeyPacket` reading).
fn read_material<'a>(algorithm: u8, rest: &mut &'a [u8]) -> Result<Material<'a>, PgpError> {
    match algorithm {
        1..=3 => {
            let modulus = read_mpi(rest)?;
            let exponent = read_mpi(rest)?;
            Ok(Material::Rsa {
                bits: modulus.bits,
                modulus,
                exponent,
            })
        }
        16 | 20 | 21 => read_discrete_log(3, rest),
        17 => read_discrete_log(4, rest),
        18 | 19 | 22 => {
            let bits = read_curve(rest)?;
            read_mpi(rest)?;
            if algorithm == 18 {
                read_kdf_parameters(rest)?;
            }
            Ok(Material::Other { bits })
        }
        25..=28 => Err(PgpError::UnsupportedAlgorithm(algorithm)),
        _ => Err(PgpError::UnknownAlgorithm(algorithm as i8)),
    }
}

/// The integers of a discrete logarithm key (`count` of them: `p`, `g`, `y`, and `q` for DSA),
/// the strength being the bit length of `p`.
fn read_discrete_log<'a>(count: usize, rest: &mut &'a [u8]) -> Result<Material<'a>, PgpError> {
    let mut bits = 0;
    for index in 0..count {
        let mpi = read_mpi(rest)?;
        if index == 0 {
            bits = mpi.bits;
        }
    }
    Ok(Material::Other { bits })
}

/// The OID of an elliptic curve, and the bit strength of the curve.
fn read_curve(rest: &mut &[u8]) -> Result<u32, PgpError> {
    let length = usize::from(lenient_byte(rest));
    let (oid, tail) = rest
        .split_at_checked(length)
        .ok_or(PgpError::PrematureEnd)?;
    *rest = tail;
    CURVES
        .iter()
        .find(|(known, _)| *known == oid)
        .map(|&(_, bits)| bits)
        .ok_or(PgpError::UnknownCurve)
}

/// The parameters of ECDH's key derivation: a length, then that many bytes.
fn read_kdf_parameters(rest: &mut &[u8]) -> Result<(), PgpError> {
    let length = usize::from(lenient_byte(rest));
    let (_, tail) = rest
        .split_at_checked(length)
        .ok_or(PgpError::PrematureEnd)?;
    *rest = tail;
    Ok(())
}

/// A multi-precision integer: its bit length on two bytes, then its bytes.
fn read_mpi<'a>(rest: &mut &'a [u8]) -> Result<Mpi<'a>, PgpError> {
    let (length, tail) = rest
        .split_first_chunk::<2>()
        .ok_or(PgpError::PrematureEnd)?;
    let bits = u32::from(u16::from_be_bytes(*length));
    let count = bits.div_ceil(8) as usize;
    let (bytes, tail) = tail.split_at_checked(count).ok_or(PgpError::PrematureEnd)?;
    *rest = tail;
    Ok(Mpi { bits, bytes })
}

/// The next multi-precision integer of `rest`, as a number, which is consumed.
pub(crate) fn read_big(rest: &mut &[u8]) -> Result<BigUint, PgpError> {
    Ok(BigUint::from_bytes_be(read_mpi(rest)?.bytes))
}

/// The key ID of the low 64 bits of a big-endian number, as the last eight bytes of it.
pub(crate) fn low_u64(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .rev()
        .take(8)
        .rev()
        .fold(0, |acc, &b| (acc << 8) | u64::from(b))
}
