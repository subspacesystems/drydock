// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! `infer::material_in`: the material drawn under the cursor, inside instances at any
//! depth (`docs/specs/m6-eyedropper-deep.md`).

use drydock_kernel::editpath::EditPath;
use drydock_kernel::infer::*;
use drydock_kernel::model::*;
use drydock_kernel::paint::{Side, create_material, paint};
use drydock_kernel::select::Entity;
use drydock_kernel::topology::FaceId;
use glam::{DMat4, DVec2, DVec3};

const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A parallel probe in body coordinates for a body mapped to the world with `m`: the cursor
/// over world (x, y), looking down (`down`) or up from below. 10 px per meter, radius 5 px.
fn probe<R>(m: DMat4, x: f64, y: f64, down: bool, f: impl FnOnce(&Probe) -> R) -> R {
    let inv = m.inverse();
    let project = move |q: DVec3| {
        let w = m.transform_point3(q);
        Some(DVec2::new(w.x, w.y) * 10.0)
    };
    let (eye, dir) = if down {
        (v(x, y, 100.0), v(0.0, 0.0, -1.0))
    } else {
        (v(x, y, -100.0), v(0.0, 0.0, 1.0))
    };
    let probe = Probe {
        cursor: DVec2::new(x, y) * 10.0,
        origin: inv.transform_point3(eye),
        dir: inv.transform_vector3(dir),
        parallel: true,
        project: &project,
        radius: 5.0,
        anchor: None,
        axes: AXES,
        centers: &[],
        lock: None,
    };
    f(&probe)
}

fn at_root(model: &Model, x: f64, y: f64) -> Option<Option<MaterialId>> {
    probe(DMat4::IDENTITY, x, y, true, |p| {
        material_in(model, &EditPath::root(), true, p)
    })
}

fn square(x0: f64, y0: f64, z: f64, size: f64) -> Vec<DVec3> {
    vec![
        v(x0, y0, z),
        v(x0 + size, y0, z),
        v(x0 + size, y0 + size, z),
        v(x0, y0 + size, z),
    ]
}

fn polygon(m: &mut Model, body: BodyId, pts: &[DVec3]) {
    let mut log = ModelEdit::new();
    m.edit_geometry(body, &mut log, |ctx, edit| {
        for i in 0..pts.len() {
            ctx.add_edge(pts[i], pts[(i + 1) % pts.len()], edit)
                .unwrap();
        }
    })
    .unwrap();
}

fn define(m: &mut Model, name: &str) -> DefinitionId {
    let mut log = ModelEdit::new();
    m.add_definition(
        DefinitionInfo {
            name: name.into(),
            group: false,
        },
        &mut log,
    )
}

fn place(
    m: &mut Model,
    body: BodyId,
    d: DefinitionId,
    transform: DMat4,
    material: Option<MaterialId>,
) -> InstanceId {
    let mut log = ModelEdit::new();
    m.add_instance(
        body,
        Instance {
            definition: d,
            transform,
            name: String::new(),
            material,
        },
        &mut log,
    )
    .unwrap()
}

fn at(x: f64, y: f64, z: f64) -> DMat4 {
    DMat4::from_translation(v(x, y, z))
}

fn only_face(m: &Model, body: BodyId) -> FaceId {
    let faces: Vec<FaceId> = m
        .body(body)
        .unwrap()
        .geometry()
        .faces()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(faces.len(), 1);
    faces[0]
}

fn normal(m: &Model, body: BodyId, face: FaceId) -> DVec3 {
    m.body(body)
        .unwrap()
        .geometry()
        .face_plane(face)
        .unwrap()
        .normal
}

fn paint_side(m: &mut Model, body: BodyId, face: FaceId, side: Side, mat: MaterialId) {
    let mut undo = UndoStack::new();
    paint(
        m,
        body,
        &[Entity::Face(face)],
        side,
        Some(mat),
        &mut undo,
        "Paint",
    )
    .unwrap();
}

/// The side of a face with body normal `n` that a ray along world `dir` sees, for a body
/// mapped with linear part `m` (normals map with the inverse transpose).
fn seen(m: DMat4, n: DVec3, dir: DVec3) -> Side {
    let world = m.inverse().transpose().transform_vector3(n);
    if world.dot(dir) < 0.0 {
        Side::Front
    } else {
        Side::Back
    }
}

struct Colors {
    red: MaterialId,
    green: MaterialId,
    blue: MaterialId,
    yellow: MaterialId,
}

fn colors(m: &mut Model) -> Colors {
    let mut undo = UndoStack::new();
    let mut make = |name: &str, c: [f32; 4]| {
        create_material(m, Material::new(name, c), &mut undo, "New").unwrap()
    };
    Colors {
        red: make("red", [1.0, 0.0, 0.0, 1.0]),
        green: make("green", [0.0, 1.0, 0.0, 1.0]),
        blue: make("blue", [0.0, 0.0, 1.0, 1.0]),
        yellow: make("yellow", [1.0, 1.0, 0.0, 1.0]),
    }
}

