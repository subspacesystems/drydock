// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Push/pull healing (`docs/specs/m3-pushpull-merge.md`).

use drydock_kernel::model::*;
use drydock_kernel::pushpull::*;
use drydock_kernel::topology::*;
use glam::DVec3;

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

/// A unit cube whose top is split into a small rectangle touching the front edge
/// (x 0.25..0.75, y 0..0.5) and the rest. Returns
/// `[bottom, rest, front, back, right, left, rect]`.
fn cube_with_notch_rect(model: &mut Model) -> Vec<FaceId> {
    let pts = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
        v(0.0, 0.0, 1.0),
        v(1.0, 0.0, 1.0),
        v(1.0, 1.0, 1.0),
        v(0.0, 1.0, 1.0),
        v(0.25, 0.0, 1.0),
        v(0.75, 0.0, 1.0),
        v(0.25, 0.5, 1.0),
        v(0.75, 0.5, 1.0),
    ];
    mesh(
        model,
        &pts,
        &[
            &[&[0, 3, 2, 1]],
            &[&[4, 8, 10, 11, 9, 5, 6, 7]],
            &[&[0, 1, 5, 9, 8, 4]],
            &[&[2, 3, 7, 6]],
            &[&[1, 2, 6, 5]],
            &[&[3, 0, 4, 7]],
            &[&[8, 9, 11, 10]],
        ],
    )
}

fn vertex_at(c: &Context, p: DVec3) -> VertexId {
    c.find_vertex(p, 1e-9).expect("vertex")
}

#[test]
fn a_side_merges_into_the_coplanar_wall() {
    let mut model = Model::new();
    let faces = cube_with_notch_rect(&mut model);
    let (front, rect) = (faces[2], faces[6]);
    let r = run(&mut model, &mut UndoStack::new(), rect, 0.5).unwrap();
    let c = ctx(&model);
    assert!(c.validate().is_empty(), "{:?}", c.validate());
    // bottom, rest, front (merged), back, right, left, cap, three more sides.
    assert_eq!(c.face_count(), 10);
    assert!(closed(c));
    assert!((volume(c) - 1.125).abs() < 1e-9);
    // The edge between the rectangle's front corners is gone.
    let a = vertex_at(c, v(0.25, 0.0, 1.0));
    let b = vertex_at(c, v(0.75, 0.0, 1.0));
    assert!(c.edge_between(a, b).is_none());
    // The wall survived with its id and grew a bump: 0,1,5,9,9',8',8,4.
    let wall = c.face(front).expect("the wall keeps its id");
    assert_eq!(wall.inner.len(), 0);
    let pts = face_points(c, front);
    assert_eq!(pts.len(), 8);
    assert!(pts.iter().any(|p| (*p - v(0.25, 0.0, 1.5)).length() < 1e-9));
    assert!(pts.iter().any(|p| (*p - v(0.75, 0.0, 1.5)).length() < 1e-9));
    assert!(normal(c, front).distance(-DVec3::Y) < 1e-9);
    // Sides: one entry per rectangle edge; the merged one reports the wall.
    assert_eq!(r.sides.len(), 4);
    assert!(r.sides.iter().all(|&s| c.face(s).is_some()));
    assert!(r.sides.contains(&front));
    assert!(r.base.is_none());
    assert!(c.face(r.cap).is_some());
    assert!(normals_outward_per_face(c));
}

/// Every face of this shape fronts away from the solid: its normal points away from the
/// centroid of the whole solid's vertices at least roughly (cube plus a small bump).
fn normals_outward_per_face(c: &Context) -> bool {
    c.faces().all(|(f, _)| {
        let n = normal(c, f);
        let p = centroid(&face_points(c, f));
        n.dot(p - v(0.5, 0.5, 0.5)) > -1e-9
    })
}

