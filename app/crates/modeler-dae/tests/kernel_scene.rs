// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m1-kernel-scene: `kernel_scene` on models built in code, and the tables fixture.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::{Key, assert_areas_match, flat_areas};
use glam::{DMat4, DQuat, DVec3, Vec3};
use modeler_dae::{Batch, FlatScene, flatten, import_file, kernel_scene, to_kernel};
use modeler_kernel::model::{
    self as km, BodyId, DefinitionId, DefinitionInfo, FaceMaterials, Instance, MaterialId,
    ModelEdit,
};
use modeler_kernel::topology::{Context, EdgeUse, Edit, Face, FaceId, Loop, VertexId};

// ---------------------------------------------------------------------------------------
// Building models

fn p(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A closed loop through new vertices and edges (no sharing).
fn new_loop(ctx: &mut Context, log: &mut Edit, pts: &[DVec3]) -> Loop {
    let vs: Vec<VertexId> = pts.iter().map(|&q| ctx.insert_vertex(q, log)).collect();
    let uses = (0..vs.len())
        .map(|i| {
            let e = ctx
                .insert_edge(vs[i], vs[(i + 1) % vs.len()], log)
                .expect("edge inserts");
            EdgeUse::forward(e)
        })
        .collect();
    Loop { uses }
}

struct Builder {
    m: km::Model,
    log: ModelEdit,
}

impl Builder {
    fn new() -> Self {
        Builder {
            m: km::Model::new(),
            log: ModelEdit::new(),
        }
    }

    /// A face with its own vertices and edges. `outer` counter-clockwise seen from the
    /// front, holes clockwise.
    fn face(&mut self, body: BodyId, outer: &[DVec3], holes: &[&[DVec3]]) -> FaceId {
        self.m
            .edit_geometry(body, &mut self.log, |ctx, log| {
                let outer = new_loop(ctx, log, outer);
                let inner = holes.iter().map(|h| new_loop(ctx, log, h)).collect();
                ctx.insert_face(Face { outer, inner }, log)
                    .expect("face inserts")
            })
            .expect("body exists")
    }

    fn edge(&mut self, body: BodyId, a: DVec3, b: DVec3) {
        self.m
            .edit_geometry(body, &mut self.log, |ctx, log| {
                let va = ctx.insert_vertex(a, log);
                let vb = ctx.insert_vertex(b, log);
                ctx.insert_edge(va, vb, log).expect("edge inserts");
            })
            .expect("body exists");
    }

    fn material(&mut self, name: &str, color: [f32; 4]) -> MaterialId {
        self.m.add_material(
            km::Material {
                name: name.to_string(),
                color,
                texture: None,
            },
            &mut self.log,
        )
    }

    fn paint(
        &mut self,
        body: BodyId,
        face: FaceId,
        front: Option<MaterialId>,
        back: Option<MaterialId>,
    ) {
        self.m
            .set_face_materials(
                body,
                face,
                FaceMaterials {
                    front,
                    back,
                    ..Default::default()
                },
                &mut self.log,
            )
            .expect("face materials set");
    }

    fn definition(&mut self, name: &str) -> DefinitionId {
        self.m.add_definition(
            DefinitionInfo {
                name: name.to_string(),
                group: false,
            },
            &mut self.log,
        )
    }

    fn instance(
        &mut self,
        body: BodyId,
        definition: DefinitionId,
        transform: DMat4,
        material: Option<MaterialId>,
    ) {
        self.m
            .add_instance(
                body,
                Instance {
                    definition,
                    transform,
                    name: String::new(),
                    material,
                },
                &mut self.log,
            )
            .expect("instance adds");
    }

    fn scene(&self) -> FlatScene {
        kernel_scene(&self.m)
    }
}

/// Unit square in z = 0, facing +Z, moved by `o`.
fn square_at(o: DVec3) -> Vec<DVec3> {
    [
        p(0.0, 0.0, 0.0),
        p(1.0, 0.0, 0.0),
        p(1.0, 1.0, 0.0),
        p(0.0, 1.0, 0.0),
    ]
    .iter()
    .map(|&q| q + o)
    .collect()
}

fn square() -> Vec<DVec3> {
    square_at(DVec3::ZERO)
}

// ---------------------------------------------------------------------------------------
// Checking scenes

fn d(v: [f32; 3]) -> DVec3 {
    DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64)
}

