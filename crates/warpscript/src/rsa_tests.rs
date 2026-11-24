// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the RSA of the `RSA*` functions. The signatures that Warp 10 3.5.0 makes, and the
//! ciphertexts it reads, are compared in `functions/rsa_tests.rs`; here are the blocks of
//! PKCS#1 and PSS that are built with plain integers, and the choice of the primes of a key,
//! driven by the random bytes of the tests.

use num_bigint::{BigInt, BigUint};

use crate::error::crypto::{BlockError, InputTooLarge, Rejection, RsaError};
use crate::functions::rsa_tables_tests::KEYS;
use crate::rsa::{Exponentiation, Hash, PssHash, RsaKey, Scheme, generate};
use crate::tests::scripted;

/// The public exponent of the keys of the tables.
const PUBLIC_EXPONENT: u32 = 65537;

/// The private and the public key of `bits` bits of the tables.
fn keys(bits: &str) -> (RsaKey, RsaKey) {
    let (_, modulus, private) = KEYS
        .iter()
        .find(|(size, _, _)| *size == bits)
        .expect("a key of this size");
    let modulus: BigInt = modulus.parse().expect("a modulus");
    (
        RsaKey {
            modulus: modulus.clone(),
            exponent: private.parse().expect("an exponent"),
        },
        RsaKey {
            modulus,
            exponent: BigInt::from(PUBLIC_EXPONENT),
        },
    )
}

/// The `size` bytes of a number below the modulus raised to the exponent of a key.
fn power(key: &RsaKey, value: &[u8]) -> Vec<u8> {
    let modulus = key.modulus.to_biguint().expect("a modulus");
    let exponent = key.exponent.to_biguint().expect("an exponent");
    let power = BigUint::from_bytes_be(value).modpow(&exponent, &modulus);
    let size = usize::try_from(modulus.bits().div_ceil(8)).expect("a size");
    let mut bytes = vec![0; size];
    let digits = power.to_bytes_be();
    bytes[size - digits.len()..].copy_from_slice(&digits);
    bytes
}

/// A block of the PKCS#1 of `size` bytes: `00`, the type, the padding, `00` and the data.
fn block(size: usize, kind: u8, fill: u8, data: &[u8]) -> Vec<u8> {
    let mut block = vec![0, kind];
    block.resize(size - data.len() - 1, fill);
    block.push(0);
    block.extend_from_slice(data);
    block
}

/// The size of a digest of the PSS signatures, in bytes.
fn digest_size(hash: PssHash) -> u8 {
    match hash {
        PssHash::Sha1 => 20,
        PssHash::Sha224 => 28,
        PssHash::Sha256 => 32,
        PssHash::Sha384 => 48,
        PssHash::Sha512 => 64,
    }
}

