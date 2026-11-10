// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the commands.

use std::io::{self, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::{Io, run, run_with};

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// The output of a command run on the given stdin.
fn output(words: &[&str], stdin: &str) -> anyhow::Result<String> {
    let mut input = Cursor::new(stdin.as_bytes().to_vec());
    let mut out = Vec::new();
    run(
        &args(words),
        &mut Io {
            stdin: &mut input,
            stdout: &mut out,
        },
    )?;
    Ok(String::from_utf8(out).expect("the output is UTF-8"))
}

/// A writer that accepts `accepts` bytes then fails; and a flush that fails when it is told.
struct Failing {
    accepts: usize,
    flush_fails: bool,
}

impl Write for Failing {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() <= self.accepts {
            self.accepts -= buf.len();
            Ok(buf.len())
        } else {
            Err(io::Error::new(io::ErrorKind::StorageFull, "device full"))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.flush_fails {
            Err(io::Error::new(io::ErrorKind::StorageFull, "device full"))
        } else {
            Ok(())
        }
    }
}

/// A reader that fails.
struct Broken;

impl Read for Broken {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "pipe broken"))
    }
}

#[test]
fn no_command_counts_the_functions() {
    let text = output(&[], "").expect("a summary");
    assert!(text.starts_with("influxftl: "), "{text}");
    assert!(text.ends_with(" functions registered\n"), "{text}");
}

#[test]
fn help_version_and_functions() {
    assert!(
        output(&["help"], "")
            .expect("usage")
            .contains("influxftl serve")
    );
    assert_eq!(
        output(&["version"], "").expect("version"),
        format!("influxftl {}\n", env!("CARGO_PKG_VERSION"))
    );
    let functions = output(&["functions"], "").expect("functions");
    assert!(functions.lines().any(|name| name == "STL"), "{functions}");
}

#[test]
fn eval_prints_the_stack() {
    assert_eq!(
        output(&["eval", "1", "2", "+"], "").expect("a stack"),
        "[3]\n"
    );
    assert_eq!(
        output(&["eval", "-"], "'a' 'b'").expect("a stack"),
        "[\"b\",\"a\"]\n"
    );
}

#[test]
fn eval_reports_the_failing_script() {
    let err = output(&["eval", "DROP"], "").expect_err("the stack is empty");
    assert_eq!(err.to_string(), "running the script");
    assert!(format!("{err:#}").contains("Empty stack."), "{err:#}");
}

#[test]
fn eval_reports_a_stdin_that_fails() {
    let mut out = Vec::new();
    let err = run(
        &args(&["eval", "-"]),
        &mut Io {
            stdin: &mut Broken,
            stdout: &mut out,
        },
    )
    .expect_err("the pipe is broken");
    assert_eq!(err.to_string(), "reading the script from stdin");
}

/// A file with the text, in the temporary directory.
fn file(name: &str, text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("influxftl-{}-{name}", std::process::id()));
    std::fs::write(&path, text).expect("the file should be written");
    path
}

#[test]
fn run_prints_the_stack_of_each_file() {
    let (a, b) = (file("a.mc2", "1 2"), file("b.mc2", "'x'"));
    let text = output(
        &[
            "run",
            a.to_str().expect("a UTF-8 path"),
            b.to_str().expect("a UTF-8 path"),
        ],
        "",
    )
    .expect("two stacks");
    assert_eq!(text, "[2,1]\n[\"x\"]\n");
    std::fs::remove_file(a).expect("the file should be removed");
    std::fs::remove_file(b).expect("the file should be removed");
}

#[test]
fn run_reports_a_file_that_fails_and_one_that_is_missing() {
    let bad = file("bad.mc2", "DROP");
    let path = bad.to_str().expect("a UTF-8 path").to_owned();
    let err = output(&["run", &path], "").expect_err("the stack is empty");
    assert_eq!(err.to_string(), format!("running {path}"));
    std::fs::remove_file(bad).expect("the file should be removed");
    let err = output(&["run", &path], "").expect_err("no file");
    assert_eq!(err.to_string(), format!("reading {path}"));
}

