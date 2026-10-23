// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Rotate in the editor (`docs/specs/m3-app-rotate.md`).

use glam::{DQuat, DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::pushpull::push_pull;
use modeler_kernel::select::Entity;
use modeler_kernel::tools::rotate::RotateStage;
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::topology::{Context, FaceId};
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

fn view(standard: StandardView) -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(standard);
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

/// The unit square at x 1..2, y 0..1 on the ground.
const SQUARE: [DVec3; 4] = [
    DVec3::new(1.0, 0.0, 0.0),
    DVec3::new(2.0, 0.0, 0.0),
    DVec3::new(2.0, 1.0, 0.0),
    DVec3::new(1.0, 1.0, 0.0),
];

fn rotated(pts: &[DVec3], center: DVec3, axis: DVec3, deg: f64) -> Vec<DVec3> {
    let q = DQuat::from_axis_angle(axis.normalize(), deg.to_radians());
    pts.iter().map(|&p| center + q * (p - center)).collect()
}

/// A point at `deg` degrees on a circle of radius `r` about `c` in the ground plane.
fn at(c: DVec3, r: f64, deg: f64) -> DVec3 {
    let a = deg.to_radians();
    c + v(a.cos(), a.sin(), 0.0) * r
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
    /// Top view, the square drawn straight into the model, the Rotate tool.
    fn new() -> World {
        let mut w = World {
            model: Model::new(),
            editor: Editor::new(),
            view: view(StandardView::Top),
        };
        w.editor.unit = Unit::Meter;
        w.draw(&SQUARE);
        w.editor.set_tool(Tool::Rotate);
        w
    }

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
        draw_segments(&mut t, &segs, "Draw").expect("draw");
    }

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    fn square_face(&self) -> FaceId {
        self.ctx().faces().next().map(|(id, _)| id).expect("a face")
    }

    /// Select the square's face with the Select tool, then back to Rotate.
    fn select_square(&mut self) {
        self.editor.set_tool(Tool::Select);
        self.click(v(1.5, 0.5, 0.0));
        assert_eq!(
            self.editor.selection(),
            &[Entity::Face(self.square_face())][..]
        );
        self.editor.set_tool(Tool::Rotate);
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

    fn stage(&self) -> RotateStage {
        self.editor.rotate_tool().expect("rotate tool").stage()
    }

    fn undo_count(&self) -> usize {
        self.editor.undo_stack().undo_count()
    }

    fn positions(&self) -> Vec<DVec3> {
        self.ctx().vertices().map(|(_, p)| p.position).collect()
    }

    fn assert_positions(&self, expected: &[DVec3]) {
        let got = self.positions();
        assert_eq!(got.len(), expected.len(), "{got:?} vs {expected:?}");
        for e in expected {
            assert!(
                got.iter().any(|g| g.distance(*e) < 1e-6),
                "{e:?} missing from {got:?}"
            );
        }
    }

    /// Center at the origin, start along +x at radius 3: the tool is left at `Angle`.
    fn start_at_origin(&mut self) {
        assert!(!self.click(DVec3::ZERO));
        assert_eq!(self.stage(), RotateStage::Start);
        assert!(!self.click(v(3.0, 0.0, 0.0)));
        assert_eq!(self.stage(), RotateStage::Angle);
    }

    fn angle(&self) -> f64 {
        self.editor
            .rotate_tool()
            .and_then(|t| t.angle())
            .expect("rotating")
    }
}

// -- the tool object -------------------------------------------------------------------

#[test]
fn rotate_tool_and_plane_only_with_rotate() {
    let mut w = World::new();
    assert!(w.editor.rotate_tool().is_some());
    assert_eq!(w.stage(), RotateStage::Center);
    let plane = w.editor.draw_plane().expect("a plane");
    assert!(plane.normal().distance(DVec3::Z) < 1e-9);
    for tool in [Tool::Line, Tool::Select, Tool::PushPull, Tool::Orbit] {
        w.editor.set_tool(tool);
        assert!(w.editor.rotate_tool().is_none(), "{tool:?}");
    }
    w.editor.set_tool(Tool::Line);
    assert_eq!(w.editor.draw_plane(), None);
}

#[test]
fn measurement_is_an_empty_angle_at_first() {
    let w = World::new();
    assert_eq!(w.editor.measurement(), Some(("Angle", String::new())));
    assert!(w.editor.preview().is_empty());
}

// -- clicks ----------------------------------------------------------------------------

#[test]
fn three_clicks_rotate_the_selection() {
    let mut w = World::new();
    w.select_square();
    w.start_at_origin();
    w.move_to(v(0.0, 2.0, 0.0));
    assert!((w.angle() - 90.0).abs() < 1e-9);
    assert_eq!(w.editor.measurement(), Some(("Angle", "90.0°".to_string())));
    assert!(w.click(v(0.0, 2.0, 0.0)));
    w.assert_positions(&rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 90.0));
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.stage(), RotateStage::Center);
    assert_eq!(w.editor.message(), None);
    // The face survives and stays selected.
    assert_eq!(w.editor.selection(), &[Entity::Face(w.square_face())][..]);
    // The finished angle stays in the box while it can be retyped.
    assert_eq!(w.editor.measurement(), Some(("Angle", "90.0°".to_string())));
    assert!(w.key(EditKey::Undo));
    w.assert_positions(&SQUARE);
}

