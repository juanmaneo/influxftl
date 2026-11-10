// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The failures of the InfluxDB 3 datastore. They stay typed down to the edge of the crate,
//! where the datastore turns them into the text a script reads.

use std::io;

use warpscript::finite::NotFinite;

/// A failure of the InfluxDB 3 datastore.
#[derive(Debug, thiserror::Error)]
pub enum InfluxError {
    /// A setting that is not UTF-8.
    #[error("{name} is not valid UTF-8")]
    NotUnicode {
        /// The name of the setting.
        name: &'static str,
    },
    /// A setting that must be given.
    #[error("{name} must be set to connect InfluxDB 3")]
    MissingSetting {
        /// The name of the setting.
        name: &'static str,
    },
    /// A token that cannot be sent in a header.
    #[error("the InfluxDB 3 token has a control character, which a header cannot carry")]
    InvalidToken,
    /// A URL that is not one `influxftl` can connect.
    #[error("invalid InfluxDB 3 URL '{url}': {reason}")]
    InvalidUrl {
        /// The URL.
        url: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// A server that could not be reached.
    #[error("cannot connect to {address}: {source}")]
    Connect {
        /// The address, `host:port`.
        address: String,
        /// Why.
        #[source]
        source: io::Error,
    },
    /// A connection that failed while it was used.
    #[error("{context}: {source}")]
    Io {
        /// What was being done.
        context: &'static str,
        /// Why it failed.
        #[source]
        source: io::Error,
    },
    /// An answer that is not HTTP.
    #[error("malformed HTTP response: {0}")]
    MalformedResponse(&'static str),
    /// An answer larger than what is read.
    #[error("the HTTP response is larger than {limit} bytes")]
    ResponseTooLarge {
        /// The limit.
        limit: usize,
    },
    /// A request InfluxDB 3 refused.
    #[error("InfluxDB 3 answered HTTP {status}: {message}")]
    Status {
        /// The status code.
        status: u16,
        /// What the server said.
        message: String,
    },
    /// An answer that is not the JSON that was asked for.
    #[error("InfluxDB 3 sent invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// An answer that is not a list of rows.
    #[error("InfluxDB 3 did not answer a list of rows")]
    NotRows,
    /// A column of a row that is missing or has another type than the one expected.
    #[error("the column '{column}' of a row of InfluxDB 3 is not {expected}")]
    BadColumn {
        /// The column.
        column: String,
        /// What it should be.
        expected: &'static str,
    },
    /// A name or a value that line protocol cannot carry.
    #[error(
        "{0:?} cannot be stored in InfluxDB 3: line protocol has no way to write a line break or a leading '#' in it"
    )]
    Unencodable(String),
    /// A label with the name of a column InfluxDB 3 datapoints use.
    #[error(
        "'{0}' cannot be the name of a label stored in InfluxDB 3: it is a column of the datapoints"
    )]
    ReservedLabel(String),
    /// A class with the name of the table that holds the attributes.
    #[error(
        "'{0}' cannot be the name of a class stored in InfluxDB 3: it is the table of the attributes"
    )]
    ReservedClass(String),
    /// A label without a value, which line protocol cannot write.
    #[error("the label '{0}' has an empty value, which cannot be stored in InfluxDB 3")]
    EmptyLabelValue(String),
    /// A tick whose nanoseconds do not fit 64 bits.
    #[error("the tick {0} is out of the range InfluxDB 3 stores")]
    TickOutOfRange(i64),
    /// A number that cannot be written in line protocol.
    #[error(transparent)]
    NotFinite(#[from] NotFinite),
    /// The system clock is before 1970 or after 2262.
    #[error("the system clock cannot be read as nanoseconds since the epoch")]
    Clock,
    /// A deletion that InfluxDB 3 Core cannot do: it deletes tables, not datapoints.
    #[error("InfluxDB 3 Core only deletes whole tables: {0}")]
    PartialDelete(String),
}
