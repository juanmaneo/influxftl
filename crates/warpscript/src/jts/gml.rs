// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The GML and KML texts: the readers of `GEO.GML`, `GEO.KML` and of the `<` texts of `->WKT`,
//! `->WKB` and `->GEOJSON`, and the writers of `->GML` and `->KML`.
//!
//! Warp 10 reads them with a SAX handler that builds the JTS geometries as the elements end, and
//! writes them with the text of each element. The reader here does the same over the events of
//! [`Tokens`], so the failures come in the order of the document (`docs/DIVERGENCES.md`).

use super::geometry::{Coord, Geometry, Line, Polygon, Ring};
use super::xml::{Event, PREMATURE_END, Tokens};
use crate::error::fault::ResourceError;
use crate::error::gml::GmlError;
use crate::error::{Class, Error, Result};
use crate::format::double_to_text;
use crate::functions::operators::parse_double_text;

/// The deepest nesting of elements the reader follows, as the readers of WKT and WKB do (a JVM
/// stack overflows somewhere past that, at a depth that depends on its settings).
const MAX_DEPTH: usize = 512;

/// The vocabulary of a text: GML names its elements with the `gml:` prefix, KML does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    /// GML, with the `gml:` prefix.
    Gml,
    /// KML, without a prefix.
    Kml,
}

impl Dialect {
    /// The prefix of the element names the writer puts.
    fn prefix(self) -> &'static str {
        match self {
            Dialect::Gml => "gml:",
            Dialect::Kml => "",
        }
    }

    /// The key of an element in the table of the reader, in lower case. GML reads the local name
    /// of an element whatever its prefix; KML reads the name as written, prefix included.
    fn key(self, name: &str) -> String {
        match self {
            Dialect::Gml => name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase(),
            Dialect::Kml => name.to_ascii_lowercase(),
        }
    }
}

/// An element that is open: its key, its attributes, the text of its own content and what its
/// children built.
struct Frame {
    key: String,
    attributes: Vec<(String, String)>,
    text: Option<String>,
    children: Vec<Built>,
}

/// What an element builds, as the handler keeps it until its parent takes it.
enum Built {
    /// The value of an `X`, `Y` or `Z` element.
    Ordinate(f64),
    /// A `coord`.
    Coord(Coord),
    /// The coordinates of a `coordinates` text.
    Sequence(Vec<Coord>),
    /// A `box`: nothing takes its value, so only its class is kept.
    Envelope,
    /// A geometry.
    Geometry(Geometry),
}

impl Built {
    /// The class of the value, which the messages of the failures name.
    fn class(&self) -> Class {
        match self {
            Built::Ordinate(_) => Class::Double,
            Built::Coord(_) => Class::Coordinate,
            Built::Sequence(_) => Class::ArraySequence,
            Built::Envelope => Class::Envelope,
            Built::Geometry(geometry) => geometry_class(geometry),
        }
    }
}

fn geometry_class(geometry: &Geometry) -> Class {
    match geometry {
        Geometry::Point(_) => Class::Point,
        Geometry::LineString(_) => Class::LineString,
        Geometry::LinearRing(_) => Class::LinearRing,
        Geometry::Polygon(_) => Class::Polygon,
        Geometry::MultiPoint(_) => Class::MultiPoint,
        Geometry::MultiLineString(_) => Class::MultiLineString,
        Geometry::MultiPolygon(_) => Class::MultiPolygon,
        Geometry::Collection(_) => Class::GeometryCollection,
    }
}

/// Read a GML or KML text: the geometry of its root element.
///
/// The document is read to its end, so that a failure of the markup after the root element is
/// given too; a root that is not a geometry is a failure once the document is read.
///
/// # Errors
///
/// [`GmlError::Xml`] for a document the XML parser refuses, [`GmlError::Sax`] for an element
/// the handler refuses, the failures of the JTS builders for the geometries they refuse
/// ([`Error::Geometry`], [`Error::NullPointer`] for an element that holds nothing, and so on)
/// and [`ResourceError::StackOverflow`] for elements nested too deep.
pub fn read(text: &str, dialect: Dialect) -> Result<Geometry> {
    let mut open: Vec<Frame> = Vec::new();
    let mut root = None;
    for event in Tokens::new(text) {
        match event? {
            Event::Start { name, attributes } => {
                if open.len() >= MAX_DEPTH {
                    return Err(ResourceError::StackOverflow.into());
                }
                open.push(Frame {
                    key: dialect.key(&name),
                    attributes,
                    text: None,
                    children: Vec::new(),
                });
            }
            // Text is read inside the root element only, so an element is open to take it.
            Event::Text(chunk) => open.last_mut().map_or((), |frame| {
                frame.text.get_or_insert_with(String::new).push_str(&chunk);
            }),
            // The tokenizer ends only the elements it has started, so the stack is never empty.
            Event::End { .. } => open.pop().map_or(Ok(()), |frame| -> Result<()> {
                let built = build(frame)?;
                match open.last_mut() {
                    Some(parent) => parent.children.push(built),
                    None => root = Some(built),
                }
                Ok(())
            })?,
        }
    }
    root.map_or(
        Err(GmlError::Xml(PREMATURE_END.to_owned()).into()),
        |built| match built {
            Built::Geometry(geometry) => Ok(geometry),
            other => Err(GmlError::Cast {
                from: other.class(),
                to: Class::Geometry,
            }
            .into()),
        },
    )
}

