// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m1-dae-to-kernel: small documents built in code, and the tables fixture.

mod common;

use std::path::{Path, PathBuf};

use common::*;
use glam::{DMat4, DVec3};
use modeler_dae::{ConvertError, Primitive, UpAxis, flatten, import_file, to_kernel};
use modeler_kernel::geom::{polygon_normal, signed_area_2d, to_plane_2d};
use modeler_kernel::model::{self as km, BodyId, FaceMaterials, ModelError};
use modeler_kernel::topology::{Context, Face, FaceId};

const QUAD: [[f32; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
];

fn quad_tris(q: [[f32; 3]; 4]) -> Vec<[[f32; 3]; 3]> {
    vec![[q[0], q[1], q[2]], [q[0], q[2], q[3]]]
}

fn rev(t: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    [t[0], t[2], t[1]]
}

fn v(p: [f32; 3]) -> DVec3 {
    DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)
}

/// A document whose root holds one geometry instance with these primitives.
fn root_doc(primitives: Vec<Primitive>) -> modeler_dae::Model {
    let mut d = doc();
    let g = add_mesh(&mut d, primitives, &[]);
    d.scene.geometry.push(g);
    d
}

fn convert(d: &modeler_dae::Model) -> km::Model {
    to_kernel(d).expect("document converts")
}

fn root(m: &km::Model) -> &Context {
    m.root().geometry()
}

fn faces(ctx: &Context) -> Vec<(FaceId, &Face)> {
    ctx.faces().collect()
}

/// True when `actual` is `expected` rotated (same cyclic order), within 1e-9.
fn same_cycle(actual: &[DVec3], expected: &[DVec3]) -> bool {
    actual.len() == expected.len()
        && (0..expected.len()).any(|shift| {
            (0..expected.len())
                .all(|i| actual[(i + shift) % actual.len()].distance(expected[i]) < 1e-9)
        })
}

fn assert_valid(ctx: &Context) {
    let violations = ctx.validate();
    assert!(violations.is_empty(), "violations: {violations:?}");
}

fn normal(ctx: &Context, face: &Face) -> DVec3 {
    polygon_normal(&ctx.loop_points(&face.outer))
}

fn total_area(ctx: &Context) -> f64 {
    ctx.faces()
        .map(|(_, f)| face_area(ctx, f, DMat4::IDENTITY))
        .sum()
}

// ---------------------------------------------------------------------------------------
// Merging triangles into faces

#[test]
fn quad_becomes_one_face() {
    let d = root_doc(vec![tris(None, &quad_tris(QUAD))]);
    let m = convert(&d);
    let ctx = root(&m);
    assert_eq!(ctx.vertex_count(), 4);
    assert_eq!(ctx.edge_count(), 4);
    let fs = faces(ctx);
    assert_eq!(fs.len(), 1);
    let face = fs[0].1;
    assert!(face.inner.is_empty());
    let expected: Vec<DVec3> = QUAD.iter().map(|p| v(*p)).collect();
    assert!(same_cycle(&ctx.loop_points(&face.outer), &expected));
    assert!(normal(ctx, face).distance(DVec3::Z) < 1e-12);
    assert_valid(ctx);
    assert!(m.validate().is_empty());
    assert_eq!(m.definitions().count(), 0);
    assert_eq!(m.root().instances().count(), 0);
}

#[test]
fn fan_centre_vertex_disappears() {
    let centre = [0.0f32, 0.0, 2.0];
    let ring: Vec<[f32; 3]> = (0..6)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / 6.0;
            [a.cos(), a.sin(), 2.0]
        })
        .collect();
    let triangles: Vec<[[f32; 3]; 3]> = (0..6)
        .map(|i| [centre, ring[i], ring[(i + 1) % 6]])
        .collect();
    let m = convert(&root_doc(vec![tris(None, &triangles)]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 1);
    assert_eq!(ctx.vertex_count(), 6);
    assert_eq!(ctx.edge_count(), 6);
    assert!(ctx.find_vertex(v(centre), 1e-6).is_none());
    let face = faces(ctx)[0].1;
    let expected: Vec<DVec3> = ring.iter().map(|p| v(*p)).collect();
    assert!(same_cycle(&ctx.loop_points(&face.outer), &expected));
    assert_valid(ctx);
}