#[test]
fn with_nothing_selected_the_center_click_picks_what_is_under_it() {
    let mut w = World::new();
    assert!(w.editor.selection().is_empty());
    let c = v(1.5, 0.5, 0.0);
    assert!(!w.click(c));
    assert_eq!(w.editor.selection(), &[Entity::Face(w.square_face())][..]);
    assert_eq!(w.stage(), RotateStage::Start);
    assert!(!w.click(v(3.5, 0.5, 0.0)));
    assert!(w.click(v(1.5, 3.0, 0.0)));
    w.assert_positions(&rotated(&SQUARE, c, DVec3::Z, 90.0));
}

#[test]
fn with_nothing_selected_or_under_the_cursor_nothing_happens() {
    let mut w = World::new();
    assert!(!w.click(v(-3.0, -3.0, 0.0)));
    assert_eq!(w.editor.message(), Some("Select something to rotate"));
    assert_eq!(w.stage(), RotateStage::Center);
    assert!(w.editor.selection().is_empty());
    w.assert_positions(&SQUARE);
}

#[test]
fn cursor_angles_snap_to_15_degrees() {
    let mut w = World::new();
    w.select_square();
    w.start_at_origin();
    w.move_to(at(DVec3::ZERO, 3.0, 44.0));
    assert!((w.angle() - 45.0).abs() < 1e-9, "{}", w.angle());
    w.move_to(at(DVec3::ZERO, 3.0, 40.0));
    assert!((w.angle() - 40.0).abs() < 1e-6, "{}", w.angle());
    assert_eq!(w.editor.measurement(), Some(("Angle", "40.0°".to_string())));
    w.move_to(at(DVec3::ZERO, 3.0, -30.5));
    assert!((w.angle() + 30.0).abs() < 1e-9, "{}", w.angle());
    assert!(w.click(at(DVec3::ZERO, 3.0, -30.5)));
    w.assert_positions(&rotated(&SQUARE, DVec3::ZERO, DVec3::Z, -30.0));
}

#[test]
fn a_cursor_snapped_to_geometry_keeps_its_exact_angle() {
    let mut w = World::new();
    // An open radial edge starting at 88 degrees on the radius 3 circle: within the 15 degree
    // snap window of 90, but the arm must line up with the endpoint.
    let end = at(DVec3::ZERO, 3.0, 88.0);
    let far = at(DVec3::ZERO, 5.0, 88.0);
    let mut undo = UndoStack::new();
    let mut t = Target {
        model: &mut w.model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    draw_segments(&mut t, &[(end, far)], "Draw").expect("draw");
    w.select_square();
    w.start_at_origin();
    w.move_to(at(DVec3::ZERO, 1.5, 91.9));
    assert!((w.angle() - 90.0).abs() < 1e-9, "{}", w.angle());
    w.move_to(end);
    assert!((w.angle() - 88.0).abs() < 1e-6, "{}", w.angle());
    assert!(w.click(end));
    let mut want = rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 88.0);
    want.extend([end, far]);
    w.assert_positions(&want);
}

