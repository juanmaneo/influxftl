// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! SVG, read as `loadShape` reads a shape file: the `svg` root, its groups, the basic shapes
//! (`rect`, `circle`, `ellipse`, `line`, `polyline`, `polygon`) and paths. The fill and the stroke
//! of each piece come from its `fill`, `stroke` and `stroke-width` attributes, inherited from the
//! groups around it, with SVG's defaults: a black fill and no stroke.
//!
//! What is not read is not drawn: other elements, transforms, `style` attributes and gradients
//! are ignored, and the path commands `S`, `T` and `A` are refused (see `DIVERGENCES.md`).

use super::curve::bezier_steps;
use super::draw::Outline;
use super::finite;
use super::path::Subpath;
use super::primitive;
use super::recording::two_thirds_toward;
use super::shape::{Piece, Shape, map_outline};
use crate::error::processing::ProcessingError;
use crate::error::{Error, Result};

/// The steps each cubic curve of a path is drawn in.
const CURVE_STEPS: usize = 20;

/// A failure to read an SVG document. The text is the part of the document that could not be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SvgError {
    /// The XML is not well formed.
    #[error("malformed XML: {reason}")]
    Xml {
        /// What is wrong with the XML.
        reason: &'static str,
    },
    /// The root element is not `svg`.
    #[error("the root element is <{found}>, not <svg>")]
    NotSvg {
        /// The name of the root element.
        found: String,
    },
    /// An element lacks an attribute it needs.
    #[error("missing attribute {attribute}")]
    Missing {
        /// The attribute that is missing.
        attribute: &'static str,
    },
    /// An attribute that is not a number.
    #[error("{attribute} is not a number: {text:?}")]
    Number {
        /// The attribute read.
        attribute: &'static str,
        /// The text read.
        text: String,
        /// Why the text is not a number.
        #[source]
        cause: std::num::ParseFloatError,
    },
    /// A colour that is neither `none`, `black`, `white` nor a hexadecimal colour.
    #[error("{attribute} is not a colour: {text:?}")]
    Colour {
        /// The attribute read.
        attribute: &'static str,
        /// The text read.
        text: String,
    },
    /// An attribute whose value does not follow its syntax.
    #[error("malformed {attribute}: {reason}")]
    Malformed {
        /// The attribute read.
        attribute: &'static str,
        /// What is wrong with its value.
        reason: &'static str,
    },
    /// A path command that is not read.
    #[error("the path command {command} is not supported")]
    PathCommand {
        /// The command, as the path spells it.
        command: char,
    },
}

/// Read an SVG document into a shape.
pub fn parse(text: &str) -> Result<Shape> {
    let root = document(text)?;
    if root.name != "svg" {
        return Err(failure(SvgError::NotSvg { found: root.name }));
    }
    let (width, height, view) = size(&root)?;
    let mut pieces = Vec::new();
    collect(&root, Presentation::default(), &mut pieces)?;
    if let Some([min_x, min_y, view_width, view_height]) = view {
        let scale = (finite(width / view_width)?, finite(height / view_height)?);
        let origin = [finite(-min_x * scale.0)?, finite(-min_y * scale.1)?];
        for piece in &mut pieces {
            piece.outline = map_outline(&piece.outline, scale, origin)?;
        }
    }
    Ok(Shape {
        width,
        height,
        pieces,
    })
}

/// A failure of the SVG reader, as a Processing failure.
fn failure(error: SvgError) -> Error {
    ProcessingError::Svg(error).into()
}

/// A failure of the XML syntax.
fn xml(reason: &'static str) -> Error {
    failure(SvgError::Xml { reason })
}

/// An element of the document: its name, its attributes in order, and its children.
#[derive(Debug)]
struct Element {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<Element>,
}

