// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! GeoJSON: the reader behind `GEO.JSON` and the writer behind `->GEOJSON`.
//!
//! Both do what the GeoJSON binding of Warp 10 (`org.wololo`, on Jackson) does, quirks
//! included. The reader reads the first JSON value of the text, takes its `type` for the name
//! of a class and reads the geometry in two steps, like Jackson then the converter do: the
//! whole tree is checked first (in the order of the properties, with the failures of Jackson),
//! then converted (a missing coordinate is a `NullPointerException`, a coordinate with a single
//! number an index failure…). Features are read and refused.

use std::fmt;

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use super::geometry::{Coord, Geometry, Line, Polygon, Ring};
use super::wkt::parse_number;
use crate::error::fault::BoundsError;
use crate::error::geojson::{Chain, Found, GeoJsonError, Step, StringTarget, Target};
use crate::error::geometry::GeometryError;
use crate::error::{Error, Result};
use crate::format::java_double_to_string;

/// The properties of the classes of the binding that have coordinates.
const COORDINATES_PROPERTIES: &[&str] = &["bbox", "type", "coordinates"];

/// The properties of a geometry collection.
const COLLECTION_PROPERTIES: &[&str] = &["geometries", "type"];

/// A JSON value, the way Jackson tells them apart: the properties of an object are kept in
/// order, with their duplicates, and the integers are not the numbers with a fraction.
#[derive(Debug)]
enum Json {
    Null,
    Bool(bool),
    Integer(i128),
    Float(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Json, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a JSON value")
    }

