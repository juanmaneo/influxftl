// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

use super::*;
use crate::finite::NotFinite;

/// The shape that `text` reads as.
fn read(text: &str) -> Shape {
    parse(text).expect("the document is read")
}

/// The error that reading `text` fails with.
fn error_of(text: &str) -> Error {
    parse(text).expect_err("the document is refused")
}

/// The SVG error that reading `text` fails with.
fn refused(text: &str) -> SvgError {
    match error_of(text) {
        Error::Processing(ProcessingError::Svg(error)) => error,
        other => panic!("not an SVG error: {other:?}"),
    }
}

/// The reason that the XML of `text` is refused for.
fn xml_reason(text: &str) -> &'static str {
    match refused(text) {
        SvgError::Xml { reason } => reason,
        other => panic!("not an XML error: {other:?}"),
    }
}

/// An `svg` root of size 1 by 1 holding `body`.
fn document_with(body: &str) -> String {
    format!(r#"<svg width="1" height="1">{body}</svg>"#)
}

/// The only piece of the document that holds `body`.
fn piece_of(body: &str) -> Piece {
    let mut pieces = read(&document_with(body)).pieces;
    assert_eq!(pieces.len(), 1, "one piece is expected");
    pieces.remove(0)
}

/// The outline of a `path` element with the path data `d`.
fn outline_of(d: &str) -> Outline {
    piece_of(&format!(r#"<path d="{d}"/>"#)).outline
}

/// The document with a `path` element of the path data `d`.
fn path_document(d: &str) -> String {
    document_with(&format!(r#"<path d="{d}"/>"#))
}

/// The SVG error that the path data `d` is refused for.
fn path_refused(d: &str) -> SvgError {
    refused(&path_document(d))
}

/// The document with a `rect` of size 1 by 1 that has the attributes `attributes`.
fn rect_with(attributes: &str) -> String {
    document_with(&format!(r#"<rect width="1" height="1" {attributes}/>"#))
}

/// The smallest and largest corners of the box that holds `points`.
fn bounds(points: &[[f32; 2]]) -> ([f32; 2], [f32; 2]) {
    points.iter().fold(
        ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]),
        |(low, high), p| {
            (
                [low[0].min(p[0]), low[1].min(p[1])],
                [high[0].max(p[0]), high[1].max(p[1])],
            )
        },
    )
}

/// Whether `a` and `b` are the same point, to within rounding.
fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4
}

#[test]
fn the_prolog_comments_and_doctype_are_skipped() {
    let shape = read(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- a comment -->
<!DOCTYPE svg>
<svg width="2" height="3"></svg>"#,
    );
    assert_eq!(
        shape,
        Shape {
            width: 2.0,
            height: 3.0,
            pieces: Vec::new(),
        }
    );
}

#[test]
fn a_construct_that_is_not_closed_is_refused() {
    let not_closed = SvgError::Xml {
        reason: "a construct that is not closed",
    };
    assert_eq!(refused(r#"<?xml version="1.0""#), not_closed);
    assert_eq!(
        refused(r#"<svg width="1" height="1"/><!-- open"#),
        not_closed
    );
    assert_eq!(refused("<!DOCTYPE svg"), not_closed);
}

#[test]
fn whitespace_and_text_inside_the_root_are_ignored() {
    let shape = read("<svg width=\"1\" height=\"1\">\n  text\n  <g>  </g>\n</svg >");
    assert_eq!(shape.pieces.len(), 0);
}

#[test]
fn text_outside_the_root_is_refused() {
    let outside = "content outside the root element";
    assert_eq!(xml_reason(r#"text <svg width="1" height="1"/>"#), outside);
    assert_eq!(xml_reason(r#"<svg width="1" height="1"/>tail"#), outside);
}

#[test]
fn end_tags_close_the_element_that_is_open() {
    assert_eq!(
        xml_reason(r#"<svg width="1" height="1"></g>"#),
        "an end tag that does not match its start tag"
    );
    assert_eq!(xml_reason("</svg>"), "an end tag with no start tag");
    assert_eq!(
        xml_reason(r#"<svg width="1" height="1"></svg"#),
        "an end tag not closed by '>'"
    );
    assert_eq!(
        xml_reason(r#"<svg width="1" height="1">"#),
        "an element that is not closed"
    );
    assert_eq!(
        xml_reason(r#"<svg width="1" height="1"></>"#),
        "a name expected"
    );
    assert_eq!(xml_reason("<>"), "a name expected");
}

#[test]
fn a_document_has_exactly_one_root() {
    assert_eq!(xml_reason(""), "no root element");
    assert_eq!(xml_reason("  \n "), "no root element");
    assert_eq!(xml_reason("<svg/><svg/>"), "content after the root element");
}

#[test]
fn attributes_are_read_in_either_quote_and_must_be_complete() {
    let shape = read("<svg width='3' height='2'/>");
    assert_eq!((shape.width, shape.height), (3.0, 2.0));
    assert_eq!(
        xml_reason(r#"<svg width height="1"/>"#),
        "an attribute with no value"
    );
    assert_eq!(
        xml_reason("<svg width=1/>"),
        "an attribute value that is not quoted"
    );
    assert_eq!(
        xml_reason(r#"<svg width="1/>"#),
        "an attribute value that is not closed"
    );
}

#[test]
fn sizes_are_read_with_their_px_unit_or_from_the_view_box() {
    let px = read(r#"<svg width="4px" height="2px"/>"#);
    assert_eq!((px.width, px.height), (4.0, 2.0));
    let viewed = read(r#"<svg viewBox="0 0 8 4"><rect width="4" height="2"/></svg>"#);
    assert_eq!((viewed.width, viewed.height), (8.0, 4.0));
    let default = read("<svg/>");
    assert_eq!((default.width, default.height), (1.0, 1.0));
}

#[test]
fn malformed_sizes_are_refused() {
    assert_eq!(
        refused(r#"<svg viewBox="0 0 8"/>"#),
        SvgError::Malformed {
            attribute: "viewBox",
            reason: "four numbers are needed",
        }
    );
    assert!(matches!(
        refused(r#"<svg width="thick" height="1"/>"#),
        SvgError::Number {
            attribute: "width",
            ..
        }
    ));
    assert_eq!(
        error_of(r#"<svg width="inf" height="1"/>"#),
        Error::NotFinite(NotFinite::PositiveInfinity)
    );
    assert_eq!(
        error_of(r#"<svg width="NaN" height="1"/>"#),
        Error::NotFinite(NotFinite::Nan)
    );
}

#[test]
fn a_view_box_that_cannot_scale_the_content_is_refused() {
    assert_eq!(
        error_of(r#"<svg width="1" height="1" viewBox="0 0 0 1"/>"#),
        Error::NotFinite(NotFinite::PositiveInfinity)
    );
    assert_eq!(
        error_of(r#"<svg width="1" height="1" viewBox="0 0 1 0"/>"#),
        Error::NotFinite(NotFinite::PositiveInfinity)
    );
    assert_eq!(
        error_of(r#"<svg viewBox="0 0 0 0"/>"#),
        Error::NotFinite(NotFinite::Nan)
    );
}

#[test]
fn a_scaled_view_box_origin_or_point_that_overflows_is_refused() {
    assert_eq!(
        error_of(r#"<svg width="2" height="1" viewBox="3e38 0 1e-30 1"/>"#),
        Error::NotFinite(NotFinite::NegativeInfinity)
    );
    assert_eq!(
        error_of(r#"<svg width="1" height="2" viewBox="0 3e38 1 1e-30"/>"#),
        Error::NotFinite(NotFinite::NegativeInfinity)
    );
    assert_eq!(
        error_of(
            r#"<svg width="4e30" height="1" viewBox="0 0 1 1"><rect x="1e38" y="0" width="1" height="1"/></svg>"#
        ),
        Error::NotFinite(NotFinite::PositiveInfinity)
    );
}

#[test]
fn paint_is_inherited_from_the_groups_and_set_by_the_element() {
    let pieces = read(
        r##"<svg width="1" height="1"><g fill="#abc" stroke="white" stroke-width="2">
<rect width="1" height="1"/>
<rect width="1" height="1" fill="none"/>
<rect width="1" height="1" fill="black" stroke="#123456" stroke-width="0.5"/>
</g></svg>"##,
    )
    .pieces;
    let paint = |piece: &Piece| (piece.fill, piece.stroke, piece.stroke_weight);
    assert_eq!(pieces.len(), 3);
    assert_eq!(
        paint(&pieces[0]),
        (Some(0xFFAA_BBCC), Some(0xFFFF_FFFF), 2.0)
    );
    assert_eq!(paint(&pieces[1]), (None, Some(0xFFFF_FFFF), 2.0));
    assert_eq!(
        paint(&pieces[2]),
        (Some(0xFF00_0000), Some(0xFF12_3456), 0.5)
    );
}

#[test]
fn an_element_without_paint_is_filled_black_and_not_stroked() {
    let piece = piece_of(r#"<rect width="1" height="1"/>"#);
    assert_eq!(
        (piece.fill, piece.stroke, piece.stroke_weight),
        (Some(0xFF00_0000), None, 1.0)
    );
}

#[test]
fn a_colour_or_stroke_weight_that_is_not_valid_is_refused() {
    assert_eq!(
        refused(&rect_with(r#"fill="red""#)),
        SvgError::Colour {
            attribute: "fill",
            text: "red".to_owned(),
        }
    );
    assert_eq!(
        refused(&rect_with(r##"stroke="#abcd""##)),
        SvgError::Colour {
            attribute: "stroke",
            text: "#abcd".to_owned(),
        }
    );
    assert_eq!(
        refused(&rect_with(r##"stroke="#zzz""##)),
        SvgError::Colour {
            attribute: "stroke",
            text: "#zzz".to_owned(),
        }
    );
    assert!(matches!(
        refused(&rect_with(r#"stroke-width="thick""#)),
        SvgError::Number {
            attribute: "stroke-width",
            ..
        }
    ));
}

#[test]
fn a_rect_starts_at_the_origin_when_it_has_no_position() {
    let piece = piece_of(r#"<rect width="2" height="3"/>"#);
    assert_eq!(bounds(&piece.outline.fill[0]), ([0.0, 0.0], [2.0, 3.0]));
    assert!(piece.outline.stroke[0].closed);
}

#[test]
fn a_rect_needs_its_size_and_must_be_representable() {
    assert_eq!(
        refused(r#"<svg><rect width="2"/></svg>"#),
        SvgError::Missing {
            attribute: "height"
        }
    );
    assert_eq!(
        refused(r#"<svg><rect height="2"/></svg>"#),
        SvgError::Missing { attribute: "width" }
    );
    assert_eq!(
        error_of(r#"<svg><rect x="3e38" width="3e38" height="1"/></svg>"#),
        Error::NotFinite(NotFinite::PositiveInfinity)
    );
}

#[test]
fn a_circle_is_a_ring_of_its_radius_about_its_centre() {
    let circle = piece_of(r#"<circle cx="2" cy="2" r="1"/>"#);
    assert!(
        circle.outline.fill[0]
            .iter()
            .all(|p| ((p[0] - 2.0).hypot(p[1] - 2.0) - 1.0).abs() < 1e-4)
    );
    let at_origin = piece_of(r#"<circle r="1"/>"#);
    assert!(
        at_origin.outline.fill[0]
            .iter()
            .all(|p| (p[0].hypot(p[1]) - 1.0).abs() < 1e-4)
    );
    assert_eq!(
        refused(r#"<svg><circle cx="1"/></svg>"#),
        SvgError::Missing { attribute: "r" }
    );
}

#[test]
fn a_circle_whose_extent_overflows_is_refused() {
    assert_eq!(
        error_of(r#"<svg><circle cx="-3e38" r="3e38"/></svg>"#),
        Error::NotFinite(NotFinite::NegativeInfinity)
    );
    assert_eq!(
        error_of(r#"<svg><circle cy="-3e38" r="3e38"/></svg>"#),
        Error::NotFinite(NotFinite::NegativeInfinity)
    );
    assert_eq!(
        error_of(r#"<svg><circle r="2e38"/></svg>"#),
        Error::NotFinite(NotFinite::PositiveInfinity)
    );
    assert_eq!(
        error_of(r#"<svg><circle r="1e38"/></svg>"#),
        Error::NotFinite(NotFinite::PositiveInfinity)
    );
}

#[test]
fn an_ellipse_is_read_from_its_radii() {
    let ellipse = piece_of(r#"<ellipse cx="2" cy="1" rx="2" ry="1"/>"#);
    assert!(
        ellipse.outline.fill[0]
            .iter()
            .all(|p| { (((p[0] - 2.0) / 2.0).powi(2) + (p[1] - 1.0).powi(2) - 1.0).abs() < 1e-4 })
    );
    assert_eq!(
        refused(r#"<svg><ellipse rx="1"/></svg>"#),
        SvgError::Missing { attribute: "ry" }
    );
    assert_eq!(
        refused(r#"<svg><ellipse ry="1"/></svg>"#),
        SvgError::Missing { attribute: "rx" }
    );
    assert_eq!(
        error_of(r#"<svg><ellipse rx="1" ry="2e38"/></svg>"#),
        Error::NotFinite(NotFinite::PositiveInfinity)
    );
}

#[test]
fn a_line_is_an_open_stroke_between_its_ends() {
    let piece = piece_of(r#"<line x1="0" y1="1" x2="4" y2="5"/>"#);
    assert_eq!(
        piece.outline,
        Outline {
            stroke: vec![Subpath {
                points: vec![[0.0, 1.0], [4.0, 5.0]],
                closed: false,
            }],
            ..Outline::default()
        }
    );
}

#[test]
fn polylines_and_polygons_read_their_points() {
    assert_eq!(
        piece_of(r#"<polyline points="0,0 4,0 4,4"/>"#).outline,
        Outline::open(vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0]])
    );
    assert_eq!(
        piece_of(r#"<polygon points="0 0 4 0 4 4"/>"#).outline,
        Outline::closed(vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0]])
    );
}

#[test]
fn points_must_be_present_and_come_in_pairs() {
    assert_eq!(
        refused(r#"<svg><polygon points="0 0 4"/></svg>"#),
        SvgError::Malformed {
            attribute: "points",
            reason: "an odd number of coordinates",
        }
    );
    assert_eq!(
        refused(r"<svg><polyline/></svg>"),
        SvgError::Missing {
            attribute: "points"
        }
    );
    assert!(matches!(
        refused(r#"<svg><polyline points="0 x"/></svg>"#),
        SvgError::Number {
            attribute: "points",
            ..
        }
    ));
}

#[test]
fn elements_that_are_not_shapes_are_skipped() {
    let shape = read(
        r#"<svg width="1" height="1"><text>hi</text><title>x</title><g><rect width="1" height="1"/></g></svg>"#,
    );
    assert_eq!(shape.pieces.len(), 1);
}

#[test]
fn a_path_element_needs_its_data() {
    assert_eq!(
        refused(r"<svg><path/></svg>"),
        SvgError::Missing { attribute: "d" }
    );
}

#[test]
fn absolute_and_relative_lines_reach_the_same_points() {
    let outline = outline_of("M1 1 L5 1 l0 4 h-4 v-4 z");
    let points = vec![[1.0, 1.0], [5.0, 1.0], [5.0, 5.0], [1.0, 5.0], [1.0, 1.0]];
    assert_eq!(
        outline.stroke,
        vec![Subpath {
            points: points.clone(),
            closed: true,
        }]
    );
    assert_eq!(outline.fill, vec![points]);
}

#[test]
fn horizontal_and_vertical_lines_close_a_square() {
    let outline = outline_of("M0 0 H4 V4 H0 Z");
    assert_eq!(
        outline.stroke,
        vec![Subpath {
            points: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
            closed: true,
        }]
    );
}

#[test]
fn numbers_after_a_command_repeat_it() {
    let line = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0]];
    assert_eq!(outline_of("M0 0 4 0 4 4").stroke[0].points, line);
    assert_eq!(outline_of("M0 0 L4 0 4 4").stroke[0].points, line);
    assert_eq!(
        outline_of("m1 1 2 0 0 2").stroke[0].points,
        vec![[1.0, 1.0], [3.0, 1.0], [3.0, 3.0]]
    );
}

#[test]
fn a_move_starts_a_subpath_and_a_command_after_a_close_starts_at_its_start() {
    let moved = outline_of("M0 0 L1 0 M5 5 L6 6");
    assert_eq!(moved.stroke.len(), 2);
    assert_eq!(moved.stroke[1].points, vec![[5.0, 5.0], [6.0, 6.0]]);
    let closed = outline_of("M0 0 L4 0 L4 4 Z L8 8");
    assert_eq!(closed.stroke.len(), 2);
    assert!(closed.stroke[0].closed);
    assert_eq!(
        closed.stroke[1],
        Subpath {
            points: vec![[0.0, 0.0], [8.0, 8.0]],
            closed: false,
        }
    );
}

#[test]
fn curves_pass_through_their_midpoints_and_end_on_their_anchors() {
    // The midpoint of the cubic is at t = 1/2, which is (2, 3) for these control points.
    let cubic = outline_of("M0 0 C0 4 4 4 4 0").stroke[0].points.clone();
    assert_eq!(cubic.len(), 21);
    assert!(near(cubic[10], [2.0, 3.0]));
    assert_eq!(cubic[20], [4.0, 0.0]);
    // A quadratic has the same shape as the cubic with its control point two thirds along.
    let quadratic = outline_of("M0 0 Q2 4 4 0").stroke[0].points.clone();
    assert_eq!(quadratic.len(), 21);
    assert!(near(quadratic[10], [2.0, 2.0]));
    assert_eq!(quadratic[20], [4.0, 0.0]);
}

#[test]
fn numbers_read_decimals_exponents_and_signs() {
    // `1.5.5` is the two numbers 1.5 and .5.
    assert_eq!(
        outline_of("M1.5.5 1e1 2E+1 3e-1 4").stroke[0].points,
        vec![[1.5, 0.5], [10.0, 20.0], [0.3, 4.0]]
    );
}

#[test]
fn path_data_that_is_malformed_is_refused() {
    assert_eq!(
        path_refused("1 2"),
        SvgError::Malformed {
            attribute: "d",
            reason: "a number before any command",
        }
    );
    assert_eq!(
        path_refused("L1 1"),
        SvgError::Malformed {
            attribute: "d",
            reason: "a path that does not begin with a move",
        }
    );
    assert_eq!(
        path_refused("M0 0 L1"),
        SvgError::Malformed {
            attribute: "d",
            reason: "a command with too few numbers",
        }
    );
    assert_eq!(
        path_refused("M0 0 #"),
        SvgError::Malformed {
            attribute: "d",
            reason: "a character that is neither a command nor a number",
        }
    );
    assert!(matches!(
        path_refused("M- 1"),
        SvgError::Number { attribute: "d", .. }
    ));
    // A curve before any move has no subpath to join, so it is refused as a line is.
    assert!(matches!(
        path_refused("C0 4 4 4 4 0"),
        SvgError::Malformed { attribute: "d", .. }
    ));
    assert!(matches!(
        path_refused("Q2 4 4 0"),
        SvgError::Malformed { attribute: "d", .. }
    ));
}

#[test]
fn commands_that_are_not_read_are_refused() {
    assert_eq!(
        path_refused("M0 0 S1 1 2 2"),
        SvgError::PathCommand { command: 'S' }
    );
    assert_eq!(
        path_refused("M0 0 A1 1 0 0 1 2 2"),
        SvgError::PathCommand { command: 'A' }
    );
    assert_eq!(
        path_refused("M0 0 T1 1"),
        SvgError::PathCommand { command: 'T' }
    );
    assert!(matches!(
        path_refused("M0 0 s1 1 2 2"),
        SvgError::PathCommand { .. }
    ));
    // An exponent with no digits is not part of its number, so its `e` is read as a command.
    assert!(matches!(
        path_refused("M0 0 H5e"),
        SvgError::PathCommand { .. }
    ));
}

#[test]
fn path_coordinates_that_overflow_are_refused() {
    let overflow = Error::NotFinite(NotFinite::PositiveInfinity);
    let negative = Error::NotFinite(NotFinite::NegativeInfinity);
    assert_eq!(error_of(&path_document("M1e39 0")), overflow);
    assert_eq!(error_of(&path_document("M3e38 0 l3e38 0")), overflow);
    assert_eq!(error_of(&path_document("M0 3e38 l0 3e38")), overflow);
    assert_eq!(error_of(&path_document("M3e38 0 m3e38 0")), overflow);
    assert_eq!(error_of(&path_document("M3e38 0 h3e38")), overflow);
    assert_eq!(error_of(&path_document("M0 3e38 v3e38")), overflow);
    assert_eq!(
        error_of(&path_document("M3e38 0 c3e38 0 0 0 0 0")),
        overflow
    );
    assert_eq!(
        error_of(&path_document("M3e38 0 c0 0 3e38 0 0 0")),
        overflow
    );
    assert_eq!(
        error_of(&path_document("M3e38 0 c0 0 0 0 3e38 0")),
        overflow
    );
    assert_eq!(error_of(&path_document("M3e38 0 q3e38 0 0 0")), overflow);
    assert_eq!(error_of(&path_document("M3e38 0 q0 0 3e38 0")), overflow);
    // A control value of 2e38 or more, tripled by the Bernstein terms of a curve, overflows.
    assert_eq!(
        error_of(&path_document("M0 0 C2e38 0 2e38 0 2e38 0")),
        overflow
    );
    assert_eq!(error_of(&path_document("M0 0 Q-3e38 0 0 0")), negative);
    // A quadratic's cubic has its control points two thirds of the way to its control point, so
    // the span from a point to a far control point overflows before any curve is drawn.
    assert_eq!(error_of(&path_document("M-3e38 0 Q3e38 0 0 0")), overflow);
    assert_eq!(error_of(&path_document("M0 0 Q3e38 0 -3e38 0")), overflow);
}
