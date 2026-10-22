// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! `push_pull` (`docs/specs/m3-pushpull.md`).

use glam::DVec3;
use modeler_kernel::model::*;
use modeler_kernel::pushpull::*;
use modeler_kernel::topology::*;

mod pushpull_common;
use pushpull_common::*;

fn run(
    model: &mut Model,
    undo: &mut UndoStack,
    face: FaceId,
    d: f64,
) -> Result<PushPulled, PushPullError> {
    push_pull(model, BodyId::Root, face, d, undo, "Push/Pull")
}

fn assert_close(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-9, "{a:?} != {b:?}");
}

fn assert_unchanged(model: &Model, before: &Model, undo: &UndoStack) {
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

fn all_z(c: &Context, f: FaceId, z: f64) -> bool {
    face_points(c, f).iter().all(|p| (p.z - z).abs() < 1e-9)
}

fn mats(front: u64, back: u64) -> FaceMaterials {
    FaceMaterials {
        front: Some(MaterialId(front)),
        back: Some(MaterialId(back)),
    }
}

fn paint(model: &mut Model, f: FaceId, m: FaceMaterials) {
    let mut edit = ModelEdit::new();
    let material = |name: &str| Material {
        name: name.into(),
        color: [1.0; 4],
        texture: None,
    };
    let a = model.add_material(material("a"), &mut edit);
    let b = model.add_material(material("b"), &mut edit);
    assert_eq!((Some(a), Some(b)), (m.front, m.back));
    model
        .set_face_materials(BodyId::Root, f, m, &mut edit)
        .unwrap();
}

// -- checks ----------------------------------------------------------------------------

#[test]
fn unknown_body_and_face_are_refused() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    let before = model.clone();
    let body = BodyId::Definition(DefinitionId(77));
    assert_eq!(
        push_pull(&mut model, body, f, 1.0, &mut undo, "Push/Pull"),
        Err(PushPullError::MissingBody(body))
    );
    assert_eq!(
        run(&mut model, &mut undo, FaceId(999), 1.0),
        Err(PushPullError::MissingFace(FaceId(999)))
    );
    // The body check comes first.
    assert_eq!(
        push_pull(&mut model, body, FaceId(999), f64::NAN, &mut undo, "x"),
        Err(PushPullError::MissingBody(body))
    );
    assert_unchanged(&model, &before, &undo);
}

#[test]
fn bad_distances_are_refused() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    let before = model.clone();
    for d in [0.0, 1e-6, -1e-5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            run(&mut model, &mut undo, f, d),
            Err(PushPullError::BadDistance),
            "{d}"
        );
    }
    assert_eq!(
        run(&mut model, &mut undo, FaceId(999), 0.0),
        Err(PushPullError::MissingFace(FaceId(999)))
    );
    assert_unchanged(&model, &before, &undo);
}

// -- isolated faces ----------------------------------------------------------------------

#[test]
fn a_lone_square_pulled_up_becomes_a_closed_box() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    let r = run(&mut model, &mut undo, f, 2.0).unwrap();
    let c = ctx(&model);
    assert!(c.validate().is_empty(), "{:?}", c.validate());
    assert_eq!(
        (c.vertex_count(), c.edge_count(), c.face_count()),
        (8, 12, 6)
    );
    assert!(c.face(f).is_none(), "the pushed face is replaced");
    assert_eq!(r.sides.len(), 4);
    let base = r.base.expect("a lone face leaves a base");
    assert!(all_z(c, r.cap, 2.0));
    assert!(all_z(c, base, 0.0));
    assert_close(normal(c, r.cap), DVec3::Z);
    assert_close(normal(c, base), -DVec3::Z);
    for &s in &r.sides {
        assert!(normal(c, s).z.abs() < 1e-9);
    }
    assert!(closed(c));
    assert!(normals_outward(c));
    assert!((volume(c) - 2.0).abs() < 1e-9);
}