    fn visit_unit<E>(self) -> std::result::Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Json, E> {
        Ok(Json::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Json, E> {
        Ok(Json::Integer(i128::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Json, E> {
        Ok(Json::Integer(i128::from(value)))
    }

    fn visit_f64<E>(self, value: f64) -> std::result::Result<Json, E> {
        Ok(Json::Float(value))
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Json, E> {
        Ok(Json::String(value.to_owned()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Json, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Json::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Json, A::Error> {
        let mut members = Vec::new();
        while let Some(member) = map.next_entry()? {
            members.push(member);
        }
        Ok(Json::Object(members))
    }
}

/// The members of a JSON object (`ObjectNode`) that has some: an empty object has no
/// property to read, which is what a value that is not an object has.
#[derive(Clone, Copy)]
struct Object<'a> {
    /// The name of the first property.
    first: &'a str,
    members: &'a [(String, Json)],
}

impl<'a> Object<'a> {
    /// `JsonNode.get(name)`: the last value of the property (a duplicate replaces the value).
    fn get(self, name: &str) -> Option<&'a Json> {
        self.members
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }
}

impl Json {
    /// The object, `None` for any other value and for an empty object (`get` of those is
    /// `null`).
    fn as_object(&self) -> Option<Object<'_>> {
        match self {
            Json::Object(members) => members.first().map(|(first, _)| Object { first, members }),
            _ => None,
        }
    }

    /// `JsonNode.asText`: the text of a scalar, nothing for an array or an object.
    fn as_text(&self) -> String {
        match self {
            Json::Null => "null".to_owned(),
            Json::Bool(value) => value.to_string(),
            Json::Integer(value) => value.to_string(),
            Json::Float(value) => java_double_to_string(*value),
            Json::String(text) => text.clone(),
            Json::Array(_) | Json::Object(_) => String::new(),
        }
    }

    /// The kind of value, for the failure of a type that cannot be read from it.
    fn found(&self) -> Found {
        match self {
            Json::Null => Found::Null,
            Json::Bool(true) => Found::True,
            Json::Bool(false) => Found::False,
            Json::Integer(_) => Found::Integer,
            Json::Float(_) => Found::Float,
            Json::String(_) => Found::String,
            Json::Array(_) => Found::Array,
            Json::Object(_) => Found::Object,
        }
    }
}

/// The `NullPointerException` of the reader of a tree, thrown wrapped in a
/// `RuntimeException` as `GeoJSONFactory.create` does.
fn wrapped_null_pointer() -> Error {
    Error::with_cause("java.lang.NullPointerException", Error::NullPointer)
}

/// The path being followed in the tree.
type Path = Vec<Step>;

/// A `double[]` (and the arrays of arrays of it): the numbers a geometry is made of.
trait Decode: Sized {
    /// The number of dimensions of the array.
    const DIMENSIONS: usize;

    /// Read `json` as the array: `None` for a `null`.
    fn decode(json: &Json, path: &mut Path) -> Result<Option<Self>>;
}

/// The failure of a value that is not an array where one is expected.
fn not_an_array(json: &Json, array: Array, path: &Path) -> Error {
    let chain = Chain(path.clone());
    Error::wrapped(match (json, array) {
        (Json::String(text), array) => GeoJsonError::NoStringConstructor {
            target: match array {
                Array::Doubles(dimensions) => StringTarget::Doubles(dimensions),
                Array::Geometries => StringTarget::Geometries,
            },
            text: text.clone(),
            chain,
        },
        (json, array) => GeoJsonError::Mismatch {
            target: match array {
                Array::Doubles(dimensions) => Target::Doubles(dimensions),
                Array::Geometries => Target::Geometries,
            },
            found: json.found(),
            chain,
        },
    })
}

/// The kind of array read.
#[derive(Clone, Copy)]
enum Array {
    Doubles(usize),
    Geometries,
}

/// The elements of the array `json` (a `null` is `None`), read by `element`; the path of an
/// element ends with the element of `array_type`.
fn elements<T>(
    json: &Json,
    array: Array,
    element_type: &'static str,
    path: &mut Path,
    mut element: impl FnMut(&Json, &mut Path) -> Result<T>,
) -> Result<Option<Vec<T>>> {
    match json {
        Json::Null => Ok(None),
        Json::Array(items) => items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                path.push(Step::Element {
                    array: element_type,
                    index,
                });
                let read = element(item, path);
                path.pop();
                read
            })
            .collect::<Result<Vec<T>>>()
            .map(Some),
        other => Err(not_an_array(other, array, path)),
    }
}

/// A `double`: a number, a string of one, or `null` (zero).
fn double(json: &Json, path: &Path) -> Result<f64> {
    match json {
        Json::Integer(value) => Ok(*value as f64),
        Json::Float(value) => Ok(*value),
        Json::Null => Ok(0.0),
        Json::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                Ok(0.0)
            } else {
                parse_number(text).ok_or_else(|| {
                    Error::wrapped(GeoJsonError::NotANumber {
                        text: text.to_owned(),
                        chain: Chain(path.clone()),
                    })
                })
            }
        }
        other => Err(Error::wrapped(GeoJsonError::Mismatch {
            target: Target::Double,
            found: other.found(),
            chain: Chain(path.clone()),
        })),
    }
}

impl Decode for Vec<f64> {
    const DIMENSIONS: usize = 1;

    fn decode(json: &Json, path: &mut Path) -> Result<Option<Self>> {
        elements(
            json,
            Array::Doubles(Self::DIMENSIONS),
            "double",
            path,
            |item, path| double(item, path),
        )
    }
}

impl<T: Decode> Decode for Vec<Option<T>> {
    const DIMENSIONS: usize = T::DIMENSIONS + 1;

    fn decode(json: &Json, path: &mut Path) -> Result<Option<Self>> {
        elements(
            json,
            Array::Doubles(Self::DIMENSIONS),
            "java.lang.Object",
            path,
            T::decode,
        )
    }
}

/// A position: `[x, y]`, `[x, y, z]`.
type Position = Vec<f64>;
/// The positions of a line.
type Positions = Vec<Option<Position>>;
/// The lines of a polygon.
type Rings = Vec<Option<Positions>>;
/// The polygons of a multi-polygon.
type Polygons = Vec<Option<Rings>>;

/// A geometry read from a tree, before it is converted.
#[derive(Debug)]
enum Decoded {
    Point(Option<Position>),
    LineString(Option<Positions>),
    Polygon(Option<Rings>),
    MultiPoint(Option<Positions>),
    MultiLineString(Option<Rings>),
    MultiPolygon(Option<Polygons>),
    Collection(Option<Vec<Option<Decoded>>>),
}

/// The classes of the binding that are geometries.
#[derive(Clone, Copy)]
enum Class {
    Point,
    LineString,
    Polygon,
    MultiPoint,
    MultiLineString,
    MultiPolygon,
    Collection,
}

impl Class {
    fn of(name: &str) -> Option<Class> {
        Some(match name {
            "Point" => Class::Point,
            "LineString" => Class::LineString,
            "Polygon" => Class::Polygon,
            "MultiPoint" => Class::MultiPoint,
            "MultiLineString" => Class::MultiLineString,
            "MultiPolygon" => Class::MultiPolygon,
            "GeometryCollection" => Class::Collection,
            _ => return None,
        })
    }

    /// The simple name of the class.
    fn name(self) -> &'static str {
        match self {
            Class::Point => "Point",
            Class::LineString => "LineString",
            Class::Polygon => "Polygon",
            Class::MultiPoint => "MultiPoint",
            Class::MultiLineString => "MultiLineString",
            Class::MultiPolygon => "MultiPolygon",
            Class::Collection => "GeometryCollection",
        }
    }

    /// Read the object as an instance of the class.
    fn decode(self, object: Object<'_>, path: &mut Path) -> Result<Decoded> {
        Ok(match self {
            Class::Point => Decoded::Point(coordinates(self, object, path)?),
            Class::LineString => Decoded::LineString(coordinates(self, object, path)?),
            Class::Polygon => Decoded::Polygon(coordinates(self, object, path)?),
            Class::MultiPoint => Decoded::MultiPoint(coordinates(self, object, path)?),
            Class::MultiLineString => Decoded::MultiLineString(coordinates(self, object, path)?),
            Class::MultiPolygon => Decoded::MultiPolygon(coordinates(self, object, path)?),
            Class::Collection => Decoded::Collection(geometries(object, path)?),
        })
    }
}

/// Read the properties of an object of `class`, each by `property`, which tells whether the
/// class has it. The path ends with the class while they are read.
fn properties(
    class: &'static str,
    known: &'static [&'static str],
    object: Object<'_>,
    path: &mut Path,
    mut property: impl FnMut(&str, &Json, &mut Path) -> Result<bool>,
) -> Result<()> {
    path.push(Step::Class(class));
    for (name, value) in object.members {
        if !property(name, value, path)? {
            return Err(Error::wrapped(GeoJsonError::UnknownProperty {
                class,
                field: name.clone(),
                known,
                chain: Chain(path.clone()),
            }));
        }
    }
    path.pop();
    Ok(())
}

