// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The HTTP execution endpoint: `POST /api/v0/exec` runs the script of the body and answers the
//! stack it leaves as JSON (top first), like the endpoint of Warp 10 does; a script that fails
//! gets a `500` with the line and the message in `X-Warp10-Error-Line` and
//! `X-Warp10-Error-Message`. `GET /health` answers `200`.
//!
//! This is a deliberately small server (one request per connection, `Content-Length` bodies):
//! it serves the clients of a deployment, behind the authentication of InfluxDB 3.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use warpscript::datastore::Datastore;

use crate::exec::{self, ScriptError};

/// The largest head (request line and headers) of a request.
const MAX_HEAD: usize = 64 * 1024;
/// The largest script.
const MAX_BODY: usize = 64 * 1024 * 1024;

/// A connection: something to read a request from and to write the answer to.
pub trait Duplex: Read + Write {}

impl<T: Read + Write> Duplex for T {}

/// A request that cannot be served, with the status that tells so.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Rejection {
    /// A request line or a header that is not HTTP.
    #[error("malformed request")]
    Malformed,
    /// A head larger than [`MAX_HEAD`].
    #[error("request head too large")]
    HeadTooLarge,
    /// A body larger than [`MAX_BODY`].
    #[error("request body too large")]
    BodyTooLarge,
    /// A `Content-Length` that is not a number.
    #[error("invalid Content-Length '{0}'")]
    BadContentLength(String),
    /// A body in chunks, which is not supported.
    #[error("chunked requests are not supported: send a Content-Length")]
    Chunked,
    /// A path that is not served.
    #[error("not found")]
    NotFound,
    /// A method the path does not serve.
    #[error("method not allowed")]
    MethodNotAllowed,
    /// A script that is not UTF-8.
    #[error("the script is not UTF-8")]
    NotUtf8,
}

impl Rejection {
    /// The status code and the reason of the answer.
    fn status(&self) -> (u16, &'static str) {
        match self {
            Rejection::Malformed | Rejection::BadContentLength(_) | Rejection::NotUtf8 => {
                (400, "Bad Request")
            }
            Rejection::HeadTooLarge => (431, "Request Header Fields Too Large"),
            Rejection::BodyTooLarge => (413, "Payload Too Large"),
            Rejection::Chunked => (411, "Length Required"),
            Rejection::NotFound => (404, "Not Found"),
            Rejection::MethodNotAllowed => (405, "Method Not Allowed"),
        }
    }
}

/// A request read.
#[derive(Debug, PartialEq, Eq)]
struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

/// The failure of reading a request: the connection, or the request.
#[derive(Debug, thiserror::Error)]
enum ReadFailure {
    #[error("reading the request: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Rejected(#[from] Rejection),
}

/// Read one line of the head, which has `budget` bytes left.
fn read_line(reader: &mut dyn BufRead, budget: &mut usize) -> Result<String, ReadFailure> {
    let mut line = String::new();
    let read = reader.take(*budget as u64 + 1).read_line(&mut line)?;
    if read > *budget {
        return Err(Rejection::HeadTooLarge.into());
    }
    if !line.ends_with('\n') {
        // The connection ended in the middle of the head.
        return Err(Rejection::Malformed.into());
    }
    *budget -= read;
    Ok(line.trim_end().to_owned())
}

/// Read a request: the request line, the headers and the body of its `Content-Length`.
fn read_request(reader: &mut dyn BufRead) -> Result<Request, ReadFailure> {
    let mut budget = MAX_HEAD;
    let line = read_line(reader, &mut budget)?;
    let mut parts = line.split(' ');
    let (Some(method), Some(path), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Rejection::Malformed.into());
    };
    if !version.starts_with("HTTP/") {
        return Err(Rejection::Malformed.into());
    }
    let mut length = 0;
    loop {
        let header = read_line(reader, &mut budget)?;
        if header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            return Err(Rejection::Malformed.into());
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            length = value
                .parse::<usize>()
                .map_err(|_| Rejection::BadContentLength(value.to_owned()))?;
        } else if name.eq_ignore_ascii_case("transfer-encoding")
            && value.eq_ignore_ascii_case("chunked")
        {
            return Err(Rejection::Chunked.into());
        }
    }
    if length > MAX_BODY {
        return Err(Rejection::BodyTooLarge.into());
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Request {
        method: method.to_owned(),
        path: path.to_owned(),
        body,
    })
}

