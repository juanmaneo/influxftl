//! Cryptographic functions of WarpLib: the message digests of the `DIGEST` family (`MD5`,
//! `SHA256`, `KECCAK.256`, `GOST`…), `SHA256HMAC` / `SHA1HMAC`, the SipHash `HASH`, the
//! Base58 conversions (`->B58`, `->B58C`, `B58->`, `B58C->`), the AES key wrap (`AESWRAP`,
//! `AESUNWRAP`) and the `ARGON2` password hash.
//!
//! The digests reproduce the BouncyCastle implementations Warp 10 uses (`TIGER` is Tiger/192
//! with the `0x01` padding of `TigerDigest`, `GOST` is GOST R 34.11-94 with the CryptoPro
//! S-box of `GOST3411Digest`, `WHIRLPOOL` the final 2003 version). The Argon2 engine itself
//! lives in [`crate::argon2`].

use aes::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use aes::{Aes128, Aes192, Aes256};
use digest::Digest;
use digest::common::BlockSizeUser;

use super::Entry;
use super::args::{fail, fail_with_cause, pop_map};
use super::collections::siphash24;
use super::math::entropy_error;
use crate::argon2;
use crate::error::{Error, Result};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("KECCAK.128", Entry::Stack(keccak_128)),
    ("KECCAK.224", Entry::Stack(keccak_224)),
    ("KECCAK.256", Entry::Stack(keccak_256)),
    ("KECCAK.288", Entry::Stack(keccak_288)),
    ("KECCAK.384", Entry::Stack(keccak_384)),
    ("KECCAK.512", Entry::Stack(keccak_512)),
    ("MD2", Entry::Stack(md2)),
    ("MD4", Entry::Stack(md4)),
    ("MD5", Entry::Stack(md5)),
    ("SHA1", Entry::Stack(sha1)),
    ("SHA224", Entry::Stack(sha224)),
    ("SHA256", Entry::Stack(sha256)),
    ("SHA384", Entry::Stack(sha384)),
    ("SHA512", Entry::Stack(sha512)),
    ("SHA3.224", Entry::Stack(sha3_224)),
    ("SHA3.256", Entry::Stack(sha3_256)),
    ("SHA3.384", Entry::Stack(sha3_384)),
    ("SHA3.512", Entry::Stack(sha3_512)),
    ("RIPEMD128", Entry::Stack(ripemd128)),
    ("RIPEMD160", Entry::Stack(ripemd160)),
    ("RIPEMD256", Entry::Stack(ripemd256)),
    ("RIPEMD320", Entry::Stack(ripemd320)),
    ("TIGER", Entry::Stack(tiger)),
    ("WHIRLPOOL", Entry::Stack(whirlpool)),
    ("GOST", Entry::Stack(gost)),
    ("SHA256HMAC", Entry::Stack(sha256hmac)),
    ("SHA1HMAC", Entry::Stack(sha1hmac)),
    ("HASH", Entry::Stack(hash)),
    ("->B58", Entry::Stack(tob58)),
    ("->B58C", Entry::Stack(tob58c)),
    ("B58->", Entry::Stack(b58to)),
    ("B58C->", Entry::Stack(b58cto)),
    ("AESWRAP", Entry::Stack(aeswrap)),
    ("AESUNWRAP", Entry::Stack(aesunwrap)),
    ("ARGON2", Entry::Stack(argon2_fn)),
];

// -----------------------------------------------------------------------------------------
// Keccak / SHA-3 sponge
// -----------------------------------------------------------------------------------------

