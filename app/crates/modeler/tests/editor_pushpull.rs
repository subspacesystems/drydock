// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Push/Pull in the editor (`docs/specs/m3-app-pushpull.md`).

use glam::{DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::pushpull::push_pull;
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::topology::{Context, FaceId};
use modeler_kernel::units::{Unit, format_length};
use modeler_view::{Bounds, Camera, Projection, StandardView};

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
        set_unit(&mut model, Unit::Meter);
        editor.set_tool(Tool::PushPull);
        World {
            model,
            editor,
            view: iso_view(),
        }
    }

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    /// The 2 m square (0,0,0)-(2,2,0) on the ground, drawn straight into the model.
    fn square(&mut self) -> FaceId {
        let pts = [
            v(0.0, 0.0, 0.0),
            v(2.0, 0.0, 0.0),
            v(2.0, 2.0, 0.0),
            v(0.0, 2.0, 0.0),
        ];
        let segs: Vec<_> = (0..4).map(|i| (pts[i], pts[(i + 1) % 4])).collect();
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segs, "Square").expect("square");
        assert_eq!(self.ctx().face_count(), 1);
        self.face_at_z(0.0).expect("square face")
    }

    /// The box (0,0,0)-(2,2,1), made outside the editor (its undo stack stays empty).
    fn slab(&mut self) -> FaceId {
        let face = self.square();
        // Up is +1 or -1 along the square's front normal, depending on how it faces.
        let up = self
            .ctx()
            .face_plane(face)
            .expect("plane")
            .normal
            .z
            .signum();
        let mut undo = UndoStack::new();
        push_pull(&mut self.model, BodyId::Root, face, up, &mut undo, "Slab").expect("slab");
        assert_eq!(self.ctx().face_count(), 6);
        assert_near(self.top(), 1.0);
        self.face_at_z(1.0).expect("top face")
    }

    /// The face whose vertices all have height `z`.
    fn face_at_z(&self, z: f64) -> Option<FaceId> {
        let ctx = self.ctx();
        ctx.faces()
            .find(|(_, face)| {
                ctx.loop_points(&face.outer)
                    .iter()
                    .all(|p| (p.z - z).abs() < 1e-6)
            })
            .map(|(id, _)| id)
    }

    fn top(&self) -> f64 {
        self.ctx()
            .vertices()
            .map(|(_, vx)| vx.position.z)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    fn bottom(&self) -> f64 {
        self.ctx()
            .vertices()
            .map(|(_, vx)| vx.position.z)
            .fold(f64::INFINITY, f64::min)
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn move_to(&mut self, p: DVec3) {
        let c = self.px(p);
        self.move_px(c);
    }

    fn move_px(&mut self, c: DVec2) {
        self.editor
            .pointer_move(&mut self.model, &self.view, c, NONE);
    }

    fn click_n(&mut self, p: DVec3, count: u32) -> bool {
        self.move_to(p);
        let c = self.px(p);
        self.editor
            .click(&mut self.model, &self.view, c, NONE, count)
    }

    fn click(&mut self, p: DVec3) -> bool {
        self.click_n(p, 1)
    }

    fn key(&mut self, key: EditKey) -> bool {
        self.editor.key(key, &mut self.model)
    }

    /// Types `s` and presses Enter; returns what Enter returned.
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

#[test]
fn hover_reports_the_face_under_the_cursor() {
    let mut w = World::new();
    let top = w.slab();
    w.move_to(v(1.0, 0.7, 1.0));
    assert_eq!(w.editor.pushpull_face(), Some(top));
    assert_eq!(w.editor.pushpull_distance(), None);
    assert!(w.editor.preview().is_empty());
    w.move_px(DVec2::new(5.0, 5.0));
    assert_eq!(w.editor.pushpull_face(), None);
}

#[test]
fn other_tools_report_no_push() {
    let mut w = World::new();
    w.slab();
    w.editor.set_tool(Tool::Line);
    w.move_to(v(1.0, 0.7, 1.0));
    assert_eq!(w.editor.pushpull_face(), None);
    assert_eq!(w.editor.pushpull_distance(), None);
}

#[test]
fn click_on_nothing_does_nothing() {
    let mut w = World::new();
    w.slab();
    let c = DVec2::new(5.0, 5.0);
    w.move_px(c);
    assert!(!w.editor.click(&mut w.model, &w.view, c, NONE, 1));
    assert_eq!(w.editor.pushpull_distance(), None);
    assert_eq!(w.editor.pushpull_face(), None);
}

#[test]
fn click_starts_a_push_and_the_cursor_sets_the_distance() {
    let mut w = World::new();
    let top = w.slab();
    assert!(!w.click(v(1.0, 0.7, 1.0)));
    assert_eq!(w.editor.pushpull_face(), Some(top));
    assert_near(w.editor.pushpull_distance().expect("pushing"), 0.0);
    w.move_to(v(1.0, 0.7, 1.6));
    assert_near(w.editor.pushpull_distance().expect("pushing"), 0.6);
    w.move_to(v(1.0, 0.7, 0.75));
    assert_near(w.editor.pushpull_distance().expect("pushing"), -0.25);
    // Still the same face while the cursor is away from it.
    w.move_px(DVec2::new(5.0, 5.0));
    assert_eq!(w.editor.pushpull_face(), Some(top));
    // Nothing changed in the model yet.
    assert_near(w.top(), 1.0);
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn distance_is_the_nearest_point_on_the_normal_line() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    // A cursor off to the side of the normal line still measures along it.
    let on_line = w.px(v(1.0, 0.7, 1.6));
    // The blue axis stays vertical on screen, so a sideways offset keeps the height.
    let side = on_line + DVec2::new(-30.0, 0.0);
    w.move_px(side);
    assert_near(w.editor.pushpull_distance().expect("pushing"), 0.6);
}

#[test]
fn preview_shows_the_face_at_its_new_place() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 1.6));
    let p = w.editor.preview();
    let corners = [
        v(0.0, 0.0, 1.0),
        v(2.0, 0.0, 1.0),
        v(2.0, 2.0, 1.0),
        v(0.0, 2.0, 1.0),
    ];
    let up = DVec3::Z * 0.6;
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        assert!(has_segment(&p, a + up, b + up), "cap edge {i} in {p:?}");
        assert!(has_segment(&p, a, a + up), "side edge {i} in {p:?}");
    }
    assert_eq!(p.len(), 8, "{p:?}");
}

