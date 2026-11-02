// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-texture-scene: DAE import keeps each face's texture placement, and `kernel_scene`
//! draws textured materials with texture coordinates.

mod common;

use std::path::PathBuf;

use common::*;
use drydock_dae::{
    Batch, FlatScene, Model, Primitive, PrimitiveKind, UpAxis, kernel_scene, to_kernel,
};
use drydock_kernel::geom::polygon_normal;
use drydock_kernel::model::{
    self as km, BodyId, DefinitionInfo, FaceMaterials, Instance, MaterialId, ModelEdit,
};
use drydock_kernel::texture::TextureMap;
use drydock_kernel::topology::{EdgeUse, Face, FaceId, Loop};
use glam::{DMat4, DQuat, DVec2, DVec3, DVec4};

// ---------------------------------------------------------------------------------------
// Import

const QUAD: [[f32; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
];

fn v3(p: [f32; 3]) -> DVec3 {
    DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)
}

fn v2(p: [f32; 2]) -> DVec2 {
    DVec2::new(p[0] as f64, p[1] as f64)
}

/// The texture coordinates the tests give a quad corner: an affine function of the point.
fn quad_uv(p: [f32; 3]) -> [f32; 2] {
    [2.0 * p[0] + 0.25, 3.0 * p[1] - 0.5]
}

/// Triangles with one texture coordinate per corner (`uv` empty: none).
fn tris_uv(symbol: &str, triangles: &[[[f32; 3]; 3]], uv: &[[[f32; 2]; 3]]) -> Primitive {
    Primitive {
        kind: PrimitiveKind::Triangles,
        material_symbol: Some(symbol.to_string()),
        positions: triangles.iter().flatten().copied().collect(),
        normals: Vec::new(),
        texcoords: uv.iter().flatten().copied().collect(),
        indices: (0..triangles.len() as u32 * 3).collect(),
    }
}

fn quad_tris() -> Vec<[[f32; 3]; 3]> {
    vec![[QUAD[0], QUAD[1], QUAD[2]], [QUAD[0], QUAD[2], QUAD[3]]]
}

fn uv_of(triangles: &[[[f32; 3]; 3]]) -> Vec<[[f32; 2]; 3]> {
    triangles.iter().map(|t| t.map(quad_uv)).collect()
}

fn textured(id: &str) -> drydock_dae::Material {
    let mut m = material(id, id, [1.0, 1.0, 1.0, 1.0]);
    m.texture = Some(PathBuf::from(format!("/x/model/{id}.png")));
    m
}

/// A document whose root holds `primitives`, symbol `"m"` bound to material 0 and `"n"` to
/// material 1.
fn doc_with(materials: Vec<drydock_dae::Material>, primitives: Vec<Primitive>) -> Model {
    let mut d = doc();
    d.materials = materials;
    let gi = add_mesh(&mut d, primitives, &[("m", 0), ("n", 1)]);
    d.scene.geometry.push(gi);
    d
}

fn convert(d: &Model) -> km::Model {
    to_kernel(d).expect("document converts")
}

fn faces(m: &km::Model, body: BodyId) -> Vec<(FaceId, Face, FaceMaterials)> {
    let b = m.body(body).expect("body exists");
    b.geometry()
        .faces()
        .map(|(id, f)| (id, f.clone(), b.face_materials(id)))
        .collect()
}

fn corners(m: &km::Model, body: BodyId, face: &Face) -> Vec<DVec3> {
    m.body(body).unwrap().geometry().loop_points(&face.outer)
}

/// `map` sends each point to its texture coordinates (texture size is 1 m × 1 m on import).
fn assert_map_hits(map: TextureMap, points: &[DVec3], uv: &[[f32; 2]]) {
    for (p, uv) in points.iter().zip(uv) {
        let got = map.apply(*p);
        assert!(
            (got - v2(*uv)).abs().max_element() < 1e-5,
            "at {p}: map gives {got}, file says {uv:?}"
        );
    }
}

