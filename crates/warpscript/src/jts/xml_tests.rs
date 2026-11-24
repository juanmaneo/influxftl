// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The events of the XML tokenizer that the GML and KML readers rely on: the tags, the
//! attributes, the references and the sections, and the failures of a document that is not
//! well formed. The messages are those of the SAX parser the oracle runs.

use super::xml::{Event, PREMATURE_END, Tokens};
use crate::error::Error;

fn start(name: &str, attributes: &[(&str, &str)]) -> Event {
    Event::Start {
        name: name.to_owned(),
        attributes: attributes
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
    }
}

fn end(name: &str) -> Event {
    Event::End {
        name: name.to_owned(),
    }
}

fn text(value: &str) -> Event {
    Event::Text(value.to_owned())
}

/// The events of `document`, which must be well formed.
fn events(document: &str) -> Vec<Event> {
    Tokens::new(document)
        .map(|event| event.expect("a well formed document"))
        .collect()
}

/// The message of the failure that `document` ends with, which must have one.
fn failure(document: &str) -> String {
    match Tokens::new(document).find_map(Result::err) {
        Some(Error::Gml(err)) => err.to_string(),
        other => panic!("{document:?} should fail, got {other:?}"),
    }
}

#[test]
fn tags_and_text_come_in_document_order() {
    assert_eq!(
        events(r#"<?xml version="1.0"?><!-- c --><a x='1' y="2"><b/>t<!-- c -->u</a>"#),
        vec![
            start("a", &[("x", "1"), ("y", "2")]),
            start("b", &[]),
            end("b"),
            text("t"),
            text("u"),
            end("a"),
        ]
    );
}

#[test]
fn the_name_of_an_element_keeps_its_prefix() {
    assert_eq!(
        events("<gml:Point xmlns:gml='g'/>"),
        vec![start("gml:Point", &[("xmlns:gml", "g")]), end("gml:Point")]
    );
}

#[test]
fn references_are_decoded_in_attributes_and_text() {
    assert_eq!(
        events(r#"<a x="&lt;&amp;&#x41;&#66;">&quot;&apos;&gt;&#x2e;&#x1F600;</a>"#),
        vec![
            start("a", &[("x", "<&AB")]),
            text("\"'>.\u{1F600}"),
            end("a"),
        ]
    );
}

#[test]
fn a_cdata_section_is_taken_as_it_is() {
    assert_eq!(
        events("<a>x<![CDATA[<b>&amp;]]></a>"),
        vec![start("a", &[]), text("x"), text("<b>&amp;"), end("a")]
    );
}

#[test]
fn line_ends_are_normalized() {
    assert_eq!(
        events("<a>x\r\ny\rz</a>"),
        vec![start("a", &[]), text("x\ny\nz"), end("a")]
    );
}

#[test]
fn comments_and_processing_instructions_are_skipped() {
    assert_eq!(
        events("<?pi x?><a><?pi?><!--c--></a><!-- after -->"),
        vec![start("a", &[]), end("a")]
    );
}

#[test]
fn a_document_that_is_not_well_formed_fails_once() {
    let mut tokens = Tokens::new("<a>&nope;</a>");
    assert_eq!(tokens.next(), Some(Ok(start("a", &[]))));
    assert!(matches!(tokens.next(), Some(Err(_))));
    assert!(tokens.next().is_none());
}

#[test]
fn the_failures_of_the_document_structure() {
    assert_eq!(failure(""), PREMATURE_END);
    assert_eq!(
        failure("<a x='1' x='2'/>"),
        "Attribute \"x\" was already specified for element \"a\"."
    );
    assert_eq!(
        failure("<a x='<'/>"),
        "The value of attribute \"x\" associated with an element type \"a\" must not contain the '<' character."
    );
    assert_eq!(
        failure("<a/>x"),
        "Content is not allowed in trailing section."
    );
    assert_eq!(
        failure("<a><!-x --></a>"),
        "Comment must start with \"<!--\"."
    );
    assert_eq!(
        failure("<a>"),
        "XML document structures must start and end within the same entity."
    );
}

#[test]
fn the_failures_of_the_references() {
    assert_eq!(
        failure("<a>&nope;</a>"),
        "The entity \"nope\" was referenced, but not declared."
    );
    assert_eq!(
        failure("<a>&nope</a>"),
        "The reference to entity \"nope\" must end with the ';' delimiter."
    );
    for reference in ["&#0;", "&#1;", "&#xD800;", "&#xFFFE;", "&#x110000;"] {
        assert_eq!(
            failure(&format!("<a>{reference}</a>")),
            "Character reference \"&#"
        );
    }
}
