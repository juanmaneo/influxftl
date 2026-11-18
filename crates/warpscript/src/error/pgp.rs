// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the OpenPGP functions (`PGPRING`, `PGPINFO`, `PGPPUBLIC`, `PGPPRIVATE`, …)
//! and of the decoding under them. Each message is the one BouncyCastle gives, because a
//! script reads it through the cause of the function's own failure.

/// A failure of the ASCII armor, worded like the `IOException`s and `ArmoredInputException`s of
/// BouncyCastle's `ArmoredInputStream`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArmorError {
    /// Input that ends inside an armored block.
    #[error("unexpected end of file in armored stream.")]
    UnexpectedEnd,
    /// A byte that is neither base64 nor a skipped space, or a base64 character outside the table.
    #[error("invalid armor")]
    InvalidArmor,
    /// A line of the armor headers that has no colon.
    #[error("invalid armor header")]
    InvalidArmorHeader,
    /// A CR LF pair in the headers that is not followed by LF after the headers.
    #[error("inconsistent line endings in headers")]
    InconsistentLineEndings,
    /// A CRC line that does not decode to exactly three bytes.
    #[error("malformed crc in armored message")]
    MalformedCrc,
    /// A CRC line whose value differs from the CRC-24 of the decoded bytes.
    #[error("crc check failed in armored message")]
    CrcCheckFailed,
    /// A base64 head of the stream with fewer than four valid characters.
    #[error("unable to decode base64 data: invalid characters encountered in base64 data")]
    Base64InvalidCharacters,
    /// A base64 head of the stream whose final block is invalid.
    #[error("unable to decode base64 data: invalid characters encountered at end of base64 data")]
    Base64InvalidCharactersAtEnd,
    /// A base64 head that starts with a line break, which BouncyCastle decodes past the end of its
    /// eight-byte buffer.
    #[error("unable to decode base64 data: 8")]
    Base64IndexOutOfBounds,
}

/// A failure of the OpenPGP functions, or of the packets, keys and rings under them.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PgpError {
    /// A failure of the ASCII armor.
    #[error(transparent)]
    Armor(#[from] ArmorError),
    /// Input that ends inside a packet.
    #[error("premature end of stream in PartialInputStream")]
    PrematureEnd,
    /// A packet whose header does not start with its high bit.
    #[error("invalid header encountered")]
    InvalidHeader,
    /// A key whose algorithm is not one of OpenPGP's (`-1` when the header ends first).
    #[error("unknown PGP public key algorithm encountered: {0}")]
    UnknownAlgorithm(i8),
    /// A key whose algorithm exists in OpenPGP but which the port does not read.
    #[error("unsupported PGP public key algorithm: {0}")]
    UnsupportedAlgorithm(u8),
    /// A key packet of a version the port does not read.
    #[error("unsupported version: {0}")]
    UnsupportedVersion(u8),
    /// An elliptic curve that is not one of those BouncyCastle knows.
    #[error("unknown curve OID")]
    UnknownCurve,
    /// Secret key material where a public key ring was expected.
    #[error("{found} found where PGPPublicKeyRing expected")]
    NotPublicRing {
        /// The Java class of the object that was read instead.
        found: &'static str,
    },
}
