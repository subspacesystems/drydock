// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Offset a face in its plane (`docs/specs/m9-offset-face.md`).

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::{PI, TAU};

use drydock_kernel::faceoffset::*;
use drydock_kernel::geom;
use drydock_kernel::model::*;
use drydock_kernel::pushpull::push_pull;
use drydock_kernel::shapes;
use drydock_kernel::tools::*;
use drydock_kernel::topology::*;
use drydock_kernel::units::Unit;
use glam::{DMat4, DVec3};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn xy(pts: &[(f64, f64)]) -> Vec<DVec3> {
    pts.iter().map(|&(x, y)| v(x, y, 0.0)).collect()
}

#[track_caller]
fn assert_points(got: &[DVec3], want: &[DVec3]) {
    assert_eq!(got.len(), want.len(), "{got:?} != {want:?}");
    for (a, b) in got.iter().zip(want) {
        assert!(a.distance(*b) < 1e-9, "{got:?} != {want:?}");
    }
}

#[track_caller]
fn assert_near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

/// Add faces to `body`. Each face is a list of loops (outer first, then holes), each loop a
/// list of indexes into `points`, counter-clockwise seen from the front for the outer loop and
/// clockwise for holes. Edges are shared between faces. Returns the face ids in order.
fn mesh_in(
    model: &mut Model,
    body: BodyId,
    points: &[DVec3],
    faces: &[&[&[usize]]],
) -> Vec<FaceId> {
    let mut edit = ModelEdit::new();
    let ids = model
        .edit_geometry(body, &mut edit, |ctx, log| {
            let vs: Vec<VertexId> = points.iter().map(|&p| ctx.insert_vertex(p, log)).collect();
            let mut edges: BTreeMap<(usize, usize), EdgeId> = BTreeMap::new();
            let mut out = Vec::new();
            for face in faces {
                let mut loops = Vec::new();
                for l in face.iter() {
                    let mut uses = Vec::new();
                    for i in 0..l.len() {
                        let (a, b) = (l[i], l[(i + 1) % l.len()]);
                        let key = (a.min(b), a.max(b));
                        let e = *edges
                            .entry(key)
                            .or_insert_with(|| ctx.insert_edge(vs[key.0], vs[key.1], log).unwrap());
                        uses.push(if a < b {
                            EdgeUse::forward(e)
                        } else {
                            EdgeUse::backward(e)
                        });
                    }
                    loops.push(Loop { uses });
                }
                let outer = loops.remove(0);
                out.push(
                    ctx.insert_face(
                        Face {
                            outer,
                            inner: loops,
                        },
                        log,
                    )
                    .unwrap(),
                );
            }
            out
        })
        .unwrap();
    let violations = model.body(body).unwrap().geometry().validate();
    assert!(violations.is_empty(), "{violations:?}");
    ids
}

fn mesh(model: &mut Model, points: &[DVec3], faces: &[&[&[usize]]]) -> Vec<FaceId> {
    mesh_in(model, BodyId::Root, points, faces)
}

/// One face on the ground through `pts` (counter-clockwise from +z), fronting +z.
fn polygon(model: &mut Model, pts: &[DVec3]) -> FaceId {
    let idx: Vec<usize> = (0..pts.len()).collect();
    mesh(model, pts, &[&[&idx]])[0]
}

fn square(model: &mut Model, lo: f64, hi: f64) -> FaceId {
    polygon(model, &xy(&[(lo, lo), (hi, lo), (hi, hi), (lo, hi)]))
}

/// A square `lo..hi` with a square hole `hlo..hhi`, fronting +z.
fn square_with_hole(model: &mut Model, lo: f64, hi: f64, hlo: f64, hhi: f64) -> FaceId {
    let pts = xy(&[
        (lo, lo),
        (hi, lo),
        (hi, hi),
        (lo, hi),
        (hlo, hlo),
        (hhi, hlo),
        (hhi, hhi),
        (hlo, hhi),
    ]);
    mesh(model, &pts, &[&[&[0, 1, 2, 3], &[4, 7, 6, 5]]])[0]
}

fn geo(model: &Model) -> &Context {
    model.root().geometry()
}

fn outer_points(c: &Context, f: FaceId) -> Vec<DVec3> {
    c.loop_points(&c.face(f).unwrap().outer)
}

fn loop_area(pts: &[DVec3]) -> f64 {
    geom::area_vector(pts).length()
}

/// Area of a face: its outer loop less its holes.
fn face_area(c: &Context, f: FaceId) -> f64 {
    let face = c.face(f).unwrap();
    loop_area(&c.loop_points(&face.outer))
        - face
            .inner
            .iter()
            .map(|l| loop_area(&c.loop_points(l)))
            .sum::<f64>()
}

fn normal(c: &Context, f: FaceId) -> DVec3 {
    c.face_plane(f).unwrap().normal
}

fn edge_set(l: &Loop) -> BTreeSet<EdgeId> {
    l.uses.iter().map(|u| u.edge).collect()
}

fn run(model: &mut Model, face: FaceId, d: f64) -> Result<FaceOffset, FaceOffsetError> {
    let mut undo = UndoStack::new();
    offset_face(model, BodyId::Root, face, d, &mut undo, "Offset")
}

/// The refusal leaves the model and the undo stack as they were.
#[track_caller]
fn refused(model: &mut Model, face: FaceId, d: f64) -> FaceOffsetError {
    let before = model.clone();
    let mut undo = UndoStack::new();
    let err = offset_face(model, BodyId::Root, face, d, &mut undo, "Offset").unwrap_err();
    assert_eq!(undo.undo_count(), 0);
    assert!(*model == before, "a refused offset changed the model");
    err
}

