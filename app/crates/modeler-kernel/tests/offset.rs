// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Grow and shrink by a clearance (`docs/specs/m7-clearance-offset.md`).

use glam::{DMat4, DVec2, DVec3};
use modeler_kernel::curve;
use modeler_kernel::model::*;
use modeler_kernel::offset::{OffsetError, offset_bodies, offset_body, offset_positions};
use modeler_kernel::solid::check_body;
use modeler_kernel::topology::*;

mod offset_common;
use offset_common::*;

const D: f64 = 0.0002;

/// Offsets one body with a fresh log, checks the model is valid afterwards (or unchanged
/// with an empty log on an error) and returns the result.
fn offset(
    model: &mut Model,
    body: BodyId,
    d: f64,
) -> Result<modeler_kernel::offset::OffsetReport, OffsetError> {
    let before = model.clone();
    let mut log = ModelEdit::new();
    let result = offset_body(model, body, d, &mut log);
    match &result {
        Ok(_) => {
            let violations = model.validate();
            assert!(violations.is_empty(), "{violations:?}");
            let check = check_body(model, body).unwrap();
            assert!(check.is_solid(), "{check:?}");
            // The log undoes the whole offset.
            let mut undone = model.clone();
            undone.apply_edit(&log.inverse()).unwrap();
            assert!(undone == before);
        }
        Err(_) => {
            assert!(*model == before, "a refused offset changed the model");
            assert!(log.is_empty(), "a refused offset logged records");
        }
    }
    result
}

fn ten_by_twenty_by_thirty(model: &mut Model) -> BodyId {
    box_part(model, DVec3::ZERO, v(mm(10.0), mm(20.0), mm(30.0)))
}

// ---------------------------------------------------------------- boxes

#[test]
fn grow_box_moves_every_corner_out_by_d() {
    let mut model = Model::new();
    let body = ten_by_twenty_by_thirty(&mut model);
    let before = positions(&model, body);
    let report = offset(&mut model, body, D).unwrap();
    assert_eq!(report.moved, 8);
    let after = positions(&model, body);
    let centre = v(mm(5.0), mm(10.0), mm(15.0));
    for (id, p) in &before {
        let out = (*p - centre).signum() * D;
        assert!(near(after[id], *p + out), "{id:?}: {:?}", after[id]);
    }
}

#[test]
fn grow_box_volume_and_report() {
    let mut model = Model::new();
    let body = ten_by_twenty_by_thirty(&mut model);
    let report = offset(&mut model, body, D).unwrap();
    let (a, b, c) = (mm(10.0), mm(20.0), mm(30.0));
    assert!(close(report.volume_before, a * b * c), "{report:?}");
    let want = (a + 2.0 * D) * (b + 2.0 * D) * (c + 2.0 * D);
    assert!(close(report.volume_after, want), "{report:?}");
    assert!(close(volume(&model, body), want));
}

#[test]
fn shrink_box_volume_and_bounds() {
    let mut model = Model::new();
    let body = ten_by_twenty_by_thirty(&mut model);
    let report = offset(&mut model, body, -D).unwrap();
    let (a, b, c) = (mm(10.0), mm(20.0), mm(30.0));
    let want = (a - 2.0 * D) * (b - 2.0 * D) * (c - 2.0 * D);
    assert!(close(report.volume_after, want), "{report:?}");
    let (lo, hi) = bounds(&model, body);
    assert!(near(lo, DVec3::splat(D)), "{lo:?}");
    assert!(near(hi, v(a - D, b - D, c - D)), "{hi:?}");
}

#[test]
fn offset_keeps_every_id_and_loop() {
    let mut model = Model::new();
    let body = ten_by_twenty_by_thirty(&mut model);
    let before = topology_of(&model, body);
    offset(&mut model, body, D).unwrap();
    assert_eq!(topology_of(&model, body), before);
}

#[test]
fn offset_moves_only_vertices() {
    let mut model = Model::new();
    let body = ten_by_twenty_by_thirty(&mut model);
    let mut log = ModelEdit::new();
    offset_body(&mut model, body, D, &mut log).unwrap();
    assert_eq!(log.records.len(), 8);
    for r in &log.records {
        assert!(
            matches!(r, ModelRecord::Geometry(b, Record::MoveVertex { .. }) if *b == body),
            "{r:?}"
        );
    }
}

