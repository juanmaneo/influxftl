// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the typography functions. The expectations of the first tests are the answers of
//! Warp 10 3.5.0 to the same scripts: the JSON stack (top first) or the error message (in its
//! JDK 11+ form). The rest check the text settings and the ink on the graphics itself, which the
//! oracle cannot see: its glyphs are not in the JSON.

use std::rc::Rc;

use crate::processing::font::{Family, Font};
use crate::processing::{HAlign, TextMode, TextStyle, VAlign};
use crate::stack::Stack;
use crate::tests::test_stack;
use crate::value::Value;

const G: &str = r"4 4 '2D' PGraphics";

/// The script of a template, its `@G` placeholder replaced by the graphics above.
fn expand(template: &str) -> String {
    template.replace("@G", G)
}

/// The stack left by `script`, top first, as the JSON array of the Warp 10 exec endpoint,
/// or the error message.
fn stack_json(script: &str) -> Result<String, String> {
    let mut stack = test_stack();
    stack
        .exec_checked(&format!("{script} DEPTH ->LIST REVERSE ->JSON"))
        .map_err(|err| err.to_string())?;
    match stack.pop().expect("pop should succeed") {
        Value::String(json) => Ok(json.to_string()),
        other => panic!("not a string: {other:?}"),
    }
}

