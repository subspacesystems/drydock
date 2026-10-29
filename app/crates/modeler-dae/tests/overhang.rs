// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-overhang: which faces need support, and the tinted copy of a scene.
//! See docs/specs/m7-overhang.md.

use glam::DVec3;
use modeler_dae::{
    Aabb, Batch, DEFAULT_OVERHANG_DEGREES, FlatScene, LineBatch, Material, OVERHANG_COLOR,
    is_overhang, tint_overhangs,
};

/// Unit normal leaning `deg` degrees from vertical, facing down (0 = a wall facing +X,
/// 90 = straight down).
fn leaning(deg: f64) -> DVec3 {
    let r = deg.to_radians();
    DVec3::new(r.cos(), 0.0, -r.sin())
}

fn material(id: &str) -> Material {
    Material {
        id: id.into(),
        name: id.into(),
        color: [0.5, 0.5, 0.5, 1.0],
        texture: None,
        opacity: 1.0,
        unlit: false,
    }
}

/// Unit cube from `lo` to `lo + 1`, outward fronts (counter-clockwise from outside), one
/// batch, normals per corner. Faces in order: bottom, top, -Y, +X, +Y, -X.
fn cube(lo: [f32; 3], mat: Option<usize>) -> Batch {
    let [x, y, z] = lo;
    let faces: [([[f32; 3]; 4], [f32; 3]); 6] = [
        (
            [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]],
            [0., 0., -1.],
        ),
        (
            [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]],
            [0., 0., 1.],
        ),
        (
            [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
            [0., -1., 0.],
        ),
        (
            [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]],
            [1., 0., 0.],
        ),
        (
            [[1., 1., 0.], [0., 1., 0.], [0., 1., 1.], [1., 1., 1.]],
            [0., 1., 0.],
        ),
        (
            [[0., 1., 0.], [0., 0., 0.], [0., 0., 1.], [0., 1., 1.]],
            [-1., 0., 0.],
        ),
    ];
    let mut b = Batch {
        material: mat,
        ..Batch::default()
    };
    for (corners, n) in faces {
        let base = b.positions.len() as u32;
        for c in corners {
            b.positions.push([c[0] + x, c[1] + y, c[2] + z]);
            b.normals.push(n);
        }
        b.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    b
}

/// Square at height `z` from (x0, y0) to (x0 + s, y0 + s); facing down when `down`.
fn square(x0: f32, y0: f32, s: f32, z: f32, down: bool, mat: Option<usize>) -> Batch {
    let n = if down { -1.0 } else { 1.0 };
    Batch {
        material: mat,
        positions: vec![
            [x0, y0, z],
            [x0 + s, y0, z],
            [x0 + s, y0 + s, z],
            [x0, y0 + s, z],
        ],
        normals: vec![[0.0, 0.0, n]; 4],
        texcoords: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        indices: if down {
            vec![0, 2, 1, 0, 3, 2]
        } else {
            vec![0, 1, 2, 0, 2, 3]
        },
    }
}

