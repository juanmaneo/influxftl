// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The InfluxDB 3 datastore: the HTTP client ([`client`]), the operations of InfluxDB 3 the
//! datastore uses ([`transport`]), the layout of Warp 10 concepts in tables ([`schema`]), SQL
//! and line protocol ([`sql`], [`lineprotocol`]) and the [`InfluxDatastore`] that ties them.

pub mod client;
pub mod error;
pub mod lineprotocol;
pub mod schema;
pub mod sql;
pub mod store;
pub mod transport;

use std::ffi::OsString;
use std::time::Duration;

use self::client::{Endpoint, HttpClient};
pub use self::error::InfluxError;
pub use self::store::InfluxDatastore;
use self::transport::HttpTransport;

/// The environment variable of the URL of InfluxDB 3 (the one of the `influxdb3` command).
pub const HOST_VARIABLE: &str = "INFLUXDB3_HOST_URL";
/// The environment variable of the database.
pub const DATABASE_VARIABLE: &str = "INFLUXDB3_DATABASE_NAME";
/// The environment variable of the token.
pub const TOKEN_VARIABLE: &str = "INFLUXDB3_AUTH_TOKEN";
/// The URL of InfluxDB 3 when none is given.
pub const DEFAULT_HOST: &str = "http://127.0.0.1:8181";
/// The time a connection may be silent before the datastore gives up on it.
pub const TIMEOUT: Duration = Duration::from_secs(120);

/// What a setting of the environment is: nothing, or text.
fn setting(
    lookup: &dyn Fn(&str) -> Option<OsString>,
    name: &'static str,
) -> Result<Option<String>, InfluxError> {
    lookup(name)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| InfluxError::NotUnicode { name })
        })
        .transpose()
}

/// Connect to InfluxDB 3 as the settings of the environment say: the database is required,
/// the URL (`http://127.0.0.1:8181`) and the token are not.
///
/// # Errors
///
/// A setting that is not UTF-8 or not valid, a database that is not given.
pub fn connect(
    lookup: &dyn Fn(&str) -> Option<OsString>,
    timeout: Duration,
) -> Result<InfluxDatastore, InfluxError> {
    let host = setting(lookup, HOST_VARIABLE)?.unwrap_or_else(|| DEFAULT_HOST.to_owned());
    let database = setting(lookup, DATABASE_VARIABLE)?.ok_or(InfluxError::MissingSetting {
        name: DATABASE_VARIABLE,
    })?;
    let token = setting(lookup, TOKEN_VARIABLE)?;
    let client = HttpClient::new(Endpoint::parse(&host)?, timeout);
    let transport = HttpTransport::new(client, database, token)?;
    Ok(InfluxDatastore::new(Box::new(transport)))
}

#[cfg(test)]
#[path = "fake_server_tests.rs"]
mod fake_server_tests;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
