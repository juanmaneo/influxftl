// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The encryption of OpenPGP messages to an RSA key, as BouncyCastle's
//! `PGPEncryptedDataGenerator` writes them: a public-key encrypted session key packet of version
//! 3, then a symmetrically encrypted and integrity protected data packet of version 1 that holds
//! a literal packet of the data and its modification detection code.

use num_bigint::BigUint;

use crate::ec::Random;
use crate::error::crypto::RsaError;
use crate::error::pgp::PgpError;
use crate::openpgp::packet::write_new_packet;
use crate::openpgp::sign::{LITERAL, literal, write_mpi};
use crate::openpgp::symmetric::Cipher;
use crate::rsa::{Exponentiation, Hash, RsaKey};

/// The tag of a public-key encrypted session key packet.
const PKESK: u8 = 1;

/// The tag of a symmetrically encrypted and integrity protected data packet.
const SEIPD: u8 = 18;

/// The version of the session key packets.
const PKESK_VERSION: u8 = 3;

/// The version of the data packets.
const SEIPD_VERSION: u8 = 1;

/// The public-key algorithm of RSA.
const RSA: u8 = 1;

/// The header of the modification detection code packet, which holds a SHA-1 digest.
const MDC_HEADER: [u8; 2] = [0xD3, 0x14];

/// What a message is encrypted to: the public key of the recipient, the key ID that names it in
/// the session key packet, the cipher of the data, and the date of the literal packet.
pub struct EncryptParams<'a> {
    /// The public key of the recipient.
    pub key: &'a RsaKey,
    /// The key ID written in the session key packet: zero hides the recipient.
    pub key_id: u64,
    /// The cipher of the data.
    pub cipher: Cipher,
    /// The date of the literal packet, in seconds since the epoch.
    pub date: u32,
}

/// The encrypted message of some data, under a session key drawn from `random`.
pub fn encrypt(
    params: &EncryptParams<'_>,
    data: &[u8],
    random: Random<'_>,
) -> Result<Vec<u8>, PgpError> {
    let mut session_key = vec![0; params.cipher.key_len()];
    random(&mut session_key).map_err(RsaError::from)?;
    encrypt_with_session_key(params, &session_key, data, random)
}

/// The encrypted message of some data under a session key that is given. The padding of the
/// session key packet and the prefix of the data packet are drawn from `random`.
pub fn encrypt_with_session_key(
    params: &EncryptParams<'_>,
    session_key: &[u8],
    data: &[u8],
    random: Random<'_>,
) -> Result<Vec<u8>, PgpError> {
    let key = Exponentiation::new(params.key)?;
    let ciphertext = key.encrypt_random(&session_info(params.cipher, session_key), random)?;
    let mut pkesk = vec![PKESK_VERSION];
    pkesk.extend(params.key_id.to_be_bytes());
    pkesk.push(RSA);
    write_mpi(&mut pkesk, &BigUint::from_bytes_be(&ciphertext));
    let mut out = Vec::new();
    write_new_packet(&mut out, PKESK, &pkesk);
    write_new_packet(
        &mut out,
        SEIPD,
        &protected_data(params, session_key, data, random)?,
    );
    Ok(out)
}

/// The session info that the session key packet encrypts: the identifier of the cipher, the
/// session key, and its checksum, the sum of its bytes modulo 65536.
fn session_info(cipher: Cipher, session_key: &[u8]) -> Vec<u8> {
    let checksum = session_key
        .iter()
        .fold(0u16, |sum, byte| sum.wrapping_add(u16::from(*byte)));
    let mut info = vec![cipher.id()];
    info.extend(session_key);
    info.extend(checksum.to_be_bytes());
    info
}

/// The body of the data packet: the version, then the cipher in CFB mode over a random prefix,
/// the literal packet of the data, and the modification detection code of all of it.
fn protected_data(
    params: &EncryptParams<'_>,
    session_key: &[u8],
    data: &[u8],
    random: Random<'_>,
) -> Result<Vec<u8>, PgpError> {
    let block = params.cipher.block_len();
    let mut plain = vec![0; block + 2];
    random(&mut plain[..block]).map_err(RsaError::from)?;
    plain[block] = plain[block - 2];
    plain[block + 1] = plain[block - 1];
    write_new_packet(&mut plain, LITERAL, &literal(params.date, data));
    plain.extend(MDC_HEADER);
    let digest = Hash::Sha1.digest(&plain);
    plain.extend(digest);
    let mut body = vec![SEIPD_VERSION];
    body.extend(params.cipher.crypt(session_key, &plain, true)?);
    Ok(body)
}