impl Element {
    /// The value of the attribute `name`, if the element has it.
    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Parse the elements of the document. Comments, processing instructions and the document type
/// are skipped; text is only allowed inside the root element.
fn document(text: &str) -> Result<Element> {
    let mut rest = text;
    let mut open: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<?") {
            rest = after_marker(after, "?>")?;
        } else if let Some(after) = rest.strip_prefix("<!--") {
            rest = after_marker(after, "-->")?;
        } else if rest.starts_with("<!") {
            rest = after_marker(rest, ">")?;
        } else if let Some(after) = rest.strip_prefix("</") {
            let (name, after) = name(after)?;
            let after = after
                .trim_start()
                .strip_prefix('>')
                .ok_or_else(|| xml("an end tag not closed by '>'"))?;
            let element = open
                .pop()
                .ok_or_else(|| xml("an end tag with no start tag"))?;
            if element.name != name {
                return Err(xml("an end tag that does not match its start tag"));
            }
            attach(&mut open, &mut root, element)?;
            rest = after;
        } else if let Some(after) = rest.strip_prefix('<') {
            let (name, after) = name(after)?;
            let (attributes, after, empty) = attributes(after)?;
            let element = Element {
                name: name.to_owned(),
                attributes,
                children: Vec::new(),
            };
            if empty {
                attach(&mut open, &mut root, element)?;
            } else {
                open.push(element);
            }
            rest = after;
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            let (content, after) = rest.split_at(end);
            if open.is_empty() && !content.trim().is_empty() {
                return Err(xml("content outside the root element"));
            }
            rest = after;
        }
    }
    if !open.is_empty() {
        return Err(xml("an element that is not closed"));
    }
    root.ok_or_else(|| xml("no root element"))
}

/// The text after the first `marker`, which closes the construct that `text` starts in.
fn after_marker<'a>(text: &'a str, marker: &str) -> Result<&'a str> {
    text.split_once(marker)
        .map(|(_, after)| after)
        .ok_or_else(|| xml("a construct that is not closed"))
}

/// A name: the text up to a space, `=`, `/` or `>`. An empty name is an error.
fn name(text: &str) -> Result<(&str, &str)> {
    let end = text
        .find(|c: char| c.is_whitespace() || matches!(c, '=' | '/' | '>'))
        .unwrap_or(text.len());
    let (name, after) = text.split_at(end);
    if name.is_empty() {
        return Err(xml("a name expected"));
    }
    Ok((name, after))
}

/// The attributes of an element: name and value, in the order of the document.
type Attributes = Vec<(String, String)>;

/// The attributes of a start tag, up to its end. The flag tells whether the tag is empty
/// (`<name/>`), and the text after the tag is returned.
fn attributes(text: &str) -> Result<(Attributes, &str, bool)> {
    let mut attributes = Vec::new();
    let mut rest = text;
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("/>") {
            return Ok((attributes, after, true));
        }
        if let Some(after) = rest.strip_prefix('>') {
            return Ok((attributes, after, false));
        }
        let (key, after) = name(rest)?;
        let after = after
            .trim_start()
            .strip_prefix('=')
            .ok_or_else(|| xml("an attribute with no value"))?
            .trim_start();
        let quote = ['"', '\''].into_iter().find(|&q| after.starts_with(q));
        let quote = quote.ok_or_else(|| xml("an attribute value that is not quoted"))?;
        let body = after
            .strip_prefix(quote)
            .ok_or_else(|| xml("an attribute value that is not quoted"))?;
        let (value, after) = body
            .split_once(quote)
            .ok_or_else(|| xml("an attribute value that is not closed"))?;
        attributes.push((key.to_owned(), value.to_owned()));
        rest = after;
    }
}

/// Add a finished element to its parent, or make it the root when no element is open.
fn attach(open: &mut [Element], root: &mut Option<Element>, element: Element) -> Result<()> {
    if let Some(parent) = open.last_mut() {
        parent.children.push(element);
    } else if root.is_none() {
        *root = Some(element);
    } else {
        return Err(xml("content after the root element"));
    }
    Ok(())
}

/// The width and height of the document, with its `viewBox` if it has one. Both come from the
/// `width` and `height` attributes; when one is missing, from the `viewBox`; when there is no
/// `viewBox` either, the shape is 1 by 1, as Processing reads it. The `viewBox` is returned as
/// its four numbers, which map the content onto the size.
fn size(root: &Element) -> Result<(f32, f32, Option<[f32; 4]>)> {
    let view = match root.attribute("viewBox") {
        Some(text) => match numbers("viewBox", text)?.as_slice() {
            &[min_x, min_y, width, height] => Some([min_x, min_y, width, height]),
            _ => {
                return Err(failure(SvgError::Malformed {
                    attribute: "viewBox",
                    reason: "four numbers are needed",
                }));
            }
        },
        None => None,
    };
    let attribute = |name: &'static str| {
        root.attribute(name)
            .map(|text| length(name, text))
            .transpose()
    };
    let (width, height) = match (attribute("width")?, attribute("height")?, view) {
        (Some(width), Some(height), _) | (_, _, Some([_, _, width, height])) => (width, height),
        _ => (1.0, 1.0),
    };
    Ok((width, height, view))
}