/// What the element of `frame` builds, from its text and its children, once it ends.
fn build(frame: Frame) -> Result<Built> {
    let Frame {
        key,
        attributes,
        text,
        children,
    } = frame;
    match key.as_str() {
        "x" | "y" | "z" => ordinate(text.as_deref()).map(Built::Ordinate),
        "coord" => coord(children),
        "coordinates" => sequence(text.as_deref(), &attributes).map(Built::Sequence),
        "box" => envelope(children),
        "point" => point(children),
        "linestring" => line_string(children),
        "linearring" => linear_ring(children),
        "polygon" => polygon(children),
        "outerboundaryis" => {
            let member = single(children)?;
            Ok(Built::Geometry(Geometry::LinearRing(as_linear_ring(
                member,
            )?)))
        }
        // A hole is not cast here: the polygon stores it, and refuses what is not a ring then.
        "innerboundaryis" | "pointmember" | "linestringmember" | "polygonmember" => {
            single(children)
        }
        "multipoint" => multi_point(children),
        "multilinestring" => multi_line_string(children),
        "multipolygon" => multi_polygon(children),
        "multigeometry" => collection(children),
        _ => Err(Error::NullPointer),
    }
}

/// The value of an `X`, `Y` or `Z` element: its text read as `Double.valueOf` reads it, after
/// `String.trim`. An element without text is a `NullPointerException`.
fn ordinate(text: Option<&str>) -> Result<f64> {
    let text = text.ok_or(Error::NullPointer)?;
    Ok(parse_double_text(text)?)
}

/// A `coord`: its ordinates, in the order of the document, are the abscissa, the ordinate (`0`
/// when there is none) and the elevation.
fn coord(children: Vec<Built>) -> Result<Built> {
    if children.is_empty() {
        return Err(Error::NullPointer);
    }
    let mut ordinates = Vec::with_capacity(children.len());
    for child in children {
        match child {
            Built::Ordinate(value) => ordinates.push(value),
            other => return Err(GmlError::Store(other.class()).into()),
        }
    }
    if ordinates.len() > 3 {
        return Err(sax("Cannot create a coordinate with more than 3 axis"));
    }
    let x = ordinates[0];
    let y = ordinates.get(1).copied().unwrap_or(0.0);
    let z = ordinates.get(2).copied();
    Ok(Built::Coord(Coord::new(x, y, z)?))
}

/// A `coordinates`: the coordinates of its text. The text is split at the `ts` attribute, or at
/// the white space when there is none, and each token is split at the `cs` attribute, or at the
/// commas. Empty pieces are dropped.
fn sequence(text: Option<&str>, attributes: &[(String, String)]) -> Result<Vec<Coord>> {
    let text = match text {
        Some(text) if !text.is_empty() => text,
        _ => {
            return Err(sax(
                "Cannot create a coordinate sequence without text to parse",
            ));
        }
    };
    let tokens: Vec<&str> = match attribute(attributes, "ts") {
        Some(separator) => text.split(separator).filter(|t| !t.is_empty()).collect(),
        None => text
            .split(|c: char| c <= ' ')
            .filter(|t| !t.is_empty())
            .collect(),
    };
    if tokens.is_empty() {
        return Err(sax(
            "Cannot create a coordinate sequence without a non-null touple to parse",
        ));
    }
    let separator = attribute(attributes, "cs").unwrap_or(",");
    let mut dimension = None;
    let mut coords = Vec::with_capacity(tokens.len());
    for token in tokens {
        let parts: Vec<&str> = token
            .split(separator)
            .filter(|part| !part.is_empty())
            .collect();
        // The first token sets the dimension of the sequence, and the others take what they
        // have of it.
        let dimension = *dimension.get_or_insert(parts.len());
        if dimension > 3 {
            return Err(GmlError::Illegal("dimension must be <= 3").into());
        }
        coords.push(token_coord(&parts, dimension)?);
    }
    Ok(coords)
}

