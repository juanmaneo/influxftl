// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The XML tokenizer behind the GML and KML readers: a pull parser that gives the events of the
//! document in its order, the way the SAX parser of the JDK does, and stops at the first
//! failure with the message that parser gives.
//!
//! Only the predefined entities and the character references are known: a document type
//! declaration is skipped, its entities are not declared (`docs/DIVERGENCES.md`).

use crate::error::Result;
use crate::error::gml::GmlError;

/// The end of the text, before the root element.
pub const PREMATURE_END: &str = "Premature end of file.";

/// The end of the text inside a markup.
const STRUCTURES: &str = "XML document structures must start and end within the same entity.";

/// Markup before the root element that is not a comment, a declaration or a root tag.
const PROLOG_MARKUP: &str =
    "The markup in the document preceding the root element must be well-formed.";

/// Markup after the root element that is not a comment, a declaration or an end of text.
const TRAILING_MARKUP: &str =
    "The markup in the document following the root element must be well-formed.";

/// Text before the root element.
const PROLOG_CONTENT: &str = "Content is not allowed in prolog.";

/// Text after the root element.
const TRAILING_CONTENT: &str = "Content is not allowed in trailing section.";

/// A `<!` that is neither a comment nor a CDATA section, where a comment or a CDATA is allowed.
const COMMENT: &str = "Comment must start with \"<!--\".";

/// A `<` that starts no markup inside an element.
const CONTENT_MARKUP: &str =
    "The content of elements must consist of well-formed character data or markup.";

/// A character reference that is not a character.
const CHARACTER_REFERENCE_DELIMITER: &str =
    "The character reference must end with the ';' delimiter.";

/// A character reference to a code point that is not an XML character. The oracle's message
/// stops after the `&#`, and so does this one.
const CHARACTER_REFERENCE_INVALID: &str = "Character reference \"&#";

/// A reference to an entity that is not one of the predefined ones.
const ENTITY_NOT_DECLARED_PREFIX: &str = "The entity \"";

/// A reference that is not followed by a name.
const ENTITY_NAME: &str =
    "The entity name must immediately follow the '&' in the entity reference.";

/// A value of an attribute that holds a `<`.
const ATTRIBUTE_LESS_THAN: &str = "The value of attribute \"";

/// An event of the document, in the order of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A start tag, or the start of an empty element tag, with its attributes in order.
    Start {
        /// The name of the element, with its prefix.
        name: String,
        /// The attributes, with their values decoded, in the order of the document.
        attributes: Vec<(String, String)>,
    },
    /// An end tag, or the end of an empty element tag.
    End {
        /// The name of the element.
        name: String,
    },
    /// Character data, with the references decoded and the line ends normalized.
    Text(String),
}

/// Where the tokenizer is in the document.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Before the root element.
    Prolog,
    /// Inside the root element.
    Content,
    /// After the root element.
    Trailing,
}

/// The events of a document, read on demand. The first failure is given once, then there are
/// no more events.
pub struct Tokens<'a> {
    /// The text not read yet.
    rest: &'a str,
    phase: Phase,
    /// The names of the open elements, the innermost last.
    open: Vec<String>,
    /// The element of an empty element tag, whose end comes next.
    empty: Option<String>,
    finished: bool,
}

impl<'a> Tokens<'a> {
    /// The events of `text`, read from its start.
    pub fn new(text: &'a str) -> Self {
        Tokens {
            rest: text,
            phase: Phase::Prolog,
            open: Vec::new(),
            empty: None,
            finished: false,
        }
    }

    /// The next event, `None` at the end of the document.
    fn step(&mut self) -> Result<Option<Event>> {
        if let Some(name) = self.empty.take() {
            return Ok(Some(self.end(name)));
        }
        match self.phase {
            Phase::Prolog => self.prolog(),
            Phase::Content => self.content(),
            Phase::Trailing => self.trailing(),
        }
    }

