// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the text and binary functions: substrings, base64, regular expressions and
//! their replacement syntax, RLP. Worded like Warp 10's (and the JDK's) because a script reads
//! them.

use std::fmt;

/// A regular expression failure (a syntax error, or a limit of the matching engine), shareable
/// and comparable by its message so that errors stay `Clone + PartialEq`.
#[derive(Debug, Clone)]
pub struct RegexFailure(fancy_regex::Error);

impl From<fancy_regex::Error> for RegexFailure {
    fn from(err: fancy_regex::Error) -> Self {
        RegexFailure(err)
    }
}

impl PartialEq for RegexFailure {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_string() == other.0.to_string()
    }
}

impl Eq for RegexFailure {}

impl fmt::Display for RegexFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for RegexFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        std::error::Error::source(&self.0)
    }
}

/// A failure of a text or binary function.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    /// A byte slice whose end precedes its start.
    #[error("{start} > {end}")]
    SliceReversed {
        /// The first index.
        start: i32,
        /// The end.
        end: i32,
    },
    /// An OPB64 text with a character that is not in the alphabet.
    #[error("Invalid OPB64 character '{c}' at offset {offset}.")]
    InvalidOpb64Character {
        /// The character.
        c: char,
        /// Its offset.
        offset: usize,
    },
    /// A hexadecimal text of an odd number of digits.
    #[error("Odd number of hexadecimal digits.")]
    OddHexDigits,
    /// A hexadecimal text with something else than hexadecimal digits.
    #[error("Invalid hex character.")]
    InvalidHexCharacter,
    /// A base64 text whose length cannot be a base64 length.
    #[error("Invalid input length {0}")]
    Base64Length(usize),
    /// A base64 text with a printable character that is not in the alphabet.
    #[error("Unrecognized character: {0}")]
    Base64Character(char),
    /// A base64 text with another character that is not in the alphabet.
    #[error("Unrecognized character: 0x{0:x}")]
    Base64CharacterCode(u16),
    /// A base64 text the decoder refuses.
    #[error(transparent)]
    Base64(#[from] base64::DecodeError),
    /// A JSON text that cannot be parsed: Jackson's message, then where it stopped.
    #[error("{message}\n at {location}")]
    JsonLocated {
        /// The message.
        message: String,
        /// The location in the text.
        location: String,
    },
    /// A JSON text nested deeper than the limit.
    #[error(
        "Document nesting depth ({depth}) exceeds the maximum allowed ({max}, from `StreamReadConstraints.getMaxNestingDepth()`)"
    )]
    JsonNestingDepth {
        /// The depth reached.
        depth: usize,
        /// The limit.
        max: usize,
    },
    /// A JSON text longer than the limit.
    #[error("Cannot write, maximum number of characters written :{0}")]
    JsonWriteLimit(i64),
    /// A template error with the line of the template it was found at.
    #[error("{message} @[:{line}]")]
    MustacheAt {
        /// The message.
        message: String,
        /// The line.
        line: u32,
    },
    /// A template that includes another one, which is not allowed.
    #[error("Template inclusion is disabled.")]
    InclusionDisabled,
    /// A code in an `{{<extend}}` section that is not allowed there.
    #[error("Illegal code in extend section: com.github.mustachejava.codes.{0}")]
    IllegalExtendCode(&'static str),
    /// A template substitution of a value that is not a string.
    #[error("Invalid type for template substitution.")]
    TemplateSubstitutionType,
    /// A character set that does not exist.
    #[error("{0}")]
    UnsupportedCharset(String),
    /// A regular expression that is malformed, or that the engine gave up on.
    #[error(transparent)]
    Regex(#[from] RegexFailure),
    /// A replacement ending with a backslash.
    #[error("character to be escaped is missing")]
    MissingEscaped,
    /// A replacement ending with a dollar sign.
    #[error("Illegal group reference: group index is missing")]
    MissingGroupIndex,
    /// A `${}` without a name.
    #[error("named capturing group has 0 length name")]
    EmptyGroupName,
    /// A `${name` without its closing brace.
    #[error("named capturing group is missing trailing '}}'")]
    UnclosedGroupName,
    /// A `${1name}`.
    #[error("capturing group name {{{0}}} starts with digit character")]
    DigitGroupName(String),
    /// A `${name}` that names no group.
    #[error("No group with name {{{0}}}")]
    NoGroupNamed(String),
    /// A dollar sign followed by something else than a group.
    #[error("Illegal group reference")]
    IllegalGroupReference,
    /// A `$n` that names no group.
    #[error("No group {0}")]
    NoGroup(usize),
    /// An RLP list holding something that cannot be encoded as a long.
    #[error("Invalid element type {0}, only LONG elements are supported.")]
    LongElementExpected(String),
    /// An RLP input ending inside an item.
    #[error("Buffer underflow.")]
    RlpBufferUnderflow,
    /// An item that RLP cannot encode.
    #[error("Invalid item type, only LIST, LONG, BYTES and STRING are supported.")]
    RlpItemType,
    /// An RLP size that takes more than four bytes.
    #[error("Unable to support size over 2**32.")]
    RlpSizeTooLarge,
}