#[test]
fn offset_leaves_the_rest_of_the_model_alone() {
    let mut model = Model::new();
    let a = ten_by_twenty_by_thirty(&mut model);
    let b = box_part(&mut model, v(0.1, 0.0, 0.0), v(0.11, 0.01, 0.01));
    let other = positions(&model, b);
    offset(&mut model, a, D).unwrap();
    assert_eq!(positions(&model, b), other);
}

#[test]
fn rotated_box_grows_by_d_along_each_face() {
    let mut model = Model::new();
    let m = DMat4::from_translation(v(0.3, -0.2, 0.1))
        * DMat4::from_axis_angle(v(1.0, 2.0, 3.0).normalize(), 0.7);
    let points: Vec<DVec3> = box_points(DVec3::ZERO, v(mm(10.0), mm(20.0), mm(30.0)))
        .into_iter()
        .map(|p| m.transform_point3(p))
        .collect();
    let body = part(&mut model, &points, &plain(&box_faces(0)));
    let old = geometry(&model, body).clone();
    let report = offset(&mut model, body, D).unwrap();
    planes_moved_by(&old, geometry(&model, body), D).unwrap();
    let want = (mm(10.0) + 2.0 * D) * (mm(20.0) + 2.0 * D) * (mm(30.0) + 2.0 * D);
    assert!(close(report.volume_after, want), "{report:?}");
}

#[test]
fn reversed_face_still_moves_out_of_the_material() {
    let mut model = Model::new();
    let mut faces = box_faces(0);
    faces[1] = reversed(&faces[1]);
    let points = box_points(DVec3::ZERO, v(mm(10.0), mm(20.0), mm(30.0)));
    let body = part(&mut model, &points, &plain(&faces));
    let mut clean = Model::new();
    let clean_body = ten_by_twenty_by_thirty(&mut clean);
    offset(&mut model, body, D).unwrap();
    offset(&mut clean, clean_body, D).unwrap();
    let got: Vec<DVec3> = positions(&model, body).into_values().collect();
    let want: Vec<DVec3> = positions(&clean, clean_body).into_values().collect();
    for (g, w) in got.iter().zip(&want) {
        assert!(near(*g, *w), "{g:?} {w:?}");
    }
    // The face keeps its (reversed) loop.
    let (_, _, after_faces) = topology_of(&model, body);
    assert_eq!(
        check_body(&model, body).unwrap().reversed_faces,
        vec![after_faces[1].0]
    );
}

#[test]
fn two_boxes_in_one_part_both_grow() {
    let mut model = Model::new();
    let mut points = box_points(DVec3::ZERO, DVec3::splat(mm(10.0)));
    points.extend(box_points(
        v(mm(20.0), 0.0, 0.0),
        v(mm(25.0), mm(10.0), mm(10.0)),
    ));
    let mut faces = box_faces(0);
    faces.extend(box_faces(8));
    let body = part(&mut model, &points, &plain(&faces));
    let report = offset(&mut model, body, D).unwrap();
    let a = (mm(10.0) + 2.0 * D).powi(3);
    let b = (mm(5.0) + 2.0 * D) * (mm(10.0) + 2.0 * D).powi(2);
    assert!(close(report.volume_after, a + b), "{report:?}");
}

// ---------------------------------------------------------------- vertex rules

#[test]
fn split_edge_vertex_moves_along_both_planes() {
    // The top face is split in two by an edge between the midpoints of two top edges: the
    // midpoints touch only two distinct planes (top and one side).
    let mut model = Model::new();
    let s = mm(10.0);
    let mut points = box_points(DVec3::ZERO, DVec3::splat(s));
    points.push(v(s / 2.0, 0.0, s)); // 8: middle of the front top edge
    points.push(v(s / 2.0, s, s)); // 9: middle of the back top edge
    let faces = vec![
        vec![0, 3, 2, 1],    // bottom
        vec![4, 8, 9, 7],    // top, left half
        vec![8, 5, 6, 9],    // top, right half
        vec![0, 1, 5, 8, 4], // front
        vec![2, 3, 7, 9, 6], // back
        vec![1, 2, 6, 5],    // right
        vec![3, 0, 4, 7],    // left
    ];
    let body = part(&mut model, &points, &plain(&faces));
    let old = geometry(&model, body).clone();
    let mid = old.find_vertex(points[8], 1e-9).unwrap();
    let report = offset(&mut model, body, D).unwrap();
    assert_eq!(report.moved, 10);
    let p = geometry(&model, body).vertex(mid).unwrap().position;
    assert!(near(p, points[8] + v(0.0, -D, D)), "{p:?}");
    planes_moved_by(&old, geometry(&model, body), D).unwrap();
    assert!(
        close(report.volume_after, (s + 2.0 * D).powi(3)),
        "{report:?}"
    );
}

