// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Offset tool in the editor (`docs/specs/m9-offset-tool.md`).

use drydock::editor::*;
use drydock::input::{EditKey, Modifiers, Tool};
use drydock_kernel::model::{BodyId, Model, UndoStack};
use drydock_kernel::tools::{Target, draw_segments};
use drydock_kernel::topology::{Context, FaceId};
use drydock_kernel::units::{Unit, format_length};
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

fn f(x: f64) -> String {
    format_length(x, Unit::Meter, 2)
}

fn assert_near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
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

    /// A 4 m square (0,0,0)-(4,4,0) with a square hole (1,1)-(3,3) drawn in it. Returns the
    /// ring face (the one with a hole).
    fn ring(&mut self) -> FaceId {
        self.draw(&[
            v(0.0, 0.0, 0.0),
            v(4.0, 0.0, 0.0),
            v(4.0, 4.0, 0.0),
            v(0.0, 4.0, 0.0),
        ]);
        self.draw(&[
            v(1.0, 1.0, 0.0),
            v(3.0, 1.0, 0.0),
            v(3.0, 3.0, 0.0),
            v(1.0, 3.0, 0.0),
        ]);
        assert_eq!(self.ctx().face_count(), 2);
        let ctx = self.ctx();
        ctx.faces()
            .find(|(_, face)| !face.inner.is_empty())
            .expect("face with a hole")
            .0
    }

    fn bounds(&self) -> (DVec3, DVec3) {
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        for (_, vx) in self.ctx().vertices() {
            lo = lo.min(vx.position);
            hi = hi.max(vx.position);
        }
        (lo, hi)
    }

    fn has_vertex(&self, p: DVec3) -> bool {
        self.ctx()
            .vertices()
            .any(|(_, vx)| vx.position.distance(p) < 1e-6)
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

    fn assert_valid(&self) {
        assert!(
            self.ctx().validate().is_empty(),
            "{:?}",
            self.ctx().validate()
        );
    }
}

fn has_segment(segs: &[(DVec3, DVec3)], a: DVec3, b: DVec3) -> bool {
    segs.iter().any(|&(p, q)| {
        (p.distance(a) < 1e-6 && q.distance(b) < 1e-6)
            || (p.distance(b) < 1e-6 && q.distance(a) < 1e-6)
    })
}

// The square's bottom edge (y = 0) is the nearest edge to a click at (0.7, 0.4, 0). Points
// below it (y < 0) are outside the face; points above it are inside. x = 0.7 is not on any
// inference point or axis.

#[test]
fn hover_reports_the_face_under_the_cursor() {
    let mut w = World::new();
    let face = w.square();
    w.move_to(v(1.0, 0.7, 0.0));
    assert_eq!(w.editor.offset_face(), Some(face));
    assert_eq!(w.editor.offset_distance(), None);
    w.move_to(v(4.0, 4.0, 0.0));
    assert_eq!(w.editor.offset_face(), None);
}

#[test]
fn other_tools_report_no_offset() {
    let mut w = World::new();
    w.square();
    w.editor.set_tool(Tool::Select);
    w.move_to(v(1.0, 0.7, 0.0));
    assert_eq!(w.editor.offset_face(), None);
    assert_eq!(w.editor.offset_distance(), None);
}

#[test]
fn the_first_click_starts_with_a_zero_distance_and_changes_nothing() {
    let mut w = World::new();
    let face = w.square();
    assert!(!w.click(v(0.7, 0.4, 0.0)));
    assert_eq!(w.editor.offset_face(), Some(face));
    assert_eq!(w.editor.offset_distance(), Some(0.0));
    assert_eq!(w.undo_count(), 0);
    assert_eq!(w.ctx().face_count(), 1);
}

#[test]
fn the_cursor_outside_the_nearest_edge_gives_a_positive_distance() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, -0.5, 0.0));
    assert_near(w.editor.offset_distance().unwrap(), 0.5);
    w.move_to(v(1.3, -1.25, 0.0));
    assert_near(w.editor.offset_distance().unwrap(), 1.25);
}

