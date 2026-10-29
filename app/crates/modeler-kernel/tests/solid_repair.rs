// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Solid repair (`docs/specs/m7-solid-repair.md`).

use std::collections::{BTreeMap, BTreeSet};

use glam::{DMat4, DVec3, DVec4};
use modeler_kernel::model::*;
use modeler_kernel::repair::{RepairError, RepairReport, Unfixed, repair_body, repair_geometry};
use modeler_kernel::solid::{check_body, check_geometry};
use modeler_kernel::texture::TextureMap;
use modeler_kernel::topology::*;

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

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9 * (1.0 + b.abs())
}

fn sorted<T: Ord + Clone>(v: &[T]) -> Vec<T> {
    let mut v = v.to_vec();
    v.sort();
    v
}

/// Repairs the root with a fresh undo stack and checks what every repair must keep: a valid
/// model, `after` equal to a new check, at most one undo step.
fn repair_root(model: &mut Model) -> (RepairReport, UndoStack) {
    let mut undo = UndoStack::new();
    let report = repair_body(model, BodyId::Root, &mut undo, "Repair Solid").unwrap();
    let violations = model.validate();
    assert!(violations.is_empty(), "{violations:?}");
    assert_eq!(Some(report.after.clone()), check_body(model, BodyId::Root));
    assert_eq!(undo.undo_count(), usize::from(report.changed()));
    (report, undo)
}

fn face_normal(model: &Model, f: FaceId) -> DVec3 {
    model.root().geometry().face_plane(f).unwrap().normal
}

fn vertex_at(model: &Model, p: DVec3) -> bool {
    model.root().geometry().find_vertex(p, 1e-9).is_some()
}

fn face_edge_set(ctx: &Context, f: FaceId) -> BTreeSet<EdgeId> {
    let face = ctx.face(f).unwrap();
    std::iter::once(&face.outer)
        .chain(face.inner.iter())
        .flat_map(|l| l.uses.iter().map(|u| u.edge))
        .collect()
}

// ---------------------------------------------------------------- nothing to do

#[test]
fn clean_cube_is_left_alone() {
    let mut model = Model::new();
    cube(&mut model);
    let before = model.clone();
    let revision = model.revision();
    let (r, undo) = repair_root(&mut model);
    assert!(!r.changed());
    assert_eq!(r.unfixed, vec![]);
    assert!(r.after.is_clean());
    assert!(close(r.after.volume.unwrap(), 1.0));
    assert_eq!(model, before);
    assert_eq!(model.revision(), revision);
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn empty_root_is_left_alone() {
    let mut model = Model::new();
    let before = model.clone();
    let (r, undo) = repair_root(&mut model);
    assert!(!r.changed());
    assert_eq!(r.unfixed, vec![]);
    assert_eq!(r.after.shells, 0);
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn changed_reads_the_lists() {
    let mut r = RepairReport::default();
    assert!(!r.changed());
    r.unfixed = vec![Unfixed::SharedEdge(EdgeId(1))];
    assert!(!r.changed());
    for i in 0..5 {
        let mut s = RepairReport::default();
        match i {
            0 => s.removed_internal = vec![FaceId(1)],
            1 => s.removed_stray = vec![FaceId(1)],
            2 => s.removed_edges = vec![EdgeId(1)],
            3 => s.filled = vec![FaceId(1)],
            _ => s.reversed = vec![FaceId(1)],
        }
        assert!(s.changed(), "{i}");
    }
}

// ---------------------------------------------------------------- reversed faces

#[test]
fn flipped_face_is_turned_round_and_keeps_its_id() {
    let mut model = Model::new();
    let mut faces = box_faces(0);
    faces[4] = reversed(&faces[4]);
    let (ids, _) = build(
        &mut model,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&faces),
    );
    let (r, undo) = repair_root(&mut model);
    assert_eq!(r.reversed, vec![ids[4]]);
    assert!(r.removed_internal.is_empty() && r.removed_stray.is_empty());
    assert!(r.removed_edges.is_empty() && r.filled.is_empty());
    assert_eq!(r.unfixed, vec![]);
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert!(close(r.after.volume.unwrap(), 1.0));
    assert_eq!(model.root().geometry().face_count(), 6);
    assert!(face_normal(&model, ids[4]).abs_diff_eq(DVec3::X, 1e-9));
    assert!(normals_outward(&model));
    assert_eq!(undo.undo_label(), Some("Repair Solid"));
}

#[test]
fn inside_out_box_is_turned_right_side_out() {
    let mut model = Model::new();
    let faces: Vec<Vec<usize>> = box_faces(0).iter().map(|f| reversed(f)).collect();
    let (ids, _) = build(
        &mut model,
        &box_points(DVec3::ZERO, v(2.0, 1.0, 1.0)),
        &specs(&faces),
    );
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.reversed, sorted(&ids));
    assert!(r.after.is_clean());
    assert!(close(r.after.volume.unwrap(), 2.0));
    assert!(normals_outward(&model));
}