/// The coordinate of the parts of a token, in a sequence of `dimension` ordinates. The parts
/// past the dimension are not read. An ordinate the token lacks is `0` in a sequence of at most
/// one dimension, and `NaN` in the others, which a geometry refuses.
fn token_coord(parts: &[&str], dimension: usize) -> Result<Coord> {
    let ordinate = |index: usize| -> Result<f64> {
        if index < dimension.min(parts.len()) {
            Ok(parse_double_text(parts[index])?)
        } else if dimension <= 1 {
            Ok(0.0)
        } else {
            Ok(f64::NAN)
        }
    };
    let x = ordinate(0)?;
    let y = ordinate(1)?;
    let z = if dimension == 3 {
        Some(ordinate(2)?)
    } else {
        None
    };
    Coord::new(x, y, z)
}

/// The value of an attribute of an element.
fn attribute<'a>(attributes: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// A `box`: the coordinate sequences it holds. Nothing reads its envelope, so none is built: a
/// sequence is never empty, which is all the envelope of the coordinates could fail on.
fn envelope(children: Vec<Built>) -> Result<Built> {
    if children.is_empty() {
        return Err(Error::NullPointer);
    }
    for child in children {
        if !matches!(child, Built::Sequence(_)) {
            return Err(GmlError::Cast {
                from: child.class(),
                to: Class::CoordinateSequence,
            }
            .into());
        }
    }
    Ok(Built::Envelope)
}

/// A `Point`: one coordinate, given as a `coord` or as a sequence of one coordinate.
fn point(mut children: Vec<Built>) -> Result<Built> {
    if children.len() > 1 {
        return Err(sax("Cannot create a point without exactly one coordinate"));
    }
    let coord = match children.pop().ok_or(Error::NullPointer)? {
        Built::Coord(coord) => coord,
        Built::Sequence(coords) => match coords.as_slice() {
            [coord] => *coord,
            _ => return Err(GmlError::Assertion.into()),
        },
        other => {
            return Err(GmlError::Cast {
                from: other.class(),
                to: Class::CoordinateSequence,
            }
            .into());
        }
    };
    Ok(Built::Geometry(Geometry::Point(Some(coord))))
}

/// A `LineString`: one coordinate sequence, or at least two coordinates.
fn line_string(mut children: Vec<Built>) -> Result<Built> {
    if children.len() == 1 {
        return match children.remove(0) {
            Built::Sequence(coords) => {
                Ok(Built::Geometry(Geometry::LineString(Line::new(coords)?)))
            }
            other => Err(sax_caused(
                "Cannot create a linestring without atleast two coordinates or one coordinate sequence",
                GmlError::Cast {
                    from: other.class(),
                    to: Class::CoordinateSequence,
                }
                .into(),
            )),
        };
    }
    if children.is_empty() {
        return Err(Error::NullPointer);
    }
    // Two or more coordinates, which is what `Line::new` asks for.
    let coords = coordinates_of(children)?;
    Ok(Built::Geometry(Geometry::LineString(Line::extended(
        coords,
    ))))
}

/// A `LinearRing`: one coordinate sequence, or at least four coordinates.
fn linear_ring(mut children: Vec<Built>) -> Result<Built> {
    const MESSAGE: &str =
        "Cannot create a linear ring without atleast four coordinates or one coordinate sequence";
    match children.len() {
        0 => Err(Error::NullPointer),
        1 => match children.remove(0) {
            Built::Sequence(coords) => ring(coords),
            other => Err(sax_caused(
                MESSAGE,
                GmlError::Cast {
                    from: other.class(),
                    to: Class::CoordinateSequence,
                }
                .into(),
            )),
        },
        2 | 3 => Err(sax(MESSAGE)),
        _ => ring(coordinates_of(children)?),
    }
}

fn ring(coords: Vec<Coord>) -> Result<Built> {
    Ok(Built::Geometry(Geometry::LinearRing(Ring::new(coords)?)))
}

