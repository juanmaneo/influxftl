// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! `influxftl` — WarpScript / WarpLib for InfluxDB 3: the command line (`eval`, `run`,
//! `serve`, `functions`) and the HTTP execution endpoint (`POST /api/v0/exec`) of the
//! `influxftl` binary.

pub mod cli;
pub mod exec;
pub mod http;
pub mod influx;

use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;

use warpscript::datastore::{Datastore, MemoryDatastore};

use cli::{Command, USAGE};

/// The standard streams of a command, which tests replace.
pub struct Io<'a> {
    /// What `eval -` reads the script from.
    pub stdin: &'a mut dyn Read,
    /// Where the results go.
    pub stdout: &'a mut dyn Write,
    /// The datastore the scripts work on, when there is one.
    pub datastore: Option<Arc<dyn Datastore>>,
}

/// The environment variable that chooses the datastore (`memory` or `influxdb3`).
pub const DATASTORE_VARIABLE: &str = "INFLUXFTL_DATASTORE";

/// The datastore the environment chooses, through [`DATASTORE_VARIABLE`]: none when it is not
/// set, a datastore in memory (which forgets everything when the process ends) for `memory`,
/// InfluxDB 3 for `influxdb3` (see [`influx::connect`] for the settings it reads).
///
/// # Errors
///
/// A setting that is not a datastore, or that InfluxDB 3 cannot be connected with.
pub fn datastore_from(
    lookup: &dyn Fn(&str) -> Option<OsString>,
) -> anyhow::Result<Option<Arc<dyn Datastore>>> {
    let Some(setting) = lookup(DATASTORE_VARIABLE) else {
        return Ok(None);
    };
    match setting.to_str() {
        Some("memory") => Ok(Some(Arc::new(MemoryDatastore::new()))),
        Some("influxdb3") => Ok(Some(Arc::new(
            influx::connect(lookup, influx::TIMEOUT).context("connecting InfluxDB 3")?,
        ))),
        Some(other) => anyhow::bail!(
            "{DATASTORE_VARIABLE}: unknown datastore '{other}' (`memory` and `influxdb3` are the only ones)"
        ),
        None => anyhow::bail!("{DATASTORE_VARIABLE} is not UTF-8"),
    }
}

/// Run a command line (without the name of the program).
///
/// # Errors
///
/// A wrong command line, a script that fails, a file or a socket that cannot be used.
pub fn run(args: &[String], io: &mut Io<'_>) -> anyhow::Result<()> {
    run_with(args, io, None)
}

/// [`run`], `serve` stopping after `connections` connections (it serves for ever without).
///
/// # Errors
///
/// A wrong command line, a script that fails, a file or a socket that cannot be used.
pub fn run_with(
    args: &[String],
    io: &mut Io<'_>,
    connections: Option<usize>,
) -> anyhow::Result<()> {
    let output = match cli::parse(args)? {
        Command::Summary => format!(
            "influxftl: {} functions registered\n",
            warpscript::functions::registered_names().len()
        ),
        Command::Help => USAGE.to_owned(),
        Command::Version => format!("influxftl {}\n", env!("CARGO_PKG_VERSION")),
        Command::Functions => warpscript::functions::registered_names().join("\n") + "\n",
        Command::Eval(script) => {
            let script = match script {
                Some(script) => script,
                None => {
                    let mut script = String::new();
                    io.stdin
                        .read_to_string(&mut script)
                        .context("reading the script from stdin")?;
                    script
                }
            };
            stacks(&[(None, script)], io.datastore.as_ref())?
        }
        Command::Run(files) => {
            let mut scripts = Vec::new();
            for file in files {
                let script = std::fs::read_to_string(&file)
                    .with_context(|| format!("reading {}", file.display()))?;
                scripts.push((Some(file), script));
            }
            stacks(&scripts, io.datastore.as_ref())?
        }
        Command::Serve(address) => return serve(&address, io, connections),
    };
    io.stdout
        .write_all(output.as_bytes())
        .context("writing to stdout")?;
    io.stdout.flush().context("flushing stdout")
}

/// The stacks the scripts leave, top first, as JSON, a line for each script.
fn stacks(
    scripts: &[(Option<PathBuf>, String)],
    datastore: Option<&Arc<dyn Datastore>>,
) -> anyhow::Result<String> {
    let mut output = String::new();
    for (file, script) in scripts {
        let execution = exec::execute(script, datastore.cloned()).with_context(|| match file {
            Some(file) => format!("running {}", file.display()),
            None => "running the script".to_owned(),
        })?;
        output.push_str(&execution.json);
        output.push('\n');
    }
    Ok(output)
}

/// Serve the execution endpoint on an address.
fn serve(address: &str, io: &mut Io<'_>, connections: Option<usize>) -> anyhow::Result<()> {
    let listener = TcpListener::bind(address).with_context(|| format!("listening on {address}"))?;
    writeln!(io.stdout, "influxftl: listening on http://{address}").context("writing to stdout")?;
    io.stdout.flush().context("flushing stdout")?;
    http::serve(&listener, connections, io.datastore.as_ref()).context("serving")
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