/// A number of a document, which must be finite.
fn number(attribute: &'static str, text: &str) -> Result<f32> {
    let value = text.trim().parse::<f32>().map_err(|cause| {
        failure(SvgError::Number {
            attribute,
            text: text.to_owned(),
            cause,
        })
    })?;
    finite(value)
}

/// A length, which is a number of user units with an optional `px`.
fn length(attribute: &'static str, text: &str) -> Result<f32> {
    let trimmed = text.trim();
    number(attribute, trimmed.strip_suffix("px").unwrap_or(trimmed))
}

/// The numbers of a list separated by spaces or commas.
fn numbers(attribute: &'static str, text: &str) -> Result<Vec<f32>> {
    text.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|part| !part.is_empty())
        .map(|part| number(attribute, part))
        .collect()
}

/// A required length of an element.
fn required(element: &Element, attribute: &'static str) -> Result<f32> {
    let text = element
        .attribute(attribute)
        .ok_or_else(|| failure(SvgError::Missing { attribute }))?;
    length(attribute, text)
}

/// A length of an element, `0` when it is not given.
fn optional(element: &Element, attribute: &'static str) -> Result<f32> {
    match element.attribute(attribute) {
        Some(text) => length(attribute, text),
        None => Ok(0.0),
    }
}

/// A colour: `none` is no paint, otherwise `black`, `white` or a hexadecimal `#rgb` or `#rrggbb`,
/// opaque. The result is ARGB.
fn colour(attribute: &'static str, text: &str) -> Result<Option<u32>> {
    let text = text.trim();
    let rgb = match text {
        "none" => return Ok(None),
        "black" => 0x00_0000,
        "white" => 0xFF_FFFF,
        _ => hex_colour(text).ok_or_else(|| {
            failure(SvgError::Colour {
                attribute,
                text: text.to_owned(),
            })
        })?,
    };
    Ok(Some(0xFF00_0000 | rgb))
}

/// The RGB value of a hexadecimal colour, `#rgb` or `#rrggbb`.
fn hex_colour(text: &str) -> Option<u32> {
    let digits = text.strip_prefix('#')?;
    let nibbles = digits
        .chars()
        .map(|c| c.to_digit(16))
        .collect::<Option<Vec<u32>>>()?;
    match nibbles[..] {
        [r, g, b] => Some(((r * 0x11) << 16) | ((g * 0x11) << 8) | (b * 0x11)),
        [r1, r2, g1, g2, b1, b2] => {
            Some(((r1 * 16 + r2) << 16) | ((g1 * 16 + g2) << 8) | (b1 * 16 + b2))
        }
        _ => None,
    }
}

/// The paint an element inherits from its groups, or sets itself.
#[derive(Clone, Copy, Debug)]
struct Presentation {
    fill: Option<u32>,
    stroke: Option<u32>,
    stroke_weight: f32,
}

impl Default for Presentation {
    fn default() -> Self {
        Presentation {
            fill: Some(0xFF00_0000),
            stroke: None,
            stroke_weight: 1.0,
        }
    }
}

/// The paint of an element: what its attributes set, over what it inherits.
fn presentation(element: &Element, inherited: Presentation) -> Result<Presentation> {
    let mut paint = inherited;
    if let Some(text) = element.attribute("fill") {
        paint.fill = colour("fill", text)?;
    }
    if let Some(text) = element.attribute("stroke") {
        paint.stroke = colour("stroke", text)?;
    }
    if let Some(text) = element.attribute("stroke-width") {
        paint.stroke_weight = length("stroke-width", text)?;
    }
    Ok(paint)
}

/// A piece of the shape with the paint of the element it comes from.
fn piece(outline: Outline, paint: Presentation) -> Piece {
    Piece {
        outline,
        fill: paint.fill,
        stroke: paint.stroke,
        stroke_weight: paint.stroke_weight,
    }
}

