// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! A small HTTP/1.1 client: one request per connection, over plain TCP. It speaks to
//! InfluxDB 3 on a network the deployment trusts or through a TLS terminating proxy; `https://`
//! URLs are refused rather than silently downgraded.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use super::error::InfluxError;

/// What is read of an answer, at most.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The size of the head: the status line and the headers.
    pub head: usize,
    /// The size of the body.
    pub body: usize,
}

impl Limits {
    /// The limits of a client: 64 KiB of head, 1 GiB of body.
    pub const DEFAULT: Limits = Limits {
        head: 64 * 1024,
        body: 1 << 30,
    };
}

/// Where a server listens: `http://host[:port][/prefix]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    host: String,
    port: u16,
    prefix: String,
}

impl Endpoint {
    /// Read a URL.
    ///
    /// # Errors
    ///
    /// A URL that does not start with `http://`, has no host or has a port that is not one.
    pub fn parse(url: &str) -> Result<Endpoint, InfluxError> {
        let invalid = |reason| InfluxError::InvalidUrl {
            url: url.to_owned(),
            reason,
        };
        let Some(rest) = url.strip_prefix("http://") else {
            return Err(invalid(if url.starts_with("https://") {
                "https is not supported: connect through a TLS terminating proxy, or use http"
            } else {
                "expected http://host[:port]"
            }));
        };
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        // The colons of an IPv6 address (`[::1]`) are not those of a port.
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) if !port.contains(']') => {
                (host, port.parse().map_err(|_| invalid("invalid port"))?)
            }
            _ => (authority, 80),
        };
        if host.is_empty() {
            return Err(invalid("missing host"));
        }
        let prefix = match path.trim_end_matches('/') {
            "" => String::new(),
            path => format!("/{path}"),
        };
        Ok(Endpoint {
            host: host.to_owned(),
            port,
            prefix,
        })
    }

    /// The `host:port` to connect.
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// An answer: its status and its body.
#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    /// The status code.
    pub status: u16,
    /// The body.
    pub body: Vec<u8>,
}

/// A connection: something to write a request to and to read the answer from.
pub trait Duplex: Read + Write {}

impl<T: Read + Write> Duplex for T {}

fn io_error(context: &'static str) -> impl FnOnce(io::Error) -> InfluxError {
    move |source| InfluxError::Io { context, source }
}

/// What a request is made of.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    /// The method.
    pub method: &'a str,
    /// The path and the query, after the prefix of the endpoint.
    pub path: &'a str,
    /// The headers, besides those of the connection.
    pub headers: &'a [(&'a str, &'a str)],
    /// The body.
    pub body: &'a [u8],
}

/// A client of an endpoint.
#[derive(Clone, Debug)]
pub struct HttpClient {
    endpoint: Endpoint,
    timeout: Duration,
    limits: Limits,
}

impl HttpClient {
    /// A client that gives up on a connection that is silent for `timeout`.
    pub fn new(endpoint: Endpoint, timeout: Duration) -> HttpClient {
        HttpClient {
            endpoint,
            timeout,
            limits: Limits::DEFAULT,
        }
    }

    /// The same client, reading answers up to other limits.
    #[must_use]
    pub fn with_limits(self, limits: Limits) -> HttpClient {
        HttpClient { limits, ..self }
    }

    /// Send a request and read the answer.
    ///
    /// # Errors
    ///
    /// The server cannot be reached, the connection fails or the answer is not HTTP.
    pub fn send(&self, request: &Request<'_>) -> Result<Response, InfluxError> {
        let address = self.endpoint.address();
        let mut stream = TcpStream::connect(&address).map_err(|source| InfluxError::Connect {
            address: address.clone(),
            source,
        })?;
        set_timeouts(&stream, self.timeout)
            .map_err(io_error("setting the timeout of the connection"))?;
        exchange(&mut stream, &self.endpoint, request, &self.limits)
    }
}

/// Give up on a connection that is silent for `timeout`. A timeout of zero is refused.
fn set_timeouts(stream: &TcpStream, timeout: Duration) -> io::Result<()> {
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))
}

/// Write a request to a connection and read the answer.
///
/// # Errors
///
/// The connection fails or the answer is not HTTP.
pub fn exchange(
    connection: &mut dyn Duplex,
    endpoint: &Endpoint,
    request: &Request<'_>,
    limits: &Limits,
) -> Result<Response, InfluxError> {
    let mut head = format!(
        "{} {}{} HTTP/1.1\r\nHost: {}\r\nUser-Agent: influxftl/{}\r\nConnection: close\r\nContent-Length: {}\r\n",
        request.method,
        endpoint.prefix,
        request.path,
        endpoint.address(),
        env!("CARGO_PKG_VERSION"),
        request.body.len()
    );
    for (name, value) in request.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    connection
        .write_all(head.as_bytes())
        .map_err(io_error("sending the request"))?;
    connection
        .write_all(request.body)
        .map_err(io_error("sending the request"))?;
    connection
        .flush()
        .map_err(io_error("sending the request"))?;
    read_response(&mut BufReader::new(connection), limits)
}

