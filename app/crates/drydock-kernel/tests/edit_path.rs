// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Editing inside an instance: `EditPath`, `pick_in`, `infer_in`
//! (`docs/specs/m4-edit-path.md`).

use drydock_kernel::editpath::EditPath;
use drydock_kernel::infer::*;
use drydock_kernel::model::*;
use drydock_kernel::select::{BoxMode, Entity, box_select};
use drydock_kernel::topology::{EdgeId, FaceId, VertexId};
use glam::{DMat4, DQuat, DVec2, DVec3};

const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn assert_close(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-9, "{a:?} != {b:?}");
}

/// Looking straight down, parallel: 10 pixels per world meter, screen = 10 * (x, y).
fn top_project(q: DVec3) -> Option<DVec2> {
    Some(DVec2::new(q.x, q.y) * 10.0)
}

/// World top-view probe with the cursor over world point (x, y). Radius 5 px (0.5 m).
fn world<R>(x: f64, y: f64, f: impl FnOnce(&Probe) -> R) -> R {
    local(DMat4::IDENTITY, x, y, f)
}

/// The same top-view probe as the app builds it for a body whose coordinates map to the world
/// with `m`: ray through the inverse, projection composed with `m`. Cursor over world (x, y).
fn local<R>(m: DMat4, x: f64, y: f64, f: impl FnOnce(&Probe) -> R) -> R {
    let inv = m.inverse();
    let project = move |q: DVec3| top_project(m.transform_point3(q));
    let probe = Probe {
        cursor: DVec2::new(x, y) * 10.0,
        origin: inv.transform_point3(v(x, y, 100.0)),
        dir: inv.transform_vector3(v(0.0, 0.0, -1.0)),
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

fn square(x0: f64, y0: f64, z: f64, size: f64) -> Vec<DVec3> {
    vec![
        v(x0, y0, z),
        v(x0 + size, y0, z),
        v(x0 + size, y0 + size, z),
        v(x0, y0 + size, z),
    ]
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

fn place(m: &mut Model, body: BodyId, d: DefinitionId, transform: DMat4) -> InstanceId {
    let mut log = ModelEdit::new();
    m.add_instance(
        body,
        Instance {
            definition: d,
            transform,
            name: String::new(),
            material: None,
        },
        &mut log,
    )
    .unwrap()
}

fn at(x: f64, y: f64, z: f64) -> DMat4 {
    DMat4::from_translation(v(x, y, z))
}

/// A definition holding the 2 m square (0,0,0)-(2,2,0).
fn square_def(m: &mut Model) -> DefinitionId {
    let d = define(m, "part");
    polygon(m, BodyId::Definition(d), &square(0.0, 0.0, 0.0, 2.0));
    d
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

fn vertex_at(m: &Model, body: BodyId, p: DVec3) -> VertexId {
    m.body(body)
        .unwrap()
        .geometry()
        .vertices()
        .find(|(_, vx)| vx.position.distance(p) < 1e-9)
        .map(|(id, _)| id)
        .unwrap()
}

fn edge_between(m: &Model, body: BodyId, a: DVec3, b: DVec3) -> EdgeId {
    let ctx = m.body(body).unwrap().geometry();
    ctx.edges()
        .find(|(_, e)| {
            let p = ctx.vertex(e.ends[0]).unwrap().position;
            let q = ctx.vertex(e.ends[1]).unwrap().position;
            (p.distance(a) < 1e-9 && q.distance(b) < 1e-9)
                || (p.distance(b) < 1e-9 && q.distance(a) < 1e-9)
        })
        .map(|(id, _)| id)
        .unwrap()
}

/// Root with instance `a` of the square definition `p` at x + 10.
fn one_instance() -> (Model, DefinitionId, InstanceId) {
    let mut m = Model::new();
    let p = square_def(&mut m);
    let a = place(&mut m, BodyId::Root, p, at(10.0, 0.0, 0.0));
    (m, p, a)
}

/// Root holds `a` (definition `p`) at x + 10. `p` holds a 4 m square (0,0)-(4,4) and
/// instance `b` of `q` rotated a quarter turn about z and moved by (-2, 0, 0) inside `p`
/// (it shows at p's x -4..-2, y 0..2, world x 6..8). `q` holds the 2 m square.
struct Nested {
    m: Model,
    p: DefinitionId,
    q: DefinitionId,
    a: InstanceId,
    b: InstanceId,
}

fn nested() -> Nested {
    let mut m = Model::new();
    let q = square_def(&mut m);
    let p = define(&mut m, "outer");
    polygon(&mut m, BodyId::Definition(p), &square(0.0, 0.0, 0.0, 4.0));
    let b_xform = at(-2.0, 0.0, 0.0) * DMat4::from_quat(DQuat::from_rotation_z(90f64.to_radians()));
    let b = place(&mut m, BodyId::Definition(p), q, b_xform);
    let a = place(&mut m, BodyId::Root, p, at(10.0, 0.0, 0.0));
    assert_eq!(m.validate(), vec![]);
    Nested { m, p, q, a, b }
}

fn path(steps: &[InstanceId]) -> EditPath {
    EditPath {
        steps: steps.to_vec(),
    }
}

fn mat_close(a: DMat4, b: DMat4) {
    let (a, b) = (a.to_cols_array(), b.to_cols_array());
    for i in 0..16 {
        assert!((a[i] - b[i]).abs() < 1e-12, "{a:?} != {b:?}");
    }
}

// -- EditPath --------------------------------------------------------------------------

#[test]
fn root_path() {
    let (m, _, _) = one_instance();
    let root = EditPath::root();
    assert!(root.is_root());
    assert_eq!(root, EditPath::default());
    assert_eq!(root.body(&m), Some(BodyId::Root));
    assert_eq!(root.transform(&m), Some(DMat4::IDENTITY));
    assert_eq!(root.close(), EditPath::root());
    assert_eq!(root.valid_prefix(&m), EditPath::root());
}

#[test]
fn open_one_instance() {
    let (m, p, a) = one_instance();
    let opened = EditPath::root().open(&m, a).unwrap();
    assert_eq!(opened, path(&[a]));
    assert!(!opened.is_root());
    assert_eq!(opened.body(&m), Some(BodyId::Definition(p)));
    mat_close(opened.transform(&m).unwrap(), at(10.0, 0.0, 0.0));
    assert_eq!(opened.close(), EditPath::root());
    assert_eq!(opened.valid_prefix(&m), opened);
}

#[test]
fn open_nested_instance() {
    let n = nested();
    let outer = EditPath::root().open(&n.m, n.a).unwrap();
    let inner = outer.open(&n.m, n.b).unwrap();
    assert_eq!(inner, path(&[n.a, n.b]));
    assert_eq!(inner.body(&n.m), Some(BodyId::Definition(n.q)));
    let expected = at(10.0, 0.0, 0.0)
        * at(-2.0, 0.0, 0.0)
        * DMat4::from_quat(DQuat::from_rotation_z(90f64.to_radians()));
    let t = inner.transform(&n.m).unwrap();
    mat_close(t, expected);
    // The square's corner (2, 0, 0) of q shows at world (8, 2, 0).
    assert_close(t.transform_point3(v(2.0, 0.0, 0.0)), v(8.0, 2.0, 0.0));
    assert_eq!(inner.close(), outer);
    assert_eq!(inner.close().close(), EditPath::root());
}

#[test]
fn open_refuses_instances_of_other_bodies() {
    let n = nested();
    // b lives in p, not in the root.
    assert_eq!(EditPath::root().open(&n.m, n.b), None);
    // a lives in the root, not in p.
    let outer = path(&[n.a]);
    assert_eq!(outer.open(&n.m, n.a), None);
    assert_eq!(outer.open(&n.m, InstanceId(999)), None);
    // A path that doesn't resolve opens nothing.
    assert_eq!(path(&[n.b]).open(&n.m, n.b), None);
}

#[test]
fn broken_paths_resolve_to_nothing() {
    let n = nested();
    for broken in [
        path(&[n.b]),
        path(&[n.a, n.a]),
        path(&[InstanceId(999)]),
        path(&[n.b, n.a]),
    ] {
        assert_eq!(broken.body(&n.m), None, "{broken:?}");
        assert_eq!(broken.transform(&n.m), None, "{broken:?}");
    }
    assert_eq!(path(&[n.b]).valid_prefix(&n.m), EditPath::root());
    assert_eq!(path(&[n.a, n.a]).valid_prefix(&n.m), path(&[n.a]));
    assert_eq!(path(&[n.a, n.b, n.b]).valid_prefix(&n.m), path(&[n.a, n.b]));
}

#[test]
fn valid_prefix_after_removals() {
    let n = nested();
    let mut m = n.m.clone();
    let full = path(&[n.a, n.b]);
    let mut log = ModelEdit::new();
    m.remove_instance(BodyId::Definition(n.p), n.b, &mut log)
        .unwrap();
    assert_eq!(full.body(&m), None);
    assert_eq!(full.valid_prefix(&m), path(&[n.a]));

    let mut m = n.m.clone();
    m.remove_instance(BodyId::Root, n.a, &mut log).unwrap();
    assert_eq!(full.valid_prefix(&m), EditPath::root());
    assert_eq!(path(&[n.a]).valid_prefix(&m), EditPath::root());
}

// -- pick_in ---------------------------------------------------------------------------

/// Root: a 2 m square at x 0..2, instance `a` of the square definition at x + 10, and a
/// second instance `c` of it at x + 20.
fn scene() -> (Model, DefinitionId, InstanceId, InstanceId) {
    let (mut m, p, a) = one_instance();
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 0.0, 2.0));
    let c = place(&mut m, BodyId::Root, p, at(20.0, 0.0, 0.0));
    (m, p, a, c)
}

#[test]
fn pick_in_root_path_matches_pick_model() {
    let (m, _, _, _) = scene();
    let n = nested();
    for model in [&m, &n.m] {
        for (x, y) in [
            (1.0, 1.0),
            (0.1, 1.0),
            (11.0, 1.0),
            (10.2, 1.0),
            (12.0, 0.6),
            (21.0, 1.5),
            (11.5, 0.5),
            (13.9, 3.9),
            (50.0, 50.0),
        ] {
            let expected = world(x, y, |pr| pick_model(model, pr));
            for rest in [false, true] {
                let got = world(x, y, |pr| pick_in(model, &EditPath::root(), rest, pr));
                assert_eq!(got, expected, "({x}, {y}) rest {rest}");
            }
        }
    }
}

#[test]
fn pick_in_open_body_face_and_edge() {
    let (m, p, a, _) = scene();
    let body = BodyId::Definition(p);
    let t = at(10.0, 0.0, 0.0);
    let face = only_face(&m, body);
    let left = edge_between(&m, body, v(0.0, 0.0, 0.0), v(0.0, 2.0, 0.0));
    for rest in [false, true] {
        assert_eq!(
            local(t, 11.0, 1.0, |pr| pick_in(&m, &path(&[a]), rest, pr)),
            Some(Entity::Face(face))
        );
        // On the left edge, and within the snap radius of it from outside the square.
        assert_eq!(
            local(t, 10.0, 1.0, |pr| pick_in(&m, &path(&[a]), rest, pr)),
            Some(Entity::Edge(left))
        );
        assert_eq!(
            local(t, 9.8, 1.0, |pr| pick_in(&m, &path(&[a]), rest, pr)),
            Some(Entity::Edge(left))
        );
    }
}

#[test]
fn pick_in_ignores_or_stops_at_the_rest() {
    let (m, _, a, _) = scene();
    let t = at(10.0, 0.0, 0.0);
    // The root square, the other instance of the same definition, and empty space.
    for (x, y) in [(1.0, 1.0), (21.0, 1.0), (20.0, 1.0), (50.0, 50.0)] {
        for rest in [false, true] {
            assert_eq!(
                local(t, x, y, |pr| pick_in(&m, &path(&[a]), rest, pr)),
                None,
                "({x}, {y}) rest {rest}"
            );
        }
    }
}

#[test]
fn pick_in_rest_in_front_hides_the_open_body_only_when_shown() {
    let (mut m, p, a, _) = scene();
    // A small root square at z = 5 above the instance.
    polygon(&mut m, BodyId::Root, &square(10.5, 0.5, 5.0, 0.2));
    let face = only_face(&m, BodyId::Definition(p));
    let t = at(10.0, 0.0, 0.0);
    assert_eq!(
        local(t, 10.6, 0.6, |pr| pick_in(&m, &path(&[a]), true, pr)),
        None
    );
    assert_eq!(
        local(t, 10.6, 0.6, |pr| pick_in(&m, &path(&[a]), false, pr)),
        Some(Entity::Face(face))
    );
    // Beside the root square, the instance's face shows either way.
    assert_eq!(
        local(t, 11.3, 1.3, |pr| pick_in(&m, &path(&[a]), true, pr)),
        Some(Entity::Face(face))
    );
}

#[test]
fn pick_in_open_body_wins_ties_with_the_rest() {
    let (mut m, p, a, _) = scene();
    // A root square exactly where the instance's square shows.
    polygon(&mut m, BodyId::Root, &square(10.0, 0.0, 0.0, 2.0));
    let face = only_face(&m, BodyId::Definition(p));
    let t = at(10.0, 0.0, 0.0);
    assert_eq!(
        local(t, 11.0, 1.0, |pr| pick_in(&m, &path(&[a]), true, pr)),
        Some(Entity::Face(face))
    );
}

#[test]
fn pick_in_reports_nested_instances_of_the_open_body() {
    let n = nested();
    let t = at(10.0, 0.0, 0.0);
    let big = only_face(&n.m, BodyId::Definition(n.p));
    // Inside p's own square.
    assert_eq!(
        local(t, 13.0, 3.0, |pr| pick_in(&n.m, &path(&[n.a]), true, pr)),
        Some(Entity::Face(big))
    );
    // Over b: the instance of the open body.
    assert_eq!(
        local(t, 7.0, 1.0, |pr| pick_in(&n.m, &path(&[n.a]), false, pr)),
        Some(Entity::Instance(n.b))
    );
    // Inside b, opened: its own face.
    let inner = path(&[n.a, n.b]);
    let ti = inner.transform(&n.m).unwrap();
    let small = only_face(&n.m, BodyId::Definition(n.q));
    assert_eq!(
        local(ti, 7.0, 1.0, |pr| pick_in(&n.m, &inner, true, pr)),
        Some(Entity::Face(small))
    );
    // p's square outside b is the rest now.
    assert_eq!(
        local(ti, 13.0, 3.0, |pr| pick_in(&n.m, &inner, true, pr)),
        None
    );
    assert_eq!(
        local(ti, 13.0, 3.0, |pr| pick_in(&n.m, &inner, false, pr)),
        None
    );
}

#[test]
fn pick_in_rotated_and_scaled_instance() {
    let mut m = Model::new();
    let p = square_def(&mut m);
    // Quarter turn about z, doubled, at x + 10: local (0..2, 0..2) shows at x 6..10, y 0..4.
    let t = at(10.0, 0.0, 0.0)
        * DMat4::from_quat(DQuat::from_rotation_z(90f64.to_radians()))
        * DMat4::from_scale(DVec3::splat(2.0));
    let a = place(&mut m, BodyId::Root, p, t);
    let body = BodyId::Definition(p);
    let face = only_face(&m, body);
    // Local edge (0,0)-(2,0) shows from world (10,0) to (10,4).
    let edge = edge_between(&m, body, v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0));
    let open = path(&[a]);
    assert_eq!(
        local(t, 8.0, 2.0, |pr| pick_in(&m, &open, true, pr)),
        Some(Entity::Face(face))
    );
    assert_eq!(
        local(t, 10.3, 2.0, |pr| pick_in(&m, &open, true, pr)),
        Some(Entity::Edge(edge))
    );
    assert_eq!(local(t, 10.7, 2.0, |pr| pick_in(&m, &open, true, pr)), None);
}

#[test]
fn pick_in_refuses_broken_paths_and_flat_transforms() {
    let (mut m, p, a, _) = scene();
    assert_eq!(
        world(11.0, 1.0, |pr| pick_in(
            &m,
            &path(&[InstanceId(999)]),
            true,
            pr
        )),
        None
    );
    let flat = place(&mut m, BodyId::Root, p, DMat4::from_scale(v(1.0, 1.0, 0.0)));
    assert_eq!(
        world(1.0, 1.0, |pr| pick_in(&m, &path(&[flat]), false, pr)),
        None
    );
    assert_eq!(
        world(1.0, 1.0, |pr| infer_in(&m, &path(&[flat]), false, pr)),
        None
    );
    // The good path still works.
    let t = at(10.0, 0.0, 0.0);
    assert!(local(t, 11.0, 1.0, |pr| pick_in(&m, &path(&[a]), false, pr)).is_some());
}

// -- infer_in --------------------------------------------------------------------------

#[test]
fn infer_in_root_path_matches_infer_model() {
    let (m, _, _, _) = scene();
    let n = nested();
    for model in [&m, &n.m] {
        for (x, y) in [
            (1.0, 1.0),
            (0.1, 0.1),
            (2.1, 1.0),
            (11.0, 1.0),
            (12.1, 2.1),
            (11.0, 0.1),
            (21.0, 1.9),
            (9.1, 0.9),
            (50.0, 50.0),
        ] {
            let expected = world(x, y, |pr| infer_model(model, pr));
            for rest in [false, true] {
                let got = world(x, y, |pr| infer_in(model, &EditPath::root(), rest, pr));
                assert_eq!(got, expected, "({x}, {y}) rest {rest}");
            }
        }
    }
}

#[test]
fn infer_in_snaps_to_the_open_body_in_its_coordinates() {
    let (m, p, a, _) = scene();
    let body = BodyId::Definition(p);
    let t = at(10.0, 0.0, 0.0);
    for rest in [false, true] {
        let hit = local(t, 12.1, 2.1, |pr| infer_in(&m, &path(&[a]), rest, pr)).unwrap();
        assert_eq!(
            hit.snap,
            Snap::Endpoint(vertex_at(&m, body, v(2.0, 2.0, 0.0)))
        );
        assert_close(hit.point, v(2.0, 2.0, 0.0));

        let mid = local(t, 11.0, 0.1, |pr| infer_in(&m, &path(&[a]), rest, pr)).unwrap();
        let bottom = edge_between(&m, body, v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0));
        assert_eq!(mid.snap, Snap::Midpoint(bottom));
        assert_close(mid.point, v(1.0, 0.0, 0.0));

        let on = local(t, 11.3, 1.2, |pr| infer_in(&m, &path(&[a]), rest, pr)).unwrap();
        assert_eq!(on.snap, Snap::OnFace(only_face(&m, body)));
        assert_close(on.point, v(1.3, 1.2, 0.0));
    }
}

