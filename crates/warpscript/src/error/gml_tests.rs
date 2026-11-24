// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The runtime failures of the GML and KML readers, their classes, and the messages and causes
//! that `Error` gives them.

use super::GmlError;
use crate::error::compat::exception_class;
use crate::error::{Class, Error};

fn sax(message: &str, cause: Option<Error>) -> GmlError {
    GmlError::Sax {
        message: message.to_owned(),
        cause: cause.map(Box::new),
    }
}

#[test]
fn the_failures_of_a_document_are_not_runtime() {
    assert!(!GmlError::Xml("bad".to_owned()).is_runtime());
    assert!(!sax("refused", None).is_runtime());
    assert!(!Error::from(GmlError::Xml("bad".to_owned())).is_runtime());
}

#[test]
fn the_failures_of_the_builders_are_runtime() {
    assert!(GmlError::Illegal("dimension must be <= 3").is_runtime());
    assert!(
        GmlError::Cast {
            from: Class::Point,
            to: Class::Geometry,
        }
        .is_runtime()
    );
    assert!(GmlError::Store(Class::Point).is_runtime());
    assert!(GmlError::Assertion.is_runtime());
    assert!(Error::from(GmlError::Assertion).is_runtime());
}

#[test]
fn a_sax_failure_is_given_with_its_cause() {
    let error = Error::from(sax("refused", Some(Error::NullPointer)));
    assert_eq!(error.message(), "refused");
    assert_eq!(error.cause(), Some(&Error::NullPointer));
}

#[test]
fn a_sax_failure_without_a_cause_has_none() {
    let error = Error::from(sax("refused", None));
    assert_eq!(error.message(), "refused");
    assert_eq!(error.cause(), None);
}

#[test]
fn each_gml_failure_has_its_java_class() {
    let classes = [
        (
            GmlError::Xml("bad".to_owned()),
            "org.xml.sax.SAXParseException",
        ),
        (sax("refused", None), "org.xml.sax.SAXException"),
        (
            GmlError::Illegal("dimension must be <= 3"),
            "java.lang.IllegalArgumentException",
        ),
        (
            GmlError::Cast {
                from: Class::Point,
                to: Class::Geometry,
            },
            "java.lang.ClassCastException",
        ),
        (
            GmlError::Store(Class::Point),
            "java.lang.ArrayStoreException",
        ),
        (
            GmlError::Assertion,
            "com.vividsolutions.jts.util.AssertionFailedException",
        ),
    ];
    for (error, class) in classes {
        assert_eq!(exception_class(&Error::from(error)), class);
    }
}
