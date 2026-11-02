// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Solid check (`docs/specs/m7-solid-check.md`).

use std::collections::BTreeMap;

use drydock_kernel::model::*;
use drydock_kernel::solid::{SolidReport, check_body, check_geometry, winding_number};
use drydock_kernel::topology::*;
use glam::{DMat4, DVec3};

mod transform_common;
use transform_common::*;

/// One face: its outer loop and its holes, as indices into the point list.
type FaceSpec<'a> = (&'a [usize], &'a [&'a [usize]]);

/// Builds faces (with holes) and loose edges in `body`; returns the face ids in order and
/// the edge ids keyed by their (low, high) point indices.
fn build_in(
    model: &mut Model,
    body: BodyId,
    points: &[DVec3],
    faces: &[FaceSpec],
    loose: &[(usize, usize)],
) -> (Vec<FaceId>, BTreeMap<(usize, usize), EdgeId>) {
    let mut edit = ModelEdit::new();
    let out = model
        .edit_geometry(body, &mut edit, |ctx, log| {
            let vs: Vec<VertexId> = points.iter().map(|&p| ctx.insert_vertex(p, log)).collect();
            let mut edges: BTreeMap<(usize, usize), EdgeId> = BTreeMap::new();
            let mut lp = |ctx: &mut Context, log: &mut Edit, idx: &[usize]| {
                let mut uses = Vec::new();
                for i in 0..idx.len() {
                    let (a, b) = (idx[i], idx[(i + 1) % idx.len()]);
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
                Loop { uses }
            };
            let mut ids = Vec::new();
            for (outer, holes) in faces {
                let outer = lp(ctx, log, outer);
                let inner = holes.iter().map(|h| lp(ctx, log, h)).collect();
                ids.push(ctx.insert_face(Face { outer, inner }, log).unwrap());
            }
            for &(a, b) in loose {
                let key = (a.min(b), a.max(b));
                edges.insert(key, ctx.insert_edge(vs[key.0], vs[key.1], log).unwrap());
            }
            (ids, edges)
        })
        .unwrap();
    let violations = model.body(body).unwrap().geometry().validate();
    assert!(violations.is_empty(), "{violations:?}");
    out
}

fn build(
    model: &mut Model,
    points: &[DVec3],
    faces: &[FaceSpec],
) -> (Vec<FaceId>, BTreeMap<(usize, usize), EdgeId>) {
    build_in(model, BodyId::Root, points, faces, &[])
}

/// The 8 corners of the box `lo..hi`, numbered like `transform_common::cube`.
fn box_points(lo: DVec3, hi: DVec3) -> Vec<DVec3> {
    vec![
        v(lo.x, lo.y, lo.z),
        v(hi.x, lo.y, lo.z),
        v(hi.x, hi.y, lo.z),
        v(lo.x, hi.y, lo.z),
        v(lo.x, lo.y, hi.z),
        v(hi.x, lo.y, hi.z),
        v(hi.x, hi.y, hi.z),
        v(lo.x, hi.y, hi.z),
    ]
}

/// Box faces with outward fronts (bottom, top, front, back, right, left), offset by `base`.
fn box_faces(base: usize) -> Vec<Vec<usize>> {
    let f: [[usize; 4]; 6] = [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [2, 3, 7, 6],
        [1, 2, 6, 5],
        [3, 0, 4, 7],
    ];
    f.iter()
        .map(|q| q.iter().map(|i| i + base).collect())
        .collect()
}

fn reversed(face: &[usize]) -> Vec<usize> {
    face.iter().rev().copied().collect()
}

fn specs(faces: &[Vec<usize>]) -> Vec<FaceSpec<'_>> {
    faces.iter().map(|f| (f.as_slice(), &[][..])).collect()
}

fn root_report(model: &Model) -> SolidReport {
    check_geometry(model.root().geometry())
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9 * (1.0 + b.abs())
}

fn sorted<T: Ord + Clone>(v: &[T]) -> Vec<T> {
    let mut v = v.to_vec();
    v.sort();
    v
}

