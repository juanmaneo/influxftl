// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the execution endpoint.

use std::io::{self, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use super::{Rejection, handle, serve, serve_connections};

/// A connection: the request to read, the answer written (which fails after `writes` calls).
struct Fake {
    input: Cursor<Vec<u8>>,
    output: Vec<u8>,
    writes: Option<usize>,
}

impl Fake {
    fn new(request: &[u8]) -> Fake {
        Fake {
            input: Cursor::new(request.to_vec()),
            output: Vec::new(),
            writes: None,
        }
    }
}

impl Read for Fake {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.input.read(buf)
    }
}

impl Write for Fake {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut self.writes {
            Some(0) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "client gone")),
            other => {
                if let Some(left) = other {
                    *left -= 1;
                }
                self.output.extend_from_slice(buf);
                Ok(buf.len())
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The answer to a request, as text.
fn answer(request: &[u8]) -> String {
    let mut fake = Fake::new(request);
    handle(&mut fake, None).expect("the connection should not fail");
    String::from_utf8(fake.output).expect("the answer is UTF-8")
}

/// The status line and the body of an answer.
fn status_and_body(answer: &str) -> (&str, &str) {
    let (head, body) = answer.split_once("\r\n\r\n").expect("a head and a body");
    (head.lines().next().expect("a status line"), body)
}

fn post(path: &str, body: &str) -> Vec<u8> {
    format!(
        "POST {path} HTTP/1.1\r\nHost: x\r\ncontent-length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

#[test]
fn a_script_is_run_and_the_stack_answered() {
    let text = answer(&post("/api/v0/exec", "1 2 +"));
    let (status, body) = status_and_body(&text);
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert_eq!(body, "[3]");
    assert!(
        text.contains("Content-Type: application/json;charset=utf-8\r\n"),
        "{text}"
    );
    assert!(text.contains("X-Warp10-Ops: 3\r\n"), "{text}");
    assert!(text.contains("X-Warp10-Elapsed: "), "{text}");
    assert!(
        text.contains("Content-Length: 3\r\nConnection: close\r\n"),
        "{text}"
    );
}

#[test]
fn a_script_that_fails_is_answered_with_its_line_and_message() {
    let text = answer(&post("/api/v0/exec", "1\nDROP DROP"));
    let (status, body) = status_and_body(&text);
    assert_eq!(status, "HTTP/1.1 500 Internal Server Error");
    assert!(body.contains("Empty stack."), "{body}");
    assert!(text.contains("X-Warp10-Error-Line: 2\r\n"), "{text}");
    assert!(
        text.contains("X-Warp10-Error-Message: Exception at '"),
        "{text}"
    );
}

#[test]
fn a_failure_without_a_line_has_no_line_header() {
    let text = answer(&post("/api/v0/exec", "<% 1"));
    assert!(text.starts_with("HTTP/1.1 500 "), "{text}");
    assert!(!text.contains("X-Warp10-Error-Line"), "{text}");
    assert!(
        text.contains("X-Warp10-Error-Message: Unbalanced <% construct.\r\n"),
        "{text}"
    );
}

#[test]
fn a_message_on_several_lines_is_a_header_on_one() {
    let text = answer(&post("/api/v0/exec", "'a\nb' MSGFAIL"));
    // The script is cut in lines: the first one fails on the unknown function.
    assert!(text.starts_with("HTTP/1.1 500 "), "{text}");
    let message = text
        .lines()
        .find(|line| line.starts_with("X-Warp10-Error-Message: "))
        .expect("the message header");
    assert!(!message.contains('\n'), "{message}");
}

#[test]
fn a_script_that_is_not_utf8_is_refused() {
    let mut request = b"POST /api/v0/exec HTTP/1.1\r\nContent-Length: 2\r\n\r\n".to_vec();
    request.extend_from_slice(&[0xff, 0xfe]);
    let text = answer(&request);
    let (status, body) = status_and_body(&text);
    assert_eq!(status, "HTTP/1.1 400 Bad Request");
    assert_eq!(body, "the script is not UTF-8");
}

#[test]
fn a_request_without_a_body_runs_the_empty_script() {
    let text = answer(b"POST /api/v0/exec HTTP/1.1\r\n\r\n");
    assert_eq!(status_and_body(&text).1, "[]");
}

#[test]
fn health_and_unknown_paths() {
    let text = answer(b"GET /health HTTP/1.1\r\n\r\n");
    assert_eq!(status_and_body(&text), ("HTTP/1.1 200 OK", "ok"));
    let text = answer(b"POST /health HTTP/1.1\r\n\r\n");
    assert_eq!(
        status_and_body(&text),
        ("HTTP/1.1 405 Method Not Allowed", "method not allowed")
    );
    let text = answer(b"GET /api/v0/exec HTTP/1.1\r\n\r\n");
    assert_eq!(
        status_and_body(&text),
        ("HTTP/1.1 405 Method Not Allowed", "method not allowed")
    );
    let text = answer(b"GET /elsewhere HTTP/1.1\r\n\r\n");
    assert_eq!(
        status_and_body(&text),
        ("HTTP/1.1 404 Not Found", "not found")
    );
}

#[test]
fn requests_that_are_not_http_are_refused() {
    for request in [
        &b"GET /health\r\n\r\n"[..],
        b"GET /health HTTP/1.1 more\r\n\r\n",
        b"GET /health SPDY/3\r\n\r\n",
        b"GET /health HTTP/1.1\r\nno colon here\r\n\r\n",
        b"GET /health HTTP/1.1\r\nHost: x",
        b"",
    ] {
        let text = answer(request);
        assert_eq!(
            status_and_body(&text),
            ("HTTP/1.1 400 Bad Request", "malformed request"),
            "{}",
            String::from_utf8_lossy(request)
        );
    }
}

#[test]
fn a_head_that_is_too_large_is_refused() {
    let request = format!(
        "GET /health HTTP/1.1\r\nX-Long: {}\r\n\r\n",
        "a".repeat(70_000)
    );
    let text = answer(request.as_bytes());
    assert_eq!(
        status_and_body(&text),
        (
            "HTTP/1.1 431 Request Header Fields Too Large",
            "request head too large"
        )
    );
}

#[test]
fn a_length_that_is_not_a_number_is_refused() {
    let text = answer(b"POST /api/v0/exec HTTP/1.1\r\nContent-Length: many\r\n\r\n");
    assert_eq!(
        status_and_body(&text),
        ("HTTP/1.1 400 Bad Request", "invalid Content-Length 'many'")
    );
}

#[test]
fn a_body_that_is_too_large_is_refused() {
    let text = answer(b"POST /api/v0/exec HTTP/1.1\r\nContent-Length: 99999999999\r\n\r\n");
    assert_eq!(
        status_and_body(&text),
        ("HTTP/1.1 413 Payload Too Large", "request body too large")
    );
}

#[test]
fn a_body_in_chunks_is_refused() {
    let text = answer(b"POST /api/v0/exec HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n");
    assert_eq!(
        status_and_body(&text),
        (
            "HTTP/1.1 411 Length Required",
            "chunked requests are not supported: send a Content-Length"
        )
    );
    // Another encoding is not chunked.
    let text = answer(b"POST /api/v0/exec HTTP/1.1\r\nTransfer-Encoding: identity\r\n\r\n");
    assert_eq!(status_and_body(&text).1, "[]");
}

/// A connection that cannot be read.
struct Unreadable;

impl Read for Unreadable {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::ConnectionReset, "reset"))
    }
}