#[test]
fn infer_in_snaps_to_the_rest_only_when_shown() {
    let (m, _, a, _) = scene();
    let t = at(10.0, 0.0, 0.0);
    let open = path(&[a]);
    // The root square's corner (2, 2, 0) and the other instance's corner (20, 0, 0).
    for (world_point, cursor) in [
        (v(2.0, 2.0, 0.0), (2.1, 2.1)),
        (v(20.0, 0.0, 0.0), (19.9, -0.1)),
    ] {
        let shown = local(t, cursor.0, cursor.1, |pr| infer_in(&m, &open, true, pr)).unwrap();
        assert!(matches!(shown.snap, Snap::Endpoint(_)), "{shown:?}");
        assert_close(shown.point, world_point - v(10.0, 0.0, 0.0));

        let hidden = local(t, cursor.0, cursor.1, |pr| infer_in(&m, &open, false, pr)).unwrap();
        assert_eq!(hidden.snap, Snap::Ground);
        assert_close(hidden.point, v(cursor.0 - 10.0, cursor.1, 0.0));
    }
}

#[test]
fn infer_in_snaps_inside_the_open_body_instances() {
    let n = nested();
    let t = at(10.0, 0.0, 0.0);
    // b's corner: q's (2, 2, 0) shows at world (6, 2, 0), local to p (-4, 2, 0).
    for rest in [false, true] {
        let hit = local(t, 5.9, 2.1, |pr| infer_in(&n.m, &path(&[n.a]), rest, pr)).unwrap();
        assert!(matches!(hit.snap, Snap::Endpoint(_)), "{hit:?}");
        assert_close(hit.point, v(-4.0, 2.0, 0.0));
    }
}