// ---------------------------------------------------------------- closed boxes

#[test]
fn unit_cube_is_a_clean_solid() {
    let mut model = Model::new();
    cube(&mut model);
    let r = root_report(&model);
    assert_eq!(r.shells, 1);
    assert!(r.open_edges.is_empty());
    assert!(r.loose_edges.is_empty());
    assert!(r.shared_edges.is_empty());
    assert!(r.internal_faces.is_empty());
    assert!(r.reversed_faces.is_empty());
    assert_eq!(r.nested_instances, 0);
    assert!(r.is_solid());
    assert!(r.is_clean());
    assert!(close(r.volume.unwrap(), 1.0), "{:?}", r.volume);
}

#[test]
fn box_volume_is_width_depth_height() {
    let mut model = Model::new();
    let pts = box_points(v(-1.0, 2.0, 0.5), v(1.0, 5.0, 4.5));
    let faces = box_faces(0);
    build(&mut model, &pts, &specs(&faces));
    let r = root_report(&model);
    assert!(r.is_clean());
    assert!(close(r.volume.unwrap(), 2.0 * 3.0 * 4.0), "{:?}", r.volume);
}

#[test]
fn check_leaves_the_model_alone() {
    let mut model = Model::new();
    cube(&mut model);
    let before = model.clone();
    let rev = model.revision();
    let _ = root_report(&model);
    let _ = check_body(&model, BodyId::Root);
    assert_eq!(model, before);
    assert_eq!(model.revision(), rev);
}

#[test]
fn rotated_tetrahedron_volume() {
    let mut model = Model::new();
    let m = DMat4::from_rotation_x(0.7) * DMat4::from_rotation_z(1.3);
    let pts: Vec<DVec3> = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(0.0, 1.0, 0.0),
        v(0.0, 0.0, 1.0),
    ]
    .iter()
    .map(|p| m.transform_point3(*p) + v(3.0, -2.0, 1.0))
    .collect();
    let faces: Vec<Vec<usize>> = vec![vec![0, 2, 1], vec![0, 1, 3], vec![1, 2, 3], vec![0, 3, 2]];
    build(&mut model, &pts, &specs(&faces));
    let r = root_report(&model);
    assert!(r.is_clean(), "{r:?}");
    assert!(close(r.volume.unwrap(), 1.0 / 6.0), "{:?}", r.volume);
}

/// An L-shaped prism: the two end faces are concave hexagons.
#[test]
fn concave_prism_volume() {
    let mut model = Model::new();
    let l = [
        (0.0, 0.0),
        (3.0, 0.0),
        (3.0, 1.0),
        (1.0, 1.0),
        (1.0, 2.0),
        (0.0, 2.0),
    ];
    let mut pts: Vec<DVec3> = l.iter().map(|&(x, y)| v(x, y, 0.0)).collect();
    pts.extend(l.iter().map(|&(x, y)| v(x, y, 2.0)));
    let mut faces = vec![vec![5, 4, 3, 2, 1, 0], vec![6, 7, 8, 9, 10, 11]];
    for i in 0..6 {
        let j = (i + 1) % 6;
        faces.push(vec![i, j, j + 6, i + 6]);
    }
    build(&mut model, &pts, &specs(&faces));
    let r = root_report(&model);
    assert!(r.is_clean(), "{r:?}");
    assert!(close(r.volume.unwrap(), 4.0 * 2.0), "{:?}", r.volume);
}

