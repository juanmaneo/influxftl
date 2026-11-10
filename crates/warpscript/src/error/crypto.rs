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