#[test]
fn names_tell_the_scheme() {
    let digests = [
        ("MD2", Hash::Md2),
        ("MD4", Hash::Md4),
        ("MD5", Hash::Md5),
        ("SHA1", Hash::Sha1),
        ("SHA224", Hash::Sha224),
        ("SHA256", Hash::Sha256),
        ("SHA384", Hash::Sha384),
        ("SHA512", Hash::Sha512),
        ("RIPEMD128", Hash::Ripemd128),
        ("RMD128", Hash::Ripemd128),
        ("RIPEMD160", Hash::Ripemd160),
        ("RMD160", Hash::Ripemd160),
        ("RIPEMD256", Hash::Ripemd256),
        ("RMD256", Hash::Ripemd256),
    ];
    for (name, hash) in digests {
        assert_eq!(
            Scheme::lookup(&format!("{name}WITHRSA")),
            Some(Scheme::Pkcs1(hash))
        );
        assert_eq!(
            Scheme::lookup(&format!("{}withrsaencryption", name.to_lowercase())),
            Some(Scheme::Pkcs1(hash))
        );
    }
    let pss_digests = [
        ("SHA1", PssHash::Sha1),
        ("SHA224", PssHash::Sha224),
        ("SHA256", PssHash::Sha256),
        ("SHA384", PssHash::Sha384),
        ("SHA512", PssHash::Sha512),
    ];
    for (name, _) in digests {
        let expected = pss_digests
            .iter()
            .find(|(pss, _)| *pss == name)
            .map(|(_, hash)| Scheme::Pss(*hash));
        for suffix in ["WITHRSAANDMGF1", "WITHRSA/PSS"] {
            assert_eq!(
                Scheme::lookup(&format!("{name}{suffix}")),
                expected,
                "{name}{suffix}"
            );
        }
    }
    for name in ["NONEWITHRSA", "RSA", "RAWRSA", "rawrsa"] {
        assert_eq!(Scheme::lookup(name), Some(Scheme::Pkcs1Raw), "{name}");
    }
    for name in [
        "RSASSA-PSS",
        "1.2.840.113549.1.1.10",
        "OID.1.2.840.113549.1.1.10",
    ] {
        assert_eq!(
            Scheme::lookup(name),
            Some(Scheme::Pss(PssHash::Sha1)),
            "{name}"
        );
    }
    for (oid, hash) in [
        ("1.2.840.113549.1.1.2", Hash::Md2),
        ("1.2.840.113549.1.1.3", Hash::Md4),
        ("1.2.840.113549.1.1.4", Hash::Md5),
        ("1.2.840.113549.1.1.5", Hash::Sha1),
        ("1.3.14.3.2.29", Hash::Sha1),
        ("1.2.840.113549.1.1.11", Hash::Sha256),
        ("1.2.840.113549.1.1.12", Hash::Sha384),
        ("1.2.840.113549.1.1.13", Hash::Sha512),
        ("1.2.840.113549.1.1.14", Hash::Sha224),
        ("1.3.36.3.3.1.2", Hash::Ripemd160),
        ("1.3.36.3.3.1.3", Hash::Ripemd128),
        ("1.3.36.3.3.1.4", Hash::Ripemd256),
    ] {
        assert_eq!(Scheme::lookup(oid), Some(Scheme::Pkcs1(hash)), "{oid}");
        assert_eq!(
            Scheme::lookup(&format!("oid.{oid}")),
            Some(Scheme::Pkcs1(hash)),
            "oid.{oid}"
        );
    }
    for name in [
        "",
        "SHA3-256WITHRSA",
        "SHA256WITHECDSA",
        "SHA256",
        "WITHRSA",
        "ISO9796-2",
        "NONEWITHRSAANDMGF1",
        "RAWRSASSA-PSS",
        "1.2.840.113549.1.1.99",
        "OID.OID.1.2.840.113549.1.1.11",
        " SHA1WITHRSA",
    ] {
        assert_eq!(Scheme::lookup(name), None, "{name}");
    }
}

#[test]
fn a_key_that_is_not_an_rsa_key_has_no_exponentiation() {
    let key = |modulus: &str, exponent: i32| RsaKey {
        modulus: modulus.parse().expect("a modulus"),
        exponent: BigInt::from(exponent),
    };
    let error = |modulus: &str, exponent: i32| {
        Exponentiation::new(&key(modulus, exponent))
            .err()
            .expect("no exponentiation")
    };
    let one = BigInt::from(1u8);
    let odd_88 = ((one.clone() << 87u32) + 1u8).to_string();
    let even_88 = (one.clone() << 87u32).to_string();
    let odd_96 = ((one.clone() << 96u32) + 1u8).to_string();
    let even_96 = ((one.clone() << 96u32) + 2u8).to_string();
    let odd_89 = ((one << 88u32) + 1u8).to_string();
    assert_eq!(error("-5", 3), RsaError::ModulusNotPositive);
    assert_eq!(error("-5", 0), RsaError::ModulusNotPositive);
    assert_eq!(error("0", 3), RsaError::ModulusTooSmall { bits: 0 });
    assert_eq!(error("5", 3), RsaError::ModulusTooSmall { bits: 3 });
    assert_eq!(error(&odd_88, 3), RsaError::ModulusTooSmall { bits: 88 });
    assert_eq!(error(&odd_88, 0), RsaError::ModulusTooSmall { bits: 88 });
    assert_eq!(error(&even_88, 3), RsaError::ModulusTooSmall { bits: 88 });
    assert_eq!(error(&odd_96, 0), RsaError::ExponentNotPositive);
    assert_eq!(error(&odd_96, -3), RsaError::ExponentNotPositive);
    assert_eq!(error(&even_96, 3), RsaError::EvenModulus);
    // 89 bits hold a block with a byte of data (12 bytes), what 88 bits do not.
    let small = Exponentiation::new(&key(&odd_89, 3)).expect("a block of one byte");
    assert_eq!(small.encrypt(b"a").len(), 12);
}