/// A 3 x 3 x 1 slab with a 1 x 1 square hole through the middle: top and bottom faces have
/// inner loops.
#[test]
fn faces_with_holes_count_in_the_volume() {
    let mut model = Model::new();
    let mut pts = Vec::new();
    for z in [0.0, 1.0] {
        for (x, y) in [(0.0, 0.0), (3.0, 0.0), (3.0, 3.0), (0.0, 3.0)] {
            pts.push(v(x, y, z));
        }
        for (x, y) in [(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0)] {
            pts.push(v(x, y, z));
        }
    }
    // 0..4 outer bottom, 4..8 hole bottom, 8..12 outer top, 12..16 hole top.
    let bottom_hole: &[usize] = &[4, 5, 6, 7];
    let top_hole: &[usize] = &[15, 14, 13, 12];
    let bottom_holes = [bottom_hole];
    let top_holes = [top_hole];
    let mut faces: Vec<FaceSpec> = vec![
        (&[0, 3, 2, 1], &bottom_holes),
        (&[8, 9, 10, 11], &top_holes),
    ];
    let mut sides = Vec::new();
    for i in 0..4 {
        let j = (i + 1) % 4;
        // Outer walls face out.
        sides.push(vec![i, j, j + 8, i + 8]);
        // Walls of the hole face into the hole (towards its axis).
        sides.push(vec![4 + j, 4 + i, 12 + i, 12 + j]);
    }
    faces.extend(sides.iter().map(|f| (f.as_slice(), &[][..])));
    build(&mut model, &pts, &faces);
    let r = root_report(&model);
    assert_eq!(r.shells, 1);
    assert!(r.is_clean(), "{r:?}");
    assert!(close(r.volume.unwrap(), 9.0 - 1.0), "{:?}", r.volume);
}

#[test]
fn two_separate_boxes_are_one_solid_with_two_shells() {
    let mut model = Model::new();
    let mut pts = box_points(v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    pts.extend(box_points(v(3.0, 0.0, 0.0), v(5.0, 1.0, 1.0)));
    let mut faces = box_faces(0);
    faces.extend(box_faces(8));
    build(&mut model, &pts, &specs(&faces));
    let r = root_report(&model);
    assert_eq!(r.shells, 2);
    assert!(r.is_clean(), "{r:?}");
    assert!(close(r.volume.unwrap(), 1.0 + 2.0), "{:?}", r.volume);
}

// ---------------------------------------------------------------- reversed faces

#[test]
fn one_flipped_face_is_reported() {
    let mut model = Model::new();
    let mut faces = box_faces(0);
    faces[4] = reversed(&faces[4]);
    let (ids, _) = build(
        &mut model,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&faces),
    );
    let r = root_report(&model);
    assert_eq!(r.reversed_faces, vec![ids[4]]);
    assert!(r.is_solid());
    assert!(!r.is_clean());
    assert!(close(r.volume.unwrap(), 1.0), "{:?}", r.volume);
}

#[test]
fn inside_out_box_has_every_face_reversed() {
    let mut model = Model::new();
    let faces: Vec<Vec<usize>> = box_faces(0).iter().map(|f| reversed(f)).collect();
    let (ids, _) = build(
        &mut model,
        &box_points(DVec3::ZERO, v(2.0, 1.0, 1.0)),
        &specs(&faces),
    );
    let r = root_report(&model);
    assert_eq!(r.reversed_faces, sorted(&ids));
    assert!(r.is_solid());
    assert!(close(r.volume.unwrap(), 2.0), "{:?}", r.volume);
}

/// The closed-shell rule follows the volume sign, not the majority: four of six flipped.
#[test]
fn most_faces_flipped_still_finds_the_flipped_ones() {
    let mut model = Model::new();
    let mut faces = box_faces(0);
    for i in [0, 1, 2, 5] {
        faces[i] = reversed(&faces[i]);
    }
    let (ids, _) = build(
        &mut model,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&faces),
    );
    let r = root_report(&model);
    assert_eq!(r.reversed_faces, sorted(&[ids[0], ids[1], ids[2], ids[5]]));
    assert!(close(r.volume.unwrap(), 1.0));
}

// ---------------------------------------------------------------- cavities

