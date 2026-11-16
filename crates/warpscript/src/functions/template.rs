// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.
// Derived from mustache.java 0.9.6 (Copyright 2010 RightTime, Inc., Apache-2.0), ported to Rust and modified.

//! `TEMPLATE`: fill a Mustache template from a `MAP`.
//!
//! Warp 10 delegates to mustache.java 0.9.6 (a `DefaultMustacheFactory` whose resolver refuses
//! every inclusion). The parser below is a statement-by-statement port of
//! `MustacheParser.compile`, including its peculiar handling of standalone lines and of
//! carriage returns, so that the output is identical to the Java one. The runtime ports
//! `ValueCode`, `IterableCode`, `NotIterableCode`, `ReflectionObjectHandler.find` and
//! `HtmlEscaper`. Java reflection on the scope objects is only emulated for a fixed set of
//! members (see `DIVERGENCES.md` §16).

use std::collections::VecDeque;

use super::Entry;
use super::args::fail;
use crate::error::fault::BoundsError;
use crate::error::text::TextError;
use crate::error::{Error, Result};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[("TEMPLATE", Entry::Stack(template))];

/// `TEMPLATE`: pops a `MAP` (the scope) and a `STRING` (the template) and pushes the
/// rendered text. Only strings, numbers, booleans, lists and maps thereof may appear in the
/// scope.
fn template(stack: &mut Stack) -> Result<()> {
    let Value::Map(scope) = stack.pop()? else {
        return Err(fail("TEMPLATE", "expects a map on top of the stack."));
    };
    let Value::String(text) = stack.pop()? else {
        return Err(fail("TEMPLATE", "operates on a string."));
    };
    let rendered = render_template(&text, Value::Map(scope))
        .map_err(|cause| Error::with_cause("TEMPLATE failed.", cause))?;
    stack.push(Value::from(rendered));
    Ok(())
}

/// Check the scope types, compile `template` and execute it against `scope`.
pub(crate) fn render_template(template: &str, scope: Value) -> Result<String> {
    check_types(&scope)?;
    let codes = Parser::parse(template)?;
    init(&codes)?;
    let mut out = String::new();
    let mut scopes = vec![scope];
    run(&codes, &mut scopes, &mut out);
    Ok(out)
}

/// `TEMPLATE.checkTypes`, applied breadth-first to the whole scope: only strings, numbers,
/// booleans, lists and maps (keys included) are accepted.
fn check_types(scope: &Value) -> Result<()> {
    let mut pending = VecDeque::from([scope.clone()]);
    while let Some(value) = pending.pop_front() {
        match value {
            Value::String(_)
            | Value::Long(_)
            | Value::Double(_)
            | Value::Boolean(_)
            | Value::Counter(_) => {}
            Value::List(list) => pending.extend(list.borrow().iter().cloned()),
            Value::Map(map) => {
                let map = map.borrow();
                pending.extend(map.keys().cloned());
                pending.extend(map.values().cloned());
            }
            _ => return Err(TextError::TemplateSubstitutionType.into()),
        }
    }
    Ok(())
}

/// A compiled element of a template (mustache.java `Code`): what it does, followed by the
/// literal text appended to it by the parser.
struct Code {
    kind: Kind,
    appended: String,
}

/// The behaviours of the codes produced by the parser.
enum Kind {
    /// Literal text only (`WriteCode`).
    Write,
    /// `{{name}}` (escaped) or `{{{name}}}` / `{{&name}}` (`ValueCode`).
    Variable { name: String, escaped: bool },
    /// `{{#name}}…{{/name}}` (`IterableCode`) or `{{^name}}…{{/name}}` (`NotIterableCode`).
    Section {
        name: String,
        inverted: bool,
        codes: Vec<Code>,
    },
    /// `{{$name}}…{{/name}}` (`ExtendNameCode`) or `{{?name}}…{{/name}}`
    /// (`ExtendCheckNameCode`): rendered as is outside of a parent template.
    Block { check: bool, codes: Vec<Code> },
    /// `{{! … }}` (`CommentCode`).
    Comment,
    /// `{{%implicit-iterator}}`: the only pragma mustache.java knows, a no-op `DefaultCode`.
    Pragma,
    /// `{{>name}}` (`PartialCode`): fails at initialisation, inclusions are disabled.
    Partial,
    /// `{{<name}}…{{/name}}` (`ExtendCode`): fails at initialisation too.
    Extend { codes: Vec<Code> },
}