/// Add the pieces of an element, and of its children when it is a group, to `pieces`.
fn collect(element: &Element, inherited: Presentation, pieces: &mut Vec<Piece>) -> Result<()> {
    let paint = presentation(element, inherited)?;
    let outline = match element.name.as_str() {
        "svg" | "g" => {
            for child in &element.children {
                collect(child, paint, pieces)?;
            }
            return Ok(());
        }
        "rect" => {
            let (x, y) = (optional(element, "x")?, optional(element, "y")?);
            let width = required(element, "width")?;
            let height = required(element, "height")?;
            primitive::rect(x, y, width, height, [0.0; 4])?
        }
        "circle" => {
            let (cx, cy) = (optional(element, "cx")?, optional(element, "cy")?);
            let radius = required(element, "r")?;
            centred_ellipse(cx, cy, radius, radius)?
        }
        "ellipse" => {
            let (cx, cy) = (optional(element, "cx")?, optional(element, "cy")?);
            let (rx, ry) = (required(element, "rx")?, required(element, "ry")?);
            centred_ellipse(cx, cy, rx, ry)?
        }
        "line" => {
            let from = [optional(element, "x1")?, optional(element, "y1")?];
            let to = [optional(element, "x2")?, optional(element, "y2")?];
            primitive::line(from, to)?
        }
        "polyline" => Outline::open(points(element)?),
        "polygon" => Outline::closed(points(element)?),
        "path" => {
            let text = element
                .attribute("d")
                .ok_or_else(|| failure(SvgError::Missing { attribute: "d" }))?;
            path(text)?
        }
        _ => return Ok(()),
    };
    pieces.push(piece(outline, paint));
    Ok(())
}

/// The ellipse of centre (`cx`, `cy`) and radii (`rx`, `ry`).
fn centred_ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Result<Outline> {
    let x = finite(cx - rx)?;
    let y = finite(cy - ry)?;
    primitive::ellipse(x, y, finite(2.0 * rx)?, finite(2.0 * ry)?)
}

/// The points of a `points` list, as pairs of coordinates.
fn points(element: &Element) -> Result<Vec<[f32; 2]>> {
    let text = element.attribute("points").ok_or_else(|| {
        failure(SvgError::Missing {
            attribute: "points",
        })
    })?;
    let coordinates = numbers("points", text)?;
    let (pairs, rest) = coordinates.as_chunks::<2>();
    if !rest.is_empty() {
        return Err(failure(SvgError::Malformed {
            attribute: "points",
            reason: "an odd number of coordinates",
        }));
    }
    Ok(pairs.to_vec())
}

/// A token of path data: a command letter, or a number.
#[derive(Clone, Copy, Debug)]
enum Token {
    Command(char),
    Number(f32),
}

/// The tokens of path data. Separators are spaces and commas; a number is read as far as its
/// syntax goes, so that `1.5.5` is two numbers.
fn tokens(text: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut rest = text;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == ',');
        let Some(first) = rest.chars().next() else {
            return Ok(tokens);
        };
        if first.is_ascii_alphabetic() {
            let (_, after) = rest.split_at(first.len_utf8());
            tokens.push(Token::Command(first));
            rest = after;
        } else if first.is_ascii_digit() || matches!(first, '+' | '-' | '.') {
            let (number_text, after) = rest.split_at(number_length(rest));
            tokens.push(Token::Number(number("d", number_text)?));
            rest = after;
        } else {
            return Err(failure(SvgError::Malformed {
                attribute: "d",
                reason: "a character that is neither a command nor a number",
            }));
        }
    }
}

/// The length of the number that starts `text`, by the syntax of SVG numbers.
fn number_length(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut end = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let mut digits = false;
    let mut dot = false;
    while let Some(&byte) = bytes.get(end) {
        if byte.is_ascii_digit() {
            digits = true;
        } else if byte == b'.' && !dot {
            dot = true;
        } else {
            break;
        }
        end += 1;
    }
    if digits && matches!(bytes.get(end), Some(b'e' | b'E')) {
        let mut exponent = end + 1;
        if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
            exponent += 1;
        }
        let start = exponent;
        while bytes.get(exponent).is_some_and(u8::is_ascii_digit) {
            exponent += 1;
        }
        if exponent > start {
            end = exponent;
        }
    }
    end
}

/// The next `N` numbers of the path data, from `index` on, which is moved past them.
fn take<const N: usize>(tokens: &[Token], index: &mut usize) -> Result<[f32; N]> {
    let mut values = [0.0; N];
    for slot in &mut values {
        match tokens.get(*index) {
            Some(Token::Number(value)) => {
                *slot = *value;
                *index += 1;
            }
            _ => {
                return Err(failure(SvgError::Malformed {
                    attribute: "d",
                    reason: "a command with too few numbers",
                }));
            }
        }
    }
    Ok(values)
}

/// The subpaths of a path being read, and the point the path is at.
#[derive(Debug, Default)]
struct PathBuilder {
    subpaths: Vec<Subpath>,
    current: [f32; 2],
    start: [f32; 2],
    open: bool,
}

