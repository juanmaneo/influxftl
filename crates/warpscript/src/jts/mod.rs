// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Geometries: the formats `GEO.WKT`, `GEO.WKB`, `GEO.JSON`, `->WKT`, `->WKB` and `->GEOJSON`
//! read and write, and the cover of a geometry by geo cells (a `GEOSHAPE`).
//!
//! Warp 10 does that with the JTS Topology Suite and its GeoJSON binding. Neither is ported:
//! their behaviour, quirks included, is what the modules reproduce, from what scripts can see
//! (see `docs/DIVERGENCES.md`).

pub mod cover;
pub mod geojson;
pub mod geometry;
pub mod gml;
pub mod outline;
pub mod wkb;
pub mod wkt;
pub mod xml;

#[cfg(test)]
mod cover_tests;
#[cfg(test)]
mod geometry_tests;
#[cfg(test)]
mod gml_tests;
#[cfg(test)]
mod outline_tests;
#[cfg(test)]
mod wkb_tests;
#[cfg(test)]
mod wkt_tests;
#[cfg(test)]
mod xml_tests;
