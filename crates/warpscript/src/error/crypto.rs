// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the crypto functions (Argon2, Base58, key unwrapping), worded like the
//! messages of the Java implementations Warp 10 uses.

/// A failure of a crypto function.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CryptoError {
    /// An Argon2 output length below zero. The message is the length.
    #[error("{0}")]
    NegativeOutputLength(i64),
    /// An Argon2 parallelism below the minimum.
    #[error("lanes must be greater than {min}")]
    LanesTooFew {
        /// The smallest parallelism.
        min: i32,
    },
    /// An Argon2 parallelism above the maximum.
    #[error("lanes must be less than {max}")]
    LanesTooMany {
        /// The largest parallelism.
        max: i32,
    },
    /// An Argon2 memory below what its parallelism needs.
    #[error("memory is less than: {min} expected {min}")]
    MemoryTooSmall {
        /// The smallest memory, in blocks.
        min: i32,
    },
    /// An Argon2 iteration count below the minimum.
    #[error("iterations is less than: {min}")]
    IterationsTooFew {
        /// The smallest iteration count.
        min: i32,
    },
    /// An Argon2 output shorter than the minimum.
    #[error("output length less than {min}")]
    OutputTooShort {
        /// The shortest output length.
        min: usize,
    },
    /// A Base58 text with a character that is not a digit.
    #[error("Invalid input '{character}' at position {position}.")]
    InvalidBase58Character {
        /// The character, as a string (a lone surrogate is shown replaced).
        character: String,
        /// Its position.
        position: usize,
    },
    /// A key unwrapped to fewer bytes than its nonce.
    #[error("{nonce} > {unwrapped}")]
    UnwrappedTooShort {
        /// The length of the nonce.
        nonce: usize,
        /// The number of bytes unwrapped.
        unwrapped: usize,
    },
    /// A secret that cannot be split.
    #[error(transparent)]
    Split(#[from] SplitError),
    /// An elliptic curve operation that failed.
    #[error(transparent)]
    Ec(#[from] EcError),
    /// Bytes that are not a point of the curve.
    #[error(transparent)]
    Point(#[from] PointError),
    /// An RSA operation that failed.
    #[error(transparent)]
    Rsa(#[from] RsaError),
    /// A hexadecimal text that BouncyCastle cannot read: the reason, which it tells twice.
    #[error("exception decoding Hex string: {0} ({0})")]
    HexDecoding(String),
    /// A signature algorithm that BouncyCastle does not have.
    #[error("no such algorithm: {0} for provider BC")]
    NoSuchAlgorithm(String),
    /// A private key and a public key of different curves.
    #[error("Point must be on the same curve")]
    CurveMismatch,
    /// A byte of a signature that is not there (`ArrayIndexOutOfBoundsException`).
    #[error("{0}")]
    SignatureIndex(usize),
    /// The range of bytes of a signature that ends before it starts (`Arrays.copyOfRange`).
    #[error("{from} > {to}")]
    RangeInverted {
        /// Where the range starts.
        from: i64,
        /// Where it ends.
        to: i64,
    },
}

/// A failure of the splitting of a secret in shares.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SplitError {
    /// A number of shares or of shares needed that cannot be split.
    #[error(
        "cannot split a secret in {shares} shares of which {needed} are needed: expected \
         2 <= needed <= shares <= 255"
    )]
    InvalidParameters {
        /// The number of shares asked.
        shares: usize,
        /// The number of shares needed to recover the secret.
        needed: usize,
    },
    /// The operating system could not provide entropy.
    #[error("Unable to obtain random data from the operating system ({0}).")]
    Entropy(#[from] getrandom::Error),
}

/// Bytes that are not a point of the curve, worded like the `IllegalArgumentException`s of
/// BouncyCastle's `ECCurve.decodePoint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PointError {
    /// No byte at all (BouncyCastle reads the first one).
    #[error("0")]
    Empty,
    /// The point at infinity (`00`) followed by something.
    #[error("Incorrect length for infinity encoding")]
    InfinityLength,
    /// A compressed point (`02`, `03`) of the wrong length.
    #[error("Incorrect length for compressed encoding")]
    CompressedLength,
    /// An uncompressed point (`04`) of the wrong length.
    #[error("Incorrect length for uncompressed encoding")]
    UncompressedLength,
    /// A hybrid point (`06`, `07`) of the wrong length.
    #[error("Incorrect length for hybrid encoding")]
    HybridLength,
    /// A first byte that is no encoding.
    #[error("Invalid point encoding 0x{0:x}")]
    Encoding(u8),
    /// A coordinate that is not below the prime of the field.
    #[error("x value invalid for {0}")]
    FieldElement(&'static str),
    /// An abscissa that has no point.
    #[error("Invalid point compression")]
    Compression,
    /// Coordinates that are not those of a point of the curve.
    #[error("Invalid point coordinates")]
    Coordinates,
    /// A hybrid point whose tag does not tell the parity of its ordinate.
    #[error("Inconsistent Y coordinate in hybrid encoding")]
    HybridParity,
}

/// An input of the RSA engine that is as large as the modulus or longer than its block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("input too large for RSA cipher.")]
pub struct InputTooLarge;

/// A block that is not a block of data of PKCS#1, worded like `InvalidCipherTextException`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BlockError {
    /// A block shorter than the modulus allows.
    #[error("block truncated")]
    Truncated,
    /// A block longer than the modulus allows.
    #[error("block incorrect size")]
    IncorrectSize,
    /// A block that is neither of type 1 nor of type 2.
    #[error("unknown block type")]
    UnknownType,
    /// A block of type 1 whose padding is not made of `ff`.
    #[error("block padding incorrect")]
    PaddingIncorrect,
    /// A block without data, or with less than 8 bytes of padding.
    #[error("no data in block")]
    NoData,
}

