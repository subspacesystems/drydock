// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Offset's refusal message survives repeated pointer moves (`docs/specs/m9-offset-message.md`).

use drydock::editor::*;
use drydock::input::{EditKey, Modifiers, Tool};
use drydock_kernel::model::{BodyId, Model, UndoStack};
use drydock_kernel::tools::{Target, draw_segments};
use drydock_kernel::topology::{Context, FaceId};
use drydock_kernel::units::Unit;
use drydock_view::{Bounds, Camera, Projection, StandardView};
use glam::{DVec2, DVec3};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// Parallel iso camera framing -5..5 on every axis, 800x600 px.
fn iso_view() -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Iso);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(-5.0),
            max: DVec3::splat(5.0),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
}

impl World {
    fn new() -> World {
        let mut editor = Editor::new();
        let mut model = Model::new();
        model
            .set_length_format(
                drydock_kernel::units::LengthFormat::Decimal {
                    unit: Unit::Meter,
                    decimals: 2,
                },
                &mut drydock_kernel::model::ModelEdit::new(),
            )
            .unwrap();
        editor.set_tool(Tool::Offset);
        World {
            model,
            editor,
            view: iso_view(),
        }
    }

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    /// Draws the closed polygon `pts` straight into the model (not through the editor).
    fn draw(&mut self, pts: &[DVec3]) {
        let n = pts.len();
        let segs: Vec<_> = (0..n).map(|i| (pts[i], pts[(i + 1) % n])).collect();
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segs, "Shape").expect("shape");
    }

    /// The square (0,0,0)-(2,2,0) on the ground.
    fn square(&mut self) -> FaceId {
        self.draw(&[
            v(0.0, 0.0, 0.0),
            v(2.0, 0.0, 0.0),
            v(2.0, 2.0, 0.0),
            v(0.0, 2.0, 0.0),
        ]);
        assert_eq!(self.ctx().face_count(), 1);
        self.ctx().faces().next().expect("face").0
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

    fn key(&mut self, key: EditKey) -> bool {
        self.editor.key(key, &mut self.model)
    }

    fn enter(&mut self, s: &str) -> bool {
        assert!(self.editor.text(s), "text {s:?} refused");
        self.key(EditKey::Enter)
    }

    fn undo_count(&self) -> usize {
        self.editor.undo_stack().undo_count()
    }
}

const TOO_BIG: &str = "That offset is too big for the face";

/// The window calls `pointer_move` on every frame, with the cursor wherever it is.
fn frames(w: &mut World, p: DVec3) {
    for _ in 0..5 {
        w.move_to(p);
    }
}

#[test]
fn a_refused_click_keeps_its_message_over_repeated_pointer_moves() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, 1.6, 0.0));
    assert!(!w.click(v(0.7, 1.6, 0.0)));
    assert_eq!(w.editor.message(), Some(TOO_BIG));
    frames(&mut w, v(0.7, 1.6, 0.0));
    assert_eq!(w.editor.message(), Some(TOO_BIG));
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn a_refused_typed_length_keeps_its_message_over_repeated_pointer_moves() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, 0.5, 0.0));
    assert!(!w.enter("3"));
    assert_eq!(w.editor.message(), Some(TOO_BIG));
    frames(&mut w, v(0.7, 0.5, 0.0));
    assert_eq!(w.editor.message(), Some(TOO_BIG));
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn a_click_past_the_far_side_keeps_its_message() {
    // 5000 mm typed in a metre model is far past the face; so is a click 10 m out.
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, 0.5, 0.0));
    assert!(!w.enter("5000"));
    assert_eq!(w.editor.message(), Some(TOO_BIG));
    frames(&mut w, v(0.7, 0.5, 0.0));
    assert_eq!(w.editor.message(), Some(TOO_BIG));
}

#[test]
fn a_bad_typed_value_keeps_its_message_over_repeated_pointer_moves() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, -0.3, 0.0));
    assert!(!w.enter("-"));
    assert_eq!(w.editor.message(), Some("Not a length"));
    frames(&mut w, v(0.7, -0.3, 0.0));
    assert_eq!(w.editor.message(), Some("Not a length"));
    assert!(w.editor.offset_distance().is_some());
}

#[test]
fn moving_to_a_distance_that_fits_clears_the_message() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, 1.6, 0.0));
    assert!(!w.click(v(0.7, 1.6, 0.0)));
    frames(&mut w, v(0.7, 1.6, 0.0));
    assert_eq!(w.editor.message(), Some(TOO_BIG));
    w.move_to(v(0.7, 0.5, 0.0));
    assert_eq!(w.editor.message(), None);
    assert!(!w.editor.preview().is_empty());
    // And a later click commits.
    assert!(w.click(v(0.7, 0.5, 0.0)));
    assert_eq!(w.undo_count(), 1);
}

#[test]
fn moving_to_a_fitting_distance_clears_a_bad_value_message() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, -0.3, 0.0));
    assert!(!w.enter("-"));
    frames(&mut w, v(0.7, -0.3, 0.0));
    w.move_to(v(0.7, -0.5, 0.0));
    assert_eq!(w.editor.message(), None);
}
