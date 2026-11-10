// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the HTTP client: the answers it reads, the connections that fail, a server on the
//! loopback interface.

use std::io::{self, BufReader, Cursor, Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use super::super::fake_server_tests::serve_once;
use super::{
    Endpoint, HttpClient, InfluxError, Limits, Request, Response, exchange, read_response,
};

fn endpoint(url: &str) -> Endpoint {
    Endpoint::parse(url).expect("a valid URL")
}

fn invalid_reason(url: &str) -> &'static str {
    match Endpoint::parse(url) {
        Err(InfluxError::InvalidUrl { reason, .. }) => reason,
        other => panic!("{url} should be an invalid URL, not {other:?}"),
    }
}

#[test]
fn an_endpoint_is_read_from_a_url() {
    let plain = endpoint("http://db");
    assert_eq!(plain.address(), "db:80");
    assert_eq!(plain.prefix, "");
    let with_port = endpoint("http://127.0.0.1:8181/");
    assert_eq!(with_port.address(), "127.0.0.1:8181");
    assert_eq!(with_port.prefix, "");
    assert_eq!(endpoint("http://db:1/a/b//").prefix, "/a/b");
    assert_eq!(endpoint("http://[::1]:8181").address(), "[::1]:8181");
    assert_eq!(endpoint("http://[::1]").address(), "[::1]:80");
}

#[test]
fn a_url_that_is_not_an_endpoint_is_refused() {
    assert!(invalid_reason("https://db").contains("TLS terminating proxy"));
    assert_eq!(invalid_reason("ftp://db"), "expected http://host[:port]");
    assert_eq!(invalid_reason("db:80"), "expected http://host[:port]");
    assert_eq!(invalid_reason("http://db:x"), "invalid port");
    assert_eq!(invalid_reason("http://db:99999"), "invalid port");
    assert_eq!(invalid_reason("http://:80"), "missing host");
    assert_eq!(invalid_reason("http:///path"), "missing host");
    let err = Endpoint::parse("ftp://db").expect_err("not http");
    assert_eq!(
        err.to_string(),
        "invalid InfluxDB 3 URL 'ftp://db': expected http://host[:port]"
    );
}

/// A connection that gives what it was given, keeps what is written to it and fails when told.
struct Wire {
    input: Cursor<Vec<u8>>,
    written: Vec<u8>,
    writes: usize,
    fail_write_at: Option<usize>,
    fail_flush: bool,
}

impl Wire {
    fn answering(answer: &[u8]) -> Wire {
        Wire {
            input: Cursor::new(answer.to_vec()),
            written: Vec::new(),
            writes: 0,
            fail_write_at: None,
            fail_flush: false,
        }
    }
}

impl Read for Wire {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.input.read(buf)
    }
}

impl Write for Wire {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.fail_write_at == Some(self.writes - 1) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "pipe broken"));
        }
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "pipe broken"))
        } else {
            Ok(())
        }
    }
}

/// A reader that gives what it was given, then fails.
struct ThenBroken(Cursor<Vec<u8>>);

impl Read for ThenBroken {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.0.read(buf)? {
            0 => Err(io::Error::new(io::ErrorKind::BrokenPipe, "pipe broken")),
            read => Ok(read),
        }
    }
}

const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi";

fn request(body: &[u8]) -> Request<'_> {
    Request {
        method: "POST",
        path: "/query?db=x",
        headers: &[
            ("Authorization", "Bearer t"),
            ("Content-Type", "text/plain"),
        ],
        body,
    }
}

fn answer(bytes: &[u8]) -> Result<Response, InfluxError> {
    read_response(&mut &bytes[..], &Limits::DEFAULT)
}

fn malformed(bytes: &[u8]) -> &'static str {
    match answer(bytes) {
        Err(InfluxError::MalformedResponse(reason)) => reason,
        other => panic!(
            "{:?} should be malformed, not {other:?}",
            String::from_utf8_lossy(bytes)
        ),
    }
}

fn io_failure(result: Result<Response, InfluxError>) -> (&'static str, io::ErrorKind) {
    match result {
        Err(InfluxError::Io { context, source }) => (context, source.kind()),
        other => panic!("an I/O failure was expected, not {other:?}"),
    }
}

#[test]
fn a_request_is_written_with_its_headers_and_its_body() {
    let mut wire = Wire::answering(OK);
    let response = exchange(
        &mut wire,
        &endpoint("http://db:8181/prefix"),
        &request(b"body"),
        &Limits::DEFAULT,
    )
    .expect("an answer");
    assert_eq!(
        response,
        Response {
            status: 200,
            body: b"hi".to_vec()
        }
    );
    assert_eq!(
        String::from_utf8(wire.written).expect("UTF-8"),
        format!(
            "POST /prefix/query?db=x HTTP/1.1\r\nHost: db:8181\r\nUser-Agent: influxftl/{}\r\nConnection: close\r\nContent-Length: 4\r\nAuthorization: Bearer t\r\nContent-Type: text/plain\r\n\r\nbody",
            env!("CARGO_PKG_VERSION")
        )
    );
}

