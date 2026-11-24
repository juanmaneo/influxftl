// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The symmetric ciphers of OpenPGP (RFC 4880 §9.2) as BouncyCastle's `PGPEncryptedData` names
//! and numbers them, and the CFB mode that the encrypted data packets run them in.

use aes::cipher::{Block, BlockCipherEncrypt, KeyInit};
use aes::{Aes128, Aes192, Aes256};
use blowfish::Blowfish;
use camellia::{Camellia128, Camellia192, Camellia256};
use cast5::Cast5;
use des::{Des, TdesEde3};
use idea::Idea;
use twofish::Twofish;

use crate::error::pgp::PgpError;

/// The names of the algorithms of `PGPENCRYPT` with their identifiers. `3DES` is another name of
/// `TRIPLE_DES`; `SAFER` has an identifier, but no cipher is ported for it.
const NAMES: [(&str, u8); 14] = [
    ("IDEA", 1),
    ("TRIPLE_DES", 2),
    ("3DES", 2),
    ("CAST5", 3),
    ("BLOWFISH", 4),
    ("SAFER", 5),
    ("DES", 6),
    ("AES_128", 7),
    ("AES_192", 8),
    ("AES_256", 9),
    ("TWOFISH", 10),
    ("CAMELLIA_128", 11),
    ("CAMELLIA_192", 12),
    ("CAMELLIA_256", 13),
];

/// The identifier of an algorithm name of `PGPENCRYPT`, which is case-sensitive.
pub fn algorithm_id(name: &str) -> Result<u8, PgpError> {
    NAMES
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, id)| *id)
        .ok_or_else(|| PgpError::InvalidCipherName(name.to_owned()))
}

/// A symmetric cipher that the port runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cipher {
    /// IDEA, with a key of 128 bits.
    Idea,
    /// Triple DES (EDE), with keys of 168 bits.
    TripleDes,
    /// CAST5, with a key of 128 bits.
    Cast5,
    /// Blowfish, with a key of 128 bits.
    Blowfish,
    /// DES, with a key of 56 bits.
    Des,
    /// AES, with a key of 128 bits.
    Aes128,
    /// AES, with a key of 192 bits.
    Aes192,
    /// AES, with a key of 256 bits.
    Aes256,
    /// Twofish, with a key of 256 bits.
    Twofish,
    /// Camellia, with a key of 128 bits.
    Camellia128,
    /// Camellia, with a key of 192 bits.
    Camellia192,
    /// Camellia, with a key of 256 bits.
    Camellia256,
}

impl Cipher {
    /// The cipher of an identifier of OpenPGP, which is not one the port runs when it is SAFER or
    /// unknown.
    pub fn from_id(id: u8) -> Result<Self, PgpError> {
        Ok(match id {
            1 => Self::Idea,
            2 => Self::TripleDes,
            3 => Self::Cast5,
            4 => Self::Blowfish,
            6 => Self::Des,
            7 => Self::Aes128,
            8 => Self::Aes192,
            9 => Self::Aes256,
            10 => Self::Twofish,
            11 => Self::Camellia128,
            12 => Self::Camellia192,
            13 => Self::Camellia256,
            _ => return Err(PgpError::UnrecognisedCipher),
        })
    }

    /// The identifier of the cipher in OpenPGP.
    pub fn id(self) -> u8 {
        match self {
            Self::Idea => 1,
            Self::TripleDes => 2,
            Self::Cast5 => 3,
            Self::Blowfish => 4,
            Self::Des => 6,
            Self::Aes128 => 7,
            Self::Aes192 => 8,
            Self::Aes256 => 9,
            Self::Twofish => 10,
            Self::Camellia128 => 11,
            Self::Camellia192 => 12,
            Self::Camellia256 => 13,
        }
    }