impl Write for Unreadable {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_connection_that_cannot_be_read_fails() {
    let err = handle(&mut Unreadable, None).expect_err("the connection was reset");
    assert_eq!(err.kind(), io::ErrorKind::ConnectionReset);
}

#[test]
fn a_connection_that_ends_in_the_body_fails() {
    let mut fake = Fake::new(b"POST /api/v0/exec HTTP/1.1\r\nContent-Length: 10\r\n\r\nabc");
    let err = handle(&mut fake, None).expect_err("the body is cut");
    assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
}

#[test]
fn a_connection_that_cannot_be_written_fails() {
    for writes in 0.. {
        let mut fake = Fake::new(b"GET /health HTTP/1.1\r\n\r\n");
        fake.writes = Some(writes);
        match handle(&mut fake, None) {
            Ok(()) => {
                // Every write could be done: the answer has been written whole.
                assert!(writes > 4, "{writes}");
                break;
            }
            Err(err) => assert_eq!(err.kind(), io::ErrorKind::BrokenPipe, "{writes}"),
        }
    }
}

#[test]
fn rejections_have_a_status() {
    assert_eq!(Rejection::NotUtf8.to_string(), "the script is not UTF-8");
}

#[test]
fn a_listener_serves_its_connections() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let address = listener.local_addr().expect("an address");
    let server = thread::spawn(move || serve(&listener, Some(2), None));
    let mut answers = Vec::new();
    for script in ["1", "'a' 'b'"] {
        let mut client = TcpStream::connect(address).expect("the server should accept");
        write!(
            client,
            "POST /api/v0/exec HTTP/1.1\r\nContent-Length: {}\r\n\r\n{script}",
            script.len()
        )
        .expect("the request should be sent");
        let mut text = String::new();
        client
            .read_to_string(&mut text)
            .expect("the answer should be read");
        answers.push(status_and_body(&text).1.to_owned());
    }
    server
        .join()
        .expect("the server should not panic")
        .expect("the connections should be accepted");
    assert_eq!(answers, ["[1]", "[\"b\",\"a\"]"]);
}