#[test]
fn blocks_of_data_are_read_in_types_1_and_2() {
    let (private, public) = keys("1024");
    let key = Exponentiation::new(&private).expect("a key");
    let size = 128;
    for (kind, fill) in [(1, 0xff), (2, 0x55)] {
        let ciphertext = power(&public, &block(size, kind, fill, b"abc"));
        assert_eq!(key.decrypt(&ciphertext), Ok(b"abc".to_vec()), "type {kind}");
    }
    // Several blocks are read, one after the other.
    let first = power(&public, &block(size, 1, 0xff, b"abc"));
    let second = power(&public, &block(size, 2, 0x55, b"def"));
    assert_eq!(
        key.decrypt(&[first, second].concat()),
        Ok(b"abcdef".to_vec())
    );
    // The eight bytes of padding are the least a block has, and no data is a block.
    assert_eq!(
        key.decrypt(&power(&public, &block(size, 1, 0xff, b""))),
        Ok(Vec::new())
    );
    let mut eight = vec![0, 1];
    eight.extend([0xff; 8]);
    eight.push(0);
    eight.resize(size, 7);
    assert_eq!(
        key.decrypt(&power(&public, &eight)),
        Ok(vec![7; size - 11]),
        "eight bytes of padding"
    );
    assert_eq!(key.decrypt(&[]), Ok(Vec::new()));
}

#[test]
fn what_is_not_a_block_of_data_is_told() {
    let (private, public) = keys("1024");
    let key = Exponentiation::new(&private).expect("a key");
    let size = 128;
    let decrypt = |block: &[u8]| key.decrypt(&power(&public, block));
    // Fewer bytes than a block once its leading zeroes are gone, and more.
    assert_eq!(
        decrypt(&[&[0, 0], &block(size, 1, 0xff, b"abc")[2..]].concat()),
        Err(RsaError::Block(BlockError::Truncated))
    );
    assert_eq!(
        decrypt(&[&[1], &block(size, 1, 0xff, b"abc")[1..]].concat()),
        Err(RsaError::Block(BlockError::IncorrectSize))
    );
    assert_eq!(
        decrypt(&block(size, 3, 0xff, b"abc")),
        Err(RsaError::Block(BlockError::UnknownType))
    );
    assert_eq!(
        decrypt(&block(size, 1, 0xfe, b"abc")),
        Err(RsaError::Block(BlockError::PaddingIncorrect))
    );
    // Less than eight bytes of padding, and no end to the padding.
    let mut seven = vec![0, 1];
    seven.extend([0xff; 7]);
    seven.push(0);
    seven.resize(size, 7);
    assert_eq!(
        decrypt(&seven),
        Err(RsaError::Block(BlockError::NoData)),
        "seven bytes of padding"
    );
    let mut endless = vec![0, 1];
    endless.resize(size, 0xff);
    assert_eq!(
        decrypt(&endless),
        Err(RsaError::Block(BlockError::NoData)),
        "no end to the padding"
    );
    // A block that is not below the modulus.
    assert_eq!(
        key.decrypt(&vec![0xff; size]),
        Err(RsaError::InputTooLarge(InputTooLarge))
    );
    let modulus = private
        .modulus
        .to_biguint()
        .expect("a modulus")
        .to_bytes_be();
    assert_eq!(
        key.decrypt(&modulus),
        Err(RsaError::InputTooLarge(InputTooLarge))
    );
    // The second block is the one that fails.
    let good = power(&public, &block(size, 1, 0xff, b"abc"));
    assert_eq!(
        key.decrypt(&[good, vec![0xff; size]].concat()),
        Err(RsaError::InputTooLarge(InputTooLarge))
    );
}

#[test]
fn a_ciphertext_fills_blocks_of_the_size_of_the_modulus() {
    for bits in ["512", "1024", "1025", "1100"] {
        let (private, public) = keys(bits);
        let size = usize::try_from(private.modulus.bits().div_ceil(8)).expect("a size");
        let capacity = size - 11;
        let with_public = Exponentiation::new(&public).expect("a key");
        let with_private = Exponentiation::new(&private).expect("a key");
        for length in [0, 1, capacity - 1, capacity, capacity + 1, 3 * capacity] {
            let data: Vec<u8> = (0..length).map(|i| (i % 251) as u8).collect();
            // Both keys encrypt, the other decrypts.
            let ciphertext = with_private.encrypt(&data);
            assert_eq!(ciphertext.len(), length.div_ceil(capacity) * size, "{bits}");
            assert_eq!(with_public.decrypt(&ciphertext), Ok(data.clone()), "{bits}");
            let ciphertext = with_public.encrypt(&data);
            assert_eq!(with_private.decrypt(&ciphertext), Ok(data), "{bits}");
        }
    }
}