#[test]
fn a_connection_that_fails_while_the_request_is_written_is_reported() {
    for (fail_write_at, fail_flush) in [(Some(0), false), (Some(1), false), (None, true)] {
        let mut wire = Wire::answering(OK);
        wire.fail_write_at = fail_write_at;
        wire.fail_flush = fail_flush;
        let result = exchange(
            &mut wire,
            &endpoint("http://db"),
            &request(b"body"),
            &Limits::DEFAULT,
        );
        assert_eq!(
            io_failure(result),
            ("sending the request", io::ErrorKind::BrokenPipe),
            "{fail_write_at:?} {fail_flush}"
        );
    }
}

#[test]
fn an_answer_is_read_by_its_length_by_chunks_or_until_the_connection_closes() {
    assert_eq!(answer(OK).expect("an answer").body, b"hi");
    assert_eq!(
        answer(b"HTTP/1.1 200 OK\r\nCONTENT-length:  3 \r\nX-Other: y\r\n\r\nabcdef")
            .expect("an answer")
            .body,
        b"abc"
    );
    assert_eq!(
        answer(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: Chunked\r\n\r\n3;ext=1\r\nabc\r\n2\r\nde\r\n0\r\nTrailer: x\r\n\r\nignored")
            .expect("an answer")
            .body,
        b"abcde"
    );
    assert_eq!(
        answer(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\nraw")
            .expect("an answer")
            .body,
        b"raw"
    );
    assert_eq!(
        answer(b"HTTP/1.0 404 Not Found\r\n\r\nnot here").expect("an answer"),
        Response {
            status: 404,
            body: b"not here".to_vec()
        }
    );
}

#[test]
fn an_answer_without_a_body_has_none() {
    for status in ["204 No Content", "304 Not Modified", "100 Continue"] {
        let bytes = format!("HTTP/1.1 {status}\r\nContent-Length: 5\r\n\r\nhello");
        assert!(
            answer(bytes.as_bytes()).expect("an answer").body.is_empty(),
            "{status}"
        );
    }
}

#[test]
fn an_answer_that_is_not_http_is_refused() {
    assert_eq!(malformed(b"garbage\r\n\r\n"), "not a status line");
    assert_eq!(malformed(b"\r\n\r\n"), "not a status line");
    assert_eq!(malformed(b"SMTP/1.1 200 OK\r\n\r\n"), "not a status line");
    assert_eq!(
        malformed(b"HTTP/1.1 abc OK\r\n\r\n"),
        "the status is not a number"
    );
    assert_eq!(
        malformed(b"HTTP/1.1 200 OK\r\nno colon\r\n\r\n"),
        "a header without a colon"
    );
    assert_eq!(
        malformed(b"HTTP/1.1 200 OK\r\nContent-Length: many\r\n\r\n"),
        "the Content-Length is not a number"
    );
    assert_eq!(malformed(b""), "the connection ended in the head");
    assert_eq!(
        malformed(b"HTTP/1.1 200 OK"),
        "the connection ended in the head"
    );
    assert_eq!(
        malformed(b"HTTP/1.1 200 OK\r\nHost: x"),
        "the connection ended in the head"
    );
}

#[test]
fn a_head_that_is_too_large_is_refused() {
    let limits = Limits {
        head: 40,
        body: 100,
    };
    let long_line = format!("HTTP/1.1 200 OK\r\nX: {}\r\n\r\n", "y".repeat(100));
    let err = read_response(&mut long_line.as_bytes(), &limits).expect_err("a long line");
    assert!(
        matches!(err, InfluxError::MalformedResponse("the head is too large")),
        "{err}"
    );
    let many_lines = "HTTP/1.1 200 OK\r\nA: 1\r\nB: 2\r\nC: 3\r\nD: 4\r\n\r\n";
    let err = read_response(&mut many_lines.as_bytes(), &limits).expect_err("many lines");
    assert!(
        matches!(err, InfluxError::MalformedResponse("the head is too large")),
        "{err}"
    );
    let exact = "HTTP/1.1 200 OK\r\nA: 1\r\n\r\n";
    assert!(exact.len() <= 40);
    read_response(&mut exact.as_bytes(), &limits).expect("a head that fits");
}

#[test]
fn a_body_that_is_too_large_is_refused() {
    let limits = Limits {
        head: 1000,
        body: 4,
    };
    let too_large = |bytes: &[u8]| {
        let err = read_response(&mut &bytes[..], &limits).expect_err("too large");
        assert!(
            matches!(err, InfluxError::ResponseTooLarge { limit: 4 }),
            "{err}"
        );
        assert_eq!(err.to_string(), "the HTTP response is larger than 4 bytes");
    };
    too_large(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello");
    too_large(b"HTTP/1.1 200 OK\r\n\r\nhello");
    too_large(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n");
    too_large(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n",
    );
    let fits = |bytes: &[u8]| {
        assert_eq!(
            read_response(&mut &bytes[..], &limits)
                .expect("a body that fits")
                .body,
            b"abcd"
        );
    };
    fits(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nabcd");
    fits(b"HTTP/1.1 200 OK\r\n\r\nabcd");
    fits(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nabcd\r\n0\r\n\r\n");
}

#[test]
fn a_chunked_body_that_is_not_well_formed_is_refused() {
    let head = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";
    let chunked = |body: &str| malformed(format!("{head}{body}").as_bytes());
    assert_eq!(chunked("zz\r\n"), "a chunk size is not hexadecimal");
    assert_eq!(
        chunked("1\r\naXX\r\n"),
        "a chunk is not followed by a line break"
    );
    assert_eq!(chunked(""), "the connection ended in the head");
    assert_eq!(chunked("0\r\n"), "the connection ended in the head");
    assert_eq!(chunked("1\r\na"), "the connection ended in the head");
}

#[test]
fn a_connection_that_fails_while_the_answer_is_read_is_reported() {
    let broken = |bytes: &[u8]| {
        read_response(
            &mut BufReader::new(ThenBroken(Cursor::new(bytes.to_vec()))),
            &Limits::DEFAULT,
        )
    };
    let reading = ("reading the response", io::ErrorKind::BrokenPipe);
    assert_eq!(io_failure(broken(b"")), reading);
    assert_eq!(
        io_failure(broken(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nab")),
        reading
    );
    assert_eq!(io_failure(broken(b"HTTP/1.1 200 OK\r\n\r\nab")), reading);
    assert_eq!(
        io_failure(broken(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nab"
        )),
        reading
    );
    assert_eq!(
        io_failure(broken(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n"
        )),
        reading
    );
    assert_eq!(
        io_failure(read_response(
            &mut &b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nab"[..],
            &Limits::DEFAULT
        )),
        ("reading the response", io::ErrorKind::UnexpectedEof)
    );
}

#[test]
fn a_request_is_sent_to_a_server() {
    let (endpoint, server) = serve_once(OK.to_vec());
    let client = HttpClient::new(endpoint, Duration::from_secs(10));
    let response = client.send(&request(b"ping")).expect("an answer");
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"hi");
    let received = server.join().expect("the server");
    assert!(
        received.starts_with("POST /query?db=x HTTP/1.1\r\n"),
        "{received}"
    );
    assert!(received.ends_with("\r\n\r\nping"), "{received}");
}

#[test]
fn the_limits_of_a_client_can_be_changed() {
    let (endpoint, server) = serve_once(OK.to_vec());
    let client = HttpClient::new(endpoint, Duration::from_secs(10)).with_limits(Limits {
        head: 1000,
        body: 1,
    });
    let err = client
        .send(&request(b""))
        .expect_err("a body that is too large");
    assert!(
        matches!(err, InfluxError::ResponseTooLarge { limit: 1 }),
        "{err}"
    );
    server.join().expect("the server");
}

#[test]
fn a_server_that_cannot_be_reached_is_reported() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
        listener.local_addr().expect("an address").port()
    };
    let client = HttpClient::new(
        endpoint(&format!("http://127.0.0.1:{port}")),
        Duration::from_secs(1),
    );
    let err = client.send(&request(b"")).expect_err("nobody listens");
    assert!(
        matches!(&err, InfluxError::Connect { address, .. } if *address == format!("127.0.0.1:{port}")),
        "{err}"
    );
    assert!(
        err.to_string()
            .starts_with(&format!("cannot connect to 127.0.0.1:{port}: ")),
        "{err}"
    );
}

#[test]
fn a_timeout_of_zero_is_refused() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("an address").port();
    let client = HttpClient::new(
        endpoint(&format!("http://127.0.0.1:{port}")),
        Duration::ZERO,
    );
    let err = client.send(&request(b"")).expect_err("a timeout of zero");
    assert!(
        matches!(
            &err,
            InfluxError::Io {
                context: "setting the timeout of the connection",
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn a_server_that_closes_the_connection_is_reported() {
    let (endpoint, server) = serve_once(Vec::new());
    let client = HttpClient::new(endpoint, Duration::from_secs(10));
    let err = client.send(&request(b"")).expect_err("no answer");
    assert!(
        matches!(
            err,
            InfluxError::MalformedResponse("the connection ended in the head")
        ),
        "{err}"
    );
    server.join().expect("the server");
}