fn bounds_of(batches: &[Batch]) -> Option<Aabb> {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    let mut any = false;
    for p in batches.iter().flat_map(|b| &b.positions) {
        any = true;
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    any.then_some(Aabb { min: lo, max: hi })
}

fn scene(materials: Vec<Material>, batches: Vec<Batch>) -> FlatScene {
    let bounds = bounds_of(&batches);
    FlatScene {
        materials,
        batches,
        lines: vec![LineBatch {
            material: None,
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
        }],
        bounds,
    }
}

fn same_batch(a: &Batch, b: &Batch) -> bool {
    a.material == b.material
        && a.positions == b.positions
        && a.normals == b.normals
        && a.texcoords == b.texcoords
        && a.indices == b.indices
}

fn same_material(a: &Material, b: &Material) -> bool {
    a.id == b.id
        && a.name == b.name
        && a.color == b.color
        && a.texture == b.texture
        && a.opacity == b.opacity
        && a.unlit == b.unlit
}

fn assert_same_scene(a: &FlatScene, b: &FlatScene) {
    assert_eq!(a.materials.len(), b.materials.len(), "material count");
    for (x, y) in a.materials.iter().zip(&b.materials) {
        assert!(same_material(x, y), "material {x:?} vs {y:?}");
    }
    assert_eq!(a.batches.len(), b.batches.len(), "batch count");
    for (x, y) in a.batches.iter().zip(&b.batches) {
        assert!(same_batch(x, y), "batch {x:?} vs {y:?}");
    }
    assert_eq!(a.lines.len(), b.lines.len());
    for (x, y) in a.lines.iter().zip(&b.lines) {
        assert_eq!(x.material, y.material);
        assert_eq!(x.positions, y.positions);
    }
    assert_eq!(a.bounds, b.bounds);
}

/// The triangles of a batch as position triples.
fn triangles(b: &Batch) -> Vec<[[f32; 3]; 3]> {
    b.indices
        .chunks(3)
        .map(|t| {
            [
                b.positions[t[0] as usize],
                b.positions[t[1] as usize],
                b.positions[t[2] as usize],
            ]
        })
        .collect()
}

// ---- is_overhang ----

#[test]
fn the_default_angle_is_45_degrees() {
    assert_eq!(DEFAULT_OVERHANG_DEGREES, 45.0);
}

#[test]
fn a_ceiling_overhangs_a_floor_and_a_wall_do_not() {
    assert!(is_overhang(DVec3::NEG_Z, 45.0));
    assert!(!is_overhang(DVec3::Z, 45.0));
    assert!(!is_overhang(DVec3::X, 45.0));
    assert!(!is_overhang(DVec3::NEG_Y, 45.0));
}

#[test]
fn surfaces_leaning_past_the_limit_overhang() {
    assert!(is_overhang(leaning(50.0), 45.0));
    assert!(is_overhang(leaning(46.0), 45.0));
    assert!(!is_overhang(leaning(40.0), 45.0));
    assert!(!is_overhang(leaning(44.0), 45.0));
    assert!(is_overhang(leaning(31.0), 30.0));
    assert!(!is_overhang(leaning(29.0), 30.0));
}

#[test]
fn a_surface_exactly_at_the_limit_does_not_overhang() {
    assert!(!is_overhang(leaning(45.0), 45.0));
    assert!(!is_overhang(leaning(60.0), 60.0));
}

#[test]
fn surfaces_facing_up_never_overhang() {
    for deg in [0.0, 10.0, 45.0, 89.0] {
        let up = leaning(deg) * DVec3::new(1.0, 1.0, -1.0);
        assert!(!is_overhang(up, 0.0), "{deg}");
    }
}

#[test]
fn the_normal_length_does_not_matter() {
    assert!(is_overhang(DVec3::new(0.0, 0.0, -5.0), 45.0));
    assert!(is_overhang(leaning(50.0) * 1e-6, 45.0));
    assert!(!is_overhang(leaning(40.0) * 1e6, 45.0));
}

#[test]
fn zero_and_non_finite_normals_do_not_overhang() {
    assert!(!is_overhang(DVec3::ZERO, 45.0));
    assert!(!is_overhang(DVec3::new(0.0, 0.0, f64::NAN), 45.0));
    assert!(!is_overhang(DVec3::new(f64::INFINITY, 0.0, -1.0), 45.0));
}

#[test]
fn ninety_degrees_and_more_allows_everything() {
    assert!(!is_overhang(DVec3::NEG_Z, 90.0));
    assert!(!is_overhang(DVec3::NEG_Z, 120.0));
    assert!(!is_overhang(DVec3::NEG_Z, f64::NAN));
}

#[test]
fn zero_degrees_and_less_flags_any_downward_lean() {
    assert!(is_overhang(leaning(1.0), 0.0));
    assert!(is_overhang(leaning(1.0), -10.0));
    assert!(!is_overhang(DVec3::X, 0.0));
    assert!(!is_overhang(DVec3::X, -10.0));
}

// ---- tint_overhangs ----

#[test]
fn a_cube_on_the_bed_is_not_tinted() {
    let s = scene(vec![material("m")], vec![cube([0.0, 0.0, 0.0], Some(0))]);
    let t = tint_overhangs(&s, 45.0);
    assert_same_scene(&t, &s);
}

#[test]
fn an_empty_scene_stays_empty() {
    let s = FlatScene::default();
    let t = tint_overhangs(&s, 45.0);
    assert!(t.materials.is_empty());
    assert!(t.batches.is_empty());
    assert!(t.lines.is_empty());
    assert_eq!(t.bounds, None);
}

#[test]
fn a_raised_cube_gets_its_bottom_tinted() {
    // A floor square at z = 0 sets the bed; the cube floats at z = 1..2.
    let floor = square(-1.0, -1.0, 4.0, 0.0, false, Some(0));
    let block = cube([0.0, 0.0, 1.0], Some(1));
    let s = scene(
        vec![material("floor"), material("block")],
        vec![floor.clone(), block.clone()],
    );
    let t = tint_overhangs(&s, 45.0);

    assert_eq!(t.materials.len(), 3);
    assert!(same_material(&t.materials[0], &s.materials[0]));
    assert!(same_material(&t.materials[1], &s.materials[1]));
    let m = &t.materials[2];
    assert_eq!(m.id, "overhang");
    assert_eq!(m.name, "Overhang");
    assert_eq!(m.color, OVERHANG_COLOR);
    assert_eq!(m.texture, None);
    assert_eq!(m.opacity, 1.0);
    assert!(!m.unlit);

    assert_eq!(t.batches.len(), 3);
    assert!(same_batch(&t.batches[0], &floor));
    // The cube keeps its vertex arrays and loses its two bottom triangles.
    let kept = &t.batches[1];
    assert_eq!(kept.material, Some(1));
    assert_eq!(kept.positions, block.positions);
    assert_eq!(kept.normals, block.normals);
    assert_eq!(kept.texcoords, block.texcoords);
    assert_eq!(kept.indices, block.indices[6..].to_vec());

    let tinted = &t.batches[2];
    assert_eq!(tinted.material, Some(2));
    assert_eq!(tinted.indices, vec![0, 1, 2, 3, 4, 5]);
    assert!(tinted.texcoords.is_empty());
    assert_eq!(tinted.normals, vec![[0.0, 0.0, -1.0]; 6]);
    let want: Vec<[f32; 3]> = block.indices[..6]
        .iter()
        .map(|&i| block.positions[i as usize])
        .collect();
    assert_eq!(tinted.positions, want);

    assert_eq!(t.lines.len(), 1);
    assert_eq!(t.lines[0].positions, s.lines[0].positions);
    assert_eq!(t.bounds, s.bounds);
}

#[test]
fn the_source_scene_is_left_alone() {
    let s = scene(
        vec![material("m")],
        vec![
            square(-1.0, -1.0, 4.0, 0.0, false, Some(0)),
            cube([0.0, 0.0, 1.0], Some(0)),
        ],
    );
    let before = s.clone();
    let _ = tint_overhangs(&s, 45.0);
    assert_same_scene(&s, &before);
}

#[test]
fn a_batch_with_every_triangle_tinted_is_dropped() {
    let floor = square(-1.0, -1.0, 4.0, 0.0, false, Some(0));
    let ceiling = square(0.0, 0.0, 1.0, 2.0, true, Some(1));
    let wall = square(5.0, 5.0, 1.0, 0.5, false, None);
    let s = scene(
        vec![material("a"), material("b")],
        vec![floor.clone(), ceiling.clone(), wall.clone()],
    );
    let t = tint_overhangs(&s, 45.0);
    assert_eq!(t.batches.len(), 3, "floor, the up-facing square, the tint");
    assert!(same_batch(&t.batches[0], &floor));
    assert!(same_batch(&t.batches[1], &wall));
    assert_eq!(t.batches[2].material, Some(2));
    assert_eq!(triangles(&t.batches[2]), triangles(&ceiling));
}

#[test]
fn the_default_material_batch_is_tinted_too() {
    let floor = square(-1.0, -1.0, 4.0, 0.0, false, None);
    let block = cube([0.0, 0.0, 1.0], None);
    let s = scene(vec![], vec![floor, block.clone()]);
    let t = tint_overhangs(&s, 45.0);
    assert_eq!(t.materials.len(), 1);
    assert_eq!(t.materials[0].id, "overhang");
    assert_eq!(t.batches.len(), 3);
    assert_eq!(t.batches[1].material, None);
    assert_eq!(t.batches[1].indices, block.indices[6..].to_vec());
    assert_eq!(t.batches[2].material, Some(0));
    assert_eq!(t.batches[2].indices.len(), 6);
}

#[test]
fn tinted_triangles_keep_walk_order_and_winding() {
    // Two floating ceilings in different batches, and one in the middle of a batch.
    let floor = square(-5.0, -5.0, 20.0, 0.0, false, None);
    let a = square(0.0, 0.0, 1.0, 3.0, true, Some(0));
    let mut b = square(2.0, 0.0, 1.0, 1.0, false, Some(0));
    // Append a downward square after b's own two triangles.
    let extra = square(4.0, 0.0, 1.0, 2.0, true, Some(0));
    let base = b.positions.len() as u32;
    b.positions.extend_from_slice(&extra.positions);
    b.normals.extend_from_slice(&extra.normals);
    b.texcoords.extend_from_slice(&extra.texcoords);
    b.indices.extend(extra.indices.iter().map(|i| i + base));
    let s = scene(vec![material("m")], vec![floor, a.clone(), b.clone()]);
    let t = tint_overhangs(&s, 45.0);

    let tinted = t.batches.last().unwrap();
    let mut want = triangles(&a);
    want.extend(triangles(&extra));
    assert_eq!(triangles(tinted), want);
    // b keeps its upward square only.
    let kept = &t.batches[t.batches.len() - 2];
    assert_eq!(kept.indices, vec![0, 1, 2, 0, 2, 3]);
    assert_eq!(kept.texcoords, b.texcoords);
}

#[test]
fn the_normal_comes_from_the_vertex_order_not_the_stored_normals() {
    // Stored normals say "up", the winding says "down": it's tinted.
    let floor = square(-1.0, -1.0, 4.0, 0.0, false, None);
    let mut lying = square(0.0, 0.0, 1.0, 1.0, true, None);
    lying.normals = vec![[0.0, 0.0, 1.0]; 4];
    let s = scene(vec![], vec![floor, lying]);
    let t = tint_overhangs(&s, 45.0);
    let tinted = t.batches.last().unwrap();
    assert_eq!(tinted.material, Some(0));
    assert_eq!(tinted.indices.len(), 6);
    assert_eq!(
        tinted.normals,
        vec![[0.0, 0.0, 1.0]; 6],
        "normals copied as stored"
    );
}

#[test]
fn faces_within_a_tenth_of_a_millimetre_of_the_bed_are_on_it() {
    let floor = square(-1.0, -1.0, 4.0, 0.0, false, None);
    let near = square(0.0, 0.0, 1.0, 0.00005, true, None);
    let s = scene(vec![], vec![floor.clone(), near]);
    assert_same_scene(&tint_overhangs(&s, 45.0), &s);

    let above = square(0.0, 0.0, 1.0, 0.001, true, None);
    let s = scene(vec![], vec![floor, above]);
    let t = tint_overhangs(&s, 45.0);
    assert_eq!(t.materials.len(), 1);
    assert_eq!(t.batches.last().unwrap().indices.len(), 6);
}

#[test]
fn the_bed_is_the_lowest_point_of_the_scene() {
    // A cube sitting at z = 3 with nothing lower: its bottom is on the bed.
    let s = scene(vec![], vec![cube([0.0, 0.0, 3.0], None)]);
    assert_same_scene(&tint_overhangs(&s, 45.0), &s);
}

#[test]
fn a_bottom_face_only_partly_on_the_bed_is_tinted() {
    // A downward triangle with one corner on the bed and two above it.
    let floor = square(-1.0, -1.0, 4.0, 0.0, false, None);
    let tri = Batch {
        material: None,
        positions: vec![[0.0, 0.0, 0.0], [0.0, 1.0, 0.2], [1.0, 0.0, 0.2]],
        normals: vec![[0.0, 0.0, -1.0]; 3],
        texcoords: vec![],
        indices: vec![0, 1, 2],
    };
    let n = DVec3::new(0.0, 1.0, 0.2).cross(DVec3::new(1.0, 0.0, 0.2));
    assert!(
        is_overhang(n, 45.0),
        "test setup: the triangle leans past 45°"
    );
    let s = scene(vec![], vec![floor, tri]);
    let t = tint_overhangs(&s, 45.0);
    assert_eq!(t.batches.last().unwrap().indices, vec![0, 1, 2]);
}

#[test]
fn the_angle_decides_which_slopes_are_tinted() {
    // A downward slope leaning 60° from vertical, raised off the bed.
    let floor = square(-5.0, -5.0, 10.0, 0.0, false, None);
    let r = 60f32.to_radians();
    // Normal (cos r, 0, -sin r): the plane through the origin spanned by Y and
    // (sin r, 0, cos r), lifted to z >= 1.
    let u = [r.sin(), 0.0, r.cos()];
    let slope = Batch {
        material: None,
        positions: vec![[0.0, 0.0, 1.0], [u[0], 0.0, 1.0 + u[2]], [0.0, 1.0, 1.0]],
        normals: vec![[0.0, 0.0, -1.0]; 3],
        texcoords: vec![],
        indices: vec![0, 2, 1],
    };
    let s = scene(vec![], vec![floor, slope]);
    for (deg, tinted) in [(45.0, true), (59.0, true), (61.0, false), (90.0, false)] {
        let t = tint_overhangs(&s, deg);
        assert_eq!(t.materials.len(), usize::from(tinted), "{deg}°");
        let last = t.batches.last().unwrap();
        assert_eq!(last.material == Some(0), tinted, "{deg}°");
    }
    let t = tint_overhangs(&s, 0.0);
    assert_eq!(t.materials.len(), 1);
}

#[test]
fn degenerate_triangles_are_never_tinted() {
    let floor = square(-1.0, -1.0, 4.0, 0.0, false, None);
    let sliver = Batch {
        material: None,
        positions: vec![[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [2.0, 0.0, 1.0]],
        normals: vec![[0.0, 0.0, -1.0]; 3],
        texcoords: vec![],
        indices: vec![0, 1, 2],
    };
    let s = scene(vec![], vec![floor, sliver]);
    assert_same_scene(&tint_overhangs(&s, 0.0), &s);
}
