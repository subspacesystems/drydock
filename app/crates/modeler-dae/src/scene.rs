// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Kernel model to world-space draw batches, so kernel models render like imports.
//!
//! Behaviour: `docs/specs/m1-kernel-scene.md`.

use modeler_kernel::model as km;

use crate::FlatScene;

/// Expand every instance of `model` into triangles (faces) and line segments (edges) in
/// world space, ready for the renderer.
pub fn kernel_scene(model: &km::Model) -> FlatScene {
    let _ = model;
    unimplemented!()
}