    /// The length of the session key the cipher takes, in bytes.
    pub fn key_len(self) -> usize {
        match self {
            Self::Idea | Self::Cast5 | Self::Blowfish | Self::Aes128 | Self::Camellia128 => 16,
            Self::TripleDes | Self::Aes192 | Self::Camellia192 => 24,
            Self::Des => 8,
            Self::Aes256 | Self::Twofish | Self::Camellia256 => 32,
        }
    }

    /// The length of the block of the cipher, in bytes.
    pub fn block_len(self) -> usize {
        match self {
            Self::Idea | Self::TripleDes | Self::Cast5 | Self::Blowfish | Self::Des => 8,
            Self::Aes128
            | Self::Aes192
            | Self::Aes256
            | Self::Twofish
            | Self::Camellia128
            | Self::Camellia192
            | Self::Camellia256 => 16,
        }
    }

    /// The data encrypted, or decrypted, in CFB mode under the session key, with the all-zero
    /// initialisation vector of the encrypted data packets.
    pub fn crypt(self, key: &[u8], data: &[u8], encrypt: bool) -> Result<Vec<u8>, PgpError> {
        let iv = vec![0; self.block_len()];
        Ok(match self {
            Self::Idea => cfb(&keyed::<Idea>(self, key)?, &iv, data, encrypt),
            Self::TripleDes => cfb(&keyed::<TdesEde3>(self, key)?, &iv, data, encrypt),
            Self::Cast5 => cfb(&keyed::<Cast5>(self, key)?, &iv, data, encrypt),
            Self::Blowfish => cfb(&keyed::<Blowfish>(self, key)?, &iv, data, encrypt),
            Self::Des => cfb(&keyed::<Des>(self, key)?, &iv, data, encrypt),
            Self::Aes128 => cfb(&keyed::<Aes128>(self, key)?, &iv, data, encrypt),
            Self::Aes192 => cfb(&keyed::<Aes192>(self, key)?, &iv, data, encrypt),
            Self::Aes256 => cfb(&keyed::<Aes256>(self, key)?, &iv, data, encrypt),
            Self::Twofish => cfb(&keyed::<Twofish>(self, key)?, &iv, data, encrypt),
            Self::Camellia128 => cfb(&keyed::<Camellia128>(self, key)?, &iv, data, encrypt),
            Self::Camellia192 => cfb(&keyed::<Camellia192>(self, key)?, &iv, data, encrypt),
            Self::Camellia256 => cfb(&keyed::<Camellia256>(self, key)?, &iv, data, encrypt),
        })
    }
}

/// The cipher of `cipher` keyed with `key`, when the key is the length the cipher takes: some
/// ciphers accept other lengths, which OpenPGP does not.
fn keyed<C: KeyInit>(cipher: Cipher, key: &[u8]) -> Result<C, PgpError> {
    C::new_from_slice(key)
        .ok()
        .filter(|_| key.len() == cipher.key_len())
        .ok_or(PgpError::SessionKeyLength {
            length: key.len(),
            expected: cipher.key_len(),
        })
}

/// The data of a block cipher in CFB mode with full blocks (RFC 4880 §13.9): the keystream of
/// each block is the cipher of the previous ciphertext block, and the first is that of the
/// initialisation vector. A last block that is partial is kept as it is.
pub fn cfb<C: BlockCipherEncrypt>(cipher: &C, iv: &[u8], input: &[u8], encrypt: bool) -> Vec<u8> {
    let mut register = Block::<C>::default();
    for (slot, byte) in register.iter_mut().zip(iv) {
        *slot = *byte;
    }
    let mut out = Vec::with_capacity(input.len());
    for chunk in input.chunks(C::block_size()) {
        let mut keystream = register.clone();
        cipher.encrypt_block(&mut keystream);
        let produced: Vec<u8> = chunk
            .iter()
            .zip(&keystream)
            .map(|(byte, key)| byte ^ key)
            .collect();
        let feedback: &[u8] = if encrypt { &produced } else { chunk };
        for (slot, byte) in register.iter_mut().zip(feedback) {
            *slot = *byte;
        }
        out.extend(produced);
    }
    out
}