/// The reason why a signature is not accepted, which `RSAVERIFY` answers `false` to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Rejection {
    /// A signature as large as the modulus, or longer than its block.
    #[error(transparent)]
    InputTooLarge(#[from] InputTooLarge),
    /// A signature that is not a block of PKCS#1.
    #[error(transparent)]
    Block(#[from] BlockError),
    /// A signature that is not the one of the data.
    #[error("signature does not match")]
    SignatureMismatch,
    /// A PSS block that does not end with its trailer.
    #[error("PSS block has no trailer")]
    PssTrailer,
    /// A PSS block whose padding is not the one of a PSS block.
    #[error("PSS padding incorrect")]
    PssPadding,
    /// A PSS block longer than the modulus allows.
    #[error("PSS block incorrect size")]
    PssBlockSize,
}

/// A failure of the RSA cryptography, worded like the exceptions of BouncyCastle whose message
/// a script can read. The failures that BouncyCastle's `CryptoHelper` turns into a `null` are
/// told apart here, and turned into what Warp 10 shows at the functions that handle them.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RsaError {
    /// A modulus below zero (`BigInteger.mod` refuses it).
    #[error("BigInteger: modulus not positive")]
    ModulusNotPositive,
    /// An even modulus: the exponentiation needs an odd one, and no RSA key has an even one.
    #[error("RSA modulus is even")]
    EvenModulus,
    /// A modulus that cannot hold a block of PKCS#1 (the 11 bytes of its framing and a byte of
    /// data), an endless loop in BouncyCastle.
    #[error("RSA modulus of {bits} bits is too small for PKCS#1 blocks")]
    ModulusTooSmall {
        /// The size of the modulus, in bits.
        bits: u64,
    },
    /// An exponent that is zero or below.
    #[error("RSA exponent is not positive")]
    ExponentNotPositive,
    /// An input as large as the modulus, or longer than its block.
    #[error(transparent)]
    InputTooLarge(#[from] InputTooLarge),
    /// A payload that does not fit a block of the key.
    #[error("input data too large")]
    DataTooLarge,
    /// A block of ciphertext that is not a block of data.
    #[error(transparent)]
    Block(#[from] BlockError),
    /// A PSS block that does not fit the key (a salt of the length of the digest).
    #[error("key too small for specified hash and salt lengths")]
    PssKeyTooSmall,
    /// A key of fewer than 12 bits.
    #[error("key strength too small")]
    StrengthTooSmall,
    /// A public exponent that is even.
    #[error("public exponent cannot be even")]
    ExponentEven,
    /// No prime that fits the key was found in the attempts a size allows.
    #[error("unable to generate prime number for RSA key")]
    PrimeNotFound,
    /// No pair of primes made a key in the attempts allowed (BouncyCastle tries for ever).
    #[error("unable to generate prime number for RSA key")]
    KeyNotFound,
    /// The operating system could not provide entropy.
    #[error("Unable to obtain random data from the operating system ({0}).")]
    Random(#[from] getrandom::Error),
}

/// A failure of the elliptic curve cryptography.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EcError {
    /// A signature that is not the DER of two integers.
    #[error("error decoding signature bytes.")]
    MalformedSignature,
    /// A private number that is not in `[1, order - 1]`.
    #[error("Scalar is not in the interval [1, n - 1]")]
    ScalarInterval,
    /// A number that is a multiple of the order, which has no inverse.
    #[error("BigInteger not invertible.")]
    NotInvertible,
    /// The operating system could not provide entropy.
    #[error("Unable to obtain random data from the operating system ({0}).")]
    Entropy(#[from] getrandom::Error),
}

/// A failure of the recovery of a secret from its shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RecoverError {
    /// No share at all.
    #[error("no share to recover the secret from")]
    NoShares,
    /// Two shares with the same abscissa: they are not shares of one split, or are repeated.
    #[error("duplicate abscissa: the shares cannot be combined")]
    DuplicateAbscissa,
}