#[test]
fn square_pyramid_apex_moves_up_its_axis() {
    // Four faces meet at the apex; by symmetry their moved planes still meet in one point.
    let mut model = Model::new();
    let (a, h) = (mm(10.0), mm(8.0));
    let points = vec![
        v(-a, -a, 0.0),
        v(a, -a, 0.0),
        v(a, a, 0.0),
        v(-a, a, 0.0),
        v(0.0, 0.0, h),
    ];
    let faces = vec![
        vec![0, 3, 2, 1],
        vec![0, 1, 4],
        vec![1, 2, 4],
        vec![2, 3, 4],
        vec![3, 0, 4],
    ];
    let body = part(&mut model, &points, &plain(&faces));
    let old = geometry(&model, body).clone();
    let apex = old.find_vertex(points[4], 1e-9).unwrap();
    offset(&mut model, body, D).unwrap();
    let p = geometry(&model, body).vertex(apex).unwrap().position;
    let rise = D * (h * h + a * a).sqrt() / a;
    assert!(near(p, v(0.0, 0.0, h + rise)), "{p:?}");
    planes_moved_by(&old, geometry(&model, body), D).unwrap();
}

#[test]
fn octahedron_grows_although_four_faces_meet_at_each_vertex() {
    let mut model = Model::new();
    let r = mm(10.0);
    let points = vec![
        v(r, 0.0, 0.0),
        v(-r, 0.0, 0.0),
        v(0.0, r, 0.0),
        v(0.0, -r, 0.0),
        v(0.0, 0.0, r),
        v(0.0, 0.0, -r),
    ];
    let faces = vec![
        vec![0, 2, 4],
        vec![2, 1, 4],
        vec![1, 3, 4],
        vec![3, 0, 4],
        vec![2, 0, 5],
        vec![1, 2, 5],
        vec![3, 1, 5],
        vec![0, 3, 5],
    ];
    let body = part(&mut model, &points, &plain(&faces));
    let old = geometry(&model, body).clone();
    let report = offset(&mut model, body, D).unwrap();
    planes_moved_by(&old, geometry(&model, body), D).unwrap();
    let grown = r + D * 3f64.sqrt();
    assert!(
        close(report.volume_after, 4.0 / 3.0 * grown.powi(3)),
        "{report:?}"
    );
}

#[test]
fn rectangular_pyramid_apex_is_unsupported() {
    // Four faces at the apex lean at two different angles: their moved planes miss each
    // other, so the apex can't follow all four.
    let mut model = Model::new();
    let (a, b, h) = (mm(10.0), mm(20.0), mm(8.0));
    let points = vec![
        v(-a, -b, 0.0),
        v(a, -b, 0.0),
        v(a, b, 0.0),
        v(-a, b, 0.0),
        v(0.0, 0.0, h),
    ];
    let faces = vec![
        vec![0, 3, 2, 1],
        vec![0, 1, 4],
        vec![1, 2, 4],
        vec![2, 3, 4],
        vec![3, 0, 4],
    ];
    let body = part(&mut model, &points, &plain(&faces));
    let apex = geometry(&model, body).find_vertex(points[4], 1e-9).unwrap();
    assert_eq!(
        offset(&mut model, body, D),
        Err(OffsetError::Unsupported(vec![apex]))
    );
}

