// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! RSA of the `RSA*` functions: the generation of keys, the encryption by blocks, and the
//! signatures of PKCS#1 v1.5 and PSS.
//!
//! A key is what Warp 10 holds: a modulus and one exponent (no prime), so everything is a
//! modular exponentiation of the whole modulus. It is `crypto-bigint`'s, in constant time in
//! the exponent, which is the secret of a private key. What is built here is what
//! BouncyCastle does around it: the blocks of PKCS#1 (type 1 to encrypt, types 1 and 2 to
//! decrypt), the `DigestInfo` of the signatures, the PSS encoding, and the choice of the primes
//! of a new key.

use std::cmp::Ordering;

use crypto_bigint::modular::{BoxedMontyForm, BoxedMontyParams};
use crypto_bigint::{BoxedUint, Odd, Resize};
use crypto_primes::{Flavor, is_prime};
use digest::Digest;
use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use num_traits::One;

use crate::ec::Random;
use crate::error::crypto::{BlockError, InputTooLarge, Rejection, RsaError};

/// The framing of a block of PKCS#1 in the bytes of the modulus: a leading `00` that BouncyCastle
/// leaves out of its blocks, the type, a `00` before the data and at least 8 bytes of padding.
const BLOCK_OVERHEAD: usize = 11;

/// The bytes before the data of a block of PKCS#1 (its type, 8 of padding, the `00`) and the
/// leading `00` that is not in the block.
const HEADER_LENGTH: usize = 10;

/// The size of the nonce of `RSAENCRYPT`, in bytes.
pub const NONCE: usize = 8;

/// The smallest strength of a key BouncyCastle generates.
const MIN_STRENGTH: i32 = 12;

/// The pairs of primes that are tried before a key is given up.
const KEY_ATTEMPTS: usize = 100;

/// The largest prime that BouncyCastle's trial division (`Primes.hasAnySmallFactors`) knows. It
/// divides by every prime up to it, the prime itself included: a prime that is not above is
/// refused as a factor of a key.
const TRIAL_DIVISION_LIMIT: u32 = 211;

/// The trailer of a PSS block.
const PSS_TRAILER: u8 = 0xbc;

/// An RSA key as Warp 10 holds it: the modulus and the exponent (public or private).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RsaKey {
    /// The modulus.
    pub modulus: BigInt,
    /// The exponent.
    pub exponent: BigInt,
}

/// A digest of the signatures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hash {
    /// MD2.
    Md2,
    /// MD4.
    Md4,
    /// MD5.
    Md5,
    /// SHA-1.
    Sha1,
    /// SHA-224.
    Sha224,
    /// SHA-256.
    Sha256,
    /// SHA-384.
    Sha384,
    /// SHA-512.
    Sha512,
    /// RIPEMD-128.
    Ripemd128,
    /// RIPEMD-160.
    Ripemd160,
    /// RIPEMD-256.
    Ripemd256,
}

impl Hash {
    /// The digest an upper case name of a signature algorithm (`SHA256`) tells.
    fn lookup(name: &str) -> Option<Self> {
        Some(match name {
            "MD2" => Hash::Md2,
            "MD4" => Hash::Md4,
            "MD5" => Hash::Md5,
            "SHA1" => Hash::Sha1,
            "SHA224" => Hash::Sha224,
            "SHA256" => Hash::Sha256,
            "SHA384" => Hash::Sha384,
            "SHA512" => Hash::Sha512,
            "RIPEMD128" | "RMD128" => Hash::Ripemd128,
            "RIPEMD160" | "RMD160" => Hash::Ripemd160,
            "RIPEMD256" | "RMD256" => Hash::Ripemd256,
            _ => return None,
        })
    }

    /// The digest of some data.
    pub fn digest(self, data: &[u8]) -> Vec<u8> {
        match self {
            Hash::Md2 => md2::Md2::digest(data).to_vec(),
            Hash::Md4 => md4::Md4::digest(data).to_vec(),
            Hash::Md5 => md5::Md5::digest(data).to_vec(),
            Hash::Sha1 => sha1::Sha1::digest(data).to_vec(),
            Hash::Sha224 => sha2::Sha224::digest(data).to_vec(),
            Hash::Sha256 => sha2::Sha256::digest(data).to_vec(),
            Hash::Sha384 => sha2::Sha384::digest(data).to_vec(),
            Hash::Sha512 => sha2::Sha512::digest(data).to_vec(),
            Hash::Ripemd128 => ripemd::Ripemd128::digest(data).to_vec(),
            Hash::Ripemd160 => ripemd::Ripemd160::digest(data).to_vec(),
            Hash::Ripemd256 => ripemd::Ripemd256::digest(data).to_vec(),
        }
    }