/// A property of the path, read by `read`.
fn property<T>(
    name: &'static str,
    path: &mut Path,
    read: impl FnOnce(&mut Path) -> Result<T>,
) -> Result<T> {
    path.push(Step::Field(name));
    let value = read(path);
    path.pop();
    value
}

/// The `coordinates` of an object of a class that has some (and its `bbox`).
fn coordinates<T: Decode>(class: Class, object: Object<'_>, path: &mut Path) -> Result<Option<T>> {
    let mut coordinates = None;
    properties(
        class.name(),
        COORDINATES_PROPERTIES,
        object,
        path,
        |name, value, path| {
            match name {
                "type" => {}
                "bbox" => {
                    property("bbox", path, |path| Vec::<f64>::decode(value, path))?;
                }
                "coordinates" => {
                    coordinates = property("coordinates", path, |path| T::decode(value, path))?;
                }
                _ => return Ok(false),
            }
            Ok(true)
        },
    )?;
    Ok(coordinates)
}

/// The `geometries` of a geometry collection.
fn geometries(object: Object<'_>, path: &mut Path) -> Result<Option<Vec<Option<Decoded>>>> {
    let mut geometries = None;
    properties(
        "GeometryCollection",
        COLLECTION_PROPERTIES,
        object,
        path,
        |name, value, path| {
            match name {
                "type" => {}
                "geometries" => {
                    geometries = property("geometries", path, |path| {
                        elements(
                            value,
                            Array::Geometries,
                            "java.lang.Object",
                            path,
                            geometry_element,
                        )
                    })?;
                }
                _ => return Ok(false),
            }
            Ok(true)
        },
    )?;
    Ok(geometries)
}