/// A Keccak-f[1600] sponge with a rate of `rate` bytes (a multiple of 8), the padding
/// domain byte `suffix` (`0x01` for Keccak, `0x06` for SHA-3) and `out_len` (at most `rate`)
/// bytes of output: `KeccakDigest` / `SHA3Digest`.
pub(crate) fn keccak_sponge(rate: usize, suffix: u8, out_len: usize, data: &[u8]) -> Vec<u8> {
    let mut state = [0u64; 25];
    let absorb = |state: &mut [u64; 25], block: &[u8]| {
        for (lane, chunk) in state.iter_mut().zip(block.chunks(8)) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            *lane ^= u64::from_le_bytes(word);
        }
        keccak::Keccak::new().with_f1600(|f1600| f1600(state));
    };
    let (blocks, remainder) = data.split_at(data.len() - data.len() % rate);
    for block in blocks.chunks(rate) {
        absorb(&mut state, block);
    }
    let mut last = vec![0u8; rate];
    last[..remainder.len()].copy_from_slice(remainder);
    last[remainder.len()] ^= suffix;
    last[rate - 1] ^= 0x80;
    absorb(&mut state, &last);
    state
        .iter()
        .flat_map(|lane| lane.to_le_bytes())
        .take(out_len)
        .collect()
}

/// `KeccakDigest(bits)`: rate `200 - bits / 4` bytes, `bits / 8` bytes of output.
fn keccak(bits: usize, data: &[u8]) -> Vec<u8> {
    keccak_sponge(200 - bits / 4, 0x01, bits / 8, data)
}

/// `SHA3Digest(bits)`: like Keccak with the SHA-3 domain separation.
fn sha3(bits: usize, data: &[u8]) -> Vec<u8> {
    keccak_sponge(200 - bits / 4, 0x06, bits / 8, data)
}

// -----------------------------------------------------------------------------------------
// DIGEST family
// -----------------------------------------------------------------------------------------

/// The work of `DIGEST`: pop a byte array, push its digest computed by `algorithm`.
fn digest_fn(stack: &mut Stack, name: &str, algorithm: fn(&[u8]) -> Vec<u8>) -> Result<()> {
    let Value::Bytes(bytes) = stack.pop()? else {
        return Err(fail(name, "operates on a byte array."));
    };
    stack.push(Value::bytes(algorithm(&bytes)));
    Ok(())
}

/// Define `fn $rust(stack)` as the `DIGEST` function `$name` computing `$algorithm`.
macro_rules! digest_functions {
    ($($rust:ident => $name:literal, $algorithm:expr;)*) => {
        $(
            fn $rust(stack: &mut Stack) -> Result<()> {
                digest_fn(stack, $name, $algorithm)
            }
        )*
    };
}

digest_functions! {
    keccak_128 => "KECCAK.128", |d| keccak(128, d);
    keccak_224 => "KECCAK.224", |d| keccak(224, d);
    keccak_256 => "KECCAK.256", |d| keccak(256, d);
    keccak_288 => "KECCAK.288", |d| keccak(288, d);
    keccak_384 => "KECCAK.384", |d| keccak(384, d);
    keccak_512 => "KECCAK.512", |d| keccak(512, d);
    md2 => "MD2", |d| md2::Md2::digest(d).to_vec();
    md4 => "MD4", |d| md4::Md4::digest(d).to_vec();
    md5 => "MD5", |d| md5::Md5::digest(d).to_vec();
    sha1 => "SHA1", |d| sha1::Sha1::digest(d).to_vec();
    sha224 => "SHA224", |d| sha2::Sha224::digest(d).to_vec();
    sha256 => "SHA256", |d| sha2::Sha256::digest(d).to_vec();
    sha384 => "SHA384", |d| sha2::Sha384::digest(d).to_vec();
    sha512 => "SHA512", |d| sha2::Sha512::digest(d).to_vec();
    sha3_224 => "SHA3.224", |d| sha3(224, d);
    sha3_256 => "SHA3.256", |d| sha3(256, d);
    sha3_384 => "SHA3.384", |d| sha3(384, d);
    sha3_512 => "SHA3.512", |d| sha3(512, d);
    ripemd128 => "RIPEMD128", |d| ripemd::Ripemd128::digest(d).to_vec();
    ripemd160 => "RIPEMD160", |d| ripemd::Ripemd160::digest(d).to_vec();
    ripemd256 => "RIPEMD256", |d| ripemd::Ripemd256::digest(d).to_vec();
    ripemd320 => "RIPEMD320", |d| ripemd::Ripemd320::digest(d).to_vec();
    tiger => "TIGER", |d| tiger::Tiger::digest(d).to_vec();
    whirlpool => "WHIRLPOOL", |d| whirlpool::Whirlpool::digest(d).to_vec();
    gost => "GOST", |d| gost94::Gost94CryptoPro::digest(d).to_vec();
}