#[test]
fn infer_in_nested_path_maps_the_rest_into_local_coordinates() {
    let n = nested();
    let inner = path(&[n.a, n.b]);
    let t = inner.transform(&n.m).unwrap();
    let body = BodyId::Definition(n.q);
    // Own corner: q's (2, 2, 0) at world (6, 2, 0).
    let own = local(t, 6.0, 2.1, |pr| infer_in(&n.m, &inner, true, pr)).unwrap();
    assert_eq!(
        own.snap,
        Snap::Endpoint(vertex_at(&n.m, body, v(2.0, 2.0, 0.0)))
    );
    assert_close(own.point, v(2.0, 2.0, 0.0));
    // p's far corner (4, 4, 0) shows at world (14, 4, 0): in q's coordinates (4, -6, 0).
    let far = local(t, 14.1, 4.1, |pr| infer_in(&n.m, &inner, true, pr)).unwrap();
    assert!(matches!(far.snap, Snap::Endpoint(_)), "{far:?}");
    assert_close(far.point, v(4.0, -6.0, 0.0));
    assert_close(t.transform_point3(far.point), v(14.0, 4.0, 0.0));
    let ground = local(t, 14.1, 4.1, |pr| infer_in(&n.m, &inner, false, pr)).unwrap();
    assert_eq!(ground.snap, Snap::Ground);
}

