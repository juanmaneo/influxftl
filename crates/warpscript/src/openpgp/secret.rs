// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The secret part of an RSA key packet: its private exponent, unlocked with the passphrase as
//! BouncyCastle's `PGPSecretKey` unlocks it. The protections are the usage 0 (none) and 254
//! (a cipher, a string-to-key of type 3 and a SHA-1 checksum); the ciphers are AES of 128, 192
//! and 256 bits in CFB mode.

use aes::cipher::KeyInit;
use aes::{Aes128, Aes192, Aes256};
use digest::Digest;
use num_bigint::{BigInt, BigUint};

use crate::error::pgp::PgpError;
use crate::openpgp::key::{PublicKey, read_big};
use crate::openpgp::sign::hash_of_id;
use crate::openpgp::symmetric::cfb;
use crate::rsa::{Hash, RsaKey};

/// The usage of a secret key that leaves its material in the clear.
const UNPROTECTED: u8 = 0;

/// The usage of a secret key that encrypts its material and checks it with a SHA-1 digest.
const SHA1_CHECKED: u8 = 254;

/// The string-to-key specifier that salts the passphrase and iterates its hashing.
const ITERATED_SALTED: u8 = 3;

/// The length of the SHA-1 digest that closes the encrypted material.
const SHA1_LENGTH: usize = 20;

/// The RSA key of the body of a secret key packet, with its private exponent unlocked by
/// `passphrase` (`PGPSecretKey.extractPrivateKey`): the modulus and the exponent.
pub fn rsa_private_key(body: &[u8], passphrase: &str) -> Result<RsaKey, PgpError> {
    let (modulus, _, mut rest) = PublicKey::rsa_numbers(body)?;
    let exponent = match take_byte(&mut rest)? {
        UNPROTECTED => read_big(&mut rest)?,
        SHA1_CHECKED => unlock(rest, passphrase)?,
        other => return Err(PgpError::UnportedProtection(other)),
    };
    Ok(RsaKey {
        modulus: BigInt::from(modulus),
        exponent: BigInt::from(exponent),
    })
}

/// The private exponent of the material of a key of usage 254: the cipher, the specifier, the
/// initialisation vector, then the encrypted MPIs and their SHA-1 digest.
fn unlock(mut rest: &[u8], passphrase: &str) -> Result<BigUint, PgpError> {
    let cipher = take_byte(&mut rest)?;
    let specifier = take_byte(&mut rest)?;
    if specifier != ITERATED_SALTED {
        return Err(PgpError::UnportedS2k(specifier));
    }
    let hash = hash_of_id(take_byte(&mut rest)?)?;
    let salt: [u8; 8] = take_array(&mut rest)?;
    let count = take_byte(&mut rest)?;
    let iv: [u8; 16] = take_array(&mut rest)?;
    let plain = decrypt(
        cipher,
        &Unlock {
            hash,
            salt,
            count,
            passphrase,
            iv,
        },
        rest,
    )?;
    let (mut mpis, digest) = plain
        .split_last_chunk::<SHA1_LENGTH>()
        .ok_or(PgpError::PrematureEnd)?;
    if sha1::Sha1::digest(mpis).as_slice() != digest {
        return Err(PgpError::ChecksumMismatch);
    }
    read_big(&mut mpis)
}

/// The parameters of the key derivation and of the cipher of a key of usage 254.
struct Unlock<'a> {
    /// The hash of the string-to-key.
    hash: Hash,
    /// The salt of the string-to-key.
    salt: [u8; 8],
    /// The count of the string-to-key, encoded as in the packet.
    count: u8,
    /// The passphrase.
    passphrase: &'a str,
    /// The initialisation vector of the cipher.
    iv: [u8; 16],
}

/// The material of a key decrypted with the AES of the cipher, for keys of 128, 192 and 256 bits.
fn decrypt(cipher: u8, unlock: &Unlock<'_>, data: &[u8]) -> Result<Vec<u8>, PgpError> {
    let Unlock {
        hash,
        salt,
        count,
        passphrase,
        iv,
    } = *unlock;
    Ok(match cipher {
        7 => cfb(
            &Aes128::new(&s2k_key::<16>(hash, salt, count, passphrase).into()),
            &iv,
            data,
            false,
        ),
        8 => cfb(
            &Aes192::new(&s2k_key::<24>(hash, salt, count, passphrase).into()),
            &iv,
            data,
            false,
        ),
        9 => cfb(
            &Aes256::new(&s2k_key::<32>(hash, salt, count, passphrase).into()),
            &iv,
            data,
            false,
        ),
        other => return Err(PgpError::UnportedCipher(other)),
    })
}

/// The key of `N` bytes that a passphrase makes (`IteratedSaltedS2K`): the hash of the salt and
/// the passphrase, repeated up to the count, and when the key is longer than a digest, the same
/// hash of the same data preceded by one more zero byte each time.
pub(crate) fn s2k_key<const N: usize>(
    hash: Hash,
    salt: [u8; 8],
    count: u8,
    passphrase: &str,
) -> [u8; N] {
    let data: Vec<u8> = salt.iter().chain(passphrase.as_bytes()).copied().collect();
    let total = s2k_octets(count).max(data.len());
    let mut key = [0u8; N];
    let mut written = 0;
    let mut zeros = 0;
    while written < N {
        let mut input = vec![0u8; zeros];
        input.extend(data.iter().cycle().take(total));
        for (slot, byte) in key.iter_mut().skip(written).zip(hash.digest(&input)) {
            *slot = byte;
            written += 1;
        }
        zeros += 1;
    }
    key
}

/// The number of bytes that a count of the packet stands for (`S2K.getIterationCount`).
fn s2k_octets(count: u8) -> usize {
    (16 + usize::from(count & 15)) << (usize::from(count >> 4) + 6)
}

/// The next byte of `rest`, which is consumed.
pub(crate) fn take_byte(rest: &mut &[u8]) -> Result<u8, PgpError> {
    let (&byte, tail) = rest.split_first().ok_or(PgpError::PrematureEnd)?;
    *rest = tail;
    Ok(byte)
}

/// The next `N` bytes of `rest`, which are consumed.
pub(crate) fn take_array<const N: usize>(rest: &mut &[u8]) -> Result<[u8; N], PgpError> {
    let (&array, tail) = rest
        .split_first_chunk::<N>()
        .ok_or(PgpError::PrematureEnd)?;
    *rest = tail;
    Ok(array)
}

/// The next `count` bytes of `rest`, which are consumed.
pub(crate) fn take_slice<'a>(rest: &mut &'a [u8], count: usize) -> Result<&'a [u8], PgpError> {
    let (head, tail) = rest.split_at_checked(count).ok_or(PgpError::PrematureEnd)?;
    *rest = tail;
    Ok(head)
}