/// A 3 m box with a 1 m cavity whose walls face into the cavity (out of the material).
#[test]
fn cavity_facing_into_itself_is_clean() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, v(3.0, 3.0, 3.0));
    pts.extend(box_points(DVec3::ONE, v(2.0, 2.0, 2.0)));
    let mut faces = box_faces(0);
    faces.extend(box_faces(8).iter().map(|f| reversed(f)));
    build(&mut model, &pts, &specs(&faces));
    let r = root_report(&model);
    assert_eq!(r.shells, 2);
    assert!(r.is_clean(), "{r:?}");
    assert!(close(r.volume.unwrap(), 27.0 - 1.0), "{:?}", r.volume);
}

#[test]
fn cavity_facing_out_of_itself_is_reversed() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, v(3.0, 3.0, 3.0));
    pts.extend(box_points(DVec3::ONE, v(2.0, 2.0, 2.0)));
    let mut faces = box_faces(0);
    faces.extend(box_faces(8));
    let (ids, _) = build(&mut model, &pts, &specs(&faces));
    let r = root_report(&model);
    assert_eq!(r.reversed_faces, sorted(&ids[6..]));
    assert!(r.is_solid());
    assert!(close(r.volume.unwrap(), 26.0), "{:?}", r.volume);
}

/// A solid cube floating in the cavity of a hollow cube (depth 2) faces out again.
#[test]
fn solid_inside_a_cavity_faces_out() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, v(5.0, 5.0, 5.0));
    pts.extend(box_points(DVec3::ONE, v(4.0, 4.0, 4.0)));
    pts.extend(box_points(v(2.0, 2.0, 2.0), v(3.0, 3.0, 3.0)));
    let mut faces = box_faces(0);
    faces.extend(box_faces(8).iter().map(|f| reversed(f)));
    faces.extend(box_faces(16).iter().map(|f| reversed(f)));
    let (ids, _) = build(&mut model, &pts, &specs(&faces));
    let r = root_report(&model);
    assert_eq!(r.shells, 3);
    assert_eq!(r.reversed_faces, sorted(&ids[12..]));
    assert!(
        close(r.volume.unwrap(), 125.0 - 27.0 + 1.0),
        "{:?}",
        r.volume
    );
}

// ---------------------------------------------------------------- open surfaces

#[test]
fn missing_face_leaves_open_edges() {
    let mut model = Model::new();
    let faces = box_faces(0);
    let (_, edges) = build(
        &mut model,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&faces[..5]),
    );
    let r = root_report(&model);
    // The left face (3, 0, 4, 7) is missing.
    let want = sorted(&[
        edges[&(0, 3)],
        edges[&(0, 4)],
        edges[&(4, 7)],
        edges[&(3, 7)],
    ]);
    assert_eq!(r.open_edges, want);
    assert_eq!(r.shells, 1);
    assert!(r.reversed_faces.is_empty());
    assert!(!r.is_solid());
    assert!(!r.is_clean());
    assert_eq!(r.volume, None);
}

#[test]
fn single_face_is_open_all_round() {
    let mut model = Model::new();
    let (_, edges) = build(
        &mut model,
        &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0)],
        &[(&[0, 1, 2], &[])],
    );
    let r = root_report(&model);
    assert_eq!(r.shells, 1);
    assert_eq!(
        r.open_edges,
        sorted(&edges.values().copied().collect::<Vec<_>>())
    );
    assert!(!r.is_solid());
    assert_eq!(r.volume, None);
}

/// Open box (no lid) with its floor flipped: the floor has less area than the walls.
#[test]
fn open_shell_keeps_the_larger_side() {
    let mut model = Model::new();
    let mut faces = box_faces(0);
    faces.remove(1); // lid
    faces[0] = reversed(&faces[0]); // floor
    let (ids, _) = build(
        &mut model,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&faces),
    );
    let r = root_report(&model);
    assert_eq!(r.reversed_faces, vec![ids[0]]);
}

/// Open box with all four walls flipped: now the walls are the larger side, so the floor
/// is the one reported.
#[test]
fn open_shell_larger_side_wins_over_the_lowest_face() {
    let mut model = Model::new();
    let mut faces = box_faces(0);
    faces.remove(1);
    for f in faces.iter_mut().skip(1) {
        *f = reversed(f);
    }
    let (ids, _) = build(
        &mut model,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&faces),
    );
    let r = root_report(&model);
    assert_eq!(r.reversed_faces, vec![ids[0]]);
}