// ---------------------------------------------------------------------------------------
// The offset loops (no change to the model)
// ---------------------------------------------------------------------------------------

#[test]
fn square_inward_points() {
    let mut m = Model::new();
    let f = square(&mut m, 0.0, 2.0);
    let loops = offset_loops(geo(&m), f, -0.5).unwrap();
    assert_eq!(loops.len(), 1);
    assert_points(
        &loops[0],
        &xy(&[(0.5, 0.5), (1.5, 0.5), (1.5, 1.5), (0.5, 1.5)]),
    );
}

#[test]
fn square_outward_points() {
    let mut m = Model::new();
    let f = square(&mut m, 0.0, 2.0);
    let loops = offset_loops(geo(&m), f, 0.5).unwrap();
    assert_points(
        &loops[0],
        &xy(&[(-0.5, -0.5), (2.5, -0.5), (2.5, 2.5), (-0.5, 2.5)]),
    );
}

#[test]
fn offset_loops_changes_nothing() {
    let mut m = Model::new();
    let f = square(&mut m, 0.0, 2.0);
    let before = m.clone();
    offset_loops(geo(&m), f, -0.5).unwrap();
    offset_loops(geo(&m), f, 0.5).unwrap();
    assert!(m == before);
}

#[test]
fn l_shape_mitres_both_ways() {
    let mut m = Model::new();
    let f = polygon(
        &mut m,
        &xy(&[
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 1.0),
            (1.0, 1.0),
            (1.0, 2.0),
            (0.0, 2.0),
        ]),
    );
    let inward = offset_loops(geo(&m), f, -0.25).unwrap();
    assert_points(
        &inward[0],
        &xy(&[
            (0.25, 0.25),
            (1.75, 0.25),
            (1.75, 0.75),
            (0.75, 0.75),
            (0.75, 1.75),
            (0.25, 1.75),
        ]),
    );
    let outward = offset_loops(geo(&m), f, 0.25).unwrap();
    assert_points(
        &outward[0],
        &xy(&[
            (-0.25, -0.25),
            (2.25, -0.25),
            (2.25, 1.25),
            (1.25, 1.25),
            (1.25, 2.25),
            (-0.25, 2.25),
        ]),
    );
}

#[test]
fn collinear_vertex_stays_one_point() {
    let mut m = Model::new();
    let f = polygon(
        &mut m,
        &xy(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]),
    );
    let loops = offset_loops(geo(&m), f, 0.5).unwrap();
    assert_points(
        &loops[0],
        &xy(&[
            (-0.5, -0.5),
            (1.0, -0.5),
            (2.5, -0.5),
            (2.5, 2.5),
            (-0.5, 2.5),
        ]),
    );
}

#[test]
fn holes_move_the_other_way() {
    let mut m = Model::new();
    let f = square_with_hole(&mut m, 0.0, 4.0, 1.0, 3.0);
    let out = offset_loops(geo(&m), f, 0.5).unwrap();
    assert_eq!(out.len(), 2);
    assert_points(
        &out[0],
        &xy(&[(-0.5, -0.5), (4.5, -0.5), (4.5, 4.5), (-0.5, 4.5)]),
    );
    // The hole loop keeps its own direction and first vertex, and shrinks.
    assert_points(
        &out[1],
        &xy(&[(1.5, 1.5), (1.5, 2.5), (2.5, 2.5), (2.5, 1.5)]),
    );
    let inward = offset_loops(geo(&m), f, -0.25).unwrap();
    assert_points(
        &inward[0],
        &xy(&[(0.25, 0.25), (3.75, 0.25), (3.75, 3.75), (0.25, 3.75)]),
    );
    assert_points(
        &inward[1],
        &xy(&[(0.75, 0.75), (0.75, 3.25), (3.25, 3.25), (3.25, 0.75)]),
    );
}

#[test]
fn a_back_facing_face_offsets_the_same() {
    // The same square, fronting -z (outer loop clockwise seen from +z).
    let mut m = Model::new();
    let f = polygon(
        &mut m,
        &xy(&[(0.0, 0.0), (0.0, 2.0), (2.0, 2.0), (2.0, 0.0)]),
    );
    let loops = offset_loops(geo(&m), f, 0.5).unwrap();
    assert_points(
        &loops[0],
        &xy(&[(-0.5, -0.5), (-0.5, 2.5), (2.5, 2.5), (2.5, -0.5)]),
    );
}

