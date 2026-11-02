// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Move in the editor (`docs/specs/m3-app-move.md`).

use drydock::editor::*;
use drydock::input::{EditKey, Modifiers, Tool};
use drydock_kernel::model::{BodyId, Model, UndoStack};
use drydock_kernel::select::Entity;
use drydock_kernel::tools::translate::MoveStage;
use drydock_kernel::tools::{Target, draw_segments};
use drydock_kernel::topology::{Context, FaceId};
use drydock_kernel::units::{self, Unit};
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

fn view(standard: StandardView) -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(standard);
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

fn shifted(pts: &[DVec3], by: DVec3) -> Vec<DVec3> {
    pts.iter().map(|&p| p + by).collect()
}

fn f(value: f64) -> String {
    units::format_length(value, Unit::Meter, 2)
}

fn has_segment(segs: &[(DVec3, DVec3)], a: DVec3, b: DVec3) -> bool {
    segs.iter().any(|&(p, q)| {
        (p.distance(a) < 1e-6 && q.distance(b) < 1e-6)
            || (p.distance(b) < 1e-6 && q.distance(a) < 1e-6)
    })
}

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
}

impl World {
    /// Top view, the square drawn straight into the model, the Move tool.
    fn new() -> World {
        let mut w = World {
            model: Model::new(),
            editor: Editor::new(),
            view: view(StandardView::Top),
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
        w.editor.set_tool(Tool::Move);
        w
    }

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    fn square_face(&self) -> FaceId {
        self.ctx().faces().next().map(|(id, _)| id).expect("a face")
    }

    /// Select the square's face with the Select tool, then back to Move.
    fn select_square(&mut self) {
        self.editor.set_tool(Tool::Select);
        self.click(v(1.5, 0.5, 0.0));
        assert_eq!(
            self.editor.selection(),
            &[Entity::Face(self.square_face())][..]
        );
        self.editor.set_tool(Tool::Move);
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

    fn stage(&self) -> MoveStage {
        self.editor.move_tool().expect("move tool").stage()
    }

    fn undo_count(&self) -> usize {
        self.editor.undo_stack().undo_count()
    }

    fn assert_positions(&self, expected: &[DVec3]) {
        let got: Vec<DVec3> = self.ctx().vertices().map(|(_, p)| p.position).collect();
        assert_eq!(got.len(), expected.len(), "{got:?} vs {expected:?}");
        for e in expected {
            assert!(
                got.iter().any(|g| g.distance(*e) < 1e-6),
                "{e:?} missing from {got:?}"
            );
        }
    }

    /// From point on the square, cursor 3 m along +x: the tool is left at `To`.
    fn start_moving(&mut self) {
        assert!(!self.click(v(1.5, 0.5, 0.0)));
        assert_eq!(self.stage(), MoveStage::To);
        self.move_to(v(4.5, 0.5, 0.0));
    }
}

#[test]
fn move_tool_only_with_move() {
    let mut w = World::new();
    assert!(w.editor.move_tool().is_some());
    assert_eq!(w.stage(), MoveStage::From);
    assert_eq!(w.editor.draw_plane(), None);
    for tool in [Tool::Line, Tool::Select, Tool::Rotate, Tool::Orbit] {
        w.editor.set_tool(tool);
        assert!(w.editor.move_tool().is_none(), "{tool:?}");
    }
}

#[test]
fn measurement_is_an_empty_distance_at_first() {
    let w = World::new();
    assert_eq!(
        w.editor.measurement(&w.model),
        Some(("Distance", String::new()))
    );
    assert!(w.editor.preview().is_empty());
}

#[test]
fn two_clicks_move_the_selection() {
    let mut w = World::new();
    w.select_square();
    w.start_moving();
    assert_eq!(w.editor.measurement(&w.model), Some(("Distance", f(3.0))));
    assert!(w.click(v(4.5, 0.5, 0.0)));
    w.assert_positions(&shifted(&SQUARE, v(3.0, 0.0, 0.0)));
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.stage(), MoveStage::From);
    assert_eq!(w.editor.message(), None);
    assert_eq!(w.editor.selection(), &[Entity::Face(w.square_face())][..]);
    assert_eq!(w.editor.measurement(&w.model), Some(("Distance", f(3.0))));
    assert!(w.key(EditKey::Undo));
    w.assert_positions(&SQUARE);
}

#[test]
fn with_nothing_selected_the_first_click_picks_what_is_under_it() {
    let mut w = World::new();
    assert!(!w.click(v(1.5, 0.5, 0.0)));
    assert_eq!(w.editor.selection(), &[Entity::Face(w.square_face())][..]);
    assert_eq!(w.stage(), MoveStage::To);
    assert!(w.click(v(1.5, 3.5, 0.0)));
    w.assert_positions(&shifted(&SQUARE, v(0.0, 3.0, 0.0)));
}

#[test]
fn with_nothing_selected_or_under_the_cursor_nothing_happens() {
    let mut w = World::new();
    assert!(!w.click(v(-3.0, -3.0, 0.0)));
    assert_eq!(w.editor.message(), Some("Select something to move"));
    assert_eq!(w.stage(), MoveStage::From);
    assert!(w.editor.selection().is_empty());
    w.assert_positions(&SQUARE);
}

#[test]
fn hint_follows_the_cursor() {
    let mut w = World::new();
    w.move_to(v(-3.0, 3.0, 0.0));
    let hint = w.editor.hint().expect("a hint");
    assert!(hint.point.distance(v(-3.0, 3.0, 0.0)) < 1e-6, "{hint:?}");
}

#[test]
fn preview_shows_the_line_and_the_moved_selection() {
    let mut w = World::new();
    w.select_square();
    assert!(w.editor.preview().is_empty());
    w.start_moving();
    let segs = w.editor.preview();
    assert!(has_segment(&segs, v(1.5, 0.5, 0.0), v(4.5, 0.5, 0.0)));
    for i in 0..4 {
        let a = SQUARE[i] + v(3.0, 0.0, 0.0);
        let b = SQUARE[(i + 1) % 4] + v(3.0, 0.0, 0.0);
        assert!(has_segment(&segs, a, b), "{a:?} {b:?}");
    }
    assert_eq!(segs.len(), 5);
    // The model has not changed.
    w.assert_positions(&SQUARE);
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn text_is_taken() {
    let mut w = World::new();
    assert!(!w.editor.text("x"));
    assert!(w.editor.text("2"));
    assert_eq!(w.editor.typed(), "2");
}

#[test]
fn typed_distance_while_moving() {
    let mut w = World::new();
    w.select_square();
    w.start_moving();
    assert!(w.enter("2"));
    w.assert_positions(&shifted(&SQUARE, v(2.0, 0.0, 0.0)));
    assert_eq!(w.editor.typed(), "");
    assert_eq!(w.stage(), MoveStage::From);
    assert_eq!(w.undo_count(), 1);
}

#[test]
fn typed_distance_right_after_changes_the_move() {
    let mut w = World::new();
    w.select_square();
    w.start_moving();
    assert!(w.click(v(4.5, 0.5, 0.0)));
    assert!(w.enter("1"));
    w.assert_positions(&shifted(&SQUARE, v(1.0, 0.0, 0.0)));
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.editor.measurement(&w.model), Some(("Distance", f(1.0))));
    assert!(w.enter("-2"));
    w.assert_positions(&shifted(&SQUARE, v(-2.0, 0.0, 0.0)));
    assert_eq!(w.undo_count(), 1);
}

#[test]
fn typed_distance_without_a_move_or_junk_is_refused() {
    let mut w = World::new();
    w.select_square();
    assert!(!w.enter("1"));
    assert_eq!(w.editor.message(), Some("Click a start point first"));
    w.start_moving();
    assert!(!w.enter("1'2'3"));
    assert_eq!(w.editor.message(), Some("Not a length"));
    assert_eq!(w.stage(), MoveStage::To);
    w.assert_positions(&SQUARE);
}

#[test]
fn typed_distance_needs_a_direction() {
    let mut w = World::new();
    w.select_square();
    assert!(!w.click(v(1.5, 0.5, 0.0)));
    assert!(!w.enter("1"));
    assert_eq!(
        w.editor.message(),
        Some("Move the cursor to show the direction")
    );
    assert_eq!(w.stage(), MoveStage::To);
    w.assert_positions(&SQUARE);
}

#[test]
fn escape_drops_the_move() {
    let mut w = World::new();
    w.select_square();
    w.start_moving();
    assert!(w.editor.text("5"));
    assert!(!w.key(EditKey::Cancel));
    assert_eq!(w.stage(), MoveStage::From);
    assert_eq!(w.editor.typed(), "");
    assert_eq!(w.editor.message(), None);
    assert!(w.editor.preview().is_empty());
    w.assert_positions(&SQUARE);
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn undo_and_redo_drop_a_move_in_progress() {
    let mut w = World::new();
    w.select_square();
    w.start_moving();
    assert!(w.click(v(4.5, 0.5, 0.0)));
    assert!(!w.click(v(4.5, 0.5, 0.0)));
    assert_eq!(w.stage(), MoveStage::To);
    assert!(w.key(EditKey::Undo));
    assert_eq!(w.stage(), MoveStage::From);
    w.assert_positions(&SQUARE);
    assert!(w.key(EditKey::Redo));
    w.assert_positions(&shifted(&SQUARE, v(3.0, 0.0, 0.0)));
    assert_eq!(w.stage(), MoveStage::From);
}

#[test]
fn switching_tools_forgets_the_move() {
    let mut w = World::new();
    w.select_square();
    w.start_moving();
    w.editor.set_tool(Tool::Select);
    w.editor.set_tool(Tool::Move);
    assert_eq!(w.stage(), MoveStage::From);
    assert!(w.editor.preview().is_empty());
    // A finished move can no longer be retyped.
    w.start_moving();
    assert!(w.click(v(4.5, 0.5, 0.0)));
    w.editor.set_tool(Tool::Select);
    w.editor.set_tool(Tool::Move);
    assert!(!w.enter("1"));
    assert_eq!(
        w.editor.measurement(&w.model),
        Some(("Distance", String::new()))
    );
}

/// Typed numbers and measurements in `unit`, 2 places.
fn set_unit(model: &mut Model, unit: Unit) {
    model
        .set_length_format(
            drydock_kernel::units::LengthFormat::Decimal { unit, decimals: 2 },
            &mut drydock_kernel::model::ModelEdit::new(),
        )
        .unwrap();
}
