// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Running a script and reporting the stack it leaves, like the `/api/v0/exec` endpoint of
//! Warp 10 does.

use std::sync::Arc;

use warpscript::datastore::Datastore;
use warpscript::functions::json::{JsonError, JsonOptions, to_json};
use warpscript::{Error, Stack, Value};

/// A script that ran.
#[derive(Debug, PartialEq, Eq)]
pub struct Execution {
    /// The stack, top first, as a JSON array.
    pub json: String,
    /// The number of operations the script took.
    pub ops: u64,
}

/// A script that failed.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("{}", self.report())]
pub struct ScriptError {
    /// The line of the script that failed, when known.
    pub line: Option<usize>,
    /// What failed (`Exception at '…' in section […] (…)`).
    pub message: String,
}

impl ScriptError {
    fn report(&self) -> String {
        match self.line {
            Some(line) => format!("Line #{line}: {}", self.message),
            None => self.message.clone(),
        }
    }

    /// The line and the message of a failure of the interpreter.
    fn of(err: &Error) -> ScriptError {
        match err {
            Error::Line { line, source } => ScriptError {
                line: Some(*line),
                message: source.to_string(),
            },
            other => ScriptError {
                line: None,
                message: other.to_string(),
            },
        }
    }
}

/// Run a script on a new stack with the default limits of Warp 10, connected to a datastore.
///
/// # Errors
///
/// The failure of the script, with the line it happened on, or a stack that cannot be written
/// as JSON.
pub fn execute(
    script: &str,
    datastore: Option<Arc<dyn Datastore>>,
) -> Result<Execution, ScriptError> {
    let mut stack = Stack::new();
    stack.set_datastore(datastore);
    stack
        .exec_checked(script)
        .map_err(|err| ScriptError::of(&err))?;
    // The stack, top first.
    let levels: Vec<Value> = stack.elements().iter().rev().cloned().collect();
    let options = JsonOptions {
        max_size: stack.limits.max_json,
        ..JsonOptions::default()
    };
    let json = to_json(&Value::list(levels), options).map_err(|err| ScriptError {
        line: None,
        message: match err {
            JsonError::TooBig => "Resulting JSON is too big.".to_owned(),
            JsonError::TooDeep => {
                "The stack is too deep to be written as JSON, or it references itself.".to_owned()
            }
        },
    })?;
    Ok(Execution {
        json,
        ops: stack.ops(),
    })
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod exec_tests;
