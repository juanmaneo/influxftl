// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The signatures of OpenPGP (RFC 4880 §5.2) as the key rings read them: their type, the key
//! that made them, their creation time, and the subpackets that tell the expiry of a key.

use crate::error::pgp::PgpError;

/// The tag of the subpacket that gives the creation time of a signature.
const CREATION_TIME: u8 = 2;
/// The tag of the subpacket that gives the key ID of the signer.
const ISSUER: u8 = 16;
/// The tag of the subpacket that gives the number of seconds until a key expires.
const KEY_EXPIRATION_TIME: u8 = 9;

/// A subpacket of the hashed or unhashed area of a signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subpacket {
    /// The type of the subpacket, without its critical bit.
    pub tag: u8,
    /// The data of the subpacket.
    pub data: Vec<u8>,
}

/// A signature, as far as the key rings need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// The type of the signature: 0x10 to 0x13 certify a user ID, 0x18 binds a subkey and 0x1F
    /// is a direct key signature.
    pub sig_type: u8,
    /// The key ID of the signer.
    pub key_id: u64,
    /// The creation time, in milliseconds since the epoch.
    pub creation_ms: i64,
    /// The subpackets of the hashed area, `None` when the signature has no such area (version 3).
    pub hashed: Option<Vec<Subpacket>>,
}

impl Signature {
    /// The signature of a signature packet body, or `None` for a version this port does not read,
    /// which the rings skip (`UnsupportedPacketVersionException`).
    pub fn parse(body: &[u8]) -> Result<Option<Self>, PgpError> {
        match body.first() {
            Some(4) => parse_v4(body).map(Some),
            Some(3) => parse_v3(body).map(Some),
            _ => Ok(None),
        }
    }

    /// The number of seconds after its creation at which the key expires, if its hashed area
    /// gives one (`getKeyExpirationTime`).
    pub fn key_expiration(&self) -> Option<u32> {
        let subpacket = self
            .hashed
            .as_ref()?
            .iter()
            .find(|s| s.tag == KEY_EXPIRATION_TIME)?;
        Some(u32::from_be_bytes(*subpacket.data.first_chunk::<4>()?))
    }
}

/// A version 4 signature: the type, the hashed and unhashed areas, then the MPIs (not read).
fn parse_v4(body: &[u8]) -> Result<Signature, PgpError> {
    let sig_type = *body.get(1).ok_or(PgpError::PrematureEnd)?;
    let (hashed, rest) = read_area(body.get(4..).ok_or(PgpError::PrematureEnd)?)?;
    let (unhashed, _) = read_area(rest)?;
    let hashed_subpackets = subpackets(hashed)?;
    let mut creation_ms = 0;
    let mut key_id = 0;
    for subpacket in &hashed_subpackets {
        match subpacket.tag {
            CREATION_TIME => creation_ms = creation_time(&subpacket.data)?,
            ISSUER => key_id = issuer(&subpacket.data)?,
            _ => {}
        }
    }
    for subpacket in subpackets(unhashed)? {
        if subpacket.tag == ISSUER {
            key_id = issuer(&subpacket.data)?;
        }
    }
    Ok(Signature {
        sig_type,
        key_id,
        creation_ms,
        hashed: Some(hashed_subpackets),
    })
}

/// A version 3 signature: the type, the creation time and the key ID sit at fixed offsets.
fn parse_v3(body: &[u8]) -> Result<Signature, PgpError> {
    let [_, _, sig_type, c0, c1, c2, c3, key_id @ ..] = body else {
        return Err(PgpError::PrematureEnd);
    };
    Ok(Signature {
        sig_type: *sig_type,
        key_id: issuer(key_id)?,
        creation_ms: i64::from(u32::from_be_bytes([*c0, *c1, *c2, *c3])) * 1000,
        hashed: None,
    })
}

/// The area of `input` that a two-byte length announces, and what follows it.
fn read_area(input: &[u8]) -> Result<(&[u8], &[u8]), PgpError> {
    let length = u16::from_be_bytes(*input.first_chunk::<2>().ok_or(PgpError::PrematureEnd)?);
    input
        .get(2..)
        .and_then(|rest| rest.split_at_checked(usize::from(length)))
        .ok_or(PgpError::PrematureEnd)
}

/// The subpackets of an area, each with its length (one, two or five octets).
fn subpackets(mut area: &[u8]) -> Result<Vec<Subpacket>, PgpError> {
    let mut subpackets = Vec::new();
    while let Some(&first) = area.first() {
        let (length, header) = match first {
            0..=191 => (usize::from(first), 1),
            192..=254 => {
                let second = *area.get(1).ok_or(PgpError::PrematureEnd)?;
                (
                    (usize::from(first) - 192) * 256 + usize::from(second) + 192,
                    2,
                )
            }
            255 => {
                let four = *area
                    .get(1..)
                    .and_then(|rest| rest.first_chunk::<4>())
                    .ok_or(PgpError::PrematureEnd)?;
                (u32::from_be_bytes(four) as usize, 5)
            }
        };
        let (subpacket, rest) = area
            .get(header..)
            .and_then(|rest| rest.split_at_checked(length))
            .ok_or(PgpError::PrematureEnd)?;
        if let Some((&tag, data)) = subpacket.split_first() {
            subpackets.push(Subpacket {
                tag: tag & 0x7F,
                data: data.to_vec(),
            });
        }
        area = rest;
    }
    Ok(subpackets)
}

/// A creation time in seconds, as milliseconds.
fn creation_time(data: &[u8]) -> Result<i64, PgpError> {
    let seconds = u32::from_be_bytes(*data.first_chunk::<4>().ok_or(PgpError::PrematureEnd)?);
    Ok(i64::from(seconds) * 1000)
}

/// The key ID of an issuer subpacket.
fn issuer(data: &[u8]) -> Result<u64, PgpError> {
    Ok(u64::from_be_bytes(
        *data.first_chunk::<8>().ok_or(PgpError::PrematureEnd)?,
    ))
}