#[test]
fn a_lone_square_pushed_down_becomes_a_closed_box_below() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    let r = run(&mut model, &mut undo, f, -0.5).unwrap();
    let c = ctx(&model);
    assert!(c.validate().is_empty(), "{:?}", c.validate());
    let base = r.base.unwrap();
    assert!(all_z(c, r.cap, -0.5));
    assert!(all_z(c, base, 0.0));
    assert_close(normal(c, r.cap), -DVec3::Z);
    assert_close(normal(c, base), DVec3::Z);
    assert!(closed(c));
    assert!(normals_outward(c));
    assert!((volume(c) - 0.5).abs() < 1e-9);
}

#[test]
fn sides_follow_the_working_loop_order() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    let uses = ctx(&model).face(f).unwrap().outer.uses.clone();
    let r = run(&mut model, &mut undo, f, 1.0).unwrap();
    let c = ctx(&model);
    for (i, &s) in r.sides.iter().enumerate() {
        let side = c.face(s).unwrap();
        assert!(side.inner.is_empty());
        assert_eq!(side.outer.uses.len(), 4);
        assert_eq!(
            side.outer.uses[0], uses[i],
            "side {i} starts with the old edge"
        );
    }
    assert_eq!(c.face(r.cap).unwrap().outer.uses.len(), 4);

    // Pushed backwards, the working loop is the reversed loop.
    let mut model = Model::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    let uses = ctx(&model).face(f).unwrap().outer.uses.clone();
    let reversed: Vec<EdgeUse> = uses
        .iter()
        .rev()
        .map(|u| EdgeUse {
            edge: u.edge,
            reversed: !u.reversed,
        })
        .collect();
    let r = run(&mut model, &mut UndoStack::new(), f, -1.0).unwrap();
    for (i, &s) in r.sides.iter().enumerate() {
        assert_eq!(ctx(&model).face(s).unwrap().outer.uses[0], reversed[i]);
    }
}

#[test]
fn side_loops_climb_from_the_old_edge() {
    let mut model = Model::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    let r = run(&mut model, &mut UndoStack::new(), f, 3.0).unwrap();
    let c = ctx(&model);
    for &s in &r.sides {
        let p = face_points(c, s);
        assert_eq!(p.len(), 4);
        assert!(p[0].z.abs() < 1e-9 && p[1].z.abs() < 1e-9);
        assert!((p[2].z - 3.0).abs() < 1e-9 && (p[3].z - 3.0).abs() < 1e-9);
        assert_close(p[2], p[1] + DVec3::Z * 3.0);
        assert_close(p[3], p[0] + DVec3::Z * 3.0);
    }
}

#[test]
fn materials_go_to_the_cap() {
    let mut model = Model::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    paint(&mut model, f, mats(0, 1));
    let r = run(&mut model, &mut UndoStack::new(), f, 1.0).unwrap();
    let b = model.root();
    assert_eq!(b.face_materials(r.cap), mats(0, 1));
    assert_eq!(b.face_materials(r.base.unwrap()), FaceMaterials::default());
    for &s in &r.sides {
        assert_eq!(b.face_materials(s), FaceMaterials::default());
    }
}

#[test]
fn materials_swap_sides_when_the_cap_is_reversed() {
    let mut model = Model::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    paint(&mut model, f, mats(0, 1));
    let r = run(&mut model, &mut UndoStack::new(), f, -1.0).unwrap();
    assert_eq!(model.root().face_materials(r.cap), mats(1, 0));
    assert_eq!(
        model.root().face_materials(r.base.unwrap()),
        FaceMaterials::default()
    );
}