#[test]
fn the_merge_is_one_undo_step() {
    let mut model = Model::new();
    let faces = cube_with_notch_rect(&mut model);
    let before = model.clone();
    let mut undo = UndoStack::new();
    run(&mut model, &mut undo, faces[6], 0.5).unwrap();
    assert_eq!(undo.undo_count(), 1);
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
    undo.redo(&mut model).unwrap();
    assert_eq!(ctx(&model).face_count(), 10);
    assert!(ctx(&model).validate().is_empty());
}

#[test]
fn pushing_the_rectangle_in_stays_refused() {
    // The pocket would open through the front wall: its front side faces the opposite way to
    // the wall, so nothing merges and the sides overlap the wall. Not handled yet.
    let mut model = Model::new();
    let faces = cube_with_notch_rect(&mut model);
    let before = model.clone();
    let mut undo = UndoStack::new();
    assert!(matches!(
        run(&mut model, &mut undo, faces[6], -0.5),
        Err(PushPullError::Overlap(..))
    ));
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn a_side_over_a_face_of_another_direction_does_not_merge() {
    // The inset's sides stand on the ring, which is not coplanar with them.
    let mut model = Model::new();
    let faces = cube_with_inset(&mut model);
    let r = run(&mut model, &mut UndoStack::new(), faces[6], 0.5).unwrap();
    let c = ctx(&model);
    assert_eq!(c.face_count(), 11);
    assert_eq!(r.sides.len(), 4);
    assert!(r.sides.iter().all(|&s| c.face(s).is_some()));
    let mut ids = r.sides.clone();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 4);
}

#[test]
fn pushing_an_inset_through_a_solid_punches_a_hole() {
    let mut model = Model::new();
    let faces = cube_with_inset(&mut model);
    let (bottom, ring, inner) = (faces[0], faces[1], faces[6]);
    let r = run(&mut model, &mut UndoStack::new(), inner, -1.0).unwrap();
    let c = ctx(&model);
    assert!(c.validate().is_empty(), "{:?}", c.validate());
    // bottom (ring), top ring, four walls, four hole walls.
    assert_eq!(c.face_count(), 10);
    assert!(closed(c));
    assert!((volume(c) - 0.75).abs() < 1e-9);
    assert!(c.face(inner).is_none());
    assert!(c.face(ring).is_some());
    assert_eq!(r.cap, bottom, "the cap is the face that now holds the hole");
    let face = c.face(bottom).unwrap();
    assert_eq!(face.inner.len(), 1);
    assert_eq!(face.inner[0].uses.len(), 4);
    assert!(all_z_of(c, bottom, 0.0));
    assert!(normal(c, bottom).distance(-DVec3::Z) < 1e-9);
    assert_eq!(r.sides.len(), 4);
    assert!(r.base.is_none());
    let axis = v(0.5, 0.5, 0.0);
    for &s in &r.sides {
        let towards = (axis - centroid(&face_points(c, s))) * DVec3::new(1.0, 1.0, 0.0);
        assert!(normal(c, s).dot(towards) > 0.0, "hole walls face in");
    }
}

fn all_z_of(c: &Context, f: FaceId, z: f64) -> bool {
    c.loop_points(&c.face(f).unwrap().outer)
        .iter()
        .all(|p| (p.z - z).abs() < 1e-9)
}

#[test]
fn the_hole_is_one_undo_step() {
    let mut model = Model::new();
    let faces = cube_with_inset(&mut model);
    let before = model.clone();
    let mut undo = UndoStack::new();
    run(&mut model, &mut undo, faces[6], -1.0).unwrap();
    assert_eq!(undo.undo_count(), 1);
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
    undo.redo(&mut model).unwrap();
    assert_eq!(ctx(&model).face_count(), 10);
}

#[test]
fn a_pocket_that_stops_short_is_still_a_pocket() {
    let mut model = Model::new();
    let faces = cube_with_inset(&mut model);
    run(&mut model, &mut UndoStack::new(), faces[6], -0.75).unwrap();
    let c = ctx(&model);
    assert_eq!(c.face_count(), 11);
    assert!(c.face(faces[0]).unwrap().inner.is_empty());
}