    /// The DER of a `DigestInfo` up to the digest: the sequence, the identifier of the digest
    /// with its `NULL` parameters, and the header of the octet string.
    fn info_prefix(self) -> &'static [u8] {
        match self {
            Hash::Md2 => &[
                0x30, 0x20, 0x30, 0x0c, 0x06, 0x08, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x02, 0x02,
                0x05, 0x00, 0x04, 0x10,
            ],
            Hash::Md4 => &[
                0x30, 0x20, 0x30, 0x0c, 0x06, 0x08, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x02, 0x04,
                0x05, 0x00, 0x04, 0x10,
            ],
            Hash::Md5 => &[
                0x30, 0x20, 0x30, 0x0c, 0x06, 0x08, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x02, 0x05,
                0x05, 0x00, 0x04, 0x10,
            ],
            Hash::Sha1 => &[
                0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x0e, 0x03, 0x02, 0x1a, 0x05, 0x00, 0x04,
                0x14,
            ],
            Hash::Sha224 => &[
                0x30, 0x2d, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x04, 0x05, 0x00, 0x04, 0x1c,
            ],
            Hash::Sha256 => &[
                0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x01, 0x05, 0x00, 0x04, 0x20,
            ],
            Hash::Sha384 => &[
                0x30, 0x41, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x02, 0x05, 0x00, 0x04, 0x30,
            ],
            Hash::Sha512 => &[
                0x30, 0x51, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x03, 0x05, 0x00, 0x04, 0x40,
            ],
            Hash::Ripemd128 => &[
                0x30, 0x1d, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x24, 0x03, 0x02, 0x02, 0x05, 0x00, 0x04,
                0x10,
            ],
            Hash::Ripemd160 => &[
                0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x24, 0x03, 0x02, 0x01, 0x05, 0x00, 0x04,
                0x14,
            ],
            Hash::Ripemd256 => &[
                0x30, 0x2d, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x24, 0x03, 0x02, 0x03, 0x05, 0x00, 0x04,
                0x20,
            ],
        }
    }
}

/// A digest of the signatures of PSS: the ones BouncyCastle has a name for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PssHash {
    /// SHA-1.
    Sha1,
    /// SHA-224.
    Sha224,
    /// SHA-256.
    Sha256,
    /// SHA-384.
    Sha384,
    /// SHA-512.
    Sha512,
}

impl PssHash {
    /// The digest an upper case name of a signature algorithm (`SHA256`) tells.
    fn lookup(name: &str) -> Option<Self> {
        Some(match name {
            "SHA1" => PssHash::Sha1,
            "SHA224" => PssHash::Sha224,
            "SHA256" => PssHash::Sha256,
            "SHA384" => PssHash::Sha384,
            "SHA512" => PssHash::Sha512,
            _ => return None,
        })
    }

    /// The digest, as the one of the other signatures.
    fn hash(self) -> Hash {
        match self {
            PssHash::Sha1 => Hash::Sha1,
            PssHash::Sha224 => Hash::Sha224,
            PssHash::Sha256 => Hash::Sha256,
            PssHash::Sha384 => Hash::Sha384,
            PssHash::Sha512 => Hash::Sha512,
        }
    }

    /// The digest of some data.
    fn digest(self, data: &[u8]) -> Vec<u8> {
        self.hash().digest(data)
    }

    /// The size of a digest, in bytes (the size of the salt too).
    fn size(self) -> usize {
        match self {
            PssHash::Sha1 => 20,
            PssHash::Sha224 => 28,
            PssHash::Sha256 => 32,
            PssHash::Sha384 => 48,
            PssHash::Sha512 => 64,
        }
    }
}

/// A scheme of signature, as the name of an algorithm tells it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    /// PKCS#1 v1.5 of a digest (`SHA256WITHRSA`).
    Pkcs1(Hash),
    /// PKCS#1 v1.5 of the data themselves, without any digest (`NONEWITHRSA`).
    Pkcs1Raw,
    /// PSS of a digest, whose salt is as long as the digest (`SHA256WITHRSAANDMGF1`).
    Pss(PssHash),
}