#[test]
fn cavity_facing_the_wrong_way_is_turned_into_itself() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, v(3.0, 3.0, 3.0));
    pts.extend(box_points(DVec3::ONE, v(2.0, 2.0, 2.0)));
    let mut faces = box_faces(0);
    faces.extend(box_faces(8));
    let (ids, _) = build(&mut model, &pts, &specs(&faces));
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.reversed, sorted(&ids[6..]));
    assert!(r.after.is_clean());
    assert_eq!(r.after.shells, 2);
    assert!(close(r.after.volume.unwrap(), 26.0));
    // The cavity's bottom now faces up, into the cavity.
    assert!(face_normal(&model, ids[6]).abs_diff_eq(DVec3::Z, 1e-9));
}

#[test]
fn turned_face_keeps_its_paint_on_the_same_side() {
    let mut model = Model::new();
    let mut faces = box_faces(0);
    faces[4] = reversed(&faces[4]);
    let (ids, _) = build(
        &mut model,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&faces),
    );
    let mut edit = ModelEdit::new();
    let red = model.add_material(Material::new("Red", [1.0, 0.0, 0.0, 1.0]), &mut edit);
    let blue = model.add_material(Material::new("Blue", [0.0, 0.0, 1.0, 1.0]), &mut edit);
    let outside_map = TextureMap {
        u: DVec4::new(0.0, 1.0, 0.0, 0.5),
        v: DVec4::new(0.0, 0.0, 1.0, 0.0),
    };
    // Painted from outside: the outside of the flipped face is its back.
    let painted = FaceMaterials {
        front: Some(blue),
        back: Some(red),
        front_map: None,
        back_map: Some(outside_map),
    };
    model
        .set_face_materials(BodyId::Root, ids[4], painted, &mut edit)
        .unwrap();
    let (r, mut undo) = repair_root(&mut model);
    assert_eq!(r.reversed, vec![ids[4]]);
    let now = model.root().face_materials(ids[4]);
    assert_eq!(
        now,
        FaceMaterials {
            front: Some(red),
            back: Some(blue),
            front_map: Some(outside_map),
            back_map: None,
        }
    );
    // Unpainted faces stay unpainted.
    assert_eq!(
        model.root().face_materials(ids[0]),
        FaceMaterials::default()
    );
    undo.undo(&mut model).unwrap();
    assert_eq!(model.root().face_materials(ids[4]), painted);
}

// ---------------------------------------------------------------- internal faces

/// Two unit boxes side by side sharing the face x = 1.
fn two_boxes_with_wall() -> (Vec<DVec3>, Vec<Vec<usize>>) {
    let pts = vec![
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
    (pts, faces)
}

#[test]
fn wall_between_two_boxes_is_removed() {
    let mut model = Model::new();
    let (pts, faces) = two_boxes_with_wall();
    let (ids, edges) = build(&mut model, &pts, &specs(&faces));
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.removed_internal, vec![ids[10]]);
    assert!(r.removed_stray.is_empty());
    // The wall's edges still border two faces each.
    assert!(r.removed_edges.is_empty());
    assert!(model.root().geometry().edge(edges[&(1, 4)]).is_some());
    assert!(r.reversed.is_empty(), "{:?}", r.reversed);
    assert_eq!(r.unfixed, vec![]);
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert_eq!(r.after.shells, 1);
    assert!(close(r.after.volume.unwrap(), 2.0));
    assert!(model.root().geometry().face(ids[10]).is_none());
}