/// A `Polygon`: a shell, then the holes, which are linear rings.
fn polygon(children: Vec<Built>) -> Result<Built> {
    let mut rings = children.into_iter();
    let shell = as_linear_ring(rings.next().ok_or(Error::NullPointer)?)?;
    let holes = rings
        .map(|child| match child {
            Built::Geometry(Geometry::LinearRing(ring)) => Ok(ring),
            other => Err(GmlError::Store(other.class()).into()),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Built::Geometry(Geometry::Polygon(Polygon::with_shell(
        shell, holes,
    ))))
}

/// The one element a member or a boundary holds.
fn single(mut children: Vec<Built>) -> Result<Built> {
    if children.len() > 1 {
        return Err(sax("Geometry Members may only contain one geometry."));
    }
    children.pop().ok_or(Error::NullPointer)
}

fn as_linear_ring(built: Built) -> Result<Ring> {
    match built {
        Built::Geometry(Geometry::LinearRing(ring)) => Ok(ring),
        other => Err(GmlError::Cast {
            from: other.class(),
            to: Class::LinearRing,
        }
        .into()),
    }
}

/// A `MultiPoint`: points.
fn multi_point(children: Vec<Built>) -> Result<Built> {
    if children.is_empty() {
        return Err(Error::NullPointer);
    }
    let points = children
        .into_iter()
        .map(|child| match child {
            Built::Geometry(Geometry::Point(point)) => Ok(point),
            other => Err(GmlError::Store(other.class()).into()),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Built::Geometry(Geometry::MultiPoint(points)))
}

/// A `MultiLineString`: line strings, and linear rings, which are read as lines.
fn multi_line_string(children: Vec<Built>) -> Result<Built> {
    if children.is_empty() {
        return Err(Error::NullPointer);
    }
    let lines = children
        .into_iter()
        .map(|child| match child {
            Built::Geometry(Geometry::LineString(line)) => Ok(line),
            Built::Geometry(Geometry::LinearRing(ring)) => {
                Ok(Line::extended(ring.coords().to_vec()))
            }
            other => Err(GmlError::Store(other.class()).into()),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Built::Geometry(Geometry::MultiLineString(lines)))
}

/// A `MultiPolygon`: polygons.
fn multi_polygon(children: Vec<Built>) -> Result<Built> {
    if children.is_empty() {
        return Err(Error::NullPointer);
    }
    let polygons = children
        .into_iter()
        .map(|child| match child {
            Built::Geometry(Geometry::Polygon(polygon)) => Ok(polygon),
            other => Err(GmlError::Store(other.class()).into()),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Built::Geometry(Geometry::MultiPolygon(polygons)))
}

/// A `MultiGeometry`: geometries.
fn collection(children: Vec<Built>) -> Result<Built> {
    if children.is_empty() {
        return Err(Error::NullPointer);
    }
    let elements = children
        .into_iter()
        .map(|child| match child {
            Built::Geometry(geometry) => Ok(geometry),
            other => Err(GmlError::Store(other.class()).into()),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Built::Geometry(Geometry::Collection(elements)))
}

/// The coordinates of children that are all coordinates, the first other child being refused
/// as an array store.
fn coordinates_of(children: Vec<Built>) -> Result<Vec<Coord>> {
    children
        .into_iter()
        .map(|child| match child {
            Built::Coord(coord) => Ok(coord),
            other => Err(GmlError::Store(other.class()).into()),
        })
        .collect()
}

fn sax(message: &str) -> Error {
    GmlError::Sax {
        message: message.to_owned(),
        cause: None,
    }
    .into()
}

fn sax_caused(message: &str, cause: Error) -> Error {
    GmlError::Sax {
        message: message.to_owned(),
        cause: Some(Box::new(cause)),
    }
    .into()
}

/// Write a geometry as GML or KML.
///
/// A multi geometry names its members in GML. In KML it is named after its kind only inside a
/// collection; a top-level one is a `MultiGeometry` whose members are written as they are.
///
/// # Errors
///
/// [`Error::NullPointer`] for an empty point, alone or in a multi geometry or a collection.
pub fn write(geometry: &Geometry, dialect: Dialect) -> Result<String> {
    let mut out = String::new();
    write_geometry(geometry, dialect, false, &mut out)?;
    Ok(out)
}

/// `nested` tells that the geometry is a member of a collection.
fn write_geometry(
    geometry: &Geometry,
    dialect: Dialect,
    nested: bool,
    out: &mut String,
) -> Result<()> {
    match geometry {
        Geometry::Point(point) => write_point(point.as_ref(), dialect, out),
        Geometry::LineString(line) => write_line(line.coords(), dialect, out),
        Geometry::LinearRing(ring) => write_line(ring.coords(), dialect, out),
        Geometry::Polygon(polygon) => write_polygon(polygon, dialect, out),
        Geometry::MultiPoint(points) => {
            let (name, member) = multi_names(dialect, nested, "MultiPoint", "pointMember");
            write_multi(dialect, name, member, points, out, |point, out| {
                write_point(point.as_ref(), dialect, out)
            })
        }
        Geometry::MultiLineString(lines) => {
            let (name, member) =
                multi_names(dialect, nested, "MultiLineString", "lineStringMember");
            write_multi(dialect, name, member, lines, out, |line, out| {
                write_line(line.coords(), dialect, out)
            })
        }
        Geometry::MultiPolygon(polygons) => {
            let (name, member) = multi_names(dialect, nested, "MultiPolygon", "polygonMember");
            write_multi(dialect, name, member, polygons, out, |polygon, out| {
                write_polygon(polygon, dialect, out)
            })
        }
        Geometry::Collection(elements) => write_multi(
            dialect,
            "MultiGeometry",
            Some("geometryMember"),
            elements,
            out,
            |element, out| write_geometry(element, dialect, true, out),
        ),
    }
}

/// The name of a multi geometry and the name of its members, `None` when they are written as
/// they are: a top-level KML multi geometry.
fn multi_names<'a>(
    dialect: Dialect,
    nested: bool,
    kind: &'a str,
    member: &'a str,
) -> (&'a str, Option<&'a str>) {
    match dialect {
        Dialect::Kml if !nested => ("MultiGeometry", None),
        _ => (kind, Some(member)),
    }
}

/// A multi geometry: its container, and its members, each in its `member` element when there is
/// one.
fn write_multi<T>(
    dialect: Dialect,
    name: &str,
    member: Option<&str>,
    items: &[T],
    out: &mut String,
    write: impl Fn(&T, &mut String) -> Result<()>,
) -> Result<()> {
    let prefix = dialect.prefix();
    out.push_str(&format!("<{prefix}{name}>"));
    for item in items {
        match member {
            Some(member) => {
                out.push_str(&format!("<{prefix}{member}>"));
                write(item, out)?;
                out.push_str(&format!("</{prefix}{member}>"));
            }
            None => write(item, out)?,
        }
    }
    out.push_str(&format!("</{prefix}{name}>"));
    Ok(())
}

fn write_point(point: Option<&Coord>, dialect: Dialect, out: &mut String) -> Result<()> {
    let coord = point.ok_or(Error::NullPointer)?;
    let prefix = dialect.prefix();
    out.push_str(&format!("<{prefix}Point>"));
    write_coordinates(std::slice::from_ref(coord), prefix, out);
    out.push_str(&format!("</{prefix}Point>"));
    Ok(())
}

/// A line string, which a linear ring is written as too.
fn write_line(coords: &[Coord], dialect: Dialect, out: &mut String) -> Result<()> {
    let prefix = dialect.prefix();
    out.push_str(&format!("<{prefix}LineString>"));
    write_coordinates(coords, prefix, out);
    out.push_str(&format!("</{prefix}LineString>"));
    Ok(())
}

fn write_polygon(polygon: &Polygon, dialect: Dialect, out: &mut String) -> Result<()> {
    let prefix = dialect.prefix();
    out.push_str(&format!("<{prefix}Polygon>"));
    write_boundary("outerBoundaryIs", polygon.shell().coords(), prefix, out);
    for hole in polygon.holes() {
        write_boundary("innerBoundaryIs", hole.coords(), prefix, out);
    }
    out.push_str(&format!("</{prefix}Polygon>"));
    Ok(())
}

fn write_boundary(name: &str, coords: &[Coord], prefix: &str, out: &mut String) {
    out.push_str(&format!("<{prefix}{name}><{prefix}LinearRing>"));
    write_coordinates(coords, prefix, out);
    out.push_str(&format!("</{prefix}LinearRing></{prefix}{name}>"));
}

/// The `coordinates` element of the coordinates: `x,y`, then `,z` when there is one, each
/// coordinate followed by a space.
fn write_coordinates(coords: &[Coord], prefix: &str, out: &mut String) {
    out.push_str(&format!("<{prefix}coordinates>"));
    for coord in coords {
        out.push_str(&double_to_text(coord.x));
        out.push(',');
        out.push_str(&double_to_text(coord.y));
        if let Some(z) = coord.z {
            out.push(',');
            out.push_str(&double_to_text(z));
        }
        out.push(' ');
    }
    out.push_str(&format!("</{prefix}coordinates>"));
}
