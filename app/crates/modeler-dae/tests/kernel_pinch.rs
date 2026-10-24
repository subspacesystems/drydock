// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-dae-pinch-faces: flat same-looking triangles whose merged outline would meet itself
//! become a few faces with clean outlines, not one face per triangle.

mod common;

use std::path::PathBuf;

use common::*;
use glam::{DMat4, DVec3};
use modeler_dae::{Primitive, PrimitiveKind, to_kernel};
use modeler_kernel::model::{self as km, BodyId, FaceMaterials};
use modeler_kernel::topology::Context;
use proptest::prelude::*;

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

fn assert_valid(ctx: &Context) {
    let violations = ctx.validate();
    assert!(violations.is_empty(), "violations: {violations:?}");
}

fn total_area(ctx: &Context) -> f64 {
    ctx.faces()
        .map(|(_, f)| face_area(ctx, f, DMat4::IDENTITY))
        .sum()
}

/// Every face lies in z = 0 and faces up.
fn assert_flat_up(ctx: &Context) {
    for (id, _) in ctx.faces() {
        let plane = ctx.face_plane(id).expect("face has a plane");
        assert!(
            plane.normal.dot(DVec3::Z) > 1.0 - 1e-9,
            "face {id:?} normal {}",
            plane.normal
        );
        assert!(plane.point.z.abs() < 1e-9);
    }
}

/// The two triangles of unit cell (x, y) in z = 0, counter-clockwise from above.
fn cell(x: i32, y: i32) -> [[[f32; 3]; 3]; 2] {
    let (x, y) = (x as f32, y as f32);
    let (a, b, c, d) = (
        [x, y, 0.0],
        [x + 1.0, y, 0.0],
        [x + 1.0, y + 1.0, 0.0],
        [x, y + 1.0, 0.0],
    );
    [[a, b, c], [a, c, d]]
}

/// The triangles of every cell of an `n` × `n` grid except `missing`.
fn grid_without(n: i32, missing: &[(i32, i32)]) -> Vec<[[f32; 3]; 3]> {
    let mut out = Vec::new();
    for y in 0..n {
        for x in 0..n {
            if !missing.contains(&(x, y)) {
                out.extend(cell(x, y));
            }
        }
    }
    out
}

/// Edges with two faces of the body, both flat and facing the same way.
fn flat_seams(ctx: &Context) -> usize {
    ctx.edges()
        .filter(|&(e, _)| {
            let fs = ctx.faces_of_edge(e);
            let [f, g] = fs[..] else { return false };
            let (Some(pf), Some(pg)) = (ctx.face_plane(f), ctx.face_plane(g)) else {
                return false;
            };
            pf.normal.dot(pg.normal) > 1.0 - 1e-9
        })
        .count()
}

// ---------------------------------------------------------------------------------------
// Outlines that touch themselves at a vertex

