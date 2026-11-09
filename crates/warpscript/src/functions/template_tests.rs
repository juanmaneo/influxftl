// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of `TEMPLATE` against mustache.java 0.9.6 (`compiler-0.9.6.jar`) driven through
//! Warp 10's `TEMPLATE` configuration: every expected output below was produced by Java.

use super::template::render_template;
use crate::error::Error;
use crate::tests::{assert_fails, fail, top};
use crate::value::Value;

/// Percent-encode `s` so that it survives the WarpScript string literal syntax.
fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if !(0x20..0x7f).contains(&b) || b"%'\"".contains(&b) {
            out.push_str(&format!("%{b:02X}"));
        } else {
            out.push(b as char);
        }
    }
    out
}

/// The scope used by most of the Java reference runs.
const BASE: &str = "{ 'name' 'World' 'n' 42 'd' 3.0 'big' 1e20 't' true 'f' false 'empty' '' \
    'list' [ 1 2 3 ] 'elist' [] 'people' [ { 'name' 'Ann' 'age' 30 } { 'name' 'Bob' 'age' 41 } ] \
    'map' { 'k' 'v' 'inner' { 'x' 'y' } } 'a.b' 'dotted-literal' \
    'x' '& < > %22 %27 ` = %0A%09%00%07~' 'matrix' [ [ 1 2 ] [ 3 ] ] 'zero' 0 'nan' NaN }";

fn render_with(template: &str, scope: &str) -> String {
    match top(&format!("'{}' {scope} TEMPLATE", encode(template))) {
        Value::String(s) => s.to_string(),
        other => panic!("TEMPLATE pushed {other:?}"),
    }
}

fn render(template: &str) -> String {
    render_with(template, BASE)
}

/// The error of `TEMPLATE` on `template`, without the `Line #1 (Exception at …)` wrapping
/// added by the interpreter.
fn error(template: &str) -> String {
    let mut err = fail(&format!("'{}' {BASE} TEMPLATE", encode(template)));
    for _ in 0..2 {
        err = match err {
            Error::Context { source, .. } => *source,
            other => panic!("unexpected error {other}"),
        };
    }
    err.to_string()
}

#[test]
fn variables_and_escaping() {
    assert_eq!(render("Hello {{name}}!"), "Hello World!");
    assert_eq!(
        render("{{x}}"),
        "&amp; &lt; &gt; &quot; &#39; &#96; &#61; &#10;&#9;&#0;&#7;~"
    );
    assert_eq!(render("{{{x}}}"), "& < > \" ' ` = \n\t\0\u{7}~");
    assert_eq!(render("{{&x}}"), "& < > \" ' ` = \n\t\0\u{7}~");
    assert_eq!(
        render("{{n}} {{d}} {{big}} {{t}} {{f}} {{zero}} {{nan}}"),
        "42 3.0 1.0E20 true false 0 NaN"
    );
    assert_eq!(render("[{{missing}}]"), "[]");
    assert_eq!(
        render_with(
            "{{a}}|{{b}}|{{c}}|{{e}}",
            "{ 'a' 1.0E-5 'b' 123456789.125 'c' -0.0 'e' 1.0E7 }"
        ),
        "1.0E-5|1.23456789125E8|-0.0|1.0E7"
    );
    assert_eq!(render_with("{{a}}", "{ 'a' 5 }"), "5");
    assert_eq!(render_with("{{a}}", "{ 'a' 'é€😀' }"), "é€😀");
    assert_eq!(render_with("{{.}}", "{ 'k' 'v' }"), "{k&#61;v}");
    assert_eq!(
        render("{{ name }}|{{# list }}{{ . }}{{/ list }}|{{^ f }}n{{/ f }}"),
        "World|123|n"
    );
    assert_eq!(render("{{& name }}|{{{ name }}}"), "World|World");
    assert_eq!(render_with("{{a b}}", "{ 'a b' 'ok' }"), "ok");
    assert_eq!(render_with("[{{1}}]", "{ 1 'x' }"), "[]");
    assert_eq!(render("{{\nname\n}}"), "World");
    assert_eq!(render("a{b}c{{name}}}"), "a{b}cWorld}");
    // A counter is a `Number`.
    assert_eq!(render_with("{{c}}", "{ 'c' COUNTER 7 COUNTERSET }"), "7");
}

