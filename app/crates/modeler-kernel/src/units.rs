// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Typed lengths: parsing what the user types into the measurement box, and writing lengths
//! back as text.
//!
//! Behaviour: `docs/specs/m2-units.md`.

/// The unit a bare number is read in, and the unit lengths are written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Millimeter,
    Centimeter,
    Meter,
    Inch,
    /// Written as feet and inches (`5' 4"`); a bare number is read as feet.
    Foot,
}

/// Why a typed length was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Nothing but white space.
    Empty,
    /// Anything that is not a length.
    Invalid,
}

/// Meters in one `unit`.
pub fn meters_per(unit: Unit) -> f64 {
    let _ = unit;
    unimplemented!("m2-units")
}

/// A typed length in meters. A bare number is read in `default`.
pub fn parse_length(input: &str, default: Unit) -> Result<f64, ParseError> {
    let _ = (input, default);
    unimplemented!("m2-units")
}

/// Comma-separated lengths (`5,3` for a rectangle), each as for [`parse_length`].
pub fn parse_lengths(input: &str, default: Unit) -> Result<Vec<f64>, ParseError> {
    let _ = (input, default);
    unimplemented!("m2-units")
}

/// A length in meters as text in `unit`, rounded to `decimals` places of the last unit
/// written (inches for `Foot` and `Inch`).
pub fn format_length(meters: f64, unit: Unit, decimals: u32) -> String {
    let _ = (meters, unit, decimals);
    unimplemented!("m2-units")
}
