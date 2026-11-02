// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Positioning a texture on a face in the editor (`docs/specs/m4-app-texture-position.md`).

use drydock::editor::*;
use drydock::input::EditKey;
use drydock_kernel::model::*;
use drydock_kernel::paint::{Side, paint};
use drydock_kernel::select::Entity;
use drydock_kernel::texture::side_map;
use drydock_kernel::tools::{Target, draw_segments};
use drydock_kernel::topology::FaceId;
use drydock_kernel::units::Unit;
use glam::{DVec2, DVec3};

fn undo_label(editor: &Editor) -> Option<String> {
    editor.undo_stack().undo_label().map(str::to_owned)
}

/// A unit square in the root with a 1 m textured material on its front; returns everything.
fn setup() -> (Model, Editor, FaceId, MaterialId) {
    let mut model = Model::new();
    let p = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    let corners = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
    let segs: Vec<_> = (0..4).map(|i| (corners[i], corners[(i + 1) % 4])).collect();
    let mut undo = UndoStack::new();
    let mut t = Target {
        model: &mut model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    draw_segments(&mut t, &segs, "Draw").expect("draw");
    let face = model
        .root()
        .geometry()
        .faces()
        .next()
        .map(|(id, _)| id)
        .expect("face");
    let mut editor = Editor::new();
    let mut material = Material::new("Brick", [1.0, 1.0, 1.0, 1.0]);
    material.texture = Some(Texture::from("brick.png"));
    let id = editor
        .create_material(&mut model, material)
        .expect("material");
    let mut undo = UndoStack::new();
    paint(
        &mut model,
        BodyId::Root,
        &[Entity::Face(face)],
        Side::Front,
        Some(id),
        &mut undo,
        "Paint",
    )
    .expect("paint");
    (model, editor, face, id)
}

fn stored(model: &Model, face: FaceId, side: Side) -> Option<drydock_kernel::texture::TextureMap> {
    let m = model.root().face_materials(face);
    match side {
        Side::Front => m.front_map,
        Side::Back => m.back_map,
    }
}

fn close(a: DVec2, b: DVec2) -> bool {
    (a - b).length() < 1e-9
}

#[test]
fn move_shifts_texture_coordinates_and_is_one_undo_step() {
    let (mut model, mut editor, face, _) = setup();
    let before = side_map(&model, BodyId::Root, face, Side::Front).unwrap();
    let p = DVec3::new(0.25, 0.5, 0.0);
    assert!(editor.position_texture(
        &mut model,
        face,
        Side::Front,
        TextureOp::Move(DVec2::new(0.1, -0.2))
    ));
    let after = side_map(&model, BodyId::Root, face, Side::Front).unwrap();
    assert!(close(
        after.apply(p),
        before.apply(p) + DVec2::new(0.1, -0.2)
    ));
    assert_eq!(undo_label(&editor).as_deref(), Some("Position Texture"));
    assert_eq!(editor.message(), None);
}

#[test]
fn rotate_turns_about_the_pivot() {
    let (mut model, mut editor, face, _) = setup();
    let before = side_map(&model, BodyId::Root, face, Side::Front).unwrap();
    let p = DVec3::new(0.7, 0.3, 0.0);
    let about = DVec2::new(0.5, 0.5);
    assert!(editor.position_texture(
        &mut model,
        face,
        Side::Front,
        TextureOp::Rotate {
            angle: std::f64::consts::FRAC_PI_2,
            about
        }
    ));
    let after = side_map(&model, BodyId::Root, face, Side::Front).unwrap();
    // The pivot stays; a point one unit right of it ends one unit above it.
    let q = before.apply(p);
    let d = q - about;
    let expected = about + DVec2::new(-d.y, d.x);
    assert!(close(after.apply(p), expected));
}

#[test]
fn scale_stretches_about_the_pivot() {
    let (mut model, mut editor, face, _) = setup();
    let before = side_map(&model, BodyId::Root, face, Side::Front).unwrap();
    let p = DVec3::new(0.9, 0.1, 0.0);
    let about = DVec2::new(0.2, 0.3);
    assert!(editor.position_texture(
        &mut model,
        face,
        Side::Front,
        TextureOp::Scale {
            factor: DVec2::new(2.0, 0.5),
            about
        }
    ));
    let after = side_map(&model, BodyId::Root, face, Side::Front).unwrap();
    let d = before.apply(p) - about;
    assert!(close(after.apply(p), about + d * DVec2::new(2.0, 0.5)));
}

#[test]
fn reset_returns_to_the_default_and_undo_restores() {
    let (mut model, mut editor, face, _) = setup();
    let default = side_map(&model, BodyId::Root, face, Side::Front).unwrap();
    assert!(editor.position_texture(
        &mut model,
        face,
        Side::Front,
        TextureOp::Move(DVec2::new(0.3, 0.3))
    ));
    assert!(stored(&model, face, Side::Front).is_some());
    assert!(editor.position_texture(&mut model, face, Side::Front, TextureOp::Reset));
    assert_eq!(stored(&model, face, Side::Front), None);
    assert_eq!(
        side_map(&model, BodyId::Root, face, Side::Front).unwrap(),
        default
    );
    assert_eq!(undo_label(&editor).as_deref(), Some("Reset Texture"));
    assert!(editor.key(EditKey::Undo, &mut model));
    assert!(stored(&model, face, Side::Front).is_some());
    assert!(editor.key(EditKey::Undo, &mut model));
    assert_eq!(stored(&model, face, Side::Front), None);
}

#[test]
fn nothing_changed_pushes_no_step() {
    let (mut model, mut editor, face, _) = setup();
    let depth = undo_label(&editor);
    assert!(!editor.position_texture(&mut model, face, Side::Front, TextureOp::Reset));
    assert!(!editor.position_texture(&mut model, face, Side::Front, TextureOp::Move(DVec2::ZERO)));
    assert_eq!(undo_label(&editor), depth);
    assert_eq!(editor.message(), None);
}

#[test]
fn a_side_without_a_texture_is_refused() {
    let (mut model, mut editor, face, _) = setup();
    let depth = undo_label(&editor);
    assert!(!editor.position_texture(
        &mut model,
        face,
        Side::Back,
        TextureOp::Move(DVec2::new(1.0, 0.0))
    ));
    assert_eq!(editor.message(), Some("That side has no texture"));
    assert_eq!(undo_label(&editor), depth);
    assert_eq!(stored(&model, face, Side::Back), None);
}

#[test]
fn a_plain_material_is_refused() {
    let (mut model, mut editor, face, _) = setup();
    let plain = editor
        .create_material(&mut model, Material::new("Red", [1.0, 0.0, 0.0, 1.0]))
        .unwrap();
    let mut undo = UndoStack::new();
    paint(
        &mut model,
        BodyId::Root,
        &[Entity::Face(face)],
        Side::Front,
        Some(plain),
        &mut undo,
        "Paint",
    )
    .unwrap();
    assert!(!editor.position_texture(
        &mut model,
        face,
        Side::Front,
        TextureOp::Move(DVec2::new(1.0, 0.0))
    ));
    assert_eq!(editor.message(), Some("That side has no texture"));
}

#[test]
fn a_missing_face_is_refused() {
    let (_, _, face, _) = setup();
    let mut empty = Model::new();
    let mut editor = Editor::new();
    assert!(!editor.position_texture(&mut empty, face, Side::Front, TextureOp::Reset));
    assert_eq!(editor.message(), Some("That side has no texture"));
}

#[test]
fn bad_numbers_are_refused() {
    let (mut model, mut editor, face, _) = setup();
    let depth = undo_label(&editor);
    for op in [
        TextureOp::Move(DVec2::new(f64::NAN, 0.0)),
        TextureOp::Rotate {
            angle: f64::INFINITY,
            about: DVec2::ZERO,
        },
        TextureOp::Scale {
            factor: DVec2::new(0.0, 1.0),
            about: DVec2::ZERO,
        },
        TextureOp::Scale {
            factor: DVec2::new(-1.0, 1.0),
            about: DVec2::ZERO,
        },
        TextureOp::Scale {
            factor: DVec2::new(1.0, 1.0),
            about: DVec2::new(f64::NAN, 0.0),
        },
    ] {
        assert!(!editor.position_texture(&mut model, face, Side::Front, op));
        assert_eq!(editor.message(), Some("Can't position that texture"));
        assert_eq!(undo_label(&editor), depth);
        assert_eq!(stored(&model, face, Side::Front), None);
    }
}

#[test]
fn only_the_given_side_changes() {
    let (mut model, mut editor, face, id) = setup();
    let mut undo = UndoStack::new();
    paint(
        &mut model,
        BodyId::Root,
        &[Entity::Face(face)],
        Side::Back,
        Some(id),
        &mut undo,
        "Paint",
    )
    .unwrap();
    let back_before = side_map(&model, BodyId::Root, face, Side::Back).unwrap();
    assert!(editor.position_texture(
        &mut model,
        face,
        Side::Front,
        TextureOp::Move(DVec2::new(0.5, 0.0))
    ));
    assert_eq!(stored(&model, face, Side::Back), None);
    assert_eq!(
        side_map(&model, BodyId::Root, face, Side::Back).unwrap(),
        back_before
    );
    let m = model.root().face_materials(face);
    assert_eq!(m.front, Some(id));
    assert_eq!(m.back, Some(id));
}