/// Removing the wall joins the halves; a flipped face in one half is then turned with the
/// rest.
#[test]
fn wall_removed_then_flipped_face_turned() {
    let mut model = Model::new();
    let (pts, mut faces) = two_boxes_with_wall();
    faces[3] = reversed(&faces[3]);
    let (ids, _) = build(&mut model, &pts, &specs(&faces));
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.removed_internal, vec![ids[10]]);
    assert_eq!(r.reversed, vec![ids[3]]);
    assert!(r.after.is_clean());
    assert!(close(r.after.volume.unwrap(), 2.0));
}

// ---------------------------------------------------------------- stray faces and edges

#[test]
fn loose_face_beside_a_box_is_removed() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, DVec3::ONE);
    pts.extend([
        v(3.0, 0.0, 0.0),
        v(4.0, 0.0, 0.0),
        v(4.0, 1.0, 0.0),
        v(3.0, 1.0, 0.0),
    ]);
    let mut faces = box_faces(0);
    faces.push(vec![8, 9, 10, 11]);
    let (ids, edges) = build(&mut model, &pts, &specs(&faces));
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.removed_stray, vec![ids[6]]);
    let want = sorted(&[
        edges[&(8, 9)],
        edges[&(9, 10)],
        edges[&(10, 11)],
        edges[&(8, 11)],
    ]);
    assert_eq!(r.removed_edges, want);
    assert!(r.filled.is_empty());
    assert_eq!(r.unfixed, vec![]);
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert!(!vertex_at(&model, v(3.0, 0.0, 0.0)));
    assert_eq!(model.root().geometry().vertex_count(), 8);
}

/// A fin hanging off one edge of a box: that edge has three uses, the fin's other edges one.
#[test]
fn fin_on_a_box_edge_is_removed() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, DVec3::ONE);
    pts.extend([v(2.0, -1.0, 1.0), v(2.0, -1.0, 0.0)]);
    let mut faces = box_faces(0);
    faces.push(vec![1, 9, 8, 5]);
    let (ids, edges) = build(&mut model, &pts, &specs(&faces));
    assert_eq!(
        check_geometry(model.root().geometry()).shared_edges,
        vec![edges[&(1, 5)]]
    );
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.removed_stray, vec![ids[6]]);
    assert_eq!(
        r.removed_edges,
        sorted(&[edges[&(1, 9)], edges[&(8, 9)], edges[&(5, 8)]])
    );
    assert!(model.root().geometry().edge(edges[&(1, 5)]).is_some());
    assert_eq!(r.unfixed, vec![]);
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert!(close(r.after.volume.unwrap(), 1.0));
}

#[test]
fn loose_edges_and_their_free_ends_are_removed() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, DVec3::ONE);
    pts.extend([v(3.0, 3.0, 3.0), v(4.0, 3.0, 3.0), v(2.0, 2.0, 2.0)]);
    let faces = box_faces(0);
    // One edge floating free, one from the box corner (1,1,1) out to (2,2,2).
    let (_, edges) = build_in(
        &mut model,
        BodyId::Root,
        &pts,
        &specs(&faces),
        &[(8, 9), (6, 10)],
    );
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.removed_edges, sorted(&[edges[&(8, 9)], edges[&(6, 10)]]));
    assert!(r.removed_stray.is_empty() && r.removed_internal.is_empty());
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert!(vertex_at(&model, v(1.0, 1.0, 1.0)));
    assert!(!vertex_at(&model, v(2.0, 2.0, 2.0)));
    assert!(!vertex_at(&model, v(3.0, 3.0, 3.0)));
    assert_eq!(model.root().geometry().vertex_count(), 8);
}

