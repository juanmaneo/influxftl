// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the GML and KML readers (`GEO.GML`, `GEO.KML`, and `->WKT`… of a GML or a
//! KML text), worded like the SAX parser and the JTS builders that the readers of Warp 10 run
//! on: a script reads them in the cause chain of the failure of the function.

use super::compat;
use super::{Class, Error};

/// The tail of a SAX failure that has a cause, ` (cause)`, or nothing without one.
fn caused(cause: Option<&Error>) -> String {
    cause.map_or_else(String::new, |cause| format!(" ({cause})"))
}

/// A failure of the GML or KML reader.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GmlError {
    /// A document that is not well-formed XML (`SAXParseException`).
    #[error("{0}")]
    Xml(String),
    /// An element that the handler of the reader refuses (`SAXException`), with the failure
    /// that caused it when there is one.
    #[error("{message}{}", caused(cause.as_deref()))]
    Sax {
        /// The message of the handler.
        message: String,
        /// The failure the handler was given.
        cause: Option<Box<Error>>,
    },
    /// A geometry that the JTS builder refuses for an argument (`IllegalArgumentException`).
    #[error("{0}")]
    Illegal(&'static str),
    /// A value of a class where another class is expected (`ClassCastException`).
    #[error(
        "{} cannot be cast to {}",
        compat::class_name(*from),
        compat::class_name(*to)
    )]
    Cast {
        /// The class of the value.
        from: Class,
        /// The class that is expected.
        to: Class,
    },
    /// A geometry put in an array of another class (`ArrayStoreException`), whose message is
    /// the class of the geometry.
    #[error("{}", compat::class_name(*.0))]
    Store(Class),
    /// A coordinate sequence the JTS builder cannot make a geometry of
    /// (`AssertionFailedException`).
    #[error("AssertionFailedException")]
    Assertion,
}

impl GmlError {
    /// `true` for the failures that are defects of the code rather than of the script (Warp 10
    /// raises them as runtime exceptions).
    pub fn is_runtime(&self) -> bool {
        match self {
            GmlError::Xml(_) | GmlError::Sax { .. } => false,
            GmlError::Illegal(_)
            | GmlError::Cast { .. }
            | GmlError::Store(_)
            | GmlError::Assertion => true,
        }
    }
}

#[cfg(test)]
#[path = "gml_tests.rs"]
mod tests;