#[test]
fn tilted_face_offsets_in_its_plane() {
    // A 2 x 2 square in a plane tilted 30 degrees about x, then 40 about z, off the origin.
    let rot = DMat4::from_translation(v(3.0, -1.0, 2.0))
        * DMat4::from_rotation_z(40f64.to_radians())
        * DMat4::from_rotation_x(30f64.to_radians());
    let flat = xy(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
    let pts: Vec<DVec3> = flat.iter().map(|p| rot.transform_point3(*p)).collect();
    let mut m = Model::new();
    let f = polygon(&mut m, &pts);
    let n = normal(geo(&m), f);
    for d in [-0.4, 0.3] {
        let got = offset_loops(geo(&m), f, d).unwrap();
        let want: Vec<DVec3> = xy(&[(-d, -d), (2.0 + d, -d), (2.0 + d, 2.0 + d), (-d, 2.0 + d)])
            .into_iter()
            .map(|p| rot.transform_point3(p))
            .collect();
        assert_points(&got[0], &want);
        for p in &got[0] {
            assert!((*p - pts[0]).dot(n).abs() < 1e-9);
        }
    }
}

/// A thin triangle pointing -x with its tip at the origin and the given tip angle.
fn spike(model: &mut Model, tip_deg: f64) -> (FaceId, [DVec3; 3]) {
    let h = 10.0 * (tip_deg / 2.0).to_radians().tan();
    let pts = [v(0.0, 0.0, 0.0), v(10.0, -h, 0.0), v(10.0, h, 0.0)];
    (polygon(model, &pts), pts)
}

#[test]
fn sharp_corner_is_bevelled_where_the_loop_opens() {
    let mut m = Model::new();
    let (f, p) = spike(&mut m, 20.0);
    let d = 0.1;
    let out = offset_loops(geo(&m), f, d).unwrap();
    // The tip gives two points: on the offset of the incoming edge (p2 -> p0), then on the
    // offset of the outgoing edge (p0 -> p1). The two base corners (80 degrees) are mitred.
    assert_eq!(out[0].len(), 4);
    let e0 = (p[0] - p[2]).normalize();
    let e1 = (p[1] - p[0]).normalize();
    assert_points(
        &out[0][..2],
        &[p[0] + d * e0.cross(DVec3::Z), p[0] + d * e1.cross(DVec3::Z)],
    );
    // Inward, the same corner closes up: one mitre point on the bisector.
    let inward = offset_loops(geo(&m), f, -d).unwrap();
    assert_eq!(inward[0].len(), 3);
    assert_points(
        &inward[0][..1],
        &[v(d / 10f64.to_radians().sin(), 0.0, 0.0)],
    );
}

#[test]
fn corner_under_the_mitre_limit_is_mitred() {
    // 32 degrees: mitre 1 / sin(16 deg) = 3.63 times the distance, under MITRE_LIMIT.
    let mut m = Model::new();
    let (f, _) = spike(&mut m, 32.0);
    let d = 0.1;
    let out = offset_loops(geo(&m), f, d).unwrap();
    assert_eq!(out[0].len(), 3);
    assert_points(&out[0][..1], &[v(-d / 16f64.to_radians().sin(), 0.0, 0.0)]);
    // 28 degrees: 1 / sin(14 deg) = 4.13, over the limit.
    let mut m = Model::new();
    let (f, _) = spike(&mut m, 28.0);
    assert_eq!(offset_loops(geo(&m), f, d).unwrap()[0].len(), 4);
    assert_eq!(MITRE_LIMIT, 4.0);
}

#[test]
fn sharp_notch_is_bevelled_inward() {
    // A square with a thin V notch from the top; its tip (2, 1) is a reflex corner, so the
    // loop opens there when it moves inward.
    let pts = xy(&[
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 4.0),
        (2.2, 4.0),
        (2.0, 1.0),
        (1.8, 4.0),
        (0.0, 4.0),
    ]);
    let mut m = Model::new();
    let f = polygon(&mut m, &pts);
    let d = -0.05;
    let inward = offset_loops(geo(&m), f, d).unwrap();
    assert_eq!(inward[0].len(), 8);
    let e0 = (pts[4] - pts[3]).normalize();
    let e1 = (pts[5] - pts[4]).normalize();
    assert_points(
        &inward[0][4..6],
        &[
            pts[4] + d * e0.cross(DVec3::Z),
            pts[4] + d * e1.cross(DVec3::Z),
        ],
    );
    // Outward the notch tip closes up and is mitred.
    assert_eq!(offset_loops(geo(&m), f, 0.05).unwrap()[0].len(), 7);
}

// ---------------------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------------------

#[test]
fn bad_distances_are_refused() {
    let mut m = Model::new();
    let f = square(&mut m, 0.0, 2.0);
    for d in [
        0.0,
        1e-6,
        -1e-6,
        geom::LINEAR_TOL,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert_eq!(refused(&mut m, f, d), FaceOffsetError::BadDistance, "{d}");
        assert_eq!(
            offset_loops(geo(&m), f, d),
            Err(FaceOffsetError::BadDistance)
        );
    }
}

#[test]
fn missing_face_and_body_are_refused() {
    let mut m = Model::new();
    let f = square(&mut m, 0.0, 2.0);
    let ghost = FaceId(9999);
    assert_eq!(
        refused(&mut m, ghost, 0.5),
        FaceOffsetError::MissingFace(ghost)
    );
    assert_eq!(
        offset_loops(geo(&m), ghost, 0.5),
        Err(FaceOffsetError::MissingFace(ghost))
    );
    let body = BodyId::Definition(DefinitionId(77));
    let mut undo = UndoStack::new();
    assert_eq!(
        offset_face(&mut m, body, f, 0.5, &mut undo, "Offset"),
        Err(FaceOffsetError::MissingBody(body))
    );
    // A missing face is reported before a bad distance.
    assert_eq!(
        refused(&mut m, ghost, f64::NAN),
        FaceOffsetError::MissingFace(ghost)
    );
}