#[test]
fn a_client_that_goes_away_does_not_stop_the_server() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let address = listener.local_addr().expect("an address");
    let server = thread::spawn(move || serve(&listener, Some(1), None));
    let mut client = TcpStream::connect(address).expect("the server should accept");
    client
        .write_all(b"POST /api/v0/exec HTTP/1.1\r\nContent-Length: 10\r\n\r\nabc")
        .expect("the request should be sent");
    drop(client);
    server
        .join()
        .expect("the server should not panic")
        .expect("the connection should be accepted");
}

#[test]
fn a_connection_that_cannot_be_accepted_stops_the_server() {
    let failing = io::Error::new(io::ErrorKind::ConnectionAborted, "aborted");
    let err = serve_connections(&mut vec![Err(failing)].into_iter(), None, None)
        .expect_err("the connection failed");
    assert_eq!(err.kind(), io::ErrorKind::ConnectionAborted);
}

#[test]
fn the_connections_share_the_datastore() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let address = listener.local_addr().expect("an address");
    let datastore: std::sync::Arc<dyn warpscript::datastore::Datastore> =
        std::sync::Arc::new(warpscript::datastore::MemoryDatastore::new());
    let server = thread::spawn(move || serve(&listener, Some(2), Some(&datastore)));
    let mut answers = Vec::new();
    for script in [
        "NEWGTS 'cpu' RENAME 1 NaN NaN NaN 5 ADDVALUE 'tok' UPDATE",
        "[ 'tok' 'cpu' { } 9 9 ] FETCH 0 GET VALUES",
    ] {
        let mut client = TcpStream::connect(address).expect("the server should accept");
        write!(
            client,
            "POST /api/v0/exec HTTP/1.1\r\nContent-Length: {}\r\n\r\n{script}",
            script.len()
        )
        .expect("the request should be sent");
        let mut text = String::new();
        client
            .read_to_string(&mut text)
            .expect("the answer should be read");
        answers.push(status_and_body(&text).1.to_owned());
    }
    server
        .join()
        .expect("the server should not panic")
        .expect("the connections should be accepted");
    assert_eq!(answers, ["[]", "[[5]]"]);
}
