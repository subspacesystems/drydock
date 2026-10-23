// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-edit-view-scene: `kernel_scene_editing` draws the open part of the model as usual and
//! the rest faded or not at all.

use glam::{DMat4, DVec3};
use modeler_dae::{FlatScene, RestView, kernel_scene, kernel_scene_editing};
use modeler_kernel::editpath::EditPath;
use modeler_kernel::model::{
    self as km, BodyId, DefinitionId, DefinitionInfo, FaceMaterials, Instance, InstanceId,
    MaterialId, ModelEdit,
};
use modeler_kernel::topology::{Context, EdgeUse, Edit, Face, FaceId, Loop, VertexId};

fn p(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

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

    /// A unit square facing +Z with its own edges, its lower corner at `at`.
    fn square(&mut self, body: BodyId, at: DVec3, front: Option<MaterialId>) -> FaceId {
        let pts = [
            at,
            at + p(1.0, 0.0, 0.0),
            at + p(1.0, 1.0, 0.0),
            at + p(0.0, 1.0, 0.0),
        ];
        let face = self
            .m
            .edit_geometry(body, &mut self.log, |ctx, log| {
                let outer = new_loop(ctx, log, &pts);
                ctx.insert_face(
                    Face {
                        outer,
                        inner: vec![],
                    },
                    log,
                )
                .expect("face inserts")
            })
            .expect("body exists");
        if front.is_some() {
            self.m
                .set_face_materials(
                    body,
                    face,
                    FaceMaterials {
                        front,
                        ..Default::default()
                    },
                    &mut self.log,
                )
                .expect("materials set");
        }
        face
    }

    fn material(&mut self, name: &str, color: [f32; 4]) -> MaterialId {
        self.m
            .add_material(km::Material::new(name, color), &mut self.log)
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

    fn instance(&mut self, body: BodyId, definition: DefinitionId, at: DVec3) -> InstanceId {
        self.m
            .add_instance(
                body,
                Instance {
                    definition,
                    transform: DMat4::from_translation(at),
                    name: String::new(),
                    material: None,
                },
                &mut self.log,
            )
            .expect("instance adds")
    }
}

/// The model every test edits.
///
/// - the root holds a red square at x 0..1, a square with no material at x 5..6, and
///   instances `a` (at x = 10) and `b` (at x = 20) of definition `d`;
/// - `d` holds a blue square at x 0..1, y 0..1 and an instance `e` (at y = 5) of `inner`;
/// - `inner` holds a square with no material at x 0..1, y 0..1.
struct Scene {
    m: km::Model,
    a: InstanceId,
    e: InstanceId,
}

fn scene() -> Scene {
    let mut b = Builder::new();
    let red = b.material("red", [1.0, 0.0, 0.0, 1.0]);
    let blue = b.material("blue", [0.0, 0.0, 1.0, 0.8]);
    let inner = b.definition("inner");
    b.square(BodyId::Definition(inner), p(0.0, 0.0, 0.0), None);
    let d = b.definition("d");
    b.square(BodyId::Definition(d), p(0.0, 0.0, 0.0), Some(blue));
    let e = b.instance(BodyId::Definition(d), inner, p(0.0, 5.0, 0.0));
    b.square(BodyId::Root, p(0.0, 0.0, 0.0), Some(red));
    b.square(BodyId::Root, p(5.0, 0.0, 0.0), None);
    let a = b.instance(BodyId::Root, d, p(10.0, 0.0, 0.0));
    b.instance(BodyId::Root, d, p(20.0, 0.0, 0.0));
    Scene { m: b.m, a, e }
}

fn path_a(s: &Scene) -> EditPath {
    EditPath { steps: vec![s.a] }
}

fn path_ae(s: &Scene) -> EditPath {
    EditPath {
        steps: vec![s.a, s.e],
    }
}

/// Every triangle as its three world-space corners, with the batch's material index.
fn triangles(scene: &FlatScene) -> Vec<(Option<usize>, [DVec3; 3])> {
    let mut out = Vec::new();
    for b in &scene.batches {
        for t in b.indices.chunks(3) {
            let c = |i: u32| {
                let q = b.positions[i as usize];
                p(q[0] as f64, q[1] as f64, q[2] as f64)
            };
            out.push((b.material, [c(t[0]), c(t[1]), c(t[2])]));
        }
    }
    out
}

fn centre(t: &[DVec3; 3]) -> DVec3 {
    (t[0] + t[1] + t[2]) / 3.0
}

/// The triangles whose centre lies in x `lo..hi`, y `ylo..yhi`.
fn in_box(scene: &FlatScene, x: (f64, f64), y: (f64, f64)) -> Vec<(Option<usize>, [DVec3; 3])> {
    triangles(scene)
        .into_iter()
        .filter(|(_, t)| {
            let c = centre(t);
            c.x > x.0 && c.x < x.1 && c.y > y.0 && c.y < y.1
        })
        .collect()
}

fn segment_count(scene: &FlatScene) -> usize {
    scene.line_count()
}

fn same_scene(a: &FlatScene, b: &FlatScene) {
    assert_eq!(a.materials, b.materials);
    assert_eq!(a.bounds, b.bounds);
    assert_eq!(a.batches.len(), b.batches.len());
    for (x, y) in a.batches.iter().zip(&b.batches) {
        assert_eq!(x.material, y.material);
        assert_eq!(x.positions, y.positions);
        assert_eq!(x.indices, y.indices);
    }
    assert_eq!(a.lines.len(), b.lines.len());
    for (x, y) in a.lines.iter().zip(&b.lines) {
        assert_eq!(x.positions, y.positions);
    }
}

const ALL: [RestView; 3] = [RestView::Normal, RestView::Faded, RestView::Hidden];

#[test]
fn the_root_path_draws_everything_in_every_mode() {
    let s = scene();
    let want = kernel_scene(&s.m);
    for rest in ALL {
        same_scene(&kernel_scene_editing(&s.m, &EditPath::root(), rest), &want);
    }
}

#[test]
fn a_path_that_does_not_resolve_is_the_root() {
    let s = scene();
    let want = kernel_scene(&s.m);
    let bad = EditPath {
        steps: vec![InstanceId(9999)],
    };
    for rest in ALL {
        same_scene(&kernel_scene_editing(&s.m, &bad, rest), &want);
    }
    // `e` lives inside `d`, not in the root.
    let bad = EditPath { steps: vec![s.e] };
    same_scene(&kernel_scene_editing(&s.m, &bad, RestView::Hidden), &want);
}

#[test]
fn normal_draws_the_whole_model_unchanged() {
    let s = scene();
    same_scene(
        &kernel_scene_editing(&s.m, &path_a(&s), RestView::Normal),
        &kernel_scene(&s.m),
    );
}

#[test]
fn hidden_draws_only_the_open_instance_and_what_is_nested_in_it() {
    let s = scene();
    let full = kernel_scene(&s.m);
    let got = kernel_scene_editing(&s.m, &path_a(&s), RestView::Hidden);
    // Instance a: its blue square and the inner square, 2 triangles each.
    assert_eq!(got.triangle_count(), 4);
    assert_eq!(in_box(&got, (10.0, 11.0), (0.0, 1.0)).len(), 2);
    assert_eq!(in_box(&got, (10.0, 11.0), (5.0, 6.0)).len(), 2);
    assert_eq!(segment_count(&got), 8);
    // Materials are the model's own, unchanged.
    assert_eq!(got.materials, full.materials);
    let blue = got.batches.iter().find(|b| b.material.is_some()).unwrap();
    assert_eq!(got.materials[blue.material.unwrap()].name, "blue");
    // Bounds cover what is drawn.
    let b = got.bounds.expect("bounds");
    assert_eq!(b.min, [10.0, 0.0, 0.0]);
    assert_eq!(b.max, [11.0, 6.0, 0.0]);
}

#[test]
fn hidden_with_a_deeper_path_keeps_only_that_instance() {
    let s = scene();
    let got = kernel_scene_editing(&s.m, &path_ae(&s), RestView::Hidden);
    assert_eq!(got.triangle_count(), 2);
    assert_eq!(in_box(&got, (10.0, 11.0), (5.0, 6.0)).len(), 2);
    assert_eq!(segment_count(&got), 4);
}

#[test]
fn faded_draws_every_face_and_only_the_open_edges() {
    let s = scene();
    let full = kernel_scene(&s.m);
    let got = kernel_scene_editing(&s.m, &path_a(&s), RestView::Faded);
    assert_eq!(got.triangle_count(), full.triangle_count());
    // Edges of the rest are not drawn.
    assert_eq!(segment_count(&got), 8);
    assert_eq!(got.bounds, full.bounds);
}

#[test]
fn faded_open_faces_keep_their_materials() {
    let s = scene();
    let full = kernel_scene(&s.m);
    let got = kernel_scene_editing(&s.m, &path_a(&s), RestView::Faded);
    // The model's materials come first and are unchanged.
    assert!(got.materials.len() > full.materials.len());
    assert_eq!(got.materials[..full.materials.len()], full.materials[..]);
    let open = in_box(&got, (10.0, 11.0), (0.0, 1.0));
    assert_eq!(open.len(), 2);
    for (material, _) in &open {
        let m = material.expect("the blue square has a material");
        assert_eq!(got.materials[m].name, "blue");
    }
    // The nested inner square has no material of its own and stays on the default batch.
    for (material, _) in in_box(&got, (10.0, 11.0), (5.0, 6.0)) {
        assert_eq!(material, None);
    }
}

#[test]
fn faded_rest_uses_faded_copies_of_the_materials() {
    let s = scene();
    let full = kernel_scene(&s.m);
    let got = kernel_scene_editing(&s.m, &path_a(&s), RestView::Faded);
    let originals = full.materials.len();

    // The red root square.
    let red = in_box(&got, (0.0, 1.0), (0.0, 1.0));
    assert_eq!(red.len(), 2);
    for (material, _) in &red {
        let m = &got.materials[material.expect("faded material")];
        assert!(m.name.starts_with("red"), "{}", m.name);
        assert!(m.name.ends_with(" (faded)"), "{}", m.name);
        assert_eq!(m.color, [1.0, 0.0, 0.0, 1.0]);
        assert!((m.opacity - 0.25).abs() < 1e-6, "{}", m.opacity);
        assert!(material.unwrap() >= originals);
    }

    // Instance b of the same definition is rest as well: its blue square is faded and
    // keeps the blue's own opacity (0.8) scaled by 0.25.
    let blue_b = in_box(&got, (20.0, 21.0), (0.0, 1.0));
    assert_eq!(blue_b.len(), 2);
    for (material, _) in &blue_b {
        let m = &got.materials[material.expect("faded material")];
        assert_eq!(m.name, "blue (faded)");
        assert_eq!(m.color, [0.0, 0.0, 1.0, 1.0]);
        assert!((m.opacity - 0.2).abs() < 1e-6, "{}", m.opacity);
    }

    // One faded copy per material, shared by all the faces that use it.
    let a = red[0].0;
    assert!(red.iter().all(|(m, _)| *m == a));
}

#[test]
fn faded_rest_with_no_material_uses_a_faded_default() {
    let s = scene();
    let got = kernel_scene_editing(&s.m, &path_a(&s), RestView::Faded);
    // The root square at x 5..6 has no material.
    let tris = in_box(&got, (5.0, 6.0), (0.0, 1.0));
    assert_eq!(tris.len(), 2);
    let m = &got.materials[tris[0].0.expect("a faded default material")];
    assert_eq!(m.name, "default (faded)");
    assert_eq!(m.color, [0.75, 0.75, 0.75, 1.0]);
    assert!((m.opacity - 0.25).abs() < 1e-6);
    // The nested inner square of b, also without a material.
    let inner_b = in_box(&got, (20.0, 21.0), (5.0, 6.0));
    assert_eq!(inner_b.len(), 2);
    assert_eq!(inner_b[0].0, tris[0].0);
}

#[test]
fn faded_with_a_deeper_path_fades_the_rest_of_the_open_instance_too() {
    let s = scene();
    let got = kernel_scene_editing(&s.m, &path_ae(&s), RestView::Faded);
    assert_eq!(got.triangle_count(), kernel_scene(&s.m).triangle_count());
    assert_eq!(segment_count(&got), 4);
    // a's blue square is rest now.
    for (material, _) in in_box(&got, (10.0, 11.0), (0.0, 1.0)) {
        assert_eq!(got.materials[material.unwrap()].name, "blue (faded)");
    }
    // The open inner square is drawn as usual (default batch).
    for (material, _) in in_box(&got, (10.0, 11.0), (5.0, 6.0)) {
        assert_eq!(material, None);
    }
}

#[test]
fn faded_draws_nothing_extra_without_rest_faces() {
    // A model with only the instance: opening it leaves no rest, so no faded materials.
    let mut b = Builder::new();
    let d = b.definition("d");
    b.square(BodyId::Definition(d), p(0.0, 0.0, 0.0), None);
    let a = b.instance(BodyId::Root, d, p(3.0, 0.0, 0.0));
    let path = EditPath { steps: vec![a] };
    let got = kernel_scene_editing(&b.m, &path, RestView::Faded);
    same_scene(&got, &kernel_scene(&b.m));
}