#[test]
fn a_lone_face_with_a_hole_becomes_a_tube() {
    let mut model = Model::new();
    let pts = [
        v(0.0, 0.0, 0.0),
        v(3.0, 0.0, 0.0),
        v(3.0, 3.0, 0.0),
        v(0.0, 3.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(2.0, 1.0, 0.0),
        v(2.0, 2.0, 0.0),
        v(1.0, 2.0, 0.0),
    ];
    let f = mesh(&mut model, &pts, &[&[&[0, 1, 2, 3], &[4, 7, 6, 5]]])[0];
    let r = run(&mut model, &mut UndoStack::new(), f, 1.0).unwrap();
    let c = ctx(&model);
    assert!(c.validate().is_empty(), "{:?}", c.validate());
    assert_eq!(
        (c.vertex_count(), c.edge_count(), c.face_count()),
        (16, 24, 10)
    );
    assert_eq!(r.sides.len(), 8);
    assert_eq!(c.face(r.cap).unwrap().inner.len(), 1);
    assert_eq!(c.face(r.base.unwrap()).unwrap().inner.len(), 1);
    assert!(closed(c));
    assert!((volume(c) - 8.0).abs() < 1e-9);
    // The four hole walls face the tube's axis.
    let axis = v(1.5, 1.5, 0.0);
    for &s in &r.sides[4..] {
        let mid = centroid(&face_points(c, s));
        let towards = (axis - mid) * DVec3::new(1.0, 1.0, 0.0);
        assert!(normal(c, s).dot(towards) > 0.0);
    }
    for &s in &r.sides[..4] {
        let mid = centroid(&face_points(c, s));
        let away = (mid - axis) * DVec3::new(1.0, 1.0, 0.0);
        assert!(normal(c, s).dot(away) > 0.0);
    }
}

#[test]
fn a_tilted_face_moves_along_its_own_normal() {
    let mut model = Model::new();
    // A square in the x = 2 plane, fronting +x.
    let pts = [
        v(2.0, 0.0, 0.0),
        v(2.0, 1.0, 0.0),
        v(2.0, 1.0, 1.0),
        v(2.0, 0.0, 1.0),
    ];
    let f = mesh(&mut model, &pts, &[&[&[0, 1, 2, 3]]])[0];
    assert_close(normal(ctx(&model), f), DVec3::X);
    let r = run(&mut model, &mut UndoStack::new(), f, 0.25).unwrap();
    let c = ctx(&model);
    assert!(
        face_points(c, r.cap)
            .iter()
            .all(|p| (p.x - 2.25).abs() < 1e-9)
    );
    assert!(normals_outward(c));
    assert!((volume(c) - 0.25).abs() < 1e-9);
}

// -- slide -------------------------------------------------------------------------------

#[test]
fn a_box_top_slides_up_and_the_walls_follow() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let faces = cube(&mut model);
    let top = faces[1];
    let r = run(&mut model, &mut undo, top, 0.5).unwrap();
    assert_eq!(
        r,
        PushPulled {
            cap: top,
            sides: vec![],
            base: None
        }
    );
    let c = ctx(&model);
    assert!(c.validate().is_empty());
    assert_eq!(
        (c.vertex_count(), c.edge_count(), c.face_count()),
        (8, 12, 6)
    );
    for &f in &faces {
        assert!(c.face(f).is_some(), "every face keeps its id");
    }
    assert!(all_z(c, top, 1.5));
    assert!(all_z(c, faces[0], 0.0));
    assert!(normals_outward(c));
    assert!((volume(c) - 1.5).abs() < 1e-9);
}

#[test]
fn a_box_top_slides_down() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let r = run(&mut model, &mut UndoStack::new(), faces[1], -0.75).unwrap();
    assert_eq!(r.cap, faces[1]);
    let c = ctx(&model);
    assert!(all_z(c, faces[1], 0.25));
    assert!((volume(c) - 0.25).abs() < 1e-9);
}

#[test]
fn a_wall_slides_too() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let right = faces[4];
    let r = run(&mut model, &mut UndoStack::new(), right, 1.0).unwrap();
    assert_eq!(r.cap, right);
    let c = ctx(&model);
    assert!(
        face_points(c, right)
            .iter()
            .all(|p| (p.x - 2.0).abs() < 1e-9)
    );
    assert!((volume(c) - 2.0).abs() < 1e-9);
}

#[test]
fn sliding_onto_the_bottom_is_refused() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let faces = cube(&mut model);
    let before = model.clone();
    assert!(matches!(
        run(&mut model, &mut undo, faces[1], -1.0),
        Err(PushPullError::Invalid(_))
    ));
    assert_unchanged(&model, &before, &undo);
}

#[test]
fn sliding_past_the_bottom_is_refused_as_inverted() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let faces = cube(&mut model);
    let before = model.clone();
    // The four walls turn inside out; the front wall has the lowest id of them.
    let walls = [faces[2], faces[3], faces[4], faces[5]];
    let lowest = *walls.iter().min().unwrap();
    assert_eq!(
        run(&mut model, &mut undo, faces[1], -1.5),
        Err(PushPullError::Inverted(lowest))
    );
    assert_unchanged(&model, &before, &undo);
}