#[test]
fn non_planar_face_is_refused() {
    let mut m = Model::new();
    let f = square(&mut m, 0.0, 2.0);
    let corner = geo(&m).loop_vertices(&geo(&m).face(f).unwrap().outer)[2];
    let mut edit = ModelEdit::new();
    m.edit_geometry(BodyId::Root, &mut edit, |ctx, log| {
        ctx.set_vertex_position(corner, v(2.0, 2.0, 0.01), log)
            .unwrap();
    })
    .unwrap();
    assert_eq!(refused(&mut m, f, -0.5), FaceOffsetError::NotPlanar(f));
    assert_eq!(
        offset_loops(geo(&m), f, 0.5),
        Err(FaceOffsetError::NotPlanar(f))
    );
}

#[test]
fn inward_too_far_collapses() {
    let mut m = Model::new();
    let f = square(&mut m, 0.0, 2.0);
    assert_eq!(refused(&mut m, f, -1.0), FaceOffsetError::Collapsed);
    assert_eq!(refused(&mut m, f, -1.5), FaceOffsetError::Collapsed);
    assert_eq!(refused(&mut m, f, -40.0), FaceOffsetError::Collapsed);
    assert_eq!(
        offset_loops(geo(&m), f, -1.0),
        Err(FaceOffsetError::Collapsed)
    );
    assert!(run(&mut m, f, -0.99).is_ok());
}

#[test]
fn slot_bottom_collapses_outward() {
    // A slot 1 wide: its bottom edge runs between two reflex corners, so it shrinks by twice
    // the distance as the face grows, and vanishes at 0.5.
    let mut m = Model::new();
    let f = polygon(
        &mut m,
        &xy(&[
            (0.0, 0.0),
            (5.0, 0.0),
            (5.0, 4.0),
            (3.0, 4.0),
            (3.0, 1.0),
            (2.0, 1.0),
            (2.0, 4.0),
            (0.0, 4.0),
        ]),
    );
    assert!(offset_loops(geo(&m), f, 0.49).is_ok());
    assert_eq!(refused(&mut m, f, 0.5), FaceOffsetError::Collapsed);
    assert_eq!(refused(&mut m, f, 0.6), FaceOffsetError::Collapsed);
    // Inward the slot only widens.
    assert!(run(&mut m, f, -0.4).is_ok());
}

#[test]
fn hole_collapses_outward() {
    let mut m = Model::new();
    let f = square_with_hole(&mut m, 0.0, 10.0, 4.0, 6.0);
    assert_eq!(refused(&mut m, f, 1.0), FaceOffsetError::Collapsed);
    assert_eq!(refused(&mut m, f, 3.0), FaceOffsetError::Collapsed);
    assert!(run(&mut m, f, 0.99).is_ok());
}

#[test]
fn outer_loop_and_hole_meeting_cross() {
    // A ring 1 wide: inward, the outer loop and the growing hole meet at 0.5.
    let mut m = Model::new();
    let f = square_with_hole(&mut m, 0.0, 10.0, 1.0, 9.0);
    assert_eq!(refused(&mut m, f, -0.5), FaceOffsetError::LoopsCross);
    // Past each other: the offset hole is outside the offset outer loop.
    assert_eq!(refused(&mut m, f, -1.5), FaceOffsetError::LoopsCross);
    assert_eq!(
        offset_loops(geo(&m), f, -1.5),
        Err(FaceOffsetError::LoopsCross)
    );
    assert!(run(&mut m, f, -0.4).is_ok());
}

/// A "C": a 6 x 6 square around a 4 x 4 hollow that opens to the right through a channel
/// whose two arm tips face each other 0.4 apart.
fn c_shape(model: &mut Model) -> FaceId {
    polygon(
        model,
        &xy(&[
            (0.0, 0.0),
            (6.0, 0.0),
            (6.0, 2.8),
            (5.0, 2.8),
            (5.0, 1.0),
            (1.0, 1.0),
            (1.0, 5.0),
            (5.0, 5.0),
            (5.0, 3.2),
            (6.0, 3.2),
            (6.0, 6.0),
            (0.0, 6.0),
        ]),
    )
}

#[test]
fn loop_crossing_itself_is_refused() {
    let mut m = Model::new();
    let f = c_shape(&mut m);
    assert!(offset_loops(geo(&m), f, 0.15).is_ok());
    assert_eq!(refused(&mut m, f, 0.2), FaceOffsetError::LoopsCross);
    assert_eq!(refused(&mut m, f, 0.3), FaceOffsetError::LoopsCross);
    assert_eq!(
        offset_loops(geo(&m), f, 0.3),
        Err(FaceOffsetError::LoopsCross)
    );
}

#[test]
fn collapse_is_reported_before_crossing() {
    // At -1.5 the ring's hole has outgrown the outer loop (LoopsCross) and nothing collapsed;
    // at -6 the outer loop has turned inside out, which is reported first.
    let mut m = Model::new();
    let f = square_with_hole(&mut m, 0.0, 10.0, 1.0, 9.0);
    assert_eq!(refused(&mut m, f, -6.0), FaceOffsetError::Collapsed);
}

#[test]
fn ring_over_another_face_is_refused() {
    // A small coplanar face sits where the outward ring would go, touching nothing.
    let mut m = Model::new();
    let pts = xy(&[
        (0.0, 0.0),
        (1.0, 0.0),
        (1.0, 1.0),
        (0.0, 1.0),
        (1.1, 0.4),
        (1.2, 0.4),
        (1.2, 0.5),
        (1.1, 0.5),
    ]);
    let ids = mesh(&mut m, &pts, &[&[&[0, 1, 2, 3]], &[&[4, 5, 6, 7]]]);
    match refused(&mut m, ids[0], 0.25) {
        FaceOffsetError::Overlap(a, b) => {
            assert_eq!(a, ids[1]);
            assert!(b > ids[1]);
        }
        other => panic!("{other:?}"),
    }
    // Small enough to stay clear of it.
    assert!(run(&mut m, ids[0], 0.05).is_ok());
}