/// Two equal squares sharing an edge, run the same way along it: a tie keeps the lowest
/// face's side.
#[test]
fn open_shell_tie_keeps_the_lowest_face() {
    let mut model = Model::new();
    let pts = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(2.0, 0.0, 0.0),
        v(2.0, 1.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
    ];
    let (ids, _) = build(
        &mut model,
        &pts,
        &[(&[0, 1, 4, 5], &[]), (&[1, 4, 3, 2], &[])],
    );
    let r = root_report(&model);
    assert_eq!(r.shells, 1);
    assert_eq!(r.reversed_faces, vec![ids[0].max(ids[1])]);
}

#[test]
fn loose_edge_is_reported() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, DVec3::ONE);
    pts.push(v(3.0, 3.0, 3.0));
    let faces = box_faces(0);
    let (_, edges) = build_in(&mut model, BodyId::Root, &pts, &specs(&faces), &[(6, 8)]);
    let r = root_report(&model);
    assert_eq!(r.loose_edges, vec![edges[&(6, 8)]]);
    assert!(r.open_edges.is_empty());
    assert!(!r.is_solid());
    assert_eq!(r.volume, None);
}

#[test]
fn empty_context_is_not_solid() {
    let model = Model::new();
    let r = root_report(&model);
    assert_eq!(r.shells, 0);
    assert_eq!(r, SolidReport::default());
    assert!(!r.is_solid());
    assert!(!r.is_clean());
}

// ---------------------------------------------------------------- shared edges, walls

/// Two unit boxes stacked side by side sharing the face x = 1: that face is internal.
#[test]
fn shared_wall_is_internal() {
    let mut model = Model::new();
    let pts = [
        v(0.0, 0.0, 0.0), // 0
        v(1.0, 0.0, 0.0), // 1
        v(2.0, 0.0, 0.0), // 2
        v(0.0, 1.0, 0.0), // 3
        v(1.0, 1.0, 0.0), // 4
        v(2.0, 1.0, 0.0), // 5
        v(0.0, 0.0, 1.0), // 6
        v(1.0, 0.0, 1.0), // 7
        v(2.0, 0.0, 1.0), // 8
        v(0.0, 1.0, 1.0), // 9
        v(1.0, 1.0, 1.0), // 10
        v(2.0, 1.0, 1.0), // 11
    ];
    let faces: Vec<Vec<usize>> = vec![
        vec![0, 3, 4, 1],   // bottom left
        vec![1, 4, 5, 2],   // bottom right
        vec![6, 7, 10, 9],  // top left
        vec![7, 8, 11, 10], // top right
        vec![0, 1, 7, 6],   // front left
        vec![1, 2, 8, 7],   // front right
        vec![4, 3, 9, 10],  // back left
        vec![5, 4, 10, 11], // back right
        vec![3, 0, 6, 9],   // left end
        vec![2, 5, 11, 8],  // right end
        vec![1, 4, 10, 7],  // the wall
    ];
    let (ids, edges) = build(&mut model, &pts, &specs(&faces));
    let r = root_report(&model);
    assert_eq!(r.internal_faces, vec![ids[10]]);
    let want = sorted(&[
        edges[&(1, 4)],
        edges[&(4, 10)],
        edges[&(7, 10)],
        edges[&(1, 7)],
    ]);
    assert_eq!(r.shared_edges, want);
    assert!(r.open_edges.is_empty());
    // Left half, right half and the wall: the wall's edges join nothing.
    assert_eq!(r.shells, 3);
    assert!(r.reversed_faces.is_empty(), "{:?}", r.reversed_faces);
    assert!(!r.is_solid());
    assert_eq!(r.volume, None);
}

