// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-texture-turn: turning a stretched texture keeps the image copy a rectangle.
//! See docs/specs/m4-texture-turn.md.

use drydock::editor::*;
use drydock::input::{Modifiers, Tool};
use drydock::texture_handles::{Grip, TextureHandles, turn_grip_point};
use drydock_kernel::model::{BodyId, Material, Model, ModelEdit, Texture, UndoStack};
use drydock_kernel::paint::{Side, create_material, paint};
use drydock_kernel::select::Entity;
use drydock_kernel::texture::{TextureMap, side_map};
use drydock_kernel::topology::FaceId;
use drydock_kernel::units::Unit;
use drydock_view::{Bounds, Camera, Projection, StandardView};
use glam::{DAffine2, DVec2, DVec3};
use proptest::prelude::*;

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn top(min: f64, max: f64) -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Top);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(min),
            max: DVec3::splat(max),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

/// A closed loop of edges at the root; returns its face.
fn face_from(model: &mut Model, pts: &[DVec3]) -> FaceId {
    let mut log = ModelEdit::new();
    model
        .edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
            for i in 0..pts.len() {
                ctx.add_edge(pts[i], pts[(i + 1) % pts.len()], edit)
                    .unwrap();
            }
        })
        .unwrap();
    model
        .body(BodyId::Root)
        .unwrap()
        .geometry()
        .faces()
        .next()
        .unwrap()
        .0
}

/// Paint a 1 m texture on both sides of `face`.
fn texture(model: &mut Model, face: FaceId) {
    let mut undo = UndoStack::new();
    let mut material = Material::new("Brick", [1.0, 1.0, 1.0, 1.0]);
    let mut texture = Texture::from("brick.png");
    texture.size = DVec2::ONE;
    material.texture = Some(texture);
    let id = create_material(model, material, &mut undo, "New").unwrap();
    for side in [Side::Front, Side::Back] {
        paint(
            model,
            BodyId::Root,
            &[Entity::Face(face)],
            side,
            Some(id),
            &mut undo,
            "Paint",
        )
        .unwrap();
    }
}

/// A 3 m square at z = 0, textured on both sides.
fn flat() -> (Model, Editor, FaceId) {
    let mut model = Model::new();
    let face = face_from(
        &mut model,
        &[
            v(0.0, 0.0, 0.0),
            v(3.0, 0.0, 0.0),
            v(3.0, 3.0, 0.0),
            v(0.0, 3.0, 0.0),
        ],
    );
    texture(&mut model, face);
    let mut editor = Editor::new();
    set_unit(&mut model, Unit::Meter);
    editor.set_tool(Tool::Select);
    (model, editor, face)
}

/// A 3 m square standing on a slanted line in plan, textured on both sides.
fn wall() -> (Model, Editor, FaceId) {
    let mut model = Model::new();
    let d = v(0.8, 0.6, 0.0);
    let face = face_from(
        &mut model,
        &[
            v(0.0, 0.0, 0.0),
            d * 3.0,
            d * 3.0 + v(0.0, 0.0, 3.0),
            v(0.0, 0.0, 3.0),
        ],
    );
    texture(&mut model, face);
    let mut editor = Editor::new();
    set_unit(&mut model, Unit::Meter);
    editor.set_tool(Tool::Select);
    (model, editor, face)
}

fn map(model: &Model, face: FaceId, side: Side) -> TextureMap {
    side_map(model, BodyId::Root, face, side).unwrap()
}

fn px(view: &Viewport, p: DVec3) -> DVec2 {
    view.project(p).unwrap()
}

fn handles(editor: &Editor, model: &Model) -> TextureHandles {
    editor.texture_handles(model).expect("handles showing")
}

/// The image copy's corners on `side` (the handles show it; they end again).
fn corners(model: &Model, editor: &mut Editor, face: FaceId, side: Side) -> [DVec3; 4] {
    assert!(editor.start_texture_handles(model, face, side));
    let c = handles(editor, model).corners;
    editor.end_texture_handles();
    c
}

fn middle(c: &[DVec3; 4]) -> DVec3 {
    c.iter().sum::<DVec3>() / 4.0
}