#[test]
fn boxes_meeting_at_one_corner_are_unsupported_there() {
    // Two boxes share one corner vertex: its six planes face both ways.
    let mut model = Model::new();
    let s = mm(10.0);
    let mut points = box_points(DVec3::ZERO, DVec3::splat(s));
    let second = box_points(DVec3::splat(s), DVec3::splat(2.0 * s));
    points.extend(second[1..].iter().copied());
    // The second box's corner 0 is the first box's corner 6.
    let map = |i: usize| if i == 0 { 6 } else { i + 7 };
    let mut faces = box_faces(0);
    for f in box_faces(0) {
        faces.push(f.into_iter().map(map).collect());
    }
    let body = part(&mut model, &points, &plain(&faces));
    let report = check_body(&model, body).unwrap();
    assert!(report.is_solid(), "{report:?}");
    let shared = geometry(&model, body)
        .find_vertex(DVec3::splat(s), 1e-9)
        .unwrap();
    assert_eq!(
        offset(&mut model, body, D),
        Err(OffsetError::Unsupported(vec![shared]))
    );
}

// ---------------------------------------------------------------- holes and cavities

/// A 30 x 30 x 10 mm block with a square hole of side `hole` mm through it, centred.
fn block_with_hole(model: &mut Model, hole: f64) -> BodyId {
    let (s, t) = (mm(30.0), mm(10.0));
    let (h0, h1) = (mm(15.0 - hole / 2.0), mm(15.0 + hole / 2.0));
    let mut points = box_points(DVec3::ZERO, v(s, s, t));
    points.extend(box_points(v(h0, h0, 0.0), v(h1, h1, t)));
    // Hole loops run clockwise seen from each face's front; hole walls face into the hole.
    let faces: Vec<FaceSpec> = vec![
        (vec![0, 3, 2, 1], vec![vec![8, 9, 10, 11]]),
        (vec![4, 5, 6, 7], vec![vec![12, 15, 14, 13]]),
        (vec![0, 1, 5, 4], vec![]),
        (vec![2, 3, 7, 6], vec![]),
        (vec![1, 2, 6, 5], vec![]),
        (vec![3, 0, 4, 7], vec![]),
        (vec![8, 12, 13, 9], vec![]),
        (vec![10, 14, 15, 11], vec![]),
        (vec![9, 13, 14, 10], vec![]),
        (vec![11, 15, 12, 8], vec![]),
    ];
    part(model, &points, &faces)
}

#[test]
fn hole_through_a_block_narrows_when_it_grows() {
    let mut model = Model::new();
    let body = block_with_hole(&mut model, 6.0);
    let old = geometry(&model, body).clone();
    let report = offset(&mut model, body, D).unwrap();
    planes_moved_by(&old, geometry(&model, body), D).unwrap();
    let t = mm(10.0) + 2.0 * D;
    let want = (mm(30.0) + 2.0 * D).powi(2) * t - (mm(6.0) - 2.0 * D).powi(2) * t;
    assert!(close(report.volume_after, want), "{report:?}");
}

#[test]
fn hole_through_a_block_widens_when_it_shrinks() {
    let mut model = Model::new();
    let body = block_with_hole(&mut model, 6.0);
    let report = offset(&mut model, body, -D).unwrap();
    let t = mm(10.0) - 2.0 * D;
    let want = (mm(30.0) - 2.0 * D).powi(2) * t - (mm(6.0) + 2.0 * D).powi(2) * t;
    assert!(close(report.volume_after, want), "{report:?}");
}

#[test]
fn hole_narrower_than_twice_d_collapses() {
    let mut model = Model::new();
    let body = block_with_hole(&mut model, 0.3);
    let Err(OffsetError::Collapse { edges, faces }) = offset(&mut model, body, D) else {
        panic!("expected a collapse");
    };
    // The hole's eight rim edges turn round; its four vertical edges only move.
    let c = geometry(&model, body);
    let hole_rim: Vec<EdgeId> = c
        .edges()
        .filter(|(_, e)| {
            let a = c.vertex(e.ends[0]).unwrap().position;
            let b = c.vertex(e.ends[1]).unwrap().position;
            a.x > mm(1.0) && a.x < mm(29.0) && a.y > mm(1.0) && a.y < mm(29.0) && a.z == b.z
        })
        .map(|(id, _)| id)
        .collect();
    assert_eq!(edges, hole_rim);
    // Each hole wall turns over (its width reverses, its height doesn't). The hole loops in
    // the top and bottom reverse along both directions: a half turn, not a flip, so those
    // faces aren't listed.
    let walls: Vec<FaceId> = c
        .faces()
        .filter(|(f, _)| {
            let p = c.face_plane(*f).unwrap();
            p.normal.z.abs() < 0.5
                && p.point.x > mm(1.0)
                && p.point.x < mm(29.0)
                && p.point.y > mm(1.0)
                && p.point.y < mm(29.0)
        })
        .map(|(f, _)| f)
        .collect();
    assert_eq!(walls.len(), 4);
    assert_eq!(faces, walls);
}

