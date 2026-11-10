// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! `influxftl` — WarpScript / WarpLib for InfluxDB 3.

use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    let mut streams = influxftl::Io {
        stdin: &mut stdin,
        stdout: &mut stdout,
    };
    match influxftl::run(&args, &mut streams) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("influxftl: {err:#}");
            ExitCode::FAILURE
        }
    }
}