#[test]
fn outward_into_a_filled_hole_is_refused() {
    // The hole is filled by another face; the outward hole ring would lie on it.
    let mut m = Model::new();
    let pts = xy(&[
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 4.0),
        (0.0, 4.0),
        (1.0, 1.0),
        (3.0, 1.0),
        (3.0, 3.0),
        (1.0, 3.0),
    ]);
    let ids = mesh(
        &mut m,
        &pts,
        &[&[&[0, 1, 2, 3], &[4, 7, 6, 5]], &[&[4, 5, 6, 7]]],
    );
    // Its outer loop would be the filling face's outer loop: a duplicate face.
    assert!(matches!(
        refused(&mut m, ids[0], 0.25),
        FaceOffsetError::Invalid(Violation::DuplicateFace(a, _)) if a == ids[1]
    ));
    // Inward the hole grows into the face's own area: fine.
    let r = run(&mut m, ids[0], -0.25).unwrap();
    assert_eq!(r.hole_rings.len(), 1);
    assert!(geo(&m).validate().is_empty());
    assert!(geo(&m).face_overlaps().is_empty());
}

#[test]
fn ring_crossing_an_edge_is_refused() {
    let mut m = Model::new();
    let f = square(&mut m, 0.0, 2.0);
    let mut edit = ModelEdit::new();
    m.edit_geometry(BodyId::Root, &mut edit, |ctx, log| {
        let a = ctx.insert_vertex(v(3.0, -3.0, 0.0), log);
        let b = ctx.insert_vertex(v(3.0, 5.0, 0.0), log);
        ctx.insert_edge(a, b, log).unwrap();
    })
    .unwrap();
    assert!(matches!(
        refused(&mut m, f, 1.5),
        FaceOffsetError::Invalid(Violation::EdgesIntersect(_, _))
    ));
    assert!(run(&mut m, f, 0.5).is_ok());
}

// ---------------------------------------------------------------------------------------
// The faces made
// ---------------------------------------------------------------------------------------

#[test]
fn inward_square_makes_ring_and_inner_face() {
    let mut m = Model::new();
    let f = square(&mut m, 0.0, 2.0);
    let old_outer = geo(&m).face(f).unwrap().outer.clone();
    let before = m.clone();
    let mut undo = UndoStack::new();
    let r = offset_face(&mut m, BodyId::Root, f, -0.5, &mut undo, "Offset").unwrap();
    let c = geo(&m);
    assert_eq!(r.ring, f, "the original face stays as the ring");
    assert_ne!(r.inner, f);
    assert!(r.hole_rings.is_empty());
    assert_eq!(c.face_count(), 2);
    let ring = c.face(f).unwrap();
    assert_eq!(ring.outer, old_outer, "the ring keeps the old outer loop");
    assert_eq!(ring.inner.len(), 1);
    assert_points(
        &outer_points(c, r.inner),
        &xy(&[(0.5, 0.5), (1.5, 0.5), (1.5, 1.5), (0.5, 1.5)]),
    );
    assert_eq!(
        edge_set(&ring.inner[0]),
        edge_set(&c.face(r.inner).unwrap().outer)
    );
    assert!(c.face(r.inner).unwrap().inner.is_empty());
    assert_near(face_area(c, r.inner), 1.0);
    assert_near(face_area(c, f), 3.0);
    assert!(normal(c, r.inner).distance(DVec3::Z) < 1e-9);
    assert!(normal(c, f).distance(DVec3::Z) < 1e-9);
    assert!(c.validate().is_empty());
    assert!(c.face_overlaps().is_empty());

    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("Offset"));
    let after = m.clone();
    undo.undo(&mut m).unwrap();
    assert!(m == before);
    undo.redo(&mut m).unwrap();
    assert!(m == after);
}

#[test]
fn outward_square_adds_a_ring_and_keeps_the_face() {
    let mut m = Model::new();
    let f = square(&mut m, 0.0, 2.0);
    let old = geo(&m).face(f).unwrap().clone();
    let before = m.clone();
    let mut undo = UndoStack::new();
    let r = offset_face(&mut m, BodyId::Root, f, 0.5, &mut undo, "Offset").unwrap();
    let c = geo(&m);
    assert_eq!(r.inner, f);
    assert_ne!(r.ring, f);
    assert_eq!(c.face(f).unwrap(), &old, "the original face is untouched");
    assert_eq!(c.face_count(), 2);
    let ring = c.face(r.ring).unwrap();
    assert_points(
        &c.loop_points(&ring.outer),
        &xy(&[(-0.5, -0.5), (2.5, -0.5), (2.5, 2.5), (-0.5, 2.5)]),
    );
    assert_eq!(ring.inner.len(), 1);
    assert_eq!(edge_set(&ring.inner[0]), edge_set(&old.outer));
    assert_near(face_area(c, r.ring), 5.0);
    assert!(normal(c, r.ring).distance(DVec3::Z) < 1e-9);
    assert!(c.validate().is_empty());
    assert!(c.face_overlaps().is_empty());
    undo.undo(&mut m).unwrap();
    assert!(m == before);
}