/// A 20 mm cube with a `cavity` mm cube of empty space in its middle.
fn cube_with_cavity(model: &mut Model, cavity: f64) -> BodyId {
    let s = mm(20.0);
    let (c0, c1) = (mm(10.0 - cavity / 2.0), mm(10.0 + cavity / 2.0));
    let mut points = box_points(DVec3::ZERO, DVec3::splat(s));
    points.extend(box_points(DVec3::splat(c0), DVec3::splat(c1)));
    let mut faces = box_faces(0);
    // Cavity walls face into the cavity (out of the material).
    faces.extend(box_faces(8).iter().map(|f| reversed(f)));
    part(model, &points, &plain(&faces))
}

#[test]
fn cavity_shrinks_when_the_part_grows() {
    let mut model = Model::new();
    let body = cube_with_cavity(&mut model, 8.0);
    let old = geometry(&model, body).clone();
    let report = offset(&mut model, body, D).unwrap();
    planes_moved_by(&old, geometry(&model, body), D).unwrap();
    let want = (mm(20.0) + 2.0 * D).powi(3) - (mm(8.0) - 2.0 * D).powi(3);
    assert!(close(report.volume_after, want), "{report:?}");
    let c = geometry(&model, body);
    let inner = c.find_vertex(DVec3::splat(mm(6.0) + D), 1e-9);
    assert!(inner.is_some(), "the cavity's low corner moved in by d");
}

#[test]
fn cavity_smaller_than_twice_d_collapses() {
    let mut model = Model::new();
    let body = cube_with_cavity(&mut model, 0.3);
    let Err(OffsetError::Collapse { edges, faces }) = offset(&mut model, body, D) else {
        panic!("expected a collapse");
    };
    // All twelve cavity edges turn round; the outside is fine. Each cavity face reverses
    // along both its directions (a half turn, not a flip), so no face is listed.
    assert_eq!(edges.len(), 12, "{edges:?}");
    assert!(faces.is_empty(), "{faces:?}");
    let c = geometry(&model, body);
    for e in &edges {
        let a = c.vertex(c.edge(*e).unwrap().ends[0]).unwrap().position;
        assert!(
            a.distance(DVec3::splat(mm(10.0))) < mm(1.0),
            "{e:?} is not a cavity edge"
        );
    }
}

// ---------------------------------------------------------------- collapse and overlap

#[test]
fn shrink_past_half_thickness_collapses_the_thin_edges() {
    let mut model = Model::new();
    let body = box_part(&mut model, DVec3::ZERO, v(mm(10.0), mm(10.0), mm(1.0)));
    let Err(OffsetError::Collapse { edges, faces }) = offset(&mut model, body, -mm(0.6)) else {
        panic!("expected a collapse");
    };
    let c = geometry(&model, body);
    let vertical: Vec<EdgeId> = c
        .edges()
        .filter(|(_, e)| {
            let a = c.vertex(e.ends[0]).unwrap().position;
            let b = c.vertex(e.ends[1]).unwrap().position;
            a.z != b.z
        })
        .map(|(id, _)| id)
        .collect();
    assert_eq!(edges, vertical);
    // The four sides turn round; top and bottom only swap heights.
    let sides: Vec<FaceId> = c
        .faces()
        .filter(|(f, _)| c.face_plane(*f).unwrap().normal.z.abs() < 0.5)
        .map(|(f, _)| f)
        .collect();
    assert_eq!(faces, sides);
}

#[test]
fn shrink_by_exactly_half_thickness_collapses() {
    let mut model = Model::new();
    let body = box_part(&mut model, DVec3::ZERO, v(mm(10.0), mm(10.0), mm(1.0)));
    let result = offset(&mut model, body, -mm(0.5));
    assert!(
        matches!(&result, Err(OffsetError::Collapse { edges, .. }) if edges.len() == 4),
        "{result:?}"
    );
}