/// Two boxes touching along one vertical edge: four faces use it.
#[test]
fn boxes_touching_along_an_edge_share_it() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, DVec3::ONE);
    pts.extend(box_points(v(1.0, 1.0, 0.0), v(2.0, 2.0, 1.0)));
    // Merge the touching corners so the edge (1,1,0)-(1,1,1) is one edge.
    // In the second box, corner 0 is (1,1,0) = first box's 2, corner 4 is (1,1,1) = 6.
    let remap = |i: usize| match i {
        8 => 2,
        12 => 6,
        i if i > 12 => i - 2,
        i if i > 8 => i - 1,
        i => i,
    };
    pts.remove(12);
    pts.remove(8);
    let mut faces = box_faces(0);
    faces.extend(
        box_faces(8)
            .iter()
            .map(|f| f.iter().map(|&i| remap(i)).collect::<Vec<_>>()),
    );
    let (_, edges) = build(&mut model, &pts, &specs(&faces));
    let r = root_report(&model);
    assert_eq!(r.shared_edges, vec![edges[&(2, 6)]]);
    assert!(r.internal_faces.is_empty());
    assert_eq!(r.shells, 2);
    assert!(r.reversed_faces.is_empty());
    assert!(!r.is_solid());
}

// ---------------------------------------------------------------- report helpers

#[test]
fn is_solid_reads_the_fields() {
    let mut r = SolidReport {
        shells: 1,
        volume: Some(1.0),
        ..SolidReport::default()
    };
    assert!(r.is_solid() && r.is_clean());
    r.reversed_faces = vec![FaceId(3)];
    assert!(r.is_solid() && !r.is_clean());
    r.reversed_faces.clear();
    for change in 0..5 {
        let mut s = r.clone();
        match change {
            0 => s.open_edges = vec![EdgeId(1)],
            1 => s.loose_edges = vec![EdgeId(1)],
            2 => s.shared_edges = vec![EdgeId(1)],
            3 => s.internal_faces = vec![FaceId(1)],
            _ => s.nested_instances = 1,
        }
        assert!(!s.is_solid(), "{change}");
        assert!(!s.is_clean(), "{change}");
    }
    r.shells = 0;
    assert!(!r.is_solid());
}

// ---------------------------------------------------------------- bodies

fn group_with_cube(model: &mut Model) -> DefinitionId {
    let mut edit = ModelEdit::new();
    let def = model.add_definition(
        DefinitionInfo {
            name: "Part".into(),
            group: true,
        },
        &mut edit,
    );
    let faces = box_faces(0);
    build_in(
        model,
        BodyId::Definition(def),
        &box_points(DVec3::ZERO, v(0.01, 0.02, 0.03)),
        &specs(&faces),
        &[],
    );
    model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: def,
                transform: DMat4::IDENTITY,
                name: String::new(),
                material: None,
            },
            &mut edit,
        )
        .unwrap();
    def
}

#[test]
fn check_body_reports_a_definition() {
    let mut model = Model::new();
    let def = group_with_cube(&mut model);
    let r = check_body(&model, BodyId::Definition(def)).unwrap();
    assert!(r.is_clean(), "{r:?}");
    assert_eq!(r.nested_instances, 0);
    assert!(
        close(r.volume.unwrap(), 0.01 * 0.02 * 0.03),
        "{:?}",
        r.volume
    );
    // The root holds only the instance: no faces, one nested instance.
    let root = check_body(&model, BodyId::Root).unwrap();
    assert_eq!(root.shells, 0);
    assert_eq!(root.nested_instances, 1);
    assert!(!root.is_solid());
}