#[test]
fn edges_list_follows_the_offset_loops() {
    let mut m = Model::new();
    let f = square_with_hole(&mut m, 0.0, 4.0, 1.0, 3.0);
    let want = offset_loops(geo(&m), f, -0.25).unwrap();
    let r = run(&mut m, f, -0.25).unwrap();
    let c = geo(&m);
    assert_eq!(r.edges.len(), 2);
    for (edges, pts) in r.edges.iter().zip(&want) {
        assert_eq!(edges.len(), pts.len());
        for (i, e) in edges.iter().enumerate() {
            let ends = c.edge(*e).unwrap().ends;
            let a = c.vertex(ends[0]).unwrap().position;
            let b = c.vertex(ends[1]).unwrap().position;
            let (p, q) = (pts[i], pts[(i + 1) % pts.len()]);
            assert!(
                (a.distance(p) < 1e-9 && b.distance(q) < 1e-9)
                    || (a.distance(q) < 1e-9 && b.distance(p) < 1e-9),
                "edge {i} doesn't join offset points {i} and {}",
                i + 1
            );
        }
    }
}

#[test]
fn inward_with_a_hole_makes_three_faces() {
    let mut m = Model::new();
    let f = square_with_hole(&mut m, 0.0, 4.0, 1.0, 3.0);
    let old = geo(&m).face(f).unwrap().clone();
    let r = run(&mut m, f, -0.25).unwrap();
    let c = geo(&m);
    assert_eq!(r.ring, f);
    assert_eq!(r.hole_rings.len(), 1);
    assert_eq!(c.face_count(), 3);
    let ring = c.face(f).unwrap();
    assert_eq!(ring.outer, old.outer);
    assert_eq!(ring.inner.len(), 1, "the ring loses the old hole");
    let inner = c.face(r.inner).unwrap();
    assert_eq!(inner.inner.len(), 1);
    let hole_ring = c.face(r.hole_rings[0]).unwrap();
    assert_eq!(hole_ring.inner.len(), 1);
    assert_eq!(edge_set(&hole_ring.inner[0]), edge_set(&old.inner[0]));
    assert_eq!(edge_set(&hole_ring.outer), edge_set(&inner.inner[0]));
    assert_near(face_area(c, f), 16.0 - 12.25);
    assert_near(face_area(c, r.inner), 12.25 - 6.25);
    assert_near(face_area(c, r.hole_rings[0]), 6.25 - 4.0);
    for g in [f, r.inner, r.hole_rings[0]] {
        assert!(normal(c, g).distance(DVec3::Z) < 1e-9);
    }
    assert!(c.validate().is_empty());
    assert!(c.face_overlaps().is_empty());
}

#[test]
fn outward_with_a_hole_rings_both_loops() {
    let mut m = Model::new();
    let f = square_with_hole(&mut m, 0.0, 4.0, 1.0, 3.0);
    let old = geo(&m).face(f).unwrap().clone();
    let r = run(&mut m, f, 0.5).unwrap();
    let c = geo(&m);
    assert_eq!(r.inner, f);
    assert_eq!(c.face(f).unwrap(), &old);
    assert_eq!(r.hole_rings.len(), 1);
    assert_eq!(c.face_count(), 3);
    let hole_ring = c.face(r.hole_rings[0]).unwrap();
    assert_eq!(edge_set(&hole_ring.outer), edge_set(&old.inner[0]));
    assert_points(
        &c.loop_points(&hole_ring.inner[0]),
        &xy(&[(1.5, 1.5), (1.5, 2.5), (2.5, 2.5), (2.5, 1.5)]),
    );
    assert_near(face_area(c, r.ring), 25.0 - 16.0);
    assert_near(face_area(c, r.hole_rings[0]), 4.0 - 1.0);
    assert_near(face_area(c, f), 12.0);
    for g in [f, r.ring, r.hole_rings[0]] {
        assert!(normal(c, g).distance(DVec3::Z) < 1e-9);
    }
    assert!(c.validate().is_empty());
    assert!(c.face_overlaps().is_empty());
}

#[test]
fn new_faces_take_the_face_materials() {
    for d in [-0.25, 0.25] {
        let mut m = Model::new();
        let f = square_with_hole(&mut m, 0.0, 4.0, 1.0, 3.0);
        let mut edit = ModelEdit::new();
        let red = m.add_material(Material::new("Red", [1.0, 0.0, 0.0, 1.0]), &mut edit);
        let blue = m.add_material(Material::new("Blue", [0.0, 0.0, 1.0, 1.0]), &mut edit);
        let paint = FaceMaterials {
            front: Some(red),
            back: Some(blue),
            ..FaceMaterials::default()
        };
        m.set_face_materials(BodyId::Root, f, paint, &mut edit)
            .unwrap();
        let r = run(&mut m, f, d).unwrap();
        for g in [r.inner, r.ring, r.hole_rings[0]] {
            assert_eq!(m.root().face_materials(g), paint, "d = {d}");
        }
    }
}

#[test]
fn unpainted_faces_stay_unpainted() {
    let mut m = Model::new();
    let f = square(&mut m, 0.0, 2.0);
    let r = run(&mut m, f, -0.5).unwrap();
    assert_eq!(m.root().face_material_entries().count(), 0);
    assert_eq!(m.root().face_materials(r.inner), FaceMaterials::default());
}

