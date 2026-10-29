// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Overhang tint: triangles that face down more steeply than a printer can bridge, moved
//! into one batch drawn in [`OVERHANG_COLOR`].
//!
//! Behaviour: `docs/specs/m7-overhang.md`.

use glam::DVec3;

use crate::FlatScene;

/// The overhang angle the app starts with, in degrees from vertical.
pub const DEFAULT_OVERHANG_DEGREES: f64 = 45.0;

/// Color of the material [`tint_overhangs`] adds.
pub const OVERHANG_COLOR: [f32; 4] = [0.9, 0.2, 0.15, 1.0];

/// Whether a surface with front normal `normal` (any length) leans past `max_degrees` from
/// vertical while facing down.
pub fn is_overhang(normal: DVec3, max_degrees: f64) -> bool {
    let _ = (normal, max_degrees);
    unimplemented!()
}

/// A copy of `scene` with every overhanging triangle that isn't on the bed moved into one
/// batch drawn in [`OVERHANG_COLOR`].
pub fn tint_overhangs(scene: &FlatScene, max_degrees: f64) -> FlatScene {
    let _ = (scene, max_degrees);
    unimplemented!()
}
