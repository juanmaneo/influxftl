// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the platform functions: tokens, capabilities, limits, secure scripts,
//! macro configuration, snapshots. Worded like Warp 10's because a script reads them.

use crate::snapshot::MAX_RECURSION_LEVEL;

/// A failure of a platform function.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlatformError {
    /// A token that cannot be read.
    #[error("Invalid token.")]
    InvalidToken,
    /// A capability token that cannot be read.
    #[error("invalid token.")]
    InvalidCapabilityToken,
    /// A capability value that is not a number.
    #[error("Error parsing capability.")]
    CapabilityParse,
    /// A stack depth above the maximum the script was allowed.
    #[error("The stack depth is over the requested maximum depth.")]
    DepthAboveMax,
    /// A secure script operation before the secure key is set.
    #[error("You need to set the secure key first.")]
    NoSecureKey,
    /// A secure script without the encryption key.
    #[error("Missing secure script encryption key.")]
    NoSecureScriptKey,
    /// A secure script that was not encrypted with the key set.
    #[error("Invalid secure key.")]
    InvalidSecureKey,
    /// A macro configuration key that is set nowhere.
    #[error("{function} macro configuration '{key}' not found, need to set one of [{attempts} ].")]
    MacroConfigNotFound {
        /// The function that read it.
        function: String,
        /// The key.
        key: String,
        /// The keys tried, each preceded by a space and quoted.
        attempts: String,
    },
    /// A structure nested deeper than a snapshot can render.
    #[error("Recursive data structures exceeded {} levels.", MAX_RECURSION_LEVEL)]
    RecursiveStructure,
    /// `MSGFAIL`: the message the script chose.
    #[error("MSGFAIL {0}")]
    Msgfail(String),
    /// The operating system could not provide entropy.
    #[error("Unable to obtain random data from the operating system ({0}).")]
    Entropy(getrandom::Error),
}
