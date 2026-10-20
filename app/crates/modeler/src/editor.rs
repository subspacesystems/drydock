// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Drawing and selection in the viewport: runs inference and the kernel's tools on the
//! model's root body, and keeps the selection, the undo stack and the typed text of the
//! measurement box. No egui, no GPU: the window code feeds it pointer and key events.
//!
//! Behaviour: `docs/specs/m2-app-editor.md`.

use glam::{DVec2, DVec3};
use modeler_kernel::infer::Inference;
use modeler_kernel::model::{Model, UndoStack};
use modeler_kernel::select::Entity;
use modeler_kernel::units::Unit;
use modeler_view::Camera;

use crate::input::{EditKey, Modifiers, Tool};

/// Snap radius for inference and picking, pixels.
pub const SNAP_RADIUS_PX: f64 = 10.0;

/// The camera and the viewport size, (width, height) in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub camera: Camera,
    pub size_px: (f64, f64),
}

impl Viewport {
    /// The ray through the pixel `cursor_px`: (origin, unit direction).
    pub fn ray(&self, cursor_px: DVec2) -> (DVec3, DVec3) {
        let _ = cursor_px;
        unimplemented!()
    }

    /// The pixel a world point appears at; `None` when it is behind a perspective camera.
    pub fn project(&self, p: DVec3) -> Option<DVec2> {
        let _ = p;
        unimplemented!()
    }
}

pub struct Editor {
    /// The unit for typed numbers without a unit, and for the measurement display.
    pub unit: Unit,
}

impl Editor {
    /// Orbit tool, nothing selected, empty undo stack, inches.
    pub fn new() -> Editor {
        unimplemented!()
    }

    pub fn tool(&self) -> Tool {
        unimplemented!()
    }

    /// Switch tools; the shape in progress, the typed text and the locks are dropped.
    pub fn set_tool(&mut self, tool: Tool) {
        let _ = tool;
        unimplemented!()
    }

    /// Sorted, no duplicates; only entities of the root body.
    pub fn selection(&self) -> &[Entity] {
        unimplemented!()
    }

    pub fn undo_stack(&self) -> &UndoStack {
        unimplemented!()
    }

    /// The cursor moved over the viewport. Never changes the model (the tools need it
    /// mutably).
    pub fn pointer_move(
        &mut self,
        model: &mut Model,
        view: &Viewport,
        cursor: DVec2,
        mods: Modifiers,
    ) {
        let _ = (model, view, cursor, mods);
        unimplemented!()
    }

    /// A click; `count` is 1, 2 (double click) or 3 (triple click). True when the model
    /// changed.
    pub fn click(
        &mut self,
        model: &mut Model,
        view: &Viewport,
        cursor: DVec2,
        mods: Modifiers,
        count: u32,
    ) -> bool {
        let _ = (model, view, cursor, mods, count);
        unimplemented!()
    }

    /// A selection box dragged from `start` to `end` (pixels) and released.
    pub fn box_select(
        &mut self,
        model: &Model,
        view: &Viewport,
        start: DVec2,
        end: DVec2,
        mods: Modifiers,
    ) {
        let _ = (model, view, start, end, mods);
        unimplemented!()
    }

    /// Typed characters for the measurement box. True when they were taken.
    pub fn text(&mut self, text: &str) -> bool {
        let _ = text;
        unimplemented!()
    }

    /// An editor key. True when the model changed.
    pub fn key(&mut self, key: EditKey, model: &mut Model) -> bool {
        let _ = (key, model);
        unimplemented!()
    }

    /// The measurement box's typed text.
    pub fn typed(&self) -> &str {
        unimplemented!()
    }

    /// The measurement box's label and value for the shape in progress.
    pub fn measurement(&self) -> Option<(&'static str, String)> {
        unimplemented!()
    }

    /// The last inferred point, for the hint.
    pub fn hint(&self) -> Option<Inference> {
        unimplemented!()
    }

    /// The direction the drawing is locked to, if any.
    pub fn lock(&self) -> Option<DVec3> {
        unimplemented!()
    }

    /// Edges of the shape in progress, as the next click would draw them.
    pub fn preview(&self) -> Vec<(DVec3, DVec3)> {
        unimplemented!()
    }

    /// Why the last input was refused, for the status bar.
    pub fn message(&self) -> Option<&str> {
        unimplemented!()
    }
}

impl Default for Editor {
    fn default() -> Self {
        Editor::new()
    }
}
