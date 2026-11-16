// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The geometry texts the writers do not read yet: GML and KML (the `<` texts), which Warp 10
//! reads with its XML handlers (the J2 step of the JTS port, see `docs/PORTING.md`).

use crate::error::compat::exception_class;
use crate::tests::fail;

#[test]
fn gml_and_kml_texts_are_not_read_yet() {
    let error = fail("'<kml/>' ->WKB");
    assert_eq!(
        exception_class(error.root_cause()),
        "java.lang.UnsupportedOperationException"
    );
}