fn tri_count(b: &Batch) -> usize {
    b.indices.len() / 3
}

/// Every triangle of `b` runs counter-clockwise seen from the side its normals point to,
/// and every normal has unit length.
fn assert_facing(b: &Batch) {
    assert_eq!(b.positions.len(), b.normals.len());
    assert!(b.texcoords.is_empty());
    for n in &b.normals {
        assert!((Vec3::from(*n).length() - 1.0).abs() < 1e-5, "normal {n:?}");
    }
    for t in b.indices.chunks(3) {
        let (a, bb, c) = (
            d(b.positions[t[0] as usize]),
            d(b.positions[t[1] as usize]),
            d(b.positions[t[2] as usize]),
        );
        let geometric = (bb - a).cross(c - a);
        if geometric.length() < 1e-9 {
            continue;
        }
        for &i in t {
            let n = d(b.normals[i as usize]);
            assert!(
                geometric.normalize().dot(n) > 0.999,
                "triangle {t:?} faces {geometric:?}, normal {n:?}"
            );
        }
    }
}

fn batch(scene: &FlatScene, material: Key) -> &Batch {
    scene
        .batches
        .iter()
        .find(|b| b.material == material)
        .unwrap_or_else(|| panic!("no batch for {material:?}"))
}

fn normals_of(b: &Batch) -> Vec<DVec3> {
    b.normals.iter().map(|&n| d(n)).collect()
}

