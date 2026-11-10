// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! A server on the loopback interface, which answers one connection with what it is given and
//! hands the request it received to the test.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};

use super::client::Endpoint;

/// An answer with a status (`200 OK`) and a body.
pub(crate) fn http(status: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/// The endpoint of a server that answers one connection with `response`, and the request it
/// got, as text, once the test joins it.
pub(crate) fn serve_once(response: Vec<u8>) -> (Endpoint, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("an address").port();
    let server = thread::spawn(move || {
        let (mut connection, _) = listener.accept().expect("a connection");
        let mut received = Vec::new();
        let mut chunk = [0; 1024];
        let (head_end, length) = loop {
            let read = connection
                .read(&mut chunk)
                .expect("the request should be read");
            assert!(read > 0, "the client closed the connection");
            received.extend_from_slice(&chunk[..read]);
            if let Some(end) = received.windows(4).position(|window| window == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&received[..end]).to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .map_or(0, |length| length.parse::<usize>().expect("a length"));
                break (end + 4, length);
            }
        };
        while received.len() < head_end + length {
            let read = connection
                .read(&mut chunk)
                .expect("the body should be read");
            assert!(read > 0, "the client closed the connection");
            received.extend_from_slice(&chunk[..read]);
        }
        connection
            .write_all(&response)
            .expect("the answer should be sent");
        String::from_utf8(received).expect("a request in UTF-8")
    });
    let endpoint = Endpoint::parse(&format!("http://127.0.0.1:{port}")).expect("a URL");
    (endpoint, server)
}