impl Scheme {
    /// The scheme of an algorithm name, with the names and the identifiers that BouncyCastle
    /// knows (without the ISO 9796-2 and the raw PSS it also has), whatever their case.
    #[must_use]
    pub fn lookup(name: &str) -> Option<Self> {
        let upper = name.to_ascii_uppercase();
        let name = upper.strip_prefix("OID.").unwrap_or(&upper);
        match name {
            "NONEWITHRSA" | "RSA" | "RAWRSA" => return Some(Scheme::Pkcs1Raw),
            // The default parameters of PSS: SHA-1, a salt of 20 bytes.
            "RSASSA-PSS" | "1.2.840.113549.1.1.10" => return Some(Scheme::Pss(PssHash::Sha1)),
            "1.2.840.113549.1.1.2" => return Some(Scheme::Pkcs1(Hash::Md2)),
            "1.2.840.113549.1.1.3" => return Some(Scheme::Pkcs1(Hash::Md4)),
            "1.2.840.113549.1.1.4" => return Some(Scheme::Pkcs1(Hash::Md5)),
            "1.2.840.113549.1.1.5" | "1.3.14.3.2.29" => return Some(Scheme::Pkcs1(Hash::Sha1)),
            "1.2.840.113549.1.1.11" => return Some(Scheme::Pkcs1(Hash::Sha256)),
            "1.2.840.113549.1.1.12" => return Some(Scheme::Pkcs1(Hash::Sha384)),
            "1.2.840.113549.1.1.13" => return Some(Scheme::Pkcs1(Hash::Sha512)),
            "1.2.840.113549.1.1.14" => return Some(Scheme::Pkcs1(Hash::Sha224)),
            "1.3.36.3.3.1.2" => return Some(Scheme::Pkcs1(Hash::Ripemd160)),
            "1.3.36.3.3.1.3" => return Some(Scheme::Pkcs1(Hash::Ripemd128)),
            "1.3.36.3.3.1.4" => return Some(Scheme::Pkcs1(Hash::Ripemd256)),
            _ => {}
        }
        if let Some(digest) = name
            .strip_suffix("WITHRSAENCRYPTION")
            .or_else(|| name.strip_suffix("WITHRSA"))
        {
            return Hash::lookup(digest).map(Scheme::Pkcs1);
        }
        let digest = name
            .strip_suffix("WITHRSAANDMGF1")
            .or_else(|| name.strip_suffix("WITHRSA/PSS"))?;
        PssHash::lookup(digest).map(Scheme::Pss)
    }
}

/// The modular exponentiation of a key, with the sizes of the blocks it makes.
pub struct Exponentiation {
    /// The modulus.
    modulus: BigUint,
    /// The parameters of the exponentiation modulo the modulus.
    params: BoxedMontyParams,
    /// The exponent.
    exponent: BoxedUint,
    /// The size of the modulus, in bytes: the size of a block of ciphertext or of signature.
    block: usize,
    /// The bytes of data a block holds once it is framed (at least 1).
    capacity: usize,
    /// The size of the modulus, in bits.
    bits: u64,
}

impl Exponentiation {
    /// The exponentiation of a key.
    ///
    /// # Errors
    ///
    /// A key that is not an RSA key: a modulus that is not positive, even or too small, an
    /// exponent that is not positive.
    pub fn new(key: &RsaKey) -> Result<Self, RsaError> {
        let modulus = key
            .modulus
            .to_biguint()
            .ok_or(RsaError::ModulusNotPositive)?;
        let bits = modulus.bits();
        let block = bits.div_ceil(8) as usize;
        let capacity = block
            .checked_sub(BLOCK_OVERHEAD)
            .filter(|capacity| *capacity > 0)
            .ok_or(RsaError::ModulusTooSmall { bits })?;
        let exponent = key
            .exponent
            .to_biguint()
            .filter(|exponent| *exponent != BigUint::ZERO)
            .ok_or(RsaError::ExponentNotPositive)?;
        let odd = Odd::new(BoxedUint::from_be_slice_vartime(&modulus.to_bytes_be()));
        let odd = Option::<Odd<BoxedUint>>::from(odd).ok_or(RsaError::EvenModulus)?;
        Ok(Self {
            modulus,
            params: BoxedMontyParams::new(odd),
            exponent: BoxedUint::from_be_slice_vartime(&exponent.to_bytes_be()),
            block,
            capacity,
            bits,
        })
    }