/// The copy's bottom and left edges as vectors.
fn edges(c: &[DVec3; 4]) -> (DVec3, DVec3) {
    (c[1] - c[0], c[3] - c[0])
}

/// The copy is a rectangle: its bottom and left edges are square to each other.
fn assert_rectangle(c: &[DVec3; 4], what: &str) {
    let (a, b) = edges(c);
    let cos = a.dot(b) / (a.length() * b.length());
    assert!(
        cos.abs() < 1e-9,
        "{what}: corner angle off square, cos {cos}"
    );
}

/// The signed angle (radians) from `a` to `b` about `n`.
fn angle_about(a: DVec3, b: DVec3, n: DVec3) -> f64 {
    let n = n.normalize();
    a.cross(b).dot(n).atan2(a.dot(b))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn close2(a: DVec2, b: DVec2) -> bool {
    (a - b).length() < 1e-9
}

fn op(editor: &mut Editor, model: &mut Model, face: FaceId, side: Side, op: TextureOp) {
    assert!(
        editor.position_texture(model, face, side, op),
        "{op:?} changed nothing"
    );
}

fn turn(angle_deg: f64) -> TextureOp {
    TextureOp::Rotate {
        angle: angle_deg.to_radians(),
        about: DVec2::new(0.3, -0.2),
    }
}

fn stretch(x: f64, y: f64) -> TextureOp {
    TextureOp::Scale {
        factor: DVec2::new(x, y),
        about: DVec2::new(0.1, 0.4),
    }
}

#[test]
fn an_unstretched_turn_is_unchanged() {
    // Equal scale on both axes: the turn is the plain texture-space turn of before.
    for (model_fn, side) in [
        (flat as fn() -> (Model, Editor, FaceId), Side::Front),
        (flat, Side::Back),
        (wall, Side::Front),
        (wall, Side::Back),
    ] {
        let (mut model, mut editor, face) = model_fn();
        op(&mut editor, &mut model, face, side, stretch(2.5, 2.5));
        op(&mut editor, &mut model, face, side, turn(17.0));
        let before = map(&model, face, side);
        let about = DVec2::new(0.3, -0.2);
        let a = 40f64.to_radians();
        let expected = before.then(
            DAffine2::from_translation(about)
                * DAffine2::from_angle(a)
                * DAffine2::from_translation(-about),
        );
        op(&mut editor, &mut model, face, side, turn(40.0));
        let after = map(&model, face, side);
        assert!(
            (after.u - expected.u).length() < 1e-9 && (after.v - expected.v).length() < 1e-9,
            "{side:?}: {after:?} vs {expected:?}"
        );
    }
}

#[test]
fn a_typed_turn_after_a_stretch_keeps_a_rectangle() {
    for (model_fn, side) in [
        (flat as fn() -> (Model, Editor, FaceId), Side::Front),
        (flat, Side::Back),
        (wall, Side::Front),
        (wall, Side::Back),
    ] {
        let (mut model, mut editor, face) = model_fn();
        op(&mut editor, &mut model, face, side, stretch(2.0, 0.5));
        let before = corners(&model, &mut editor, face, side);
        assert_rectangle(&before, "stretched");
        op(&mut editor, &mut model, face, side, turn(30.0));
        let after = corners(&model, &mut editor, face, side);
        assert_rectangle(&after, &format!("{side:?} turned"));
        let (b0, b1) = edges(&before);
        let (a0, a1) = edges(&after);
        assert!(close(a0.length(), b0.length()), "{side:?} width changed");
        assert!(close(a1.length(), b1.length()), "{side:?} height changed");
    }
}

#[test]
fn a_stretched_texture_turns_by_the_same_angle_as_an_unstretched_one() {
    for (model_fn, side) in [
        (flat as fn() -> (Model, Editor, FaceId), Side::Front),
        (flat, Side::Back),
        (wall, Side::Front),
        (wall, Side::Back),
    ] {
        let (mut plain, mut plain_editor, face) = model_fn();
        let n = map(&plain, face, side)
            .u
            .truncate()
            .cross(map(&plain, face, side).v.truncate());
        let c0 = corners(&plain, &mut plain_editor, face, side);
        op(&mut plain_editor, &mut plain, face, side, turn(30.0));
        let c1 = corners(&plain, &mut plain_editor, face, side);
        let expected = angle_about(edges(&c0).0, edges(&c1).0, n);
        assert!(expected.abs() > 0.1, "{side:?}: the plain turn turns");

        let (mut model, mut editor, face) = model_fn();
        op(&mut editor, &mut model, face, side, stretch(3.0, 0.7));
        let s0 = corners(&model, &mut editor, face, side);
        op(&mut editor, &mut model, face, side, turn(30.0));
        let s1 = corners(&model, &mut editor, face, side);
        let got = angle_about(edges(&s0).0, edges(&s1).0, n);
        assert!(close(got, expected), "{side:?}: {got} vs {expected}");
    }
}

#[test]
fn a_typed_turn_keeps_the_about_point_in_place() {
    let (mut model, mut editor, face) = wall();
    op(
        &mut editor,
        &mut model,
        face,
        Side::Front,
        stretch(2.0, 0.5),
    );
    let m0 = map(&model, face, Side::Front);
    // The face point that has texture point `about` now.
    let about = DVec2::new(0.3, -0.2);
    let p0 = v(0.0, 0.0, 0.0);
    let (e1, e2) = (v(0.8, 0.6, 0.0), v(0.0, 0.0, 1.0));
    let a = DVec2::new(m0.u.truncate().dot(e1), m0.v.truncate().dot(e1));
    let b = DVec2::new(m0.u.truncate().dot(e2), m0.v.truncate().dot(e2));
    let rhs = about - m0.apply(p0);
    let det = a.x * b.y - b.x * a.y;
    let s = (rhs.x * b.y - b.x * rhs.y) / det;
    let t = (a.x * rhs.y - rhs.x * a.y) / det;
    let fixed = p0 + e1 * s + e2 * t;
    assert!(close2(m0.apply(fixed), about));
    op(&mut editor, &mut model, face, Side::Front, turn(25.0));
    let m1 = map(&model, face, Side::Front);
    assert!(close2(m1.apply(fixed), about), "{}", m1.apply(fixed));
}

/// Grab the turn grip and let go with the cursor at `p`.
fn handle_turn(model: &mut Model, editor: &mut Editor, view: &Viewport, face: FaceId, p: DVec3) {
    assert!(editor.start_texture_handles(model, face, Side::Front));
    let h = handles(editor, model);
    assert!(!editor.click(model, view, px(view, turn_grip_point(&h.corners)), NONE, 1));
    assert_eq!(handles(editor, model).held, Some(Grip::Turn));
    editor.pointer_move(model, view, px(view, p), NONE);
    assert!(editor.click(model, view, px(view, p), NONE, 1));
}

#[test]
fn the_turn_grip_points_the_top_of_a_stretched_copy_at_the_cursor() {
    let (mut model, mut editor, face) = flat();
    let view = top(-2.0, 5.0);
    op(
        &mut editor,
        &mut model,
        face,
        Side::Front,
        stretch(2.0, 0.5),
    );
    let c0 = corners(&model, &mut editor, face, Side::Front);
    let m0 = map(&model, face, Side::Front);
    let center = middle(&c0);
    // 50 degrees from +x on the face, away from any face edge direction.
    let dir = v(50f64.to_radians().cos(), 50f64.to_radians().sin(), 0.0);
    handle_turn(&mut model, &mut editor, &view, face, center + dir * 0.9);
    let m1 = map(&model, face, Side::Front);
    assert!(
        close2(m1.apply(center), m0.apply(center)),
        "the copy's center moved"
    );
    let c1 = handles(&editor, &model).corners;
    assert_rectangle(&c1, "turned by the grip");
    let up = edges(&c1).1.normalize();
    assert!(
        (up - dir).length() < 1e-6,
        "the top points at {up}, the cursor is at {dir}"
    );
    let (b0, b1) = edges(&c0);
    let (a0, a1) = edges(&c1);
    assert!(close(a0.length(), b0.length()) && close(a1.length(), b1.length()));
}

#[test]
fn the_turn_grip_snaps_a_stretched_copy_to_the_face_edges() {
    let (mut model, mut editor, face) = flat();
    let view = top(-2.0, 5.0);
    op(
        &mut editor,
        &mut model,
        face,
        Side::Front,
        stretch(2.0, 0.5),
    );
    let c0 = corners(&model, &mut editor, face, Side::Front);
    let center = middle(&c0);
    // 2 degrees off the face's x edges.
    let a = (-2f64).to_radians();
    handle_turn(
        &mut model,
        &mut editor,
        &view,
        face,
        center + v(a.cos(), a.sin(), 0.0) * 0.9,
    );
    let c1 = handles(&editor, &model).corners;
    assert_rectangle(&c1, "snapped");
    let up = edges(&c1).1.normalize();
    assert!((up - DVec3::X).length() < 1e-9, "top points at {up}");
}

#[test]
fn a_corner_scale_after_a_turn_keeps_a_rectangle() {
    let (mut model, mut editor, face) = flat();
    let view = top(-2.0, 5.0);
    op(
        &mut editor,
        &mut model,
        face,
        Side::Front,
        stretch(1.5, 0.6),
    );
    op(&mut editor, &mut model, face, Side::Front, turn(35.0));
    assert!(editor.start_texture_handles(&model, face, Side::Front));
    let h = handles(&editor, &model);
    let pin = h.corners[0];
    assert!(!editor.click(&mut model, &view, px(&view, h.corners[2]), NONE, 1));
    assert_eq!(handles(&editor, &model).held, Some(Grip::Scale(2)));
    // Out along the copy's own bottom edge 1.4 times and up its left edge 0.8 times.
    let (e0, e1) = edges(&h.corners);
    let p = pin + e0 * 1.4 + e1 * 0.8;
    assert!(editor.click(&mut model, &view, px(&view, p), NONE, 1));
    let c = handles(&editor, &model).corners;
    assert_rectangle(&c, "scaled after the turn");
}

#[test]
fn turning_back_after_a_stretch_restores_the_map() {
    let (mut model, mut editor, face) = wall();
    op(
        &mut editor,
        &mut model,
        face,
        Side::Front,
        stretch(2.0, 0.5),
    );
    let m0 = map(&model, face, Side::Front);
    op(&mut editor, &mut model, face, Side::Front, turn(30.0));
    op(&mut editor, &mut model, face, Side::Front, turn(-30.0));
    let m1 = map(&model, face, Side::Front);
    assert!(
        (m1.u - m0.u).length() < 1e-9 && (m1.v - m0.v).length() < 1e-9,
        "{m1:?} vs {m0:?}"
    );
}

#[derive(Clone, Debug)]
enum Step {
    Move(f64, f64),
    Turn(f64),
    Scale(f64, f64),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (-2.0..2.0f64, -2.0..2.0f64).prop_map(|(x, y)| Step::Move(x, y)),
        (-170.0..170.0f64)
            .prop_filter("turns", |a: &f64| a.abs() > 0.5)
            .prop_map(Step::Turn),
        (0.3..3.0f64, 0.3..3.0f64)
            .prop_filter("stretches", |(x, y)| (x - 1.0).abs() > 0.01
                || (y - 1.0).abs() > 0.01)
            .prop_map(|(x, y)| Step::Scale(x, y)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Any typed moves, turns and stretches keep the image copy a rectangle on both kinds
    /// of face and both sides.
    #[test]
    fn typed_steps_never_skew_the_copy(
        steps in proptest::collection::vec(step(), 1..8),
        on_wall in any::<bool>(),
        back in any::<bool>(),
    ) {
        let (mut model, mut editor, face) = if on_wall { wall() } else { flat() };
        let side = if back { Side::Back } else { Side::Front };
        for s in &steps {
            let o = match *s {
                Step::Move(x, y) => TextureOp::Move(DVec2::new(x, y)),
                Step::Turn(a) => turn(a),
                Step::Scale(x, y) => stretch(x, y),
            };
            editor.position_texture(&mut model, face, side, o);
            let c = corners(&model, &mut editor, face, side);
            let (a, b) = edges(&c);
            let cos = a.dot(b) / (a.length() * b.length());
            prop_assert!(cos.abs() < 1e-7, "after {s:?} in {steps:?}: cos {cos}");
        }
    }
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
