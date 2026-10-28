// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Erase in the editor (`docs/specs/m3-app-erase.md`).

use glam::{DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{self, Action, EditKey, Modifiers, Tool};
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::select::Entity;
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::topology::Context;
use modeler_kernel::units::Unit;
use modeler_view::{Bounds, Camera, Projection, StandardView};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn top_view() -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Top);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(-8.0),
            max: DVec3::splat(8.0),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

const SQUARE: [DVec3; 4] = [
    DVec3::new(1.0, 0.0, 0.0),
    DVec3::new(2.0, 0.0, 0.0),
    DVec3::new(2.0, 1.0, 0.0),
    DVec3::new(1.0, 1.0, 0.0),
];

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
}

impl World {
    /// Top view, the unit square at x 1..2, y 0..1 drawn into the model, Select tool.
    fn new() -> World {
        let mut w = World {
            model: Model::new(),
            editor: Editor::new(),
            view: top_view(),
        };
        set_unit(&mut w.model, Unit::Meter);
        let segs: Vec<_> = (0..4).map(|i| (SQUARE[i], SQUARE[(i + 1) % 4])).collect();
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut w.model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segs, "Draw").expect("draw");
        w.editor.set_tool(Tool::Select);
        w
    }

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    fn counts(&self) -> (usize, usize) {
        (self.ctx().edge_count(), self.ctx().face_count())
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn click(&mut self, p: DVec3) -> bool {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, NONE);
        self.editor.click(&mut self.model, &self.view, c, NONE, 1)
    }

    fn select_at(&mut self, p: DVec3) {
        self.editor.set_tool(Tool::Select);
        self.click(p);
        assert_eq!(self.editor.selection().len(), 1, "selected at {p:?}");
    }

    fn undo_label(&self) -> Option<String> {
        self.editor.undo_stack().undo_label().map(str::to_owned)
    }
}

#[test]
fn empty_selection_is_refused() {
    let mut w = World::new();
    let steps = w.editor.undo_stack().undo_count();
    assert!(!w.editor.erase_selection(&mut w.model));
    assert_eq!(w.editor.message(), Some("Select something to erase"));
    assert_eq!(w.counts(), (4, 1));
    assert_eq!(w.editor.undo_stack().undo_count(), steps);
}

#[test]
fn selected_face_goes_and_its_edges_stay() {
    let mut w = World::new();
    w.select_at(v(1.5, 0.5, 0.0));
    assert!(matches!(w.editor.selection()[0], Entity::Face(_)));
    assert!(w.editor.erase_selection(&mut w.model));
    assert_eq!(w.counts(), (4, 0));
    assert!(w.editor.selection().is_empty());
    assert_eq!(w.editor.message(), None);
    assert_eq!(w.undo_label().as_deref(), Some("Erase"));
}

#[test]
fn selected_edge_takes_the_face_with_it() {
    let mut w = World::new();
    w.select_at(v(1.5, 0.0, 0.0));
    assert!(matches!(w.editor.selection()[0], Entity::Edge(_)));
    assert!(w.editor.erase_selection(&mut w.model));
    assert_eq!(w.counts(), (3, 0));
    assert!(w.editor.selection().is_empty());
}

#[test]
fn erase_is_one_undo_step_and_undo_restores() {
    let mut w = World::new();
    w.select_at(v(1.5, 0.0, 0.0));
    let steps = w.editor.undo_stack().undo_count();
    assert!(w.editor.erase_selection(&mut w.model));
    assert_eq!(w.editor.undo_stack().undo_count(), steps + 1);
    assert!(w.editor.key(EditKey::Undo, &mut w.model));
    assert_eq!(w.counts(), (4, 1));
    assert!(w.editor.key(EditKey::Redo, &mut w.model));
    assert_eq!(w.counts(), (3, 0));
}

#[test]
fn erase_drops_an_operation_in_progress() {
    let mut w = World::new();
    w.select_at(v(1.5, 0.0, 0.0));
    w.editor.set_tool(Tool::Line);
    w.click(v(-3.0, -3.0, 0.0));
    let c = w.px(v(-3.0, -1.0, 0.0));
    w.editor.pointer_move(&mut w.model, &w.view, c, NONE);
    assert!(!w.editor.preview().is_empty());
    assert!(w.editor.erase_selection(&mut w.model));
    assert!(w.editor.preview().is_empty());
}

