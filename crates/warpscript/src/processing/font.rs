// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! `PFONT`: a font, named by `createFont`, which `textFont` selects and `text` draws with.

/// A font: its name and the size it was created with.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    /// The name the font was created with.
    pub name: String,
    /// The size the font was created with.
    pub size: f32,
}
