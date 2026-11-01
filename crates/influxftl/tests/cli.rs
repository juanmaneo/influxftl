#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! End-to-end tests of the `influxftl` binary.

use std::process::Command;

#[test]
fn prints_the_number_of_registered_functions() {
    let output = Command::new(env!("CARGO_BIN_EXE_influxftl"))
        .output()
        .expect("failed to run influxftl");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.starts_with("influxftl: ") && stdout.trim_end().ends_with(" functions registered"),
        "{stdout}"
    );
}