#[test]
fn sections() {
    assert_eq!(render("{{#list}}{{.}},{{/list}}"), "1,2,3,");
    assert_eq!(
        render("{{#people}}{{name}} is {{age}}\n{{/people}}"),
        "Ann is 30\nBob is 41\n"
    );
    assert_eq!(render("[{{#f}}yes{{/f}}][{{^f}}no{{/f}}]"), "[][no]");
    assert_eq!(render("[{{#t}}yes{{/t}}][{{^t}}no{{/t}}]"), "[yes][]");
    assert_eq!(
        render("[{{#empty}}yes{{/empty}}][{{^empty}}no{{/empty}}]"),
        "[][no]"
    );
    assert_eq!(
        render("[{{#name}}{{.}}{{/name}}][{{^name}}no{{/name}}]"),
        "[World][]"
    );
    assert_eq!(
        render("{{#map}}{{k}}-{{inner.x}}-{{name}}{{/map}}"),
        "v-y-World"
    );
    assert_eq!(render("[{{#zero}}{{.}}{{/zero}}]"), "[0]");
    assert_eq!(
        render("[{{#elist}}x{{/elist}}][{{^elist}}none{{/elist}}]"),
        "[][none]"
    );
    assert_eq!(
        render("[{{#missing}}x{{/missing}}][{{^missing}}none{{/missing}}]"),
        "[][none]"
    );
    assert_eq!(render("{{#matrix}}{{#.}}{{.}}{{/.}};{{/matrix}}"), "12;3;");
    assert_eq!(render("[{{#t}}{{/t}}]"), "[]");
    assert_eq!(render("{{#map}}{{#list}}{{.}}{{/list}}{{/map}}"), "123");
    assert_eq!(render_with("{{#b}}{{.}}{{/b}}", "{ 'b' true }"), "true");
    assert_eq!(
        render("{{#people}}{{#list}}{{name}}{{.}}{{/list}}{{/people}}"),
        "Ann1Ann2Ann3Bob1Bob2Bob3"
    );
    assert_eq!(render("{{^elist}}{{name}}{{/elist}}"), "World");
    assert_eq!(render("{{#n}}{{.}}{{#.}}{{.}}{{/.}}{{/n}}"), "4242");
    assert_eq!(render("{{#\nt}}x{{/t}}"), "x");
    assert_eq!(render("[{{$x}}y{{/x}}][{{?x}}z{{/x}}]"), "[y][z]");
    assert_eq!(render("{{#t}}x{{/t}}{{#f}}y{{/f}}\nnext"), "x\nnext");
}

#[test]
fn dotted_names() {
    assert_eq!(
        render("{{map.inner.x}}|{{map.missing}}|{{a.b}}|{{map.k.z}}|{{missing.x}}"),
        "y||dotted-literal||"
    );
    assert_eq!(render("{{#people}}{{map.k}}{{/people}}"), "vv");
    assert_eq!(render("{{#map}}{{name}}{{/map}}"), "World");
    // An intermediate step that is missing moves on to the next scope…
    assert_eq!(
        render_with(
            "{{#inner}}{{a.b.c}}{{/inner}}",
            "{ 'a' { 'b' { 'c' 'outer' } } 'inner' { 'a' { 'x' 1 } } }"
        ),
        "outer"
    );
    // … while a missing last step is final.
    assert_eq!(render_with("{{a.b.c}}", "{ 'a' { 'b' { 'z' 1 } } }"), "");
}

