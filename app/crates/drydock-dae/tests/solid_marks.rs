// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-solid-ui: the problem marks of one group or component and the scene tint
//! (`docs/specs/m7-solid-ui.md`).

use std::time::{Duration, Instant};

use drydock_dae::{
    Aabb, Batch, FlatScene, LineBatch, MARK_TOLERANCE_M, Material, PROBLEM_COLOR, SolidMarks,
    solid_marks, tint_problems,
};
use drydock_kernel::model::*;
use drydock_kernel::topology::*;
use glam::{DMat4, DVec3};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// The 8 corners of the box `0..s`, numbered bottom (0..4, counter-clockwise from above)
/// then top (4..8).
fn corners(s: f64) -> Vec<DVec3> {
    vec![
        v(0.0, 0.0, 0.0),
        v(s, 0.0, 0.0),
        v(s, s, 0.0),
        v(0.0, s, 0.0),
        v(0.0, 0.0, s),
        v(s, 0.0, s),
        v(s, s, s),
        v(0.0, s, s),
    ]
}

/// Box faces with outward fronts: bottom, top, front (y = 0), back (y = s), right (x = s),
/// left.
const FACES: [[usize; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [2, 3, 7, 6],
    [1, 2, 6, 5],
    [3, 0, 4, 7],
];

/// A definition holding a box of side `s` without the faces in `skip` and with the faces in
/// `flip` turned round, plus `loose` extra edges (pairs of box corners), placed in the root
/// by `transform`. Returns the model, the instance and the face ids that were built.
fn box_instance(
    s: f64,
    skip: &[usize],
    flip: &[usize],
    loose: &[(usize, usize)],
    transform: DMat4,
) -> (Model, InstanceId, Vec<FaceId>) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let definition = model.add_definition(
        DefinitionInfo {
            name: "Box".into(),
            group: true,
        },
        &mut log,
    );
    let body = BodyId::Definition(definition);
    let points = corners(s);
    let ids = model
        .edit_geometry(body, &mut log, |ctx, edit| {
            let vs: Vec<VertexId> = points.iter().map(|&p| ctx.insert_vertex(p, edit)).collect();
            let mut edges = std::collections::BTreeMap::new();
            let mut ids = Vec::new();
            for (n, quad) in FACES.iter().enumerate() {
                if skip.contains(&n) {
                    continue;
                }
                let mut quad = quad.to_vec();
                if flip.contains(&n) {
                    quad.reverse();
                }
                let mut uses = Vec::new();
                for i in 0..4 {
                    let (a, b) = (quad[i], quad[(i + 1) % 4]);
                    let key = (a.min(b), a.max(b));
                    let e = *edges
                        .entry(key)
                        .or_insert_with(|| ctx.insert_edge(vs[key.0], vs[key.1], edit).unwrap());
                    uses.push(if a < b {
                        EdgeUse::forward(e)
                    } else {
                        EdgeUse::backward(e)
                    });
                }
                ids.push(
                    ctx.insert_face(
                        Face {
                            outer: Loop { uses },
                            inner: vec![],
                        },
                        edit,
                    )
                    .unwrap(),
                );
            }
            for &(a, b) in loose {
                let key = (a.min(b), a.max(b));
                edges
                    .entry(key)
                    .or_insert_with(|| ctx.insert_edge(vs[key.0], vs[key.1], edit).unwrap());
            }
            ids
        })
        .unwrap();
    let instance = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition,
                transform,
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    (model, instance, ids)
}

fn near(a: DVec3, b: DVec3) -> bool {
    a.distance(b) < 1e-9
}

// ---- solid_marks ----

#[test]
fn a_clean_box_has_no_marks() {
    let (model, instance, _) = box_instance(1.0, &[], &[], &[], DMat4::IDENTITY);
    let marks = solid_marks(&model, BodyId::Root, instance, DMat4::IDENTITY);
    assert!(marks.is_empty());
    assert_eq!(marks, SolidMarks::default());
}

