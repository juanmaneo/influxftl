// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The states of the tokenizer that no text reaches: the content of a document with no element
//! open, and the end tag of such a document. The tokenizer keeps them as premature ends, so
//! they are driven from their state directly.

use super::{PREMATURE_END, Phase, Tokens};
use crate::error::Error;
use crate::error::gml::GmlError;

fn premature_end() -> Error {
    GmlError::Xml(PREMATURE_END.to_owned()).into()
}

#[test]
fn content_with_no_element_open_is_a_premature_end() {
    let mut tokens = Tokens::new("text");
    tokens.phase = Phase::Content;
    assert_eq!(tokens.content(), Err(premature_end()));
}

#[test]
fn an_end_tag_with_no_element_open_is_a_premature_end() {
    let mut tokens = Tokens::new("</a>");
    tokens.phase = Phase::Content;
    assert_eq!(tokens.end_tag(), Err(premature_end()));
}
