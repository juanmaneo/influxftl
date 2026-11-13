// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the GeoJSON reader (`GEO.JSON`, and `->WKT`… of a GeoJSON text), worded like
//! the Jackson databind that the GeoJSON binding of Warp 10 reads a text with: a script reads
//! them in the cause chain of the failure of the function.

/// `at [Source: UNKNOWN; byte offset: #UNKNOWN]`, the location Jackson gives to a failure of
/// a tree it read (the text is gone by then).
const SOURCE: &str = "at [Source: UNKNOWN; byte offset: #UNKNOWN]";

/// The type ids of the geometries Jackson knows a GeoJSON by.
const KNOWN_TYPE_IDS: &str = "[Feature, FeatureCollection, GeometryCollection, LineString, \
                              MultiLineString, MultiPoint, MultiPolygon, Point, Polygon]";

/// A step of the path Jackson was following in the tree when it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The class of a geometry: `org.wololo.geojson.Point`.
    Class(&'static str),
    /// A property of the class: `["coordinates"]`.
    Field(&'static str),
    /// An element of an array of `array` elements: `->double[][1]`.
    Element {
        /// The class of the elements, as Jackson names the array.
        array: &'static str,
        /// The index of the element.
        index: usize,
    },
}

/// The path Jackson followed in the tree (`through reference chain`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Chain(pub Vec<Step>);

impl Chain {
    /// ` (through reference chain: ...)`, ending with `field` when there is one, or nothing for
    /// the path of the root.
    pub fn through(&self, field: Option<&str>) -> String {
        if self.0.is_empty() && field.is_none() {
            return String::new();
        }
        let steps = self
            .0
            .iter()
            .enumerate()
            .map(|(position, step)| match step {
                Step::Class(name) if position == 0 => format!("org.wololo.geojson.{name}"),
                Step::Class(name) => format!("->org.wololo.geojson.{name}"),
                Step::Field(name) => format!("[\"{name}\"]"),
                Step::Element { array, index } => format!("->{array}[][{index}]"),
            });
        let field = field.map(|field| format!("[\"{field}\"]"));
        let path: String = steps.chain(field).collect();
        format!(" (through reference chain: {path})")
    }
}

/// The kind of JSON value that a type cannot be read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    /// `true`.
    True,
    /// `false`.
    False,
    /// `null`.
    Null,
    /// A string.
    String,
    /// An integer.
    Integer,
    /// A number with a fraction or an exponent.
    Float,
    /// An array.
    Array,
    /// An object.
    Object,
}

impl std::fmt::Display for Found {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, token) = match self {
            Found::Null => ("Null", "VALUE_NULL"),
            Found::String => ("String", "VALUE_STRING"),
            Found::True => ("Boolean", "VALUE_TRUE"),
            Found::False => ("Boolean", "VALUE_FALSE"),
            Found::Integer => ("Integer", "VALUE_NUMBER_INT"),
            Found::Float => ("Floating-point", "VALUE_NUMBER_FLOAT"),
            Found::Array => ("Array", "START_ARRAY"),
            Found::Object => ("Object", "START_OBJECT"),
        };
        write!(f, "{kind} value (token `JsonToken.{token}`)")
    }
}

/// The Java type a value is read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A `double`.
    Double,
    /// A `double[]`, a `double[][]`… of this many dimensions.
    Doubles(usize),
    /// A `Geometry[]`.
    Geometries,
    /// The `Map` of the properties of a feature.
    Properties,
}

impl std::fmt::Display for Target {
    /// The name of the type as the failure of a value of the wrong kind gives it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Double => write!(f, "double"),
            Target::Doubles(dimensions) => write!(f, "{}D", "[".repeat(*dimensions)),
            Target::Geometries => write!(f, "[Lorg.wololo.geojson.Geometry;"),
            Target::Properties => {
                write!(
                    f,
                    "java.util.LinkedHashMap<java.lang.String,java.lang.Object>"
                )
            }
        }
    }
}

/// The Java type a string is read as, which has no constructor from a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringTarget {
    /// A `double[]`, a `double[][]`… of this many dimensions.
    Doubles(usize),
    /// A `Geometry[]`.
    Geometries,
    /// The `Map` of the properties of a feature.
    Properties,
}