#[test]
fn an_open_box_marks_the_rim_in_world_space() {
    let (model, instance, _) = box_instance(
        0.5,
        &[1],
        &[],
        &[],
        DMat4::from_translation(v(10.0, 0.0, 0.0)),
    );
    let outer = DMat4::from_translation(v(0.0, 5.0, 0.0));
    let marks = solid_marks(&model, BodyId::Root, instance, outer);
    assert!(marks.triangles.is_empty());
    assert_eq!(marks.segments.len(), 4);
    let rim = [
        (v(0.0, 0.0, 0.5), v(0.5, 0.0, 0.5)),
        (v(0.5, 0.0, 0.5), v(0.5, 0.5, 0.5)),
        (v(0.5, 0.5, 0.5), v(0.0, 0.5, 0.5)),
        (v(0.0, 0.5, 0.5), v(0.0, 0.0, 0.5)),
    ];
    let shift = v(10.0, 5.0, 0.0);
    for (a, b) in rim {
        let (a, b) = (a + shift, b + shift);
        assert!(
            marks
                .segments
                .iter()
                .any(|s| (near(s[0], a) && near(s[1], b)) || (near(s[0], b) && near(s[1], a))),
            "missing rim edge {a:?} {b:?} in {:?}",
            marks.segments
        );
    }
}

#[test]
fn a_turned_face_marks_its_two_triangles() {
    let (model, instance, _) = box_instance(2.0, &[], &[4], &[], DMat4::IDENTITY);
    let marks = solid_marks(&model, BodyId::Root, instance, DMat4::IDENTITY);
    assert!(marks.segments.is_empty());
    assert_eq!(marks.triangles.len(), 2);
    let mut seen = Vec::new();
    for triangle in &marks.triangles {
        for &corner in triangle {
            assert!((corner.x - 2.0).abs() < 1e-9, "{corner:?} is not on x = 2");
            seen.push(corner);
        }
    }
    for want in [
        v(2.0, 0.0, 0.0),
        v(2.0, 2.0, 0.0),
        v(2.0, 2.0, 2.0),
        v(2.0, 0.0, 2.0),
    ] {
        assert!(seen.iter().any(|&c| near(c, want)), "no corner at {want:?}");
    }
}