    /// The exponentiation of a value, given as the bytes of a number, that is below the modulus
    /// (a block of `block - 1` bytes is, so is the value of a signature of PSS): the bytes of the
    /// result, without leading zero.
    fn power_below(&self, value: &[u8]) -> Vec<u8> {
        let base = BoxedUint::from_be_slice_vartime(value).resize(self.params.bits_precision());
        BoxedMontyForm::new(base, &self.params)
            .pow(&self.exponent)
            .retrieve()
            .to_be_bytes_trimmed_vartime()
            .into_vec()
    }

    /// The exponentiation of an input of the engine (`RSACoreEngine.convertInput`): at most a
    /// block, and below the modulus.
    fn power(&self, input: &[u8]) -> Result<Vec<u8>, InputTooLarge> {
        if input.len() > self.block || BigUint::from_bytes_be(input) >= self.modulus {
            return Err(InputTooLarge);
        }
        Ok(self.power_below(input))
    }

    /// The block of type 1 of some data: `01 ff … ff 00 data`, in the `block - 1` bytes the
    /// engine takes. The data are at most `capacity` bytes, which the callers keep.
    fn encode_block(&self, data: &[u8]) -> Vec<u8> {
        let size = self.block - 1;
        let fill = size.saturating_sub(data.len() + 2);
        let mut block = Vec::with_capacity(size);
        block.push(1);
        block.resize(1 + fill, 0xff);
        block.push(0);
        block.extend_from_slice(data);
        block
    }

    /// The data of a block, the exponentiation of a block of ciphertext
    /// (`PKCS1Encoding.decodeBlock`): types 1 and 2 are accepted whoever the key is.
    fn decode_block<'b>(&self, block: &'b [u8]) -> Result<&'b [u8], BlockError> {
        match block.len().cmp(&(self.block - 1)) {
            Ordering::Less => return Err(BlockError::Truncated),
            Ordering::Greater => return Err(BlockError::IncorrectSize),
            Ordering::Equal => {}
        }
        // The block has `block - 1` bytes, at least 11.
        let kind = block[0];
        if kind != 1 && kind != 2 {
            return Err(BlockError::UnknownType);
        }
        let padding = &block[1..];
        let end = padding.iter().position(|byte| *byte == 0);
        let padded = &padding[..end.unwrap_or(padding.len())];
        if kind == 1 && padded.iter().any(|byte| *byte != 0xff) {
            return Err(BlockError::PaddingIncorrect);
        }
        match end {
            Some(end) if end + 2 >= HEADER_LENGTH => Ok(&padding[end + 1..]),
            _ => Err(BlockError::NoData),
        }
    }

    /// The ciphertext of some data: a block of type 1 for each `capacity` bytes, and the
    /// exponentiation of each, whichever the key is (`CryptoHelper.encryptRSA`).
    #[must_use]
    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        let mut ciphertext = Vec::with_capacity(data.len().div_ceil(self.capacity) * self.block);
        for chunk in data.chunks(self.capacity) {
            let power = self.power_below(&self.encode_block(chunk));
            ciphertext.extend(left_pad(power, self.block));
        }
        ciphertext
    }

    /// The ciphertext of a payload that fits one block, as a block of type 2 whose padding is
    /// random and never zero (`PKCS1Encoding` in encryption mode, with a public key).
    ///
    /// # Errors
    ///
    /// A payload larger than the capacity of a block, or no entropy for the padding.
    pub fn encrypt_random(&self, data: &[u8], random: Random<'_>) -> Result<Vec<u8>, RsaError> {
        if data.len() > self.capacity {
            return Err(RsaError::DataTooLarge);
        }
        let size = self.block - 1;
        let mut block = Vec::with_capacity(size);
        block.push(2);
        for _ in 0..size - data.len() - 2 {
            block.push(nonzero_byte(random)?);
        }
        block.push(0);
        block.extend_from_slice(data);
        Ok(left_pad(self.power_below(&block), self.block))
    }

    /// The data of a ciphertext (`CryptoHelper.decryptRSA`), a block at a time.
    ///
    /// # Errors
    ///
    /// A block that is not below the modulus, or whose exponentiation is not a block of data.
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, RsaError> {
        let mut data = Vec::new();
        for chunk in ciphertext.chunks(self.block) {
            let power = self.power(chunk)?;
            data.extend_from_slice(self.decode_block(&power)?);
        }
        Ok(data)
    }

    /// The signer or the verifier of a scheme.
    ///
    /// # Errors
    ///
    /// A key too small for the digest and the salt of PSS.
    pub fn signer(&self, scheme: Scheme) -> Result<Signer<'_>, RsaError> {
        if let Scheme::Pss(hash) = scheme {
            // `PSSSigner.init`: the bits of the block hold two digests (one is the salt) and
            // the 9 bits of framing.
            if self.bits - 1 < 16 * hash.size() as u64 + 9 {
                return Err(RsaError::PssKeyTooSmall);
            }
        }
        Ok(Signer { key: self, scheme })
    }

    /// The signer or the verifier of a PKCS#1 digest. Unlike `signer`, it cannot fail: only PSS
    /// needs a key large enough for its salt.
    pub fn pkcs1(&self, hash: Hash) -> Signer<'_> {
        Signer {
            key: self,
            scheme: Scheme::Pkcs1(hash),
        }
    }

    /// The size of a PSS block, in bytes.
    fn pss_block(&self) -> usize {
        (self.bits - 1).div_ceil(8) as usize
    }

    /// The mask of the first byte of a PSS block, which clears the bits that are not in it.
    fn pss_mask(&self) -> u8 {
        0xff >> (8 * self.pss_block() as u64 - (self.bits - 1))
    }
}