/// A geometry of a collection: its `type` tells the class (`@JsonTypeInfo`).
fn geometry_element(json: &Json, path: &mut Path) -> Result<Option<Decoded>> {
    if matches!(json, Json::Null) {
        return Ok(None);
    }
    let object = json.as_object();
    let type_id = object.and_then(|object| match object.get("type") {
        Some(Json::Null | Json::Array(_) | Json::Object(_)) | None => None,
        Some(id) => Some(id.as_text()),
    });
    let chain = || Chain(path.clone());
    let (Some(object), Some(id)) = (object, type_id) else {
        return Err(Error::wrapped(GeoJsonError::MissingTypeId {
            chain: chain(),
        }));
    };
    if let Some(class) = Class::of(&id) {
        return class.decode(object, path).map(Some);
    }
    Err(Error::wrapped(match id.as_str() {
        "Feature" => GeoJsonError::NotSubtype {
            id: "Feature",
            chain: chain(),
        },
        "FeatureCollection" => GeoJsonError::NotSubtype {
            id: "FeatureCollection",
            chain: chain(),
        },
        _ => GeoJsonError::UnknownTypeId {
            id,
            chain: Some(chain()),
        },
    }))
}

/// `GeoJSONFactory.readFeature`: the feature is read (and its geometry checked), not kept.
fn read_feature(json: &Json) -> Result<()> {
    let object = json.as_object().ok_or_else(wrapped_null_pointer)?;
    let properties = object.get("properties").ok_or_else(wrapped_null_pointer)?;
    match properties {
        Json::Null | Json::Object(_) => {}
        Json::String(text) => {
            return Err(Error::wrapped(GeoJsonError::NoStringConstructor {
                target: StringTarget::Properties,
                text: text.clone(),
                chain: Chain::default(),
            }));
        }
        other => {
            return Err(Error::wrapped(GeoJsonError::Mismatch {
                target: Target::Properties,
                found: other.found(),
                chain: Chain::default(),
            }));
        }
    }
    let geometry = object
        .get("geometry")
        .and_then(Json::as_object)
        .ok_or_else(wrapped_null_pointer)?;
    let type_name = geometry
        .get("type")
        .ok_or_else(wrapped_null_pointer)?
        .as_text();
    match type_name.as_str() {
        "Feature" | "FeatureCollection" => Err(Error::wrapped(GeoJsonError::NotAGeometry(
            if type_name == "Feature" {
                "Feature"
            } else {
                "FeatureCollection"
            },
        ))),
        _ => read_class(geometry, &type_name).map(|_| ()),
    }
}

/// `readGeometry`: the object read as the class `org.wololo.geojson.<type_name>`.
fn read_class(object: Object<'_>, type_name: &str) -> Result<Decoded> {
    let mut path = Path::new();
    if let Some(class) = Class::of(type_name) {
        return class.decode(object, &mut path);
    }
    Err(match type_name {
        "Geometry" => Error::wrapped(GeoJsonError::UnknownTypeId {
            id: type_name.to_owned(),
            chain: None,
        }),
        "GeoJSON" => Error::wrapped(GeoJsonError::AbstractClass),
        "GeoJSONFactory" => Error::wrapped(GeoJsonError::UnknownProperty {
            class: "GeoJSONFactory",
            field: object.first.to_owned(),
            known: &[],
            chain: Chain(vec![Step::Class("GeoJSONFactory")]),
        }),
        other => Error::wrapped(GeoJsonError::NoClass(other.to_owned())),
    })
}

/// `GeoJSONFactory.readFeatureCollection`: the features are read (and checked), not kept.
fn read_feature_collection(object: Object<'_>) -> Result<()> {
    let features = object.get("features").ok_or_else(wrapped_null_pointer)?;
    // `JsonNode.iterator()`: the elements of an array, the values of an object, nothing of
    // the other values.
    let features: Vec<&Json> = match features {
        Json::Array(items) => items.iter().collect(),
        Json::Object(members) => members.iter().map(|(_, value)| value).collect(),
        Json::Null | Json::Bool(_) | Json::Integer(_) | Json::Float(_) | Json::String(_) => {
            Vec::new()
        }
    };
    features.into_iter().try_for_each(read_feature)
}