#[test]
fn infer_in_rest_hides_points_only_when_shown() {
    let (mut m, p, a, _) = scene();
    // A root square at z = 5 above the instance's corner (12, 2).
    polygon(&mut m, BodyId::Root, &square(11.0, 1.0, 5.0, 2.0));
    let t = at(10.0, 0.0, 0.0);
    let corner = vertex_at(&m, BodyId::Definition(p), v(2.0, 2.0, 0.0));
    let hidden = local(t, 12.0, 2.0, |pr| infer_in(&m, &path(&[a]), true, pr)).unwrap();
    assert_ne!(hidden.snap, Snap::Endpoint(corner));
    let seen = local(t, 12.0, 2.0, |pr| infer_in(&m, &path(&[a]), false, pr)).unwrap();
    assert_eq!(seen.snap, Snap::Endpoint(corner));
}

#[test]
fn infer_in_open_body_wins_ties_with_the_rest() {
    let (mut m, p, a, _) = scene();
    polygon(&mut m, BodyId::Root, &square(10.0, 0.0, 0.0, 2.0));
    let t = at(10.0, 0.0, 0.0);
    let corner = vertex_at(&m, BodyId::Definition(p), v(2.0, 2.0, 0.0));
    let hit = local(t, 12.1, 2.1, |pr| infer_in(&m, &path(&[a]), true, pr)).unwrap();
    assert_eq!(hit.snap, Snap::Endpoint(corner));
}

