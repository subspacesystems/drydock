// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Units section of Model Info: a unit list (five decimal units and Architectural) and
//! a precision list, and the small helpers the app's length fields share. Kept apart from
//! `App` so tests can drive it with `egui_kittest`; the section returns the format picked
//! and the app applies it through `Editor::set_length_format`.
//!
//! Behaviour: `docs/specs/m6-units-ui.md`.

use eframe::egui;
use modeler_kernel::units::{LengthFormat, Unit};

/// The rows of the "Unit:" list, top to bottom.
pub const UNIT_NAMES: [&str; 6] = [
    "Millimeters",
    "Centimeters",
    "Meters",
    "Inches",
    "Feet",
    "Architectural",
];

/// The row of [`UNIT_NAMES`] that shows `format`.
pub fn unit_index(format: LengthFormat) -> usize {
    let _ = format;
    unimplemented!("m6-units-ui")
}

/// `format` after picking row `index` of [`UNIT_NAMES`]: a decimal unit keeps the decimal
/// places, Architectural from a decimal unit starts at 1/16", a decimal unit from
/// Architectural starts at 2 places; the row already shown, or an index past the list,
/// changes nothing.
pub fn pick_unit(format: LengthFormat, index: usize) -> LengthFormat {
    let _ = (format, index);
    unimplemented!("m6-units-ui")
}

/// The rows of the "Precision:" list for `format`'s unit, top to bottom: 0 to
/// `LengthFormat::MAX_DECIMALS` places, or each of `LengthFormat::DENOMINATORS`.
pub fn precisions(format: LengthFormat) -> Vec<LengthFormat> {
    let _ = format;
    unimplemented!("m6-units-ui")
}

/// A precision as the list shows it: `0`, `0.0`, `0.00` ... for decimal places; `1"`,
/// `1/2"` ... `1/64"` for Architectural.
pub fn precision_label(format: LengthFormat) -> String {
    let _ = format;
    unimplemented!("m6-units-ui")
}

/// The short name of `unit` used in field labels: `mm`, `cm`, `m`, `in`, `ft`.
pub fn unit_abbrev(unit: Unit) -> &'static str {
    let _ = unit;
    unimplemented!("m6-units-ui")
}

/// Draw the Units section into `ui`: a "Units" heading, then "Unit:" and "Precision:" rows,
/// each a combo box labelled by its row label. `Some` on the frame the user picks a format
/// other than `format`.
pub fn units_section(ui: &mut egui::Ui, format: LengthFormat) -> Option<LengthFormat> {
    let _ = (ui, format);
    unimplemented!("m6-units-ui")
}

/// A drag value for a length kept in meters in `meters`, shown and typed in `unit`.
pub fn length_drag(meters: &mut f64, unit: Unit) -> egui::DragValue<'_> {
    let _ = (meters, unit);
    unimplemented!("m6-units-ui")
}
