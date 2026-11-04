// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! `sweep_face` and `sweep_sections` (`docs/specs/m9-sweep-face.md`).

use drydock_kernel::curve;
use drydock_kernel::model::*;
use drydock_kernel::shapes;
use drydock_kernel::sweep::SweepPath;
use drydock_kernel::sweepface::*;
use drydock_kernel::topology::*;
use glam::DVec3;

mod pushpull_common;
use pushpull_common::*;
mod sweep_face_common;
use sweep_face_common::*;

const X: DVec3 = DVec3::X;
const Y: DVec3 = DVec3::Y;
const Z: DVec3 = DVec3::Z;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn assert_vec(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-9, "{a:?} != {b:?}");
}

fn assert_unchanged(model: &Model, before: &Model, undo: &UndoStack) {
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

/// A square of half-size `half` centred on `centre` in the plane x = centre.x, fronting +x
/// (`plus`) or -x.
fn square_x(model: &mut Model, centre: DVec3, half: f64, plus: bool) -> FaceId {
    let (u, w) = if plus { (Y, Z) } else { (Z, Y) };
    let pts = rect_points(centre, u, w, -half, half, -half, half);
    mesh(model, &pts, &[&[&[0, 1, 2, 3]]])[0]
}

/// A rectangle in the plane x = `x`: y from `y0` to `y1`, z from `z0` to `z1`, fronting +x.
fn rect_x(model: &mut Model, x: f64, y0: f64, y1: f64, z0: f64, z1: f64) -> FaceId {
    let pts = rect_points(v(x, 0.0, 0.0), Y, Z, y0, y1, z0, z1);
    mesh(model, &pts, &[&[&[0, 1, 2, 3]]])[0]
}

fn straight(len: f64) -> SweepPath {
    path(&[v(0.0, 0.0, 0.0), v(len, 0.0, 0.0)], false)
}

/// The 4 x 4 square on the ground, counter-clockwise seen from above.
fn square_ring() -> SweepPath {
    path(
        &[
            v(0.0, 0.0, 0.0),
            v(4.0, 0.0, 0.0),
            v(4.0, 4.0, 0.0),
            v(0.0, 4.0, 0.0),
        ],
        true,
    )
}

fn paint(model: &mut Model, f: FaceId) -> FaceMaterials {
    let mut edit = ModelEdit::new();
    let material = |name: &str| Material {
        name: name.into(),
        color: [1.0; 4],
        texture: None,
    };
    let a = model.add_material(material("a"), &mut edit);
    let b = model.add_material(material("b"), &mut edit);
    let m = FaceMaterials {
        front: Some(a),
        back: Some(b),
        ..Default::default()
    };
    model
        .set_face_materials(BodyId::Root, f, m, &mut edit)
        .unwrap();
    m
}

fn volume_of(c: &Context) -> f64 {
    let r = solid(c);
    assert!(r.is_clean(), "{r:?}");
    r.volume.unwrap()
}

// -- refusals ----------------------------------------------------------------------------

#[test]
fn unknown_body_and_face_are_refused() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut m, v(0.0, 0.0, 0.0), 0.5, false);
    let before = m.clone();
    let body = BodyId::Definition(DefinitionId(77));
    assert_eq!(
        sweep_face(&mut m, body, f, &straight(2.0), &mut undo, LABEL),
        Err(SweepError::MissingBody(body))
    );
    assert_eq!(
        sweep(&mut m, &mut undo, FaceId(999), &straight(2.0)),
        Err(SweepError::MissingFace(FaceId(999)))
    );
    assert_unchanged(&m, &before, &undo);
}