#[test]
fn box_top_inward_then_push_pull() {
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
    let mut m = Model::new();
    let ids = mesh(
        &mut m,
        &pts,
        &[
            &[&[0, 3, 2, 1]],
            &[&[4, 5, 6, 7]],
            &[&[0, 1, 5, 4]],
            &[&[2, 3, 7, 6]],
            &[&[1, 2, 6, 5]],
            &[&[3, 0, 4, 7]],
        ],
    );
    let top = ids[1];
    let r = run(&mut m, top, -0.2).unwrap();
    assert_eq!(geo(&m).face_count(), 7);
    assert!(geo(&m).validate().is_empty());
    let mut undo = UndoStack::new();
    push_pull(&mut m, BodyId::Root, r.inner, -0.3, &mut undo, "Push/Pull").unwrap();
    assert!(geo(&m).validate().is_empty());
    assert!(
        geo(&m)
            .edges()
            .all(|(e, _)| geo(&m).faces_of_edge(e).len() == 2)
    );
}

#[test]
fn box_top_outward_makes_a_flange() {
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
    let mut m = Model::new();
    let ids = mesh(
        &mut m,
        &pts,
        &[
            &[&[0, 3, 2, 1]],
            &[&[4, 5, 6, 7]],
            &[&[0, 1, 5, 4]],
            &[&[2, 3, 7, 6]],
            &[&[1, 2, 6, 5]],
            &[&[3, 0, 4, 7]],
        ],
    );
    let top = ids[1];
    let r = run(&mut m, top, 0.25).unwrap();
    let c = geo(&m);
    assert_eq!(c.face_count(), 7);
    for u in &c.face(top).unwrap().outer.uses {
        assert_eq!(c.faces_of_edge(u.edge).len(), 3);
    }
    assert!(normal(c, r.ring).distance(DVec3::Z) < 1e-9);
    assert!(c.validate().is_empty());
}