#[test]
fn a_loose_edge_is_marked() {
    let (model, instance, _) = box_instance(1.0, &[], &[], &[(0, 6)], DMat4::IDENTITY);
    let marks = solid_marks(&model, BodyId::Root, instance, DMat4::IDENTITY);
    assert!(marks.triangles.is_empty());
    assert_eq!(marks.segments.len(), 1);
    let s = marks.segments[0];
    let (a, b) = (v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    assert!((near(s[0], a) && near(s[1], b)) || (near(s[0], b) && near(s[1], a)));
}

#[test]
fn the_transform_is_applied_instance_first_then_outer() {
    let rotate = DMat4::from_rotation_z(std::f64::consts::FRAC_PI_2);
    let (model, instance, _) = box_instance(1.0, &[1], &[], &[], rotate);
    let outer = DMat4::from_translation(v(0.0, 0.0, 3.0));
    let marks = solid_marks(&model, BodyId::Root, instance, outer);
    assert_eq!(marks.segments.len(), 4);
    // The rim corner (1, 0, 1) turns to (0, 1, 1), then lifts to z = 4.
    assert!(
        marks
            .segments
            .iter()
            .flatten()
            .any(|&p| near(p, v(0.0, 1.0, 4.0)))
    );
}

#[test]
fn a_missing_instance_or_body_has_no_marks() {
    let (model, instance, _) = box_instance(1.0, &[1], &[], &[], DMat4::IDENTITY);
    let none = solid_marks(
        &model,
        BodyId::Root,
        InstanceId(instance.0 + 99),
        DMat4::IDENTITY,
    );
    assert!(none.is_empty());
    let other = BodyId::Definition(DefinitionId(999));
    assert!(solid_marks(&model, other, instance, DMat4::IDENTITY).is_empty());
}

#[test]
fn nested_instances_are_not_looked_into() {
    // A clean box whose definition also holds a broken (open) box.
    let (mut model, inner, _) = box_instance(1.0, &[1], &[], &[], DMat4::IDENTITY);
    let broken = model.root().instance(inner).unwrap().definition;
    let mut log = ModelEdit::new();
    let outer_def = model.add_definition(
        DefinitionInfo {
            name: "Outer".into(),
            group: true,
        },
        &mut log,
    );
    model
        .add_instance(
            BodyId::Definition(outer_def),
            Instance {
                definition: broken,
                transform: DMat4::IDENTITY,
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    let outer_instance = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: outer_def,
                transform: DMat4::IDENTITY,
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    let marks = solid_marks(&model, BodyId::Root, outer_instance, DMat4::IDENTITY);
    assert!(marks.is_empty(), "{marks:?}");
}

// ---- tint_problems ----

fn grey() -> Material {
    Material {
        id: "grey".into(),
        name: "grey".into(),
        color: [0.8, 0.8, 0.8, 1.0],
        texture: None,
        opacity: 1.0,
        unlit: false,
    }
}

/// A unit square in z = 0 as two triangles, (0 1 2) and (0 2 3).
fn square_batch() -> Batch {
    Batch {
        material: Some(0),
        positions: vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        texcoords: vec![],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

fn square_scene() -> FlatScene {
    FlatScene {
        materials: vec![grey()],
        batches: vec![square_batch()],
        lines: vec![LineBatch {
            material: None,
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
        }],
        bounds: Some(Aabb {
            min: [0.0, 0.0, 0.0],
            max: [1.0, 1.0, 0.0],
        }),
    }
}

fn tri(a: DVec3, b: DVec3, c: DVec3) -> [DVec3; 3] {
    [a, b, c]
}

fn problem_material(scene: &FlatScene) -> usize {
    let last = scene.materials.len() - 1;
    let m = &scene.materials[last];
    assert_eq!(m.name, "Problem");
    assert_eq!(m.color, PROBLEM_COLOR);
    last
}

#[test]
fn a_matching_triangle_moves_to_the_problem_batch() {
    let scene = square_scene();
    let marks = SolidMarks {
        triangles: vec![tri(v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0))],
        segments: vec![],
    };
    let out = tint_problems(&scene, &marks);
    assert_eq!(out.materials.len(), 2);
    let m = problem_material(&out);
    assert_eq!(out.triangle_count(), scene.triangle_count());
    let tinted = out.batches.iter().find(|b| b.material == Some(m)).unwrap();
    assert_eq!(tinted.indices.len(), 3);
    let kept = out
        .batches
        .iter()
        .find(|b| b.material == Some(0))
        .expect("the other triangle keeps its material");
    assert_eq!(kept.indices.len(), 3);
    assert_eq!(out.lines[0].positions, scene.lines[0].positions);
    assert_eq!(out.bounds, scene.bounds);
    assert_eq!(scene.materials.len(), 1, "the input is untouched");
}

#[test]
fn corners_match_in_any_order_within_the_tolerance() {
    let scene = square_scene();
    let tol = MARK_TOLERANCE_M;
    let inside = SolidMarks {
        triangles: vec![tri(
            v(1.0 + tol * 0.5, 1.0, 0.0),
            v(0.0, 0.0, tol * 0.5),
            v(1.0, 0.0, 0.0),
        )],
        segments: vec![],
    };
    let out = tint_problems(&scene, &inside);
    let m = problem_material(&out);
    assert_eq!(
        out.batches
            .iter()
            .filter(|b| b.material == Some(m))
            .map(|b| b.indices.len())
            .sum::<usize>(),
        3
    );
    let outside = SolidMarks {
        triangles: vec![tri(
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0 + tol * 4.0, 1.0, 0.0),
        )],
        segments: vec![],
    };
    let out = tint_problems(&scene, &outside);
    assert_eq!(out.materials.len(), 1, "no match, no material");
    assert_eq!(out.batches.len(), 1);
}

#[test]
fn both_sides_of_a_face_are_tinted() {
    let mut scene = square_scene();
    let mut back = square_batch();
    back.indices = vec![0, 2, 1, 0, 3, 2];
    scene.batches.push(back);
    let marks = SolidMarks {
        triangles: vec![tri(v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0))],
        segments: vec![],
    };
    let out = tint_problems(&scene, &marks);
    let m = problem_material(&out);
    let tinted: usize = out
        .batches
        .iter()
        .filter(|b| b.material == Some(m))
        .map(|b| b.indices.len() / 3)
        .sum();
    assert_eq!(tinted, 2);
    assert_eq!(out.triangle_count(), 4);
}

#[test]
fn a_batch_with_nothing_left_is_dropped() {
    let scene = square_scene();
    let marks = SolidMarks {
        triangles: vec![
            tri(v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0)),
            tri(v(0.0, 0.0, 0.0), v(1.0, 1.0, 0.0), v(0.0, 1.0, 0.0)),
        ],
        segments: vec![],
    };
    let out = tint_problems(&scene, &marks);
    let m = problem_material(&out);
    assert_eq!(out.batches.len(), 1);
    assert_eq!(out.batches[0].material, Some(m));
    assert_eq!(out.triangle_count(), 2);
}

#[test]
fn a_matching_segment_moves_to_its_own_line_batch() {
    let scene = square_scene();
    let marks = SolidMarks {
        triangles: vec![],
        // The second scene segment, written the other way round.
        segments: vec![[v(1.0, 1.0, 0.0), v(1.0, 0.0, 0.0)]],
    };
    let out = tint_problems(&scene, &marks);
    let m = problem_material(&out);
    assert_eq!(out.line_count(), scene.line_count());
    let marked = out.lines.iter().find(|l| l.material == Some(m)).unwrap();
    assert_eq!(marked.positions.len(), 2);
    let plain: usize = out
        .lines
        .iter()
        .filter(|l| l.material != Some(m))
        .map(|l| l.positions.len())
        .sum();
    assert_eq!(plain, 4);
    assert_eq!(out.triangle_count(), scene.triangle_count());
    assert!(
        out.batches.iter().all(|b| b.material != Some(m)),
        "no triangle was marked"
    );
}

#[test]
fn nothing_to_mark_returns_the_scene_unchanged() {
    let scene = square_scene();
    let out = tint_problems(&scene, &SolidMarks::default());
    assert_eq!(out.materials.len(), 1);
    assert_eq!(out.batches.len(), 1);
    assert_eq!(out.lines.len(), 1);
    assert_eq!(out.triangle_count(), 2);
    let elsewhere = SolidMarks {
        triangles: vec![tri(v(5.0, 5.0, 5.0), v(6.0, 5.0, 5.0), v(6.0, 6.0, 5.0))],
        segments: vec![[v(5.0, 5.0, 5.0), v(6.0, 5.0, 5.0)]],
    };
    let out = tint_problems(&scene, &elsewhere);
    assert_eq!(out.materials.len(), 1);
    assert_eq!(out.lines.len(), 1);
}

#[test]
fn a_big_scene_is_tinted_quickly() {
    // 100 000 triangles in a strip; every other one is marked.
    let n = 50_000usize;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    let mut marks = SolidMarks::default();
    for i in 0..n {
        let x = i as f32;
        let base = positions.len() as u32;
        positions.extend([
            [x, 0.0, 0.0],
            [x + 1.0, 0.0, 0.0],
            [x + 1.0, 1.0, 0.0],
            [x, 1.0, 0.0],
        ]);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        let x = i as f64;
        marks.triangles.push(tri(
            v(x, 0.0, 0.0),
            v(x + 1.0, 0.0, 0.0),
            v(x + 1.0, 1.0, 0.0),
        ));
        marks.segments.push([v(x, 0.0, 0.0), v(x + 1.0, 0.0, 0.0)]);
    }
    let lines = LineBatch {
        material: None,
        positions: (0..n)
            .flat_map(|i| [[i as f32, 0.0, 0.0], [i as f32 + 1.0, 0.0, 0.0]])
            .collect(),
    };
    let scene = FlatScene {
        materials: vec![grey()],
        batches: vec![Batch {
            material: Some(0),
            normals: vec![[0.0, 0.0, 1.0]; positions.len()],
            positions,
            texcoords: vec![],
            indices,
        }],
        lines: vec![lines],
        bounds: None,
    };
    let start = Instant::now();
    let out = tint_problems(&scene, &marks);
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "took {:?}",
        start.elapsed()
    );
    let m = problem_material(&out);
    let tinted: usize = out
        .batches
        .iter()
        .filter(|b| b.material == Some(m))
        .map(|b| b.indices.len() / 3)
        .sum();
    assert_eq!(tinted, n);
    assert_eq!(out.triangle_count(), 2 * n);
    let marked: usize = out
        .lines
        .iter()
        .filter(|l| l.material == Some(m))
        .map(|l| l.positions.len() / 2)
        .sum();
    assert_eq!(marked, n);
}