#[test]
fn a_ciphertext_is_the_power_of_the_block_of_type_1() {
    let (private, public) = keys("1024");
    let data = b"hello world";
    let expected = power(&public, &block(128, 1, 0xff, data));
    let ciphertext = Exponentiation::new(&private).expect("a key").encrypt(data);
    // The key of the private exponent encrypts to the power of the private exponent.
    assert_eq!(ciphertext, power(&private, &block(128, 1, 0xff, data)));
    assert_ne!(ciphertext, expected);
    let ciphertext = Exponentiation::new(&public).expect("a key").encrypt(data);
    assert_eq!(ciphertext, expected);
}

#[test]
fn a_random_ciphertext_is_a_block_of_type_2_with_nonzero_padding() {
    let (private, public) = keys("1024");
    let key = Exponentiation::new(&public).expect("a key");
    // The first draw is a zero, which is drawn again: the padding is 1 to 122.
    let mut bytes = vec![0, 0, 0];
    bytes.extend(1..=200u8);
    let ciphertext = key
        .encrypt_random(b"abc", &mut scripted(bytes))
        .expect("a ciphertext");
    let mut expected = vec![0, 2];
    expected.extend(1..=122u8);
    expected.push(0);
    expected.extend_from_slice(b"abc");
    assert_eq!(power(&private, &ciphertext), expected);
    assert_eq!(
        Exponentiation::new(&private)
            .expect("a key")
            .decrypt(&ciphertext),
        Ok(b"abc".to_vec())
    );
}

#[test]
fn a_random_ciphertext_holds_a_payload_of_the_capacity_of_its_block() {
    for bits in ["512", "1024", "1025", "1100"] {
        let (private, public) = keys(bits);
        let size = usize::try_from(private.modulus.bits().div_ceil(8)).expect("a size");
        let capacity = size - 11;
        let with_public = Exponentiation::new(&public).expect("a key");
        let with_private = Exponentiation::new(&private).expect("a key");
        let data: Vec<u8> = (0..capacity).map(|i| (i % 251) as u8).collect();
        let ciphertext = with_public
            .encrypt_random(&data, &mut scripted(vec![7; 2 * size]))
            .expect("a ciphertext");
        assert_eq!(ciphertext.len(), size, "{bits}");
        assert_eq!(with_private.decrypt(&ciphertext), Ok(data), "{bits}");
        assert_eq!(
            with_public.encrypt_random(&vec![0; capacity + 1], &mut scripted(Vec::new())),
            Err(RsaError::DataTooLarge),
            "{bits}"
        );
    }
}

#[test]
fn a_random_ciphertext_needs_entropy_for_its_padding() {
    let (_, public) = keys("1024");
    let key = Exponentiation::new(&public).expect("a key");
    assert_eq!(
        key.encrypt_random(b"abc", &mut scripted(vec![1; 10])),
        Err(RsaError::Random(getrandom::Error::UNSUPPORTED))
    );
}

/// The keys of a size and the signer of a scheme on it.
fn sign_and_verify(bits: &str, scheme: Scheme, salt: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let (private, public) = keys(bits);
    let key = Exponentiation::new(&private).expect("a key");
    let signature = key
        .signer(scheme)
        .expect("a signer")
        .sign(b"hello", &mut scripted(salt))
        .expect("a signature");
    assert_eq!(
        Exponentiation::new(&public)
            .expect("a key")
            .signer(scheme)
            .expect("a signer")
            .verify(b"hello", &signature),
        Ok(())
    );
    let block = power(&public, &signature);
    (signature, block)
}