#[test]
fn bad_paths_are_refused() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut m, v(0.0, 0.0, 0.0), 0.5, false);
    let before = m.clone();
    let mut short_ids = straight(2.0);
    short_ids.vertices.pop();
    let bad = [
        path(&[v(0.0, 0.0, 0.0)], false),
        path(&[v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0)], true),
        path(&[v(0.0, 0.0, 0.0), v(f64::NAN, 0.0, 0.0)], false),
        path(&[v(0.0, 0.0, 0.0), v(f64::INFINITY, 0.0, 0.0)], false),
        path(
            &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 0.0, 0.0)],
            false,
        ),
        path(
            &[
                v(0.0, 0.0, 0.0),
                v(1.0, 0.0, 0.0),
                v(1.0, 1.0, 0.0),
                v(0.0, 0.0, 0.000001),
            ],
            true,
        ),
        short_ids,
    ];
    for p in &bad {
        assert_eq!(
            sweep(&mut m, &mut undo, f, p),
            Err(SweepError::BadPath),
            "{p:?}"
        );
        assert_eq!(
            sweep_sections(ctx(&m), f, p),
            Err(SweepError::BadPath),
            "{p:?}"
        );
    }
    assert_unchanged(&m, &before, &undo);
}

#[test]
fn a_profile_sharing_an_edge_is_refused() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let faces = cube(&mut m);
    let bottom = faces[0];
    let lowest = ctx(&m)
        .face(bottom)
        .unwrap()
        .outer
        .uses
        .iter()
        .map(|u| u.edge)
        .min()
        .unwrap();
    let before = m.clone();
    let p = path(&[v(0.5, 0.5, 0.0), v(0.5, 0.5, -2.0)], false);
    assert_eq!(
        sweep(&mut m, &mut undo, bottom, &p),
        Err(SweepError::ProfileShared(lowest))
    );
    assert_unchanged(&m, &before, &undo);
}

#[test]
fn a_path_doubling_back_has_no_mitre() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut m, v(0.0, 0.0, 0.0), 0.1, false);
    let before = m.clone();
    // Turns by about 179.4 degrees at the middle vertex.
    let p = path(
        &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 0.01, 0.0)],
        false,
    );
    assert_eq!(
        sweep(&mut m, &mut undo, f, &p),
        Err(SweepError::DegenerateMitre(VertexId(1001)))
    );
    assert_unchanged(&m, &before, &undo);
}

#[test]
fn a_profile_nearly_parallel_to_the_path_is_refused() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut m, v(0.0, 0.0, 0.0), 0.1, false);
    let before = m.clone();
    let along = path(&[v(0.0, 0.0, 0.0), v(0.0, 2.0, 0.0)], false);
    assert_eq!(
        sweep(&mut m, &mut undo, f, &along),
        Err(SweepError::ProfileParallel)
    );
    let a = 0.5_f64.to_radians();
    let shallow = path(
        &[v(0.0, 0.0, 0.0), v(2.0 * a.sin(), 2.0 * a.cos(), 0.0)],
        false,
    );
    assert_eq!(
        sweep(&mut m, &mut undo, f, &shallow),
        Err(SweepError::ProfileParallel)
    );
    assert_unchanged(&m, &before, &undo);

    // Two degrees is enough.
    let a = 2.0_f64.to_radians();
    let steep = path(
        &[v(0.0, 0.0, 0.0), v(2.0 * a.sin(), 2.0 * a.cos(), 0.0)],
        false,
    );
    sweep(&mut m, &mut undo, f, &steep).unwrap();
    assert!(close(volume_of(ctx(&m)), 0.04 * a.sin() * 2.0));
}

#[test]
fn a_profile_too_big_for_the_turn_folds() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut m, v(0.0, 0.0, 0.0), 0.5, false);
    let before = m.clone();
    let p = path(
        &[v(0.0, 0.0, 0.0), v(0.4, 0.0, 0.0), v(0.4, 1.0, 0.0)],
        false,
    );
    assert_eq!(
        sweep(&mut m, &mut undo, f, &p),
        Err(SweepError::Folded(VertexId(1001)))
    );
    assert_eq!(
        sweep_sections(ctx(&m), f, &p),
        Err(SweepError::Folded(VertexId(1001)))
    );
    assert_unchanged(&m, &before, &undo);
}

#[test]
fn a_profile_beyond_the_first_joint_folds() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut m, v(1.2, 0.0, 0.0), 0.01, false);
    let before = m.clone();
    let p = path(
        &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 3.0, 0.0)],
        false,
    );
    assert_eq!(
        sweep(&mut m, &mut undo, f, &p),
        Err(SweepError::Folded(VertexId(1001)))
    );
    assert_unchanged(&m, &before, &undo);
}