#[test]
fn java_members() {
    assert_eq!(
        render(
            "{{name.length}}|{{name.isEmpty}}|{{name.empty}}|{{name.toUpperCase}}|{{name.toLowerCase}}|{{name.toString}}|{{name.trim}}|{{name.class}}"
        ),
        "5|false|false|WORLD|world|World|World|"
    );
    assert_eq!(
        render_with("{{s.length}}|{{s.trim}}|", "{ 's' ' 😀 ' }"),
        "4|😀|"
    );
    assert_eq!(
        render(
            "{{list.size}}|{{list.isEmpty}}|{{list.empty}}|{{list.0}}|{{list.toString}}|{{elist.isEmpty}}"
        ),
        "3|false|false||[1, 2, 3]|true"
    );
    assert_eq!(
        render(
            "{{n.intValue}}|{{d.intValue}}|{{d.isNaN}}|{{nan.naN}}|{{d.infinite}}|{{d.longValue}}|{{n.doubleValue}}|{{n.longValue}}|{{d.doubleValue}}|{{n.toString}}|{{d.toString}}|{{nan.isInfinite}}"
        ),
        "42|3|false|true|false|3|42.0|42|3.0|42|3.0|false"
    );
    assert_eq!(
        render_with(
            "{{a.intValue}}|{{b.intValue}}|{{c.intValue}}",
            "{ 'a' 4294967297 'b' 1e300 'c' NaN }"
        ),
        "1|2147483647|0"
    );
    assert_eq!(
        render("{{t.booleanValue}}|{{#t}}{{booleanValue}}{{/t}}|{{t.toString}}|{{t.other}}"),
        "true|true|true|"
    );
    assert_eq!(
        render(
            "{{#name}}{{length}}{{/name}}|{{#list}}{{intValue}}{{/list}}|{{^list.isEmpty}}nonempty{{/list.isEmpty}}"
        ),
        "5|123|nonempty"
    );
    // Maps are never reflected upon.
    assert_eq!(render("{{map.size}}|{{map.isEmpty}}|{{map.keySet}}"), "||");
    assert_eq!(
        render("{{n.other}}|{{d.other}}|{{list.other}}|{{t.length}}"),
        "|||"
    );
}

/// A closing brace inside a tag, counters (`AtomicLong`s, the only other `Number` the type
/// check lets through) and the truthiness of numbers and maps.
#[test]
fn braces_counters_and_truthiness() {
    const SCOPE: &str = "{ 'n' 42 'map' { 'k' 'v' } 'a}b' 'Z' 'nan' NaN 'zero' 0 \
        'c' COUNTER 5 COUNTERSET 'list' [ 1 2 ] }";
    let render = |template: &str| render_with(template, SCOPE);
    assert_eq!(render("x{{a}b}}y"), "xZy");
    assert_eq!(render("x{{a}}}y"), "x}y");
    assert_eq!(
        render(
            "{{c}}|{{c.x}}|{{#c}}[{{x}}]{{/c}}|[{{^c}}no{{/c}}]|{{c.intValue}}|{{#c}}{{.}}{{/c}}"
        ),
        "5||[]|[]|5|5"
    );
    assert_eq!(
        render(
            "[{{^n}}no{{/n}}][{{^map}}no{{/map}}][{{^nan}}no{{/nan}}][{{^zero}}no{{/zero}}][{{^list}}no{{/list}}]"
        ),
        "[][][][][]"
    );
    assert_eq!(render("{{#map}}{{k}}{{/map}}"), "v");
}

#[test]
fn comments_pragmas_and_delimiters() {
    assert_eq!(render("a{{! this is a comment }}b"), "ab");
    assert_eq!(render("a\n{{! comment }}\nb"), "a\n\nb");
    assert_eq!(
        render("{{%implicit-iterator}}{{%unknown}}{{%Implicit-Iterator with args}}a"),
        "a"
    );
    assert_eq!(
        render("{{=<% %>=}}<%name%> {{name}} <%={{ }}=%>{{name}}"),
        "World {{name}} World"
    );
    assert_eq!(
        render("{{=<% %>=}}<%{x}%>|<%&x%>"),
        "& < > \" ' ` = \n\t\0\u{7}~|& < > \" ' ` = \n\t\0\u{7}~"
    );
    assert_eq!(render("{{={ }=}}{name}"), "World");
    assert_eq!(render("{{=<< >>=}}<<name>> <<{name}>>"), "World World");
    assert_eq!(render("{{=<% %>=}}<%{name}%>"), "World");
    // A delimiter change inside a section does not outlive the section.
    assert_eq!(
        render("{{#t}}{{=<% %>=}}<%name%><%/t%>{{name}}"),
        "WorldWorld"
    );
    // Single `{` or `}` are ordinary text.
    assert_eq!(render("{ {{name}} } }}"), "{ World } }}");
    assert_eq!(render("{{=<% %>=}}<% name %>%"), "World%");
}