#[test]
fn hint_follows_the_cursor() {
    let mut w = World::new();
    w.select_square();
    w.move_to(DVec3::ZERO);
    assert!(w.editor.hint().is_some());
    w.start_at_origin();
    let p = at(DVec3::ZERO, 3.0, 40.0);
    w.move_to(p);
    let hint = w.editor.hint().expect("a hint");
    assert!(hint.point.distance(p) < 1e-6, "{:?}", hint.point);
}

#[test]
fn preview_shows_the_arms_and_the_moved_selection() {
    let mut w = World::new();
    w.select_square();
    assert!(!w.click(DVec3::ZERO));
    w.move_to(v(3.0, 0.0, 0.0));
    let arm = w.editor.preview();
    assert_eq!(arm.len(), 1, "{arm:?}");
    assert!(has_segment(&arm, DVec3::ZERO, v(3.0, 0.0, 0.0)));

    assert!(!w.click(v(3.0, 0.0, 0.0)));
    w.move_to(v(0.0, 2.0, 0.0));
    let segs = w.editor.preview();
    assert_eq!(segs.len(), 6, "{segs:?}");
    assert!(segs[0].0.distance(DVec3::ZERO) < 1e-9);
    assert!(segs[0].1.distance(v(3.0, 0.0, 0.0)) < 1e-6);
    assert!(segs[1].1.distance(v(0.0, 3.0, 0.0)) < 1e-6);
    let r = rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 90.0);
    for i in 0..4 {
        assert!(
            has_segment(&segs, r[i], r[(i + 1) % 4]),
            "{:?}-{:?} missing from {segs:?}",
            r[i],
            r[(i + 1) % 4]
        );
    }
    // The model itself has not moved yet.
    w.assert_positions(&SQUARE);
}

// -- typed angles ----------------------------------------------------------------------

#[test]
fn text_is_taken() {
    let mut w = World::new();
    assert!(!w.editor.text("x"));
    assert!(w.editor.text("45"));
    assert_eq!(w.editor.typed(), "45");
}

#[test]
fn typed_angle_while_rotating() {
    let mut w = World::new();
    w.select_square();
    w.start_at_origin();
    w.move_to(at(DVec3::ZERO, 3.0, -40.0));
    assert!(w.enter("45"));
    w.assert_positions(&rotated(&SQUARE, DVec3::ZERO, DVec3::Z, -45.0));
    assert_eq!(w.editor.typed(), "");
    assert_eq!(w.stage(), RotateStage::Center);
    assert_eq!(w.undo_count(), 1);
}

#[test]
fn typed_angle_right_after_changes_the_rotation() {
    let mut w = World::new();
    w.select_square();
    w.start_at_origin();
    assert!(w.click(v(0.0, 2.0, 0.0)));
    assert!(w.enter("30"));
    w.assert_positions(&rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 30.0));
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.editor.measurement(), Some(("Angle", "30.0°".to_string())));
    assert!(w.key(EditKey::Undo));
    w.assert_positions(&SQUARE);
    // After the undo the angle is no longer shown or changeable.
    assert_eq!(w.editor.measurement(), Some(("Angle", String::new())));
    assert!(!w.enter("10"));
    assert_eq!(w.editor.message(), Some("Click the center first"));
    w.assert_positions(&SQUARE);
}

#[test]
fn typed_angle_before_a_center_or_junk_is_refused() {
    let mut w = World::new();
    w.select_square();
    assert!(!w.enter("45"));
    assert_eq!(w.editor.message(), Some("Click the center first"));
    assert_eq!(w.editor.typed(), "");
    w.start_at_origin();
    assert!(!w.enter("4x"));
    assert_eq!(w.editor.message(), Some("Not an angle"));
    assert_eq!(w.stage(), RotateStage::Angle);
    // A good angle clears the message.
    assert!(w.enter("90"));
    assert_eq!(w.editor.message(), None);
}