// -----------------------------------------------------------------------------------------
// HMAC
// -----------------------------------------------------------------------------------------

/// RFC 2104 HMAC with the digest `D` (`org.bouncycastle.crypto.macs.HMac`): keys longer than
/// the block size are hashed first.
pub(crate) fn hmac<D: Digest + BlockSizeUser>(key: &[u8], data: &[u8]) -> Vec<u8> {
    let block_size = D::block_size();
    let mut padded_key = vec![0u8; block_size];
    if key.len() > block_size {
        let hashed = D::digest(key);
        padded_key[..hashed.len()].copy_from_slice(&hashed);
    } else {
        padded_key[..key.len()].copy_from_slice(key);
    }
    let mut inner = D::new();
    inner.update(padded_key.iter().map(|b| b ^ 0x36).collect::<Vec<u8>>());
    inner.update(data);
    let mut outer = D::new();
    outer.update(padded_key.iter().map(|b| b ^ 0x5c).collect::<Vec<u8>>());
    outer.update(inner.finalize());
    outer.finalize().to_vec()
}

/// The work of `HMAC`: pop the key then the data (byte arrays), push the MAC.
fn hmac_fn(stack: &mut Stack, name: &str, mac: fn(&[u8], &[u8]) -> Vec<u8>) -> Result<()> {
    let Value::Bytes(key) = stack.pop()? else {
        return Err(fail(
            name,
            "expects a byte array (key) on top of the stack.",
        ));
    };
    let Value::Bytes(data) = stack.pop()? else {
        return Err(fail(name, "operates on a byte array."));
    };
    stack.push(Value::bytes(mac(&key, &data)));
    Ok(())
}

/// `SHA256HMAC`: HMAC-SHA256 of a byte array with a key.
fn sha256hmac(stack: &mut Stack) -> Result<()> {
    hmac_fn(stack, "SHA256HMAC", hmac::<sha2::Sha256>)
}

/// `SHA1HMAC`: HMAC-SHA1 of a byte array with a key.
fn sha1hmac(stack: &mut Stack) -> Result<()> {
    hmac_fn(stack, "SHA1HMAC", hmac::<sha1::Sha1>)
}

// -----------------------------------------------------------------------------------------
// HASH (SipHash-2-4)
// -----------------------------------------------------------------------------------------

/// The key `HASH` uses when the script gives none.
const HASH_KEY: (u64, u64) = (0xF214_F963_5AF5_4BBE, 0x8DD1_E8D8_56D2_6E46);