/// The bytes on the right of a block of `size` bytes (`size` is at least the length).
fn left_pad(bytes: Vec<u8>, size: usize) -> Vec<u8> {
    let mut padded = vec![0; size.saturating_sub(bytes.len())];
    padded.extend(bytes);
    padded
}

/// A byte of random data that is not zero: the zeros are drawn again.
fn nonzero_byte(random: Random<'_>) -> Result<u8, RsaError> {
    let mut byte = [0];
    loop {
        random(&mut byte)?;
        if byte[0] != 0 {
            return Ok(byte[0]);
        }
    }
}

/// A key that signs and verifies with a scheme.
pub struct Signer<'a> {
    /// The key.
    key: &'a Exponentiation,
    /// The scheme.
    scheme: Scheme,
}

impl Signer<'_> {
    /// The signature of some data.
    ///
    /// # Errors
    ///
    /// Data that are too large for a block of the key, or no entropy for the salt.
    pub fn sign(&self, data: &[u8], random: Random<'_>) -> Result<Vec<u8>, RsaError> {
        let key = self.key;
        let power = match self.scheme {
            Scheme::Pkcs1(hash) => {
                key.sign_block(&[hash.info_prefix(), &hash.digest(data)].concat())?
            }
            Scheme::Pkcs1Raw => key.sign_block(data)?,
            Scheme::Pss(hash) => key.power_below(&key.pss_encode(hash, data, random)?),
        };
        Ok(left_pad(power, key.block))
    }

    /// Whether a signature is the one of some data.
    ///
    /// # Errors
    ///
    /// The reason why the signature is not accepted: it cannot be read, or it is not the one of
    /// the data.
    pub fn verify(&self, data: &[u8], signature: &[u8]) -> Result<(), Rejection> {
        let key = self.key;
        let power = key.power(signature)?;
        match self.scheme {
            Scheme::Pkcs1(hash) => {
                let decoded = key.decode_block(&power)?;
                if digest_info_matches(decoded, hash, &hash.digest(data)) {
                    Ok(())
                } else {
                    Err(Rejection::SignatureMismatch)
                }
            }
            Scheme::Pkcs1Raw => {
                if key.decode_block(&power)? == data {
                    Ok(())
                } else {
                    Err(Rejection::SignatureMismatch)
                }
            }
            Scheme::Pss(hash) => key.pss_verify(hash, data, &power),
        }
    }
}

impl Exponentiation {
    /// The exponentiation of a block of type 1 of some data (the signature of PKCS#1).
    fn sign_block(&self, data: &[u8]) -> Result<Vec<u8>, RsaError> {
        if data.len() > self.capacity {
            return Err(RsaError::DataTooLarge);
        }
        Ok(self.power_below(&self.encode_block(data)))
    }