/// Read one line of the head, which has `budget` bytes left, without its line break.
fn read_line(reader: &mut dyn BufRead, budget: &mut usize) -> Result<String, InfluxError> {
    let mut line = String::new();
    let read = reader
        .take(*budget as u64 + 1)
        .read_line(&mut line)
        .map_err(io_error("reading the response"))?;
    if read > *budget {
        return Err(InfluxError::MalformedResponse("the head is too large"));
    }
    if !line.ends_with('\n') {
        return Err(InfluxError::MalformedResponse(
            "the connection ended in the head",
        ));
    }
    *budget -= read;
    Ok(line.trim_end().to_owned())
}

/// The status of a status line (`HTTP/1.1 200 OK`).
fn parse_status(line: &str) -> Result<u16, InfluxError> {
    let mut parts = line.split(' ');
    match (parts.next(), parts.next()) {
        (Some(version), Some(status)) if version.starts_with("HTTP/") => status
            .parse()
            .map_err(|_| InfluxError::MalformedResponse("the status is not a number")),
        _ => Err(InfluxError::MalformedResponse("not a status line")),
    }
}

/// How the body of an answer is delimited.
enum Framing {
    Length(usize),
    Chunked,
    Close,
}

/// Read an answer: the status line, the headers and the body.
///
/// # Errors
///
/// A connection that fails, an answer that is not HTTP or a body that is too large.
pub fn read_response(reader: &mut dyn BufRead, limits: &Limits) -> Result<Response, InfluxError> {
    let mut budget = limits.head;
    let status = parse_status(&read_line(reader, &mut budget)?)?;
    let mut framing = Framing::Close;
    loop {
        let header = read_line(reader, &mut budget)?;
        if header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            return Err(InfluxError::MalformedResponse("a header without a colon"));
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            let length = value.parse().map_err(|_| {
                InfluxError::MalformedResponse("the Content-Length is not a number")
            })?;
            framing = Framing::Length(length);
        } else if name.eq_ignore_ascii_case("transfer-encoding")
            && value.eq_ignore_ascii_case("chunked")
        {
            framing = Framing::Chunked;
        }
    }
    // These answers have no body, whatever their headers say.
    let body = if status == 204 || status == 304 || status / 100 == 1 {
        Vec::new()
    } else {
        match framing {
            Framing::Length(length) => read_exact_bounded(reader, length, limits.body)?,
            Framing::Chunked => read_chunks(reader, &mut budget, limits.body)?,
            Framing::Close => read_to_close(reader, limits.body)?,
        }
    };
    Ok(Response { status, body })
}

fn too_large(limit: usize) -> InfluxError {
    InfluxError::ResponseTooLarge { limit }
}

/// A body of `length` bytes.
fn read_exact_bounded(
    reader: &mut dyn BufRead,
    length: usize,
    limit: usize,
) -> Result<Vec<u8>, InfluxError> {
    if length > limit {
        return Err(too_large(limit));
    }
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .map_err(io_error("reading the response"))?;
    Ok(body)
}

/// A body that ends when the connection does.
fn read_to_close(reader: &mut dyn BufRead, limit: usize) -> Result<Vec<u8>, InfluxError> {
    let mut body = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut body)
        .map_err(io_error("reading the response"))?;
    if body.len() > limit {
        return Err(too_large(limit));
    }
    Ok(body)
}

/// A body in chunks (`Transfer-Encoding: chunked`), followed by its trailers.
fn read_chunks(
    reader: &mut dyn BufRead,
    budget: &mut usize,
    limit: usize,
) -> Result<Vec<u8>, InfluxError> {
    let mut body = Vec::new();
    loop {
        let line = read_line(reader, budget)?;
        // The size is in hexadecimal, followed by extensions after a semicolon.
        let size = line
            .split_once(';')
            .map_or(line.as_str(), |(size, _)| size)
            .trim();
        let size = usize::from_str_radix(size, 16)
            .map_err(|_| InfluxError::MalformedResponse("a chunk size is not hexadecimal"))?;
        if size == 0 {
            // The trailers end with an empty line.
            while !read_line(reader, budget)?.is_empty() {}
            return Ok(body);
        }
        if body.len().saturating_add(size) > limit {
            return Err(too_large(limit));
        }
        let start = body.len();
        body.resize(start + size, 0);
        reader
            .read_exact(&mut body[start..])
            .map_err(io_error("reading the response"))?;
        if !read_line(reader, budget)?.is_empty() {
            return Err(InfluxError::MalformedResponse(
                "a chunk is not followed by a line break",
            ));
        }
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod client_tests;
