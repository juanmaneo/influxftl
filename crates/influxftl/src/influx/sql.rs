// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The pieces of the SQL the datastore sends: every name and every text of a script goes
//! through these, so that none can change the query.

/// An identifier (a table or a column), quoted.
pub fn identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// A text literal, quoted.
pub fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// A timestamp literal of `nanoseconds` since the epoch.
pub fn timestamp(nanoseconds: i64) -> String {
    format!("arrow_cast({nanoseconds}, 'Timestamp(Nanosecond, None)')")
}

/// A tick (in microseconds) as nanoseconds, or `None` when they do not fit 64 bits.
pub fn nanoseconds(tick: i64) -> Option<i64> {
    tick.checked_mul(1000)
}

#[cfg(test)]
#[path = "sql_tests.rs"]
mod sql_tests;
