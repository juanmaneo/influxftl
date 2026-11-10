// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! End-to-end tests of the `influxftl` binary.

use std::process::Command;

#[test]
fn prints_the_number_of_registered_functions() {
    let output = Command::new(env!("CARGO_BIN_EXE_influxftl"))
        .output()
        .expect("influxftl should start");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("the report is UTF-8");
    assert!(
        stdout.starts_with("influxftl: ") && stdout.trim_end().ends_with(" functions registered"),
        "{stdout}"
    );
}

/// `/dev/full` fails every write with `ENOSPC`: the failure is reported and the exit code is not 0.
#[cfg(target_os = "linux")]
#[test]
fn a_full_stdout_fails_the_process() {
    let full = std::fs::File::create("/dev/full").expect("/dev/full should be writable");
    let output = Command::new(env!("CARGO_BIN_EXE_influxftl"))
        .stdout(full)
        .output()
        .expect("influxftl should start");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("the report is UTF-8");
    assert!(stderr.contains("writing to stdout"), "{stderr}");
}

#[test]
fn evaluates_a_script() {
    let output = Command::new(env!("CARGO_BIN_EXE_influxftl"))
        .args(["eval", "1", "2", "+"])
        .output()
        .expect("influxftl should start");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "[3]\n");
}

#[test]
fn a_failing_script_fails_the_process() {
    let output = Command::new(env!("CARGO_BIN_EXE_influxftl"))
        .args(["eval", "DROP"])
        .output()
        .expect("influxftl should start");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("influxftl: running the script: Line #1: "),
        "{stderr}"
    );
    assert!(stderr.contains("Empty stack."), "{stderr}");
}

#[test]
fn runs_the_example() {
    let output = Command::new(env!("CARGO_BIN_EXE_influxftl"))
        .args([
            "run",
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/hello.mc2"),
        ])
        .output()
        .expect("influxftl should start");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "[21.0,\"hello WarpScript\"]\n"
    );
}