#[test]
fn standalone_lines() {
    assert_eq!(render("{{#t}}\nline\n{{/t}}\n"), "line\n");
    assert_eq!(render("  {{#t}}\n  x\n  {{/t}}\nend"), "  x\nend");
    assert_eq!(render("a {{#t}}\nc\n{{/t}} d\n"), "a c\n d\n");
    assert_eq!(
        render("{{#list}}\n- {{.}}\n{{/list}}\nafter"),
        "- 1\n- 2\n- 3\nafter"
    );
    assert_eq!(render("x\n  {{#t}}  \ny\n{{/t}}"), "x\n  y\n");
    assert_eq!(render("a\r\n{{#t}}\r\nb\r\n{{/t}}\r\nc\rd"), "a\r\nb\r\ncd");
    assert_eq!(render("{{#t}}\n\n{{/t}}"), "\n");
    assert_eq!(render("  \n{{#t}}\n  \n{{/t}}"), "  \n  \n");
    assert_eq!(render("{{#t}}\nx {{/t}}\ny"), "x y");
    assert_eq!(render("{{^f}}\nno\n{{/f}}\n"), "no\n");
    assert_eq!(render("{{#t}}\n{{#t}}\nin\n{{/t}}\n{{/t}}\n"), "in\n");
    assert_eq!(render("pre {{#t}}\nin\n{{/t}}\n"), "pre in\n\n");
    assert_eq!(render("{{#t}}\nin {{/t}}\n"), "in ");
    assert_eq!(render("{{#t}} \nin\n{{/t}}\n"), " in\n");
    assert_eq!(render("  \t \n"), "  \t \n");
    assert_eq!(render("{{#t}}a\n  {{/t}}b"), "a\nb");
    assert_eq!(render("\r\n\r"), "\r\n");
    assert_eq!(render("\t{{#t}}\n\tin\n\t{{/t}}\n"), "\tin\n");
    assert_eq!(render("{{#t}}\nin\n{{/t}} \n|"), "in\n |");
    assert_eq!(render("{{#t}}\nin\n{{/t}}{{name}}\n"), "in\nWorld");
    assert_eq!(render("{{! c }} x\n|"), " x\n|");
    assert_eq!(render("{{name}}\n"), "World\n");
    assert_eq!(render("{{^f}}\n{{/f}}x"), "x");
    assert_eq!(render("l1\n{{#t}}\nl2\n{{/t}}\nl3\n"), "l1\nl2\nl3\n");
    assert_eq!(render("x {{#t}} y\nz\n{{/t}}\n"), "x  y\nz\n\n");
    assert_eq!(render("{{#t}}\nin\n{{/t}}  "), "in\n  ");
    assert_eq!(render("{{#t}}\nin\n{{/t}}  \nx"), "in\n  x");
    assert_eq!(render("{{=| |=}}|#t||name||/t|"), "World");
}

#[test]
fn inclusions_are_disabled() {
    assert_eq!(
        error("{{>inc}}"),
        "TEMPLATE failed. (Template inclusion is disabled.)"
    );
    assert_eq!(
        error("{{#t}}{{>inc}}{{/t}}"),
        "TEMPLATE failed. (Template inclusion is disabled.)"
    );
    assert_eq!(
        error("{{<base}}{{$x}}y{{/x}}{{! c }}text{{/base}}"),
        "TEMPLATE failed. (Template inclusion is disabled.)"
    );
    assert_eq!(
        error("{{<base}}{{$x}}{{>p}}{{/x}}{{/base}}"),
        "TEMPLATE failed. (Template inclusion is disabled.)"
    );
    let illegal = |body: &str, class: &str| {
        assert_eq!(
            error(&format!("{{{{<base}}}}{body}{{{{/base}}}}")),
            format!(
                "TEMPLATE failed. (Illegal code in extend section: com.github.mustachejava.codes.{class})"
            )
        );
    };
    illegal("{{name}}", "ValueCode");
    illegal("{{$x}}y{{/x}}{{#t}}{{/t}}", "IterableCode");
    illegal("{{^t}}{{/t}}", "NotIterableCode");
    illegal("{{?x}}{{/x}}", "ExtendCheckNameCode");
    illegal("{{%implicit-iterator}}", "DefaultCode");
    illegal("{{>p}}", "PartialCode");
    illegal("{{<other}}{{/other}}", "ExtendCode");
    // Parse errors come first.
    assert_eq!(
        error("{{>inc}}{{#a}}"),
        "TEMPLATE failed. (Failed to close 'a' tag @[:1])"
    );
}