#[test]
fn typed_angle_at_the_start_stage_is_refused() {
    let mut w = World::new();
    w.select_square();
    assert!(!w.click(DVec3::ZERO));
    assert!(!w.enter("45"));
    assert_eq!(w.editor.message(), Some("Click the center first"));
    assert_eq!(w.stage(), RotateStage::Start);
}

// -- keys and switches -----------------------------------------------------------------

#[test]
fn escape_drops_the_rotation() {
    let mut w = World::new();
    w.select_square();
    w.start_at_origin();
    w.move_to(v(0.0, 2.0, 0.0));
    assert!(w.editor.text("1"));
    assert!(!w.key(EditKey::Cancel));
    assert_eq!(w.stage(), RotateStage::Center);
    assert_eq!(w.editor.typed(), "");
    assert!(w.editor.preview().is_empty());
    w.assert_positions(&SQUARE);
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn undo_and_redo_drop_a_rotation_in_progress() {
    let mut w = World::new();
    w.select_square();
    w.start_at_origin();
    assert!(w.click(v(0.0, 2.0, 0.0)));
    w.start_at_origin();
    assert!(w.key(EditKey::Undo));
    assert_eq!(w.stage(), RotateStage::Center);
    w.assert_positions(&SQUARE);
    assert!(!w.click(DVec3::ZERO));
    assert!(w.key(EditKey::Redo));
    assert_eq!(w.stage(), RotateStage::Center);
    w.assert_positions(&rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 90.0));
}

#[test]
fn switching_tools_forgets_the_rotation() {
    let mut w = World::new();
    w.select_square();
    w.start_at_origin();
    assert!(w.click(v(0.0, 2.0, 0.0)));
    w.start_at_origin();
    w.editor.set_tool(Tool::Line);
    w.editor.set_tool(Tool::Rotate);
    assert_eq!(w.stage(), RotateStage::Center);
    assert_eq!(w.editor.measurement(), Some(("Angle", String::new())));
    assert!(!w.enter("45"));
    w.assert_positions(&rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 90.0));
}

#[test]
fn arrow_keys_lock_the_axis_before_the_center() {
    let mut w = World::new();
    w.select_square();
    assert!(!w.key(EditKey::Lock(0)));
    assert_eq!(w.editor.plane_lock(), Some(0));
    w.move_to(v(1.5, 0.5, 0.0));
    let n = w.editor.draw_plane().unwrap().normal();
    assert!(n.cross(DVec3::X).length() < 1e-9, "{n:?}");
    assert!(!w.key(EditKey::Lock(0)));
    assert_eq!(w.editor.plane_lock(), None);
    assert!(!w.key(EditKey::Lock(3)));
    assert_eq!(w.editor.plane_lock(), None);

    // Locked to red: a quarter turn stands the square up in the plane x = const.
    assert!(!w.key(EditKey::Lock(0)));
    assert!(!w.click(DVec3::ZERO));
    // Locks don't change once the center is placed.
    assert!(!w.key(EditKey::Lock(2)));
    assert_eq!(w.editor.plane_lock(), Some(0));
    assert!(!w.click(v(0.0, 3.0, 0.0)));
    assert_eq!(w.stage(), RotateStage::Angle);
    assert!(w.enter("90"));
    let got = w.positions();
    assert_eq!(got.len(), 4);
    assert!(got.iter().all(|p| p.y.abs() < 1e-6), "{got:?}");
    assert!(
        got.iter().any(|p| (p.z.abs() - 1.0).abs() < 1e-6),
        "{got:?}"
    );
    assert!(w.ctx().validate().is_empty());
    // The lock stays after a rotation; Esc clears it.
    assert_eq!(w.editor.plane_lock(), Some(0));
    w.key(EditKey::Cancel);
    assert_eq!(w.editor.plane_lock(), None);
}

