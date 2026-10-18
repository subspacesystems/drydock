// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Camera state and the effect of input actions on it. No egui, no GPU.

use modeler_dae::FlatScene;
use modeler_view::{Bounds, Camera};

use crate::input::{Action, Tool};

pub struct ViewState {
    pub camera: Camera,
    pub tool: Tool,
    /// Bounds of the open model, if any.
    pub bounds: Option<Bounds>,
}

/// What the caller must do after an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Show the open-file dialog.
    OpenDialog,
}

/// The scene's bounding box in `modeler_view` terms (`None` for an empty scene).
pub fn scene_bounds(scene: &FlatScene) -> Option<Bounds> {
    let _ = scene;
    unimplemented!("m0-app")
}

impl ViewState {
    /// Default camera, Orbit tool, no model.
    pub fn new() -> ViewState {
        unimplemented!("m0-app")
    }

    /// A model was opened: remember its bounds and frame it (zoom extents, keeping the
    /// current orientation). `None` leaves the camera alone.
    pub fn set_bounds(&mut self, bounds: Option<Bounds>, viewport_px: (f64, f64)) {
        let _ = (bounds, viewport_px);
        unimplemented!("m0-app")
    }

    /// Apply an input action. `viewport_px` is (width, height) of the viewport.
    pub fn apply(&mut self, action: Action, viewport_px: (f64, f64)) -> Effect {
        let _ = (action, viewport_px);
        unimplemented!("m0-app")
    }
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState::new()
    }
}
