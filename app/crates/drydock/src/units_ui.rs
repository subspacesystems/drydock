// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Units section of Model Info: a unit list (five decimal units and Architectural) and
//! a precision list, and the small helpers the app's length fields share. Kept apart from
//! `App` so tests can drive it with `egui_kittest`; the section returns the format picked
//! and the app applies it through `Editor::set_length_format`.
//!
//! Behaviour: `docs/specs/m6-units-ui.md`.

use drydock_kernel::units::{LengthFormat, Unit, meters_per};
use eframe::egui;

use crate::dialog::field_label;

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
    match format {
        LengthFormat::Decimal { unit, .. } => match unit {
            Unit::Millimeter => 0,
            Unit::Centimeter => 1,
            Unit::Meter => 2,
            Unit::Inch => 3,
            Unit::Foot => 4,
        },
        LengthFormat::Architectural { .. } => 5,
    }
}

/// `format` after picking row `index` of [`UNIT_NAMES`]: a decimal unit keeps the decimal
/// places, Architectural from a decimal unit starts at 1/16", a decimal unit from
/// Architectural starts at 2 places; the row already shown, or an index past the list,
/// changes nothing.
pub fn pick_unit(format: LengthFormat, index: usize) -> LengthFormat {
    if index >= UNIT_NAMES.len() || index == unit_index(format) {
        return format;
    }
    let unit = match index {
        0 => Unit::Millimeter,
        1 => Unit::Centimeter,
        2 => Unit::Meter,
        3 => Unit::Inch,
        4 => Unit::Foot,
        _ => return LengthFormat::Architectural { denominator: 16 },
    };
    match format {
        LengthFormat::Decimal { decimals, .. } => LengthFormat::Decimal { unit, decimals },
        LengthFormat::Architectural { .. } => LengthFormat::Decimal { unit, decimals: 2 },
    }
}

/// The rows of the "Precision:" list for `format`'s unit, top to bottom: 0 to
/// `LengthFormat::MAX_DECIMALS` places, or each of `LengthFormat::DENOMINATORS`.
pub fn precisions(format: LengthFormat) -> Vec<LengthFormat> {
    match format {
        LengthFormat::Decimal { unit, .. } => (0..=LengthFormat::MAX_DECIMALS)
            .map(|decimals| LengthFormat::Decimal { unit, decimals })
            .collect(),
        LengthFormat::Architectural { .. } => LengthFormat::DENOMINATORS
            .into_iter()
            .map(|denominator| LengthFormat::Architectural { denominator })
            .collect(),
    }
}

/// A precision as the list shows it: `0`, `0.0`, `0.00` ... for decimal places; `1"`,
/// `1/2"` ... `1/64"` for Architectural.
pub fn precision_label(format: LengthFormat) -> String {
    match format {
        LengthFormat::Decimal { decimals, .. } => {
            if decimals == 0 {
                "0".to_owned()
            } else {
                format!("0.{}", "0".repeat(usize::from(decimals)))
            }
        }
        LengthFormat::Architectural { denominator: 1 } => "1\"".to_owned(),
        LengthFormat::Architectural { denominator } => format!("1/{denominator}\""),
    }
}

/// The short name of `unit` used in field labels: `mm`, `cm`, `m`, `in`, `ft`.
pub fn unit_abbrev(unit: Unit) -> &'static str {
    match unit {
        Unit::Millimeter => "mm",
        Unit::Centimeter => "cm",
        Unit::Meter => "m",
        Unit::Inch => "in",
        Unit::Foot => "ft",
    }
}

/// Draw the Units section into `ui`: a "Units" heading, then "Unit:" and "Precision:" rows,
/// each a combo box labelled by its row label. `Some` on the frame the user picks a format
/// other than `format`.
pub fn units_section(ui: &mut egui::Ui, format: LengthFormat) -> Option<LengthFormat> {
    let mut picked = None;
    ui.strong("Units");

    let unit_label = ui.label(field_label("Unit"));
    egui::ComboBox::from_id_salt("units-unit")
        .selected_text(UNIT_NAMES[unit_index(format)])
        .show_ui(ui, |ui| {
            for (index, name) in UNIT_NAMES.into_iter().enumerate() {
                let selected = index == unit_index(format);
                if ui.selectable_label(selected, name).clicked() {
                    let new = pick_unit(format, index);
                    if new != format {
                        picked = Some(new);
                    }
                }
            }
        })
        .response
        .labelled_by(unit_label.id);

    let precision = ui.label(field_label("Precision"));
    egui::ComboBox::from_id_salt("units-precision")
        .selected_text(precision_label(format))
        .show_ui(ui, |ui| {
            for candidate in precisions(format) {
                let selected = candidate == format;
                if ui
                    .selectable_label(selected, precision_label(candidate))
                    .clicked()
                    && candidate != format
                {
                    picked = Some(candidate);
                }
            }
        })
        .response
        .labelled_by(precision.id);

    picked
}

/// A drag value for a length kept in meters in `meters`, shown and typed in `unit`.
pub fn length_drag(meters: &mut f64, unit: Unit) -> egui::DragValue<'_> {
    let per = meters_per(unit);
    egui::DragValue::from_get_set(move |value| {
        if let Some(value) = value {
            *meters = value * per;
        }
        *meters / per
    })
}