/// An answer: the status, the headers and the body.
struct Response {
    status: u16,
    reason: &'static str,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Response {
    fn text(status: u16, reason: &'static str, body: &str) -> Response {
        Response {
            status,
            reason,
            headers: vec![("Content-Type", "text/plain;charset=utf-8".to_owned())],
            body: body.as_bytes().to_vec(),
        }
    }

    fn write_to(&self, out: &mut dyn Write) -> io::Result<()> {
        write!(out, "HTTP/1.1 {} {}\r\n", self.status, self.reason)?;
        for (name, value) in &self.headers {
            write!(out, "{name}: {value}\r\n")?;
        }
        write!(
            out,
            "Content-Length: {}\r\nConnection: close\r\n\r\n",
            self.body.len()
        )?;
        out.write_all(&self.body)?;
        out.flush()
    }
}

/// The answer to a header value: a line.
fn header_value(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

/// Run the script of a request and answer.
fn exec(script: &[u8], datastore: Option<Arc<dyn Datastore>>) -> Result<Response, Rejection> {
    let script = std::str::from_utf8(script).map_err(|_| Rejection::NotUtf8)?;
    let start = Instant::now();
    Ok(match exec::execute(script, datastore) {
        Ok(execution) => Response {
            status: 200,
            reason: "OK",
            headers: vec![
                ("Content-Type", "application/json;charset=utf-8".to_owned()),
                ("X-Warp10-Elapsed", start.elapsed().as_nanos().to_string()),
                ("X-Warp10-Ops", execution.ops.to_string()),
            ],
            body: execution.json.into_bytes(),
        },
        Err(ScriptError { line, message }) => {
            let mut response = Response::text(500, "Internal Server Error", &message);
            response
                .headers
                .push(("X-Warp10-Error-Message", header_value(&message)));
            if let Some(line) = line {
                response
                    .headers
                    .push(("X-Warp10-Error-Line", line.to_string()));
            }
            response
        }
    })
}

/// The answer to a request.
fn route(request: &Request, datastore: Option<Arc<dyn Datastore>>) -> Result<Response, Rejection> {
    match (request.method.as_str(), request.path.as_str()) {
        ("POST", "/api/v0/exec") => exec(&request.body, datastore),
        ("GET", "/health") => Ok(Response::text(200, "OK", "ok")),
        (_, "/api/v0/exec" | "/health") => Err(Rejection::MethodNotAllowed),
        _ => Err(Rejection::NotFound),
    }
}

/// Serve the request of a connection.
///
/// # Errors
///
/// The connection failed: the request ended early or the answer could not be written.
pub fn handle(
    connection: &mut dyn Duplex,
    datastore: Option<Arc<dyn Datastore>>,
) -> io::Result<()> {
    let request = {
        let mut reader = BufReader::new(&mut *connection);
        read_request(&mut reader)
    };
    let response = match request {
        Err(ReadFailure::Io(err)) => return Err(err),
        Err(ReadFailure::Rejected(rejection)) => rejection_response(&rejection),
        Ok(request) => {
            route(&request, datastore).unwrap_or_else(|rejection| rejection_response(&rejection))
        }
    };
    response.write_to(connection)
}

/// The answer to a request that cannot be served.
fn rejection_response(rejection: &Rejection) -> Response {
    let (status, reason) = rejection.status();
    Response::text(status, reason, &rejection.to_string())
}

/// Serve the connections of a listener, a thread for each one, `limit` of them or forever.
///
/// # Errors
///
/// The listener failed to accept a connection.
pub fn serve(
    listener: &TcpListener,
    limit: Option<usize>,
    datastore: Option<&Arc<dyn Datastore>>,
) -> io::Result<()> {
    serve_connections(&mut listener.incoming(), limit, datastore)
}

/// Serve connections, a thread for each one, `limit` of them or all of them.
///
/// # Errors
///
/// A connection that could not be accepted.
pub fn serve_connections(
    connections: &mut dyn Iterator<Item = io::Result<TcpStream>>,
    limit: Option<usize>,
    datastore: Option<&Arc<dyn Datastore>>,
) -> io::Result<()> {
    let mut handlers = Vec::new();
    for (served, connection) in connections.enumerate() {
        let mut connection = connection?;
        let datastore = datastore.cloned();
        handlers.push(thread::spawn(move || {
            // A client that goes away is not the concern of the server.
            if let Err(err) = handle(&mut connection, datastore) {
                eprintln!("influxftl: {err}");
            }
        }));
        if limit.is_some_and(|limit| served + 1 >= limit) {
            break;
        }
    }
    for handler in handlers {
        // A handler that panicked has already reported on stderr.
        drop(handler.join());
    }
    Ok(())
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
