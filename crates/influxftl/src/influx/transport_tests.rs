// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the transport to InfluxDB 3, against a server on the loopback interface.

use std::net::TcpListener;
use std::time::Duration;

use serde_json::{Value, json};

use super::super::client::{Endpoint, HttpClient};
use super::super::fake_server_tests::{http, serve_once};
use super::{HttpTransport, InfluxError, Transport, parse_rows};

fn transport_to(endpoint: Endpoint, token: Option<&str>) -> HttpTransport {
    HttpTransport::new(
        HttpClient::new(endpoint, Duration::from_secs(10)),
        "my db".to_owned(),
        token.map(str::to_owned),
    )
    .expect("a transport")
}

/// The head of a request (with the line break of its last header) and its body.
fn split(request: &str) -> (&str, &str) {
    let (head, body) = request
        .split_once("\r\n\r\n")
        .expect("a request has a head and a body");
    (&request[..head.len() + 2], body)
}

#[test]
fn the_rows_of_an_answer_are_read() {
    let rows = parse_rows(br#"[{"a":1,"b":"x"},{"c":true}]"#).expect("rows");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["a"], json!(1));
    assert_eq!(rows[1]["c"], json!(true));
    assert_eq!(parse_rows(b"[]").expect("no rows").len(), 0);
}

#[test]
fn an_answer_that_is_not_rows_is_refused() {
    assert!(matches!(parse_rows(b"{}"), Err(InfluxError::NotRows)));
    assert!(matches!(parse_rows(b"[1]"), Err(InfluxError::NotRows)));
    assert!(matches!(parse_rows(b"[{},[]]"), Err(InfluxError::NotRows)));
    assert!(matches!(parse_rows(b"\"x\""), Err(InfluxError::NotRows)));
    let err = parse_rows(b"not json").expect_err("not JSON");
    assert!(matches!(err, InfluxError::Json(_)), "{err}");
    assert!(
        err.to_string()
            .starts_with("InfluxDB 3 sent invalid JSON: "),
        "{err}"
    );
}

#[test]
fn a_token_with_a_control_character_is_refused() {
    let endpoint = Endpoint::parse("http://db").expect("a URL");
    for token in ["a\nb", "a\rb", "a\0b", "a\tb"] {
        let err = HttpTransport::new(
            HttpClient::new(endpoint.clone(), Duration::from_secs(1)),
            "d".to_owned(),
            Some(token.to_owned()),
        )
        .expect_err("a token with a control character");
        assert!(matches!(err, InfluxError::InvalidToken), "{token:?}");
    }
    for token in [None, Some("abc.def-ghi")] {
        HttpTransport::new(
            HttpClient::new(endpoint.clone(), Duration::from_secs(1)),
            "d".to_owned(),
            token.map(str::to_owned),
        )
        .expect("a transport");
    }
}

#[test]
fn a_query_is_posted_as_json() {
    let (endpoint, server) = serve_once(http("200 OK", r#"[{"n":1}]"#));
    let rows = transport_to(endpoint, Some("s3cret"))
        .query("SELECT 'a' FROM \"t\"")
        .expect("rows");
    assert_eq!(rows[0]["n"], json!(1));
    let request = server.join().expect("the server");
    let (head, body) = split(&request);
    assert!(
        head.starts_with("POST /api/v3/query_sql HTTP/1.1\r\n"),
        "{head}"
    );
    assert!(
        head.contains("\r\nAuthorization: Bearer s3cret\r\n"),
        "{head}"
    );
    assert!(
        head.contains("\r\nContent-Type: application/json\r\n"),
        "{head}"
    );
    let body: Value = serde_json::from_str(body).expect("a JSON body");
    assert_eq!(
        body,
        json!({"db": "my db", "q": "SELECT 'a' FROM \"t\"", "format": "json"})
    );
}

#[test]
fn a_query_without_a_token_has_no_authorization() {
    let (endpoint, server) = serve_once(http("200 OK", "[]"));
    assert_eq!(
        transport_to(endpoint, None)
            .query("SELECT 1")
            .expect("rows")
            .len(),
        0
    );
    let request = server.join().expect("the server");
    assert!(!request.contains("Authorization"), "{request}");
}

#[test]
fn a_query_that_is_refused_is_reported() {
    let (endpoint, server) = serve_once(http("400 Bad Request", "no such table\n"));
    let err = transport_to(endpoint, None)
        .query("SELECT 1")
        .expect_err("refused");
    assert!(
        matches!(&err, InfluxError::Status { status: 400, message } if message == "no such table"),
        "{err}"
    );
    assert_eq!(
        err.to_string(),
        "InfluxDB 3 answered HTTP 400: no such table"
    );
    server.join().expect("the server");
}

#[test]
fn an_answer_to_a_query_that_is_not_rows_is_reported() {
    let (endpoint, server) = serve_once(http("200 OK", "{\"error\":1}"));
    let err = transport_to(endpoint, None)
        .query("SELECT 1")
        .expect_err("not rows");
    assert!(matches!(err, InfluxError::NotRows), "{err}");
    server.join().expect("the server");
}

#[test]
fn a_server_that_cannot_be_reached_is_reported() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
        listener.local_addr().expect("an address").port()
    };
    let endpoint = Endpoint::parse(&format!("http://127.0.0.1:{port}")).expect("a URL");
    let err = transport_to(endpoint, None)
        .query("SELECT 1")
        .expect_err("nobody listens");
    assert!(matches!(err, InfluxError::Connect { .. }), "{err}");
}

#[test]
fn lines_are_written_with_nanosecond_timestamps() {
    let (endpoint, server) = serve_once(http("204 No Content", ""));
    transport_to(endpoint, Some("t"))
        .write("cpu value=1i 1\ncpu value=2i 2")
        .expect("a write");
    let request = server.join().expect("the server");
    let (head, body) = split(&request);
    assert!(
        head.starts_with("POST /api/v3/write_lp?db=my%20db&precision=nanosecond HTTP/1.1\r\n"),
        "{head}"
    );
    assert!(
        head.contains("\r\nContent-Type: text/plain; charset=utf-8\r\n"),
        "{head}"
    );
    assert_eq!(body, "cpu value=1i 1\ncpu value=2i 2");
}

#[test]
fn lines_that_are_refused_are_reported() {
    let (endpoint, server) = serve_once(http("400 Bad Request", "partial write"));
    let err = transport_to(endpoint, None)
        .write("x")
        .expect_err("refused");
    assert!(
        matches!(err, InfluxError::Status { status: 400, .. }),
        "{err}"
    );
    server.join().expect("the server");
}

#[test]
fn a_table_is_deleted_by_its_name() {
    let (endpoint, server) = serve_once(http("200 OK", ""));
    transport_to(endpoint, None)
        .delete_table("a b&c")
        .expect("a deletion");
    let request = server.join().expect("the server");
    let (head, body) = split(&request);
    assert!(
        head.starts_with("DELETE /api/v3/configure/table?db=my%20db&table=a%20b%26c HTTP/1.1\r\n"),
        "{head}"
    );
    assert!(!head.contains("Content-Type"), "{head}");
    assert_eq!(body, "");
}

#[test]
fn a_table_that_does_not_exist_is_reported() {
    let (endpoint, server) = serve_once(http("404 Not Found", "table not found"));
    let err = transport_to(endpoint, None)
        .delete_table("t")
        .expect_err("no table");
    assert!(
        matches!(err, InfluxError::Status { status: 404, .. }),
        "{err}"
    );
    server.join().expect("the server");
}