#[test]
fn second_click_pulls_a_lone_face_into_a_box() {
    let mut w = World::new();
    w.square();
    assert!(!w.click(v(1.0, 0.7, 0.0)));
    w.move_to(v(1.0, 0.7, 0.8));
    assert!(w.click(v(1.0, 0.7, 0.8)));
    assert_eq!(w.ctx().face_count(), 6);
    assert_near(w.top(), 0.8);
    assert_near(w.bottom(), 0.0);
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.editor.pushpull_distance(), None);
    assert_eq!(w.editor.message(), None);
    w.assert_valid();
}

#[test]
fn second_click_slides_a_box_top() {
    let mut w = World::new();
    let top = w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 1.6));
    assert!(w.click(v(1.0, 0.7, 1.6)));
    assert_near(w.top(), 1.6);
    assert_eq!(w.ctx().face_count(), 6);
    assert_eq!(w.face_at_z(1.6), Some(top));
    assert_eq!(w.undo_count(), 1);
    w.assert_valid();
}

#[test]
fn undo_puts_it_back() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 1.6));
    w.click(v(1.0, 0.7, 1.6));
    assert!(w.key(EditKey::Undo));
    assert_near(w.top(), 1.0);
    assert!(w.key(EditKey::Redo));
    assert_near(w.top(), 1.6);
}

#[test]
fn zero_distance_click_does_nothing() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    assert!(!w.click(v(1.0, 0.7, 1.0)));
    assert_eq!(w.undo_count(), 0);
    // Still pushing.
    assert!(w.editor.pushpull_distance().is_some());
}

#[test]
fn typed_distance_while_pushing_goes_the_cursor_way() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 1.3));
    assert!(w.enter("0.5"));
    assert_near(w.top(), 1.5);
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.editor.pushpull_distance(), None);
    assert_eq!(w.editor.typed(), "");
}

#[test]
fn typed_distance_while_pushing_in() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 0.7));
    assert!(w.enter("0.25"));
    assert_near(w.top(), 0.75);
    w.assert_valid();
}

#[test]
fn typed_negative_distance_goes_the_other_way() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 1.3));
    assert!(w.enter("-0.25"));
    assert_near(w.top(), 0.75);
}

#[test]
fn typed_distance_with_the_cursor_on_the_face_goes_along_the_normal() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    assert!(w.enter("0.5"));
    assert_near(w.top(), 1.5);
}

#[test]
fn typed_units_type_through() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 1.3));
    assert!(w.enter("250mm"));
    assert_near(w.top(), 1.25);
}

#[test]
fn typed_after_a_push_changes_it() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 1.6));
    w.click(v(1.0, 0.7, 1.6));
    assert_eq!(w.undo_count(), 1);
    assert!(w.enter("0.25"));
    assert_near(w.top(), 1.25);
    // The new distance replaces the push: still one undo step.
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.editor.undo_stack().redo_count(), 0);
    assert!(w.key(EditKey::Undo));
    assert_near(w.top(), 1.0);
}

#[test]
fn typed_after_a_push_in_keeps_its_direction() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 0.6));
    w.click(v(1.0, 0.7, 0.6));
    assert_near(w.top(), 0.6);
    assert!(w.enter("0.5"));
    assert_near(w.top(), 0.5);
    assert_eq!(w.undo_count(), 1);
}

#[test]
fn typed_after_a_lone_face_pull_changes_it() {
    let mut w = World::new();
    w.square();
    w.click(v(1.0, 0.7, 0.0));
    w.move_to(v(1.0, 0.7, 0.8));
    assert!(w.click(v(1.0, 0.7, 0.8)));
    assert!(w.enter("2"));
    assert_eq!(w.ctx().face_count(), 6);
    assert_near(w.top(), 2.0);
    assert_near(w.bottom(), 0.0);
    assert_eq!(w.undo_count(), 1);
    w.assert_valid();
}

