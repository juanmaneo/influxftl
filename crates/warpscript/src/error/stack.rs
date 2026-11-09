// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the interpreter itself: limits, malformed scripts, unknown names. A script
//! sees them as `WarpScriptException`s worded like Warp 10's.

use crate::stack::{
    COMMENT_START, DEADLINE_MESSAGE, MACRO_START, MULTILINE_START, SECURE_SCRIPT_START,
};

/// A failure of the interpreter.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StackError {
    /// More elements on the stack than the limit allows.
    #[error(
        "Stack depth would exceed set limit of {max}. Consider raising the limit or using capabilities."
    )]
    DepthLimit {
        /// The limit.
        max: usize,
    },
    /// An access to more elements than the stack holds.
    #[error("Index out of bound.")]
    IndexOutOfBound,
    /// A stack index at or above the depth.
    #[error("Index out of bound, {index} >= {size}")]
    IndexAboveStack {
        /// The index.
        index: i32,
        /// The depth of the stack.
        size: i32,
    },
    /// A negative stack index further below than the depth.
    #[error("Index out of bound, {shifted} < -{size}")]
    IndexBelowStack {
        /// The index shifted by the depth.
        shifted: i32,
        /// The depth of the stack.
        size: i32,
    },
    /// A count or index that is not a number.
    #[error("Unexpected type, expecting a numerical value.")]
    NotNumeric,
    /// A value that is not a saved context.
    #[error("Invalid stack context.")]
    InvalidContext,
    /// `LOAD` / `STORE` / `CSTORE` with a symbol that is not written out.
    #[error("Encountered a LOAD/STORE or CSTORE operation with a non explicit symbol name.")]
    NonExplicitSymbol,
    /// An access to a level the stack does not have.
    #[error("Invalid level.")]
    InvalidLevel,
    /// More symbols than the limit allows.
    #[error("Symbol table has reached its maximum number of entries: {max}")]
    SymbolLimit {
        /// The limit.
        max: usize,
    },
    /// A register that does not exist.
    #[error("Invalid register number, must be between 0 and {max}")]
    InvalidRegister {
        /// The number of the last register.
        max: usize,
    },
    /// More operations than the limit allows.
    #[error(
        "Operation count ({count}) exceeded maximum of {max}. Consider raising the limit or using capabilities."
    )]
    OperationLimit {
        /// The number of operations executed.
        count: u64,
        /// The limit.
        max: u64,
    },
    /// Macros called from macros deeper than the limit allows.
    #[error("Maximum recursion level reached ({level})")]
    RecursionLimit {
        /// The level reached.
        level: u64,
    },
    /// The execution time limit.
    #[error("{}", DEADLINE_MESSAGE)]
    Deadline,
    /// A multiline string that is never closed.
    #[error("Unbalanced {} construct.", MULTILINE_START)]
    UnbalancedMultiline,
    /// A comment that is never closed.
    #[error("Unbalanced {} construct.", COMMENT_START)]
    UnbalancedComment,
    /// A secure script that is never closed.
    #[error("Unbalanced {} construct.", SECURE_SCRIPT_START)]
    UnbalancedSecureScript,
    /// A macro that is never closed.
    #[error("Unbalanced {} construct.", MACRO_START)]
    UnbalancedMacro,
    /// A comment end outside of a comment.
    #[error("Not inside a comment.")]
    NotInComment,
    /// A string literal without its closing quote.
    #[error("Cannot find end of string")]
    UnterminatedString,
    /// A multiline string start sharing its line with something else.
    #[error(
        "Can only start multiline strings by using {} on a line by itself.",
        MULTILINE_START
    )]
    MultilineNotAlone,
    /// A secure script end outside of a secure script.
    #[error("Not inside a secure script definition.")]
    NotInSecureScript,
    /// A secure script start inside a secure script.
    #[error("Already inside a secure script definition.")]
    AlreadyInSecureScript,
    /// A macro end outside of a macro.
    #[error("Not inside a macro definition.")]
    NotInMacro,
    /// A `!symbol` binding the value of a symbol that is not defined, inside a macro.
    #[error("Early binding is not possible inside a macro.")]
    EarlyBindingInMacro,
    /// A symbol that is not defined.
    #[error("Unknown symbol '{0}'")]
    UnknownSymbol(String),
    /// A function that was redefined where redefinitions are refused.
    #[error("Disallowed redefined function '{0}'.")]
    RedefinedFunction(String),
    /// A function that does not exist.
    #[error("Unknown function '{0}'")]
    UnknownFunction(String),
    /// A symbol that is defined but is not a macro.
    #[error("'{0}' is not a macro.")]
    NotAMacro(String),
    /// A macro that is not defined.
    #[error("Unknown macro '{0}'")]
    UnknownMacro(String),
    /// A forced macro opened inside a forced macro.
    #[error("Already in a forced Macro.")]
    AlreadyForcedMacro,
    /// A forced macro closed inside an unclosed macro.
    #[error("Invalid level for closing forced Macro, check that all macros are correctly closed.")]
    ForcedMacroLevel,
}