/// The box (0,0,0)-(2,2,1), seen from the iso view.
fn slab_world() -> World {
    let mut w = World {
        model: Model::new(),
        editor: Editor::new(),
        view: view(StandardView::Iso),
    };
    w.editor.unit = Unit::Meter;
    w.draw(&[
        v(0.0, 0.0, 0.0),
        v(2.0, 0.0, 0.0),
        v(2.0, 2.0, 0.0),
        v(0.0, 2.0, 0.0),
    ]);
    let face = w.square_face();
    let up = w.ctx().face_plane(face).unwrap().normal.z.signum();
    let mut undo = UndoStack::new();
    push_pull(&mut w.model, BodyId::Root, face, up, &mut undo, "Slab").expect("slab");
    assert_eq!(w.ctx().face_count(), 6);
    w.editor.set_tool(Tool::Rotate);
    w
}

#[test]
fn the_face_under_the_cursor_sets_the_plane_until_the_center() {
    let mut w = slab_world();
    let (_, dir) = w.view.ray(DVec2::new(400.0, 300.0));
    // A side face that faces the camera.
    let (side, normal, centre) = w
        .ctx()
        .faces()
        .filter_map(|(id, face)| {
            let n = w.ctx().face_plane(id)?.normal;
            let pts = w.ctx().loop_points(&face.outer);
            let c = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
            (n.z.abs() < 0.5 && n.dot(dir) < 0.0).then_some((id, n, c))
        })
        .next()
        .expect("a visible side");
    w.move_to(centre);
    let n = w.editor.draw_plane().unwrap().normal();
    assert!(n.distance(normal) < 1e-9, "{n:?} vs {normal:?} ({side:?})");

    // Over the top face the plane is horizontal, facing up.
    w.move_to(v(1.0, 1.0, 1.0));
    let n = w.editor.draw_plane().unwrap().normal();
    assert!(n.distance(DVec3::Z) < 1e-9, "{n:?}");

    // Once the center is on the top face, hovering the side keeps the plane.
    w.editor.set_tool(Tool::Select);
    let top = w.px(v(1.0, 1.0, 1.0));
    w.move_to(v(1.0, 1.0, 1.0));
    w.editor.click(&mut w.model, &w.view, top, NONE, 3);
    assert_eq!(w.editor.selection().len(), 6 + 12);
    w.editor.set_tool(Tool::Rotate);
    assert!(!w.click(v(1.0, 1.0, 1.0)));
    w.move_to(centre);
    let n = w.editor.draw_plane().unwrap().normal();
    assert!(n.distance(DVec3::Z) < 1e-9, "{n:?}");

    // Turn the whole box a quarter turn about the vertical through its top's center.
    assert!(!w.click(v(2.0, 1.0, 1.0)));
    assert!(w.enter("90"));
    assert_eq!(w.ctx().face_count(), 6);
    assert!(w.ctx().validate().is_empty());
    let pts = w.positions();
    let min = pts.iter().copied().fold(DVec3::INFINITY, DVec3::min);
    let max = pts.iter().copied().fold(DVec3::NEG_INFINITY, DVec3::max);
    assert!(min.distance(v(0.0, 0.0, 0.0)) < 1e-6, "{min:?}");
    assert!(max.distance(v(2.0, 2.0, 1.0)) < 1e-6, "{max:?}");
    assert_eq!(w.editor.selection().len(), 18);
}

#[test]
fn points_after_the_center_lie_in_the_protractor_plane() {
    let mut w = World::new();
    w.view = view(StandardView::Iso);
    w.select_square();
    // A wall-like protractor: locked to red, centered at the origin.
    assert!(!w.key(EditKey::Lock(0)));
    assert!(!w.click(DVec3::ZERO));
    // Over empty space the cursor ray meets the plane x = 0 off the ground.
    let p = v(0.0, 2.0, 2.0);
    w.move_to(p);
    let hint = w.editor.hint().expect("a hint");
    assert!(hint.point.distance(p) < 1e-6, "{:?}", hint.point);
    assert!(!w.click(p));
    // Start along (0, 1, 1); a typed quarter turn about red.
    assert!(w.enter("90"));
    let got = w.positions();
    assert!(got.iter().all(|q| q.y.abs() < 1e-6), "{got:?}");
}