#[test]
fn the_cursor_inside_gives_a_negative_distance() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, 0.5, 0.0));
    assert_near(w.editor.offset_distance().unwrap(), -0.5);
}

#[test]
fn the_preview_is_the_new_outline() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    assert!(w.editor.preview().is_empty(), "no distance yet");
    w.move_to(v(0.7, -0.5, 0.0));
    let p = w.editor.preview();
    assert_eq!(p.len(), 4, "{p:?}");
    let (a, b, c, d) = (
        v(-0.5, -0.5, 0.0),
        v(2.5, -0.5, 0.0),
        v(2.5, 2.5, 0.0),
        v(-0.5, 2.5, 0.0),
    );
    assert!(has_segment(&p, a, b) && has_segment(&p, b, c));
    assert!(has_segment(&p, c, d) && has_segment(&p, d, a));
    w.move_to(v(0.7, 0.5, 0.0));
    let p = w.editor.preview();
    assert_eq!(p.len(), 4, "{p:?}");
    assert!(has_segment(&p, v(0.5, 0.5, 0.0), v(1.5, 0.5, 0.0)));
}

#[test]
fn preview_is_empty_when_the_offset_is_refused() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, 1.6, 0.0));
    assert_near(w.editor.offset_distance().unwrap(), -1.6);
    assert!(w.editor.preview().is_empty());
}

#[test]
fn the_measurement_box_shows_the_distance() {
    let mut w = World::new();
    w.square();
    assert_eq!(
        w.editor.measurement(&w.model),
        Some(("Distance", String::new()))
    );
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, -0.5, 0.0));
    assert_eq!(w.editor.measurement(&w.model), Some(("Distance", f(0.5))));
    w.move_to(v(0.7, 0.5, 0.0));
    assert_eq!(
        w.editor.measurement(&w.model),
        Some(("Distance", f(0.5))),
        "the box shows the length without its sign"
    );
}

#[test]
fn the_second_click_grows_the_face_in_one_undo_step() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, -0.5, 0.0));
    assert!(w.click(v(0.7, -0.5, 0.0)), "an undo step was pushed");
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.ctx().face_count(), 2);
    let (lo, hi) = w.bounds();
    assert!(lo.distance(v(-0.5, -0.5, 0.0)) < 1e-6, "{lo:?}");
    assert!(hi.distance(v(2.5, 2.5, 0.0)) < 1e-6, "{hi:?}");
    w.assert_valid();
    assert_eq!(w.editor.offset_distance(), None, "finished");
    assert_eq!(
        w.editor.measurement(&w.model),
        Some(("Distance", f(0.5))),
        "the box keeps the last distance"
    );
    assert!(w.key(EditKey::Undo));
    assert_eq!(w.ctx().face_count(), 1);
    let (lo, hi) = w.bounds();
    assert!(lo.distance(v(0.0, 0.0, 0.0)) < 1e-6 && hi.distance(v(2.0, 2.0, 0.0)) < 1e-6);
    assert!(w.key(EditKey::Redo));
    assert_eq!(w.ctx().face_count(), 2);
}

#[test]
fn the_second_click_insets_the_face() {
    let mut w = World::new();
    let face = w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, 0.5, 0.0));
    assert!(w.click(v(0.7, 0.5, 0.0)));
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.ctx().face_count(), 2);
    assert_eq!(w.ctx().vertices().count(), 8);
    assert!(w.has_vertex(v(0.5, 0.5, 0.0)) && w.has_vertex(v(1.5, 1.5, 0.0)));
    let (lo, hi) = w.bounds();
    assert!(lo.distance(v(0.0, 0.0, 0.0)) < 1e-6 && hi.distance(v(2.0, 2.0, 0.0)) < 1e-6);
    assert!(
        w.ctx().face(face).is_some(),
        "the original face is still there"
    );
    w.assert_valid();
}

