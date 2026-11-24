// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The decryption of OpenPGP messages under an RSA key, as BouncyCastle's `PGPEncryptedData`
//! opens them: the session key packets, of which the first one of version 3 is used, then the
//! data packet, whose content is a literal packet, possibly compressed.
//!
//! The modification detection code of the data packet is not checked, as Warp 10 does not check
//! it either: a message that is tampered with in its trailer still opens.

use std::io::Read;

use num_bigint::BigUint;

use crate::error::pgp::PgpError;
use crate::openpgp::key::read_big;
use crate::openpgp::packet::PacketReader;
use crate::openpgp::secret::{take_byte, take_slice};
use crate::openpgp::sign::{LITERAL, literal_content};
use crate::openpgp::symmetric::Cipher;
use crate::rsa::{Exponentiation, RsaKey};

/// The tag of a public-key encrypted session key packet.
const PKESK: u8 = 1;

/// The tag of a symmetrically encrypted data packet, which carries no integrity check.
const SED: u8 = 9;

/// The tag of a compressed data packet.
const COMPRESSED: u8 = 8;

/// The tag of a symmetrically encrypted and integrity protected data packet.
const SEIPD: u8 = 18;

/// The version of the session key packets the port reads.
const PKESK_VERSION: u8 = 3;

/// The version of the data packets the port reads.
const SEIPD_VERSION: u8 = 1;

/// The public-key algorithms of RSA: encryption and signature, or encryption only.
const RSA: u8 = 1;
const RSA_ENCRYPT_ONLY: u8 = 2;

/// The compression algorithms of OpenPGP: none, ZIP (raw deflate), ZLIB and BZIP2.
const UNCOMPRESSED: u8 = 0;
const ZIP: u8 = 1;
const ZLIB: u8 = 2;
const BZIP2: u8 = 3;

/// The length of the key ID of a session key packet, which the port does not compare.
const KEY_ID_LENGTH: usize = 8;

/// The data of the literal packet that a message holds, which is encrypted to the private key.
pub fn decrypt(message: &[u8], private: &RsaKey) -> Result<Vec<u8>, PgpError> {
    let mut reader = PacketReader::new(message);
    let mut keys = Vec::new();
    let tag = loop {
        let tag = reader.next_tag()?;
        if tag != Some(PKESK) {
            break tag;
        }
        let packet = reader.read()?;
        if let Some((&PKESK_VERSION, rest)) = packet.body.split_first() {
            keys.push(rest.to_vec());
        }
    };
    let data = match tag {
        None => return Err(PgpError::UnexpectedNull),
        Some(SEIPD) => reader.read()?.body,
        Some(SED) => return Err(PgpError::UnsupportedPacket(SED)),
        Some(_) => return Err(PgpError::NoEncryptedContent),
    };
    let Some(key) = keys.first() else {
        return Err(PgpError::NoSessionKey);
    };
    let (cipher, session_key) = open_session_key(key, private)?;
    content(&open_data(&data, cipher, &session_key)?)
}

/// The cipher and the session key of a session key packet of version 3, after its version: the
/// key ID, which is not compared, the algorithm, and the RSA encrypted session info.
fn open_session_key(body: &[u8], private: &RsaKey) -> Result<(Cipher, Vec<u8>), PgpError> {
    let mut rest = body;
    take_slice(&mut rest, KEY_ID_LENGTH)?;
    let algorithm = take_byte(&mut rest)?;
    if !matches!(algorithm, RSA | RSA_ENCRYPT_ONLY) {
        return Err(PgpError::UnsupportedAlgorithm(algorithm));
    }
    let number = read_big(&mut rest)?;
    session_key_of(&session_info(&number, private)?)
}

/// The session info of an RSA encrypted number, which the private key decrypts.
fn session_info(number: &BigUint, private: &RsaKey) -> Result<Vec<u8>, PgpError> {
    let key = Exponentiation::new(private)?;
    key.decrypt(&number.to_bytes_be())
        .map_err(PgpError::SessionInfo)
}

/// The cipher and the session key of a session info: the identifier of the cipher, the session
/// key, and the checksum of the session key. The checksum is checked before the cipher is read.
fn session_key_of(info: &[u8]) -> Result<(Cipher, Vec<u8>), PgpError> {
    let Some((data, checksum)) = info.split_last_chunk::<2>() else {
        return Err(PgpError::SessionChecksum);
    };
    let Some((&id, key)) = data.split_first() else {
        return Err(PgpError::SessionChecksum);
    };
    let sum = key
        .iter()
        .fold(0u16, |sum, byte| sum.wrapping_add(u16::from(*byte)));
    if u16::from_be_bytes(*checksum) != sum {
        return Err(PgpError::SessionChecksum);
    }
    Ok((Cipher::from_id(id)?, key.to_vec()))
}

/// The plain data of a data packet, after its version: the cipher in CFB mode, whose random
/// prefix is the block of the cipher and two bytes that repeat its last two.
fn open_data(body: &[u8], cipher: Cipher, session_key: &[u8]) -> Result<Vec<u8>, PgpError> {
    let Some((&version, encrypted)) = body.split_first() else {
        return Err(PgpError::PrematureEnd);
    };
    if version != SEIPD_VERSION {
        return Err(PgpError::UnsupportedVersion(version));
    }
    let plain = cipher.crypt(session_key, encrypted, false)?;
    let prefix = cipher.block_len() + 2;
    plain
        .get(prefix..)
        .map(<[u8]>::to_vec)
        .ok_or(PgpError::PrematureEnd)
}

/// The literal data of the content of a data packet, which is a literal packet or a compressed
/// packet whose content is a literal packet.
fn content(data: &[u8]) -> Result<Vec<u8>, PgpError> {
    let mut reader = PacketReader::new(data);
    if reader.next_tag()? == Some(COMPRESSED) {
        let packet = reader.read()?;
        return literal(&inflate(&packet.body)?);
    }
    literal(data)
}

/// The data of the literal packet that starts `data`, which must be one.
fn literal(data: &[u8]) -> Result<Vec<u8>, PgpError> {
    let mut reader = PacketReader::new(data);
    if reader.next_tag()? != Some(LITERAL) {
        return Err(PgpError::NoEncryptedContent);
    }
    literal_content(&reader.read()?.body)
}

/// The data of a compressed packet: its algorithm, then the data that it compresses.
fn inflate(body: &[u8]) -> Result<Vec<u8>, PgpError> {
    let Some((&algorithm, data)) = body.split_first() else {
        return Err(PgpError::PrematureEnd);
    };
    let mut out = Vec::new();
    let read = match algorithm {
        UNCOMPRESSED => return Ok(data.to_vec()),
        // Warp 10 reads an empty ZIP or ZLIB body as an empty stream, without a failure.
        ZIP | ZLIB if data.is_empty() => return Ok(Vec::new()),
        ZIP => flate2::read::DeflateDecoder::new(data).read_to_end(&mut out),
        ZLIB => flate2::read::ZlibDecoder::new(data).read_to_end(&mut out),
        BZIP2 => bzip2_rs::DecoderReader::new(data).read_to_end(&mut out),
        other => return Err(PgpError::UnknownCompression(other)),
    };
    read.map_err(|error| PgpError::Decompression(error.to_string()))?;
    Ok(out)
}