fn segments(scene: &FlatScene) -> Vec<(DVec3, DVec3)> {
    assert!(scene.lines.len() <= 1, "edges go in one line batch");
    scene
        .lines
        .iter()
        .flat_map(|l| {
            assert_eq!(l.material, None);
            l.positions
                .chunks(2)
                .map(|s| (d(s[0]), d(s[1])))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn areas(scene: &FlatScene) -> BTreeMap<Key, f64> {
    flat_areas(scene)
}

fn close(a: DVec3, b: DVec3) -> bool {
    a.distance(b) < 1e-5
}

// ---------------------------------------------------------------------------------------
// Faces

#[test]
fn empty_model_draws_nothing() {
    let s = Builder::new().scene();
    assert!(s.materials.is_empty());
    assert!(s.batches.is_empty());
    assert!(s.lines.is_empty());
    assert_eq!(s.bounds, None);
}

#[test]
fn one_square_face() {
    let mut b = Builder::new();
    b.face(BodyId::Root, &square(), &[]);
    let s = b.scene();
    assert_eq!(s.batches.len(), 1);
    let bt = &s.batches[0];
    assert_eq!(bt.material, None);
    assert_eq!(tri_count(bt), 2);
    assert_eq!(bt.positions.len(), 4);
    assert_facing(bt);
    assert!(normals_of(bt).iter().all(|&n| close(n, DVec3::Z)));
    assert!((areas(&s)[&None] - 1.0).abs() < 1e-9);

    let segs = segments(&s);
    assert_eq!(segs.len(), 4);
    let sq = square();
    for i in 0..4 {
        let (a, c) = (sq[i], sq[(i + 1) % 4]);
        assert!(
            segs.iter().any(|&(x, y)| close(x, a) && close(y, c)),
            "edge {a:?} -> {c:?} missing"
        );
    }
    let bounds = s.bounds.expect("bounds");
    assert_eq!(bounds.min, [0.0, 0.0, 0.0]);
    assert_eq!(bounds.max, [1.0, 1.0, 0.0]);
}

#[test]
fn face_wound_the_other_way_faces_down() {
    let mut b = Builder::new();
    let mut sq = square();
    sq.reverse();
    b.face(BodyId::Root, &sq, &[]);
    let s = b.scene();
    let bt = batch(&s, None);
    assert_facing(bt);
    assert!(normals_of(bt).iter().all(|&n| close(n, -DVec3::Z)));
}

#[test]
fn vertical_and_slanted_faces() {
    let mut b = Builder::new();
    // In the plane y = 2, facing -Y.
    b.face(
        BodyId::Root,
        &[
            p(0.0, 2.0, 0.0),
            p(1.0, 2.0, 0.0),
            p(1.0, 2.0, 1.0),
            p(0.0, 2.0, 1.0),
        ],
        &[],
    );
    // Slanted triangle.
    b.face(
        BodyId::Root,
        &[p(3.0, 0.0, 0.0), p(4.0, 0.0, 1.0), p(3.0, 1.0, 2.0)],
        &[],
    );
    let s = b.scene();
    let bt = batch(&s, None);
    assert_eq!(tri_count(bt), 3);
    assert_facing(bt);
    let n = normals_of(bt);
    assert!(n.iter().filter(|&&v| close(v, -DVec3::Y)).count() == 4);
    let slanted = (p(1.0, 0.0, 1.0)).cross(p(0.0, 1.0, 2.0)).normalize();
    assert!(n.iter().filter(|&&v| close(v, slanted)).count() == 3);
}

#[test]
fn face_with_a_hole() {
    let mut b = Builder::new();
    let outer = [
        p(0.0, 0.0, 0.0),
        p(3.0, 0.0, 0.0),
        p(3.0, 3.0, 0.0),
        p(0.0, 3.0, 0.0),
    ];
    let hole = [
        p(1.0, 1.0, 0.0),
        p(1.0, 2.0, 0.0),
        p(2.0, 2.0, 0.0),
        p(2.0, 1.0, 0.0),
    ];
    b.face(BodyId::Root, &outer, &[&hole]);
    let s = b.scene();
    let bt = batch(&s, None);
    assert_eq!(tri_count(bt), 8);
    assert_eq!(bt.positions.len(), 8);
    assert_facing(bt);
    assert!((areas(&s)[&None] - 8.0).abs() < 1e-9);
    // No triangle covers the hole's centre.
    let c = p(1.5, 1.5, 0.0);
    for t in bt.indices.chunks(3) {
        let (a, bb, cc) = (
            d(bt.positions[t[0] as usize]),
            d(bt.positions[t[1] as usize]),
            d(bt.positions[t[2] as usize]),
        );
        let inside = (bb - a).cross(c - a).z > 0.0
            && (cc - bb).cross(c - bb).z > 0.0
            && (a - cc).cross(c - cc).z > 0.0;
        assert!(!inside, "triangle {t:?} covers the hole");
    }
    assert_eq!(segments(&s).len(), 8);
}

#[test]
fn face_with_a_split_edge() {
    let mut b = Builder::new();
    b.face(
        BodyId::Root,
        &[
            p(0.0, 0.0, 0.0),
            p(0.5, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(1.0, 1.0, 0.0),
            p(0.0, 1.0, 0.0),
        ],
        &[],
    );
    let s = b.scene();
    assert_eq!(tri_count(batch(&s, None)), 3);
    assert!((areas(&s)[&None] - 1.0).abs() < 1e-9);
}

#[test]
fn concave_face() {
    let mut b = Builder::new();
    let l = [
        p(0.0, 0.0, 1.0),
        p(2.0, 0.0, 1.0),
        p(2.0, 1.0, 1.0),
        p(1.0, 1.0, 1.0),
        p(1.0, 2.0, 1.0),
        p(0.0, 2.0, 1.0),
    ];
    b.face(BodyId::Root, &l, &[]);
    let s = b.scene();
    assert_eq!(tri_count(batch(&s, None)), 4);
    assert!((areas(&s)[&None] - 3.0).abs() < 1e-9);
    assert_facing(batch(&s, None));
}

#[test]
fn face_without_a_normal_is_skipped_but_its_edges_draw() {
    let mut b = Builder::new();
    b.face(
        BodyId::Root,
        &[p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(2.0, 0.0, 0.0)],
        &[],
    );
    let s = b.scene();
    assert!(s.batches.is_empty());
    assert_eq!(segments(&s).len(), 3);
    let bounds = s.bounds.expect("lines have bounds");
    assert_eq!(bounds.max, [2.0, 0.0, 0.0]);
}

// ---------------------------------------------------------------------------------------
// Materials

#[test]
fn materials_convert_in_id_order() {
    let mut b = Builder::new();
    let red = b.material("red", [1.0, 0.0, 0.0, 0.5]);
    let wood = b.m.add_material(
        km::Material {
            name: "wood".into(),
            color: [0.6, 0.4, 0.2, 1.0],
            texture: Some("wood.jpg".into()),
        },
        &mut b.log,
    );
    let s = b.scene();
    assert_eq!(s.materials.len(), 2);
    let (m0, m1) = (&s.materials[0], &s.materials[1]);
    assert_eq!(m0.id, red.0.to_string());
    assert_eq!(m0.name, "red");
    assert_eq!(m0.color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(m0.opacity, 0.5);
    assert_eq!(m0.texture, None);
    assert!(!m0.unlit);
    assert_eq!(m1.id, wood.0.to_string());
    assert_eq!(m1.name, "wood");
    assert_eq!(m1.color, [0.6, 0.4, 0.2, 1.0]);
    assert_eq!(m1.opacity, 1.0);
    assert_eq!(m1.texture, None, "no texture coordinates yet");
}

#[test]
fn materials_follow_kernel_id_order_after_removal() {
    let mut b = Builder::new();
    let a = b.material("a", [1.0; 4]);
    let _c = b.material("c", [1.0; 4]);
    b.m.remove_material(a, &mut b.log)
        .expect("unused material removes");
    let d2 = b.material("d", [1.0; 4]);
    let s = b.scene();
    let names: Vec<&str> = s.materials.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["c", "d"]);
    let f = b.face(BodyId::Root, &square(), &[]);
    b.paint(BodyId::Root, f, Some(d2), None);
    let s = b.scene();
    assert_eq!(s.batches.len(), 1);
    assert_eq!(s.batches[0].material, Some(1));
}

#[test]
fn front_material_only_draws_one_side() {
    let mut b = Builder::new();
    let _a = b.material("a", [1.0; 4]);
    let bm = b.material("b", [1.0; 4]);
    let f = b.face(BodyId::Root, &square(), &[]);
    b.paint(BodyId::Root, f, Some(bm), None);
    let s = b.scene();
    assert_eq!(s.batches.len(), 1);
    assert_eq!(s.batches[0].material, Some(1));
    assert_eq!(tri_count(&s.batches[0]), 2);
}

#[test]
fn back_material_adds_a_reversed_copy() {
    let mut b = Builder::new();
    let a = b.material("a", [1.0; 4]);
    let bm = b.material("b", [1.0; 4]);
    let f = b.face(BodyId::Root, &square(), &[]);
    b.paint(BodyId::Root, f, Some(a), Some(bm));
    let s = b.scene();
    assert_eq!(s.batches.len(), 2);
    let front = batch(&s, Some(0));
    let back = batch(&s, Some(1));
    assert_eq!((tri_count(front), tri_count(back)), (2, 2));
    assert_eq!(back.positions.len(), 4);
    assert_facing(front);
    assert_facing(back);
    assert!(normals_of(front).iter().all(|&n| close(n, DVec3::Z)));
    assert!(normals_of(back).iter().all(|&n| close(n, -DVec3::Z)));
}

#[test]
fn back_material_without_front() {
    let mut b = Builder::new();
    let a = b.material("a", [1.0; 4]);
    let f = b.face(BodyId::Root, &square(), &[]);
    b.paint(BodyId::Root, f, None, Some(a));
    let s = b.scene();
    let order: Vec<Key> = s.batches.iter().map(|b| b.material).collect();
    assert_eq!(order, [Some(0), None]);
    assert!(
        normals_of(&s.batches[0])
            .iter()
            .all(|&n| close(n, -DVec3::Z))
    );
    assert!(
        normals_of(&s.batches[1])
            .iter()
            .all(|&n| close(n, DVec3::Z))
    );
}

#[test]
fn same_material_both_sides_shares_one_batch() {
    let mut b = Builder::new();
    let a = b.material("a", [1.0; 4]);
    let f = b.face(BodyId::Root, &square(), &[]);
    b.paint(BodyId::Root, f, Some(a), Some(a));
    let s = b.scene();
    assert_eq!(s.batches.len(), 1);
    assert_eq!(tri_count(&s.batches[0]), 4);
    assert_facing(&s.batches[0]);
}

#[test]
fn batches_in_material_index_order_then_default() {
    let mut b = Builder::new();
    let m: Vec<MaterialId> = (0..3)
        .map(|i| b.material(&format!("m{i}"), [1.0; 4]))
        .collect();
    let paints = [Some(m[2]), Some(m[0]), None, Some(m[1]), Some(m[2])];
    for (i, front) in paints.into_iter().enumerate() {
        let f = b.face(BodyId::Root, &square_at(p(i as f64 * 2.0, 0.0, 0.0)), &[]);
        if front.is_some() {
            b.paint(BodyId::Root, f, front, None);
        }
    }
    let s = b.scene();
    let order: Vec<Key> = s.batches.iter().map(|b| b.material).collect();
    assert_eq!(order, [Some(0), Some(1), Some(2), None]);
    assert_eq!(tri_count(batch(&s, Some(2))), 4);
    let a = areas(&s);
    assert!((a[&Some(2)] - 2.0).abs() < 1e-9);
    assert!((a[&None] - 1.0).abs() < 1e-9);
}

#[test]
fn unused_material_gets_no_batch() {
    let mut b = Builder::new();
    b.material("a", [1.0; 4]);
    b.face(BodyId::Root, &square(), &[]);
    let s = b.scene();
    assert_eq!(s.materials.len(), 1);
    assert_eq!(s.batches.len(), 1);
    assert_eq!(s.batches[0].material, None);
}

// ---------------------------------------------------------------------------------------
// Instances

#[test]
fn instances_are_placed_by_their_transforms() {
    let mut b = Builder::new();
    let def = b.definition("tile");
    b.face(BodyId::Definition(def), &square(), &[]);
    b.instance(
        BodyId::Root,
        def,
        DMat4::from_translation(p(5.0, 0.0, 0.0)),
        None,
    );
    b.instance(
        BodyId::Root,
        def,
        DMat4::from_translation(p(0.0, 5.0, 1.0)),
        None,
    );
    let s = b.scene();
    let bt = batch(&s, None);
    assert_eq!(tri_count(bt), 4);
    let pos: Vec<DVec3> = bt.positions.iter().map(|&q| d(q)).collect();
    assert!(pos.iter().any(|&q| close(q, p(6.0, 1.0, 0.0))));
    assert!(pos.iter().any(|&q| close(q, p(1.0, 6.0, 1.0))));
    assert!(!pos.iter().any(|&q| close(q, p(0.0, 0.0, 0.0))));
    assert_eq!(segments(&s).len(), 8);
    let bounds = s.bounds.expect("bounds");
    assert_eq!(bounds.min, [0.0, 0.0, 0.0]);
    assert_eq!(bounds.max, [6.0, 6.0, 1.0]);
}

#[test]
fn definition_without_instances_is_not_drawn() {
    let mut b = Builder::new();
    let def = b.definition("unused");
    b.face(BodyId::Definition(def), &square(), &[]);
    let s = b.scene();
    assert!(s.batches.is_empty());
    assert!(s.lines.is_empty());
    assert_eq!(s.bounds, None);
}

#[test]
fn rotated_and_stretched_instance_normals() {
    let mut b = Builder::new();
    let def = b.definition("slab");
    // Slanted face, normal (1, 0, 1) / sqrt 2.
    b.face(
        BodyId::Definition(def),
        &[
            p(0.0, 0.0, 1.0),
            p(1.0, 0.0, 0.0),
            p(1.0, 1.0, 0.0),
            p(0.0, 1.0, 1.0),
        ],
        &[],
    );
    let t = DMat4::from_translation(p(1.0, 2.0, 3.0))
        * DMat4::from_quat(DQuat::from_rotation_z(0.7))
        * DMat4::from_scale(p(2.0, 1.0, 0.5));
    b.instance(BodyId::Root, def, t, None);
    let s = b.scene();
    let bt = batch(&s, None);
    assert_facing(bt);
    let n = p(1.0, 0.0, 1.0).normalize();
    let want = DMat4::from_mat3(glam::DMat3::from_mat4(t).inverse().transpose())
        .transform_vector3(n)
        .normalize();
    for got in normals_of(bt) {
        assert!(close(got, want), "normal {got:?}, want {want:?}");
    }
    let pos: Vec<DVec3> = bt.positions.iter().map(|&q| d(q)).collect();
    assert!(
        pos.iter()
            .any(|&q| close(q, t.transform_point3(p(1.0, 1.0, 0.0))))
    );
}

#[test]
fn mirrored_instance_keeps_triangles_facing_their_normal() {
    let mut b = Builder::new();
    let def = b.definition("tile");
    b.face(BodyId::Definition(def), &square(), &[]);
    b.instance(
        BodyId::Root,
        def,
        DMat4::from_scale(p(-1.0, 1.0, 1.0)),
        None,
    );
    let s = b.scene();
    let bt = batch(&s, None);
    assert_facing(bt);
    // Mirroring in x keeps the normal of a horizontal face.
    assert!(normals_of(bt).iter().all(|&n| close(n, DVec3::Z)));
    let bounds = s.bounds.expect("bounds");
    assert_eq!(bounds.min, [-1.0, 0.0, 0.0]);
}

#[test]
fn mirrored_instance_with_back_material() {
    let mut b = Builder::new();
    let a = b.material("a", [1.0; 4]);
    let def = b.definition("tile");
    let f = b.face(BodyId::Definition(def), &square(), &[]);
    b.paint(BodyId::Definition(def), f, None, Some(a));
    b.instance(
        BodyId::Root,
        def,
        DMat4::from_scale(p(1.0, 1.0, -1.0)),
        None,
    );
    let s = b.scene();
    // Mirroring in z flips the face: front now looks down, back up.
    assert_facing(batch(&s, None));
    assert_facing(batch(&s, Some(0)));
    assert!(
        normals_of(batch(&s, None))
            .iter()
            .all(|&n| close(n, -DVec3::Z))
    );
    assert!(
        normals_of(batch(&s, Some(0)))
            .iter()
            .all(|&n| close(n, DVec3::Z))
    );
}

#[test]
fn nested_instances_compose_transforms() {
    let mut b = Builder::new();
    let inner = b.definition("inner");
    let outer = b.definition("outer");
    b.face(BodyId::Definition(inner), &square(), &[]);
    b.instance(
        BodyId::Definition(outer),
        inner,
        DMat4::from_translation(p(0.0, 0.0, 2.0)),
        None,
    );
    b.instance(
        BodyId::Root,
        outer,
        DMat4::from_scale(DVec3::splat(3.0)),
        None,
    );
    let s = b.scene();
    let bounds = s.bounds.expect("bounds");
    assert_eq!(bounds.min, [0.0, 0.0, 6.0]);
    assert_eq!(bounds.max, [3.0, 3.0, 6.0]);
    assert!((areas(&s)[&None] - 9.0).abs() < 1e-6);
}

#[test]
fn instance_materials_paint_unpainted_sides_innermost_first() {
    let mut b = Builder::new();
    let ma = b.material("a", [1.0; 4]);
    let mb = b.material("b", [1.0; 4]);
    let mc = b.material("c", [1.0; 4]);
    let inner = b.definition("inner");
    let outer = b.definition("outer");
    let id = BodyId::Definition(inner);
    let od = BodyId::Definition(outer);
    b.face(id, &square(), &[]);
    let painted = b.face(id, &square_at(p(2.0, 0.0, 0.0)), &[]);
    b.paint(id, painted, Some(mc), None);
    b.face(od, &square_at(p(0.0, 3.0, 0.0)), &[]);
    b.instance(od, inner, DMat4::IDENTITY, Some(ma));
    b.instance(BodyId::Root, outer, DMat4::IDENTITY, Some(mb));
    b.instance(
        BodyId::Root,
        outer,
        DMat4::from_translation(p(10.0, 0.0, 0.0)),
        None,
    );
    b.instance(
        BodyId::Root,
        inner,
        DMat4::from_translation(p(20.0, 0.0, 0.0)),
        None,
    );
    let s = b.scene();
    let a = areas(&s);
    // Outer(b) > inner(a): plain face a+a, painted c+a; outer's face b+b.
    // Outer(none) > inner(a): plain a+a, painted c+a; outer's face default front only.
    // Inner(none): plain default front only, painted c front only.
    let want: BTreeMap<Key, f64> = [(Some(0), 6.0), (Some(1), 2.0), (Some(2), 3.0), (None, 2.0)]
        .into_iter()
        .collect();
    assert_areas_match("instance materials", &a, &want, 1e-9, 1e-9);
    for bt in &s.batches {
        assert_facing(bt);
    }
}

#[test]
fn face_material_beats_instance_material() {
    let mut b = Builder::new();
    let ma = b.material("a", [1.0; 4]);
    let mb = b.material("b", [1.0; 4]);
    let def = b.definition("tile");
    let f = b.face(BodyId::Definition(def), &square(), &[]);
    b.paint(BodyId::Definition(def), f, Some(ma), Some(ma));
    b.instance(BodyId::Root, def, DMat4::IDENTITY, Some(mb));
    let s = b.scene();
    let order: Vec<Key> = s.batches.iter().map(|b| b.material).collect();
    assert_eq!(order, [Some(0)]);
}

#[test]
fn deep_nesting_does_not_overflow() {
    let mut b = Builder::new();
    let defs: Vec<DefinitionId> = (0..200).map(|i| b.definition(&format!("d{i}"))).collect();
    b.face(BodyId::Definition(defs[199]), &square(), &[]);
    for w in defs.windows(2) {
        b.instance(BodyId::Definition(w[0]), w[1], DMat4::IDENTITY, None);
    }
    b.instance(BodyId::Root, defs[0], DMat4::IDENTITY, None);
    let _ = b.scene();

    // Ten levels draw normally.
    let mut b = Builder::new();
    let defs: Vec<DefinitionId> = (0..10).map(|i| b.definition(&format!("d{i}"))).collect();
    b.face(BodyId::Definition(defs[9]), &square(), &[]);
    for w in defs.windows(2) {
        b.instance(
            BodyId::Definition(w[0]),
            w[1],
            DMat4::from_translation(p(1.0, 0.0, 0.0)),
            None,
        );
    }
    b.instance(BodyId::Root, defs[0], DMat4::IDENTITY, None);
    let s = b.scene();
    assert_eq!(tri_count(batch(&s, None)), 2);
    assert_eq!(s.bounds.expect("bounds").min, [9.0, 0.0, 0.0]);
}

// ---------------------------------------------------------------------------------------
// Edges, bounds, bad numbers

#[test]
fn stray_edges_draw_and_count_in_bounds() {
    let mut b = Builder::new();
    b.face(BodyId::Root, &square(), &[]);
    b.edge(BodyId::Root, p(0.0, 0.0, 0.0), p(0.0, 0.0, 4.0));
    let s = b.scene();
    let segs = segments(&s);
    assert_eq!(segs.len(), 5);
    assert!(
        segs.iter()
            .any(|&(a, c)| close(a, DVec3::ZERO) && close(c, p(0.0, 0.0, 4.0)))
    );
    assert_eq!(s.bounds.expect("bounds").max, [1.0, 1.0, 4.0]);
}

#[test]
fn edges_only_model() {
    let mut b = Builder::new();
    b.edge(BodyId::Root, p(-1.0, 0.0, 0.0), p(1.0, 2.0, 3.0));
    let s = b.scene();
    assert!(s.batches.is_empty());
    assert_eq!(segments(&s).len(), 1);
    let bounds = s.bounds.expect("bounds");
    assert_eq!(bounds.min, [-1.0, 0.0, 0.0]);
    assert_eq!(bounds.max, [1.0, 2.0, 3.0]);
}

#[test]
fn shared_edges_draw_once() {
    let mut b = Builder::new();
    b.m.edit_geometry(BodyId::Root, &mut b.log, |ctx, log| {
        let v: Vec<VertexId> = [
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(2.0, 0.0, 0.0),
            p(2.0, 1.0, 0.0),
            p(1.0, 1.0, 0.0),
            p(0.0, 1.0, 0.0),
        ]
        .iter()
        .map(|&q| ctx.insert_vertex(q, log))
        .collect();
        let mut e = |a: usize, c: usize| ctx.insert_edge(v[a], v[c], log).expect("edge");
        let (e01, e12, e23, e34, e45, e50, e14) = (
            e(0, 1),
            e(1, 2),
            e(2, 3),
            e(3, 4),
            e(4, 5),
            e(5, 0),
            e(1, 4),
        );
        let f = EdgeUse::forward;
        let r = EdgeUse::backward;
        ctx.insert_face(
            Face {
                outer: Loop {
                    uses: vec![f(e01), f(e14), f(e45), f(e50)],
                },
                inner: vec![],
            },
            log,
        )
        .expect("face");
        ctx.insert_face(
            Face {
                outer: Loop {
                    uses: vec![f(e12), f(e23), f(e34), r(e14)],
                },
                inner: vec![],
            },
            log,
        )
        .expect("face");
    })
    .expect("root");
    let s = b.scene();
    assert_eq!(segments(&s).len(), 7);
    assert_eq!(tri_count(batch(&s, None)), 4);
    assert_facing(batch(&s, None));
}

#[test]
fn non_finite_geometry_is_skipped() {
    let mut b = Builder::new();
    b.face(BodyId::Root, &square(), &[]);
    b.face(
        BodyId::Root,
        &[p(0.0, 0.0, 5.0), p(f64::NAN, 0.0, 5.0), p(1.0, 1.0, 5.0)],
        &[],
    );
    b.edge(BodyId::Root, p(0.0, 0.0, 0.0), p(f64::INFINITY, 0.0, 0.0));
    let def = b.definition("far");
    b.face(BodyId::Definition(def), &square(), &[]);
    b.instance(
        BodyId::Root,
        def,
        DMat4::from_scale(DVec3::splat(1e308)) * DMat4::from_scale(DVec3::splat(1e308)),
        None,
    );
    let s = b.scene();
    let bt = batch(&s, None);
    assert_eq!(tri_count(bt), 2);
    assert!(bt.positions.iter().flatten().all(|c| c.is_finite()));
    for l in &s.lines {
        assert!(l.positions.iter().flatten().all(|c| c.is_finite()));
    }
    let bounds = s.bounds.expect("bounds");
    assert!(
        bounds
            .min
            .iter()
            .chain(bounds.max.iter())
            .all(|c| c.is_finite())
    );
    // The bad face's finite edge (0, 0, 5) -> (1, 1, 5) still draws.
    assert_eq!(bounds.max, [1.0, 1.0, 5.0]);
    assert_eq!(segments(&s).len(), 4 + 1);
}

// ---------------------------------------------------------------------------------------
// Imported documents

#[test]
fn tables_fixture_draws_like_flatten() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tables.dae");
    let doc = import_file(&path).expect("fixture imports");
    let m = to_kernel(&doc).expect("fixture converts");
    let s = kernel_scene(&m);
    let f = flatten(&doc);
    assert_eq!(s.materials.len(), f.materials.len());
    for (a, b) in s.materials.iter().zip(&f.materials) {
        assert_eq!(a.color[..3], b.color[..3]);
        assert_eq!(a.opacity, b.opacity);
    }
    assert_areas_match("tables", &areas(&s), &areas(&f), 1e-5, 1e-9);
    for bt in &s.batches {
        assert_facing(bt);
    }
    let (sb, fb) = (s.bounds.expect("bounds"), f.bounds.expect("bounds"));
    for k in 0..3 {
        assert!((sb.min[k] - fb.min[k]).abs() < 1e-4, "{sb:?} vs {fb:?}");
        assert!((sb.max[k] - fb.max[k]).abs() < 1e-4, "{sb:?} vs {fb:?}");
    }
    assert!(s.triangle_count() <= f.triangle_count());
}