    /// The end of the element `name`, which ends the document when it is the root.
    fn end(&mut self, name: String) -> Event {
        if self.open.is_empty() {
            self.phase = Phase::Trailing;
        }
        Event::End { name }
    }

    fn prolog(&mut self) -> Result<Option<Event>> {
        loop {
            self.skip_space();
            let text = self.rest;
            if text.is_empty() {
                return Err(fail(PREMATURE_END));
            }
            if text.starts_with("<?") {
                // An XML declaration that is cut off after its keyword is the end of the text.
                if text
                    .strip_prefix("<?xml")
                    .is_some_and(|rest| rest.trim_start_matches(is_space).is_empty())
                {
                    return Err(fail(PREMATURE_END));
                }
                self.skip_until(2, "?>")?;
            } else if text.starts_with("<!--") {
                self.skip_until(4, "-->")?;
            } else if text.starts_with("<!DOCTYPE") {
                self.skip_doctype()?;
            } else if "<!--".starts_with(text) {
                return Err(fail(STRUCTURES));
            } else if text.starts_with("<!-") {
                return Err(fail(COMMENT));
            } else if text.starts_with("<!") {
                return Err(fail(PROLOG_MARKUP));
            } else if let Some(body) = text.strip_prefix('<') {
                if name_length(body) == 0 {
                    return Err(fail(PROLOG_MARKUP));
                }
                let event = self.start_tag()?;
                self.phase = Phase::Content;
                return Ok(Some(event));
            } else {
                return Err(fail(PROLOG_CONTENT));
            }
        }
    }

    fn content(&mut self) -> Result<Option<Event>> {
        loop {
            let text = self.rest;
            if self.open.is_empty() {
                return Err(fail(PREMATURE_END));
            }
            if text.is_empty() {
                return Err(fail(STRUCTURES));
            }
            if !text.starts_with('<') {
                return self.characters().map(Some);
            }
            if text.starts_with("<!--") {
                self.skip_until(4, "-->")?;
            } else if text.starts_with("<![CDATA[") {
                return self.cdata().map(Some);
            } else if text.starts_with("<?") {
                self.skip_until(2, "?>")?;
            } else if text.starts_with("</") {
                return self.end_tag().map(Some);
            } else if "<!--".starts_with(text) {
                return Err(fail(STRUCTURES));
            } else if text.starts_with("<!-") {
                return Err(fail(COMMENT));
            } else if text.starts_with("<!") {
                return Err(fail(CONTENT_MARKUP));
            } else if name_length(&text[1..]) > 0 {
                return self.start_tag().map(Some);
            } else {
                return Err(fail(CONTENT_MARKUP));
            }
        }
    }

    fn trailing(&mut self) -> Result<Option<Event>> {
        loop {
            self.skip_space();
            let text = self.rest;
            if text.is_empty() {
                return Ok(None);
            }
            if text.starts_with("<!--") {
                self.skip_until(4, "-->")?;
            } else if text.starts_with("<?") {
                self.skip_until(2, "?>")?;
            } else if text.starts_with("<!") {
                return Err(fail(COMMENT));
            } else if text == "<" {
                return Err(fail(STRUCTURES));
            } else if text.starts_with('<') {
                return Err(fail(TRAILING_MARKUP));
            } else {
                return Err(fail(TRAILING_CONTENT));
            }
        }
    }

    /// The text up to the next markup, with its references decoded.
    fn characters(&mut self) -> Result<Event> {
        let text = self.rest;
        let end = text.find('<').unwrap_or(text.len());
        self.rest = &text[end..];
        Ok(Event::Text(decode(&line_ends(&text[..end]))?))
    }

    /// A CDATA section, whose text is taken as it is.
    fn cdata(&mut self) -> Result<Event> {
        let body = &self.rest["<![CDATA[".len()..];
        let Some(end) = body.find("]]>") else {
            return Err(fail(STRUCTURES));
        };
        self.rest = &body[end + "]]>".len()..];
        Ok(Event::Text(line_ends(&body[..end])))
    }