/// With nothing but stray faces there is nothing to keep: the sheet stays, and its border
/// can't be filled because the fill would cover the sheet itself.
#[test]
fn single_sheet_is_left_alone() {
    let mut model = Model::new();
    let (_, edges) = build(
        &mut model,
        &[
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
        ],
        &[(&[0, 1, 2, 3], &[])],
    );
    let before = model.clone();
    let (r, undo) = repair_root(&mut model);
    assert!(!r.changed(), "{r:?}");
    let all = sorted(&edges.values().copied().collect::<Vec<_>>());
    assert_eq!(r.unfixed, vec![Unfixed::Covered(all)]);
    assert!(!r.after.is_solid());
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

/// Two faces folded along an edge: not stray (they share an edge), and their border isn't
/// flat.
#[test]
fn folded_sheet_border_is_not_flat() {
    let mut model = Model::new();
    let (_, edges) = build(
        &mut model,
        &[
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
            v(0.0, 1.0, 1.0),
            v(1.0, 1.0, 1.0),
        ],
        &[(&[0, 1, 2, 3], &[]), (&[3, 2, 5, 4], &[])],
    );
    let before = model.clone();
    let (r, _) = repair_root(&mut model);
    assert!(!r.changed());
    let open: Vec<EdgeId> = edges
        .iter()
        .filter(|(k, _)| **k != (2, 3))
        .map(|(_, e)| *e)
        .collect();
    assert_eq!(r.unfixed, vec![Unfixed::NotFlat(sorted(&open))]);
    assert_eq!(model, before);
}

// ---------------------------------------------------------------- holes

#[test]
fn missing_top_is_filled() {
    let mut model = Model::new();
    let faces = box_faces(0);
    let kept: Vec<Vec<usize>> = faces
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .map(|(_, f)| f.clone())
        .collect();
    let (_, edges) = build(
        &mut model,
        &box_points(DVec3::ZERO, v(1.0, 2.0, 3.0)),
        &specs(&kept),
    );
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.filled.len(), 1);
    let fill = r.filled[0];
    let ctx = model.root().geometry();
    let want: BTreeSet<EdgeId> = [(4, 5), (5, 6), (6, 7), (4, 7)]
        .iter()
        .map(|k| edges[k])
        .collect();
    assert_eq!(face_edge_set(ctx, fill), want);
    assert!(ctx.face(fill).unwrap().inner.is_empty());
    // It runs against its neighbours, so it already faces out.
    assert!(face_normal(&model, fill).abs_diff_eq(DVec3::Z, 1e-9));
    assert!(r.reversed.is_empty(), "{:?}", r.reversed);
    assert!(r.removed_edges.is_empty());
    assert_eq!(r.unfixed, vec![]);
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert!(close(r.after.volume.unwrap(), 6.0));
}

#[test]
fn missing_top_of_an_inside_out_box_is_filled_and_everything_turned() {
    let mut model = Model::new();
    let kept: Vec<Vec<usize>> = box_faces(0)
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .map(|(_, f)| reversed(f))
        .collect();
    let (ids, _) = build(
        &mut model,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&kept),
    );
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.filled.len(), 1);
    // The fill faced in like its neighbours, so it is turned with them.
    let mut want = ids.clone();
    want.push(r.filled[0]);
    assert_eq!(r.reversed, sorted(&want));
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert!(close(r.after.volume.unwrap(), 1.0));
    assert!(normals_outward(&model));
}

#[test]
fn missing_top_and_flipped_side_are_both_fixed() {
    let mut model = Model::new();
    let mut kept: Vec<Vec<usize>> = box_faces(0)
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .map(|(_, f)| f.clone())
        .collect();
    // Kept order: bottom, front, back, right, left. Flip the right face.
    kept[3] = reversed(&kept[3]);
    let (ids, _) = build(
        &mut model,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&kept),
    );
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.filled.len(), 1);
    assert!(r.reversed.contains(&ids[3]), "{:?}", r.reversed);
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert!(close(r.after.volume.unwrap(), 1.0));
    assert!(normals_outward(&model));
}

#[test]
fn open_tube_gets_both_ends_filled() {
    let mut model = Model::new();
    let kept: Vec<Vec<usize>> = box_faces(0)[2..].to_vec();
    build(
        &mut model,
        &box_points(DVec3::ZERO, v(1.0, 1.0, 4.0)),
        &specs(&kept),
    );
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.filled.len(), 2);
    assert_eq!(r.unfixed, vec![]);
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert!(close(r.after.volume.unwrap(), 4.0));
    assert!(normals_outward(&model));
}