    /// The PSS block of some data, which a signature exponentiates: a salt as long as the
    /// digest, and a mask generated from the digest of the data and the salt.
    fn pss_encode(
        &self,
        hash: PssHash,
        data: &[u8],
        random: Random<'_>,
    ) -> Result<Vec<u8>, RsaError> {
        let size = hash.size();
        let mut salt = vec![0; size];
        random(&mut salt)?;
        let digest = pss_digest(hash, &hash.digest(data), &salt);
        // The block is `masked || digest || bc`, the data block `00 … 00 01 salt` once masked.
        let mut masked = vec![0; self.pss_block() - 2 * size - 2];
        masked.push(1);
        masked.extend_from_slice(&salt);
        let mask = mgf1(hash, &digest, masked.len());
        for (byte, mask) in masked.iter_mut().zip(mask) {
            *byte ^= mask;
        }
        masked[0] &= self.pss_mask();
        masked.extend_from_slice(&digest);
        masked.push(PSS_TRAILER);
        Ok(masked)
    }

    /// Whether the exponentiation of a signature is the PSS block of some data.
    fn pss_verify(&self, hash: PssHash, data: &[u8], power: &[u8]) -> Result<(), Rejection> {
        let size = hash.size();
        let length = self.pss_block();
        if power.len() > length {
            return Err(Rejection::PssBlockSize);
        }
        let block = left_pad(power.to_vec(), length);
        if block[length - 1] != PSS_TRAILER {
            return Err(Rejection::PssTrailer);
        }
        let (masked, rest) = block.split_at(length - size - 1);
        let digest = &rest[..size];
        let mut unmasked = masked.to_vec();
        for (byte, mask) in unmasked.iter_mut().zip(mgf1(hash, digest, masked.len())) {
            *byte ^= mask;
        }
        unmasked[0] &= self.pss_mask();
        let (padding, salt) = unmasked.split_at(length - 2 * size - 2);
        if padding.iter().any(|byte| *byte != 0) || salt[0] != 1 {
            return Err(Rejection::PssPadding);
        }
        if pss_digest(hash, &hash.digest(data), &salt[1..]) == digest {
            Ok(())
        } else {
            Err(Rejection::SignatureMismatch)
        }
    }
}

/// The digest of a PSS block: eight zeroes, the digest of the data and the salt.
fn pss_digest(hash: PssHash, data_digest: &[u8], salt: &[u8]) -> Vec<u8> {
    hash.digest(&[&[0; 8], data_digest, salt].concat())
}

/// The mask of `length` bytes that MGF1 makes from a seed.
fn mgf1(hash: PssHash, seed: &[u8], length: usize) -> Vec<u8> {
    let mut mask = Vec::with_capacity(length + hash.size());
    for counter in 0..length.div_ceil(hash.size()) as u32 {
        mask.extend(hash.digest(&[seed, &counter.to_be_bytes()].concat()));
    }
    mask.truncate(length);
    mask
}

/// Whether the data of a block of signature is the `DigestInfo` of a digest. BouncyCastle also
/// takes the one whose algorithm has no `NULL` parameters, but leaves the last two bytes of the
/// digest unchecked, which is compared in full here.
fn digest_info_matches(decoded: &[u8], hash: Hash, digest: &[u8]) -> bool {
    let prefix = hash.info_prefix();
    let expected = [prefix, digest].concat();
    if decoded.len() == expected.len() {
        return decoded == expected;
    }
    // Without the `NULL` (`05 00`) that precedes the octet string, and the two lengths that
    // hold it (the sequence of the identifier and the sequence of the `DigestInfo`).
    let null = prefix.len() - 4;
    let mut short = [&prefix[..null], &prefix[null + 2..], digest].concat();
    short[1] -= 2;
    short[3] -= 2;
    decoded == short
}

/// The keys of a generation: the private one and the public one.
pub struct KeyPair {
    /// The private key: the modulus and the private exponent.
    pub private: RsaKey,
    /// The public key: the modulus and the public exponent.
    pub public: RsaKey,
}

/// A random prime of `bits` bits, the first bit set.
fn random_prime(bits: u32, random: Random<'_>) -> Result<BigUint, RsaError> {
    let length = bits.div_ceil(8) as usize;
    // The bits that are not in the number, in the first byte.
    let excess = length as u32 * 8 - bits;
    let mut bytes = vec![0; length];
    loop {
        random(&mut bytes)?;
        bytes[0] &= 0xff >> excess;
        bytes[0] |= 0x80 >> excess;
        bytes[length - 1] |= 1;
        if is_prime(Flavor::Any, &BoxedUint::from_be_slice_vartime(&bytes)) {
            return Ok(BigUint::from_bytes_be(&bytes));
        }
    }
}