#[test]
fn shrink_leaving_less_than_the_tolerance_collapses() {
    // 0.005 mm left: thinner than two points can be apart.
    let mut model = Model::new();
    let body = box_part(&mut model, DVec3::ZERO, v(mm(10.0), mm(10.0), mm(1.0)));
    let result = offset(&mut model, body, -mm(0.4975));
    assert!(
        matches!(&result, Err(OffsetError::Collapse { edges, .. }) if edges.len() == 4),
        "{result:?}"
    );
}

#[test]
fn shrink_just_under_half_thickness_works() {
    let mut model = Model::new();
    let body = box_part(&mut model, DVec3::ZERO, v(mm(10.0), mm(10.0), mm(1.0)));
    let report = offset(&mut model, body, -mm(0.45)).unwrap();
    let want = mm(9.1) * mm(9.1) * mm(0.1);
    assert!(close(report.volume_after, want), "{report:?}");
}

/// A 30 x 30 mm square ring 10 mm high with a 2 mm slot cut through its right wall, as
/// one prism (the ring's hole is open to the outside through the slot).
fn slotted_ring(model: &mut Model) -> BodyId {
    let base: Vec<DVec2> = [
        (0.0, 0.0),
        (30.0, 0.0),
        (30.0, 14.0),
        (20.0, 14.0),
        (20.0, 10.0),
        (10.0, 10.0),
        (10.0, 20.0),
        (20.0, 20.0),
        (20.0, 16.0),
        (30.0, 16.0),
        (30.0, 30.0),
        (0.0, 30.0),
    ]
    .iter()
    .map(|&(x, y)| DVec2::new(mm(x), mm(y)))
    .collect();
    let (points, faces) = prism(&base, 0.0, mm(10.0));
    part(model, &points, &plain(&faces))
}

#[test]
fn slot_wider_than_twice_d_stays_open() {
    let mut model = Model::new();
    let body = slotted_ring(&mut model);
    let old = geometry(&model, body).clone();
    offset(&mut model, body, mm(0.5)).unwrap();
    planes_moved_by(&old, geometry(&model, body), mm(0.5)).unwrap();
}

#[test]
fn slot_closed_by_growing_is_refused() {
    let mut model = Model::new();
    let body = slotted_ring(&mut model);
    assert_eq!(
        offset(&mut model, body, mm(1.5)),
        Err(OffsetError::SelfIntersect)
    );
}

#[test]
fn boxes_growing_into_each_other_are_refused() {
    // Two boxes 0.3 mm apart in one part: growing each by 0.2 mm makes them overlap.
    let mut model = Model::new();
    let mut points = box_points(DVec3::ZERO, DVec3::splat(mm(10.0)));
    points.extend(box_points(
        v(mm(10.3), mm(2.0), mm(2.0)),
        v(mm(15.0), mm(8.0), mm(8.0)),
    ));
    let mut faces = box_faces(0);
    faces.extend(box_faces(8));
    let body = part(&mut model, &points, &plain(&faces));
    assert_eq!(offset(&mut model, body, D), Err(OffsetError::SelfIntersect));
    // Shrinking them is fine.
    offset(&mut model, body, -D).unwrap();
}

// ---------------------------------------------------------------- refusals

#[test]
fn zero_nan_and_infinite_distances_are_refused() {
    let mut model = Model::new();
    let body = ten_by_twenty_by_thirty(&mut model);
    for d in [0.0, -0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            offset(&mut model, body, d),
            Err(OffsetError::BadDistance),
            "{d}"
        );
    }
}

#[test]
fn missing_body_is_refused() {
    let mut model = Model::new();
    let body = BodyId::Definition(DefinitionId(99));
    assert_eq!(
        offset(&mut model, body, D),
        Err(OffsetError::MissingBody(body))
    );
}

#[test]
fn bad_distance_comes_before_missing_body() {
    let mut model = Model::new();
    let body = BodyId::Definition(DefinitionId(99));
    assert_eq!(offset(&mut model, body, 0.0), Err(OffsetError::BadDistance));
}

#[test]
fn open_box_is_not_solid() {
    let mut model = Model::new();
    let mut faces = box_faces(0);
    faces.remove(1);
    let points = box_points(DVec3::ZERO, DVec3::splat(mm(10.0)));
    let body = part(&mut model, &points, &plain(&faces));
    let check = check_body(&model, body).unwrap();
    assert_eq!(
        offset(&mut model, body, D),
        Err(OffsetError::NotSolid(Box::new(check)))
    );
}