/// `GeoJSONFactory.create`: the geometry a text holds, read from its tree.
///
/// A feature or a collection of features is read like Warp 10 does, then refused
/// (`UnsupportedOperationException`: the converter has no geometry for it).
fn create(root: &Json) -> Result<Decoded> {
    let object = root.as_object().ok_or_else(wrapped_null_pointer)?;
    let type_name = object
        .get("type")
        .ok_or_else(wrapped_null_pointer)?
        .as_text();
    match type_name.as_str() {
        "FeatureCollection" => {
            read_feature_collection(object)?;
            Err(GeometryError::Unsupported.into())
        }
        "Feature" => {
            read_feature(root)?;
            Err(GeometryError::Unsupported.into())
        }
        _ => read_class(object, &type_name),
    }
}

/// The first JSON value of `text`.
fn parse(text: &str) -> Result<Json> {
    // Jackson reads a blank text as no tree at all.
    if text.trim_matches([' ', '\t', '\n', '\r']).is_empty() {
        return Err(wrapped_null_pointer());
    }
    Json::deserialize(&mut serde_json::Deserializer::from_str(text)).map_err(|err| {
        Error::wrapped(GeoJsonError::Syntax {
            message: err.to_string(),
            end_of_input: err.is_eof(),
        })
    })
}

/// A position as a coordinate: `[x, y]` is two dimensional, anything else takes the three
/// first numbers.
fn convert_position(position: Option<&Position>) -> Result<Coord> {
    let position = position.ok_or(Error::NullPointer)?;
    let at = |index: usize| {
        position.get(index).copied().ok_or(BoundsError::ArrayIndex {
            index: index as i64,
            length: position.len(),
        })
    };
    if position.len() == 2 {
        Coord::new(at(0)?, at(1)?, None)
    } else {
        Coord::new(at(0)?, at(1)?, Some(at(2)?))
    }
}

fn convert_positions(positions: Option<&Positions>) -> Result<Vec<Coord>> {
    positions
        .ok_or(Error::NullPointer)?
        .iter()
        .map(|position| convert_position(position.as_ref()))
        .collect()
}

/// The rings of a polygon: the first is the shell.
fn convert_polygon(rings: Option<&Rings>) -> Result<Polygon> {
    let rings = rings.ok_or(Error::NullPointer)?;
    let (shell, holes) = rings.split_first().ok_or(BoundsError::ArrayIndex {
        index: 0,
        length: 0,
    })?;
    let shell = Ring::new(convert_positions(shell.as_ref())?)?;
    let holes = holes
        .iter()
        .map(|hole| Ring::new(convert_positions(hole.as_ref())?))
        .collect::<Result<Vec<_>>>()?;
    Polygon::new(shell, holes)
}

fn convert_lines(lines: Option<&Rings>) -> Result<Vec<Line>> {
    lines
        .ok_or(Error::NullPointer)?
        .iter()
        .map(|line| Line::new(convert_positions(line.as_ref())?))
        .collect()
}

fn convert(decoded: &Decoded) -> Result<Geometry> {
    Ok(match decoded {
        Decoded::Point(position) => Geometry::Point(Some(convert_position(position.as_ref())?)),
        Decoded::LineString(positions) => {
            Geometry::LineString(Line::new(convert_positions(positions.as_ref())?)?)
        }
        Decoded::Polygon(rings) => Geometry::Polygon(convert_polygon(rings.as_ref())?),
        Decoded::MultiPoint(positions) => Geometry::MultiPoint(
            convert_positions(positions.as_ref())?
                .into_iter()
                .map(Some)
                .collect(),
        ),
        Decoded::MultiLineString(lines) => {
            Geometry::MultiLineString(convert_lines(lines.as_ref())?)
        }
        Decoded::MultiPolygon(polygons) => Geometry::MultiPolygon(
            polygons
                .as_ref()
                .ok_or(Error::NullPointer)?
                .iter()
                .map(|rings| convert_polygon(rings.as_ref()))
                .collect::<Result<_>>()?,
        ),
        Decoded::Collection(geometries) => Geometry::Collection(
            geometries
                .as_ref()
                .ok_or(Error::NullPointer)?
                .iter()
                .map(|geometry| convert(geometry.as_ref().ok_or(GeometryError::Unsupported)?))
                .collect::<Result<_>>()?,
        ),
    })
}

