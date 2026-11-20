// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The signatures of OpenPGP for RSA keys, as BouncyCastle's `PGPSignatureGenerator` writes them
//! and `PGPSignature.verify` checks them: version 4 signatures, and the one-pass signed messages
//! that wrap a literal packet around the signed data.

use num_bigint::{BigInt, BigUint};

use crate::ec::system_random;
use crate::error::pgp::PgpError;
use crate::openpgp::key::{PublicKey, read_big};
use crate::openpgp::packet::{PacketReader, write_new_packet, write_packet};
use crate::openpgp::secret::{take_array, take_byte, take_slice};
use crate::rsa::{Exponentiation, Hash, RsaKey, Scheme};

/// The digests of the signatures, by name, with their identifiers in OpenPGP.
const DIGESTS: &[(&str, u8, Hash)] = &[
    ("MD2", 5, Hash::Md2),
    ("MD5", 1, Hash::Md5),
    ("SHA1", 2, Hash::Sha1),
    ("RIPEMD160", 3, Hash::Ripemd160),
    ("SHA224", 11, Hash::Sha224),
    ("SHA256", 8, Hash::Sha256),
    ("SHA384", 9, Hash::Sha384),
    ("SHA512", 10, Hash::Sha512),
];

/// The identifier of SHA3-256, which BouncyCastle names but the port does not compute.
const SHA3_256: u8 = 12;

/// The version of the signatures.
const VERSION: u8 = 4;

/// The version of the one-pass signed message's packet.
const ONE_PASS_VERSION: u8 = 3;

/// The signature type of a signature over binary data.
const BINARY: u8 = 0x00;

/// The subpacket type of the creation time of a signature.
const CREATION_TIME: u8 = 2;

/// The subpacket type of the issuer key ID of a signature.
const ISSUER_KEY_ID: u8 = 16;

/// The tag of a signature packet.
const SIGNATURE: u8 = 2;

/// The tag of a one-pass signed message's packet.
const ONE_PASS: u8 = 4;

/// The tag of a literal data packet.
const LITERAL: u8 = 11;

/// The format of a literal packet of binary data.
const BINARY_FORMAT: u8 = b'b';

/// The file name of a literal packet made by the console functions.
const LITERAL_NAME: &[u8] = b"_CONSOLE";

/// The identifier and the digest of a digest name, in the case BouncyCastle reads it
/// (`PGPUtil.getDigestIDForName`), or the failure of a name it does not know.
pub fn digest_by_name(name: &str) -> Result<(u8, Hash), PgpError> {
    let upper = name.to_ascii_uppercase();
    if upper == "SHA3-256" {
        return Err(PgpError::UnportedDigest(SHA3_256));
    }
    DIGESTS
        .iter()
        .find(|(known, _, _)| *known == upper)
        .map(|&(_, id, hash)| (id, hash))
        .ok_or_else(|| PgpError::UnknownDigest(name.to_ascii_lowercase()))
}

/// The digest of an identifier of OpenPGP, or the failure of an identifier the port does not
/// compute.
pub fn hash_of_id(id: u8) -> Result<Hash, PgpError> {
    DIGESTS
        .iter()
        .find(|(_, known, _)| *known == id)
        .map(|&(_, _, hash)| hash)
        .ok_or(PgpError::UnportedDigest(id))
}

/// What a signature is made with: the RSA key, the algorithm and the key ID of the key, the
/// digest, and the creation time in seconds.
pub struct SigningParams<'a> {
    /// The private key.
    pub key: &'a RsaKey,
    /// The public-key algorithm of the key.
    pub algorithm: u8,
    /// The key ID of the key.
    pub key_id: u64,
    /// The identifier of the digest in OpenPGP.
    pub hash_id: u8,
    /// The digest.
    pub hash: Hash,
    /// The creation time of the signature, in seconds since the epoch.
    pub created: u32,
}

/// The signature packet of some data, which is detached from them.
pub fn detached_packet(params: &SigningParams<'_>, data: &[u8]) -> Result<Vec<u8>, PgpError> {
    let mut out = Vec::new();
    write_packet(&mut out, SIGNATURE, &signature_body(params, data)?);
    Ok(out)
}

/// The one-pass signed message of some data: the one-pass packet, a literal packet of the data
/// dated `date` and the signature packet.
pub fn inline_message(
    params: &SigningParams<'_>,
    date: u32,
    data: &[u8],
) -> Result<Vec<u8>, PgpError> {
    let mut out = Vec::new();
    write_packet(&mut out, ONE_PASS, &one_pass(params));
    write_new_packet(&mut out, LITERAL, &literal(date, data));
    write_packet(&mut out, SIGNATURE, &signature_body(params, data)?);
    Ok(out)
}

/// The RSA key of the body of a public key packet, to verify the signatures with.
pub fn public_rsa(body: &[u8]) -> Result<RsaKey, PgpError> {
    let (modulus, exponent, _) = PublicKey::rsa_numbers(body)?;
    Ok(RsaKey {
        modulus: BigInt::from(modulus),
        exponent: BigInt::from(exponent),
    })
}

/// The body of the signature packet of a detached signature.
pub fn detached_body(input: &[u8]) -> Result<Vec<u8>, PgpError> {
    take_packet(&mut PacketReader::new(input), SIGNATURE)
}

/// The data and the body of the signature of a one-pass signed message.
pub fn read_inline(input: &[u8]) -> Result<(Vec<u8>, Vec<u8>), PgpError> {
    let mut reader = PacketReader::new(input);
    take_packet(&mut reader, ONE_PASS)?;
    let literal = take_packet(&mut reader, LITERAL)?;
    let signature = take_packet(&mut reader, SIGNATURE)?;
    Ok((literal_content(&literal)?, signature))
}