impl Kind {
    /// The Java class of a code that `ExtendCode.init` rejects inside a `{{<name}}` section;
    /// `None` for the codes it accepts (text, comments and `{{$name}}` blocks).
    fn illegal_in_extend(&self) -> Option<&'static str> {
        match self {
            Kind::Write | Kind::Comment | Kind::Block { check: false, .. } => None,
            Kind::Variable { .. } => Some("ValueCode"),
            Kind::Section {
                inverted: false, ..
            } => Some("IterableCode"),
            Kind::Section { inverted: true, .. } => Some("NotIterableCode"),
            Kind::Block { check: true, .. } => Some("ExtendCheckNameCode"),
            Kind::Pragma => Some("DefaultCode"),
            Kind::Partial => Some("PartialCode"),
            Kind::Extend { .. } => Some("ExtendCode"),
        }
    }
}

/// `MustacheParser` state: the template characters, the read position and the shared line
/// counter (`currentLine`).
struct Parser {
    chars: Vec<char>,
    pos: usize,
    line: u32,
}

/// A `MustacheException` carrying a template context (`message @[:line]`).
fn mustache_error(message: &str, line: u32) -> Error {
    TextError::MustacheAt {
        message: message.to_owned(),
        line,
    }
    .into()
}

/// Java `String.trim()`.
pub(super) fn trim_control(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

/// Flush the pending literal text: appended to the previous code, or a new `WriteCode` when
/// there is none (`DefaultMustacheVisitor.write`).
fn write(codes: &mut Vec<Code>, out: &mut String) {
    if !out.is_empty() {
        match codes.last_mut() {
            Some(last) => last.appended.push_str(out),
            None => codes.push(Code {
                kind: Kind::Write,
                appended: out.clone(),
            }),
        }
        out.clear();
    }
}

impl Parser {
    /// Compile a whole template with the default `{{` `}}` delimiters.
    fn parse(template: &str) -> Result<Vec<Code>> {
        let mut parser = Parser {
            chars: template.chars().collect(),
            pos: 0,
            line: 0,
        };
        parser.compile(None, vec!['{', '{'], vec!['}', '}'], true)
    }

    fn read(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        self.pos += usize::from(c.is_some());
        c
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// Read the body of a tag opened by `sm`, up to `em` (`{{= =}}` delimiter changes run
    /// to the closing `=` first). Returns the body and whether the input ended before `em`.
    fn read_tag(&mut self, em: &[char]) -> (String, bool) {
        let mut sb = String::new();
        let mut delimiter = false;
        if self.peek() == Some('=') {
            self.read();
            sb.push('=');
            delimiter = true;
        }
        while let Some(c) = self.read() {
            if delimiter {
                if c == '=' {
                    delimiter = false;
                } else {
                    sb.push(c);
                }
                continue;
            }
            if c == em[0] {
                if em.len() > 1 {
                    if self.peek() == Some(em[1]) {
                        self.read();
                        return (sb, false);
                    }
                } else {
                    return (sb, false);
                }
            }
            sb.push(c);
        }
        (sb, true)
    }

    /// `MustacheParser.compile`: parse until the closing tag `tag` (or the end of the input
    /// at the top level) with the delimiters `sm` / `em`.
    #[allow(clippy::too_many_lines)]
    fn compile(
        &mut self,
        tag: Option<&str>,
        mut sm: Vec<char>,
        mut em: Vec<char>,
        mut start_of_line: bool,
    ) -> Result<Vec<Code>> {
        let mut saw_cr = false;
        let start_line = self.line;
        let mut codes = Vec::new();
        let mut only_whitespace = true;
        let mut iterable = self.line != 0;
        if self.line == 0 {
            self.line = 1;
        }
        let mut out = String::new();
        while let Some(c) = self.read() {
            // A carriage return is only kept when the newline that follows it is kept.
            if c == '\r' {
                saw_cr = true;
                continue;
            }
            if c == '\n' {
                self.line += 1;
                if !iterable || !only_whitespace {
                    if saw_cr {
                        out.push('\r');
                    }
                    out.push('\n');
                }
                write(&mut codes, &mut out);
                iterable = false;
                only_whitespace = true;
                start_of_line = true;
                continue;
            }
            saw_cr = false;
            if c == sm[0] && (sm.len() == 1 || self.peek() == Some(sm[1])) {
                if sm.len() > 1 {
                    self.read();
                }
                let (command, eof) = self.read_tag(&em);
                let Some(ch) = command.chars().next() else {
                    return Err(mustache_error("Empty mustache", self.line));
                };
                let variable = trim_control(&command[ch.len_utf8()..]);
                match ch {
                    '#' | '^' | '<' | '?' | '$' => {
                        start_of_line &= only_whitespace;
                        let line = self.line;
                        let inner =
                            self.compile(Some(variable), sm.clone(), em.clone(), start_of_line)?;
                        let lines = self.line - line;
                        if !only_whitespace || lines == 0 {
                            write(&mut codes, &mut out);
                        }
                        out.clear();
                        let name = variable.to_owned();
                        let kind = match ch {
                            '#' | '^' => Kind::Section {
                                name,
                                inverted: ch == '^',
                                codes: inner,
                            },
                            '<' => Kind::Extend { codes: inner },
                            _ => Kind::Block {
                                check: ch == '?',
                                codes: inner,
                            },
                        };
                        codes.push(Code {
                            kind,
                            appended: String::new(),
                        });
                        iterable = lines != 0;
                    }
                    '/' => {
                        if !start_of_line || !only_whitespace {
                            write(&mut codes, &mut out);
                        }
                        if Some(variable) != tag {
                            let tag = tag.unwrap_or("null");
                            return Err(mustache_error(
                                &format!(
                                    "Mismatched start/end tags: {tag} != {variable} in :{}",
                                    self.line
                                ),
                                self.line,
                            ));
                        }
                        return Ok(codes);
                    }
                    '>' => {
                        write(&mut codes, &mut out);
                        codes.push(Code {
                            kind: Kind::Partial,
                            appended: String::new(),
                        });
                    }
                    '{' => {
                        write(&mut codes, &mut out);
                        let name = self.unescaped_name(variable, &em)?;
                        codes.push(Code {
                            kind: Kind::Variable {
                                name,
                                escaped: false,
                            },
                            appended: String::new(),
                        });
                    }
                    '&' => {
                        write(&mut codes, &mut out);
                        codes.push(Code {
                            kind: Kind::Variable {
                                name: variable.to_owned(),
                                escaped: false,
                            },
                            appended: String::new(),
                        });
                    }
                    '%' => {
                        write(&mut codes, &mut out);
                        let pragma = variable.split(' ').next().unwrap_or_default();
                        if pragma.to_lowercase() == "implicit-iterator" {
                            codes.push(Code {
                                kind: Kind::Pragma,
                                appended: String::new(),
                            });
                        }
                    }
                    '!' => {
                        codes.push(Code {
                            kind: Kind::Comment,
                            appended: String::new(),
                        });
                        write(&mut codes, &mut out);
                    }
                    '=' => {
                        write(&mut codes, &mut out);
                        let trimmed = trim_control(&command[1..]);
                        let split: Vec<&str> = trimmed
                            .split([' ', '\t', '\n', '\x0B', '\x0C', '\r'])
                            .filter(|part| !part.is_empty())
                            .collect();
                        if split.len() != 2 {
                            return Err(mustache_error(
                                &format!("Invalid delimiter string: {trimmed}"),
                                self.line,
                            ));
                        }
                        sm = split[0].chars().collect();
                        em = split[1].chars().collect();
                    }
                    _ => {
                        if eof {
                            return Err(mustache_error("Improperly closed variable", self.line));
                        }
                        write(&mut codes, &mut out);
                        codes.push(Code {
                            kind: Kind::Variable {
                                name: trim_control(&command).to_owned(),
                                escaped: true,
                            },
                            appended: String::new(),
                        });
                    }
                }
                // Additional text is no longer at the start of the line.
                start_of_line = false;
                continue;
            }
            only_whitespace = only_whitespace && (c == ' ' || c == '\t');
            out.push(c);
        }
        write(&mut codes, &mut out);
        if let Some(tag) = tag {
            return Err(mustache_error(
                &format!("Failed to close '{tag}' tag"),
                start_line,
            ));
        }
        Ok(codes)
    }

    /// The name of a `{{{name}}}` variable: with the default end delimiter the third `}` must
    /// follow, otherwise the trailing `}` is part of the captured text and is dropped.
    fn unescaped_name(&mut self, variable: &str, em: &[char]) -> Result<String> {
        match em.get(1) {
            // `em.charAt(1)` on a one character delimiter.
            None => Err(BoundsError::StringIndex {
                index: 1,
                length: 1,
            }
            .into()),
            Some('}') => {
                if self.read() != Some('}') {
                    return Err(mustache_error(
                        &format!("Improperly closed variable in :{}", self.line),
                        self.line,
                    ));
                }
                Ok(variable.to_owned())
            }
            Some(_) => match variable.chars().next_back() {
                Some(last) => Ok(variable[..variable.len() - last.len_utf8()].to_owned()),
                // `"".substring(0, -1)`
                None => Err(BoundsError::StringRange {
                    start: 0,
                    end: -1,
                    length: 0,
                }
                .into()),
            },
        }
    }
}

/// `Mustache.init()`: resolve partials and parents — which Warp 10 forbids — and validate
/// the content of `{{<name}}` sections first, like `ExtendCode.init` does.
fn init(codes: &[Code]) -> Result<()> {
    for code in codes {
        match &code.kind {
            Kind::Partial => return Err(TextError::InclusionDisabled.into()),
            Kind::Extend { codes } => {
                for code in codes {
                    if let Some(class) = code.kind.illegal_in_extend() {
                        return Err(TextError::IllegalExtendCode(class).into());
                    }
                    if let Kind::Block { codes, .. } = &code.kind {
                        init(codes)?;
                    }
                }
                return Err(TextError::InclusionDisabled.into());
            }
            Kind::Section { codes, .. } | Kind::Block { codes, .. } => init(codes)?,
            Kind::Write | Kind::Variable { .. } | Kind::Comment | Kind::Pragma => {}
        }
    }
    Ok(())
}

/// `DefaultMustache.run`: execute the codes against the scope stack.
fn run(codes: &[Code], scopes: &mut Vec<Value>, out: &mut String) {
    for code in codes {
        match &code.kind {
            Kind::Variable { name, escaped } => {
                if let Some(value) = get(name, scopes) {
                    let text = value.to_string();
                    if *escaped {
                        html_escape(&text, out);
                    } else {
                        out.push_str(&text);
                    }
                }
            }
            Kind::Section {
                name,
                inverted: false,
                codes,
            } => {
                if let Some(value) = get(name, scopes) {
                    iterate(&value, codes, scopes, out);
                }
            }
            Kind::Section {
                name,
                inverted: true,
                codes,
            } => {
                if falsey(get(name, scopes).as_ref()) {
                    run(codes, scopes, out);
                }
            }
            Kind::Block { codes, .. } => run(codes, scopes, out),
            Kind::Write | Kind::Comment | Kind::Pragma | Kind::Partial | Kind::Extend { .. } => {}
        }
        out.push_str(&code.appended);
    }
}

/// `DefaultCode.get`: `.` is the innermost scope, anything else goes through the lookup.
fn get(name: &str, scopes: &[Value]) -> Option<Value> {
    if name == "." {
        scopes.last().cloned()
    } else {
        find(name, scopes)
    }
}

/// `ReflectionObjectHandler.find`: scopes are searched innermost first; a dotted name is
/// first tried literally, then resolved step by step — once its first step resolves, a
/// missing later step is final (unless an intermediate step is missing, which moves on to
/// the next scope).
fn find(name: &str, scopes: &[Value]) -> Option<Value> {
    'scopes: for scope in scopes.iter().rev() {
        if let Some(value) = member(scope, name) {
            return Some(value);
        }
        if !name.contains('.') {
            continue;
        }
        let mut current = scope.clone();
        let mut subname = name;
        while let Some((lookup, rest)) = subname.split_once('.') {
            subname = rest;
            match member(&current, lookup) {
                Some(next) => current = next,
                None => continue 'scopes,
            }
        }
        return member(&current, subname);
    }
    None
}

/// `ReflectionObjectHandler.findWrapper`: a map is queried by key (string keys only); on
/// any other object, mustache.java reflects on the Java members — emulated for the members
/// a template may reasonably use.
fn member(scope: &Value, name: &str) -> Option<Value> {
    match scope {
        Value::Map(map) => map.borrow().get(&Value::from(name)).cloned(),
        Value::String(s) => match name {
            "length" => Some(Value::Long(s.encode_utf16().count() as i64)),
            "isEmpty" | "empty" => Some(Value::Boolean(s.is_empty())),
            "toUpperCase" => Some(Value::from(s.to_uppercase())),
            "toLowerCase" => Some(Value::from(s.to_lowercase())),
            "trim" => Some(Value::from(trim_control(s))),
            "toString" => Some(scope.clone()),
            _ => None,
        },
        Value::Double(d) => match name {
            "intValue" => Some(Value::Long(i64::from(*d as i32))),
            "longValue" => Some(Value::Long(crate::value::double_to_long(*d))),
            "doubleValue" => Some(scope.clone()),
            "isNaN" | "naN" => Some(Value::Boolean(d.is_nan())),
            "isInfinite" | "infinite" => Some(Value::Boolean(d.is_infinite())),
            "toString" => Some(Value::from(scope.to_string())),
            _ => None,
        },
        Value::Boolean(b) => match name {
            "booleanValue" => Some(scope.clone()),
            "toString" => Some(Value::from(b.to_string())),
            _ => None,
        },
        Value::List(list) => match name {
            "size" => Some(Value::Long(list.borrow().len() as i64)),
            "isEmpty" | "empty" => Some(Value::Boolean(list.borrow().is_empty())),
            "toString" => Some(Value::from(scope.to_string())),
            _ => None,
        },
        // `LONG`s and counters (`AtomicLong`s), the remaining types `TEMPLATE` accepts, are
        // `Number`s: the members below apply to their value.
        number => {
            let l = match number {
                Value::Counter(counter) => counter.get(),
                other => other.as_i64().unwrap_or_default(),
            };
            match name {
                "intValue" => Some(Value::Long(i64::from(l as i32))),
                "longValue" => Some(Value::Long(l)),
                "doubleValue" => Some(Value::Double(l as f64)),
                "toString" => Some(Value::from(l.to_string())),
                _ => None,
            }
        }
    }
}

/// `BaseObjectHandler.iterate`: `false` and the empty string render nothing, a list renders
/// the section once per element (each pushed as a scope), anything else (a map, a number, a
/// non-empty string, `true`) once with the value pushed as a scope.
fn iterate(value: &Value, codes: &[Code], scopes: &mut Vec<Value>, out: &mut String) {
    let items = match value {
        Value::Boolean(false) => return,
        Value::String(s) if s.is_empty() => return,
        Value::List(list) => list.borrow().clone(),
        other => vec![other.clone()],
    };
    for item in items {
        scopes.push(item);
        run(codes, scopes, out);
        scopes.pop();
    }
}

/// `BaseObjectHandler.falsey`: missing, `false`, the empty string and the empty list are
/// falsey; every other object (numbers, maps…) is truthy.
fn falsey(value: Option<&Value>) -> bool {
    match value {
        None => true,
        Some(Value::Boolean(b)) => !b,
        Some(Value::String(s)) => s.is_empty(),
        Some(Value::List(list)) => list.borrow().is_empty(),
        Some(_) => false,
    }
}

/// `HtmlEscaper.escape`: the HTML special characters and the control characters below
/// U+000E are written as entities.
fn html_escape(value: &str, out: &mut String) {
    for c in value.chars() {
        match c {
            '\0'..='\r' => {
                // Writing to a `String` cannot fail.
                out.push_str(&format!("&#{};", c as u32));
            }
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '`' => out.push_str("&#96;"),
            '=' => out.push_str("&#61;"),
            _ => out.push(c),
        }
    }
}