/// Check every case, reporting all the mismatches at once.
fn check(cases: &[(&str, Result<&str, &str>)]) {
    let mut failures = Vec::new();
    for (template, expected) in cases {
        let script = expand(template);
        match (expected, stack_json(&script)) {
            (Ok(json), Ok(actual)) if actual == *json => {}
            (Err(message), Err(actual)) if actual.contains(message) => {}
            (expected, actual) => failures.push(format!(
                "{template}\n  expected {expected:?}\n  actual   {actual:?}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The stack left by `script`, which must run. `@G` stands for the graphics above.
fn run(script: &str) -> Stack {
    let mut stack = test_stack();
    stack
        .exec_checked(&expand(script))
        .expect("the script should run");
    stack
}

/// The text settings of the graphics on top of the stack left by `script`.
fn settings(script: &str) -> TextStyle {
    match run(script)
        .pop()
        .expect("the graphics should be on the stack")
    {
        Value::Graphics(graphics) => graphics.borrow().style.text.clone(),
        other => panic!("not a PGRAPHICS: {other:?}"),
    }
}

/// The font on top of the stack left by `script`.
fn font_on_top(script: &str) -> Rc<Font> {
    match run(script).pop().expect("the font should be on the stack") {
        Value::Font(font) => font,
        other => panic!("not a PFONT: {other:?}"),
    }
}

/// The name of the font of a text style, `None` for the default font.
fn font_name(style: &TextStyle) -> Option<&str> {
    style.font.as_deref().map(|font| font.name.as_str())
}

/// The number of pixels that are not clear on the graphics on top of the stack left by `script`.
fn inked(script: &str) -> usize {
    match run(script)
        .pop()
        .expect("the graphics should be on the stack")
    {
        Value::Graphics(graphics) => graphics
            .borrow()
            .image
            .pixels
            .iter()
            .filter(|&&pixel| pixel != 0)
            .count(),
        other => panic!("not a PGRAPHICS: {other:?}"),
    }
}

#[test]
fn create_font_answers_a_font_or_null() {
    check(&[
        (r"@G 'Foo' 12 PcreateFont ISNULL", Ok(r"[false,null]")),
        (r"@G 'Foo' PcreateFont ISNULL", Ok(r"[false,null]")),
        (r"@G 'Foo' 0.5 PcreateFont ISNULL", Ok(r"[false,null]")),
        (
            r"@G 'Foo' 12 true 'latin' PcreateFont ISNULL",
            Ok(r"[false,null]"),
        ),
        (r"@G 'Foo' 0 PcreateFont", Ok(r"[null,null]")),
        (r"@G 'Foo' 0 PcreateFont ISNULL", Ok(r"[true,null]")),
        (r"@G 'Foo' 0.4 PcreateFont ISNULL", Ok(r"[true,null]")),
        (r"@G 'Foo' -3 PcreateFont ISNULL", Ok(r"[true,null]")),
        (r"@G 'Foo.TTF' 12 PcreateFont ISNULL", Ok(r"[true,null]")),
        (r"@G 'Foo.ttf' 12 PcreateFont ISNULL", Ok(r"[true,null]")),
        (r"@G 'Foo.OtF' 12 PcreateFont ISNULL", Ok(r"[true,null]")),
        (
            r"@G 'Foo' 12.4 PcreateFont PtextFont PtextAscent",
            Ok(r"[13.0,null]"),
        ),
        (
            r"@G 'https://x' 12 PcreateFont",
            Err(r"Font resolver not set, rejecting all URLs."),
        ),
        (
            r"@G 'http://x' 12 PcreateFont",
            Err(r"Font resolver not set, rejecting all URLs."),
        ),
        (
            r"@G 'file://x' 12 PcreateFont",
            Err(r"Font resolver not set, rejecting all URLs."),
        ),
        (
            r"@G 'Foo' 12 true NULL PcreateFont",
            Err(r"NullPointerException"),
        ),
        (r"@G 'Foo' NULL PcreateFont", Err(r"NullPointerException")),
        (r"@G NULL PcreateFont", Err(r"NullPointerException")),
        (
            r"@G 'Foo' 'x' PcreateFont",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 'Foo' 12 PcreateFont 'Foo' PcreateFont",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 'Foo' 12 true 'latin' 'x' PcreateFont",
            Err(r"Invalid number of parameters, expected one of [1, 2, 3, 4] but found 5"),
        ),
        (
            r"@G PcreateFont",
            Err(r"Invalid number of parameters, expected one of [1, 2, 3, 4] but found 0"),
        ),
    ]);
}

#[test]
fn text_font_selects_a_font_and_sets_the_size_it_gives() {
    check(&[
        (
            r"@G 'Foo' 20 PcreateFont 30 PtextFont PtextAscent",
            Ok(r"[33.0,null]"),
        ),
        (
            r"@G 'Foo' 20 PcreateFont PtextFont PtextAscent",
            Ok(r"[22.0,null]"),
        ),
        (
            r"@G 'Foo' 20 PcreateFont 0 PtextFont PtextAscent",
            Ok(r"[13.0,null]"),
        ),
        (
            r"@G 'Foo' 24 PcreateFont PtextFont 'Hello' PtextWidth",
            Ok(r"[59.0,null]"),
        ),
        (
            r"@G 'Foo' 12 PcreateFont PtextFont 'Hello' PtextWidth",
            Ok(r"[29.0,null]"),
        ),
        (
            r"@G 'Serif' 12 PcreateFont PtextFont 'Hello' PtextWidth",
            Ok(r"[31.0,null]"),
        ),
        (
            r"@G 'Monospaced' 12 PcreateFont PtextFont 'iiii' PtextWidth",
            Ok(r"[28.0,null]"),
        ),
        (
            r"@G 'Foo' 20 PcreateFont 'x' PtextFont",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 'Foo' 12 PcreateFont NULL PtextFont",
            Err(r"NullPointerException"),
        ),
        (
            r"@G NULL PtextFont",
            Err(r"A null PFont was passed to textFont()"),
        ),
        (
            r"@G 'x' PtextFont",
            Err(r"class java.lang.String cannot be cast to class processing.core.PFont"),
        ),
        (
            r"@G PtextFont",
            Err(r"Invalid number of parameters, expected one of [1, 2] but found 0"),
        ),
    ]);
}

#[test]
fn text_size_and_leading_are_set_and_measured() {
    check(&[
        (r"@G 24 PtextSize 'Hello' PtextWidth", Ok(r"[59.0,null]")),
        (r"@G 24 PtextSize PtextAscent", Ok(r"[26.0,null]")),
        (r"@G 24 PtextSize PtextDescent", Ok(r"[7.0,null]")),
        (r"@G 0 PtextSize PtextAscent", Ok(r"[13.0,null]")),
        (r"@G 13.4 PtextSize PtextAscent", Ok(r"[15.0,null]")),
        (r"@G 0.4 PtextSize", Err(r"NullPointerException")),
        (r"@G NULL PtextSize", Err(r"NullPointerException")),
        (
            r"@G 'x' PtextSize",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G PtextSize",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (r"@G 20 PtextLeading", Ok(r"[null]")),
        (
            r"@G 'x' PtextLeading",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G NULL PtextLeading", Err(r"NullPointerException")),
        (
            r"@G PtextLeading",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
    ]);
}

#[test]
fn text_align_takes_a_horizontal_option_and_a_vertical_one() {
    check(&[
        (r"@G 'LEFT' PtextAlign", Ok(r"[null]")),
        (r"@G 'RIGHT' 'TOP' PtextAlign", Ok(r"[null]")),
        (r"@G 'CENTER' 'CENTER' PtextAlign", Ok(r"[null]")),
        (r"@G 'LEFT' 'BASELINE' PtextAlign", Ok(r"[null]")),
        (
            r"@G 'left' PtextAlign",
            Err(r"PtextAlign invalid alignment for X, should be 'LEFT', 'RIGHT' or 'CENTER'."),
        ),
        (
            r"@G 'MIDDLE' PtextAlign",
            Err(r"PtextAlign invalid alignment for X, should be 'LEFT', 'RIGHT' or 'CENTER'."),
        ),
        (
            r"@G 'LEFT' 'MIDDLE' PtextAlign",
            Err(
                r"PtextAlign invalid alignment for Y, should be 'TOP', 'BOTTOM', 'CENTER' or 'BASELINE'.",
            ),
        ),
        (r"@G 'LEFT' NULL PtextAlign", Err(r"NullPointerException")),
        (r"@G NULL PtextAlign", Err(r"NullPointerException")),
        (
            r"@G PtextAlign",
            Err(r"Invalid number of parameters, expected one of [1, 2] but found 0"),
        ),
    ]);
}

#[test]
fn text_mode_takes_model_or_shape() {
    check(&[
        (r"@G 'MODEL' PtextMode", Ok(r"[null]")),
        (r"@G 'SHAPE' PtextMode", Ok(r"[null]")),
        (
            r"@G 'model' PtextMode",
            Err(r"PtextMode invalid mode, should be 'MODEL' or 'SHAPE'."),
        ),
        (
            r"@G 1 PtextMode",
            Err(r"PtextMode invalid mode, should be 'MODEL' or 'SHAPE'."),
        ),
        (
            r"@G NULL PtextMode",
            Err(r"PtextMode invalid mode, should be 'MODEL' or 'SHAPE'."),
        ),
        (
            r"@G PtextMode",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
    ]);
}

#[test]
fn ptext_takes_a_point_a_point_with_a_depth_or_a_box() {
    check(&[
        (r"@G 'Hi' 1 2 Ptext", Ok(r"[null]")),
        (r"@G 'Hi' 1 2 3 Ptext", Ok(r"[null]")),
        (r"@G 'Hi' 1 2 3 4 Ptext", Ok(r"[null]")),
        (r"@G 'Hi' 3e38 0 Ptext", Ok(r"[null]")),
        (r"@G 'Hi' 0 0 -5 -5 Ptext", Ok(r"[null]")),
        (
            r"@G 'Hi' 1 2 3 4 5 Ptext",
            Err(r"Invalid number of parameters, expected one of [3, 4, 5] but found 6"),
        ),
        (
            r"@G Ptext",
            Err(r"Invalid number of parameters, expected one of [3, 4, 5] but found 0"),
        ),
        (r"@G NULL 1 2 Ptext", Err(r"NullPointerException")),
        (
            r"@G 'Hi' 1 'a' Ptext",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 'Hi' 1 2 'a' Ptext",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 'Hi' 1 2 3 'a' Ptext",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 'Hi' 1 2 'a' 4 Ptext",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
    ]);
}

#[test]
fn text_measures_answer_widths_and_heights() {
    check(&[
        (r"@G 'Hello' PtextWidth", Ok(r"[29.0,null]")),
        (r"@G 42 PtextWidth", Ok(r"[14.0,null]")),
        (r"@G '%25' PtextWidth", Ok(r"[10.0,null]")),
        (r"@G 'iiii' PtextWidth", Ok(r"[12.0,null]")),
        (r"@G 'Hi%0Athere' PtextWidth", Ok(r"[30.0,null]")),
        (r"@G '中' PtextWidth", Ok(r"[12.0,null]")),
        (r"@G NULL PtextWidth", Err(r"NullPointerException")),
        (
            r"@G 'Hello' 'x' PtextWidth",
            Err(r"Invalid number of parameters, expected one of [1] but found 2"),
        ),
        (
            r"@G PtextWidth",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (r"@G PtextAscent", Ok(r"[13.0,null]")),
        (r"@G PtextDescent", Ok(r"[4.0,null]")),
        (
            r"@G 1 PtextAscent",
            Err(r"Invalid number of parameters, expected one of [0] but found 1"),
        ),
        (
            r"@G 1 PtextDescent",
            Err(r"Invalid number of parameters, expected one of [0] but found 1"),
        ),
    ]);
}

#[test]
fn a_created_font_keeps_its_name_and_its_size_rounded_half_up() {
    let font = font_on_top("@G 'Serif' 12.5 PcreateFont");
    assert_eq!(font.name, "Serif");
    assert_eq!(font.size, 13.0);
    assert_eq!(font.family(), Family::Serif);
    assert_eq!(font_on_top("@G 'Foo' PcreateFont").size, 12.0);
}

#[test]
fn text_font_selects_the_font_and_takes_its_size_unless_it_is_not_positive() {
    let style = settings("@G 'Foo' 20 PcreateFont PtextFont");
    assert_eq!(font_name(&style), Some("Foo"));
    assert_eq!(style.size, 20.0);

    let style = settings("@G 'Foo' 20 PcreateFont 30 PtextFont");
    assert_eq!(font_name(&style), Some("Foo"));
    assert_eq!(style.size, 30.0);

    let style = settings("@G 'Foo' 20 PcreateFont 0 PtextFont");
    assert_eq!(font_name(&style), Some("Foo"));
    assert_eq!(style.size, 12.0);
}

#[test]
fn text_size_sets_a_default_font_only_when_none_is_set() {
    let style = settings("@G 13.4 PtextSize");
    assert_eq!(style.size, 13.4);
    let font = style.font.expect("a default font should be set");
    assert_eq!((font.name.as_str(), font.size), ("Lucida Sans", 13.0));

    let style = settings("@G 'Serif' 12 PcreateFont PtextFont 20 PtextSize");
    assert_eq!(style.size, 20.0);
    let font = style.font.expect("the font should stay set");
    assert_eq!((font.name.as_str(), font.size), ("Serif", 12.0));
}

#[test]
fn text_size_ignores_a_size_that_is_not_positive() {
    let style = settings("@G -3 PtextSize");
    assert_eq!(style.size, 12.0);
    assert!(style.font.is_none());
}

#[test]
fn text_leading_sets_the_leading() {
    assert_eq!(settings("@G 20 PtextLeading").leading, 20.0);
}

#[test]
fn text_align_sets_the_axes_it_is_given() {
    let align = |args: &str| {
        let style = settings(&format!("@G {args} PtextAlign"));
        (style.align_x, style.align_y)
    };
    assert_eq!(align("'RIGHT'"), (HAlign::Right, VAlign::Baseline));
    assert_eq!(align("'CENTER' 'TOP'"), (HAlign::Center, VAlign::Top));
    assert_eq!(align("'LEFT' 'BOTTOM'"), (HAlign::Left, VAlign::Bottom));
    assert_eq!(align("'RIGHT' 'CENTER'"), (HAlign::Right, VAlign::Center));
    assert_eq!(align("'LEFT' 'BASELINE'"), (HAlign::Left, VAlign::Baseline));
    let style = settings("@G 'CENTER' 'TOP' PtextAlign 'LEFT' PtextAlign");
    assert_eq!((style.align_x, style.align_y), (HAlign::Left, VAlign::Top));
}

#[test]
fn text_mode_sets_the_mode() {
    assert_eq!(settings("@G 'SHAPE' PtextMode").mode, TextMode::Shape);
    assert_eq!(
        settings("@G 'SHAPE' PtextMode 'MODEL' PtextMode").mode,
        TextMode::Model
    );
}

#[test]
fn ptext_draws_the_glyphs_in_the_fill() {
    let clear = inked("8 12 '2D' PGraphics 255 Pfill");
    assert_eq!(inked("8 12 '2D' PGraphics 255 Pfill ' ' 0 10 Ptext"), clear);
    assert!(inked("8 12 '2D' PGraphics 255 Pfill 'H' 0 10 Ptext") > clear);
}

#[test]
fn ptext_in_a_box_draws_its_text_inside_the_box() {
    let clear = inked("8 12 '2D' PGraphics 255 Pfill");
    assert!(inked("8 12 '2D' PGraphics 255 Pfill 'H' 0 10 8 12 Ptext") > clear);
    assert_eq!(
        inked("8 12 '2D' PGraphics 255 Pfill ' ' 0 10 8 12 Ptext"),
        clear
    );
}

#[test]
fn ptext_draws_the_same_glyphs_in_shape_mode_as_in_model_mode() {
    assert_eq!(
        inked("8 12 '2D' PGraphics 255 Pfill 'SHAPE' PtextMode 'H' 0 10 Ptext"),
        inked("8 12 '2D' PGraphics 255 Pfill 'H' 0 10 Ptext"),
    );
}

#[test]
fn ptext_draws_in_the_plane_of_a_context_whatever_its_depth() {
    let clear = inked("8 12 '2D' PGraphics 255 Pfill");
    assert!(inked("8 12 '2D' PGraphics 255 Pfill 'H' 0 10 5 Ptext") > clear);
    let clear = inked("8 12 '3D' PGraphics 255 Pfill");
    assert!(inked("8 12 '3D' PGraphics 255 Pfill 'H' 0 10 5 Ptext") > clear);
}