#[test]
fn a_ring_too_tight_for_the_profile_folds() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    // Inside the 1 x 1 ring, nearer its first side, reaching 0.7 in from it.
    let f = rect_x(&mut m, 0.5, 0.1, 0.7, 0.0, 0.1);
    let before = m.clone();
    let p = path(
        &[
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
        ],
        true,
    );
    assert_eq!(
        sweep(&mut m, &mut undo, f, &p),
        Err(SweepError::Folded(VertexId(1001)))
    );
    assert_unchanged(&m, &before, &undo);
}

#[test]
fn a_profile_away_from_a_closed_path_is_refused() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = rect_x(&mut m, 10.0, -0.2, -0.1, 0.0, 0.1);
    let before = m.clone();
    assert_eq!(
        sweep(&mut m, &mut undo, f, &square_ring()),
        Err(SweepError::NotOnPath)
    );
    assert_unchanged(&m, &before, &undo);
}

#[test]
fn a_profile_at_a_corner_must_lie_in_its_mitre_plane() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    // Square to the first side at the corner (0, 0, 0), not on the corner's mitre.
    let f = rect_x(&mut m, 0.0, -0.2, -0.1, 0.0, 0.1);
    let before = m.clone();
    assert_eq!(
        sweep(&mut m, &mut undo, f, &square_ring()),
        Err(SweepError::MitreMismatch(VertexId(1000)))
    );
    assert_unchanged(&m, &before, &undo);
}

#[test]
fn a_closed_path_that_is_not_flat_twists_the_profile() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = rect_x(&mut m, 2.0, -0.2, -0.1, 0.0, 0.1);
    let before = m.clone();
    let p = path(
        &[
            v(0.0, 0.0, 0.0),
            v(4.0, 0.0, 0.0),
            v(4.0, 4.0, 0.0),
            v(4.0, 4.0, 4.0),
        ],
        true,
    );
    assert_eq!(sweep(&mut m, &mut undo, f, &p), Err(SweepError::Twisted));
    assert_eq!(sweep_sections(ctx(&m), f, &p), Err(SweepError::Twisted));
    assert_unchanged(&m, &before, &undo);
}

#[test]
fn a_side_over_an_existing_face_is_refused_as_overlap() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let pts = rect_points(v(1.0, 0.0, 0.0), X, Y, -0.5, 0.5, -0.1, 0.1);
    let floor = mesh(&mut m, &pts, &[&[&[0, 1, 2, 3]]])[0];
    // Its top side runs along z = 0, over the floor.
    let f = rect_x(&mut m, 0.0, -0.2, 0.2, -0.1, 0.0);
    let before = m.clone();
    let r = sweep(&mut m, &mut undo, f, &straight(2.0));
    assert!(
        matches!(r, Err(SweepError::Overlap(a, _)) if a == floor),
        "{r:?}"
    );
    assert_unchanged(&m, &before, &undo);
}

#[test]
fn a_sweep_crossing_an_edge_is_refused_as_invalid() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut m, v(0.0, 0.0, 0.0), 0.1, false);
    let mut edit = ModelEdit::new();
    m.edit_geometry(BodyId::Root, &mut edit, |c, log| {
        let a = c.insert_vertex(v(1.0, 0.1, -1.0), log);
        let b = c.insert_vertex(v(1.0, 0.1, 1.0), log);
        c.insert_edge(a, b, log).unwrap();
    })
    .unwrap();
    let before = m.clone();
    let r = sweep(&mut m, &mut undo, f, &straight(2.0));
    assert!(
        matches!(r, Err(SweepError::Invalid(Violation::EdgesIntersect(..)))),
        "{r:?}"
    );
    assert_unchanged(&m, &before, &undo);
}

// -- open paths --------------------------------------------------------------------------

