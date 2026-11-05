// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Inference feedback text and colors for the viewport (`docs/specs/m2-hints.md`).

use crate::look::{self, Theme};
use drydock_kernel::infer::{Inference, Snap};

/// What to draw next to the cursor for an inferred point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HintLabel {
    pub text: String,
    /// RGB of the marker, the label and the axis line.
    pub color: [u8; 3],
}

/// Display color of an axis index (0 red, 1 green, 2 blue) in `theme`; gray for anything
/// else (`look::axis_color`).
pub fn axis_color(theme: Theme, axis: usize) -> [u8; 3] {
    look::axis_color(theme, axis)
}

/// Marker color for a snap kind when no axis applies.
pub fn snap_color(snap: Snap) -> [u8; 3] {
    match snap {
        Snap::Endpoint(_) => [40, 200, 80],
        Snap::Midpoint(_) | Snap::Center(_) => [60, 200, 220],
        Snap::OnEdge(_) => [220, 70, 60],
        Snap::OnFace(_) => [70, 130, 230],
        Snap::OnAxis(_) | Snap::Ground | Snap::Along => [160, 160, 160],
    }
}

fn axis_name(axis: usize) -> &'static str {
    match axis {
        0 => "Red",
        1 => "Green",
        2 => "Blue",
        _ => "?",
    }
}

/// Name of a snap kind, e.g. "Endpoint" or "On Red Axis".
pub fn snap_name(snap: Snap) -> String {
    match snap {
        Snap::Endpoint(_) => "Endpoint".to_owned(),
        Snap::Midpoint(_) => "Midpoint".to_owned(),
        Snap::Center(_) => "Center".to_owned(),
        Snap::OnEdge(_) => "On Edge".to_owned(),
        Snap::OnFace(_) => "On Face".to_owned(),
        Snap::OnAxis(axis) => format!("On {} Axis", axis_name(axis)),
        Snap::Ground => "On Ground".to_owned(),
        Snap::Along => "Locked Direction".to_owned(),
    }
}

/// The label for an inference.
pub fn hint_label(inference: &Inference, theme: Theme) -> HintLabel {
    let snap = snap_name(inference.snap);
    match inference.axis {
        None => HintLabel {
            text: snap,
            color: snap_color(inference.snap),
        },
        Some(axis) if inference.snap == Snap::OnAxis(axis) => HintLabel {
            text: snap,
            color: axis_color(theme, axis),
        },
        Some(axis) => HintLabel {
            text: format!("{snap} on {} Axis", axis_name(axis)),
            color: axis_color(theme, axis),
        },
    }
}