#[test]
fn tool_click_on_an_edge_erases_it_and_its_face() {
    let mut w = World::new();
    w.editor.set_tool(Tool::Erase);
    assert!(w.click(v(1.5, 0.0, 0.0)));
    assert_eq!(w.counts(), (3, 0));
    assert_eq!(w.undo_label().as_deref(), Some("Erase"));
}

#[test]
fn tool_click_on_a_face_erases_only_the_face() {
    let mut w = World::new();
    w.editor.set_tool(Tool::Erase);
    assert!(w.click(v(1.5, 0.5, 0.0)));
    assert_eq!(w.counts(), (4, 0));
}

#[test]
fn tool_click_on_nothing_changes_nothing() {
    let mut w = World::new();
    w.editor.set_tool(Tool::Erase);
    let steps = w.editor.undo_stack().undo_count();
    assert!(!w.click(v(-4.0, 3.0, 0.0)));
    assert_eq!(w.counts(), (4, 1));
    assert_eq!(w.editor.undo_stack().undo_count(), steps);
    assert_eq!(w.editor.message(), None);
}

#[test]
fn tool_click_erases_only_what_was_clicked_and_keeps_the_selection() {
    let mut w = World::new();
    w.select_at(v(1.5, 0.5, 0.0));
    w.editor.set_tool(Tool::Erase);
    // The bottom edge, not the selected face.
    assert!(w.click(v(1.5, 0.0, 0.0)));
    assert_eq!(w.counts(), (3, 0));
    // The face went with the edge, so it left the selection too.
    assert!(w.editor.selection().is_empty());
}

#[test]
fn pointer_move_with_the_erase_tool_changes_nothing() {
    let mut w = World::new();
    w.editor.set_tool(Tool::Erase);
    let c = w.px(v(1.5, 0.0, 0.0));
    w.editor.pointer_move(&mut w.model, &w.view, c, NONE);
    assert_eq!(w.counts(), (4, 1));
    assert!(!w.editor.text("5"));
}

#[test]
fn backspace_erases_with_select_or_erase_tool_when_no_text() {
    let mut w = World::new();
    w.select_at(v(1.5, 0.5, 0.0));
    assert!(w.editor.key(EditKey::Backspace, &mut w.model));
    assert_eq!(w.counts(), (4, 0));

    let mut w = World::new();
    w.select_at(v(1.5, 0.5, 0.0));
    w.editor.set_tool(Tool::Erase);
    assert!(w.editor.key(EditKey::Backspace, &mut w.model));
    assert_eq!(w.counts(), (4, 0));
}

#[test]
fn backspace_with_another_tool_does_nothing() {
    let mut w = World::new();
    w.select_at(v(1.5, 0.5, 0.0));
    w.editor.set_tool(Tool::Move);
    assert!(!w.editor.key(EditKey::Backspace, &mut w.model));
    assert_eq!(w.counts(), (4, 1));
}

#[test]
fn backspace_edits_typed_text_before_it_erases() {
    let mut w = World::new();
    w.select_at(v(1.5, 0.5, 0.0));
    w.editor.set_tool(Tool::Move);
    w.click(v(1.5, 0.5, 0.0));
    assert!(w.editor.text("12"));
    assert!(!w.editor.key(EditKey::Backspace, &mut w.model));
    assert_eq!(w.editor.typed(), "1");
    assert_eq!(w.counts(), (4, 1));
}

#[test]
fn keys_delete_and_e() {
    assert_eq!(input::key("Delete", NONE), Some(Action::Erase));
    assert_eq!(input::key("E", NONE), Some(Action::SelectTool(Tool::Erase)));
    let shift = Modifiers {
        shift: true,
        ..NONE
    };
    assert_eq!(input::key("Delete", shift), None);
}

/// Typed numbers and measurements in `unit`, 2 places.
fn set_unit(model: &mut Model, unit: Unit) {
    model
        .set_length_format(
            modeler_kernel::units::LengthFormat::Decimal { unit, decimals: 2 },
            &mut modeler_kernel::model::ModelEdit::new(),
        )
        .unwrap();
}
