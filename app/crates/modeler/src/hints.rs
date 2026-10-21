// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Inference feedback text and colors for the viewport (`docs/specs/m2-hints.md`).

use modeler_kernel::infer::{Inference, Snap};

/// What to draw next to the cursor for an inferred point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HintLabel {
    pub text: String,
    /// RGB of the marker, the label and the axis line.
    pub color: [u8; 3],
}

/// Display color of an axis index (0 red, 1 green, 2 blue); gray for anything else.
pub fn axis_color(axis: usize) -> [u8; 3] {
    let _ = axis;
    unimplemented!("m2-hints")
}

/// Marker color for a snap kind when no axis applies.
pub fn snap_color(snap: Snap) -> [u8; 3] {
    let _ = snap;
    unimplemented!("m2-hints")
}

/// Name of a snap kind, e.g. "Endpoint" or "On Red Axis".
pub fn snap_name(snap: Snap) -> String {
    let _ = snap;
    unimplemented!("m2-hints")
}

/// The label for an inference.
pub fn hint_label(inference: &Inference) -> HintLabel {
    let _ = inference;
    unimplemented!("m2-hints")
}