/// A definition holding the 2 m square (0,0,0)-(2,2,0) and its only face.
fn square_def(m: &mut Model, name: &str) -> (DefinitionId, FaceId) {
    let d = define(m, name);
    polygon(m, BodyId::Definition(d), &square(0.0, 0.0, 0.0, 2.0));
    let f = only_face(m, BodyId::Definition(d));
    (d, f)
}

#[test]
fn a_root_face_gives_the_side_the_ray_sees() {
    let mut m = Model::new();
    let c = colors(&mut m);
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 0.0, 2.0));
    let f = only_face(&m, BodyId::Root);
    let n = normal(&m, BodyId::Root, f);
    let top = seen(DMat4::IDENTITY, n, v(0.0, 0.0, -1.0));
    let bottom = seen(DMat4::IDENTITY, n, v(0.0, 0.0, 1.0));
    assert_ne!(top, bottom);
    paint_side(&mut m, BodyId::Root, f, top, c.green);
    paint_side(&mut m, BodyId::Root, f, bottom, c.yellow);
    assert_eq!(at_root(&m, 1.0, 1.0), Some(Some(c.green)));
    let below = probe(DMat4::IDENTITY, 1.0, 1.0, false, |p| {
        material_in(&m, &EditPath::root(), true, p)
    });
    assert_eq!(below, Some(Some(c.yellow)));
}

#[test]
fn an_unpainted_root_face_is_the_default_and_empty_space_is_none() {
    let mut m = Model::new();
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 0.0, 2.0));
    assert_eq!(at_root(&m, 1.0, 1.0), Some(None));
    assert_eq!(at_root(&m, 10.0, 10.0), None);
    assert_eq!(at_root(&Model::new(), 1.0, 1.0), None);
}

#[test]
fn a_painted_face_inside_an_instance_wins_over_the_instance() {
    let mut m = Model::new();
    let c = colors(&mut m);
    let (d, f) = square_def(&mut m, "part");
    let body = BodyId::Definition(d);
    let top = seen(at(5.0, 0.0, 0.0), normal(&m, body, f), v(0.0, 0.0, -1.0));
    paint_side(&mut m, body, f, top, c.green);
    place(&mut m, BodyId::Root, d, at(5.0, 0.0, 0.0), Some(c.blue));
    assert_eq!(at_root(&m, 6.0, 1.0), Some(Some(c.green)));
}

#[test]
fn an_unpainted_face_takes_the_instance_material() {
    let mut m = Model::new();
    let c = colors(&mut m);
    let (d, _) = square_def(&mut m, "part");
    place(&mut m, BodyId::Root, d, at(5.0, 0.0, 0.0), Some(c.blue));
    place(&mut m, BodyId::Root, d, at(0.0, 5.0, 0.0), None);
    assert_eq!(at_root(&m, 6.0, 1.0), Some(Some(c.blue)));
    assert_eq!(at_root(&m, 1.0, 6.0), Some(None), "unpainted all the way");
}

#[test]
fn the_innermost_painted_instance_wins() {
    let mut m = Model::new();
    let c = colors(&mut m);
    let (inner, _) = square_def(&mut m, "inner");
    let outer = define(&mut m, "outer");
    place(
        &mut m,
        BodyId::Definition(outer),
        inner,
        at(0.0, 0.0, 0.0),
        None,
    );
    place(
        &mut m,
        BodyId::Definition(outer),
        inner,
        at(3.0, 0.0, 0.0),
        Some(c.blue),
    );
    place(&mut m, BodyId::Root, outer, at(10.0, 0.0, 0.0), Some(c.red));
    // Inner instance unpainted: the outer instance's red shows.
    assert_eq!(at_root(&m, 11.0, 1.0), Some(Some(c.red)));
    // Inner instance blue: blue shows.
    assert_eq!(at_root(&m, 14.0, 1.0), Some(Some(c.blue)));
}

#[test]
fn a_face_nested_two_deep_gives_its_own_material() {
    let mut m = Model::new();
    let c = colors(&mut m);
    let (inner, f) = square_def(&mut m, "inner");
    let outer = define(&mut m, "outer");
    place(
        &mut m,
        BodyId::Definition(outer),
        inner,
        at(1.0, 1.0, 0.0),
        Some(c.blue),
    );
    place(&mut m, BodyId::Root, outer, at(10.0, 0.0, 0.0), Some(c.red));
    let body = BodyId::Definition(inner);
    let top = seen(at(11.0, 1.0, 0.0), normal(&m, body, f), v(0.0, 0.0, -1.0));
    paint_side(&mut m, body, f, top, c.yellow);
    assert_eq!(at_root(&m, 12.0, 2.0), Some(Some(c.yellow)));
}

