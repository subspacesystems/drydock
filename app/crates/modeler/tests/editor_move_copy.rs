// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Move with copy in the editor (`docs/specs/m4-app-move-copy.md`).

use glam::{DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::select::Entity;
use modeler_kernel::tools::translate::MoveStage;
use modeler_kernel::tools::{Target, draw_segments};
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

fn view() -> Viewport {
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

/// The unit square at x 1..2, y 0..1 on the ground.
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
    /// The square drawn into the model, selected, the Move tool active.
    fn new() -> World {
        let mut w = World {
            model: Model::new(),
            editor: Editor::new(),
            view: view(),
        };
        set_unit(&mut w.model, Unit::Meter);
        let n = SQUARE.len();
        let segs: Vec<_> = (0..n).map(|i| (SQUARE[i], SQUARE[(i + 1) % n])).collect();
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut w.model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segs, "Draw").expect("draw");
        w.editor.set_tool(Tool::Select);
        w.click(v(1.5, 0.5, 0.0));
        assert_eq!(w.editor.selection().len(), 1);
        w.editor.set_tool(Tool::Move);
        w
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn move_to(&mut self, p: DVec3) {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, NONE);
    }

    fn click(&mut self, p: DVec3) -> bool {
        self.move_to(p);
        let c = self.px(p);
        self.editor.click(&mut self.model, &self.view, c, NONE, 1)
    }

    fn positions(&self) -> Vec<DVec3> {
        self.model
            .root()
            .geometry()
            .vertices()
            .map(|(_, p)| p.position)
            .collect()
    }

    fn faces(&self) -> usize {
        self.model.root().geometry().face_count()
    }

    fn undo_count(&self) -> usize {
        self.editor.undo_stack().undo_count()
    }

    fn stage(&self) -> MoveStage {
        self.editor.move_tool().expect("move tool").stage()
    }

    fn has(&self, p: DVec3) -> bool {
        self.positions().iter().any(|q| q.distance(p) < 1e-6)
    }

    /// First click on the square, cursor 3 m along +x.
    fn start(&mut self) {
        assert!(!self.click(v(1.5, 0.5, 0.0)));
        assert_eq!(self.stage(), MoveStage::To);
        self.move_to(v(4.5, 0.5, 0.0));
    }
}

#[test]
fn copy_mode_starts_off_and_toggles() {
    let mut w = World::new();
    assert!(!w.editor.move_copy());
    w.editor.set_move_copy(true);
    assert!(w.editor.move_copy());
    w.editor.set_move_copy(false);
    assert!(!w.editor.move_copy());
}

#[test]
fn set_tool_turns_copy_off() {
    let mut w = World::new();
    w.editor.set_move_copy(true);
    w.editor.set_tool(Tool::Move);
    assert!(!w.editor.move_copy());
    w.editor.set_move_copy(true);
    w.editor.set_tool(Tool::Select);
    assert!(!w.editor.move_copy());
}

#[test]
fn click_copies_and_keeps_the_original() {
    let mut w = World::new();
    w.editor.set_move_copy(true);
    w.start();
    assert!(w.click(v(4.5, 0.5, 0.0)));
    assert_eq!(w.faces(), 2);
    for p in SQUARE {
        assert!(w.has(p), "original corner {p:?} moved");
        assert!(w.has(p + v(3.0, 0.0, 0.0)), "copy corner of {p:?} missing");
    }
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Copy"));
}

#[test]
fn copy_is_one_undo_step_labelled_copy() {
    let mut w = World::new();
    let base = w.undo_count();
    w.editor.set_move_copy(true);
    w.start();
    assert!(w.click(v(4.5, 0.5, 0.0)));
    assert_eq!(w.undo_count(), base + 1);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Copy"));
    assert!(w.editor.key(EditKey::Undo, &mut w.model));
    assert_eq!(w.faces(), 1);
    assert_eq!(w.positions().len(), 4);
    for p in SQUARE {
        assert!(w.has(p));
    }
}

#[test]
fn selection_becomes_the_copy_and_tool_restarts() {
    let mut w = World::new();
    let original = w.editor.selection().to_vec();
    w.editor.set_move_copy(true);
    w.start();
    assert!(w.click(v(4.5, 0.5, 0.0)));
    assert_eq!(w.stage(), MoveStage::From);
    assert!(w.editor.move_copy());
    assert_eq!(w.editor.message(), None);
    let now = w.editor.selection().to_vec();
    assert_eq!(now.len(), 1);
    assert_ne!(now, original);
    assert!(matches!(now[0], Entity::Face(_)));
    // Copying again copies the copy: it ends up another 3 m on.
    assert!(!w.click(v(4.5, 0.5, 0.0)));
    w.move_to(v(7.5, 0.5, 0.0));
    assert!(w.click(v(7.5, 0.5, 0.0)));
    assert_eq!(w.faces(), 3);
    for p in SQUARE {
        assert!(w.has(p + v(6.0, 0.0, 0.0)));
    }
}

#[test]
fn typed_distance_copies() {
    let mut w = World::new();
    w.editor.set_move_copy(true);
    w.start();
    assert!(w.editor.text("2"));
    assert!(w.editor.key(EditKey::Enter, &mut w.model));
    assert_eq!(w.faces(), 2);
    for p in SQUARE {
        assert!(w.has(p));
        assert!(w.has(p + v(2.0, 0.0, 0.0)));
    }
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Copy"));
}

#[test]
fn zero_offset_does_nothing() {
    let mut w = World::new();
    let base = w.undo_count();
    w.editor.set_move_copy(true);
    w.start();
    w.move_to(v(1.5, 0.5, 0.0));
    assert!(!w.click(v(1.5, 0.5, 0.0)));
    assert_eq!(w.faces(), 1);
    assert_eq!(w.undo_count(), base);
}

#[test]
fn copy_mode_off_still_moves() {
    let mut w = World::new();
    w.start();
    assert!(w.click(v(4.5, 0.5, 0.0)));
    assert_eq!(w.faces(), 1);
    assert!(!w.has(SQUARE[0]));
    assert!(w.has(SQUARE[0] + v(3.0, 0.0, 0.0)));
}

#[test]
fn mode_can_change_between_the_clicks() {
    let mut w = World::new();
    w.editor.set_move_copy(true);
    w.start();
    w.editor.set_move_copy(false);
    assert!(w.click(v(4.5, 0.5, 0.0)));
    assert_eq!(w.faces(), 1);
    assert!(!w.has(SQUARE[0]));

    let mut w = World::new();
    w.start();
    w.editor.set_move_copy(true);
    assert!(w.click(v(4.5, 0.5, 0.0)));
    assert_eq!(w.faces(), 2);
}

#[test]
fn nothing_to_retype_after_a_copy() {
    let mut w = World::new();
    w.editor.set_move_copy(true);
    w.start();
    assert!(w.click(v(4.5, 0.5, 0.0)));
    assert!(w.editor.text("5"));
    assert!(!w.editor.key(EditKey::Enter, &mut w.model));
    assert_eq!(w.editor.message(), Some("Click a start point first"));
    assert_eq!(w.faces(), 2);
}

#[test]
fn cancel_before_the_second_click_copies_nothing() {
    let mut w = World::new();
    w.editor.set_move_copy(true);
    w.start();
    w.editor.key(EditKey::Cancel, &mut w.model);
    assert_eq!(w.stage(), MoveStage::From);
    assert_eq!(w.faces(), 1);
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