#[test]
fn a_hole_edge_counts_the_side_into_the_hole_as_outside() {
    // Start near the hole's bottom edge (y = 1) from the ring; the cursor in the hole is
    // positive: the face grows, so the outer loop moves out and the hole shrinks.
    let mut w = World::new();
    w.ring();
    w.click(v(1.7, 0.8, 0.0));
    w.move_to(v(1.7, 1.5, 0.0));
    assert_near(w.editor.offset_distance().unwrap(), 0.5);
    assert!(w.click(v(1.7, 1.5, 0.0)));
    assert_eq!(w.undo_count(), 1);
    let (lo, hi) = w.bounds();
    assert!(lo.distance(v(-0.5, -0.5, 0.0)) < 1e-6, "{lo:?}");
    assert!(hi.distance(v(4.5, 4.5, 0.0)) < 1e-6, "{hi:?}");
    assert!(w.has_vertex(v(1.5, 1.5, 0.0)) && w.has_vertex(v(2.5, 2.5, 0.0)));
    w.assert_valid();
}

#[test]
fn a_click_at_zero_distance_does_nothing() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, 0.0, 0.0));
    assert!(!w.click(v(0.7, 0.0, 0.0)));
    assert_eq!(w.undo_count(), 0);
    assert_eq!(w.ctx().face_count(), 1);
    assert!(w.editor.offset_distance().is_some(), "still offsetting");
}

#[test]
fn a_typed_length_sets_the_distance_outward() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, -0.3, 0.0));
    assert!(w.enter("0.25"));
    assert_eq!(w.undo_count(), 1);
    let (lo, hi) = w.bounds();
    assert!(lo.distance(v(-0.25, -0.25, 0.0)) < 1e-6, "{lo:?}");
    assert!(hi.distance(v(2.25, 2.25, 0.0)) < 1e-6, "{hi:?}");
    assert_eq!(w.editor.offset_distance(), None);
    assert_eq!(w.editor.message(), None);
    w.assert_valid();
}

#[test]
fn a_typed_length_follows_the_side_the_cursor_is_on() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, 0.5, 0.0));
    assert!(w.enter("0.25"));
    assert!(w.has_vertex(v(0.25, 0.25, 0.0)) && w.has_vertex(v(1.75, 1.75, 0.0)));
    let (lo, hi) = w.bounds();
    assert!(lo.distance(v(0.0, 0.0, 0.0)) < 1e-6 && hi.distance(v(2.0, 2.0, 0.0)) < 1e-6);
}

#[test]
fn a_negative_typed_length_flips_the_side() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, -0.3, 0.0));
    assert!(w.enter("-0.25"));
    assert!(w.has_vertex(v(0.25, 0.25, 0.0)) && w.has_vertex(v(1.75, 1.75, 0.0)));
}

#[test]
fn a_typed_length_with_the_cursor_still_goes_outward() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    assert!(w.enter("0.25"));
    let (lo, hi) = w.bounds();
    assert!(lo.distance(v(-0.25, -0.25, 0.0)) < 1e-6, "{lo:?}");
    assert!(hi.distance(v(2.25, 2.25, 0.0)) < 1e-6, "{hi:?}");
}

#[test]
fn a_typed_length_uses_the_model_unit() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, -0.3, 0.0));
    assert!(w.enter("25cm"));
    let (lo, _) = w.bounds();
    assert!(lo.distance(v(-0.25, -0.25, 0.0)) < 1e-6, "{lo:?}");
}

#[test]
fn a_bad_typed_value_says_so_and_keeps_going() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, -0.3, 0.0));
    assert!(!w.enter("-"));
    assert_eq!(w.editor.message(), Some("Not a length"));
    assert_eq!(w.undo_count(), 0);
    assert!(w.editor.offset_distance().is_some());
}