#[test]
fn infer_in_scaled_instance_keeps_the_pixel_radius() {
    let mut m = Model::new();
    let p = square_def(&mut m);
    let t = at(10.0, 0.0, 0.0) * DMat4::from_scale(DVec3::splat(2.0));
    let a = place(&mut m, BodyId::Root, p, t);
    let corner = vertex_at(&m, BodyId::Definition(p), v(2.0, 2.0, 0.0));
    // Corner shows at world (14, 4). 0.4 m off is 4 px: snaps; 0.7 m off is 7 px: doesn't.
    let near = local(t, 14.4, 4.0, |pr| infer_in(&m, &path(&[a]), false, pr)).unwrap();
    assert_eq!(near.snap, Snap::Endpoint(corner));
    assert_close(near.point, v(2.0, 2.0, 0.0));
    let far = local(t, 14.7, 4.0, |pr| infer_in(&m, &path(&[a]), false, pr)).unwrap();
    assert_ne!(far.snap, Snap::Endpoint(corner));
}

#[test]
fn infer_in_follows_the_model_after_edits() {
    let (mut m, p, a, _) = scene();
    let t = at(10.0, 0.0, 0.0);
    let open = path(&[a]);
    let before = local(t, 13.1, 3.1, |pr| infer_in(&m, &open, false, pr)).unwrap();
    assert_eq!(before.snap, Snap::Ground);
    polygon(&mut m, BodyId::Definition(p), &square(2.0, 2.0, 0.0, 1.0));
    let after = local(t, 13.1, 3.1, |pr| infer_in(&m, &open, false, pr)).unwrap();
    assert!(matches!(after.snap, Snap::Endpoint(_)), "{after:?}");
    assert_close(after.point, v(3.0, 3.0, 0.0));
}

// -- box selection in the open body ---------------------------------------------------

#[test]
fn box_select_in_the_open_body() {
    let (m, p, a, _) = scene();
    let open = path(&[a]);
    let body = open.body(&m).unwrap();
    assert_eq!(body, BodyId::Definition(p));
    let t = open.transform(&m).unwrap();
    let project = move |q: DVec3| top_project(t.transform_point3(q));
    let got = box_select(
        &m,
        body,
        &project,
        DVec2::new(95.0, -5.0),
        DVec2::new(125.0, 25.0),
        BoxMode::Window,
    );
    let ctx = m.body(body).unwrap().geometry();
    let mut expected: Vec<Entity> = ctx.edges().map(|(id, _)| Entity::Edge(id)).collect();
    expected.extend(ctx.faces().map(|(id, _)| Entity::Face(id)));
    expected.sort();
    assert_eq!(got, expected);
}