#[test]
fn works_inside_a_group() {
    let mut m = Model::new();
    let mut edit = ModelEdit::new();
    let def = m.add_definition(
        DefinitionInfo {
            name: "Part".into(),
            group: true,
        },
        &mut edit,
    );
    m.add_instance(
        BodyId::Root,
        Instance {
            definition: def,
            transform: DMat4::from_translation(v(5.0, 0.0, 0.0)),
            name: String::new(),
            material: None,
        },
        &mut edit,
    )
    .unwrap();
    let body = BodyId::Definition(def);
    let pts = xy(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
    let f = mesh_in(&mut m, body, &pts, &[&[&[0, 1, 2, 3]]])[0];
    let root_before = m.root().geometry().clone();
    let mut undo = UndoStack::new();
    let r = offset_face(&mut m, body, f, -0.5, &mut undo, "Offset").unwrap();
    let c = m.body(body).unwrap().geometry();
    assert_eq!(c.face_count(), 2);
    assert_points(
        &outer_points(c, r.inner),
        &xy(&[(0.5, 0.5), (1.5, 0.5), (1.5, 1.5), (0.5, 1.5)]),
    );
    assert!(m.root().geometry() == &root_before);
}

// ---------------------------------------------------------------------------------------
// Curves
// ---------------------------------------------------------------------------------------

fn target<'a>(model: &'a mut Model, undo: &'a mut UndoStack) -> Target<'a> {
    Target {
        model,
        body: BodyId::Root,
        undo,
        unit: Unit::Meter,
    }
}

/// The face whose outer loop has exactly the vertices at `pts` (any order).
fn face_with(c: &Context, pts: &[DVec3]) -> FaceId {
    c.faces()
        .find(|(_, face)| {
            let got = c.loop_points(&face.outer);
            got.len() == pts.len()
                && pts
                    .iter()
                    .all(|p| got.iter().any(|q| q.distance(*p) < 1e-9))
        })
        .map(|(id, _)| id)
        .expect("no such face")
}

fn new_curves(before: &BTreeSet<CurveId>, c: &Context) -> Vec<CurveId> {
    c.curves()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect()
}

#[test]
fn circle_stays_a_curve() {
    let n = 24;
    let step = TAU / n as f64;
    for d in [-0.1, 0.1] {
        let mut m = Model::new();
        let mut undo = UndoStack::new();
        let pts = shapes::circle(DVec3::ZERO, DVec3::Z, DVec3::X, 1.0, n);
        draw_curve(&mut target(&mut m, &mut undo), &pts, true, "Circle").unwrap();
        let f = face_with(geo(&m), &pts);
        let old: BTreeSet<CurveId> = geo(&m).curves().map(|(id, _)| id).collect();
        assert_eq!(old.len(), 1);
        let r = run(&mut m, f, d).unwrap();
        let c = geo(&m);
        assert!(c.validate().is_empty(), "{:?}", c.validate());
        let added = new_curves(&old, c);
        assert_eq!(added.len(), 1, "d = {d}");
        let curve = c.curve(added[0]).unwrap();
        assert!(curve.is_closed());
        assert!(curve.center.distance(DVec3::ZERO) < 1e-9);
        assert_near(curve.radius, 1.0 + d / (step / 2.0).cos());
        let loop_vs: BTreeSet<VertexId> = r.edges[0]
            .iter()
            .flat_map(|e| c.edge(*e).unwrap().ends)
            .collect();
        let curve_vs: BTreeSet<VertexId> = curve.vertices.iter().copied().collect();
        assert_eq!(curve_vs, loop_vs);
        // The old circle is still there, unchanged.
        let kept = c.curve(*old.iter().next().unwrap()).unwrap();
        assert_near(kept.radius, 1.0);
    }
}

#[test]
fn circular_hole_stays_a_curve() {
    let n = 24;
    let step = TAU / n as f64;
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let sq = xy(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
    let segs: Vec<(DVec3, DVec3)> = (0..4).map(|i| (sq[i], sq[(i + 1) % 4])).collect();
    draw_segments(&mut target(&mut m, &mut undo), &segs, "Rectangle").unwrap();
    let pts = shapes::circle(v(2.0, 2.0, 0.0), DVec3::Z, DVec3::X, 0.5, n);
    draw_curve(&mut target(&mut m, &mut undo), &pts, true, "Circle").unwrap();
    let f = face_with(geo(&m), &sq);
    let disk = face_with(geo(&m), &pts);
    assert_eq!(geo(&m).face(f).unwrap().inner.len(), 1);
    let old: BTreeSet<CurveId> = geo(&m).curves().map(|(id, _)| id).collect();

    // Inward the hole grows by the distance (its mitre points by d / cos(half step)).
    let mut grown = m.clone();
    let r = run(&mut grown, f, -0.2).unwrap();
    let c = geo(&grown);
    assert!(c.validate().is_empty());
    let added = new_curves(&old, c);
    assert_eq!(added.len(), 1);
    let curve = c.curve(added[0]).unwrap();
    assert!(curve.center.distance(v(2.0, 2.0, 0.0)) < 1e-9);
    assert_near(curve.radius, 0.5 + 0.2 / (step / 2.0).cos());
    assert_eq!(r.hole_rings.len(), 1);
    assert!(c.face(disk).is_some());

    // Outward the hole ring would lie on the disk that fills the hole.
    assert!(matches!(
        refused(&mut m, f, 0.2),
        FaceOffsetError::Invalid(Violation::DuplicateFace(a, _)) if a == disk
    ));
}

#[test]
fn arc_meeting_a_line_keeps_its_middle() {
    // A "D": a 12-step half circle of radius 1 closed by its diameter. Offset, the arc's two
    // end vertices are mitred with the diameter and leave the circle; the 11 vertices between
    // them stay on a concentric circle and become the new curve.
    let n = 12;
    let step = PI / n as f64;
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let arc = shapes::arc(DVec3::ZERO, DVec3::Z, DVec3::X, 1.0, 0.0, PI, n);
    assert_eq!(arc.len(), n + 1);
    draw_curve(&mut target(&mut m, &mut undo), &arc, false, "Arc").unwrap();
    draw_segments(&mut target(&mut m, &mut undo), &[(arc[n], arc[0])], "Line").unwrap();
    assert_eq!(geo(&m).face_count(), 1);
    let f = geo(&m).faces().next().unwrap().0;
    let old: BTreeSet<CurveId> = geo(&m).curves().map(|(id, _)| id).collect();
    for d in [-0.1, 0.1] {
        let mut w = m.clone();
        run(&mut w, f, d).unwrap();
        let c = geo(&w);
        assert!(c.validate().is_empty());
        let added = new_curves(&old, c);
        assert_eq!(added.len(), 1, "d = {d}");
        let curve = c.curve(added[0]).unwrap();
        assert!(!curve.is_closed());
        assert_eq!(curve.vertices.len(), n - 1);
        assert!(curve.center.distance(DVec3::ZERO) < 1e-9);
        assert_near(curve.radius, 1.0 + d / (step / 2.0).cos());
    }
}

#[test]
fn short_arc_meeting_lines_is_dropped() {
    // A 3-step arc closed by its chord: its end vertices leave the circle and only 2 vertices
    // stay on it, too few for a curve. The offset loop is plain segments.
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let arc = shapes::arc(DVec3::ZERO, DVec3::Z, DVec3::X, 1.0, 0.0, PI, 3);
    draw_curve(&mut target(&mut m, &mut undo), &arc, false, "Arc").unwrap();
    draw_segments(&mut target(&mut m, &mut undo), &[(arc[3], arc[0])], "Line").unwrap();
    let f = geo(&m).faces().next().unwrap().0;
    let old: BTreeSet<CurveId> = geo(&m).curves().map(|(id, _)| id).collect();
    assert_eq!(old.len(), 1);
    let r = run(&mut m, f, -0.1).unwrap();
    let c = geo(&m);
    assert!(new_curves(&old, c).is_empty());
    assert_eq!(c.curve_count(), 1);
    assert_eq!(r.edges[0].len(), 4);
    assert!(c.validate().is_empty());
}

#[test]
fn closed_three_step_curve_carries() {
    // Each corner of a 3-step circle is 60 degrees: mitred both ways, so the whole chain maps
    // and stays on a concentric circle.
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let pts = shapes::circle(DVec3::ZERO, DVec3::Z, DVec3::X, 1.0, 3);
    draw_curve(&mut target(&mut m, &mut undo), &pts, true, "Circle").unwrap();
    let f = face_with(geo(&m), &pts);
    let old: BTreeSet<CurveId> = geo(&m).curves().map(|(id, _)| id).collect();
    for d in [-0.1, 0.1] {
        let mut w = m.clone();
        run(&mut w, f, d).unwrap();
        let added = new_curves(&old, geo(&w));
        assert_eq!(added.len(), 1, "d = {d}");
        // Half step 60 degrees: the corners move by d / cos(60 deg) = 2 d.
        assert_near(geo(&w).curve(added[0]).unwrap().radius, 1.0 + 2.0 * d);
    }
}