#[test]
fn typed_after_undo_is_not_taken_as_a_change() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 1.6));
    w.click(v(1.0, 0.7, 1.6));
    w.key(EditKey::Undo);
    assert!(!w.enter("0.25"));
    assert_near(w.top(), 1.0);
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn refused_typed_change_keeps_the_push() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 0.6));
    w.click(v(1.0, 0.7, 0.6));
    // Pushing the top 1.5 m down would turn the box inside out.
    assert!(!w.enter("1.5"));
    assert!(w.editor.message().is_some());
    assert_near(w.top(), 0.6);
    assert_eq!(w.undo_count(), 1);
    w.assert_valid();
}

#[test]
fn refused_push_shows_a_message_and_keeps_pushing() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 0.7));
    assert!(!w.enter("1.5"));
    assert!(w.editor.message().is_some());
    assert_near(w.top(), 1.0);
    assert_eq!(w.undo_count(), 0);
    assert!(w.editor.pushpull_distance().is_some());
    // A good distance afterwards still works and clears the message.
    assert!(w.enter("0.5"));
    assert_near(w.top(), 0.5);
    assert_eq!(w.editor.message(), None);
}

#[test]
fn double_click_repeats_the_last_distance() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 1.3));
    w.enter("0.5");
    assert_near(w.top(), 1.5);
    // A double click arrives as a click then a click with count 2.
    w.click_n(v(1.0, 0.7, 1.5), 1);
    assert!(w.click_n(v(1.0, 0.7, 1.5), 2));
    assert_near(w.top(), 2.0);
    assert_eq!(w.undo_count(), 2);
    assert_eq!(w.editor.pushpull_distance(), None);
}

#[test]
fn double_click_with_no_last_distance_does_nothing() {
    let mut w = World::new();
    w.slab();
    w.click_n(v(1.0, 0.7, 1.0), 1);
    assert!(!w.click_n(v(1.0, 0.7, 1.0), 2));
    assert_near(w.top(), 1.0);
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn escape_drops_the_push() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 1.6));
    w.editor.text("3");
    assert!(!w.key(EditKey::Cancel));
    assert_eq!(w.editor.pushpull_distance(), None);
    assert_eq!(w.editor.typed(), "");
    assert!(w.editor.preview().is_empty());
    assert_near(w.top(), 1.0);
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn switching_tools_drops_the_push() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 1.6));
    w.editor.set_tool(Tool::Select);
    w.editor.set_tool(Tool::PushPull);
    assert_eq!(w.editor.pushpull_distance(), None);
    assert!(w.editor.preview().is_empty());
    assert_near(w.top(), 1.0);
}

#[test]
fn text_is_taken() {
    let mut w = World::new();
    assert!(!w.editor.text("x"));
    assert!(w.editor.text("5"));
    assert_eq!(w.editor.typed(), "5");
}

#[test]
fn measurement_shows_the_distance() {
    let mut w = World::new();
    w.slab();
    assert_eq!(
        w.editor.measurement(&w.model),
        Some(("Distance", String::new()))
    );
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 0.75));
    assert_eq!(w.editor.measurement(&w.model), Some(("Distance", f(0.25))));
    w.click(v(1.0, 0.7, 0.75));
    // After the push it shows the distance just used.
    assert_eq!(w.editor.measurement(&w.model), Some(("Distance", f(0.25))));
}

#[test]
fn endpoint_snap_sets_the_distance_by_its_offset() {
    let mut w = World::new();
    w.slab();
    // A post standing on the ground away from the box, 3 m tall.
    {
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut w.model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &[(v(4.0, -1.0, 0.0), v(4.0, -1.0, 3.0))], "Post").expect("post");
    }
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(4.0, -1.0, 3.0));
    assert_near(w.editor.pushpull_distance().expect("pushing"), 2.0);
    assert!(w.click(v(4.0, -1.0, 3.0)));
    assert_near(w.top(), 3.0);
}

#[test]
fn hint_is_shown_while_pushing() {
    let mut w = World::new();
    w.slab();
    w.click(v(1.0, 0.7, 1.0));
    w.move_to(v(1.0, 0.7, 1.6));
    let hint = w.editor.hint().expect("hint");
    assert!(hint.point.distance(v(1.0, 0.7, 1.6)) < 1e-6, "{hint:?}");
}

#[test]
fn pushed_face_selection_is_kept_when_it_survives() {
    let mut w = World::new();
    w.slab();
    w.editor.set_tool(Tool::Select);
    let c = w.px(v(1.0, 0.7, 1.0));
    w.editor.click(&mut w.model, &w.view, c, NONE, 1);
    let selected = w.editor.selection().to_vec();
    assert_eq!(selected.len(), 1);
    w.editor.set_tool(Tool::PushPull);
    w.click(v(1.0, 0.7, 1.0));
    w.enter("0.5");
    assert_eq!(w.editor.selection(), selected.as_slice());
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