/// A square window in the top face: the window's border belongs to the top face only (its
/// hole), and the fill closes it.
#[test]
fn window_in_a_face_is_filled() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, v(3.0, 3.0, 1.0));
    pts.extend([
        v(1.0, 1.0, 1.0),
        v(2.0, 1.0, 1.0),
        v(2.0, 2.0, 1.0),
        v(1.0, 2.0, 1.0),
    ]);
    let faces = box_faces(0);
    let window: &[usize] = &[11, 10, 9, 8];
    let windows = [window];
    let mut list: Vec<FaceSpec> = specs(&faces);
    list[1] = (&faces[1], &windows);
    let (_, edges) = build(&mut model, &pts, &list);
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.filled.len(), 1);
    let want: BTreeSet<EdgeId> = [(8, 9), (9, 10), (10, 11), (8, 11)]
        .iter()
        .map(|k| edges[k])
        .collect();
    assert_eq!(face_edge_set(model.root().geometry(), r.filled[0]), want);
    assert!(face_normal(&model, r.filled[0]).abs_diff_eq(DVec3::Z, 1e-9));
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert!(close(r.after.volume.unwrap(), 9.0));
}

/// The top was split in two and one half is gone: the hole's border includes the split
/// edge, which the other half (in the same plane) still uses.
#[test]
fn missing_half_of_a_split_top_is_filled() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, v(2.0, 1.0, 1.0));
    pts.extend([v(1.0, 0.0, 1.0), v(1.0, 1.0, 1.0)]); // 8, 9
    let faces: Vec<Vec<usize>> = vec![
        vec![0, 3, 2, 1],    // bottom
        vec![4, 8, 9, 7],    // left half of the top (the right half 8, 5, 6, 9 is missing)
        vec![0, 1, 5, 8, 4], // front, split at 8
        vec![2, 3, 7, 9, 6], // back, split at 9
        vec![1, 2, 6, 5],    // right
        vec![3, 0, 4, 7],    // left
    ];
    let (_, edges) = build(&mut model, &pts, &specs(&faces));
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.filled.len(), 1);
    let want: BTreeSet<EdgeId> = [(5, 8), (5, 6), (6, 9), (8, 9)]
        .iter()
        .map(|k| edges[k])
        .collect();
    assert_eq!(face_edge_set(model.root().geometry(), r.filled[0]), want);
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert!(close(r.after.volume.unwrap(), 2.0));
}

#[test]
fn hole_around_a_corner_is_not_flat() {
    let mut model = Model::new();
    // Top (1) and front (2) missing.
    let kept: Vec<Vec<usize>> = box_faces(0)
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1 && *i != 2)
        .map(|(_, f)| f.clone())
        .collect();
    let (_, edges) = build(
        &mut model,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&kept),
    );
    let before = model.clone();
    let (r, undo) = repair_root(&mut model);
    let border = sorted(
        &[(5, 6), (6, 7), (4, 7), (0, 1), (1, 5), (0, 4)]
            .iter()
            .map(|k| edges[k])
            .collect::<Vec<_>>(),
    );
    assert_eq!(r.unfixed, vec![Unfixed::NotFlat(border.clone())]);
    assert!(!r.changed());
    assert_eq!(r.after.open_edges, border);
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

/// A square block with a square hole through it and its top ring missing: two open loops in
/// one plane, one inside the other. Filling either alone would be wrong.
#[test]
fn ring_shaped_hole_is_reported_not_filled() {
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
    let bottom_holes = [bottom_hole];
    let mut faces: Vec<FaceSpec> = vec![(&[0, 3, 2, 1], &bottom_holes)];
    let mut sides = Vec::new();
    for i in 0..4 {
        let j = (i + 1) % 4;
        sides.push(vec![i, j, j + 8, i + 8]);
        sides.push(vec![4 + j, 4 + i, 12 + i, 12 + j]);
    }
    faces.extend(sides.iter().map(|f| (f.as_slice(), &[][..])));
    let (_, edges) = build(&mut model, &pts, &faces);
    let before = model.clone();
    let (r, _) = repair_root(&mut model);
    let outer = sorted(
        &[(8, 9), (9, 10), (10, 11), (8, 11)]
            .iter()
            .map(|k| edges[k])
            .collect::<Vec<_>>(),
    );
    let inner = sorted(
        &[(12, 13), (13, 14), (14, 15), (12, 15)]
            .iter()
            .map(|k| edges[k])
            .collect::<Vec<_>>(),
    );
    // Loops in the order of their lowest edge id.
    let mut want = vec![
        Unfixed::Nested(outer.clone()),
        Unfixed::Nested(inner.clone()),
    ];
    if inner[0] < outer[0] {
        want.reverse();
    }
    assert_eq!(r.unfixed, want);
    assert!(!r.changed());
    assert_eq!(model, before);
}

