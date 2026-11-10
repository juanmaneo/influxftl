// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The command line of `influxftl`.

use std::path::PathBuf;

/// The text `influxftl help` prints.
pub const USAGE: &str = "\
influxftl — WarpScript / WarpLib for InfluxDB 3

Usage:
  influxftl                       count the registered functions
  influxftl eval <script>...      run a script (`-` reads it from stdin), print the stack as JSON
  influxftl run <file>...         run the scripts of files, print their stacks as JSON
  influxftl serve [--listen ADDR] serve POST /api/v0/exec (default 127.0.0.1:8080)
  influxftl functions             list the registered functions
  influxftl help | version
";

/// The address `serve` listens on by default.
pub const DEFAULT_ADDRESS: &str = "127.0.0.1:8080";

/// What a command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Count the registered functions.
    Summary,
    /// Print the usage.
    Help,
    /// Print the version.
    Version,
    /// List the registered functions.
    Functions,
    /// Run a script, given or (`None`) read from stdin.
    Eval(Option<String>),
    /// Run the scripts of files.
    Run(Vec<PathBuf>),
    /// Serve the execution endpoint on an address.
    Serve(String),
}

/// A command line that is not one.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum UsageError {
    /// A command that does not exist.
    #[error("unknown command '{0}' (try `influxftl help`)")]
    UnknownCommand(String),
    /// A command without what it works on.
    #[error("`influxftl {command}` needs {missing}")]
    Missing {
        /// The command.
        command: &'static str,
        /// What it needs.
        missing: &'static str,
    },
    /// An option the command does not have.
    #[error("`influxftl {command}` has no option '{option}'")]
    UnknownOption {
        /// The command.
        command: &'static str,
        /// The option.
        option: String,
    },
}

/// Read a command line (without the name of the program).
///
/// # Errors
///
/// A command that does not exist, a command without its arguments, an option that does not
/// exist.
pub fn parse(args: &[String]) -> Result<Command, UsageError> {
    let Some((command, rest)) = args.split_first() else {
        return Ok(Command::Summary);
    };
    match command.as_str() {
        "help" | "--help" | "-h" => Ok(Command::Help),
        "version" | "--version" | "-V" => Ok(Command::Version),
        "functions" => Ok(Command::Functions),
        "eval" => match rest {
            [] => Err(UsageError::Missing {
                command: "eval",
                missing: "a script",
            }),
            [dash] if dash == "-" => Ok(Command::Eval(None)),
            words => Ok(Command::Eval(Some(words.join(" ")))),
        },
        "run" => {
            if rest.is_empty() {
                Err(UsageError::Missing {
                    command: "run",
                    missing: "a file",
                })
            } else {
                Ok(Command::Run(rest.iter().map(PathBuf::from).collect()))
            }
        }
        "serve" => match rest {
            [] => Ok(Command::Serve(DEFAULT_ADDRESS.to_owned())),
            [option, address] if option == "--listen" => Ok(Command::Serve(address.clone())),
            [option] if option == "--listen" => Err(UsageError::Missing {
                command: "serve --listen",
                missing: "an address",
            }),
            [option, ..] => Err(UsageError::UnknownOption {
                command: "serve",
                option: option.clone(),
            }),
        },
        other => Err(UsageError::UnknownCommand(other.to_owned())),
    }
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod cli_tests;