#[test]
fn typing_before_a_face_is_picked_asks_for_one() {
    let mut w = World::new();
    w.square();
    assert!(!w.enter("0.25"));
    assert_eq!(w.editor.message(), Some("Click a face first"));
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn an_offset_that_is_too_big_is_refused_with_a_message() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, 1.6, 0.0));
    assert!(!w.click(v(0.7, 1.6, 0.0)));
    assert_eq!(
        w.editor.message(),
        Some("That offset is too big for the face")
    );
    assert_eq!(w.undo_count(), 0);
    assert_eq!(w.ctx().face_count(), 1);
    assert!(w.editor.offset_distance().is_some(), "still offsetting");
    // Moving back to a distance that fits lets the next click through and clears the message.
    w.move_to(v(0.7, 0.5, 0.0));
    assert!(w.click(v(0.7, 0.5, 0.0)));
    assert_eq!(w.editor.message(), None);
    assert_eq!(w.undo_count(), 1);
}

#[test]
fn a_typed_length_that_is_too_big_is_refused_with_a_message() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, 0.5, 0.0));
    assert!(!w.enter("3"));
    assert_eq!(
        w.editor.message(),
        Some("That offset is too big for the face")
    );
    assert_eq!(w.undo_count(), 0);
    assert_eq!(w.ctx().face_count(), 1);
}

#[test]
fn an_outward_offset_that_would_hit_another_face_is_refused() {
    // A second square 0.5 m to the right of the first: growing the first by 1 m runs into it.
    let mut w = World::new();
    let first = w.square();
    w.draw(&[
        v(2.5, 0.0, 0.0),
        v(4.5, 0.0, 0.0),
        v(4.5, 2.0, 0.0),
        v(2.5, 2.0, 0.0),
    ]);
    assert_eq!(w.ctx().face_count(), 2);
    w.move_to(v(0.7, 0.4, 0.0));
    assert_eq!(w.editor.offset_face(), Some(first));
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, -1.0, 0.0));
    assert!(!w.click(v(0.7, -1.0, 0.0)));
    let message = w.editor.message().expect("a message");
    assert!(!message.is_empty());
    assert_eq!(w.undo_count(), 0);
    assert_eq!(w.ctx().face_count(), 2);
    w.assert_valid();
}

#[test]
fn escape_drops_the_offset() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, -0.5, 0.0));
    assert!(w.editor.text("1"));
    assert!(!w.key(EditKey::Cancel));
    assert_eq!(w.editor.offset_distance(), None);
    assert_eq!(w.editor.message(), None);
    assert!(w.editor.preview().is_empty());
    assert_eq!(w.undo_count(), 0);
    assert_eq!(w.ctx().face_count(), 1);
    // The typed text went with it: Enter has nothing to send.
    assert!(!w.key(EditKey::Enter));
    // A new click starts a new offset instead of finishing the old one.
    assert!(!w.click(v(0.7, 0.4, 0.0)));
    assert_eq!(w.editor.offset_distance(), Some(0.0));
}

#[test]
fn undo_and_a_tool_switch_drop_the_offset() {
    let mut w = World::new();
    w.square();
    w.click(v(0.7, 0.4, 0.0));
    w.move_to(v(0.7, -0.5, 0.0));
    w.key(EditKey::Undo);
    assert_eq!(w.editor.offset_distance(), None);

    w.click(v(0.7, 0.4, 0.0));
    w.editor.set_tool(Tool::Select);
    w.editor.set_tool(Tool::Offset);
    assert_eq!(w.editor.offset_distance(), None);
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn a_click_off_every_face_does_nothing() {
    let mut w = World::new();
    w.square();
    assert!(!w.click(v(4.0, 4.0, 0.0)));
    assert_eq!(w.editor.offset_distance(), None);
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn the_tool_has_no_drawing_plane() {
    let w = World::new();
    assert!(w.editor.draw_plane().is_none());
}