#[test]
fn a_failing_stdout_is_reported() {
    let mut input = Cursor::new(Vec::new());
    let run_on = |out: &mut Failing, words: &[&str], input: &mut Cursor<Vec<u8>>| {
        run(
            &args(words),
            &mut Io {
                stdin: input,
                stdout: out,
            },
        )
    };
    let err = run_on(
        &mut Failing {
            accepts: 0,
            flush_fails: false,
        },
        &[],
        &mut input,
    )
    .expect_err("the device is full");
    assert_eq!(err.to_string(), "writing to stdout");
    let err = run_on(
        &mut Failing {
            accepts: usize::MAX,
            flush_fails: true,
        },
        &[],
        &mut input,
    )
    .expect_err("the device is full");
    assert_eq!(err.to_string(), "flushing stdout");
}

#[test]
fn a_bad_command_line_is_reported() {
    let err = output(&["bogus"], "").expect_err("not a command");
    assert_eq!(
        err.to_string(),
        "unknown command 'bogus' (try `influxftl help`)"
    );
}

#[test]
fn serve_cannot_listen_on_a_bad_address() {
    let err = output(&["serve", "--listen", "no such address"], "").expect_err("a bad address");
    assert_eq!(err.to_string(), "listening on no such address");
}

/// A writer that gives a copy of what it received to the test.
#[derive(Clone)]
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("the lock is poisoned"))?
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A port nothing listens on.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("a port")
        .local_addr()
        .expect("an address")
        .port()
}

#[test]
fn serve_answers_a_request() {
    let shared = Shared(Arc::new(Mutex::new(Vec::new())));
    let mut writer = shared.clone();
    let address = format!("127.0.0.1:{}", free_port());
    let listen = address.clone();
    let server = thread::spawn(move || {
        let mut input = Cursor::new(Vec::new());
        run_with(
            &args(&["serve", "--listen", &listen]),
            &mut Io {
                stdin: &mut input,
                stdout: &mut writer,
            },
            Some(1),
        )
        .map_err(|err| format!("{err:#}"))
    });
    // The server announces that it listens.
    while !String::from_utf8_lossy(&shared.0.lock().expect("the lock")).contains("listening on") {
        thread::sleep(Duration::from_millis(5));
    }
    let mut client = TcpStream::connect(&address).expect("the server should accept");
    let script = "40 2 +";
    write!(
        client,
        "POST /api/v0/exec HTTP/1.1\r\nHost: {address}\r\nContent-Length: {}\r\n\r\n{script}",
        script.len()
    )
    .expect("the request should be sent");
    let mut response = String::new();
    client
        .read_to_string(&mut response)
        .expect("the response should be read");
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    assert!(response.ends_with("\r\n\r\n[42]"), "{response}");
    assert_eq!(server.join().expect("the server should not panic"), Ok(()));
    let announced =
        String::from_utf8(shared.0.lock().expect("the lock").clone()).expect("the output is UTF-8");
    assert_eq!(
        announced,
        format!("influxftl: listening on http://{address}\n")
    );
}

#[test]
fn serve_reports_a_failing_stdout() {
    let mut input = Cursor::new(Vec::new());
    let err = run_with(
        &args(&["serve", "--listen", "127.0.0.1:0"]),
        &mut Io {
            stdin: &mut input,
            stdout: &mut Failing {
                accepts: 0,
                flush_fails: false,
            },
        },
        Some(1),
    )
    .expect_err("the device is full");
    assert_eq!(err.to_string(), "writing to stdout");
    let mut input = Cursor::new(Vec::new());
    let err = run_with(
        &args(&["serve", "--listen", "127.0.0.1:0"]),
        &mut Io {
            stdin: &mut input,
            stdout: &mut Failing {
                accepts: usize::MAX,
                flush_fails: true,
            },
        },
        Some(1),
    )
    .expect_err("the device is full");
    assert_eq!(err.to_string(), "flushing stdout");
}
