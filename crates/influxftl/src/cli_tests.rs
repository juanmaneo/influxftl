// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the command line.

use std::path::PathBuf;

use super::{Command, DEFAULT_ADDRESS, UsageError, parse};

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn no_command_counts_the_functions() {
    assert_eq!(parse(&[]), Ok(Command::Summary));
}

#[test]
fn simple_commands() {
    for (words, command) in [
        (&["help"][..], Command::Help),
        (&["--help"], Command::Help),
        (&["-h"], Command::Help),
        (&["version"], Command::Version),
        (&["--version"], Command::Version),
        (&["-V"], Command::Version),
        (&["functions"], Command::Functions),
    ] {
        assert_eq!(parse(&args(words)), Ok(command), "{words:?}");
    }
}

#[test]
fn eval_takes_the_words_of_a_script_or_stdin() {
    assert_eq!(
        parse(&args(&["eval", "1", "2", "+"])),
        Ok(Command::Eval(Some("1 2 +".to_owned())))
    );
    assert_eq!(parse(&args(&["eval", "-"])), Ok(Command::Eval(None)));
    assert_eq!(
        parse(&args(&["eval"])),
        Err(UsageError::Missing {
            command: "eval",
            missing: "a script"
        })
    );
}

#[test]
fn run_takes_files() {
    assert_eq!(
        parse(&args(&["run", "a.mc2", "b.mc2"])),
        Ok(Command::Run(vec![
            PathBuf::from("a.mc2"),
            PathBuf::from("b.mc2")
        ]))
    );
    assert_eq!(
        parse(&args(&["run"])),
        Err(UsageError::Missing {
            command: "run",
            missing: "a file"
        })
    );
}

#[test]
fn serve_takes_an_address() {
    assert_eq!(
        parse(&args(&["serve"])),
        Ok(Command::Serve(DEFAULT_ADDRESS.to_owned()))
    );
    assert_eq!(
        parse(&args(&["serve", "--listen", "0.0.0.0:9"])),
        Ok(Command::Serve("0.0.0.0:9".to_owned()))
    );
    assert_eq!(
        parse(&args(&["serve", "--listen"])),
        Err(UsageError::Missing {
            command: "serve --listen",
            missing: "an address"
        })
    );
    assert_eq!(
        parse(&args(&["serve", "--port", "1"])),
        Err(UsageError::UnknownOption {
            command: "serve",
            option: "--port".to_owned()
        })
    );
}

#[test]
fn an_unknown_command_is_refused() {
    let err = parse(&args(&["bogus"])).expect_err("not a command");
    assert_eq!(err, UsageError::UnknownCommand("bogus".to_owned()));
    assert_eq!(
        err.to_string(),
        "unknown command 'bogus' (try `influxftl help`)"
    );
}
