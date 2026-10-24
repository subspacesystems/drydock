// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Camera state and the effect of input actions on it. No egui, no GPU.

use glam::DVec3;
use modeler_dae::FlatScene;
use modeler_view::{Bounds, Camera, Projection};

use crate::input::{Action, EditKey, Tool};

pub struct ViewState {
    pub camera: Camera,
    pub tool: Tool,
    /// Bounds of the open model, if any.
    pub bounds: Option<Bounds>,
    /// The point the current orbit drag turns about (what was under the cursor when the
    /// drag started); `None` orbits about the camera target. `docs/specs/m4-orbit-pivot.md`.
    pub pivot: Option<DVec3>,
    /// The red, green and blue axes are drawn (View > Axes; `docs/specs/m4-ui-axes.md`).
    pub axes: bool,
}

/// What the caller must do after an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Start a new empty document (asking first when the open one is modified).
    New,
    /// Show the open-file dialog.
    OpenDialog,
    /// Save the document (`docs/specs/m1-app-native.md`).
    Save,
    /// Show the save dialog.
    SaveAsDialog,
    /// Pass the key to the editor.
    Edit(EditKey),
    /// Flip the selection about an axis (0 red, 1 green, 2 blue).
    Flip(usize),
    /// Erase the selection.
    Erase,
    /// Copy the selection to the clipboard.
    Copy,
    /// Copy the selection to the clipboard, then erase it.
    Cut,
    /// Start pasting the clipboard.
    Paste,
    /// Show the About window.
    About,
    /// Ask for a name, then make the selection a component (or a group when `group`).
    MakeComponent {
        group: bool,
    },
    /// Explode the selected instance.
    Explode,
    /// Flip the editor's hide-the-rest option.
    ToggleHideRest,
    /// Open the recent file at this index (`docs/specs/m4-ui-menus.md`).
    OpenRecent(usize),
    /// Quit the app (asking first when the document is modified).
    Exit,
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
            pivot: None,
            axes: true,
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
            Action::Orbit { dx, dy } => match self.pivot {
                Some(pivot) => self.camera.orbit_about(pivot, dx, dy),
                None => self.camera.orbit(dx, dy),
            },
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
            Action::New => return Effect::New,
            Action::Open => return Effect::OpenDialog,
            Action::Save => return Effect::Save,
            Action::SaveAs => return Effect::SaveAsDialog,
            Action::Edit(key) => return Effect::Edit(key),
            Action::Flip(axis) => return Effect::Flip(axis),
            Action::Erase => return Effect::Erase,
            Action::Copy => return Effect::Copy,
            Action::Cut => return Effect::Cut,
            Action::Paste => return Effect::Paste,
            Action::About => return Effect::About,
            Action::MakeComponent => return Effect::MakeComponent { group: false },
            Action::MakeGroup => return Effect::MakeComponent { group: true },
            Action::Explode => return Effect::Explode,
            Action::ToggleHideRest => return Effect::ToggleHideRest,
            Action::ToggleAxes => unimplemented!("m4-ui-axes"),
            Action::OpenRecent(index) => return Effect::OpenRecent(index),
            Action::Exit => return Effect::Exit,
        }
        Effect::None
    }
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState::new()
    }
}
