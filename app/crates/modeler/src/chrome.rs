// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Toolbar and menu bar of the window, kept apart from `App` so tests can drive them
//! with `egui_kittest`. Each function draws into the given `Ui` and returns the action the
//! user chose this frame, if any.
//!
//! Behaviour: `docs/specs/m3-app-ui.md`.

use crate::input::{Action, Tool};

/// What the menus need to know about the editor.
#[derive(Clone, Copy, Debug, Default)]
pub struct MenuState<'a> {
    /// Label of the step Undo would undo, if any.
    pub undo: Option<&'a str>,
    /// Label of the step Redo would redo, if any.
    pub redo: Option<&'a str>,
    /// The camera is in parallel projection.
    pub parallel: bool,
    /// Something is selected (Flip needs it).
    pub has_selection: bool,
}

/// The toolbar's buttons, top to bottom: tool and button label.
pub const TOOL_BUTTONS: [(Tool, &str); 14] = [
    (Tool::Select, "Select"),
    (Tool::Line, "Line"),
    (Tool::Rectangle, "Rectangle"),
    (Tool::Circle, "Circle"),
    (Tool::Arc, "Arc"),
    (Tool::Arc3, "3-Point Arc"),
    (Tool::PushPull, "Push/Pull"),
    (Tool::Move, "Move"),
    (Tool::Rotate, "Rotate"),
    (Tool::Scale, "Scale"),
    (Tool::Tape, "Tape Measure"),
    (Tool::Orbit, "Orbit"),
    (Tool::Pan, "Pan"),
    (Tool::Zoom, "Zoom"),
];

/// The tool buttons; `current` is drawn selected. Clicking one returns
/// `Action::SelectTool`.
pub fn toolbar(ui: &mut eframe::egui::Ui, current: Tool) -> Option<Action> {
    let _ = (ui, current);
    unimplemented!("m3-app-ui")
}

/// The File, Edit and View menus.
pub fn menu_bar(ui: &mut eframe::egui::Ui, state: &MenuState<'_>) -> Option<Action> {
    let _ = (ui, state);
    unimplemented!("m3-app-ui")
}