/// A failure of the GeoJSON reader.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GeoJsonError {
    /// A text that is not JSON (`JsonParseException`; `JsonEOFException` when the text ends
    /// too early). The words are those of the JSON parser of this library.
    #[error("{message}")]
    Syntax {
        /// The message of the parser.
        message: String,
        /// Whether the text ended in the middle of a value.
        end_of_input: bool,
    },
    /// A GeoJSON `type` that is not the name of a class of the binding
    /// (`ClassNotFoundException`).
    #[error("org.wololo.geojson.{0}")]
    NoClass(String),
    /// The `type` of an abstract class of the binding (`InvalidDefinitionException`).
    #[error(
        "Cannot construct instance of `org.wololo.geojson.GeoJSON` (no Creators, like default \
         constructor, exist): abstract types either need to be mapped to concrete types, have \
         custom deserializer, or contain additional type information  {SOURCE}"
    )]
    AbstractClass,
    /// A property that the class does not have (`UnrecognizedPropertyException`).
    #[error(
        "Unrecognized field \"{field}\" (class org.wololo.geojson.{class}), not marked as \
         ignorable ({} known properties: {}])  {SOURCE}{}",
        known.len(),
        known.iter().map(|name| format!("\"{name}\"")).collect::<Vec<_>>().join(", "),
        chain.through(Some(field))
    )]
    UnknownProperty {
        /// The class of the binding.
        class: &'static str,
        /// The property found.
        field: String,
        /// The properties the class has.
        known: &'static [&'static str],
        /// The path to the object, which ends with the class.
        chain: Chain,
    },
    /// A value that the type cannot be read from (`MismatchedInputException`).
    #[error(
        "Cannot deserialize value of type `{target}` from {found}  {SOURCE}{}",
        chain.through(None)
    )]
    Mismatch {
        /// The type read.
        target: Target,
        /// The value found.
        found: Found,
        /// The path to the value.
        chain: Chain,
    },
    /// A string that is not a number (`InvalidFormatException`).
    #[error(
        "Cannot deserialize value of type `double` from String \"{text}\": not a valid `double` \
         value (as String to convert)  {SOURCE}{}",
        chain.through(None)
    )]
    NotANumber {
        /// The string.
        text: String,
        /// The path to the value.
        chain: Chain,
    },
    /// A string where an array or a map is expected (`InvalidDefinitionException`, and
    /// `MismatchedInputException` for a map).
    #[error(
        "Cannot construct instance of `{}`: no String-argument constructor/factory method to \
         deserialize from String value ('{text}')  {SOURCE}{}",
        match target {
            StringTarget::Doubles(dimensions) => format!("double{}", "[]".repeat(*dimensions)),
            StringTarget::Geometries => "org.wololo.geojson.Geometry[]".to_owned(),
            StringTarget::Properties =>
                "java.util.LinkedHashMap` (although at least one Creator exists".to_owned(),
        },
        chain.through(None)
    )]
    NoStringConstructor {
        /// The type read.
        target: StringTarget,
        /// The string.
        text: String,
        /// The path to the value.
        chain: Chain,
    },
    /// A geometry of a type that is not one of the geometry classes
    /// (`InvalidTypeIdException`). Inside a collection, the path to the property.
    #[error(
        "Could not resolve type id '{id}' as a subtype of `org.wololo.geojson.Geometry`: known \
         type ids = {KNOWN_TYPE_IDS}{}  {SOURCE}{}",
        if chain.is_some() { " (for POJO property 'geometries')" } else { "" },
        chain.as_ref().map(|chain| chain.through(None)).unwrap_or_default()
    )]
    UnknownTypeId {
        /// The type.
        id: String,
        /// The path to the geometry, for a geometry in a collection.
        chain: Option<Chain>,
    },
    /// A feature where a geometry is expected (`InvalidTypeIdException`).
    #[error(
        "Could not resolve type id '{id}' as a subtype of `org.wololo.geojson.Geometry`: Class \
         `org.wololo.geojson.{id}` not subtype of `org.wololo.geojson.Geometry`  {SOURCE}{}",
        chain.through(None)
    )]
    NotSubtype {
        /// The type: `Feature` or `FeatureCollection`.
        id: &'static str,
        /// The path to the geometry.
        chain: Chain,
    },
    /// A geometry in a collection that has no type (`InvalidTypeIdException`).
    #[error(
        "Could not resolve subtype of [simple type, class org.wololo.geojson.Geometry]: missing \
         type id property 'type' (for POJO property 'geometries')  {SOURCE}{}",
        chain.through(None)
    )]
    MissingTypeId {
        /// The path to the geometry.
        chain: Chain,
    },
    /// The geometry of a feature that is a feature (`ClassCastException`).
    #[error("org.wololo.geojson.{0} cannot be cast to org.wololo.geojson.Geometry")]
    NotAGeometry(&'static str),
}

impl GeoJsonError {
    /// The Java class of the exception.
    pub fn exception_class(&self) -> &'static str {
        match self {
            GeoJsonError::Syntax {
                end_of_input: true, ..
            } => "com.fasterxml.jackson.core.io.JsonEOFException",
            GeoJsonError::Syntax { .. } => "com.fasterxml.jackson.core.JsonParseException",
            GeoJsonError::NoClass(_) => "java.lang.ClassNotFoundException",
            GeoJsonError::AbstractClass
            | GeoJsonError::NoStringConstructor {
                target: StringTarget::Doubles(_) | StringTarget::Geometries,
                ..
            } => "com.fasterxml.jackson.databind.exc.InvalidDefinitionException",
            GeoJsonError::UnknownProperty { .. } => {
                "com.fasterxml.jackson.databind.exc.UnrecognizedPropertyException"
            }
            GeoJsonError::Mismatch { .. }
            | GeoJsonError::NoStringConstructor {
                target: StringTarget::Properties,
                ..
            } => "com.fasterxml.jackson.databind.exc.MismatchedInputException",
            GeoJsonError::NotANumber { .. } => {
                "com.fasterxml.jackson.databind.exc.InvalidFormatException"
            }
            GeoJsonError::UnknownTypeId { .. }
            | GeoJsonError::NotSubtype { .. }
            | GeoJsonError::MissingTypeId { .. } => {
                "com.fasterxml.jackson.databind.exc.InvalidTypeIdException"
            }
            GeoJsonError::NotAGeometry(_) => "java.lang.ClassCastException",
        }
    }
}