/// Two boxes side by side, both missing their tops: two loops in one plane, neither inside
/// the other, so both are filled.
#[test]
fn side_by_side_holes_in_one_plane_are_both_filled() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, DVec3::ONE);
    pts.extend(box_points(v(2.0, 0.0, 0.0), v(3.0, 1.0, 1.0)));
    let kept: Vec<Vec<usize>> = box_faces(0)
        .into_iter()
        .chain(box_faces(8))
        .enumerate()
        .filter(|(i, _)| *i != 1 && *i != 7)
        .map(|(_, f)| f)
        .collect();
    build(&mut model, &pts, &specs(&kept));
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.filled.len(), 2);
    assert_eq!(r.unfixed, vec![]);
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert_eq!(r.after.shells, 2);
    assert!(close(r.after.volume.unwrap(), 2.0));
}

// ---------------------------------------------------------------- shared edges

#[test]
fn boxes_touching_along_an_edge_are_reported() {
    let mut model = Model::new();
    let mut pts = box_points(DVec3::ZERO, DVec3::ONE);
    pts.extend(box_points(v(1.0, 1.0, 0.0), v(2.0, 2.0, 1.0)));
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
    let before = model.clone();
    let (r, undo) = repair_root(&mut model);
    assert_eq!(r.unfixed, vec![Unfixed::SharedEdge(edges[&(2, 6)])]);
    assert!(!r.changed());
    assert!(!r.after.is_solid());
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

// ---------------------------------------------------------------- undo, bodies, log

/// Flipped face, missing top, loose edge and a stray face, all at once.
fn badly_broken(model: &mut Model, body: BodyId) {
    let mut pts = box_points(DVec3::ZERO, v(0.02, 0.03, 0.01));
    pts.extend([
        v(0.1, 0.0, 0.0),
        v(0.2, 0.0, 0.0),
        v(0.1, 0.1, 0.0),
        v(0.3, 0.3, 0.3),
        v(0.4, 0.3, 0.3),
    ]);
    let mut faces: Vec<Vec<usize>> = box_faces(0)
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .map(|(_, f)| f.clone())
        .collect();
    faces[1] = reversed(&faces[1]);
    faces.push(vec![8, 9, 10]);
    build_in(model, body, &pts, &specs(&faces), &[(11, 12)]);
}

#[test]
fn repair_is_one_undo_step() {
    let mut model = Model::new();
    badly_broken(&mut model, BodyId::Root);
    let before = model.clone();
    let mut undo = UndoStack::new();
    let r = repair_body(&mut model, BodyId::Root, &mut undo, "Repair Solid").unwrap();
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert!(close(r.after.volume.unwrap(), 0.02 * 0.03 * 0.01));
    assert_eq!(r.removed_stray.len(), 1);
    assert_eq!(r.removed_edges.len(), 4);
    assert_eq!(r.filled.len(), 1);
    assert!(!r.reversed.is_empty());
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("Repair Solid"));
    let repaired = model.clone();
    assert_eq!(
        undo.undo(&mut model).unwrap().as_deref(),
        Some("Repair Solid")
    );
    assert_eq!(model, before);
    undo.redo(&mut model).unwrap();
    assert_eq!(model, repaired);
    assert!(model.validate().is_empty());
}

#[test]
fn second_repair_does_nothing() {
    let mut model = Model::new();
    badly_broken(&mut model, BodyId::Root);
    let mut undo = UndoStack::new();
    repair_body(&mut model, BodyId::Root, &mut undo, "Repair Solid").unwrap();
    let once = model.clone();
    let r = repair_body(&mut model, BodyId::Root, &mut undo, "Repair Solid").unwrap();
    assert!(!r.changed(), "{r:?}");
    assert_eq!(r.unfixed, vec![]);
    assert_eq!(model, once);
    assert_eq!(undo.undo_count(), 1);
}

