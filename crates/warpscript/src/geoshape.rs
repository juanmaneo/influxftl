// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from GeoXP Lib (Copyright 1999-2019 Mathias Herberts, 2019-2020 iroise.net S.A.S.,
// 2020- SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `GEOSHAPE`: a set of geocells (`GeoXPLib.GeoXPShape`).
//!
//! A geocell packs the resolution (`1` to `15`, half the number of bits of each coordinate
//! divided by two) in its 4 most significant bits and the leading 60 bits of the HHCode of
//! the cell in the others. The cells of a shape are sorted as signed longs, which is why
//! the cells of resolutions 8 to 15 come before those of resolutions 1 to 7.

use crate::encoder::{
    DEFAULT_COMP_RATIO_THRESHOLD, EncodedValue, GtsEncoder, encoder_to_wrapper, wrapper_to_decoder,
};
use crate::encoding::opb64_encode;
use crate::error::Result;
use crate::error::fault::BoundsError;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::thrift::{deserialize_wrapper, serialize_wrapper};

/// The geocells of a `GEOSHAPE`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct GeoShape {
    cells: Vec<i64>,
}

impl GeoShape {
    /// `GeoXPLib.fromCells`: the shape of `cells`, which the caller has sorted.
    pub fn from_cells(cells: Vec<i64>) -> GeoShape {
        GeoShape { cells }
    }

    /// `GeoXPLib.getCells`.
    pub fn cells(&self) -> &[i64] {
        &self.cells
    }

    /// `GEOPACK.pack`: the cells as the timestamps of a gzipped `GTSWrapper`, in order
    /// preserving base 64.
    pub fn pack(&self) -> String {
        let mut encoder = GtsEncoder::new(0);
        for cell in &self.cells {
            encoder.add_value(
                *cell,
                NO_LOCATION,
                NO_ELEVATION,
                &EncodedValue::Boolean(true),
            );
        }
        let wrapper = encoder_to_wrapper(&encoder, true, DEFAULT_COMP_RATIO_THRESHOLD, true);
        opb64_encode(&serialize_wrapper(&wrapper))
    }

    /// `GEOUNPACK`: the shape of a serialized `GTSWrapper` holding geocells as timestamps.
    /// The timestamps whose resolution is 0 are not cells and are dropped; the cells are
    /// sorted and deduplicated.
    pub fn unpack(serialized: &[u8]) -> Result<GeoShape> {
        let wrapper = deserialize_wrapper(serialized)?;
        let mut decoder = wrapper_to_decoder(&wrapper)?;
        // `(int) wrapper.getCount()` sizes the array the cells are read into.
        let count = wrapper.count() as i32;
        if count < 0 {
            return Err(BoundsError::NegativeSize(i64::from(count)).into());
        }
        let mut cells = Vec::new();
        while decoder.next()? {
            let cell = decoder.timestamp();
            if cell as u64 >> 60 != 0 {
                cells.push(cell);
            }
        }
        cells.sort_unstable();
        cells.dedup();
        Ok(GeoShape { cells })
    }
}