#[test]
fn parse_errors() {
    assert_eq!(error("{{}}"), "TEMPLATE failed. (Empty mustache @[:1])");
    assert_eq!(error("{{"), "TEMPLATE failed. (Empty mustache @[:1])");
    assert_eq!(
        error("{{#t}}{{/f}}"),
        "TEMPLATE failed. (Mismatched start/end tags: t != f in :1 @[:1])"
    );
    assert_eq!(
        error("a\n\n{{/f}}"),
        "TEMPLATE failed. (Mismatched start/end tags: null != f in :3 @[:3])"
    );
    assert_eq!(
        error("x\n{{#t}}\ny"),
        "TEMPLATE failed. (Failed to close 't' tag @[:2])"
    );
    assert_eq!(
        error("{{{name}}"),
        "TEMPLATE failed. (Improperly closed variable in :1 @[:1])"
    );
    assert_eq!(
        error("{{name"),
        "TEMPLATE failed. (Improperly closed variable @[:1])"
    );
    assert_eq!(
        error("{{=<% %> x=}}"),
        "TEMPLATE failed. (Invalid delimiter string: <% %> x @[:1])"
    );
    assert_eq!(
        error("{{=<%=}}"),
        "TEMPLATE failed. (Invalid delimiter string: <% @[:1])"
    );
    assert_eq!(
        error("{{==}}"),
        "TEMPLATE failed. (Invalid delimiter string:  @[:1])"
    );
    assert_eq!(
        error("{{=[ ]=}}[{name}]"),
        "TEMPLATE failed. (Index 1 out of bounds for length 1)"
    );
    assert_eq!(
        error("{{=<% %>=}}<%{%>"),
        "TEMPLATE failed. (Range [0, -1) out of bounds for length 0)"
    );
    assert_eq!(
        error("{{=<% %=}}<%{name}%"),
        "TEMPLATE failed. (Index 1 out of bounds for length 1)"
    );
    // Unterminated tags that do not fail.
    assert_eq!(render("{{#t}}x{{/t"), "x");
    assert_eq!(render("a{{!comment"), "a");
    assert_eq!(render("a{{&name"), "aWorld");
}

#[test]
fn arguments_and_scope_types() {
    assert_fails(
        "'x' 1 TEMPLATE",
        "TEMPLATE expects a map on top of the stack.",
    );
    assert_fails("1 {} TEMPLATE", "TEMPLATE operates on a string.");
    assert_fails("TEMPLATE", "Empty stack.");
    assert_fails("{} TEMPLATE", "Empty stack.");
    let invalid = "TEMPLATE failed. (Invalid type for template substitution.)";
    assert_fails("'x' { 'a' NULL } TEMPLATE", invalid);
    assert_fails("'x' { 'a' [ 1 [ 'b' 'YQ==' B64-> ] ] } TEMPLATE", invalid);
    assert_fails("'x' { 'a' [ 1 ] ->SET } TEMPLATE", invalid);
    // Keys are checked too (a list key is fine, a null key is not).
    assert_eq!(render_with("ok", "{ [ 1 ] 'a' }"), "ok");
    assert_fails("'x' { NULL 'a' } TEMPLATE", invalid);
    assert_fails("'x' { 'a' <% %> } TEMPLATE", invalid);
    assert!(render_template("", Value::Null).is_err());
    assert_eq!(render_template("{{.}}", Value::Long(3)).unwrap(), "3");
    assert_eq!(
        render_template(
            "{{#.}}{{.}}{{/.}}",
            Value::list(vec![Value::Long(1), Value::Long(2)])
        )
        .unwrap(),
        "12"
    );
}