#[test]
fn textured_quad_becomes_one_face_with_its_placement() {
    let t = quad_tris();
    let d = doc_with(vec![textured("wood")], vec![tris_uv("m", &t, &uv_of(&t))]);
    let m = convert(&d);
    let fs = faces(&m, BodyId::Root);
    assert_eq!(
        fs.len(),
        1,
        "two coplanar triangles with one placement merge"
    );
    let (_, face, fm) = &fs[0];
    let map = fm.front_map.expect("textured side keeps its placement");
    assert_eq!(fm.back_map, None);
    let pts = corners(&m, BodyId::Root, face);
    let uv: Vec<[f32; 2]> = pts
        .iter()
        .map(|p| quad_uv([p.x as f32, p.y as f32, p.z as f32]))
        .collect();
    assert_map_hits(map, &pts, &uv);
    // The map ignores how far a point is off the face.
    let lifted = map.apply(DVec3::new(0.5, 0.5, 3.0));
    assert!((lifted - map.apply(DVec3::new(0.5, 0.5, 0.0))).length() < 1e-9);
}

#[test]
fn imported_texture_is_one_meter_square() {
    let t = quad_tris();
    let d = doc_with(vec![textured("wood")], vec![tris_uv("m", &t, &uv_of(&t))]);
    let m = convert(&d);
    let (_, mat) = m.materials().next().expect("one material");
    let texture = mat.texture.as_ref().expect("material keeps its texture");
    assert_eq!(texture.size, DVec2::ONE);
}

#[test]
fn untextured_material_stores_no_placement() {
    let t = quad_tris();
    let d = doc_with(
        vec![material("red", "red", [1.0, 0.0, 0.0, 1.0])],
        vec![tris_uv("m", &t, &uv_of(&t))],
    );
    let m = convert(&d);
    let fs = faces(&m, BodyId::Root);
    assert_eq!(fs.len(), 1);
    assert!(fs[0].2.front.is_some());
    assert_eq!(fs[0].2.front_map, None);
    assert_eq!(fs[0].2.back_map, None);
}

#[test]
fn textured_material_without_coordinates_keeps_the_default() {
    let t = quad_tris();
    let d = doc_with(vec![textured("wood")], vec![tris_uv("m", &t, &[])]);
    let m = convert(&d);
    let fs = faces(&m, BodyId::Root);
    assert_eq!(fs.len(), 1);
    assert!(fs[0].2.front.is_some());
    assert_eq!(fs[0].2.front_map, None);
}

#[test]
fn different_placements_do_not_merge() {
    let t = quad_tris();
    let mut uv = uv_of(&t);
    for c in uv[1].iter_mut() {
        c[0] += 0.5;
    }
    let d = doc_with(vec![textured("wood")], vec![tris_uv("m", &t, &uv)]);
    let m = convert(&d);
    let fs = faces(&m, BodyId::Root);
    assert_eq!(
        fs.len(),
        2,
        "a seam in the texture keeps the triangles apart"
    );
    for (_, face, fm) in &fs {
        let map = fm.front_map.expect("each face keeps a placement");
        let pts = corners(&m, BodyId::Root, face);
        // Find which file triangle this face is, by its corners.
        let k = (0..2)
            .find(|&k| {
                pts.iter()
                    .all(|p| t[k].iter().any(|q| (v3(*q) - *p).length() < 1e-9))
            })
            .expect("face is one of the file's triangles");
        assert_map_hits(map, &t[k].map(v3), &uv[k]);
    }
}

#[test]
fn tiny_differences_in_coordinates_still_merge() {
    let t = quad_tris();
    let mut uv = uv_of(&t);
    // The corner only the second triangle has, off by much less than a pixel.
    uv[1][2][0] += 2e-5;
    uv[1][2][1] -= 2e-5;
    let d = doc_with(vec![textured("wood")], vec![tris_uv("m", &t, &uv)]);
    let m = convert(&d);
    let fs = faces(&m, BodyId::Root);
    assert_eq!(fs.len(), 1);
    assert!(fs[0].2.front_map.is_some());
}