#[test]
fn a_straight_path_makes_a_box() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut m, v(0.0, 0.0, 0.0), 0.5, false);
    let s = sweep(&mut m, &mut undo, f, &straight(2.0)).unwrap();
    let c = ctx(&m);
    assert_eq!(s.start, Some(f));
    let end = s.end.unwrap();
    assert_eq!(s.sides.len(), 4);
    assert!(s.sides.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(c.face_count(), 6);
    assert_eq!(c.vertex_count(), 8);
    assert_vec(unit_normal(c, f), -X);
    assert_vec(unit_normal(c, end), X);
    assert!(face_points(c, end).iter().all(|p| close(p.x, 2.0)));
    assert!(close(volume_of(c), 2.0));
    assert!(normals_outward(c));
    assert!(c.validate().is_empty());
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some(LABEL));
}

#[test]
fn a_profile_fronting_along_the_path_is_turned() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut m, v(0.0, 0.0, 0.0), 0.5, true);
    let corners = face_points(ctx(&m), f);
    let s = sweep(&mut m, &mut undo, f, &straight(2.0)).unwrap();
    let c = ctx(&m);
    assert_eq!(s.start, Some(f));
    assert_vec(unit_normal(c, f), -X);
    let mut turned = corners.clone();
    turned.reverse();
    assert!(same_cycle(&face_points(c, f), &turned, 1e-12));
    assert!(close(volume_of(c), 2.0));
    assert!(normals_outward(c));
}

#[test]
fn the_end_cap_takes_the_profiles_materials() {
    for plus in [false, true] {
        let mut m = Model::new();
        let mut undo = UndoStack::new();
        let f = square_x(&mut m, v(0.0, 0.0, 0.0), 0.5, plus);
        let mats = paint(&mut m, f);
        let s = sweep(&mut m, &mut undo, f, &straight(2.0)).unwrap();
        let body = m.root();
        assert_eq!(body.face_materials(f), mats, "plus {plus}");
        assert_eq!(body.face_materials(s.end.unwrap()), mats, "plus {plus}");
        for side in &s.sides {
            assert_eq!(body.face_materials(*side), FaceMaterials::default());
        }
    }
}

#[test]
fn the_end_nearer_the_profile_starts_the_sweep() {
    let mut forward = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut forward, v(0.0, 0.0, 0.0), 0.5, false);
    sweep(&mut forward, &mut undo, f, &straight(2.0)).unwrap();

    let mut backward = Model::new();
    let g = square_x(&mut backward, v(0.0, 0.0, 0.0), 0.5, false);
    let p = reversed(&straight(2.0));
    assert_vec(p.points[0], v(2.0, 0.0, 0.0));
    let s = sweep(&mut backward, &mut undo, g, &p).unwrap();
    assert_eq!(s.start, Some(g));
    assert!(same_faces(ctx(&forward), ctx(&backward), 1e-9));
}

#[test]
fn a_profile_off_the_path_end_sweeps_from_its_own_plane() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    // Past the far end of the path: that end is nearer, so the sweep runs back to x = 0.
    let f = square_x(&mut m, v(2.5, 0.0, 0.0), 0.5, false);
    let s = sweep(&mut m, &mut undo, f, &straight(2.0)).unwrap();
    let c = ctx(&m);
    assert_vec(unit_normal(c, f), X);
    let end = s.end.unwrap();
    assert_vec(unit_normal(c, end), -X);
    assert!(face_points(c, end).iter().all(|p| close(p.x, 0.0)));
    assert!(close(volume_of(c), 2.5));
}

#[test]
fn an_l_path_mitres_the_corner() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut m, v(0.0, 0.0, 0.0), 0.1, false);
    let profile = face_points(ctx(&m), f);
    let p = path(
        &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0)],
        false,
    );
    let sections = sweep_sections(ctx(&m), f, &p).unwrap();
    assert_eq!(sections.len(), 3);
    for s in &sections {
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].len(), 4);
    }
    for (i, q) in profile.iter().enumerate() {
        assert_vec(sections[0][0][i], *q);
        // The mitre plane x + y = 1 at the corner.
        assert_vec(sections[1][0][i], v(1.0 - q.y, q.y, q.z));
        // Square to the last segment at its end.
        assert_vec(sections[2][0][i], v(1.0 - q.y, 1.0, q.z));
    }
    assert_eq!(m, {
        let mut fresh = Model::new();
        square_x(&mut fresh, v(0.0, 0.0, 0.0), 0.1, false);
        fresh
    });

    let s = sweep(&mut m, &mut undo, f, &p).unwrap();
    let c = ctx(&m);
    // The top and bottom sides of the two legs share a plane and merge into L shapes.
    assert_eq!(s.sides.len(), 6);
    assert_eq!(c.face_count(), 8);
    let flat: Vec<FaceId> = s
        .sides
        .iter()
        .copied()
        .filter(|f| unit_normal(c, *f).z.abs() > 0.5)
        .collect();
    assert_eq!(flat.len(), 2);
    assert!(flat.iter().all(|f| face_points(c, *f).len() == 6));
    assert!(close(volume_of(c), 0.04 * 2.0));
    let end = s.end.unwrap();
    assert_vec(unit_normal(c, end), Y);
    assert!(c.validate().is_empty());
}

