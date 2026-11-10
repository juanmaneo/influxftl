// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! What the datastore needs of InfluxDB 3: SQL queries, line protocol writes and the deletion
//! of a table, over the HTTP API (`/api/v3/query_sql`, `/api/v3/write_lp`,
//! `/api/v3/configure/table`).

use serde_json::{Map, Value, json};
use warpscript::format::url_encode;

use super::client::{HttpClient, Request, Response};
use super::error::InfluxError;

/// A row of the result of a query: its columns, without those that are null.
pub type Row = Map<String, Value>;

/// The operations of InfluxDB 3 the datastore uses.
pub trait Transport: std::fmt::Debug + Send + Sync {
    /// Run a SQL query.
    ///
    /// # Errors
    ///
    /// A server that cannot be reached, or that refuses the query.
    fn query(&self, sql: &str) -> Result<Vec<Row>, InfluxError>;

    /// Write line protocol, with nanosecond timestamps.
    ///
    /// # Errors
    ///
    /// A server that cannot be reached, or that refuses the lines.
    fn write(&self, line_protocol: &str) -> Result<(), InfluxError>;

    /// Delete a table with all its data.
    ///
    /// # Errors
    ///
    /// A server that cannot be reached, or that refuses the deletion.
    fn delete_table(&self, table: &str) -> Result<(), InfluxError>;
}

/// The rows of an answer to a query.
///
/// # Errors
///
/// A body that is not JSON, or not a list of objects.
pub fn parse_rows(body: &[u8]) -> Result<Vec<Row>, InfluxError> {
    match serde_json::from_slice(body)? {
        Value::Array(items) => items
            .into_iter()
            .map(|item| match item {
                Value::Object(row) => Ok(row),
                _ => Err(InfluxError::NotRows),
            })
            .collect(),
        _ => Err(InfluxError::NotRows),
    }
}

/// InfluxDB 3 over HTTP.
#[derive(Debug)]
pub struct HttpTransport {
    client: HttpClient,
    database: String,
    token: Option<String>,
}

impl HttpTransport {
    /// A transport to a database of a server.
    ///
    /// # Errors
    ///
    /// A token that a header cannot carry.
    pub fn new(
        client: HttpClient,
        database: String,
        token: Option<String>,
    ) -> Result<HttpTransport, InfluxError> {
        if token
            .as_deref()
            .is_some_and(|t| t.chars().any(char::is_control))
        {
            return Err(InfluxError::InvalidToken);
        }
        Ok(HttpTransport {
            client,
            database,
            token,
        })
    }

    /// Send a request, which InfluxDB 3 must answer with a success.
    fn call(
        &self,
        method: &str,
        path: &str,
        content_type: Option<&str>,
        body: &[u8],
    ) -> Result<Response, InfluxError> {
        let authorization = self.token.as_ref().map(|token| format!("Bearer {token}"));
        let mut headers = Vec::new();
        if let Some(content_type) = content_type {
            headers.push(("Content-Type", content_type));
        }
        if let Some(authorization) = &authorization {
            headers.push(("Authorization", authorization.as_str()));
        }
        let response = self.client.send(&Request {
            method,
            path,
            headers: &headers,
            body,
        })?;
        if (200..300).contains(&response.status) {
            Ok(response)
        } else {
            Err(InfluxError::Status {
                status: response.status,
                message: String::from_utf8_lossy(&response.body).trim().to_owned(),
            })
        }
    }
}

impl Transport for HttpTransport {
    fn query(&self, sql: &str) -> Result<Vec<Row>, InfluxError> {
        let request = json!({ "db": self.database, "q": sql, "format": "json" }).to_string();
        let response = self.call(
            "POST",
            "/api/v3/query_sql",
            Some("application/json"),
            request.as_bytes(),
        )?;
        parse_rows(&response.body)
    }

    fn write(&self, line_protocol: &str) -> Result<(), InfluxError> {
        let path = format!(
            "/api/v3/write_lp?db={}&precision=nanosecond",
            url_encode(&self.database)
        );
        self.call(
            "POST",
            &path,
            Some("text/plain; charset=utf-8"),
            line_protocol.as_bytes(),
        )
        .map(drop)
    }

    fn delete_table(&self, table: &str) -> Result<(), InfluxError> {
        let path = format!(
            "/api/v3/configure/table?db={}&table={}",
            url_encode(&self.database),
            url_encode(table)
        );
        self.call("DELETE", &path, None, &[]).map(drop)
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod transport_tests;