#[test]
fn placed_and_unplaced_triangles_do_not_merge() {
    let t = quad_tris();
    let uv = uv_of(&t);
    let d = doc_with(
        vec![textured("wood")],
        vec![tris_uv("m", &t[..1], &uv[..1]), tris_uv("m", &t[1..], &[])],
    );
    let m = convert(&d);
    let fs = faces(&m, BodyId::Root);
    assert_eq!(fs.len(), 2);
    let placed = fs.iter().filter(|f| f.2.front_map.is_some()).count();
    assert_eq!(placed, 1);
}

#[test]
fn back_side_keeps_the_twin_coordinates() {
    let t = [QUAD[0], QUAD[1], QUAD[2]];
    let twin = [t[0], t[2], t[1]];
    let twin_uv = [[0.0, 0.0], [5.0, 1.0], [4.0, 0.0]];
    let d = doc_with(
        vec![
            material("red", "red", [1.0, 0.0, 0.0, 1.0]),
            textured("tile"),
        ],
        vec![
            tris_uv("m", &[t], &[t.map(quad_uv)]),
            tris_uv("n", &[twin], &[twin_uv]),
        ],
    );
    let m = convert(&d);
    let fs = faces(&m, BodyId::Root);
    assert_eq!(fs.len(), 1);
    let fm = fs[0].2;
    assert!(fm.front.is_some() && fm.back.is_some());
    assert_eq!(fm.front_map, None, "red has no texture");
    let map = fm.back_map.expect("textured back keeps its placement");
    assert_map_hits(map, &twin.map(v3), &twin_uv);
}