#[test]
fn an_oblique_profile_keeps_its_slant_and_ends_square() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let a = 30.0_f64.to_radians();
    let n = -v(a.cos(), a.sin(), 0.0);
    let (u, w) = basis(n);
    let pts = rect_points(v(0.0, 0.0, 0.0), u, w, -0.5, 0.5, -0.5, 0.5);
    let f = mesh(&mut m, &pts, &[&[&[0, 1, 2, 3]]])[0];
    let s = sweep(&mut m, &mut undo, f, &straight(2.0)).unwrap();
    let c = ctx(&m);
    assert_vec(unit_normal(c, f), n);
    let end = s.end.unwrap();
    assert_vec(unit_normal(c, end), X);
    assert!(face_points(c, end).iter().all(|p| close(p.x, 2.0)));
    assert!(close(volume_of(c), a.cos() * 2.0));
}

#[test]
fn a_profile_with_a_hole_makes_a_tube() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let o = v(0.0, 0.0, 0.0);
    let mut pts = rect_points(o, Z, Y, -0.5, 0.5, -0.5, 0.5).to_vec();
    pts.extend(rect_points(o, Z, Y, -0.25, 0.25, -0.25, 0.25));
    let f = mesh(&mut m, &pts, &[&[&[0, 1, 2, 3], &[7, 6, 5, 4]]])[0];
    let s = sweep(&mut m, &mut undo, f, &straight(2.0)).unwrap();
    let c = ctx(&m);
    assert_eq!(s.sides.len(), 8);
    assert_eq!(c.face_count(), 10);
    let end = s.end.unwrap();
    assert_eq!(c.face(end).unwrap().inner.len(), 1);
    assert!(close(volume_of(c), 0.75 * 2.0));
}

#[test]
fn sides_across_a_straight_joint_merge() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut m, v(0.0, 0.0, 0.0), 0.5, false);
    let p = path(
        &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(2.0, 0.0, 0.0)],
        false,
    );
    let s = sweep(&mut m, &mut undo, f, &p).unwrap();
    let c = ctx(&m);
    assert_eq!(s.sides.len(), 4);
    assert!(s.sides.iter().all(|f| c.face(*f).is_some()));
    assert_eq!(c.face_count(), 6);
    // The joint's vertices stay, each between two collinear edges.
    assert_eq!(c.vertex_count(), 12);
    assert_eq!(c.edge_count(), 16);
    assert!(close(volume_of(c), 2.0));
    assert!(c.validate().is_empty());
    assert_eq!(undo.undo_count(), 1);
}

#[test]
fn sides_from_collinear_profile_edges_merge() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let r = rect_points(v(0.0, 0.0, 0.0), Z, Y, -0.5, 0.5, -0.5, 0.5);
    let pts = [r[0], (r[0] + r[1]) * 0.5, r[1], r[2], r[3]];
    let f = mesh(&mut m, &pts, &[&[&[0, 1, 2, 3, 4]]])[0];
    let s = sweep(&mut m, &mut undo, f, &straight(2.0)).unwrap();
    let c = ctx(&m);
    assert_eq!(s.sides.len(), 4);
    assert_eq!(c.face_count(), 6);
    assert_eq!(c.vertex_count(), 10);
    assert!(close(volume_of(c), 2.0));
}

