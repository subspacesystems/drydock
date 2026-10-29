// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Camera state and the effect of input actions on it. No egui, no GPU.

use glam::{DVec2, DVec3};
use modeler_dae::FlatScene;
use modeler_view::{Bounds, Camera, Projection};

use crate::dock::PanelId;
use crate::input::{Action, EditKey, Tool};

pub struct ViewState {
    pub camera: Camera,
    pub tool: Tool,
    /// Bounds of the open model, if any.
    pub bounds: Option<Bounds>,
    /// The point the current orbit drag turns about (what was under the cursor when the
    /// drag started); `None` orbits about the camera target. `docs/specs/m4-orbit-pivot.md`.
    pub pivot: Option<DVec3>,
    /// The point the current Zoom-tool drag scales about (the model point under the cursor
    /// when the drag started); `None` zooms toward the target-plane point under the cursor.
    /// `docs/specs/m4-zoom-cursor.md`.
    pub zoom_pivot: Option<DVec3>,
    /// The red, green and blue axes are drawn (View > Axes; `docs/specs/m4-ui-axes.md`).
    pub axes: bool,
    /// Overhanging faces are tinted (View > Overhangs; `docs/specs/m7-overhang.md`).
    pub overhangs: bool,
    /// The overhang angle in degrees from vertical.
    pub overhang_degrees: f64,
    /// Ctrl+wheel notches not yet turned into a step through a surface (a fraction of a
    /// notch from a smooth wheel or touchpad). `docs/specs/m4-zoom-through.md`.
    pub through_carry: f64,
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
    /// Show the Model Info window.
    ModelInfo,
    /// Show or hide the Shadows window.
    ToggleShadows,
    /// Show or hide one of the docking panels.
    TogglePanel(PanelId),
    /// Put every panel back in its default place.
    ResetPanels,
    /// Ask for a name, then make the selection a component (or a group when `group`).
    MakeComponent {
        group: bool,
    },
    /// Explode the selected instance.
    Explode,
    /// Flip the editor's hide-the-rest option.
    ToggleHideRest,
    /// The overhang tint changed: upload the scene again (`docs/specs/m7-overhang.md`).
    ReuploadScene,
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

/// The model point a zoom with the cursor at `cursor_px` scales about: where the cursor's
/// ray first meets a triangle of `scene`; `None` over empty space or without a scene.
/// `viewport_px` is the viewport (width, height). Behaviour: `docs/specs/m4-zoom-cursor.md`.
pub fn zoom_pivot_at(
    camera: &Camera,
    scene: Option<&FlatScene>,
    viewport_px: (f64, f64),
    cursor_px: DVec2,
) -> Option<DVec3> {
    let scene = scene?;
    let viewport = crate::editor::Viewport {
        camera: *camera,
        size_px: viewport_px,
    };
    let (origin, dir) = viewport.ray(cursor_px);
    scene.raycast(origin, dir)
}

/// How far the eye travels along the cursor's ray to step through the first surface it
/// meets. `hits` are distances along the (unit) ray to every triangle it crosses, in any
/// order, repeats allowed; `None` when nothing is ahead. Behaviour:
/// `docs/specs/m4-zoom-through.md`.
pub fn through_distance(hits: &[f64]) -> Option<f64> {
    let valid = || hits.iter().copied().filter(|h| h.is_finite() && *h > 0.0);
    let t1 = valid().fold(f64::INFINITY, f64::min);
    if !t1.is_finite() {
        return None;
    }
    let same = t1 + 1e-6 * t1 + 1e-9;
    let t2 = valid().filter(|h| *h > same).fold(f64::INFINITY, f64::min);
    Some(if t2.is_finite() {
        t1 + (t2 - t1) / 2.0
    } else {
        2.0 * t1
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
            zoom_pivot: None,
            axes: true,
            overhangs: false,
            overhang_degrees: modeler_dae::DEFAULT_OVERHANG_DEGREES,
            through_carry: 0.0,
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

    /// A mouse-wheel scroll of `lines` notches (positive = away from the user, zoom in) with
    /// the cursor at `cursor_px`: zooms about the model point under the cursor, or over empty
    /// space as `Action::Zoom` does. Behaviour: `docs/specs/m4-zoom-cursor.md`.
    pub fn scroll_zoom(
        &mut self,
        lines: f64,
        cursor_px: DVec2,
        scene: Option<&FlatScene>,
        viewport_px: (f64, f64),
    ) {
        if lines == 0.0 {
            return;
        }
        if let Some(pivot) = zoom_pivot_at(&self.camera, scene, viewport_px, cursor_px) {
            self.camera.zoom_about(lines, pivot);
            return;
        }
        let (width, height) = viewport_px;
        let (_, aspect) = height_and_aspect(viewport_px);
        let ndc = DVec2::new(
            2.0 * cursor_px.x / width - 1.0,
            1.0 - 2.0 * cursor_px.y / height,
        );
        self.camera.zoom(lines, ndc, aspect);
    }

    /// A Ctrl+wheel scroll of `lines` notches with the cursor at `cursor_px`: each whole
    /// notch forward steps the eye through the next surface under the cursor, each notch
    /// back steps out through the surface behind it. Behaviour:
    /// `docs/specs/m4-zoom-through.md`.
    pub fn scroll_through(
        &mut self,
        lines: f64,
        cursor_px: DVec2,
        scene: Option<&FlatScene>,
        viewport_px: (f64, f64),
    ) {
        if lines == 0.0 {
            return;
        }
        let Some(scene) = scene else {
            self.scroll_zoom(lines, cursor_px, scene, viewport_px);
            self.through_carry = 0.0;
            return;
        };
        if self.camera.projection == Projection::Parallel {
            self.scroll_zoom(lines, cursor_px, Some(scene), viewport_px);
            self.through_carry = 0.0;
            return;
        }
        self.through_carry += lines;
        while self.through_carry >= 1.0 {
            self.step_through(1.0, cursor_px, scene, viewport_px);
            self.through_carry -= 1.0;
        }
        while self.through_carry <= -1.0 {
            self.step_through(-1.0, cursor_px, scene, viewport_px);
            self.through_carry += 1.0;
        }
    }

    /// One notch of `direction` (+1 forward, -1 back) through the surface under the cursor.
    fn step_through(
        &mut self,
        direction: f64,
        cursor_px: DVec2,
        scene: &FlatScene,
        viewport_px: (f64, f64),
    ) {
        let viewport = crate::editor::Viewport {
            camera: self.camera,
            size_px: viewport_px,
        };
        let (origin, dir) = viewport.ray(cursor_px);
        let dir = dir.normalize();
        let distance = if direction > 0.0 {
            through_distance(&scene.ray_hits(origin, dir))
        } else {
            through_distance(&scene.ray_hits(origin, -dir))
        };
        match distance {
            Some(distance) if direction > 0.0 => self.camera.target += dir * distance,
            Some(distance) => self.camera.target -= dir * distance,
            None => self.scroll_zoom(direction, cursor_px, Some(scene), viewport_px),
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
            Action::Zoom { steps, cursor_ndc } => match self.zoom_pivot {
                Some(pivot) => self.camera.zoom_about(steps, pivot),
                None => self.camera.zoom(steps, cursor_ndc, aspect),
            },
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
            Action::ModelInfo => return Effect::ModelInfo,
            Action::ToggleShadows => return Effect::ToggleShadows,
            Action::TogglePanel(id) => return Effect::TogglePanel(id),
            Action::ResetPanels => return Effect::ResetPanels,
            Action::MakeComponent => return Effect::MakeComponent { group: false },
            Action::MakeGroup => return Effect::MakeComponent { group: true },
            Action::Explode => return Effect::Explode,
            Action::ToggleHideRest => return Effect::ToggleHideRest,
            Action::ToggleAxes => self.axes = !self.axes,
            Action::ToggleOverhangs => unimplemented!("m7-overhang"),
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