#[test]
fn nested_instance_stops_a_solid() {
    let mut model = Model::new();
    let inner = group_with_cube(&mut model);
    let mut edit = ModelEdit::new();
    let outer = model.add_definition(
        DefinitionInfo {
            name: "Outer".into(),
            group: true,
        },
        &mut edit,
    );
    build_in(
        &mut model,
        BodyId::Definition(outer),
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&box_faces(0)),
        &[],
    );
    model
        .add_instance(
            BodyId::Definition(outer),
            Instance {
                definition: inner,
                transform: DMat4::from_translation(v(5.0, 0.0, 0.0)),
                name: String::new(),
                material: None,
            },
            &mut edit,
        )
        .unwrap();
    let r = check_body(&model, BodyId::Definition(outer)).unwrap();
    assert_eq!(r.nested_instances, 1);
    assert_eq!(r.shells, 1);
    assert!(r.open_edges.is_empty() && r.reversed_faces.is_empty());
    assert!(!r.is_solid());
    assert_eq!(r.volume, None);
}

#[test]
fn check_body_of_a_missing_body_is_none() {
    let model = Model::new();
    assert_eq!(
        check_body(&model, BodyId::Definition(DefinitionId(99))),
        None
    );
}

// ---------------------------------------------------------------- winding number

#[test]
fn winding_number_inside_and_outside() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let ctx = model.root().geometry();
    let w_in = winding_number(ctx, &faces, v(0.3, 0.6, 0.5));
    let w_out = winding_number(ctx, &faces, v(1.5, 0.5, 0.5));
    let w_far = winding_number(ctx, &faces, v(-40.0, 7.0, 3.0));
    assert!((w_in - 1.0).abs() < 1e-9, "{w_in}");
    assert!(w_out.abs() < 1e-9, "{w_out}");
    assert!(w_far.abs() < 1e-9, "{w_far}");
}

#[test]
fn winding_number_of_an_inside_out_box_is_minus_one() {
    let mut model = Model::new();
    let faces: Vec<Vec<usize>> = box_faces(0).iter().map(|f| reversed(f)).collect();
    let (ids, _) = build(
        &mut model,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&faces),
    );
    let w = winding_number(model.root().geometry(), &ids, v(0.5, 0.5, 0.5));
    assert!((w + 1.0).abs() < 1e-9, "{w}");
}

#[test]
fn winding_number_counts_only_listed_faces_and_skips_unknown_ids() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let ctx = model.root().geometry();
    // The bottom face alone (front down), seen from just above its centre: the point sees
    // its back, as from inside the cube, so it covers almost half the sphere, positively.
    let w = winding_number(ctx, &faces[..1], v(0.5, 0.5, 1e-4));
    assert!(w > 0.49 && w < 0.5, "{w}");
    let mut with_unknown = faces.clone();
    with_unknown.push(FaceId(9999));
    let w = winding_number(ctx, &with_unknown, v(0.3, 0.6, 0.5));
    assert!((w - 1.0).abs() < 1e-9, "{w}");
}

/// Holes subtract from a face's solid angle.
#[test]
fn winding_number_of_a_face_with_a_hole() {
    let mut model = Model::new();
    let pts = [
        v(-2.0, -2.0, 0.0),
        v(2.0, -2.0, 0.0),
        v(2.0, 2.0, 0.0),
        v(-2.0, 2.0, 0.0),
        v(-1.0, -1.0, 0.0),
        v(-1.0, 1.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(1.0, -1.0, 0.0),
    ];
    let hole: &[usize] = &[4, 5, 6, 7];
    let holes = [hole];
    let (ids, _) = build(&mut model, &pts, &[(&[0, 1, 2, 3], &holes)]);
    // Above the hole's centre the ring covers less than the full square would. The point
    // sees the front, so the sign is negative.
    let ctx = model.root().geometry();
    let ring = winding_number(ctx, &ids, v(0.0, 0.0, 1.0));
    // Solid angle of an a x b rectangle centred under a point at height h:
    // 4 asin(a b / sqrt((a^2 + 4h^2)(b^2 + 4h^2))).
    let rect = |s: f64| 4.0 * (s * s / ((s * s + 4.0) * (s * s + 4.0)).sqrt()).asin();
    let want = -(rect(4.0) - rect(2.0)) / (4.0 * std::f64::consts::PI);
    assert!((ring - want).abs() < 1e-9, "{ring} vs {want}");
}