// -- extrude from a solid ------------------------------------------------------------------

#[test]
fn an_inset_on_a_box_top_pulls_out_a_block() {
    let mut model = Model::new();
    let faces = cube_with_inset(&mut model);
    let (ring, inner) = (faces[1], faces[6]);
    let r = run(&mut model, &mut UndoStack::new(), inner, 0.5).unwrap();
    let c = ctx(&model);
    assert!(c.validate().is_empty(), "{:?}", c.validate());
    assert!(r.base.is_none());
    assert_eq!(r.sides.len(), 4);
    assert!(c.face(inner).is_none());
    assert!(c.face(ring).is_some(), "the ring is untouched");
    assert!(all_z(c, r.cap, 1.5));
    assert_close(normal(c, r.cap), DVec3::Z);
    assert_eq!(c.face_count(), 11);
    assert!(closed(c));
    assert!((volume(c) - (1.0 + 0.125)).abs() < 1e-9);
    let axis = v(0.5, 0.5, 0.0);
    for &s in &r.sides {
        let away = (centroid(&face_points(c, s)) - axis) * DVec3::new(1.0, 1.0, 0.0);
        assert!(normal(c, s).dot(away) > 0.0, "block walls face out");
    }
}

#[test]
fn an_inset_pushed_in_makes_a_pocket() {
    let mut model = Model::new();
    let faces = cube_with_inset(&mut model);
    let inner = faces[6];
    let r = run(&mut model, &mut UndoStack::new(), inner, -0.5).unwrap();
    let c = ctx(&model);
    assert!(c.validate().is_empty(), "{:?}", c.validate());
    assert!(r.base.is_none());
    assert!(all_z(c, r.cap, 0.5));
    assert_close(normal(c, r.cap), DVec3::Z);
    assert!(closed(c));
    assert!((volume(c) - (1.0 - 0.125)).abs() < 1e-9);
    let axis = v(0.5, 0.5, 0.0);
    for &s in &r.sides {
        let towards = (axis - centroid(&face_points(c, s))) * DVec3::new(1.0, 1.0, 0.0);
        assert!(normal(c, s).dot(towards) > 0.0, "pocket walls face in");
    }
}

#[test]
fn a_ring_with_stretch_edges_and_others_extrudes() {
    // The ring's outer edges are stretch edges, its hole edges are not: so it extrudes, and
    // the inner face stays at the bottom of the new tube.
    let mut model = Model::new();
    let faces = cube_with_inset(&mut model);
    let (ring, inner) = (faces[1], faces[6]);
    let r = run(&mut model, &mut UndoStack::new(), ring, 0.5).unwrap();
    let c = ctx(&model);
    assert!(c.validate().is_empty(), "{:?}", c.validate());
    assert_eq!(r.sides.len(), 8);
    assert!(r.base.is_none());
    assert!(c.face(inner).is_some() && all_z(c, inner, 1.0));
    assert_eq!(c.face(r.cap).unwrap().inner.len(), 1);
    assert!(all_z(c, r.cap, 1.5));
    for f in [faces[0], faces[2], faces[3], faces[4], faces[5]] {
        assert!(c.face(f).is_some(), "walls keep their ids");
    }
    assert!(closed(c));
    assert!((volume(c) - (1.0 + 0.5 * 0.75)).abs() < 1e-9);
}

#[test]
fn a_neighbour_in_front_is_not_stretched() {
    // A wall standing up from the square's edge is on the front side: the square extrudes
    // downward fine, and upward would put a side on top of the wall.
    let mut model = Model::new();
    let pts = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
        v(1.0, 0.0, 1.0),
        v(0.0, 0.0, 1.0),
    ];
    let faces = mesh(&mut model, &pts, &[&[&[0, 1, 2, 3]], &[&[0, 5, 4, 1]]]);
    let mut undo = UndoStack::new();
    let before = model.clone();
    assert!(run(&mut model, &mut undo, faces[0], 0.5).is_err());
    assert_unchanged(&model, &before, &undo);
    let r = run(&mut model, &mut undo, faces[0], -0.5).unwrap();
    let c = ctx(&model);
    assert!(c.validate().is_empty());
    assert!(
        r.base.is_none(),
        "the square shares an edge, so it isn't isolated"
    );
    assert_eq!(r.sides.len(), 4);
    assert!(c.face(faces[1]).is_some());
}