#[test]
fn placement_uses_converted_coordinates() {
    // Inches, Y up: file (x, y, z) is (x, -z, y) × 0.0254 in the model.
    let q = [
        [0.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        [10.0, 0.0, -20.0],
        [0.0, 0.0, -20.0],
    ];
    let t = vec![[q[0], q[1], q[2]], [q[0], q[2], q[3]]];
    let uv: Vec<[[f32; 2]; 3]> = t
        .iter()
        .map(|tri| tri.map(|p| [p[0] / 10.0, -p[2] / 20.0]))
        .collect();
    let mut d = doc_with(vec![textured("wood")], vec![tris_uv("m", &t, &uv)]);
    d.unit_meters = 0.0254;
    d.up_axis = UpAxis::Y;
    let m = convert(&d);
    let fs = faces(&m, BodyId::Root);
    assert_eq!(fs.len(), 1);
    let map = fs[0].2.front_map.expect("placement kept");
    let s = 0.0254;
    for (tri, tri_uv) in t.iter().zip(&uv) {
        let pts = tri.map(|p| DVec3::new(p[0] as f64 * s, -p[2] as f64 * s, p[1] as f64 * s));
        assert_map_hits(map, &pts, tri_uv);
    }
}

#[test]
fn definitions_keep_placement_in_their_own_coordinates() {
    let t = quad_tris();
    let mut d = doc();
    d.materials = vec![textured("wood")];
    let gi = add_mesh(&mut d, vec![tris_uv("m", &t, &uv_of(&t))], &[("m", 0)]);
    let mut inner = node("leg");
    inner.geometry.push(gi);
    d.definitions.push(definition("leg", "leg", inner));
    for x in [3.0, 7.0] {
        let mut k = node("");
        k.transform = DMat4::from_translation(DVec3::new(x, 0.0, 0.0));
        k.instances.push(0);
        d.scene.children.push(k);
    }
    let m = convert(&d);
    let body = m
        .definitions()
        .map(|(id, _)| BodyId::Definition(id))
        .find(|&b| m.body(b).unwrap().geometry().faces().count() > 0)
        .expect("a definition holds the quad");
    let fs = faces(&m, body);
    assert_eq!(fs.len(), 1);
    let map = fs[0].2.front_map.expect("placement kept");
    for (tri, tri_uv) in t.iter().zip(uv_of(&t)) {
        assert_map_hits(map, &tri.map(v3), &tri_uv);
    }
}

// ---------------------------------------------------------------------------------------
// Drawing

struct B {
    m: km::Model,
    log: ModelEdit,
}

impl B {
    fn new() -> Self {
        B {
            m: km::Model::new(),
            log: ModelEdit::new(),
        }
    }

    fn material(&mut self, name: &str, texture: Option<km::Texture>) -> MaterialId {
        let mut mat = km::Material::new(name, [0.5, 0.5, 0.5, 1.0]);
        mat.texture = texture;
        self.m.add_material(mat, &mut self.log)
    }

    fn face(&mut self, body: BodyId, pts: &[DVec3]) -> FaceId {
        self.m
            .edit_geometry(body, &mut self.log, |ctx, log| {
                let vs: Vec<_> = pts.iter().map(|&q| ctx.insert_vertex(q, log)).collect();
                let uses = (0..vs.len())
                    .map(|i| {
                        EdgeUse::forward(
                            ctx.insert_edge(vs[i], vs[(i + 1) % vs.len()], log)
                                .expect("edge inserts"),
                        )
                    })
                    .collect();
                ctx.insert_face(
                    Face {
                        outer: Loop { uses },
                        inner: Vec::new(),
                    },
                    log,
                )
                .expect("face inserts")
            })
            .expect("body exists")
    }

    fn paint(&mut self, body: BodyId, face: FaceId, fm: FaceMaterials) {
        self.m
            .set_face_materials(body, face, fm, &mut self.log)
            .expect("materials set");
    }
}

fn p(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// Square from (x0, y0) to (x0 + 1, y0 + 1) in z = 0, facing +Z.
fn square(x0: f64, y0: f64) -> Vec<DVec3> {
    vec![
        p(x0, y0, 0.0),
        p(x0 + 1.0, y0, 0.0),
        p(x0 + 1.0, y0 + 1.0, 0.0),
        p(x0, y0 + 1.0, 0.0),
    ]
}

fn batch_of(s: &FlatScene, material: usize) -> &Batch {
    s.batches
        .iter()
        .find(|b| b.material == Some(material))
        .expect("batch for material")
}

fn index_of(s: &FlatScene, id: MaterialId) -> usize {
    s.materials
        .iter()
        .position(|m| m.id == id.0.to_string())
        .expect("material in scene")
}

/// Every vertex of `b` has texture coordinates, and they are `want(world position, normal)`.
fn assert_texcoords(b: &Batch, want: impl Fn(DVec3, DVec3) -> DVec2) {
    assert_eq!(b.texcoords.len(), b.positions.len());
    for ((pos, n), uv) in b.positions.iter().zip(&b.normals).zip(&b.texcoords) {
        let (pos, n) = (v3(*pos), v3(*n));
        let want = want(pos, n);
        assert!(
            (v2(*uv) - want).abs().max_element() < 1e-5,
            "at {pos} (normal {n}): uv {uv:?}, want {want}"
        );
    }
}

#[test]
fn scene_materials_carry_the_texture_path() {
    let mut b = B::new();
    let red = b.material("red", None);
    let wood = b.material(
        "wood",
        Some(km::Texture::new("/x/wood.png", DVec2::new(2.0, 1.0))),
    );
    let s = kernel_scene(&b.m);
    assert_eq!(s.materials[index_of(&s, red)].texture, None);
    assert_eq!(
        s.materials[index_of(&s, wood)].texture,
        Some(PathBuf::from("/x/wood.png"))
    );
}

#[test]
fn default_placement_on_a_horizontal_face() {
    let mut b = B::new();
    let size = DVec2::new(2.0, 0.5);
    let wood = b.material("wood", Some(km::Texture::new("wood.png", size)));
    let f = b.face(BodyId::Root, &square(3.0, -1.0));
    b.paint(
        BodyId::Root,
        f,
        FaceMaterials {
            front: Some(wood),
            ..Default::default()
        },
    );
    let s = kernel_scene(&b.m);
    let batch = batch_of(&s, index_of(&s, wood));
    assert_eq!(batch.indices.len(), 6);
    assert_texcoords(batch, |q, _| DVec2::new(q.x, q.y) / size);
}

#[test]
fn stored_placement_is_used() {
    let mut b = B::new();
    let size = DVec2::new(0.5, 0.25);
    let wood = b.material("wood", Some(km::Texture::new("wood.png", size)));
    let f = b.face(BodyId::Root, &square(0.0, 0.0));
    let map = TextureMap {
        u: DVec4::new(0.0, 1.0, 0.0, 0.5),
        v: DVec4::new(-1.0, 0.0, 0.0, 2.0),
    };
    b.paint(
        BodyId::Root,
        f,
        FaceMaterials {
            front: Some(wood),
            front_map: Some(map),
            ..Default::default()
        },
    );
    let s = kernel_scene(&b.m);
    assert_texcoords(batch_of(&s, index_of(&s, wood)), |q, _| {
        DVec2::new(q.y + 0.5, 2.0 - q.x) / size
    });
}

#[test]
fn back_side_uses_its_own_placement() {
    let mut b = B::new();
    let red = b.material("red", None);
    let tile = b.material("tile", Some(km::Texture::new("tile.png", DVec2::ONE)));
    let f = b.face(BodyId::Root, &square(1.0, 2.0));
    b.paint(
        BodyId::Root,
        f,
        FaceMaterials {
            front: Some(red),
            back: Some(tile),
            ..Default::default()
        },
    );
    let s = kernel_scene(&b.m);
    let red_batch = batch_of(&s, index_of(&s, red));
    assert!(
        red_batch.texcoords.is_empty(),
        "untextured batches have none"
    );
    let tile_batch = batch_of(&s, index_of(&s, tile));
    // Default for -Z: right is -x seen from below, up is +y.
    assert_texcoords(tile_batch, |q, n| {
        assert!(n.z < -0.99, "only the back is in this batch");
        DVec2::new(-q.x, q.y)
    });

    // A stored back map wins; the front's (untextured) side has none.
    let map = TextureMap {
        u: DVec4::new(0.0, 0.0, 0.0, 0.25),
        v: DVec4::new(1.0, 1.0, 0.0, 0.0),
    };
    b.paint(
        BodyId::Root,
        f,
        FaceMaterials {
            front: Some(red),
            back: Some(tile),
            back_map: Some(map),
            ..Default::default()
        },
    );
    let s = kernel_scene(&b.m);
    assert_texcoords(batch_of(&s, index_of(&s, tile)), |q, _| {
        DVec2::new(0.25, q.x + q.y)
    });
}

#[test]
fn untextured_and_default_batches_have_no_coordinates() {
    let mut b = B::new();
    let red = b.material("red", None);
    let _wood = b.material("wood", Some(km::Texture::new("wood.png", DVec2::ONE)));
    let f = b.face(BodyId::Root, &square(0.0, 0.0));
    b.face(BodyId::Root, &square(5.0, 0.0));
    b.paint(
        BodyId::Root,
        f,
        FaceMaterials {
            front: Some(red),
            ..Default::default()
        },
    );
    let s = kernel_scene(&b.m);
    assert!(!s.batches.is_empty());
    for batch in &s.batches {
        assert!(batch.texcoords.is_empty(), "{:?}", batch.material);
    }
}

#[test]
fn every_face_in_a_textured_batch_gets_coordinates() {
    let mut b = B::new();
    let wood = b.material("wood", Some(km::Texture::new("wood.png", DVec2::ONE)));
    let a = b.face(BodyId::Root, &square(0.0, 0.0));
    let c = b.face(BodyId::Root, &square(4.0, 0.0));
    let map = TextureMap::default_for(DVec3::Z)
        .then(glam::DAffine2::from_translation(DVec2::new(0.5, 0.0)));
    b.paint(
        BodyId::Root,
        a,
        FaceMaterials {
            front: Some(wood),
            ..Default::default()
        },
    );
    b.paint(
        BodyId::Root,
        c,
        FaceMaterials {
            front: Some(wood),
            front_map: Some(map),
            ..Default::default()
        },
    );
    let s = kernel_scene(&b.m);
    assert_texcoords(batch_of(&s, index_of(&s, wood)), |q, _| {
        if q.x < 2.0 {
            DVec2::new(q.x, q.y)
        } else {
            DVec2::new(q.x + 0.5, q.y)
        }
    });
}

fn instance_of(
    b: &mut B,
    faces: &[Vec<DVec3>],
    transform: DMat4,
    material: Option<MaterialId>,
) -> BodyId {
    let def = b.m.add_definition(
        DefinitionInfo {
            name: "d".into(),
            group: false,
        },
        &mut b.log,
    );
    let body = BodyId::Definition(def);
    for f in faces {
        b.face(body, f);
    }
    b.m.add_instance(
        BodyId::Root,
        Instance {
            definition: def,
            transform,
            name: String::new(),
            material,
        },
        &mut b.log,
    )
    .expect("instance adds");
    body
}

#[test]
fn coordinates_come_from_the_body_not_the_world() {
    let mut b = B::new();
    let size = DVec2::new(0.5, 2.0);
    let wood = b.material("wood", Some(km::Texture::new("wood.png", size)));
    let t = DMat4::from_scale_rotation_translation(
        DVec3::splat(3.0),
        DQuat::from_rotation_z(0.7),
        DVec3::new(10.0, -4.0, 1.0),
    );
    let body = instance_of(&mut b, &[square(0.0, 0.0)], t, None);
    let f = faces(&b.m, body)[0].0;
    b.paint(
        body,
        f,
        FaceMaterials {
            front: Some(wood),
            ..Default::default()
        },
    );
    let s = kernel_scene(&b.m);
    let inv = t.inverse();
    assert_texcoords(batch_of(&s, index_of(&s, wood)), |q, _| {
        let local = inv.transform_point3(q);
        DVec2::new(local.x, local.y) / size
    });
}

#[test]
fn inherited_textured_material_uses_the_face_default() {
    let mut b = B::new();
    let wood = b.material("wood", Some(km::Texture::new("wood.png", DVec2::ONE)));
    // A face in x = 1 facing +X: default right is +y, up is +z. The instance material
    // paints both sides; the back (facing -X) has right = -y.
    let wall = vec![
        p(1.0, 0.0, 0.0),
        p(1.0, 1.0, 0.0),
        p(1.0, 1.0, 1.0),
        p(1.0, 0.0, 1.0),
    ];
    assert!(polygon_normal(&wall).x > 0.0);
    let t = DMat4::from_translation(DVec3::new(0.0, 5.0, 0.0));
    instance_of(&mut b, &[wall], t, Some(wood));
    let s = kernel_scene(&b.m);
    let batch = batch_of(&s, index_of(&s, wood));
    assert_eq!(batch.indices.len(), 12, "both sides drawn");
    assert_texcoords(batch, |q, n| {
        if n.x > 0.0 {
            DVec2::new(q.y - 5.0, q.z)
        } else {
            DVec2::new(5.0 - q.y, q.z)
        }
    });
}

// ---------------------------------------------------------------------------------------
// Import, then draw

#[test]
fn imported_coordinates_survive_into_the_kernel_scene() {
    let t = quad_tris();
    let d = doc_with(vec![textured("wood")], vec![tris_uv("m", &t, &uv_of(&t))]);
    let m = convert(&d);
    let s = kernel_scene(&m);
    assert_eq!(s.materials.len(), 1);
    assert_eq!(
        s.materials[0].texture,
        Some(PathBuf::from("/x/model/wood.png"))
    );
    assert_texcoords(batch_of(&s, 0), |q, _| {
        v2(quad_uv([q.x as f32, q.y as f32, q.z as f32]))
    });
}
