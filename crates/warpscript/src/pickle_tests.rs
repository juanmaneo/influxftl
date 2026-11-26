// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

use indexmap::IndexMap;

use super::dumps;
use crate::value::Value;
use crate::value_tests::processing_objects;

#[test]
fn processing_objects_are_pickled_as_the_dictionary_of_their_class() {
    for (value, _, _, class) in processing_objects() {
        let bean = Value::map(IndexMap::from([(
            Value::string("__class__"),
            Value::string(class),
        )]));
        let pickled = dumps(&value).expect("a bean pickles");
        assert_eq!(pickled, dumps(&bean).expect("a map pickles"), "{class}");
        assert!(String::from_utf8_lossy(&pickled).contains(class), "{class}");
    }
}