#[test]
fn ring_of_triangles_becomes_face_with_hole() {
    let o = [
        [0.0f32, 0.0, 0.0],
        [3.0, 0.0, 0.0],
        [3.0, 3.0, 0.0],
        [0.0, 3.0, 0.0],
    ];
    let i = [
        [1.0f32, 1.0, 0.0],
        [2.0, 1.0, 0.0],
        [2.0, 2.0, 0.0],
        [1.0, 2.0, 0.0],
    ];
    let mut triangles = Vec::new();
    for k in 0..4 {
        let n = (k + 1) % 4;
        triangles.push([o[k], o[n], i[n]]);
        triangles.push([o[k], i[n], i[k]]);
    }
    let m = convert(&root_doc(vec![tris(None, &triangles)]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 1);
    assert_eq!(ctx.vertex_count(), 8);
    assert_eq!(ctx.edge_count(), 8);
    let face = faces(ctx)[0].1;
    assert_eq!(face.inner.len(), 1);
    let outer: Vec<DVec3> = o.iter().map(|p| v(*p)).collect();
    let hole: Vec<DVec3> = [i[0], i[3], i[2], i[1]].iter().map(|p| v(*p)).collect();
    assert!(same_cycle(&ctx.loop_points(&face.outer), &outer));
    assert!(same_cycle(&ctx.loop_points(&face.inner[0]), &hole));
    let hole_2d = to_plane_2d(&ctx.loop_points(&face.inner[0]), DVec3::Z);
    assert!(signed_area_2d(&hole_2d) < 0.0);
    assert!((total_area(ctx) - 8.0).abs() < 1e-9);
    assert_valid(ctx);
}

fn cube_triangles() -> Vec<[[f32; 3]; 3]> {
    let c = |x: u32, y: u32, z: u32| [x as f32, y as f32, z as f32];
    // Each side as a counter-clockwise quad seen from outside.
    let sides = [
        [c(0, 0, 0), c(0, 1, 0), c(1, 1, 0), c(1, 0, 0)], // bottom, -Z
        [c(0, 0, 1), c(1, 0, 1), c(1, 1, 1), c(0, 1, 1)], // top, +Z
        [c(0, 0, 0), c(1, 0, 0), c(1, 0, 1), c(0, 0, 1)], // -Y
        [c(0, 1, 0), c(0, 1, 1), c(1, 1, 1), c(1, 1, 0)], // +Y
        [c(0, 0, 0), c(0, 0, 1), c(0, 1, 1), c(0, 1, 0)], // -X
        [c(1, 0, 0), c(1, 1, 0), c(1, 1, 1), c(1, 0, 1)], // +X
    ];
    sides.iter().flat_map(|q| quad_tris(*q)).collect()
}

#[test]
fn cube_gives_six_outward_faces() {
    let m = convert(&root_doc(vec![tris(None, &cube_triangles())]));
    let ctx = root(&m);
    assert_eq!(ctx.vertex_count(), 8);
    assert_eq!(ctx.edge_count(), 12);
    assert_eq!(ctx.face_count(), 6);
    let centre = DVec3::splat(0.5);
    for (_, face) in ctx.faces() {
        assert_eq!(face.outer.uses.len(), 4);
        let pts = ctx.loop_points(&face.outer);
        let mid = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
        assert!(
            normal(ctx, face).dot(mid - centre) > 0.4,
            "face points inward"
        );
    }
    for (e, _) in ctx.edges() {
        assert_eq!(ctx.faces_of_edge(e).len(), 2);
    }
    assert_valid(ctx);
}

#[test]
fn different_materials_stay_apart() {
    let mut d = doc();
    d.materials
        .push(material("red", "Red", [1.0, 0.0, 0.0, 1.0]));
    d.materials
        .push(material("blue", "Blue", [0.0, 0.0, 1.0, 1.0]));
    let t = quad_tris(QUAD);
    let g = add_mesh(
        &mut d,
        vec![tris(Some("a"), &t[..1]), tris(Some("b"), &t[1..])],
        &[("a", 0), ("b", 1)],
    );
    d.scene.geometry.push(g);
    let m = convert(&d);
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 2);
    assert_eq!(ctx.edge_count(), 5);
    let ids: Vec<km::MaterialId> = m.materials().map(|(id, _)| id).collect();
    let mut fronts: Vec<Option<km::MaterialId>> = ctx
        .faces()
        .map(|(f, _)| m.root().face_materials(f).front)
        .collect();
    fronts.sort();
    assert_eq!(fronts, vec![Some(ids[0]), Some(ids[1])]);
    for (f, _) in ctx.faces() {
        assert_eq!(m.root().face_materials(f).back, None);
    }
    assert_valid(ctx);
}

