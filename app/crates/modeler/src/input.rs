// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Mouse and keyboard to viewport actions. Pure functions; the window code feeds them
//! egui input and applies the result to the camera.

use glam::DVec2;
use modeler_view::StandardView;

/// Pixels of vertical drag per zoom step with the Zoom tool.
pub const ZOOM_DRAG_PIXELS_PER_STEP: f64 = 25.0;

/// The active viewport tool (left mouse button).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Orbit,
    Pan,
    Zoom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// What an input does to the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Orbit { dx: f64, dy: f64 },
    Pan { dx: f64, dy: f64 },
    Zoom { steps: f64, cursor_ndc: DVec2 },
    ZoomExtents,
    View(StandardView),
    ToggleProjection,
    SelectTool(Tool),
    Open,
}

/// A mouse drag of `delta` pixels (screen space, y down) with `button` held.
pub fn drag(
    tool: Tool,
    button: Button,
    mods: Modifiers,
    delta: DVec2,
    start_ndc: DVec2,
) -> Option<Action> {
    let _ = (tool, button, mods, delta, start_ndc);
    unimplemented!("m0-app")
}

/// A scroll of `lines` notches (positive = away from the user) with the cursor at
/// `cursor_ndc`.
pub fn scroll(lines: f64, cursor_ndc: DVec2) -> Action {
    let _ = (lines, cursor_ndc);
    unimplemented!("m0-app")
}

/// A key press. `key` is the egui key name (e.g. "O", "Z", "F2").
pub fn key(key: &str, mods: Modifiers) -> Option<Action> {
    let _ = (key, mods);
    unimplemented!("m0-app")
}