/// 3x3 cells minus the centre and the top-right cell: the ring's outline passes the
/// corner the two missing cells share twice. Before this task: 14 triangles.
#[test]
fn corner_pinch_gives_two_faces() {
    let m = convert(&root_doc(vec![tris(
        None,
        &grid_without(3, &[(1, 1), (2, 2)]),
    )]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 2);
    assert!((total_area(ctx) - 7.0).abs() < 1e-9);
    assert_flat_up(ctx);
    assert_valid(ctx);
}

/// 4x4 cells minus two cells that touch at one corner: two holes meeting at a vertex. Two
/// faces would do; the greedy split may need three.
#[test]
fn holes_touching_at_a_corner_give_two_or_three_faces() {
    let m = convert(&root_doc(vec![tris(
        None,
        &grid_without(4, &[(1, 1), (2, 2)]),
    )]));
    let ctx = root(&m);
    assert!(
        (2..=3).contains(&ctx.face_count()),
        "{} faces",
        ctx.face_count()
    );
    assert!((total_area(ctx) - 14.0).abs() < 1e-9);
    assert_flat_up(ctx);
    assert_valid(ctx);
}

/// 3x3 cells minus a corner cell and the centre: the hole touches the outer outline.
#[test]
fn hole_touching_the_outline_gives_two_faces() {
    let m = convert(&root_doc(vec![tris(
        None,
        &grid_without(3, &[(0, 0), (1, 1)]),
    )]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 2);
    assert!((total_area(ctx) - 7.0).abs() < 1e-9);
    assert_flat_up(ctx);
    assert_valid(ctx);
}

/// 5x5 cells minus the centre and the four cells diagonal to it: four pinches, one face per
/// piece the greedy split needs and no more than five.
#[test]
fn several_pinches_give_a_few_faces() {
    let missing = [(2, 2), (1, 1), (3, 1), (1, 3), (3, 3)];
    let m = convert(&root_doc(vec![tris(None, &grid_without(5, &missing))]));
    let ctx = root(&m);
    assert!(
        (2..=5).contains(&ctx.face_count()),
        "{} faces",
        ctx.face_count()
    );
    assert!((total_area(ctx) - 20.0).abs() < 1e-9);
    assert_flat_up(ctx);
    assert_valid(ctx);
}

/// Two squares that share only one corner, written as one triangle soup: two faces (they
/// never were one patch, and still aren't).
#[test]
fn squares_sharing_a_corner_stay_two_faces() {
    let mut triangles = cell(0, 0).to_vec();
    triangles.extend(cell(1, 1));
    let m = convert(&root_doc(vec![tris(None, &triangles)]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 2);
    for (_, f) in ctx.faces() {
        assert_eq!(f.outer.uses.len(), 4);
        assert!(f.inner.is_empty());
    }
    assert_valid(ctx);
}

/// The order the triangles are written in doesn't change how many faces come out.
#[test]
fn triangle_order_does_not_matter_for_a_single_pinch() {
    let mut triangles = grid_without(3, &[(1, 1), (2, 2)]);
    triangles.reverse();
    let m = convert(&root_doc(vec![tris(None, &triangles)]));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 2);
    assert!((total_area(ctx) - 7.0).abs() < 1e-9);
    assert_valid(ctx);
}

/// Splitting leaves only the edges it needs: two faces from one pinch share at most two
/// flat edges.
#[test]
fn split_leaves_few_flat_seams() {
    let m = convert(&root_doc(vec![tris(
        None,
        &grid_without(3, &[(1, 1), (2, 2)]),
    )]));
    let ctx = root(&m);
    let seams = flat_seams(ctx);
    assert!((1..=2).contains(&seams), "{seams} flat seams");
}

/// The same pinched ring converts the same way twice.
#[test]
fn split_is_deterministic() {
    let d = root_doc(vec![tris(None, &grid_without(4, &[(1, 1), (2, 2)]))]);
    let outlines = |m: &km::Model| -> Vec<Vec<DVec3>> {
        let ctx = root(m);
        ctx.faces()
            .map(|(_, f)| ctx.loop_points(&f.outer))
            .collect()
    };
    assert_eq!(outlines(&convert(&d)), outlines(&convert(&d)));
}

// ---------------------------------------------------------------------------------------
// Outlines that touch themselves along an edge (no shared vertex)

/// A U of cells with a bar on top of its left arm that reaches across the gap and over the
/// right arm: the bar's lower edge runs from (0.5, 2) to (2.5, 2) with no vertex at (1, 2)
/// or (2, 2), where the cells below have corners. The merged outline would pass those
/// points twice without a shared vertex, so it is split; every face is valid.
#[test]
fn outline_touching_an_edge_midway_is_split() {
    let mut triangles = Vec::new();
    // Left arm: cells (0, 0), (0, 1); bottom: (1, 0); right arm: (2, 0), (2, 1).
    for (x, y) in [(0, 0), (0, 1), (1, 0), (2, 0), (2, 1)] {
        triangles.extend(cell(x, y));
    }
    // Top bar over the left arm only, joined to it by the edge (0, 2)-(0.5, 2) and
    // reaching across to x = 2.5. Its lower edge ends at (0.5, 2) and (2.5, 2).
    let (a, b, c, d, e, f) = (
        [0.0f32, 2.0, 0.0],
        [0.5, 2.0, 0.0],
        [2.5, 2.0, 0.0],
        [2.5, 3.0, 0.0],
        [0.5, 3.0, 0.0],
        [0.0, 3.0, 0.0],
    );
    triangles.extend([[a, b, e], [a, e, f], [b, c, d], [b, d, e]]);
    // The left arm's top edge is (0, 2)-(1, 2); split it at (0.5, 2) so the bar joins.
    let left_top = cell(0, 1);
    triangles.retain(|t| *t != left_top[0] && *t != left_top[1]);
    let (p, q, r, s) = (
        [0.0f32, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 2.0, 0.0],
        [0.0, 2.0, 0.0],
    );
    triangles.extend([[p, q, b], [q, r, b], [p, b, s]]);
    let m = convert(&root_doc(vec![tris(None, &triangles)]));
    let ctx = root(&m);
    assert!(
        (2..=3).contains(&ctx.face_count()),
        "{} faces",
        ctx.face_count()
    );
    for (id, _) in ctx.faces() {
        assert!(
            ctx.face_violation(id).is_none(),
            "{:?}",
            ctx.face_violation(id)
        );
    }
    assert!((total_area(ctx) - 7.5).abs() < 1e-9);
    assert_flat_up(ctx);
}

// ---------------------------------------------------------------------------------------
// Materials

fn textured(id: &str) -> modeler_dae::Material {
    let mut m = material(id, id, [1.0, 1.0, 1.0, 1.0]);
    m.texture = Some(PathBuf::from(format!("/x/model/{id}.png")));
    m
}

/// Triangles with texture coordinates `uv(p)` at every corner.
fn tris_uv(symbol: &str, triangles: &[[[f32; 3]; 3]], uv: fn([f32; 3]) -> [f32; 2]) -> Primitive {
    Primitive {
        kind: PrimitiveKind::Triangles,
        material_symbol: Some(symbol.to_string()),
        positions: triangles.iter().flatten().copied().collect(),
        normals: Vec::new(),
        texcoords: triangles.iter().flatten().map(|&p| uv(p)).collect(),
        indices: (0..triangles.len() as u32 * 3).collect(),
    }
}

fn affine_uv(p: [f32; 3]) -> [f32; 2] {
    [0.5 * p[0] + 0.25, 0.75 * p[1] - 0.5]
}

/// A textured, two-sided pinched ring: every face keeps the front material, the twin's
/// back material and the file's texture placement.
#[test]
fn split_faces_keep_materials_and_placement() {
    let triangles = grid_without(3, &[(1, 1), (2, 2)]);
    let backs: Vec<[[f32; 3]; 3]> = triangles.iter().map(|t| [t[0], t[2], t[1]]).collect();
    let mut d = doc();
    d.materials = vec![
        textured("wood"),
        material("paint", "paint", [0.2, 0.4, 0.6, 1.0]),
    ];
    let g = add_mesh(
        &mut d,
        vec![tris_uv("m", &triangles, affine_uv), tris(Some("n"), &backs)],
        &[("m", 0), ("n", 1)],
    );
    d.scene.geometry.push(g);
    let m = convert(&d);
    let ids: Vec<km::MaterialId> = m.materials().map(|(id, _)| id).collect();
    let body = m.body(BodyId::Root).unwrap();
    let ctx = body.geometry();
    assert_eq!(ctx.face_count(), 2);
    for (id, f) in ctx.faces() {
        let fm: FaceMaterials = body.face_materials(id);
        assert_eq!(fm.front, Some(ids[0]));
        assert_eq!(fm.back, Some(ids[1]));
        let map = fm.front_map.expect("front placement kept");
        for p in ctx.loop_points(&f.outer) {
            let want = affine_uv([p.x as f32, p.y as f32, p.z as f32]);
            let got = map.apply(p);
            assert!(
                (got.x - want[0] as f64).abs() < 1e-5 && (got.y - want[1] as f64).abs() < 1e-5,
                "at {p}: {got} vs {want:?}"
            );
        }
    }
    assert_valid(ctx);
}

/// Triangles of different materials in a pinched ring never share a face.
#[test]
fn split_never_mixes_materials() {
    let mut d = doc();
    d.materials = vec![
        material("a", "a", [1.0, 0.0, 0.0, 1.0]),
        material("b", "b", [0.0, 0.0, 1.0, 1.0]),
    ];
    // Left column material a, the rest b; the ring is pinched at (2, 2).
    let all = grid_without(3, &[(1, 1), (2, 2)]);
    let (left, rest): (Vec<_>, Vec<_>) = all.into_iter().partition(|t| t[0][0] < 1.0);
    let g = add_mesh(
        &mut d,
        vec![tris(Some("m"), &left), tris(Some("n"), &rest)],
        &[("m", 0), ("n", 1)],
    );
    d.scene.geometry.push(g);
    let m = convert(&d);
    let body = m.body(BodyId::Root).unwrap();
    let ctx = body.geometry();
    let ids: Vec<km::MaterialId> = m.materials().map(|(id, _)| id).collect();
    let mut area = [0.0; 2];
    for (id, f) in ctx.faces() {
        let k = ids
            .iter()
            .position(|&m| Some(m) == body.face_materials(id).front)
            .expect("face has a material");
        area[k] += face_area(ctx, f, DMat4::IDENTITY);
    }
    assert!((area[0] - 3.0).abs() < 1e-9, "{area:?}");
    assert!((area[1] - 4.0).abs() < 1e-9, "{area:?}");
    // a: the left column; b: an L of three cells and the lone top-middle cell.
    assert_eq!(ctx.face_count(), 3);
    assert_valid(ctx);
}

// ---------------------------------------------------------------------------------------
// Random flat grids

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Any subset of a flat 6x6 grid, either diagonal per cell, in any order: valid faces,
    /// area kept, all facing up, and never more faces than cells.
    #[test]
    fn random_flat_grids_split_cleanly(
        present in proptest::collection::vec(prop::bool::weighted(0.75), 36),
        flip in proptest::collection::vec(any::<bool>(), 36),
        seed in any::<u64>(),
    ) {
        let mut triangles = Vec::new();
        for y in 0..6 {
            for x in 0..6 {
                let c = (y * 6 + x) as usize;
                if !present[c] {
                    continue;
                }
                let [t0, t1] = cell(x, y);
                if flip[c] {
                    let (a, b, cc, d) = (t0[0], t0[1], t0[2], t1[2]);
                    triangles.push([a, b, d]);
                    triangles.push([b, cc, d]);
                } else {
                    triangles.push(t0);
                    triangles.push(t1);
                }
            }
        }
        // Deterministic shuffle from `seed`.
        let mut state = seed | 1;
        for i in (1..triangles.len()).rev() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            triangles.swap(i, (state % (i as u64 + 1)) as usize);
        }
        let cells = present.iter().filter(|&&p| p).count();
        let m = convert(&root_doc(vec![tris(None, &triangles)]));
        let ctx = root(&m);
        let violations = ctx.validate();
        prop_assert!(violations.is_empty(), "violations: {:?}", violations);
        prop_assert!((total_area(ctx) - cells as f64).abs() < 1e-9);
        for (id, _) in ctx.faces() {
            let plane = ctx.face_plane(id).unwrap();
            prop_assert!(plane.normal.dot(DVec3::Z) > 1.0 - 1e-9);
        }
        prop_assert!(ctx.face_count() <= cells.max(1), "{} faces for {} cells", ctx.face_count(), cells);
    }
}