fn add_group(model: &mut Model) -> DefinitionId {
    let mut edit = ModelEdit::new();
    let def = model.add_definition(
        DefinitionInfo {
            name: "Part".into(),
            group: true,
        },
        &mut edit,
    );
    model
        .add_instance(
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
    def
}

#[test]
fn repair_body_works_inside_a_group_and_leaves_the_root_alone() {
    let mut model = Model::new();
    cube(&mut model);
    let def = add_group(&mut model);
    let body = BodyId::Definition(def);
    badly_broken(&mut model, body);
    let root_before = model.root().clone();
    let mut undo = UndoStack::new();
    let r = repair_body(&mut model, body, &mut undo, "Repair Solid").unwrap();
    assert!(r.after.is_clean(), "{:?}", r.after);
    assert_eq!(Some(r.after.clone()), check_body(&model, body));
    assert_eq!(model.root().geometry(), root_before.geometry());
    assert_eq!(undo.undo_count(), 1);
    assert!(model.validate().is_empty());
}

#[test]
fn nested_instances_are_reported_and_the_rest_repaired() {
    let mut model = Model::new();
    let mut faces = box_faces(0);
    faces[0] = reversed(&faces[0]);
    let (ids, _) = build(
        &mut model,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&faces),
    );
    let def = add_group(&mut model);
    cube_in(&mut model, BodyId::Definition(def));
    let (r, _) = repair_root(&mut model);
    assert_eq!(r.reversed, vec![ids[0]]);
    assert_eq!(r.unfixed, vec![Unfixed::NestedInstances(1)]);
    assert_eq!(r.after.nested_instances, 1);
    assert_eq!(r.after.volume, None);
    assert!(r.after.reversed_faces.is_empty());
}

#[test]
fn nested_instance_alone_changes_nothing() {
    let mut model = Model::new();
    cube(&mut model);
    add_group(&mut model);
    let before = model.clone();
    let (r, undo) = repair_root(&mut model);
    assert!(!r.changed());
    assert_eq!(r.unfixed, vec![Unfixed::NestedInstances(1)]);
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

fn cube_in(model: &mut Model, body: BodyId) {
    build_in(
        model,
        body,
        &box_points(DVec3::ZERO, DVec3::ONE),
        &specs(&box_faces(0)),
        &[],
    );
}

#[test]
fn missing_body_is_an_error() {
    let mut model = Model::new();
    cube(&mut model);
    let before = model.clone();
    let mut undo = UndoStack::new();
    let body = BodyId::Definition(DefinitionId(99));
    assert_eq!(
        repair_body(&mut model, body, &mut undo, "Repair Solid"),
        Err(RepairError::Model(ModelError::MissingBody(body)))
    );
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

/// `repair_geometry` logs exactly what it changed: the log's inverse restores the input and
/// the log replayed on a copy of the input gives the result.
#[test]
fn repair_geometry_log_replays_and_inverts() {
    let mut model = Model::new();
    badly_broken(&mut model, BodyId::Root);
    let original = model.root().geometry().clone();
    let mut ctx = original.clone();
    let mut log = Edit::new();
    let r = repair_geometry(&mut ctx, &mut log);
    assert!(r.changed());
    assert!(r.after.is_clean());
    assert_eq!(r.after, check_geometry(&ctx));
    assert!(ctx.validate().is_empty(), "{:?}", ctx.validate());

    let mut replay = original.clone();
    replay.apply_edit(&log).unwrap();
    assert_eq!(replay, ctx);

    let mut back = ctx.clone();
    back.apply_edit(&log.inverse()).unwrap();
    assert_eq!(back, original);
}

#[test]
fn repair_geometry_logs_nothing_when_nothing_changes() {
    for build_it in [
        |m: &mut Model| {
            cube(m);
        },
        |m: &mut Model| {
            mesh(
                m,
                &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0)],
                &[&[0, 1, 2]],
            );
        },
    ] {
        let mut model = Model::new();
        build_it(&mut model);
        let mut ctx = model.root().geometry().clone();
        let mut log = Edit::new();
        let r = repair_geometry(&mut ctx, &mut log);
        assert!(!r.changed());
        assert!(log.is_empty(), "{:?}", log);
        assert_eq!(&ctx, model.root().geometry());
    }
}