/// Read a geometry from GeoJSON: the first JSON value of the text.
///
/// # Errors
///
/// The failures of Jackson reading the text ([`GeoJsonError`], wrapped like the
/// `RuntimeException` that carries them in Warp 10), a `NullPointerException` for a text that is
/// no object, an object without `type`, a geometry without coordinates, an
/// `ArrayIndexOutOfBoundsException` for a position of fewer than 2 numbers, the
/// `IllegalArgumentException` of a line string or a ring that is not valid,
/// [`GeometryError::Unsupported`] for a feature and [`NotFinite`](crate::finite::NotFinite)
/// for a coordinate that is not a finite number.
pub fn read(text: &str) -> Result<Geometry> {
    convert(&create(&parse(text)?)?)
}

/// `GeoJSONWriter.write`: the geometry as a GeoJSON text.
///
/// # Errors
///
/// [`Error::NullPointer`] for an empty point (alone, or in a collection) and
/// [`GeometryError::Unsupported`] for a linear ring.
pub fn write(geometry: &Geometry) -> Result<String> {
    let mut out = String::new();
    write_geometry(geometry, &mut out)?;
    Ok(out)
}

/// `[x,y]`, with the elevation when there is one.
fn write_position(coord: &Coord, out: &mut String) {
    out.push('[');
    out.push_str(&java_double_to_string(coord.x));
    out.push(',');
    out.push_str(&java_double_to_string(coord.y));
    if let Some(z) = coord.z {
        out.push(',');
        out.push_str(&java_double_to_string(z));
    }
    out.push(']');
}

/// `[[x,y],…]`.
fn write_positions<'a>(coords: impl IntoIterator<Item = &'a Coord>, out: &mut String) {
    write_array(coords, out, write_position);
}

/// The items between brackets, separated by commas, each written by `item`.
fn write_array<T>(
    items: impl IntoIterator<Item = T>,
    out: &mut String,
    mut item: impl FnMut(T, &mut String),
) {
    out.push('[');
    for (index, element) in items.into_iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        item(element, out);
    }
    out.push(']');
}

/// The rings of a polygon: `[[…]]`, an empty shell being an empty ring.
fn write_rings(polygon: &Polygon, out: &mut String) {
    let rings = std::iter::once(polygon.shell()).chain(polygon.holes());
    write_array(rings, out, |ring, out| write_positions(ring.coords(), out));
}

/// `{"type":…,"coordinates":…}`, the coordinates written by `coordinates`.
fn write_typed(name: &str, out: &mut String, coordinates: impl FnOnce(&mut String)) {
    out.push_str("{\"type\":\"");
    out.push_str(name);
    out.push_str("\",\"coordinates\":");
    coordinates(out);
    out.push('}');
}

fn write_geometry(geometry: &Geometry, out: &mut String) -> Result<()> {
    match geometry {
        Geometry::Point(point) => {
            let point = point.as_ref().ok_or(Error::NullPointer)?;
            write_typed("Point", out, |out| write_position(point, out));
        }
        Geometry::LineString(line) => {
            write_typed("LineString", out, |out| write_positions(line.coords(), out));
        }
        Geometry::LinearRing(_) => return Err(GeometryError::Unsupported.into()),
        Geometry::Polygon(polygon) => write_typed("Polygon", out, |out| write_rings(polygon, out)),
        Geometry::MultiPoint(points) => write_typed("MultiPoint", out, |out| {
            write_positions(points.iter().flatten(), out);
        }),
        Geometry::MultiLineString(lines) => write_typed("MultiLineString", out, |out| {
            write_array(lines, out, |line, out| write_positions(line.coords(), out));
        }),
        Geometry::MultiPolygon(polygons) => write_typed("MultiPolygon", out, |out| {
            write_array(polygons, out, write_rings);
        }),
        Geometry::Collection(elements) => {
            out.push_str("{\"type\":\"GeometryCollection\",\"geometries\":[");
            for (index, element) in elements.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_geometry(element, out)?;
            }
            out.push_str("]}");
        }
    }
    Ok(())
}