    /// A start tag, with its attributes, and the end of it when the tag is empty.
    fn start_tag(&mut self) -> Result<Event> {
        let text = self.rest;
        let body = &text[1..];
        let length = name_length(body);
        let name = body[..length].to_owned();
        self.rest = &body[length..];
        let mut attributes: Vec<(String, String)> = Vec::new();
        loop {
            self.skip_space();
            let text = self.rest;
            if let Some(rest) = text.strip_prefix("/>") {
                self.rest = rest;
                self.empty = Some(name.clone());
                break;
            }
            if let Some(rest) = text.strip_prefix('>') {
                self.rest = rest;
                self.open.push(name.clone());
                break;
            }
            if text.is_empty() {
                return Err(fail(STRUCTURES));
            }
            let length = name_length(text);
            if length == 0 {
                return Err(fail(&format!(
                    "Element type \"{name}\" must be followed by either attribute specifications, \
                     \">\" or \"/>\"."
                )));
            }
            let attribute = &text[..length];
            self.rest = &text[length..];
            self.skip_space();
            let Some(after) = self.rest.strip_prefix('=') else {
                return Err(fail(&format!(
                    "Attribute name \"{attribute}\" associated with an element type \"{name}\" must be \
                     followed by the ' = ' character."
                )));
            };
            self.rest = after;
            self.skip_space();
            let text = self.rest;
            let Some(quote) = text.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
                return Err(fail(&format!(
                    "Open quote is expected for attribute \"{attribute}\" associated with an  element type  \"{name}\"."
                )));
            };
            let body = &text[1..];
            // The value is refused at its first `<`, even when the closing quote comes after it
            // or does not come at all.
            let end = body.find(quote);
            if body[..end.unwrap_or(body.len())].contains('<') {
                return Err(fail(&format!(
                    "{ATTRIBUTE_LESS_THAN}{attribute}\" associated with an element type \"{name}\" must not \
                     contain the '<' character."
                )));
            }
            let Some(end) = end else {
                return Err(fail(STRUCTURES));
            };
            let raw = &body[..end];
            self.rest = &body[end + 1..];
            if attributes.iter().any(|(key, _)| key == attribute) {
                return Err(fail(&format!(
                    "Attribute \"{attribute}\" was already specified for element \"{name}\"."
                )));
            }
            let value = decode(&line_ends(raw).replace(['\t', '\n'], " "))?;
            attributes.push((attribute.to_owned(), value));
        }
        Ok(Event::Start { name, attributes })
    }

    /// The end tag of the innermost open element.
    fn end_tag(&mut self) -> Result<Event> {
        let text = self.rest;
        let body = &text[2..];
        let Some(name) = self.open.last().cloned() else {
            return Err(fail(PREMATURE_END));
        };
        let Some(rest) = body.strip_prefix(name.as_str()) else {
            return Err(fail(&terminated(&name)));
        };
        let rest = rest.trim_start_matches(is_space);
        let Some(rest) = rest.strip_prefix('>') else {
            if rest.is_empty() {
                return Err(fail(STRUCTURES));
            }
            return Err(fail(&format!(
                "The end-tag for element type \"{name}\" must end with a '>' delimiter."
            )));
        };
        self.rest = rest;
        self.open.pop();
        Ok(self.end(name))
    }

    /// Skip the markup that starts with `opened` characters and ends with `closing`.
    fn skip_until(&mut self, opened: usize, closing: &str) -> Result<()> {
        let body = &self.rest[opened..];
        let Some(end) = body.find(closing) else {
            return Err(fail(STRUCTURES));
        };
        self.rest = &body[end + closing.len()..];
        Ok(())
    }

    /// A document type declaration, up to its `>` past an internal subset and quoted literals.
    ///
    /// The end of the text inside an internal subset is the end of the text before the root
    /// element, as the parser reads it; elsewhere it is a structure failure.
    fn skip_doctype(&mut self) -> Result<()> {
        let text = self.rest;
        let mut quote = None;
        let mut depth = 0_usize;
        let mut subset = false;
        for (at, c) in text.char_indices().skip("<!DOCTYPE".len()) {
            if let Some(open) = quote {
                if c == open {
                    quote = None;
                }
                continue;
            }
            match c {
                '"' | '\'' => quote = Some(c),
                '[' => {
                    depth += 1;
                    subset = true;
                }
                ']' => depth = depth.saturating_sub(1),
                '>' if depth == 0 => {
                    self.rest = &text[at + 1..];
                    return Ok(());
                }
                _ => {}
            }
        }
        if subset && quote.is_none() {
            return Err(fail(PREMATURE_END));
        }
        Err(fail(STRUCTURES))
    }

    fn skip_space(&mut self) {
        self.rest = self.rest.trim_start_matches(is_space);
    }
}