#[test]
fn a_side_over_a_coplanar_face_is_refused_as_overlap() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let f = square(&mut model, 0.0, 0.0, 1.0);
    // A small panel in the y = 0 plane, inside where the front side would go.
    let pts = [
        v(0.25, 0.0, 0.25),
        v(0.75, 0.0, 0.25),
        v(0.75, 0.0, 0.75),
        v(0.25, 0.0, 0.75),
    ];
    let panel = mesh(&mut model, &pts, &[&[&[0, 1, 2, 3]]])[0];
    let before = model.clone();
    match run(&mut model, &mut undo, f, 1.0) {
        Err(PushPullError::Overlap(a, b)) => {
            assert!(a < b);
            assert!(a == panel || b == panel);
        }
        other => panic!("expected Overlap, got {other:?}"),
    }
    assert_unchanged(&model, &before, &undo);
    assert!(run(&mut model, &mut undo, f, -1.0).is_ok());
}

// -- undo --------------------------------------------------------------------------------

#[test]
fn an_extrude_is_one_undo_step() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let faces = cube_with_inset(&mut model);
    let before = model.clone();
    run(&mut model, &mut undo, faces[6], 0.5).unwrap();
    let after = ctx(&model).clone();
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("Push/Pull"));
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
    undo.redo(&mut model).unwrap();
    assert_eq!(ctx(&model), &after);
}

#[test]
fn a_slide_is_one_undo_step() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let faces = cube(&mut model);
    let before = model.clone();
    run(&mut model, &mut undo, faces[1], 2.0).unwrap();
    let after = ctx(&model).clone();
    assert_eq!(undo.undo_count(), 1);
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
    undo.redo(&mut model).unwrap();
    assert_eq!(ctx(&model), &after);
}

#[test]
fn faces_elsewhere_are_untouched() {
    let mut model = Model::new();
    let a = square(&mut model, 0.0, 0.0, 1.0);
    let b = square(&mut model, 5.0, 0.0, 1.0);
    let face_b = ctx(&model).face(b).unwrap().clone();
    let pts_b = face_points(ctx(&model), b);
    run(&mut model, &mut UndoStack::new(), a, 1.0).unwrap();
    assert_eq!(ctx(&model).face(b), Some(&face_b));
    assert_eq!(face_points(ctx(&model), b), pts_b);
}

#[test]
fn a_tray_floor_with_walls_in_front_extrudes_instead_of_sliding() {
    // Every edge of the floor is shared with an upright wall, but the walls stand on the
    // floor's front side, so none is a stretch edge.
    let mut model = Model::new();
    let pts = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
        v(0.0, 0.0, 1.0),
        v(1.0, 0.0, 1.0),
        v(1.0, 1.0, 1.0),
        v(0.0, 1.0, 1.0),
    ];
    let faces = mesh(
        &mut model,
        &pts,
        &[
            &[&[0, 1, 2, 3]],
            &[&[0, 1, 5, 4]],
            &[&[2, 3, 7, 6]],
            &[&[1, 2, 6, 5]],
            &[&[3, 0, 4, 7]],
        ],
    );
    let floor = faces[0];
    let mut undo = UndoStack::new();
    let before = model.clone();
    assert!(run(&mut model, &mut undo, floor, 0.5).is_err());
    assert_unchanged(&model, &before, &undo);
    let r = run(&mut model, &mut undo, floor, -0.5).unwrap();
    let c = ctx(&model);
    assert!(c.validate().is_empty(), "{:?}", c.validate());
    assert_eq!(r.sides.len(), 4);
    assert!(r.base.is_none());
    assert!(all_z(c, r.cap, -0.5));
    for &w in &faces[1..] {
        assert!(
            face_points(c, w).iter().all(|p| p.z >= -1e-9),
            "walls don't move"
        );
    }
}