#[test]
fn unbound_symbol_is_default_material() {
    let mut d = doc();
    d.materials
        .push(material("red", "Red", [1.0, 0.0, 0.0, 1.0]));
    let g = add_mesh(
        &mut d,
        vec![tris(Some("nope"), &quad_tris(QUAD))],
        &[("a", 0)],
    );
    d.scene.geometry.push(g);
    let m = convert(&d);
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 1);
    let (f, _) = faces(ctx)[0];
    assert_eq!(m.root().face_materials(f), FaceMaterials::default());
    assert_eq!(m.root().face_material_entries().count(), 0);
}

#[test]
fn segment_on_shared_edge_keeps_faces_apart() {
    let t = quad_tris(QUAD);
    let m = convert(&root_doc(vec![tris(None, &t), segs(&[[QUAD[0], QUAD[2]]])]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 2);
    assert_eq!(ctx.edge_count(), 5);
    assert_eq!(ctx.vertex_count(), 4);
    assert_valid(ctx);
}

#[test]
fn segments_become_edges() {
    let a = [0.0f32, 0.0, 0.0];
    let b = [1.0f32, 0.0, 0.0];
    let c = [1.0f32, 2.0, 0.0];
    let m = convert(&root_doc(vec![segs(&[[a, b], [b, c], [c, c]])]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 0);
    assert_eq!(ctx.vertex_count(), 3);
    assert_eq!(ctx.edge_count(), 2);
    assert_valid(ctx);
}

#[test]
fn segment_along_face_boundary_reuses_its_edge() {
    let m = convert(&root_doc(vec![
        tris(None, &quad_tris(QUAD)),
        segs(&[[QUAD[1], QUAD[0]], [QUAD[2], [5.0, 5.0, 0.0]]]),
    ]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 1);
    assert_eq!(ctx.edge_count(), 5);
    assert_eq!(ctx.vertex_count(), 5);
    assert_valid(ctx);
}

#[test]
fn bend_gives_two_faces() {
    let a = [0.0f32, 0.0, 0.0];
    let b = [1.0f32, 0.0, 0.0];
    let m = convert(&root_doc(vec![tris(
        None,
        &[[a, b, [1.0, 1.0, 0.0]], [b, a, [0.0, 0.0, 1.0]]],
    )]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 2);
    assert_eq!(ctx.edge_count(), 5);
    assert_valid(ctx);
}

#[test]
fn lift_beyond_half_tolerance_splits_quad() {
    let lifted = |z: f32| {
        let mut q = QUAD;
        q[3][2] = z;
        convert(&root_doc(vec![tris(None, &quad_tris(q))]))
    };
    // 2 um is within LINEAR_TOL / 2 of the first triangle's plane: one face.
    let m = lifted(2e-6);
    assert_eq!(root(&m).face_count(), 1);
    assert_valid(root(&m));
    // 100 um isn't: two faces.
    let m = lifted(1e-4);
    assert_eq!(root(&m).face_count(), 2);
    assert_valid(root(&m));
}

#[test]
fn facing_the_other_way_is_not_joined() {
    // Second triangle shares the edge in the same direction (inconsistent winding).
    let t = quad_tris(QUAD);
    let m = convert(&root_doc(vec![tris(None, &[t[0], rev(t[1])])]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 2);
    assert_valid(ctx);
}

#[test]
fn twin_triangles_give_back_material() {
    let mut d = doc();
    d.materials
        .push(material("front", "Front", [1.0, 1.0, 1.0, 1.0]));
    d.materials
        .push(material("back", "Back", [0.0, 0.0, 0.0, 1.0]));
    let t = quad_tris(QUAD);
    let back: Vec<_> = t.iter().map(|x| rev(*x)).collect();
    let g = add_mesh(
        &mut d,
        vec![tris(Some("f"), &t), tris(Some("b"), &back)],
        &[("f", 0), ("b", 1)],
    );
    d.scene.geometry.push(g);
    let m = convert(&d);
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 1);
    assert_eq!(ctx.edge_count(), 4);
    let ids: Vec<km::MaterialId> = m.materials().map(|(id, _)| id).collect();
    let (f, face) = faces(ctx)[0];
    assert_eq!(
        m.root().face_materials(f),
        FaceMaterials {
            front: Some(ids[0]),
            back: Some(ids[1]),
            ..Default::default()
        }
    );
    assert!(normal(ctx, face).distance(DVec3::Z) < 1e-12);
    assert_valid(ctx);
}

#[test]
fn twin_with_no_material_leaves_back_default() {
    let t = quad_tris(QUAD);
    let mut all = t.clone();
    all.extend(t.iter().map(|x| rev(*x)));
    all.push(rev(t[0])); // a second twin: dropped
    let m = convert(&root_doc(vec![tris(None, &all)]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 1);
    assert_eq!(m.root().face_material_entries().count(), 0);
}

#[test]
fn duplicate_triangles_dropped() {
    let t = quad_tris(QUAD);
    // Same triangles again, one with its corners rotated.
    let again = vec![t[0], [t[1][1], t[1][2], t[1][0]]];
    let m = convert(&root_doc(vec![tris(None, &t), tris(None, &again)]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 1);
    assert_eq!(ctx.edge_count(), 4);
    assert_valid(ctx);
}

#[test]
fn degenerate_triangles_dropped() {
    let m = convert(&root_doc(vec![tris(
        None,
        &[
            // Collinear.
            [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            // Two corners weld (4 um apart).
            [[0.0, 5.0, 0.0], [4e-6, 5.0, 0.0], [1.0, 6.0, 0.0]],
            // Sliver: apex 1 um from the base line.
            [[0.0, 9.0, 0.0], [2.0, 9.0, 0.0], [1.0, 9.000001, 0.0]],
            // Not finite.
            [[f32::NAN, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            [[f32::INFINITY, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
        ],
    )]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 0);
    assert_eq!(ctx.edge_count(), 0);
    assert_eq!(ctx.vertex_count(), 0);
}

#[test]
fn indices_out_of_range_are_skipped() {
    let mut p = tris(None, &quad_tris(QUAD));
    p.indices.extend([0, 1, 99]);
    let mut s = segs(&[[QUAD[0], [0.0, 0.0, 5.0]]]);
    s.indices.extend([0, 7]);
    let m = convert(&root_doc(vec![p, s]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 1);
    assert_eq!(ctx.edge_count(), 5);
}

#[test]
fn points_within_tolerance_weld() {
    let d = 3e-6f32;
    let t0 = [QUAD[0], QUAD[1], QUAD[2]];
    let t1 = [[d, 0.0, 0.0], [1.0, 1.0 + d, 0.0], QUAD[3]];
    let m = convert(&root_doc(vec![tris(None, &[t0, t1])]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 1);
    assert_eq!(ctx.vertex_count(), 4);
    // Slots keep the first point that opened them.
    assert!(ctx.find_vertex(DVec3::ZERO, 1e-9).is_some());
    assert!(ctx.find_vertex(DVec3::new(1.0, 1.0, 0.0), 1e-9).is_some());
    assert_valid(ctx);
}

#[test]
fn points_beyond_tolerance_stay_apart() {
    let d = 3e-5f32;
    let t0 = [QUAD[0], QUAD[1], QUAD[2]];
    let t1 = [[0.0, d, 0.0], [1.0, 1.0 + d, 0.0], QUAD[3]];
    let m = convert(&root_doc(vec![tris(None, &[t0, t1])]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 2);
    assert_eq!(ctx.vertex_count(), 6);
    assert_eq!(ctx.edge_count(), 6);
}

#[test]
fn closest_slot_wins() {
    // Slots at x = 0 and x = 1.6e-5; the point at 1.0e-5 is within tolerance of both
    // and closer to the second.
    let a = [0.0f32, 0.0, 0.0];
    let b = [1.6e-5f32, 0.0, 0.0];
    let p = [1.0e-5f32, 0.0, 0.0];
    let far = [0.0f32, 1.0, 0.0];
    let m = convert(&root_doc(vec![segs(&[
        [a, far],
        [b, [0.0, -1.0, 0.0]],
        [p, [5.0, 0.0, 0.0]],
    ])]));
    let ctx = root(&m);
    let vb = ctx.find_vertex(v(b), 1e-9).expect("slot b is a vertex");
    let ve = ctx.find_vertex(DVec3::new(5.0, 0.0, 0.0), 1e-9).unwrap();
    assert!(ctx.edge_between(vb, ve).is_some());
    assert_eq!(ctx.vertex_count(), 5);
}

#[test]
fn fully_dropped_document_is_empty() {
    let m = convert(&doc());
    assert!(m.root().is_empty());
    assert_eq!(m.definitions().count(), 0);
    assert_eq!(m.materials().count(), 0);
}

/// 3x3 cells without the centre is one face with one hole. (A grid whose outline touches
/// itself: `kernel_pinch.rs`.)
#[test]
fn grid_with_centre_missing_is_one_face() {
    let mut triangles = Vec::new();
    for y in 0..3 {
        for x in 0..3 {
            if (x, y) == (1, 1) {
                continue;
            }
            let (x, y) = (x as f32, y as f32);
            triangles.extend(quad_tris([
                [x, y, 0.0],
                [x + 1.0, y, 0.0],
                [x + 1.0, y + 1.0, 0.0],
                [x, y + 1.0, 0.0],
            ]));
        }
    }
    let m = convert(&root_doc(vec![tris(None, &triangles)]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 1);
    let face = faces(ctx)[0].1;
    assert_eq!(face.inner.len(), 1);
    // Collinear boundary points stay: 12 outer, 4 hole.
    assert_eq!(face.outer.uses.len(), 12);
    assert_eq!(face.inner[0].uses.len(), 4);
    assert!((total_area(ctx) - 8.0).abs() < 1e-9);
    assert_valid(ctx);
}

// ---------------------------------------------------------------------------------------
// Frame, materials and hierarchy

#[test]
fn units_and_up_axis() {
    let mut d = root_doc(vec![tris(
        None,
        &[[[1.0, 2.0, 3.0], [2.0, 2.0, 3.0], [1.0, 2.0, 4.0]]],
    )]);
    d.unit_meters = 0.0254;
    d.up_axis = UpAxis::Y;
    let m = convert(&d);
    let ctx = root(&m);
    // Y up: file (x, y, z) is (x, -z, y), times 0.0254.
    let s = 0.0254f64;
    for p in [[1.0, -3.0, 2.0], [2.0, -3.0, 2.0], [1.0, -4.0, 2.0]] {
        let q = DVec3::new(p[0] * s, p[1] * s, p[2] * s);
        assert!(ctx.find_vertex(q, 1e-7).is_some(), "no vertex at {q}");
    }
    assert_eq!(ctx.face_count(), 1);

    d.up_axis = UpAxis::X;
    let m = convert(&d);
    let ctx = root(&m);
    // X up: file (x, y, z) is (-y, -z, x).
    for p in [[-2.0, -3.0, 1.0], [-2.0, -3.0, 2.0], [-2.0, -4.0, 1.0]] {
        let q = DVec3::new(p[0] * s, p[1] * s, p[2] * s);
        assert!(ctx.find_vertex(q, 1e-7).is_some(), "no vertex at {q}");
    }
}

#[test]
fn materials_convert_in_order() {
    let mut d = doc();
    let mut a = material("m-a", "Alpha", [0.25, 0.5, 0.75, 0.9]);
    a.opacity = 0.5;
    a.texture = Some(PathBuf::from("/x/model/wood.jpg"));
    d.materials.push(a);
    d.materials.push(material("m-b", "", [0.1, 0.2, 0.3, 1.0]));
    let m = convert(&d);
    let mats: Vec<&km::Material> = m.materials().map(|(_, mat)| mat).collect();
    assert_eq!(mats.len(), 2);
    assert_eq!(mats[0].name, "Alpha");
    assert_eq!(mats[0].color, [0.25, 0.5, 0.75, 0.5]);
    assert_eq!(
        mats[0].texture.as_ref().map(|t| t.path.as_str()),
        Some("/x/model/wood.jpg")
    );
    assert_eq!(mats[1].name, "m-b");
    assert_eq!(mats[1].color, [0.1, 0.2, 0.3, 1.0]);
    assert_eq!(mats[1].texture, None);
}

fn translate(x: f64, y: f64, z: f64) -> DMat4 {
    DMat4::from_translation(DVec3::new(x, y, z))
}

fn assert_mat_eq(a: DMat4, b: DMat4) {
    assert!(
        a.to_cols_array()
            .iter()
            .zip(b.to_cols_array())
            .all(|(x, y)| (x - y).abs() < 1e-9),
        "{a:?} != {b:?}"
    );
}

fn def_by_name(m: &km::Model, name: &str) -> km::DefinitionId {
    let found: Vec<_> = m
        .definitions()
        .filter(|(_, info)| info.name == name)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(found.len(), 1, "definitions named {name:?}: {found:?}");
    found[0]
}

#[test]
fn components_become_definitions_and_instances() {
    let mut d = doc();
    d.unit_meters = 0.5;
    let g = add_mesh(&mut d, vec![tris(None, &quad_tris(QUAD))], &[]);
    let mut dn = node("chair-node");
    dn.geometry.push(g);
    dn.transform = translate(0.0, 0.0, 4.0);
    d.definitions.push(definition("chair-id", "Chair", dn));
    d.definitions
        .push(definition("unused-id", "", node("unused")));
    let mut k1 = node("chair 1");
    k1.transform = translate(10.0, 0.0, 0.0);
    k1.instances.push(0);
    let mut k2 = node("chair 2");
    k2.transform = translate(0.0, 20.0, 0.0);
    k2.instances.push(0);
    d.scene.children.push(k1);
    d.scene.children.push(k2);

    let m = convert(&d);
    assert!(m.validate().is_empty());
    let defs: Vec<(km::DefinitionId, &km::DefinitionInfo)> = m.definitions().collect();
    assert_eq!(defs.len(), 2);
    assert_eq!(defs[0].1.name, "Chair");
    assert!(!defs[0].1.group);
    assert_eq!(defs[1].1.name, "unused-id");
    assert!(m.body(BodyId::Definition(defs[1].0)).unwrap().is_empty());

    let chair = defs[0].0;
    let body = m.body(BodyId::Definition(chair)).unwrap();
    assert_eq!(body.geometry().face_count(), 1);
    // Geometry is scaled into meters; the definition node's own transform isn't baked in.
    assert!(
        body.geometry()
            .find_vertex(DVec3::new(0.5, 0.5, 0.0), 1e-9)
            .is_some()
    );

    assert!(root(&m).vertices().next().is_none());
    let insts: Vec<&km::Instance> = m.root().instances().map(|(_, i)| i).collect();
    assert_eq!(insts.len(), 2);
    assert_eq!(insts[0].name, "chair 1");
    assert_eq!(insts[1].name, "chair 2");
    for i in &insts {
        assert_eq!(i.definition, chair);
        assert_eq!(i.material, None);
    }
    assert_mat_eq(insts[0].transform, translate(5.0, 0.0, 2.0));
    assert_mat_eq(insts[1].transform, translate(0.0, 10.0, 2.0));
}

#[test]
fn transforms_are_conjugated_by_the_frame() {
    let mut d = doc();
    d.unit_meters = 2.0;
    d.up_axis = UpAxis::Y;
    let g = add_mesh(&mut d, vec![tris(None, &quad_tris(QUAD))], &[]);
    let mut dn = node("n");
    dn.geometry.push(g);
    d.definitions.push(definition("d", "D", dn));
    let file_t = DMat4::from_rotation_y(0.3) * translate(1.0, 2.0, 3.0);
    let mut k = node("k");
    k.transform = file_t;
    k.instances.push(0);
    d.scene.children.push(k);

    let m = convert(&d);
    // frame(Y) * scale(2): (x, y, z) -> (2x, -2z, 2y).
    let c = DMat4::from_cols_array(&[
        2.0, 0.0, 0.0, 0.0, //
        0.0, 0.0, 2.0, 0.0, //
        0.0, -2.0, 0.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]);
    let inst = m.root().instances().next().unwrap().1;
    assert_mat_eq(inst.transform, c * file_t * c.inverse());
    // The instance places the definition where flatten draws it.
    let body = m.body(BodyId::Definition(inst.definition)).unwrap();
    let flat = flatten(&d);
    let drawn: Vec<DVec3> = flat.batches[0].positions.iter().map(|p| v(*p)).collect();
    for (_, vert) in body.geometry().vertices() {
        let w = inst.transform.transform_point3(vert.position);
        assert!(
            drawn.iter().any(|p| p.distance(w) < 1e-5),
            "vertex at {w} isn't drawn by flatten"
        );
    }
}

#[test]
fn group_nodes_become_groups() {
    let mut d = doc();
    let g1 = add_mesh(&mut d, vec![tris(None, &quad_tris(QUAD))], &[]);
    let g2 = add_mesh(
        &mut d,
        vec![tris(
            None,
            &[[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]],
        )],
        &[],
    );
    let mut h = node("H");
    h.geometry.push(g2);
    h.transform = translate(0.0, 0.0, 7.0);
    let mut gnode = node("G");
    gnode.geometry.push(g1);
    gnode.transform = translate(3.0, 0.0, 0.0);
    gnode.children.push(h);
    d.scene.children.push(gnode);

    let m = convert(&d);
    assert!(m.validate().is_empty());
    let defs: Vec<(km::DefinitionId, &km::DefinitionInfo)> = m.definitions().collect();
    assert_eq!(defs.len(), 2);
    assert_eq!((defs[0].1.name.as_str(), defs[0].1.group), ("G", true));
    assert_eq!((defs[1].1.name.as_str(), defs[1].1.group), ("H", true));
    let (g_id, h_id) = (defs[0].0, defs[1].0);

    let root_insts: Vec<&km::Instance> = m.root().instances().map(|(_, i)| i).collect();
    assert_eq!(root_insts.len(), 1);
    assert_eq!(root_insts[0].definition, g_id);
    assert_eq!(root_insts[0].name, "G");
    assert_mat_eq(root_insts[0].transform, translate(3.0, 0.0, 0.0));

    let gbody = m.body(BodyId::Definition(g_id)).unwrap();
    assert_eq!(gbody.geometry().face_count(), 1);
    let g_insts: Vec<&km::Instance> = gbody.instances().map(|(_, i)| i).collect();
    assert_eq!(g_insts.len(), 1);
    assert_eq!(g_insts[0].definition, h_id);
    assert_eq!(g_insts[0].name, "H");
    assert_mat_eq(g_insts[0].transform, translate(0.0, 0.0, 7.0));
    let hbody = m.body(BodyId::Definition(h_id)).unwrap();
    assert_eq!(hbody.geometry().face_count(), 1);
}

#[test]
fn empty_nodes_are_skipped() {
    let mut d = doc();
    d.scene.children.push(node("camera"));
    let mut outer = node("outer");
    outer.children.push(node("inner"));
    d.scene.children.push(outer);
    let m = convert(&d);
    assert_eq!(m.definitions().count(), 0);
    assert_eq!(m.root().instances().count(), 0);
}

#[test]
fn node_with_instance_and_geometry_is_a_group() {
    let mut d = doc();
    let g = add_mesh(&mut d, vec![tris(None, &quad_tris(QUAD))], &[]);
    let mut dn = node("dn");
    dn.geometry.push(g.clone());
    dn.transform = translate(0.0, 1.0, 0.0);
    d.definitions.push(definition("d", "D", dn));
    let mut k = node("K");
    k.geometry.push(g);
    k.instances.push(0);
    k.transform = translate(9.0, 0.0, 0.0);
    d.scene.children.push(k);
    // An instance ref directly in the scene's contents.
    d.scene.instances.push(0);

    let m = convert(&d);
    assert!(m.validate().is_empty());
    let dd = def_by_name(&m, "D");
    let kk = def_by_name(&m, "K");
    assert!(m.definition(kk).unwrap().group);
    let kbody = m.body(BodyId::Definition(kk)).unwrap();
    assert_eq!(kbody.geometry().face_count(), 1);
    let k_insts: Vec<&km::Instance> = kbody.instances().map(|(_, i)| i).collect();
    assert_eq!(k_insts.len(), 1);
    assert_eq!(k_insts[0].definition, dd);
    assert_eq!(k_insts[0].name, "");
    assert_mat_eq(k_insts[0].transform, translate(0.0, 1.0, 0.0));

    let root_insts: Vec<&km::Instance> = m.root().instances().map(|(_, i)| i).collect();
    assert_eq!(root_insts.len(), 2);
    let to_k = root_insts.iter().find(|i| i.definition == kk).unwrap();
    assert_eq!(to_k.name, "K");
    assert_mat_eq(to_k.transform, translate(9.0, 0.0, 0.0));
    let to_d = root_insts.iter().find(|i| i.definition == dd).unwrap();
    assert_eq!(to_d.name, "");
    assert_mat_eq(to_d.transform, translate(0.0, 1.0, 0.0));
}

#[test]
fn nested_components() {
    let mut d = doc();
    let g = add_mesh(&mut d, vec![tris(None, &quad_tris(QUAD))], &[]);
    let mut leg = node("leg");
    leg.geometry.push(g);
    d.definitions.push(definition("leg", "Leg", leg));
    let mut table = node("table");
    for x in [0.0, 2.0] {
        let mut k = node("leg instance");
        k.instances.push(0);
        k.transform = translate(x, 0.0, 0.0);
        table.children.push(k);
    }
    d.definitions.push(definition("table", "Table", table));
    let mut k = node("table 1");
    k.instances.push(1);
    d.scene.children.push(k);

    let m = convert(&d);
    assert!(m.validate().is_empty());
    let leg = def_by_name(&m, "Leg");
    let table = def_by_name(&m, "Table");
    assert_eq!(m.definitions().count(), 2);
    let tbody = m.body(BodyId::Definition(table)).unwrap();
    assert_eq!(tbody.instances().count(), 2);
    assert!(tbody.instances().all(|(_, i)| i.definition == leg));
    assert_eq!(m.instances_of(leg).len(), 2);
    assert_eq!(m.instances_of(table).len(), 1);
}

#[test]
fn definition_cycle_is_an_error() {
    let mut d = doc();
    let g = add_mesh(&mut d, vec![tris(None, &quad_tris(QUAD))], &[]);
    let mut a = node("a");
    a.geometry.push(g);
    a.instances.push(1);
    let mut b = node("b");
    b.instances.push(0);
    d.definitions.push(definition("a", "A", a));
    d.definitions.push(definition("b", "B", b));
    d.scene.instances.push(0);
    assert_eq!(
        to_kernel(&d).unwrap_err(),
        ConvertError::Model(ModelError::Cycle)
    );
}

#[test]
fn every_body_is_valid_for_mixed_document() {
    let mut d = doc();
    d.materials.push(material("a", "A", [1.0, 0.0, 0.0, 1.0]));
    let g = add_mesh(
        &mut d,
        vec![
            tris(Some("s"), &cube_triangles()),
            segs(&[[[0.0, 0.0, 1.0], [0.0, 0.0, 3.0]]]),
        ],
        &[("s", 0)],
    );
    let mut dn = node("box");
    dn.geometry.push(g.clone());
    d.definitions.push(definition("box", "Box", dn));
    let mut k = node("box 1");
    k.instances.push(0);
    k.transform = translate(5.0, 0.0, 0.0);
    d.scene.children.push(k);
    let mut grp = node("grp");
    grp.geometry.push(g.clone());
    d.scene.children.push(grp);
    d.scene.geometry.push(g);

    let m = convert(&d);
    assert!(m.validate().is_empty());
    for body in
        std::iter::once(BodyId::Root).chain(m.definitions().map(|(id, _)| BodyId::Definition(id)))
    {
        let ctx = m.body(body).unwrap().geometry();
        assert_valid(ctx);
        assert_eq!(ctx.face_count(), 6);
        assert_eq!(ctx.edge_count(), 13);
    }
    let (front, back) = kernel_areas(&m);
    assert!((front[&Some(0)] - 18.0).abs() < 1e-9);
    assert!((back[&None] - 18.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------------------------
// The tables fixture (a typical exported model with components)

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tables.dae")
}

#[test]
fn tables_fixture_keeps_area_per_material() {
    let d = import_file(&fixture()).expect("fixture imports");
    let m = convert(&d);
    assert!(m.validate().is_empty());
    let (front, _) = kernel_areas(&m);
    assert_areas_match("tables", &front, &flat_areas(&flatten(&d)), 1e-5, 1e-9);
    let components = m.definitions().filter(|(_, info)| !info.group).count();
    assert_eq!(components, d.definitions.len());
    let triangles = flatten(&d).triangle_count();
    let mut faces = 0;
    for body in
        std::iter::once(BodyId::Root).chain(m.definitions().map(|(id, _)| BodyId::Definition(id)))
    {
        let ctx = m.body(body).unwrap().geometry();
        assert_valid(ctx);
        faces += ctx.face_count();
    }
    assert!(
        faces > 0 && faces < triangles,
        "faces {faces}, triangles {triangles}"
    );
}