#[test]
fn a_curved_profile_keeps_its_curve_on_the_start_cap() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let pts = shapes::circle(v(0.0, 0.0, 0.0), -X, Y, 0.3, 12);
    let idx: Vec<usize> = (0..12).collect();
    let f = mesh(&mut m, &pts, &[&[&idx]])[0];
    let mut edit = ModelEdit::new();
    let id = m
        .edit_geometry(BodyId::Root, &mut edit, |c, log| {
            curve::add_curve(c, &pts, true, log)
        })
        .unwrap()
        .unwrap();
    let s = sweep(&mut m, &mut undo, f, &straight(1.0)).unwrap();
    let c = ctx(&m);
    assert_eq!(s.sides.len(), 12);
    // The second is the end cap's (m9-sweep-curves).
    assert_eq!(c.curve_count(), 2);
    let cap: Vec<EdgeId> = c
        .face(f)
        .unwrap()
        .outer
        .uses
        .iter()
        .map(|u| u.edge)
        .collect();
    let steps = curve::curve_edges(c, id);
    assert_eq!(steps.len(), 12);
    assert!(steps.iter().all(|e| cap.contains(e)));
    assert!(solid(c).is_clean());
}

#[test]
fn a_sweep_is_one_undo_step() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = square_x(&mut m, v(0.0, 0.0, 0.0), 0.5, true);
    paint(&mut m, f);
    let before = m.clone();
    let p = path(
        &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0)],
        false,
    );
    sweep(&mut m, &mut undo, f, &p).unwrap();
    let after = m.clone();
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo(&mut m).unwrap().as_deref(), Some(LABEL));
    assert_eq!(m, before);
    undo.redo(&mut m).unwrap();
    assert_eq!(m, after);
}

// -- closed paths ------------------------------------------------------------------------

/// A 0.1 x 0.1 profile in the mitre plane of the ring's first corner, 0.1 to 0.2 outside the
/// ring, on the ground.
fn corner_profile(m: &mut Model) -> FaceId {
    let pts = [
        v(-0.1, -0.1, 0.0),
        v(-0.2, -0.2, 0.0),
        v(-0.2, -0.2, 0.1),
        v(-0.1, -0.1, 0.1),
    ];
    mesh(m, &pts, &[&[&[0, 1, 2, 3]]])[0]
}

#[test]
fn a_ring_from_a_profile_at_a_corner() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = corner_profile(&mut m);
    let corners = ctx(&m).loop_vertices(&ctx(&m).face(f).unwrap().outer);
    let s = sweep(&mut m, &mut undo, f, &square_ring()).unwrap();
    let c = ctx(&m);
    assert_eq!(s.start, None);
    assert_eq!(s.end, None);
    assert!(c.face(f).is_none());
    // Four walls per leg; the top and bottom pieces share planes and merge into two flat
    // rings, each a face with a hole, and the mitre edges left inside them go.
    assert_eq!(s.sides.len(), 10);
    assert_eq!(c.face_count(), 10);
    assert_eq!(c.edge_count(), 24);
    for f in &s.sides {
        let flat = unit_normal(c, *f).z.abs() > 0.5;
        assert_eq!(c.face(*f).unwrap().inner.len(), usize::from(flat));
    }
    // The profile's corners are the ring's section at that corner.
    assert!(corners.iter().all(|v| c.vertex(*v).is_some()));
    assert_eq!(c.vertex_count(), 16);
    // 0.01 square, its centre 0.15 outside a ring 16 round: 0.01 * (16 + 8 * 0.15).
    assert!(close(volume_of(c), 0.172));
    assert!(c.validate().is_empty());
}

#[test]
fn a_ring_from_a_profile_part_way_along_a_side() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = rect_x(&mut m, 2.0, -0.2, -0.1, 0.0, 0.1);
    let corners = ctx(&m).loop_vertices(&ctx(&m).face(f).unwrap().outer);
    let s = sweep(&mut m, &mut undo, f, &square_ring()).unwrap();
    let c = ctx(&m);
    assert_eq!((s.start, s.end), (None, None));
    assert_eq!(s.sides.len(), 10);
    // The profile, its edges and its corners are gone; the ring has a section at each corner.
    assert!(c.face(f).is_none());
    assert!(corners.iter().all(|v| c.vertex(*v).is_none()));
    assert_eq!(c.face_count(), 10);
    assert_eq!(c.vertex_count(), 16);
    assert_eq!(c.edge_count(), 24);
    assert!(close(volume_of(c), 0.172));
}