impl Iterator for Tokens<'_> {
    type Item = Result<Event>;

    fn next(&mut self) -> Option<Result<Event>> {
        if self.finished {
            return None;
        }
        match self.step() {
            Ok(Some(event)) => Some(Ok(event)),
            Ok(None) => {
                self.finished = true;
                None
            }
            Err(err) => {
                self.finished = true;
                Some(Err(err))
            }
        }
    }
}

/// A failure of the document, with the message of the parser.
fn fail(message: &str) -> crate::error::Error {
    GmlError::Xml(message.to_owned()).into()
}

/// The message of an element that is not ended before the end of the text.
fn terminated(name: &str) -> String {
    format!("The element type \"{name}\" must be terminated by the matching end-tag \"</{name}>\".")
}

/// The XML white space.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

/// The characters of an XML document (the `Char` production).
fn is_xml_char(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

/// The length of the name at the start of `text`, `0` when there is none.
fn name_length(text: &str) -> usize {
    let mut chars = text.char_indices();
    match chars.next() {
        Some((_, c)) if is_name_start(c) => {}
        _ => return 0,
    }
    chars
        .find(|&(_, c)| !is_name_char(c))
        .map_or(text.len(), |(at, _)| at)
}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || matches!(c, '_' | ':')
}

fn is_name_char(c: char) -> bool {
    is_name_start(c) || c.is_ascii_digit() || matches!(c, '-' | '.')
}

/// The line ends of `text` as the parser gives them: `\r\n` and `\r` are `\n`.
fn line_ends(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// `text` with its references decoded.
fn decode(text: &str) -> Result<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let (c, length) = reference(&rest[at..])?;
        out.push(c);
        rest = &rest[at + length..];
    }
    out.push_str(rest);
    Ok(out)
}

/// The character a reference at the start of `text` stands for, and the length of the
/// reference.
fn reference(text: &str) -> Result<(char, usize)> {
    let body = &text[1..];
    if let Some(number) = body.strip_prefix('#') {
        let Some(end) = number.find(';') else {
            return Err(fail(CHARACTER_REFERENCE_DELIMITER));
        };
        let digits = &number[..end];
        let code = match digits.strip_prefix('x') {
            Some(hex) => u32::from_str_radix(hex, 16),
            None => digits.parse::<u32>(),
        };
        let c = code
            .ok()
            .and_then(char::from_u32)
            .filter(|&c| is_xml_char(c))
            .ok_or_else(|| fail(CHARACTER_REFERENCE_INVALID))?;
        return Ok((c, end + 3));
    }
    let length = name_length(body);
    if length == 0 {
        return Err(fail(ENTITY_NAME));
    }
    let name = &body[..length];
    if !body[length..].starts_with(';') {
        return Err(fail(&format!(
            "The reference to entity \"{name}\" must end with the ';' delimiter."
        )));
    }
    let c = match name {
        "lt" => '<',
        "gt" => '>',
        "amp" => '&',
        "quot" => '"',
        "apos" => '\'',
        _ => {
            return Err(fail(&format!(
                "{ENTITY_NOT_DECLARED_PREFIX}{name}\" was referenced, but not declared."
            )));
        }
    };
    Ok((c, length + 2))
}

#[cfg(test)]
#[path = "xml_state_tests.rs"]
mod state_tests;