/// `HASH`: SipHash-2-4 of a string (UTF-8) or byte array, with an optional pair of `LONG`
/// key words below the data.
fn hash(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "HASH";
    let mut top = stack.pop()?;
    let (mut k0, mut k1) = HASH_KEY;
    if let Value::Long(l) = top {
        k1 = l as u64;
        let Value::Long(l) = stack.pop()? else {
            return Err(fail(NAME, "expects two LONGs as hash key."));
        };
        k0 = l as u64;
        top = stack.pop()?;
    }
    let hashed = match &top {
        Value::String(s) => siphash24(k0, k1, s.as_bytes()),
        Value::Bytes(bytes) => siphash24(k0, k1, bytes),
        _ => return Err(fail(NAME, "operates on a byte array or STRING.")),
    };
    stack.push(Value::Long(hashed as i64));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// Base58
// -----------------------------------------------------------------------------------------

/// The Base58 alphabet of `TOB58`.
const B58_ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// `TOB58.encode`: leading zero bytes become `1`s, the rest is the big-endian number in
/// base 58. Empty input fails like Java (`data[0]` is out of bounds).
pub(crate) fn b58_encode(data: &[u8]) -> Result<String> {
    if data.is_empty() {
        return Err(Error::new("Index 0 out of bounds for length 0"));
    }
    let zeros = data.iter().take_while(|&&b| b == 0).count();
    // Repeated division of the big-endian number by 58 (`BigInteger.divide`).
    let mut number: Vec<u8> = data[zeros..].to_vec();
    let mut digits = Vec::new();
    while !number.is_empty() {
        let mut remainder = 0u32;
        for byte in &mut number {
            let value = (remainder << 8) | u32::from(*byte);
            *byte = (value / 58) as u8;
            remainder = value % 58;
        }
        digits.push(B58_ALPHABET[remainder as usize]);
        while number.first() == Some(&0) {
            number.remove(0);
        }
    }
    let mut encoded = vec![b'1'; zeros];
    encoded.extend(digits.iter().rev());
    Ok(String::from_utf8_lossy(&encoded).into_owned())
}

/// The value of a Base58 digit (`B58TO.TEBAHPLA`): `Ok(None)` for an invalid character,
/// `Err` for the characters Java looks up past the end of its 123-entry table.
fn b58_digit(unit: u16) -> Result<Option<u8>> {
    if unit > 127 {
        return Ok(None);
    }
    if unit >= 123 {
        return Err(Error::new(format!(
            "Index {unit} out of bounds for length 123"
        )));
    }
    Ok(B58_ALPHABET
        .iter()
        .position(|&c| u16::from(c) == unit)
        .map(|p| p as u8))
}

/// `B58TO.decode`: leading `1`s become zero bytes, the rest is read as a base 58 number.
pub(crate) fn b58_decode(encoded: &str) -> Result<Vec<u8>> {
    let units: Vec<u16> = encoded.encode_utf16().collect();
    let zeros = units.iter().take_while(|&&u| u == u16::from(b'1')).count();
    // Big-endian base 256 accumulator (`BigInteger.multiply(58).add(digit)`).
    let mut number: Vec<u8> = Vec::new();
    for (i, &unit) in units.iter().enumerate().skip(zeros) {
        let Some(digit) = b58_digit(unit)? else {
            return Err(Error::new(format!(
                "Invalid input '{}' at position {i}.",
                String::from_utf16_lossy(&[unit])
            )));
        };
        let mut carry = u32::from(digit);
        for byte in number.iter_mut().rev() {
            let value = u32::from(*byte) * 58 + carry;
            *byte = (value & 0xFF) as u8;
            carry = value >> 8;
        }
        while carry > 0 {
            number.insert(0, (carry & 0xFF) as u8);
            carry >>= 8;
        }
    }
    let mut decoded = vec![0u8; zeros];
    decoded.extend(number);
    Ok(decoded)
}

/// Double SHA-256 (`TOB58` / `B58TO` checksum).
fn double_sha256(data: &[u8]) -> Vec<u8> {
    sha2::Sha256::digest(sha2::Sha256::digest(data)).to_vec()
}

/// `->B58` / `->B58C`: Base58 of a string (UTF-8) or byte array; the checked variant takes a
/// byte array prefix on top and appends 4 bytes of double SHA-256 checksum.
fn tob58_impl(stack: &mut Stack, name: &str, check: bool) -> Result<()> {
    let mut top = stack.pop()?;
    let mut prefix: Vec<u8> = Vec::new();
    if check {
        let Value::Bytes(bytes) = top else {
            return Err(fail(name, "expects a byte array prefix."));
        };
        prefix = bytes.to_vec();
        top = stack.pop()?;
    }
    let mut payload = match &top {
        Value::Bytes(bytes) => bytes.to_vec(),
        Value::String(s) => s.as_bytes().to_vec(),
        _ => return Err(fail(name, "operates on STRING or a byte array.")),
    };
    if check {
        let mut data = prefix;
        data.extend_from_slice(&payload);
        let hash = double_sha256(&data);
        data.extend_from_slice(&hash[..4]);
        payload = data;
    }
    stack.push(b58_encode(&payload)?);
    Ok(())
}

/// `->B58`: Base58 encoding.
fn tob58(stack: &mut Stack) -> Result<()> {
    tob58_impl(stack, "->B58", false)
}

/// `->B58C`: Base58Check encoding with a prefix.
fn tob58c(stack: &mut Stack) -> Result<()> {
    tob58_impl(stack, "->B58C", true)
}

/// `B58->` / `B58C->`: decode a Base58 string; the checked variant takes a byte array prefix
/// on top, verifies it and the checksum, and strips both.
fn b58to_impl(stack: &mut Stack, name: &str, check: bool) -> Result<()> {
    let mut top = stack.pop()?;
    let mut prefix: Vec<u8> = Vec::new();
    if check {
        let Value::Bytes(bytes) = top else {
            return Err(fail(name, "expects a byte array prefix."));
        };
        prefix = bytes.to_vec();
        top = stack.pop()?;
    }
    let Value::String(encoded) = top else {
        return Err(fail(name, "operates on a STRING."));
    };
    let mut decoded = b58_decode(&encoded).map_err(|err| {
        if err.message().starts_with("Invalid input") {
            fail_with_cause(name, "encountered an error while decoding Base58.", err)
        } else {
            err
        }
    })?;
    if check {
        if decoded.len() < prefix.len() + 4 {
            return Err(fail(name, "Base58 STRING too short."));
        }
        if !decoded.starts_with(&prefix) {
            return Err(fail(name, "invalid prefix."));
        }
        let (body, checksum) = decoded.split_at(decoded.len() - 4);
        if double_sha256(body)[..4] != *checksum {
            return Err(fail(name, "invalid checksum."));
        }
        decoded = body[prefix.len()..].to_vec();
    }
    stack.push(Value::bytes(decoded));
    Ok(())
}

/// `B58->`: Base58 decoding.
fn b58to(stack: &mut Stack) -> Result<()> {
    b58to_impl(stack, "B58->", false)
}

/// `B58C->`: Base58Check decoding with a prefix.
fn b58cto(stack: &mut Stack) -> Result<()> {
    b58to_impl(stack, "B58C->", true)
}

// -----------------------------------------------------------------------------------------
// AES key wrap (RFC 3394, `org.bouncycastle.crypto.engines.AESWrapEngine`)
// -----------------------------------------------------------------------------------------

/// The initial value of RFC 3394.
const KEK_IV: [u8; 8] = [0xA6; 8];

/// An AES key encryption key of 128, 192 or 256 bits.
pub(crate) enum Kek {
    Aes128(Aes128),
    Aes192(Aes192),
    Aes256(Aes256),
}

impl Kek {
    /// The key for `key`, `None` when its length is not 16, 24 or 32 bytes.
    pub(crate) fn new(key: &[u8]) -> Option<Kek> {
        match key.len() {
            16 => key
                .first_chunk::<16>()
                .map(|k| Kek::Aes128(Aes128::new(k.into()))),
            24 => key
                .first_chunk::<24>()
                .map(|k| Kek::Aes192(Aes192::new(k.into()))),
            32 => key
                .first_chunk::<32>()
                .map(|k| Kek::Aes256(Aes256::new(k.into()))),
            _ => None,
        }
    }

    /// Encrypt one 16-byte block in place.
    fn encrypt(&self, block: &mut [u8; 16]) {
        match self {
            Kek::Aes128(aes) => aes.encrypt_block(block.into()),
            Kek::Aes192(aes) => aes.encrypt_block(block.into()),
            Kek::Aes256(aes) => aes.encrypt_block(block.into()),
        }
    }

    /// Decrypt one 16-byte block in place.
    fn decrypt(&self, block: &mut [u8; 16]) {
        match self {
            Kek::Aes128(aes) => aes.decrypt_block(block.into()),
            Kek::Aes192(aes) => aes.decrypt_block(block.into()),
            Kek::Aes256(aes) => aes.decrypt_block(block.into()),
        }
    }
}

/// XOR the big-endian counter `t` into the first 8 bytes of `buf` (RFC 3394 step 2).
fn xor_counter(buf: &mut [u8; 16], t: u64) {
    for (byte, counter) in buf[..8].iter_mut().zip(t.to_be_bytes()) {
        *byte ^= counter;
    }
}

/// `RFC3394WrapEngine.wrap` of `data` (a multiple of 8 bytes, at least 8). Like
/// BouncyCastle, a single 8-byte block is wrapped with one AES encryption of `IV || P`
/// (RFC 5649 §4.1) instead of the six rounds.
pub(crate) fn rfc3394_wrap(kek: &Kek, data: &[u8]) -> Vec<u8> {
    let n = data.len() / 8;
    if n == 1 {
        let mut single = [0u8; 16];
        single[..8].copy_from_slice(&KEK_IV);
        single[8..].copy_from_slice(&data[..8]);
        kek.encrypt(&mut single);
        return single.to_vec();
    }
    let mut block = Vec::with_capacity(data.len() + 8);
    block.extend_from_slice(&KEK_IV);
    block.extend_from_slice(data);
    for j in 0..6u64 {
        for i in 1..=n {
            let mut buf = [0u8; 16];
            buf[..8].copy_from_slice(&block[..8]);
            buf[8..].copy_from_slice(&block[8 * i..8 * i + 8]);
            kek.encrypt(&mut buf);
            xor_counter(&mut buf, n as u64 * j + i as u64);
            block[..8].copy_from_slice(&buf[..8]);
            block[8 * i..8 * i + 8].copy_from_slice(&buf[8..]);
        }
    }
    block
}

/// `RFC3394WrapEngine.unwrap`: `None` when the data is not a multiple of 8 bytes, shorter than
/// 16 bytes or fails the integrity check (`InvalidCipherTextException`).
fn rfc3394_unwrap(kek: &Kek, data: &[u8]) -> Option<Vec<u8>> {
    if !data.len().is_multiple_of(8) || data.len() < 16 {
        return None;
    }
    let n = data.len() / 8 - 1;
    let mut a = [0u8; 8];
    a.copy_from_slice(&data[..8]);
    let mut block = data[8..].to_vec();
    if n == 1 {
        let mut single = [0u8; 16];
        single.copy_from_slice(data);
        kek.decrypt(&mut single);
        a.copy_from_slice(&single[..8]);
        block.copy_from_slice(&single[8..]);
        return (a == KEK_IV).then_some(block);
    }
    for j in (0..6u64).rev() {
        for i in (1..=n).rev() {
            let mut buf = [0u8; 16];
            buf[..8].copy_from_slice(&a);
            buf[8..].copy_from_slice(&block[8 * (i - 1)..8 * i]);
            xor_counter(&mut buf, n as u64 * j + i as u64);
            kek.decrypt(&mut buf);
            a.copy_from_slice(&buf[..8]);
            block[8 * (i - 1)..8 * i].copy_from_slice(&buf[8..]);
        }
    }
    (a == KEK_IV).then_some(block)
}

/// `CryptoUtils.wrap`: PKCS#7 padding to a multiple of 8 bytes, then the RFC 3394 wrap.
pub(crate) fn aes_wrap(kek: &Kek, data: &[u8]) -> Vec<u8> {
    let pad = 8 - data.len() % 8;
    let mut padded = data.to_vec();
    padded.extend(std::iter::repeat_n(pad as u8, pad));
    rfc3394_wrap(kek, &padded)
}

/// `CryptoUtils.unwrap`: the RFC 3394 unwrap followed by the PKCS#7 padding check and
/// removal (`PKCS7Padding.padCount`); `None` for undecipherable data.
pub(crate) fn aes_unwrap(kek: &Kek, data: &[u8]) -> Option<Vec<u8>> {
    let decrypted = rfc3394_unwrap(kek, data)?;
    let count = usize::from(decrypted.last().copied().unwrap_or_default());
    if count == 0 || count > decrypted.len() {
        return None;
    }
    let (body, padding) = decrypted.split_at(decrypted.len() - count);
    padding
        .iter()
        .all(|&b| usize::from(b) == count)
        .then(|| body.to_vec())
}

/// Pop the AES key of `AESWRAP` / `AESUNWRAP`.
fn pop_kek(stack: &mut Stack, name: &str) -> Result<Kek> {
    const MESSAGE: &str =
        "expects a byte array containing a 128, 192 or 256 bits AES key on the stack.";
    let Value::Bytes(key) = stack.pop()? else {
        return Err(fail(name, MESSAGE));
    };
    Kek::new(&key).ok_or_else(|| fail(name, MESSAGE))
}

/// Length of the random nonce `AESWRAP` prepends to the data.
pub(crate) const NONCE_LEN: usize = 8;

/// `AESWRAP` with the given nonce: wrap a byte array or string (UTF-8) prefixed by the
/// nonce with an AES key.
pub(crate) fn aeswrap_impl(
    stack: &mut Stack,
    nonce: core::result::Result<[u8; NONCE_LEN], getrandom::Error>,
) -> Result<()> {
    const NAME: &str = "AESWRAP";
    let kek = pop_kek(stack, NAME)?;
    let mut data = match stack.pop()? {
        Value::Bytes(bytes) => bytes.to_vec(),
        Value::String(s) => s.as_bytes().to_vec(),
        _ => return Err(fail(NAME, "operates on a byte array or a String.")),
    };
    let nonce = nonce.map_err(entropy_error)?;
    data.splice(0..0, nonce);
    stack.push(Value::bytes(aes_wrap(&kek, &data)));
    Ok(())
}

/// `AESWRAP`: wrap a byte array or string, prefixed by a random nonce, with an AES key.
fn aeswrap(stack: &mut Stack) -> Result<()> {
    let mut nonce = [0u8; NONCE_LEN];
    let entropy = getrandom::fill(&mut nonce).map(|()| nonce);
    aeswrap_impl(stack, entropy)
}

/// `AESUNWRAP`: unwrap a byte array with an AES key and drop the nonce.
fn aesunwrap(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "AESUNWRAP";
    let kek = pop_kek(stack, NAME)?;
    let Value::Bytes(data) = stack.pop()? else {
        return Err(fail(NAME, "operates on a byte array."));
    };
    let Some(unwrapped) = aes_unwrap(&kek, &data) else {
        return Err(fail(NAME, "encountered undecipherable data."));
    };
    // `Arrays.copyOfRange(unwrapped, 8, unwrapped.length)` fails when there is no nonce.
    if unwrapped.len() < NONCE_LEN {
        return Err(Error::new(format!("{NONCE_LEN} > {}", unwrapped.len())));
    }
    stack.push(Value::bytes(&unwrapped[NONCE_LEN..]));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// ARGON2
// -----------------------------------------------------------------------------------------

/// Default limits of `ARGON2`; going above them requires the matching capability.
const ARGON2_MAXITER_DEFAULT: i32 = 3;
const ARGON2_MAXMEM_DEFAULT: i32 = 32;
const ARGON2_MAXPAR_DEFAULT: i32 = 1;
const ARGON2_MAXSIZE_DEFAULT: i32 = 32;

/// Check an `ARGON2` parameter above its default limit against the capability `cap`
/// (`Long.parseLong(Capabilities.get(stack, cap))`).
fn argon2_check_capability(
    stack: &Stack,
    name: &str,
    value: i32,
    default: i32,
    cap: &str,
    excess: &str,
) -> Result<()> {
    if value <= default {
        return Ok(());
    }
    let Some(limit) = stack.capability(cap) else {
        return Err(fail(name, &format!("missing capability '{cap}'.")));
    };
    let limit: i64 = limit
        .parse()
        .map_err(|_| Error::new(format!("For input string: \"{limit}\"")))?;
    if i64::from(value) > limit {
        return Err(fail(name, excess));
    }
    Ok(())
}

/// `ARGON2`: derive a key from the `password` of a parameter map with Argon2 (`type` `i`,
/// `d` or `id`, `iterations`, `memory` in KiB, `parallelism`, `salt`, `secret`,
/// `additional`, `size`). Parameters above the defaults (3 iterations, 32 KiB, 1 lane,
/// 32 bytes) need the `argon2.maxiter`, `argon2.maxmem`, `argon2.maxpar` and
/// `argon2.maxsize` capabilities.
fn argon2_fn(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ARGON2";
    let map = pop_map(stack, NAME, "expects a parameter map.")?;
    let map = map.borrow();
    let get = |key: &str| map.get(&Value::from(key)).cloned();
    let mut params = argon2::Params::default();
    let kind = get("type").map_or_else(|| "null".to_owned(), |v| v.to_string());
    if kind.eq_ignore_ascii_case("i") {
        params.kind = argon2::ARGON2_I;
    } else if kind.eq_ignore_ascii_case("d") {
        params.kind = argon2::ARGON2_D;
    }
    if let Some(Value::Long(iterations)) = get("iterations") {
        let iterations = iterations as i32;
        argon2_check_capability(
            stack,
            NAME,
            iterations,
            ARGON2_MAXITER_DEFAULT,
            "argon2.maxiter",
            "number of iterations exceeds capability value.",
        )?;
        params.iterations = iterations;
    }
    if let Some(Value::Long(memory)) = get("memory") {
        let memory = memory as i32;
        argon2_check_capability(
            stack,
            NAME,
            memory,
            ARGON2_MAXMEM_DEFAULT,
            "argon2.maxmem",
            "memory exceeds capability value.",
        )?;
        params.memory = memory;
    }
    if let Some(Value::Long(lanes)) = get("parallelism") {
        let lanes = lanes as i32;
        argon2_check_capability(
            stack,
            NAME,
            lanes,
            ARGON2_MAXPAR_DEFAULT,
            "argon2.maxpar",
            "lane count exceeds capability value.",
        )?;
        params.lanes = lanes;
    }
    if let Some(Value::Bytes(salt)) = get("salt") {
        params.salt = Some(salt.to_vec());
    }
    if let Some(Value::Bytes(secret)) = get("secret") {
        params.secret = Some(secret.to_vec());
    }
    if let Some(Value::Bytes(additional)) = get("additional") {
        params.additional = Some(additional.to_vec());
    }
    let mut size = ARGON2_MAXSIZE_DEFAULT;
    if let Some(Value::Long(requested)) = get("size") {
        size = requested as i32;
        argon2_check_capability(
            stack,
            NAME,
            size,
            ARGON2_MAXSIZE_DEFAULT,
            "argon2.maxsize",
            "requested output size exceeds capability value.",
        )?;
    }
    let password = match get("password") {
        Some(Value::Bytes(bytes)) => bytes.to_vec(),
        Some(Value::String(s)) => s.as_bytes().to_vec(),
        _ => {
            return Err(fail(NAME, "missing password as either STRING or BYTES."));
        }
    };
    drop(map);
    let out = argon2::generate(&params, &password, i64::from(size))?;
    stack.push(Value::bytes(out));
    Ok(())
}