#[test]
fn the_nearest_face_along_the_ray_wins() {
    let mut m = Model::new();
    let c = colors(&mut m);
    let (d, _) = square_def(&mut m, "part");
    place(&mut m, BodyId::Root, d, at(0.0, 0.0, 0.0), Some(c.red));
    place(&mut m, BodyId::Root, d, at(0.0, 0.0, 3.0), Some(c.green));
    assert_eq!(at_root(&m, 1.0, 1.0), Some(Some(c.green)), "from above");
    let below = probe(DMat4::IDENTITY, 1.0, 1.0, false, |p| {
        material_in(&m, &EditPath::root(), true, p)
    });
    assert_eq!(below, Some(Some(c.red)), "from below");
}

#[test]
fn a_cursor_near_an_edge_still_gives_the_face() {
    let mut m = Model::new();
    let c = colors(&mut m);
    let (d, f) = square_def(&mut m, "part");
    let body = BodyId::Definition(d);
    let top = seen(at(5.0, 0.0, 0.0), normal(&m, body, f), v(0.0, 0.0, -1.0));
    paint_side(&mut m, body, f, top, c.green);
    place(&mut m, BodyId::Root, d, at(5.0, 0.0, 0.0), None);
    // 0.1 m inside the face: well within the 0.5 m snap radius of its edge.
    assert_eq!(at_root(&m, 5.1, 1.0), Some(Some(c.green)));
}

#[test]
fn a_mirrored_instance_shows_the_side_its_normal_faces() {
    let mut m = Model::new();
    let c = colors(&mut m);
    let (d, f) = square_def(&mut m, "part");
    let body = BodyId::Definition(d);
    let n = normal(&m, body, f);
    paint_side(&mut m, body, f, Side::Front, c.green);
    paint_side(&mut m, body, f, Side::Back, c.yellow);
    let mirror = at(5.0, 0.0, 1.0) * DMat4::from_scale(v(1.0, 1.0, -1.0));
    place(&mut m, BodyId::Root, d, mirror, None);
    let want = match seen(mirror, n, v(0.0, 0.0, -1.0)) {
        Side::Front => c.green,
        Side::Back => c.yellow,
    };
    assert_eq!(at_root(&m, 6.0, 1.0), Some(Some(want)));
    // The same face unmirrored shows the other side from above.
    let plain = at(0.0, 5.0, 1.0);
    place(&mut m, BodyId::Root, d, plain, None);
    let other = match seen(plain, n, v(0.0, 0.0, -1.0)) {
        Side::Front => c.green,
        Side::Back => c.yellow,
    };
    assert_ne!(want, other);
    assert_eq!(at_root(&m, 1.0, 6.0), Some(Some(other)));
}

/// Root holds instance `a` of "box" (blue) at x 0 and instance `b` of "other" (red) at x 5.
/// "box" holds a square and an unpainted instance of "inner" at x 3 (inside `a`: world x 3).
struct Opened {
    m: Model,
    c: Colors,
    a: InstanceId,
}

fn opened() -> Opened {
    let mut m = Model::new();
    let c = colors(&mut m);
    let (inner, _) = square_def(&mut m, "inner");
    let (boxd, _) = square_def(&mut m, "box");
    place(
        &mut m,
        BodyId::Definition(boxd),
        inner,
        at(2.5, 0.0, 0.0),
        None,
    );
    let (other, _) = square_def(&mut m, "other");
    let a = place(&mut m, BodyId::Root, boxd, at(0.0, 0.0, 0.0), Some(c.blue));
    place(&mut m, BodyId::Root, other, at(10.0, 0.0, 0.0), Some(c.red));
    Opened { m, c, a }
}

#[test]
fn inside_an_open_instance_its_faces_inherit_the_open_instance() {
    let o = opened();
    let path = EditPath::root().open(&o.m, o.a).unwrap();
    let m = path.transform(&o.m).unwrap();
    let got = probe(m, 1.0, 1.0, true, |p| material_in(&o.m, &path, true, p));
    assert_eq!(got, Some(Some(o.c.blue)), "the open body's own face");
    let got = probe(m, 3.5, 1.0, true, |p| material_in(&o.m, &path, true, p));
    assert_eq!(got, Some(Some(o.c.blue)), "a face in an unpainted child");
}

#[test]
fn the_rest_of_the_model_counts_only_when_shown() {
    let o = opened();
    let path = EditPath::root().open(&o.m, o.a).unwrap();
    let m = path.transform(&o.m).unwrap();
    let shown = probe(m, 11.0, 1.0, true, |p| material_in(&o.m, &path, true, p));
    assert_eq!(shown, Some(Some(o.c.red)));
    let hidden = probe(m, 11.0, 1.0, true, |p| material_in(&o.m, &path, false, p));
    assert_eq!(hidden, None);
}

#[test]
fn a_path_that_does_not_resolve_is_none() {
    let mut o = opened();
    let path = EditPath::root().open(&o.m, o.a).unwrap();
    let mut log = ModelEdit::new();
    o.m.remove_instance(BodyId::Root, o.a, &mut log).unwrap();
    let got = probe(DMat4::IDENTITY, 1.0, 1.0, true, |p| {
        material_in(&o.m, &path, true, p)
    });
    assert_eq!(got, None);
}
