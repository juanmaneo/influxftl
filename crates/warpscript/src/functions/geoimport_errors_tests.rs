// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The `<` texts that `->WKB` reads as GML or KML, and the failures Warp 10 reports for them:
//! a root that is not a geometry is a `NullPointerException`, and truncated XML is a SAX error.
//! The more detailed cases are in `geoimport_gml_tests`.

use crate::error::compat::exception_class;
use crate::tests::fail;

#[test]
fn kml_root_that_is_not_a_geometry_is_a_null_pointer() {
    let error = fail("'<kml/>' ->WKB");
    assert_eq!(
        exception_class(error.root_cause()),
        "java.lang.NullPointerException"
    );
}

#[test]
fn truncated_xml_is_a_sax_parse_error() {
    let error = fail("'<' ->WKB");
    assert_eq!(
        exception_class(error.root_cause()),
        "org.xml.sax.SAXParseException"
    );
}