impl PathBuilder {
    /// A point given by the path, absolute or relative to the current point.
    fn point(&self, relative: bool, [x, y]: [f32; 2]) -> Result<[f32; 2]> {
        let base = if relative { self.current } else { [0.0, 0.0] };
        Ok([finite(base[0] + x)?, finite(base[1] + y)?])
    }

    fn move_to(&mut self, point: [f32; 2]) {
        self.subpaths.push(Subpath {
            points: vec![point],
            closed: false,
        });
        self.start = point;
        self.current = point;
        self.open = true;
    }

    fn line_to(&mut self, point: [f32; 2]) -> Result<()> {
        if !self.open {
            // A command after `Z` starts a subpath at the point the closed one started at.
            if self.subpaths.is_empty() {
                return Err(failure(SvgError::Malformed {
                    attribute: "d",
                    reason: "a path that does not begin with a move",
                }));
            }
            self.move_to(self.start);
        }
        if let Some(subpath) = self.subpaths.last_mut() {
            subpath.points.push(point);
        }
        self.current = point;
        Ok(())
    }

    fn curve_to(&mut self, first: [f32; 2], second: [f32; 2], end: [f32; 2]) -> Result<()> {
        let curve = bezier_steps([self.current, first, second, end], CURVE_STEPS)?;
        for point in curve {
            self.line_to(point)?;
        }
        Ok(())
    }

    fn close(&mut self) {
        if let Some(subpath) = self.subpaths.last_mut() {
            subpath.closed = true;
        }
        self.current = self.start;
        self.open = false;
    }
}

/// The outline of path data: each subpath is stroked, and filled as a ring.
fn path(text: &str) -> Result<Outline> {
    let tokens = tokens(text)?;
    let mut builder = PathBuilder::default();
    let mut index = 0;
    while let Some(&Token::Command(first)) = tokens.get(index) {
        index += 1;
        let mut command = first;
        loop {
            let relative = command.is_ascii_lowercase();
            match command.to_ascii_uppercase() {
                'M' => {
                    let [x, y] = take::<2>(&tokens, &mut index)?;
                    let point = builder.point(relative, [x, y])?;
                    builder.move_to(point);
                }
                'L' => {
                    let [x, y] = take::<2>(&tokens, &mut index)?;
                    let point = builder.point(relative, [x, y])?;
                    builder.line_to(point)?;
                }
                'H' => {
                    let [x] = take::<1>(&tokens, &mut index)?;
                    let x = if relative { builder.current[0] + x } else { x };
                    builder.line_to([finite(x)?, builder.current[1]])?;
                }
                'V' => {
                    let [y] = take::<1>(&tokens, &mut index)?;
                    let y = if relative { builder.current[1] + y } else { y };
                    builder.line_to([builder.current[0], finite(y)?])?;
                }
                'C' => {
                    let [first_x, first_y, second_x, second_y, end_x, end_y] =
                        take::<6>(&tokens, &mut index)?;
                    let first = builder.point(relative, [first_x, first_y])?;
                    let second = builder.point(relative, [second_x, second_y])?;
                    let end = builder.point(relative, [end_x, end_y])?;
                    builder.curve_to(first, second, end)?;
                }
                'Q' => {
                    let [a, b, c, d] = take::<4>(&tokens, &mut index)?;
                    let control = builder.point(relative, [a, b])?;
                    let end = builder.point(relative, [c, d])?;
                    // A quadratic curve is the cubic one with the same shape.
                    let start = builder.current;
                    let first = two_thirds_toward(start, control)?;
                    let second = two_thirds_toward(end, control)?;
                    builder.curve_to(first, second, end)?;
                }
                'Z' => {
                    builder.close();
                    break;
                }
                other => {
                    return Err(failure(SvgError::PathCommand { command: other }));
                }
            }
            if !matches!(tokens.get(index), Some(Token::Number(_))) {
                break;
            }
            // Numbers after a move are the pairs of lines to the following points.
            command = match command {
                'M' => 'L',
                'm' => 'l',
                repeated => repeated,
            };
        }
    }
    if index < tokens.len() {
        return Err(failure(SvgError::Malformed {
            attribute: "d",
            reason: "a number before any command",
        }));
    }
    let fill = builder.subpaths.iter().map(|s| s.points.clone()).collect();
    Ok(Outline {
        fill,
        stroke: builder.subpaths,
        points: Vec::new(),
    })
}
