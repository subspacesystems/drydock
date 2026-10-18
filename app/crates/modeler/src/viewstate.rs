// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Camera state and the effect of input actions on it. No egui, no GPU.

use glam::DVec3;
use modeler_dae::FlatScene;
use modeler_view::{Bounds, Camera, Projection};

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
    let to_dvec = |v: [f32; 3]| DVec3::new(f64::from(v[0]), f64::from(v[1]), f64::from(v[2]));
    scene.bounds.map(|aabb| Bounds {
        min: to_dvec(aabb.min),
        max: to_dvec(aabb.max),
    })
}

/// Viewport height and aspect; a viewport with a zero side counts as 1 px high, aspect 1.
fn height_and_aspect(viewport_px: (f64, f64)) -> (f64, f64) {
    let (width, height) = viewport_px;
    if width > 0.0 && height > 0.0 && width.is_finite() && height.is_finite() {
        (height, width / height)
    } else {
        (1.0, 1.0)
    }
}

impl ViewState {
    /// Default camera, Orbit tool, no model.
    pub fn new() -> ViewState {
        ViewState {
            camera: Camera::default(),
            tool: Tool::Orbit,
            bounds: None,
        }
    }

    /// A model was opened: remember its bounds and frame it (zoom extents, keeping the
    /// current orientation). `None` leaves the camera alone.
    pub fn set_bounds(&mut self, bounds: Option<Bounds>, viewport_px: (f64, f64)) {
        self.bounds = bounds;
        if let Some(bounds) = bounds {
            let (_, aspect) = height_and_aspect(viewport_px);
            self.camera.zoom_extents(&bounds, aspect);
        }
    }

    /// Apply an input action. `viewport_px` is (width, height) of the viewport.
    pub fn apply(&mut self, action: Action, viewport_px: (f64, f64)) -> Effect {
        let (height, aspect) = height_and_aspect(viewport_px);
        match action {
            Action::Orbit { dx, dy } => self.camera.orbit(dx, dy),
            Action::Pan { dx, dy } => self.camera.pan(dx, dy, height),
            Action::Zoom { steps, cursor_ndc } => self.camera.zoom(steps, cursor_ndc, aspect),
            Action::ZoomExtents => {
                if let Some(bounds) = self.bounds {
                    self.camera.zoom_extents(&bounds, aspect);
                }
            }
            Action::View(view) => self.camera.set_standard_view(view),
            Action::ToggleProjection => {
                self.camera.projection = match self.camera.projection {
                    Projection::Perspective => Projection::Parallel,
                    Projection::Parallel => Projection::Perspective,
                };
            }
            Action::SelectTool(tool) => self.tool = tool,
            Action::Open => return Effect::OpenDialog,
        }
        Effect::None
    }
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState::new()
    }
}