#[test]
fn part_with_a_nested_group_is_not_solid() {
    let mut model = Model::new();
    let body = ten_by_twenty_by_thirty(&mut model);
    let mut edit = ModelEdit::new();
    let inner = model.add_definition(
        DefinitionInfo {
            name: "Inner".into(),
            group: true,
        },
        &mut edit,
    );
    model
        .add_instance(
            body,
            Instance {
                definition: inner,
                transform: DMat4::IDENTITY,
                name: String::new(),
                material: None,
            },
            &mut edit,
        )
        .unwrap();
    let check = check_body(&model, body).unwrap();
    assert_eq!(check.nested_instances, 1);
    assert_eq!(
        offset(&mut model, body, D),
        Err(OffsetError::NotSolid(Box::new(check)))
    );
}

#[test]
fn root_holding_only_groups_is_not_solid() {
    let mut model = Model::new();
    ten_by_twenty_by_thirty(&mut model);
    let result = offset(&mut model, BodyId::Root, D);
    assert!(
        matches!(result, Err(OffsetError::NotSolid(_))),
        "{result:?}"
    );
}

/// A hexagonal prism (radius 10 mm, 5 mm high) whose top rim is a circle curve.
fn hex_prism_with_curve(model: &mut Model) -> BodyId {
    let r = mm(10.0);
    let base: Vec<DVec2> = (0..6)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / 6.0;
            DVec2::new(r * a.cos(), r * a.sin())
        })
        .collect();
    let (points, faces) = prism(&base, 0.0, mm(5.0));
    let body = part(model, &points, &plain(&faces));
    let mut edit = ModelEdit::new();
    let id = model
        .edit_geometry(body, &mut edit, |ctx, log| {
            curve::add_curve(ctx, &points[6..], true, log)
        })
        .unwrap();
    assert!(id.is_some());
    body
}

#[test]
fn part_with_a_curve_is_refused() {
    let mut model = Model::new();
    let body = hex_prism_with_curve(&mut model);
    assert_eq!(offset(&mut model, body, D), Err(OffsetError::Curved(1)));
}

#[test]
fn not_solid_comes_before_curved() {
    let mut model = Model::new();
    let body = hex_prism_with_curve(&mut model);
    // Remove the bottom face: open and curved.
    let bottom = topology_of(&model, body).2[0].0;
    let mut edit = ModelEdit::new();
    model
        .edit_geometry(body, &mut edit, |ctx, log| ctx.delete_face(bottom, log))
        .unwrap()
        .unwrap();
    let result = offset(&mut model, body, D);
    assert!(
        matches!(result, Err(OffsetError::NotSolid(_))),
        "{result:?}"
    );
}

#[test]
fn unsupported_comes_before_collapse() {
    // A thin rectangular pyramid: the apex is unsupported and the shrink collapses it.
    let mut model = Model::new();
    let (a, b, h) = (mm(10.0), mm(20.0), mm(0.5));
    let points = vec![
        v(-a, -b, 0.0),
        v(a, -b, 0.0),
        v(a, b, 0.0),
        v(-a, b, 0.0),
        v(0.0, 0.0, h),
    ];
    let faces = vec![
        vec![0, 3, 2, 1],
        vec![0, 1, 4],
        vec![1, 2, 4],
        vec![2, 3, 4],
        vec![3, 0, 4],
    ];
    let body = part(&mut model, &points, &plain(&faces));
    let result = offset(&mut model, body, -mm(1.0));
    assert!(
        matches!(result, Err(OffsetError::Unsupported(_))),
        "{result:?}"
    );
}

// ---------------------------------------------------------------- offset_positions

#[test]
fn offset_positions_leaves_the_context_alone_and_lists_every_vertex() {
    let mut model = Model::new();
    let body = block_with_hole(&mut model, 6.0);
    let ctx = geometry(&model, body).clone();
    let moved = offset_positions(&ctx, D).unwrap();
    assert!(ctx == *geometry(&model, body));
    let ids: Vec<VertexId> = moved.keys().copied().collect();
    let want: Vec<VertexId> = ctx.vertices().map(|(id, _)| id).collect();
    assert_eq!(ids, want);
    let corner = ctx.find_vertex(DVec3::ZERO, 1e-9).unwrap();
    assert!(
        near(moved[&corner], DVec3::splat(-D)),
        "{:?}",
        moved[&corner]
    );
}

