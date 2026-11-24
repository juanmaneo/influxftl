// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of the symmetric ciphers: their names and identifiers, their key and block lengths, the
//! CFB data against known answers, and the session keys of the wrong length.

use super::symmetric::{Cipher, algorithm_id};
use crate::encoding::hex_decode;
use crate::error::pgp::PgpError;

/// The data of the known answers: 39 bytes, so that the last block is partial for every cipher.
const PLAIN: &[u8] = b"influxftl symmetric cipher check, 2026!";

const ALL: [Cipher; 12] = [
    Cipher::Idea,
    Cipher::TripleDes,
    Cipher::Cast5,
    Cipher::Blowfish,
    Cipher::Des,
    Cipher::Aes128,
    Cipher::Aes192,
    Cipher::Aes256,
    Cipher::Twofish,
    Cipher::Camellia128,
    Cipher::Camellia192,
    Cipher::Camellia256,
];

/// The bytes of a hexadecimal known answer.
fn bytes(text: &str) -> Vec<u8> {
    match hex_decode(text) {
        Ok(bytes) => bytes,
        Err(error) => panic!("the known answer must be hexadecimal: {error}"),
    }
}

/// The session key of `length` bytes that the known answers use.
fn key(length: usize) -> Vec<u8> {
    (0..length).map(|i| (i * 7 + 3) as u8).collect()
}

#[test]
fn every_name_gives_its_identifier() {
    let names = [
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
    for (name, id) in names {
        assert_eq!(algorithm_id(name), Ok(id));
    }
}

#[test]
fn a_name_is_refused_as_it_is_written() {
    for name in ["AES128", "aes_256", "NULL", "NOPE", "", "7"] {
        assert_eq!(
            algorithm_id(name),
            Err(PgpError::InvalidCipherName(name.to_owned()))
        );
    }
}

#[test]
fn an_identifier_gives_its_cipher_back() {
    for cipher in ALL {
        assert_eq!(Cipher::from_id(cipher.id()), Ok(cipher));
    }
}

#[test]
fn safer_and_the_unknown_identifiers_are_not_recognised() {
    for id in [0, 5, 14, 255] {
        assert_eq!(Cipher::from_id(id), Err(PgpError::UnrecognisedCipher));
    }
}

#[test]
fn key_and_block_lengths_are_those_of_openpgp() {
    let lengths = [
        (Cipher::Idea, 16, 8),
        (Cipher::TripleDes, 24, 8),
        (Cipher::Cast5, 16, 8),
        (Cipher::Blowfish, 16, 8),
        (Cipher::Des, 8, 8),
        (Cipher::Aes128, 16, 16),
        (Cipher::Aes192, 24, 16),
        (Cipher::Aes256, 32, 16),
        (Cipher::Twofish, 32, 16),
        (Cipher::Camellia128, 16, 16),
        (Cipher::Camellia192, 24, 16),
        (Cipher::Camellia256, 32, 16),
    ];
    for (cipher, key_len, block_len) in lengths {
        assert_eq!(cipher.key_len(), key_len, "{cipher:?}");
        assert_eq!(cipher.block_len(), block_len, "{cipher:?}");
    }
}

#[test]
fn cfb_matches_the_known_answers() {
    let answers = [
        (
            Cipher::Aes128,
            "241e3f2951756a509e981d04585f178b74f7a6cf2e6e016d15c68c19227de41f9d30530bdd131a",
        ),
        (
            Cipher::Aes192,
            "ecf47d692e65ee8742d3f971115b12c0cd6cf80bb6271bff9817a6b070c91f54ed0df8c34f6964",
        ),
        (
            Cipher::Aes256,
            "486a5dd4360a8b03c6a0e821d4326074afe4670a557cffed03a6002c65c555cd9a6ab276ac58c0",
        ),
        (
            Cipher::Camellia128,
            "3a78fd828c0e85f65c54f32798060591191284c8031dc07087bc7d47f39f088ceeaaa324b91c89",
        ),
        (
            Cipher::Camellia192,
            "7a68283fd921b4c39fa40b3dc0ba1554068f4019e855f30f2bb1e4adc3a57cee1e9546f2b3eb6d",
        ),
        (
            Cipher::Camellia256,
            "c8eaf22ba412a522ef42706deddb8f5a5f214bab4c7bf5d3bfb33d2b6b14aa8aa5be386b440226",
        ),
        (
            Cipher::TripleDes,
            "749ebc6ebb285a10f7bb6cba8e3eb0924340a04b7f4f4092edf5ae7162215ce995f7cdb1cc2944",
        ),
        (
            Cipher::Blowfish,
            "e158ed3b7b627df9096bcf4de1b5617b83ba0f9751ad5a4d1199f337300372fe83bf05dbe69bc1",
        ),
        (
            Cipher::Cast5,
            "b7dd6efd832e84d14fc8747cc8baca6e269a3b0241d679982282e92079d0d3e55ddee778680815",
        ),
        (
            Cipher::Des,
            "3d16511ff6b3e6b2fa7fa582f5b282b3e346fcdbfd5b3220b280f758fafa84a0eb9aa9f2bb4de1",
        ),
    ];
    for (cipher, answer) in answers {
        assert_eq!(
            cipher.crypt(&key(cipher.key_len()), PLAIN, true),
            Ok(bytes(answer)),
            "{cipher:?}"
        );
    }
}

#[test]
fn decryption_inverts_encryption_at_every_length() {
    for cipher in ALL {
        let key = key(cipher.key_len());
        for length in [0usize, 1, 7, 8, 9, 15, 16, 17, 39] {
            let plain: Vec<u8> = (0..length).map(|i| (i as u8) ^ 0x5a).collect();
            let encrypted = cipher.crypt(&key, &plain, true);
            assert_eq!(encrypted.clone().map(|data| data.len()), Ok(length));
            assert_eq!(
                encrypted.and_then(|data| cipher.crypt(&key, &data, false)),
                Ok(plain),
                "{cipher:?}"
            );
        }
    }
}

#[test]
fn a_session_key_of_another_length_is_refused_for_every_cipher() {
    for cipher in ALL {
        for length in [3usize, 20] {
            assert_eq!(
                cipher.crypt(&key(length), PLAIN, true),
                Err(PgpError::SessionKeyLength {
                    length,
                    expected: cipher.key_len(),
                }),
                "{cipher:?}"
            );
        }
    }
}