/// Whether a version 4 signature body is the signature of some data under a public key. The
/// two bytes of the digest that open the signature are compared first, as BouncyCastle does.
pub fn verify(key: &RsaKey, body: &[u8], data: &[u8]) -> Result<bool, PgpError> {
    let signed = read_signature(body)?;
    let message = message(data, &signed.header);
    if left16(signed.hash, &message) != signed.left {
        return Ok(false);
    }
    let public = Exponentiation::new(key)?;
    let verdict = public
        .signer(Scheme::Pkcs1(signed.hash))?
        .verify(&message, &signed.signature.to_bytes_be());
    Ok(verdict.is_ok())
}

/// The header of a version 4 signature: its version, type, algorithm, digest and the hashed
/// area, which holds the creation time.
fn header(params: &SigningParams<'_>) -> Vec<u8> {
    let mut header = vec![
        VERSION,
        BINARY,
        params.algorithm,
        params.hash_id,
        0,
        6,
        5,
        CREATION_TIME,
    ];
    header.extend(params.created.to_be_bytes());
    header
}

/// The bytes that a signature covers: the data, then the header, then the trailer that repeats
/// the version and the length of the header.
fn message(data: &[u8], header: &[u8]) -> Vec<u8> {
    let mut message = data.to_vec();
    message.extend(header);
    message.extend([VERSION, 0xFF]);
    message.extend((header.len() as u32).to_be_bytes());
    message
}

/// The first two bytes of the digest of the message.
fn left16(hash: Hash, message: &[u8]) -> Vec<u8> {
    let mut digest = hash.digest(message);
    digest.truncate(2);
    digest
}

/// The body of a version 4 signature of some data: its header, the issuer key ID, the digest
/// prefix and the signature MPI.
fn signature_body(params: &SigningParams<'_>, data: &[u8]) -> Result<Vec<u8>, PgpError> {
    let header = header(params);
    let message = message(data, &header);
    let key = Exponentiation::new(params.key)?;
    let signature = key
        .signer(Scheme::Pkcs1(params.hash))?
        .sign(&message, &mut system_random)?;
    let mut body = header;
    body.extend([0, 10, 9, ISSUER_KEY_ID]);
    body.extend(params.key_id.to_be_bytes());
    body.extend(left16(params.hash, &message));
    write_mpi(&mut body, &BigUint::from_bytes_be(&signature));
    Ok(body)
}

/// The body of the one-pass packet of a message: its version, type, digest, algorithm, the key
/// ID, and 1 for the packet that follows it is the last.
fn one_pass(params: &SigningParams<'_>) -> Vec<u8> {
    let mut body = vec![ONE_PASS_VERSION, BINARY, params.hash_id, params.algorithm];
    body.extend(params.key_id.to_be_bytes());
    body.push(1);
    body
}

/// The body of a literal packet of binary data named `_CONSOLE`.
fn literal(date: u32, data: &[u8]) -> Vec<u8> {
    let mut body = vec![BINARY_FORMAT, LITERAL_NAME.len() as u8];
    body.extend(LITERAL_NAME);
    body.extend(date.to_be_bytes());
    body.extend(data);
    body
}

/// The data of a literal packet: after its format and its file name, and its date.
fn literal_content(body: &[u8]) -> Result<Vec<u8>, PgpError> {
    let mut rest = body;
    take_byte(&mut rest)?;
    let name_length = take_byte(&mut rest)?;
    take_slice(&mut rest, usize::from(name_length))?;
    take_array::<4>(&mut rest)?;
    Ok(rest.to_vec())
}

/// The packet of the reader, whose tag must be `tag`, and its body.
fn take_packet(reader: &mut PacketReader<'_>, tag: u8) -> Result<Vec<u8>, PgpError> {
    let packet = reader.read()?;
    if packet.tag == tag {
        Ok(packet.body)
    } else {
        Err(PgpError::UnexpectedPacket(packet.tag))
    }
}

/// A version 4 signature as it is read: the header that the signature covers, the digest and its
/// prefix, and the signature MPI.
struct Signed {
    /// The version, type, algorithm, digest and hashed area, as the signature covers them.
    header: Vec<u8>,
    /// The digest of the signature.
    hash: Hash,
    /// The first two bytes of the digest of the signed message.
    left: Vec<u8>,
    /// The signature.
    signature: BigUint,
}

/// A version 4 signature read from its body. The signature packet's unhashed area is skipped.
fn read_signature(body: &[u8]) -> Result<Signed, PgpError> {
    let mut rest = body;
    let version = take_byte(&mut rest)?;
    if version != VERSION {
        return Err(PgpError::UnsupportedVersion(version));
    }
    let class = take_byte(&mut rest)?;
    let algorithm = take_byte(&mut rest)?;
    let hash_id = take_byte(&mut rest)?;
    let hashed_length: [u8; 2] = take_array(&mut rest)?;
    let hashed = take_slice(&mut rest, usize::from(u16::from_be_bytes(hashed_length)))?;
    let unhashed_length: [u8; 2] = take_array(&mut rest)?;
    take_slice(&mut rest, usize::from(u16::from_be_bytes(unhashed_length)))?;
    let left = take_slice(&mut rest, 2)?.to_vec();
    let signature = read_big(&mut rest)?;
    let mut header = vec![version, class, algorithm, hash_id];
    header.extend(hashed_length);
    header.extend(hashed);
    Ok(Signed {
        header,
        hash: hash_of_id(hash_id)?,
        left,
        signature,
    })
}

/// The MPI of a number: its bit length, then its bytes.
fn write_mpi(out: &mut Vec<u8>, number: &BigUint) {
    out.extend((number.bits() as u16).to_be_bytes());
    out.extend(number.to_bytes_be());
}