#[test]
fn a_pkcs1_signature_is_the_power_of_the_block_of_the_digest_info() {
    let (_, public) = keys("1024");
    let (signature, decoded) = sign_and_verify("1024", Scheme::Pkcs1(Hash::Sha1), Vec::new());
    // `00 01 ff … ff 00` and the `DigestInfo` of SHA-1 of `hello`.
    assert_eq!(decoded[..2], [0, 1]);
    let digest_info =
        hex_decode("3021300906052b0e03021a05000414aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    assert_eq!(decoded[128 - digest_info.len() - 1], 0);
    assert_eq!(decoded[128 - digest_info.len()..], digest_info[..]);
    assert!(
        decoded[2..128 - digest_info.len() - 1]
            .iter()
            .all(|byte| *byte == 0xff)
    );
    assert_eq!(power(&public, &signature), decoded);
}

fn hex_decode(text: &str) -> Vec<u8> {
    crate::encoding::hex_decode(text).expect("hexadecimal digits")
}

#[test]
fn a_signature_is_not_accepted_when_it_is_not_the_one_of_the_data() {
    let (private, public) = keys("1024");
    let signer_key = Exponentiation::new(&private).expect("a key");
    let verifier = Exponentiation::new(&public).expect("a key");
    let scheme = Scheme::Pkcs1(Hash::Sha1);
    let signature = signer_key
        .signer(scheme)
        .expect("a signer")
        .sign(b"hello", &mut scripted(Vec::new()))
        .expect("a signature");
    let verify = |data: &[u8], signature: &[u8]| {
        verifier
            .signer(scheme)
            .expect("a verifier")
            .verify(data, signature)
    };
    assert_eq!(verify(b"hello", &signature), Ok(()));
    assert_eq!(
        verify(b"hellp", &signature),
        Err(Rejection::SignatureMismatch)
    );
    assert_eq!(
        verify(b"hello", &[0xff; 128]),
        Err(Rejection::InputTooLarge(InputTooLarge))
    );
    assert_eq!(
        verify(b"hello", &[1; 129]),
        Err(Rejection::InputTooLarge(InputTooLarge))
    );
    assert_eq!(
        verify(b"hello", &[1]),
        Err(Rejection::Block(BlockError::Truncated))
    );
    // A block of the digest info of another digest, with the same size.
    let other = block(
        128,
        1,
        0xff,
        &hex_decode("3021300906052b0e03021a05000414000000000000000000000000000000000000000000"),
    );
    assert_eq!(
        verify(b"hello", &power(&private, &other)),
        Err(Rejection::SignatureMismatch)
    );
}

#[test]
fn a_digest_info_without_null_is_accepted_whole() {
    let (private, public) = keys("1024");
    let verifier = Exponentiation::new(&public).expect("a key");
    let signer = verifier
        .signer(Scheme::Pkcs1(Hash::Sha1))
        .expect("a verifier");
    // `DigestInfo` of SHA-1 of `hello` without the `05 00` of the parameters of the algorithm.
    let digest = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";
    let short = |digest: &str| hex_decode(&format!("301f300706052b0e03021a0414{digest}"));
    let sign = |info: &[u8]| power(&private, &block(128, 1, 0xff, info));
    assert_eq!(signer.verify(b"hello", &sign(&short(digest))), Ok(()));
    // BouncyCastle leaves the last two bytes of the digest unchecked in that form: not here.
    let altered = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea90000";
    assert_eq!(
        signer.verify(b"hello", &sign(&short(altered))),
        Err(Rejection::SignatureMismatch)
    );
    let truncated = &short(digest)[..short(digest).len() - 1];
    assert_eq!(
        signer.verify(b"hello", &sign(truncated)),
        Err(Rejection::SignatureMismatch)
    );
}

#[test]
fn a_raw_signature_holds_the_data_in_a_block() {
    let (private, public) = keys("1024");
    let signer_key = Exponentiation::new(&private).expect("a key");
    let verifier = Exponentiation::new(&public).expect("a key");
    let signer = signer_key.signer(Scheme::Pkcs1Raw).expect("a signer");
    let signature = signer
        .sign(b"hello", &mut scripted(Vec::new()))
        .expect("a signature");
    assert_eq!(power(&public, &signature), block(128, 1, 0xff, b"hello"));
    let verifier = verifier.signer(Scheme::Pkcs1Raw).expect("a verifier");
    assert_eq!(verifier.verify(b"hello", &signature), Ok(()));
    assert_eq!(
        verifier.verify(b"hellp", &signature),
        Err(Rejection::SignatureMismatch)
    );
    assert_eq!(
        verifier.verify(b"hello", &power(&private, &block(128, 3, 0xff, b"hello"))),
        Err(Rejection::Block(BlockError::UnknownType))
    );
    // 117 bytes fill a block, one more do not fit.
    let fits = vec![5; 117];
    let signature = signer
        .sign(&fits, &mut scripted(Vec::new()))
        .expect("a signature");
    assert_eq!(verifier.verify(&fits, &signature), Ok(()));
    assert_eq!(
        signer.sign(&[5; 118], &mut scripted(Vec::new())),
        Err(RsaError::DataTooLarge)
    );
}

#[test]
fn a_pss_signature_is_made_of_the_salt_it_is_given() {
    let salt: Vec<u8> = (0..20).collect();
    let (signature, em) = sign_and_verify("1024", Scheme::Pss(PssHash::Sha1), salt.clone());
    let (again, _) = sign_and_verify("1024", Scheme::Pss(PssHash::Sha1), salt);
    assert_eq!(signature, again, "the same salt, the same signature");
    let (other, _) = sign_and_verify("1024", Scheme::Pss(PssHash::Sha1), (1..21).collect());
    assert_ne!(signature, other);
    // The block of 1023 bits: the first bit is 0, the last byte `bc`.
    assert_eq!(em.len(), 128);
    assert_eq!(em[0] & 0x80, 0);
    assert_eq!(em[127], 0xbc);
}

#[test]
fn pss_signatures_of_every_size_of_modulus_are_verified() {
    // 1024 bits (a block of 1023), 1025 (a block of 1024: whole bytes) and 1100 (1099).
    for bits in ["1024", "1025", "1100"] {
        for hash in [
            PssHash::Sha1,
            PssHash::Sha224,
            PssHash::Sha256,
            PssHash::Sha384,
            PssHash::Sha512,
        ] {
            if bits != "1100" && hash == PssHash::Sha512 {
                continue;
            }
            let salt = (0..digest_size(hash)).collect();
            sign_and_verify(bits, Scheme::Pss(hash), salt);
        }
    }
}

#[test]
fn a_pss_block_that_is_not_one_is_rejected() {
    let (private, public) = keys("1024");
    let scheme = Scheme::Pss(PssHash::Sha256);
    let signer_key = Exponentiation::new(&private).expect("a key");
    let verifier = Exponentiation::new(&public).expect("a key");
    let signature = signer_key
        .signer(scheme)
        .expect("a signer")
        .sign(b"hello", &mut scripted((0..32).collect()))
        .expect("a signature");
    let verify = |data: &[u8], signature: &[u8]| {
        verifier
            .signer(scheme)
            .expect("a verifier")
            .verify(data, signature)
    };
    assert_eq!(verify(b"hello", &signature), Ok(()));
    assert_eq!(
        verify(b"hellp", &signature),
        Err(Rejection::SignatureMismatch)
    );
    let em = power(&public, &signature);
    let tampered = |index: usize, mask: u8| {
        let mut em = em.clone();
        em[index] ^= mask;
        power(&private, &em)
    };
    // The trailer, a bit of the padding (the first byte), the `01` that precedes the salt.
    assert_eq!(
        verify(b"hello", &tampered(127, 1)),
        Err(Rejection::PssTrailer)
    );
    assert_eq!(
        verify(b"hello", &tampered(0, 1)),
        Err(Rejection::PssPadding)
    );
    assert_eq!(
        verify(b"hello", &tampered(128 - 2 * 32 - 2, 3)),
        Err(Rejection::PssPadding)
    );
    // The salt is part of the digest: another one is not the signature of the data.
    assert_eq!(
        verify(b"hello", &tampered(128 - 32 - 2, 1)),
        Err(Rejection::SignatureMismatch)
    );
    // The modulus of 1025 bits has a block of 1024: a value of 1025 bits is not one.
    let (private, public) = keys("1025");
    let verifier = Exponentiation::new(&public).expect("a key");
    let too_large = power(&private, &[&[1], vec![0; 127].as_slice(), &[5]].concat());
    assert_eq!(
        verifier
            .signer(scheme)
            .expect("a verifier")
            .verify(b"hello", &too_large),
        Err(Rejection::PssBlockSize)
    );
}

#[test]
fn pss_needs_a_modulus_that_holds_two_digests() {
    let (private, _) = keys("1024");
    let key = Exponentiation::new(&private).expect("a key");
    assert_eq!(
        key.signer(Scheme::Pss(PssHash::Sha512)).err(),
        Some(RsaError::PssKeyTooSmall)
    );
    assert!(key.signer(Scheme::Pss(PssHash::Sha384)).is_ok());
    assert_eq!(
        key.signer(Scheme::Pss(PssHash::Sha256))
            .expect("a signer")
            .sign(b"hello", &mut scripted(Vec::new())),
        Err(RsaError::Random(getrandom::Error::UNSUPPORTED))
    );
    let (small, _) = keys("512");
    let key = Exponentiation::new(&small).expect("a key");
    for hash in [PssHash::Sha512, PssHash::Sha384, PssHash::Sha256] {
        assert_eq!(
            key.signer(Scheme::Pss(hash)).err(),
            Some(RsaError::PssKeyTooSmall),
            "{hash:?}"
        );
    }
    assert!(key.signer(Scheme::Pss(PssHash::Sha224)).is_ok());
}

/// The 2-byte candidates (big-endian) of a source of random bytes.
fn candidates(numbers: &[u32]) -> Vec<u8> {
    numbers
        .iter()
        .flat_map(|number| number.to_be_bytes()[2..].to_vec())
        .collect()
}

/// A key pair of 24 bits for the public exponent 3, from the primes of 12 bits the source gives:
/// its modulus and its private exponent.
fn generated(exponent: &str, strength: i32, bytes: Vec<u8>) -> Result<(String, String), RsaError> {
    let exponent: BigInt = exponent.parse().expect("an exponent");
    let pair = generate(&exponent, strength, &mut scripted(bytes))?;
    assert_eq!(pair.private.modulus, pair.public.modulus);
    assert_eq!(pair.public.exponent, exponent);
    Ok((
        pair.private.modulus.to_string(),
        pair.private.exponent.to_string(),
    ))
}

/// The key of the primes 2897 and 3167, for the exponent 3: its modulus and private exponent.
fn the_key() -> Result<(String, String), RsaError> {
    Ok(("9174799".to_owned(), "1528123".to_owned()))
}

#[test]
fn a_key_is_made_of_two_primes_that_the_source_gives() {
    assert_eq!(generated("3", 24, candidates(&[2897, 3167])), the_key());
    // The primes in the other order make the same key.
    assert_eq!(generated("3", 24, candidates(&[3167, 2897])), the_key());
}

#[test]
fn candidates_that_are_not_primes_for_a_key_are_drawn_again() {
    // A composite (3003), a prime whose square is below 2^23 (2053), a prime that is 1 modulo 3
    // (2917), then 2897.
    assert_eq!(
        generated("3", 24, candidates(&[3003, 2053, 2917, 2897, 3167])),
        the_key()
    );
    // The draws that come after a prime that is kept are those of the second one.
    assert_eq!(
        generated("3", 24, candidates(&[2897, 2053, 3003, 2917, 3167])),
        the_key()
    );
}

#[test]
fn the_bits_of_a_candidate_out_of_its_size_are_dropped() {
    // 26 bits: primes of 13 bits, a candidate on 2 bytes whose top 3 bits are noise, whose bit of
    // 13 bits and whose last bit are not set.
    assert_eq!(
        generated("3", 26, vec![0xe6, 0xa8, 0xa7, 0xc8]),
        Ok(("35322289".to_owned(), "1471267".to_owned()))
    );
}

#[test]
fn primes_that_are_too_close_make_no_key() {
    // The same prime twice, then two that are far apart.
    assert_eq!(
        generated("3", 24, candidates(&[2897, 2897, 2897, 3167])),
        the_key()
    );
}

#[test]
fn a_modulus_of_low_weight_makes_no_key() {
    // 3209 * 3911 has a non-adjacent form of 5 digits, the key needs 6.
    assert_eq!(
        generated("3", 24, candidates(&[3209, 3911, 2897, 3167])),
        the_key()
    );
}

#[test]
fn a_small_private_exponent_makes_no_key() {
    // With 2750621, 2897 and 3167 give a private exponent of 5, below 2^12: the pair is dropped.
    assert_eq!(
        generated("2750621", 24, candidates(&[2897, 3167, 2897, 3163])),
        Ok(("9163211".to_owned(), "660293".to_owned()))
    );
}

#[test]
fn a_key_is_given_up_after_a_hundred_pairs() {
    let pairs: Vec<u32> = [2897, 2897].repeat(100);
    assert_eq!(
        generated("3", 24, candidates(&pairs)),
        Err(RsaError::KeyNotFound)
    );
    // A hundred pairs and no more: the source is not asked for the 101st.
    let mut more = pairs;
    more.extend([2897, 3167]);
    assert_eq!(
        generated("3", 24, candidates(&more)),
        Err(RsaError::KeyNotFound)
    );
}

#[test]
fn a_prime_is_looked_for_in_five_draws_per_bit() {
    // 5 * 12 draws of a prime that is too small, for the first prime and for the second.
    let small = [2053; 60];
    assert_eq!(
        generated("3", 24, candidates(&small)),
        Err(RsaError::PrimeNotFound)
    );
    let mut second = vec![2897];
    second.extend(small);
    assert_eq!(
        generated("3", 24, candidates(&second)),
        Err(RsaError::PrimeNotFound)
    );
    // 5 * 6 draws of 47, a prime BouncyCastle takes for a factor of what it tests.
    assert_eq!(
        generated("3", 12, vec![0x2f; 30]),
        Err(RsaError::PrimeNotFound)
    );
    // The primes of 8 bits are all below 212 or too close together: after a hundred pairs, no key.
    let eight_bits = [191u8, 197, 199, 211, 227, 233, 239, 251];
    assert_eq!(
        generated("3", 16, eight_bits.repeat(2000)),
        Err(RsaError::KeyNotFound)
    );
}

#[test]
fn an_odd_strength_has_a_second_prime_that_is_too_small() {
    // 25 bits: a prime of 13 bits, then the second one has 12 bits whose square is below 2^24.
    assert_eq!(
        generated("3", 25, candidates(&[4127])),
        Err(RsaError::PrimeNotFound)
    );
}

#[test]
fn a_key_needs_random_bytes() {
    let exhausted = RsaError::Random(getrandom::Error::UNSUPPORTED);
    assert_eq!(generated("3", 24, Vec::new()), Err(exhausted.clone()));
    assert_eq!(generated("3", 24, candidates(&[2897])), Err(exhausted));
}

#[test]
fn what_cannot_be_a_key_is_refused_before_any_draw() {
    for strength in [i32::MIN, -64, 0, 11] {
        assert_eq!(
            generated("3", strength, Vec::new()),
            Err(RsaError::StrengthTooSmall),
            "{strength}"
        );
    }
    for exponent in ["0", "2", "4", "-2", "100"] {
        assert_eq!(
            generated(exponent, 24, Vec::new()),
            Err(RsaError::ExponentEven),
            "{exponent}"
        );
    }
    for exponent in ["-1", "-3"] {
        assert_eq!(
            generated(exponent, 24, Vec::new()),
            Err(RsaError::ModulusNotPositive),
            "{exponent}"
        );
    }
}

#[test]
fn failures_are_worded_like_those_of_warp_10() {
    let errors: [(&dyn std::error::Error, &str); 24] = [
        (
            &RsaError::ModulusNotPositive,
            "BigInteger: modulus not positive",
        ),
        (&RsaError::EvenModulus, "RSA modulus is even"),
        (
            &RsaError::ModulusTooSmall { bits: 8 },
            "RSA modulus of 8 bits is too small for PKCS#1 blocks",
        ),
        (
            &RsaError::ExponentNotPositive,
            "RSA exponent is not positive",
        ),
        (
            &RsaError::InputTooLarge(InputTooLarge),
            "input too large for RSA cipher.",
        ),
        (&RsaError::DataTooLarge, "input data too large"),
        (&RsaError::Block(BlockError::Truncated), "block truncated"),
        (
            &RsaError::Block(BlockError::IncorrectSize),
            "block incorrect size",
        ),
        (
            &RsaError::Block(BlockError::UnknownType),
            "unknown block type",
        ),
        (
            &RsaError::Block(BlockError::PaddingIncorrect),
            "block padding incorrect",
        ),
        (&RsaError::Block(BlockError::NoData), "no data in block"),
        (
            &RsaError::PssKeyTooSmall,
            "key too small for specified hash and salt lengths",
        ),
        (&RsaError::StrengthTooSmall, "key strength too small"),
        (&RsaError::ExponentEven, "public exponent cannot be even"),
        (
            &RsaError::PrimeNotFound,
            "unable to generate prime number for RSA key",
        ),
        (
            &RsaError::KeyNotFound,
            "unable to generate prime number for RSA key",
        ),
        (
            &Rejection::InputTooLarge(InputTooLarge),
            "input too large for RSA cipher.",
        ),
        (&Rejection::Block(BlockError::NoData), "no data in block"),
        (&Rejection::SignatureMismatch, "signature does not match"),
        (&Rejection::PssTrailer, "PSS block has no trailer"),
        (&Rejection::PssPadding, "PSS padding incorrect"),
        (&Rejection::PssBlockSize, "PSS block incorrect size"),
        (&InputTooLarge, "input too large for RSA cipher."),
        (&BlockError::NoData, "no data in block"),
    ];
    for (error, message) in errors {
        assert_eq!(error.to_string(), message);
    }
    assert!(
        RsaError::Random(getrandom::Error::UNSUPPORTED)
            .to_string()
            .starts_with("Unable to obtain random data from the operating system (")
    );
}