#[test]
fn ring_sections_follow_the_path_from_the_profile() {
    let mut m = Model::new();
    let f = rect_x(&mut m, 2.0, -0.2, -0.1, 0.0, 0.1);
    let profile = face_points(ctx(&m), f);
    let sections = sweep_sections(ctx(&m), f, &square_ring()).unwrap();
    assert_eq!(sections.len(), 4);
    // Section 0 is on the mitre at (4, 0, 0), x + y = 4; then round the ring.
    for (i, q) in profile.iter().enumerate() {
        assert_vec(sections[0][0][i], v(4.0 - q.y, q.y, q.z));
        assert_vec(sections[1][0][i], v(4.0 - q.y, 4.0 - q.y, q.z));
        assert_vec(sections[2][0][i], v(q.y, 4.0 - q.y, q.z));
        assert_vec(sections[3][0][i], v(q.y, q.y, q.z));
    }
}

#[test]
fn a_ring_is_the_same_either_way_round() {
    let mut a = Model::new();
    let mut undo = UndoStack::new();
    let f = rect_x(&mut a, 2.0, -0.2, -0.1, 0.0, 0.1);
    sweep(&mut a, &mut undo, f, &square_ring()).unwrap();
    let mut b = Model::new();
    let g = rect_x(&mut b, 2.0, -0.2, -0.1, 0.0, 0.1);
    sweep(&mut b, &mut undo, g, &reversed(&square_ring())).unwrap();
    assert!(same_faces(ctx(&a), ctx(&b), 1e-9));
}

#[test]
fn a_ring_drops_the_profiles_materials_and_undo_brings_them_back() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let f = rect_x(&mut m, 2.0, -0.2, -0.1, 0.0, 0.1);
    let mats = paint(&mut m, f);
    let before = m.clone();
    sweep(&mut m, &mut undo, f, &square_ring()).unwrap();
    assert_eq!(m.root().face_materials(f), FaceMaterials::default());
    assert!(
        m.root()
            .face_material_entries()
            .all(|(_, fm)| fm == FaceMaterials::default())
    );
    undo.undo(&mut m).unwrap();
    assert_eq!(m, before);
    assert_eq!(m.root().face_materials(f), mats);
}

#[test]
fn a_moulding_round_a_face_reuses_the_faces_edges() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    // A 2 x 1 floor face with a vertex part way along its first side, and a 0.1 x 0.1
    // profile standing on that vertex, outside the floor, square to the side.
    let pts = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(2.0, 0.0, 0.0),
        v(2.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
        v(1.0, -0.1, 0.0),
        v(1.0, -0.1, 0.1),
        v(1.0, 0.0, 0.1),
    ];
    let ids = mesh(&mut m, &pts, &[&[&[0, 1, 2, 3, 4]], &[&[1, 5, 6, 7]]]);
    let (floor, f) = (ids[0], ids[1]);
    let c = ctx(&m);
    let floor_face = c.face(floor).unwrap().clone();
    let p = SweepPath {
        vertices: c.loop_vertices(&floor_face.outer),
        points: c.loop_points(&floor_face.outer),
        edges: floor_face.outer.uses.iter().map(|u| u.edge).collect(),
        closed: true,
    };
    let s = sweep(&mut m, &mut undo, f, &p).unwrap();
    let c = ctx(&m);
    assert!(c.validate().is_empty(), "{:?}", c.validate());
    assert_eq!(c.face(floor), Some(&floor_face));
    // Five legs of four sides. The two walls merge across the straight joint; the top and
    // bottom pieces each merge into one flat ring, the bottom one round the floor.
    assert_eq!(s.sides.len(), 10);
    assert_eq!(c.face_count(), 11);
    // The inner bottom corner of every section is the floor's own vertex there.
    assert_eq!(c.vertex_count(), 5 + 3 + 4 * 3);
    for e in &p.edges {
        assert_eq!(c.faces_of_edge(*e).len(), 3, "edge {e:?}");
    }
}