/// A prime of `bits` bits for a key (`RSAKeyPairGenerator.chooseRandomPrime`): its square is
/// not below the bound, it is above the primes that the trial division of BouncyCastle takes for
/// divisors of it, and `exponent` is prime with the prime less one.
fn choose_prime(
    bits: u32,
    exponent: &BigUint,
    squared_bound: &BigUint,
    random: Random<'_>,
) -> Result<BigUint, RsaError> {
    // No number of `bits` bits is as large as the bound asks: BouncyCastle draws primes for
    // nothing before it gives up.
    let largest = (BigUint::one() << bits) - 1u32;
    if &largest * &largest < *squared_bound {
        return Err(RsaError::PrimeNotFound);
    }
    for _ in 0..5 * bits {
        let prime = random_prime(bits, random)?;
        if &prime * &prime < *squared_bound
            || prime <= BigUint::from(TRIAL_DIVISION_LIMIT)
            || !exponent.gcd(&(&prime - 1u32)).is_one()
        {
            continue;
        }
        return Ok(prime);
    }
    Err(RsaError::PrimeNotFound)
}

/// The inverse of a number modulo another, when they are coprime.
fn inverse(value: &BigUint, modulus: &BigUint) -> Option<BigUint> {
    let modulus = BigInt::from(modulus.clone());
    let extended = BigInt::from(value.clone()).extended_gcd(&modulus);
    extended
        .gcd
        .is_one()
        .then(|| extended.x.mod_floor(&modulus).magnitude().clone())
}

/// The number of non-zero digits of the non-adjacent form of a number, which is the number of
/// bits of `3n xor n` (the keys of a low weight are weak against a factorisation).
fn naf_weight(number: &BigUint) -> u64 {
    let triple = number * 3u32;
    (&triple ^ number)
        .to_u64_digits()
        .iter()
        .map(|digit| u64::from(digit.count_ones()))
        .sum()
}

/// A new key pair of `strength` bits, as `RSAKeyPairGenerator` of BouncyCastle makes them: two
/// primes of half the bits, far apart, that the public exponent is prime with, whose product has
/// a NAF of enough weight, and a private exponent above `2^(strength / 2)` (the inverse of the
/// public exponent modulo the least common multiple of the primes less one).
///
/// # Errors
///
/// A strength below 12 or an exponent that is even or below zero, no prime to be found (any odd
/// strength), or no entropy.
pub fn generate(exponent: &BigInt, strength: i32, random: Random<'_>) -> Result<KeyPair, RsaError> {
    if strength < MIN_STRENGTH {
        return Err(RsaError::StrengthTooSmall);
    }
    if exponent.is_even() {
        return Err(RsaError::ExponentEven);
    }
    let public_exponent = exponent.to_biguint().ok_or(RsaError::ModulusNotPositive)?;
    let strength = strength.unsigned_abs();
    let p_bits = strength.div_ceil(2);
    let q_bits = strength - p_bits;
    let min_diff_bits = (strength / 2).saturating_sub(100).max(strength / 3);
    let squared_bound = BigUint::one() << (strength - 1);
    let min_diff = BigUint::one() << min_diff_bits;
    let d_lower_bound = BigUint::one() << (strength / 2);
    let min_weight = u64::from(strength >> 2);
    for _ in 0..KEY_ATTEMPTS {
        let p = choose_prime(p_bits, &public_exponent, &squared_bound, random)?;
        let q = choose_prime(q_bits, &public_exponent, &squared_bound, random)?;
        // The primes must be far apart, and the product of enough weight. The product of two
        // primes that are not below the bound has exactly `strength` bits.
        let diff = if p > q { &p - &q } else { &q - &p };
        let n = &p * &q;
        if diff <= min_diff || naf_weight(&n) < min_weight {
            continue;
        }
        let lcm = (&p - 1u32).lcm(&(&q - 1u32));
        let Some(d) = inverse(&public_exponent, &lcm).filter(|d| *d > d_lower_bound) else {
            continue;
        };
        return Ok(KeyPair {
            private: RsaKey {
                modulus: BigInt::from(n.clone()),
                exponent: BigInt::from(d),
            },
            public: RsaKey {
                modulus: BigInt::from(n),
                exponent: exponent.clone(),
            },
        });
    }
    Err(RsaError::KeyNotFound)
}
