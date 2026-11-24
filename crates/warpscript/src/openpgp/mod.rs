// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The OpenPGP format (RFC 4880) as the OpenPGP functions of Warp 10 read and write it through
//! BouncyCastle: the ASCII armor, the packets, the keys and the key rings.
//!
//! The functions of this format are in [`crate::functions::pgp`]. Nothing here depends on a
//! third-party OpenPGP library: the behaviour to reproduce is the one BouncyCastle has, down to
//! its error messages.

pub mod armor;
#[cfg(test)]
mod armor_tests;
pub mod key;
#[cfg(test)]
mod key_tests;
pub mod packet;
#[cfg(test)]
mod packet_tests;
#[cfg(test)]
pub(crate) mod pgp_fixtures;
pub mod ring;
#[cfg(test)]
mod ring_tests;
pub mod secret;
#[cfg(test)]
mod secret_tests;
pub mod sign;
#[cfg(test)]
mod sign_tests;
pub mod signature;
#[cfg(test)]
mod signature_tests;
pub mod symmetric;
#[cfg(test)]
mod symmetric_tests;