#[test]
fn offset_positions_of_an_open_surface_is_not_solid() {
    let mut model = Model::new();
    let mut faces = box_faces(0);
    faces.remove(0);
    let points = box_points(DVec3::ZERO, DVec3::splat(mm(10.0)));
    let body = part(&mut model, &points, &plain(&faces));
    let result = offset_positions(geometry(&model, body), D);
    assert!(
        matches!(result, Err(OffsetError::NotSolid(_))),
        "{result:?}"
    );
}

// ---------------------------------------------------------------- several parts, undo

#[test]
fn offset_bodies_is_one_undo_step() {
    let mut model = Model::new();
    let a = ten_by_twenty_by_thirty(&mut model);
    let b = box_part(&mut model, v(0.1, 0.0, 0.0), v(0.12, 0.02, 0.02));
    let before = model.clone();
    let mut undo = UndoStack::new();
    let results = offset_bodies(&mut model, &[a, b], D, &mut undo, "Grow by Clearance");
    assert!(results.iter().all(|r| r.is_ok()), "{results:?}");
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("Grow by Clearance"));
    let grown = model.clone();
    undo.undo(&mut model).unwrap();
    assert!(model == before);
    undo.redo(&mut model).unwrap();
    assert!(model == grown);
}

#[test]
fn offset_bodies_skips_the_parts_it_cannot_do() {
    let mut model = Model::new();
    let good = ten_by_twenty_by_thirty(&mut model);
    let mut faces = box_faces(0);
    faces.remove(1);
    let open = part(
        &mut model,
        &box_points(v(0.1, 0.0, 0.0), v(0.11, 0.01, 0.01)),
        &plain(&faces),
    );
    let open_before = positions(&model, open);
    let mut undo = UndoStack::new();
    let results = offset_bodies(&mut model, &[open, good], D, &mut undo, "Grow by Clearance");
    assert!(
        matches!(results[0], Err(OffsetError::NotSolid(_))),
        "{results:?}"
    );
    assert!(results[1].is_ok(), "{results:?}");
    assert_eq!(positions(&model, open), open_before);
    assert_eq!(undo.undo_count(), 1);
    let want = (mm(10.0) + 2.0 * D) * (mm(20.0) + 2.0 * D) * (mm(30.0) + 2.0 * D);
    assert!(close(volume(&model, good), want));
}

#[test]
fn offset_bodies_pushes_nothing_when_every_part_fails() {
    let mut model = Model::new();
    let body = box_part(&mut model, DVec3::ZERO, v(mm(10.0), mm(10.0), mm(1.0)));
    let before = model.clone();
    let mut undo = UndoStack::new();
    let results = offset_bodies(
        &mut model,
        &[body],
        -mm(0.6),
        &mut undo,
        "Shrink by Clearance",
    );
    assert!(
        matches!(results[0], Err(OffsetError::Collapse { .. })),
        "{results:?}"
    );
    assert_eq!(undo.undo_count(), 0);
    assert!(model == before);
    let results = offset_bodies(&mut model, &[], D, &mut undo, "Grow by Clearance");
    assert!(results.is_empty());
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn offset_bodies_offsets_a_repeated_body_once() {
    let mut model = Model::new();
    let body = ten_by_twenty_by_thirty(&mut model);
    let mut undo = UndoStack::new();
    let results = offset_bodies(&mut model, &[body, body], D, &mut undo, "Grow by Clearance");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], results[1]);
    let want = (mm(10.0) + 2.0 * D) * (mm(20.0) + 2.0 * D) * (mm(30.0) + 2.0 * D);
    assert!(close(volume(&model, body), want));
}

#[test]
fn offset_bodies_reports_bad_distance_for_each_part() {
    let mut model = Model::new();
    let body = ten_by_twenty_by_thirty(&mut model);
    let mut undo = UndoStack::new();
    let results = offset_bodies(&mut model, &[body], 0.0, &mut undo, "Grow by Clearance");
    assert_eq!(results, vec![Err(OffsetError::BadDistance)]);
    assert_eq!(undo.undo_count(), 0);
}
