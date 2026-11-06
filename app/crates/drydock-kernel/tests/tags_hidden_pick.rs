// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Hidden instances are left out of picking, snapping, the eyedropper and box selection
//! (`docs/specs/m10-tags-hidden-pick.md`).

use drydock_kernel::editpath::EditPath;
use drydock_kernel::infer::*;
use drydock_kernel::model::*;
use drydock_kernel::paint::create_material;
use drydock_kernel::select::{BoxMode, Entity, box_select, instance_segments};
use drydock_kernel::tags::TagId;
use drydock_kernel::topology::FaceId;
use glam::{DMat4, DVec2, DVec3};

const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn assert_close(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-9, "{a:?} != {b:?}");
}

/// Looking straight down, parallel: 10 pixels per meter, screen = 10 * (x, y).
fn top_project(q: DVec3) -> Option<DVec2> {
    Some(DVec2::new(q.x, q.y) * 10.0)
}

/// Probe for the top view with the cursor over point (x, y). Radius 5 px (0.5 m).
fn top(x: f64, y: f64) -> Probe<'static> {
    Probe {
        cursor: DVec2::new(x, y) * 10.0,
        origin: v(x, y, 100.0),
        dir: v(0.0, 0.0, -1.0),
        parallel: true,
        project: &top_project,
        radius: 5.0,
        anchor: None,
        axes: AXES,
        centers: &[],
        lock: None,
    }
}

fn s(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y) * 10.0
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

fn place_with(
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

fn place(m: &mut Model, body: BodyId, d: DefinitionId, transform: DMat4) -> InstanceId {
    place_with(m, body, d, transform, None)
}

fn at(x: f64, y: f64, z: f64) -> DMat4 {
    DMat4::from_translation(v(x, y, z))
}

/// A definition holding the 2 m square (0,0)-(2,2) at height `z`.
fn square_def(m: &mut Model, name: &str, z: f64) -> DefinitionId {
    let d = define(m, name);
    polygon(m, BodyId::Definition(d), &square(0.0, 0.0, z, 2.0));
    d
}

/// The 20 m ground square (0,0)-(20,20) at z = 0 in the root; returns its face.
fn ground(m: &mut Model) -> FaceId {
    polygon(m, BodyId::Root, &square(0.0, 0.0, 0.0, 20.0));
    let faces: Vec<FaceId> = m.root().geometry().faces().map(|(id, _)| id).collect();
    assert_eq!(faces.len(), 1);
    faces[0]
}

/// A new visible tag given to `instance`.
fn tag(m: &mut Model, name: &str, instance: InstanceId) -> TagId {
    let mut log = ModelEdit::new();
    let t = m.add_tag(name, &mut log).unwrap();
    m.assign_tag(instance, Some(t), &mut log).unwrap();
    t
}

fn set_visible(m: &mut Model, t: TagId, visible: bool) {
    let mut log = ModelEdit::new();
    m.set_tag_visible(t, visible, &mut log).unwrap();
}

fn hide(m: &mut Model, t: TagId) {
    set_visible(m, t, false);
}

fn show(m: &mut Model, t: TagId) {
    set_visible(m, t, true);
}

fn path(steps: &[InstanceId]) -> EditPath {
    EditPath {
        steps: steps.to_vec(),
    }
}

/// Ground, and one instance `a` of a 2 m square at height 1 covering (10,10)-(12,12).
struct Simple {
    m: Model,
    ground: FaceId,
    a: InstanceId,
}

fn simple() -> Simple {
    let mut m = Model::new();
    let ground = ground(&mut m);
    let d = square_def(&mut m, "part", 1.0);
    let a = place(&mut m, BodyId::Root, d, at(10.0, 10.0, 0.0));
    Simple { m, ground, a }
}

/// Ground and, all at height 1:
/// - `inner`: a definition holding a 2 m square (0,0)-(2,2);
/// - `outer`: a definition holding its own square (10,10)-(12,12) and `nested`, an instance
///   of `inner` moved to (15,10), so covering (15,10)-(17,12);
/// - `outer_a`: `outer` placed in the root unmoved (world = its coordinates);
/// - `outer_b`: `outer` placed at (-10,0): own square (0,10)-(2,12), nested (5,10)-(7,12);
/// - `side`: `inner` placed in the root at (3,3), covering (3,3)-(5,5).
struct Nested {
    m: Model,
    ground: FaceId,
    nested: InstanceId,
    outer_a: InstanceId,
    outer_b: InstanceId,
    side: InstanceId,
}

/// `nested()` with materials on `side` and `outer_b`, in a model that already holds them.
fn nested_with(
    mut m: Model,
    side_material: Option<MaterialId>,
    outer_b_material: Option<MaterialId>,
) -> Nested {
    let ground = ground(&mut m);
    let inner = square_def(&mut m, "inner", 1.0);
    let outer = define(&mut m, "outer");
    polygon(
        &mut m,
        BodyId::Definition(outer),
        &square(10.0, 10.0, 1.0, 2.0),
    );
    let nested = place(
        &mut m,
        BodyId::Definition(outer),
        inner,
        at(15.0, 10.0, 0.0),
    );
    let outer_a = place(&mut m, BodyId::Root, outer, DMat4::IDENTITY);
    let outer_b = place_with(
        &mut m,
        BodyId::Root,
        outer,
        at(-10.0, 0.0, 0.0),
        outer_b_material,
    );
    let side = place_with(
        &mut m,
        BodyId::Root,
        inner,
        at(3.0, 3.0, 0.0),
        side_material,
    );
    Nested {
        m,
        ground,
        nested,
        outer_a,
        outer_b,
        side,
    }
}

fn nested() -> Nested {
    nested_with(Model::new(), None, None)
}

// -- pick_model ----------------------------------------------------------------------

#[test]
fn a_hidden_instance_is_not_picked_and_the_face_behind_it_is() {
    let mut s = simple();
    let t = tag(&mut s.m, "Furniture", s.a);
    assert_eq!(
        pick_model(&s.m, &top(11.0, 11.0)),
        Some(Entity::Instance(s.a))
    );
    hide(&mut s.m, t);
    assert_eq!(
        pick_model(&s.m, &top(11.0, 11.0)),
        Some(Entity::Face(s.ground))
    );
    show(&mut s.m, t);
    assert_eq!(
        pick_model(&s.m, &top(11.0, 11.0)),
        Some(Entity::Instance(s.a))
    );
}

#[test]
fn a_hidden_instance_edge_is_not_picked() {
    let mut s = simple();
    let t = tag(&mut s.m, "Furniture", s.a);
    // Within the snap radius of the instance's left edge (x = 10), outside its face.
    assert_eq!(
        pick_model(&s.m, &top(9.8, 11.0)),
        Some(Entity::Instance(s.a))
    );
    hide(&mut s.m, t);
    assert_eq!(
        pick_model(&s.m, &top(9.8, 11.0)),
        Some(Entity::Face(s.ground))
    );
}

#[test]
fn a_visible_tag_changes_nothing() {
    let mut s = simple();
    let before = pick_model(&s.m, &top(11.0, 11.0));
    let t = tag(&mut s.m, "Furniture", s.a);
    assert_eq!(pick_model(&s.m, &top(11.0, 11.0)), before);
    hide(&mut s.m, t);
    show(&mut s.m, t);
    assert_eq!(pick_model(&s.m, &top(11.0, 11.0)), before);
}

#[test]
fn the_instance_behind_a_hidden_one_is_picked() {
    let mut m = Model::new();
    let ground = ground(&mut m);
    let high = square_def(&mut m, "high", 2.0);
    let low = square_def(&mut m, "low", 1.0);
    let a = place(&mut m, BodyId::Root, high, at(10.0, 10.0, 0.0));
    let b = place(&mut m, BodyId::Root, low, at(10.0, 10.0, 0.0));
    assert_eq!(pick_model(&m, &top(11.0, 11.0)), Some(Entity::Instance(a)));
    let t = tag(&mut m, "Upper", a);
    hide(&mut m, t);
    assert_eq!(pick_model(&m, &top(11.0, 11.0)), Some(Entity::Instance(b)));
    let tb = tag(&mut m, "Lower", b);
    hide(&mut m, tb);
    assert_eq!(pick_model(&m, &top(11.0, 11.0)), Some(Entity::Face(ground)));
}

#[test]
fn a_tag_hides_only_the_instance_that_carries_it() {
    let mut m = Model::new();
    ground(&mut m);
    let d = square_def(&mut m, "part", 1.0);
    let a = place(&mut m, BodyId::Root, d, at(10.0, 10.0, 0.0));
    let b = place(&mut m, BodyId::Root, d, at(2.0, 2.0, 0.0));
    let t = tag(&mut m, "Furniture", a);
    hide(&mut m, t);
    assert_eq!(pick_model(&m, &top(3.0, 3.0)), Some(Entity::Instance(b)));
    assert_ne!(pick_model(&m, &top(11.0, 11.0)), Some(Entity::Instance(a)));
}

#[test]
fn with_only_hidden_instances_and_no_root_geometry_nothing_is_picked() {
    let mut m = Model::new();
    let d = square_def(&mut m, "part", 1.0);
    let a = place(&mut m, BodyId::Root, d, at(10.0, 10.0, 0.0));
    let t = tag(&mut m, "Furniture", a);
    hide(&mut m, t);
    assert_eq!(pick_model(&m, &top(11.0, 11.0)), None);
    assert_eq!(pick_model(&m, &top(10.0, 11.0)), None);
    let inf = infer_model(&m, &top(10.1, 10.1)).unwrap();
    assert_eq!(inf.snap, Snap::Ground);
    assert_close(inf.point, v(10.1, 10.1, 0.0));
}

#[test]
fn with_only_hidden_instances_the_model_acts_like_its_root() {
    let mut s = simple();
    let t = tag(&mut s.m, "Furniture", s.a);
    hide(&mut s.m, t);
    let ctx = s.m.root().geometry();
    for (x, y) in [
        (11.0, 11.0),
        (10.1, 10.1),
        (0.1, 0.2),
        (9.8, 11.0),
        (25.0, 3.0),
    ] {
        assert_eq!(
            pick_model(&s.m, &top(x, y)),
            pick(ctx, &top(x, y)),
            "({x}, {y})"
        );
        assert_eq!(
            infer_model(&s.m, &top(x, y)),
            infer(ctx, &top(x, y)),
            "({x}, {y})"
        );
    }
}

#[test]
fn a_hidden_nested_instance_is_not_picked_inside_a_visible_one() {
    let mut n = nested();
    assert_eq!(
        pick_model(&n.m, &top(16.0, 11.0)),
        Some(Entity::Instance(n.outer_a))
    );
    assert_eq!(
        pick_model(&n.m, &top(6.0, 11.0)),
        Some(Entity::Instance(n.outer_b))
    );
    let t = tag(&mut n.m, "Inner", n.nested);
    hide(&mut n.m, t);
    // The nested instance lives in the definition, so both placements lose it.
    assert_eq!(
        pick_model(&n.m, &top(16.0, 11.0)),
        Some(Entity::Face(n.ground))
    );
    assert_eq!(
        pick_model(&n.m, &top(6.0, 11.0)),
        Some(Entity::Face(n.ground))
    );
    // The outer instances' own squares still pick them.
    assert_eq!(
        pick_model(&n.m, &top(11.0, 11.0)),
        Some(Entity::Instance(n.outer_a))
    );
    assert_eq!(
        pick_model(&n.m, &top(1.0, 11.0)),
        Some(Entity::Instance(n.outer_b))
    );
    // `side` places the same definition as `nested` and carries no tag.
    assert_eq!(
        pick_model(&n.m, &top(4.0, 4.0)),
        Some(Entity::Instance(n.side))
    );
}

#[test]
fn what_is_inside_a_hidden_instance_is_never_reached() {
    let mut n = nested();
    let shown = tag(&mut n.m, "Shown", n.nested);
    let hidden = tag(&mut n.m, "Hidden", n.outer_a);
    hide(&mut n.m, hidden);
    assert_eq!(
        pick_model(&n.m, &top(16.0, 11.0)),
        Some(Entity::Face(n.ground))
    );
    assert_eq!(
        pick_model(&n.m, &top(11.0, 11.0)),
        Some(Entity::Face(n.ground))
    );
    let inf = infer_model(&n.m, &top(15.1, 10.1)).unwrap();
    assert_eq!(inf.snap, Snap::OnFace(n.ground));
    // outer_b carries no tag, so it and its nested square stay.
    assert_eq!(
        pick_model(&n.m, &top(6.0, 11.0)),
        Some(Entity::Instance(n.outer_b))
    );
    let _ = shown;
}

// -- infer_model ---------------------------------------------------------------------

#[test]
fn a_hidden_instance_endpoint_does_not_snap() {
    let mut s = simple();
    let t = tag(&mut s.m, "Furniture", s.a);
    let inf = infer_model(&s.m, &top(10.1, 10.1)).unwrap();
    assert!(matches!(inf.snap, Snap::Endpoint(_)), "{inf:?}");
    assert_close(inf.point, v(10.0, 10.0, 1.0));

    hide(&mut s.m, t);
    let inf = infer_model(&s.m, &top(10.1, 10.1)).unwrap();
    assert_eq!(inf.snap, Snap::OnFace(s.ground));
    assert_close(inf.point, v(10.1, 10.1, 0.0));

    show(&mut s.m, t);
    let inf = infer_model(&s.m, &top(10.1, 10.1)).unwrap();
    assert!(matches!(inf.snap, Snap::Endpoint(_)), "{inf:?}");
}

#[test]
fn a_hidden_instance_midpoint_and_edge_do_not_snap() {
    let mut s = simple();
    let t = tag(&mut s.m, "Furniture", s.a);
    let mid = infer_model(&s.m, &top(11.0, 10.1)).unwrap();
    assert!(matches!(mid.snap, Snap::Midpoint(_)), "{mid:?}");
    let on = infer_model(&s.m, &top(11.5, 9.9)).unwrap();
    assert!(matches!(on.snap, Snap::OnEdge(_)), "{on:?}");

    hide(&mut s.m, t);
    let mid = infer_model(&s.m, &top(11.0, 10.1)).unwrap();
    assert_eq!(mid.snap, Snap::OnFace(s.ground));
    assert_close(mid.point, v(11.0, 10.1, 0.0));
    let on = infer_model(&s.m, &top(11.5, 9.9)).unwrap();
    assert_eq!(on.snap, Snap::OnFace(s.ground));
    assert_close(on.point, v(11.5, 9.9, 0.0));
}

#[test]
fn a_hidden_nested_instance_does_not_snap() {
    let mut n = nested();
    let inf = infer_model(&n.m, &top(5.1, 10.1)).unwrap();
    assert!(matches!(inf.snap, Snap::Endpoint(_)), "{inf:?}");
    assert_close(inf.point, v(5.0, 10.0, 1.0));
    let t = tag(&mut n.m, "Inner", n.nested);
    hide(&mut n.m, t);
    let inf = infer_model(&n.m, &top(5.1, 10.1)).unwrap();
    assert_eq!(inf.snap, Snap::OnFace(n.ground));
    assert_close(inf.point, v(5.1, 10.1, 0.0));
}

#[test]
fn the_direction_lock_ignores_hidden_points() {
    let mut s = simple();
    let t = tag(&mut s.m, "Furniture", s.a);
    let locked = |m: &Model| {
        let mut p = top(10.1, 10.1);
        p.anchor = Some(v(0.0, 10.0, 1.0));
        p.lock = Some(DVec3::X);
        infer_model(m, &p).unwrap()
    };
    let inf = locked(&s.m);
    assert!(matches!(inf.snap, Snap::Endpoint(_)), "{inf:?}");
    hide(&mut s.m, t);
    let inf = locked(&s.m);
    assert!(!matches!(inf.snap, Snap::Endpoint(_)), "{inf:?}");
}

// -- pick_in and infer_in --------------------------------------------------------------

#[test]
fn pick_in_the_root_skips_hidden_instances() {
    let mut s = simple();
    let t = tag(&mut s.m, "Furniture", s.a);
    hide(&mut s.m, t);
    for rest in [false, true] {
        assert_eq!(
            pick_in(&s.m, &EditPath::root(), rest, &top(11.0, 11.0)),
            Some(Entity::Face(s.ground))
        );
    }
}

#[test]
fn pick_in_an_open_instance_skips_its_hidden_instances() {
    let mut n = nested();
    let open = path(&[n.outer_a]);
    assert_eq!(
        pick_in(&n.m, &open, false, &top(16.0, 11.0)),
        Some(Entity::Instance(n.nested))
    );
    let t = tag(&mut n.m, "Inner", n.nested);
    hide(&mut n.m, t);
    for rest in [false, true] {
        assert_eq!(pick_in(&n.m, &open, rest, &top(16.0, 11.0)), None);
        // The open instance's own square is still there.
        assert!(matches!(
            pick_in(&n.m, &open, rest, &top(11.0, 11.0)),
            Some(Entity::Face(_))
        ));
    }
}

#[test]
fn infer_in_skips_hidden_instances_in_the_open_body() {
    let mut n = nested();
    let open = path(&[n.outer_a]);
    let inf = infer_in(&n.m, &open, false, &top(15.1, 10.1)).unwrap();
    assert!(matches!(inf.snap, Snap::Endpoint(_)), "{inf:?}");
    assert_close(inf.point, v(15.0, 10.0, 1.0));
    let t = tag(&mut n.m, "Inner", n.nested);
    hide(&mut n.m, t);
    let inf = infer_in(&n.m, &open, false, &top(15.1, 10.1)).unwrap();
    assert_eq!(inf.snap, Snap::Ground);
    assert_close(inf.point, v(15.1, 10.1, 0.0));
    let inf = infer_in(&n.m, &open, true, &top(15.1, 10.1)).unwrap();
    assert!(matches!(inf.snap, Snap::OnFace(_)), "{inf:?}");
    assert_close(inf.point, v(15.1, 10.1, 0.0));
}

#[test]
fn infer_in_skips_hidden_instances_in_the_rest_of_the_model() {
    let mut n = nested();
    let open = path(&[n.outer_a]);
    // `side`, a sibling of the open instance.
    let inf = infer_in(&n.m, &open, true, &top(3.1, 3.1)).unwrap();
    assert!(matches!(inf.snap, Snap::Endpoint(_)), "{inf:?}");
    assert_close(inf.point, v(3.0, 3.0, 1.0));
    // `nested` inside `outer_b`, deeper in the rest.
    let inf = infer_in(&n.m, &open, true, &top(5.1, 10.1)).unwrap();
    assert!(matches!(inf.snap, Snap::Endpoint(_)), "{inf:?}");
    assert_close(inf.point, v(5.0, 10.0, 1.0));

    let ts = tag(&mut n.m, "Side", n.side);
    hide(&mut n.m, ts);
    let inf = infer_in(&n.m, &open, true, &top(3.1, 3.1)).unwrap();
    assert!(matches!(inf.snap, Snap::OnFace(_)), "{inf:?}");
    assert_close(inf.point, v(3.1, 3.1, 0.0));

    let tn = tag(&mut n.m, "Inner", n.nested);
    hide(&mut n.m, tn);
    let inf = infer_in(&n.m, &open, true, &top(5.1, 10.1)).unwrap();
    assert!(matches!(inf.snap, Snap::OnFace(_)), "{inf:?}");
    assert_close(inf.point, v(5.1, 10.1, 0.0));

    show(&mut n.m, ts);
    show(&mut n.m, tn);
    let inf = infer_in(&n.m, &open, true, &top(3.1, 3.1)).unwrap();
    assert!(matches!(inf.snap, Snap::Endpoint(_)), "{inf:?}");
    let inf = infer_in(&n.m, &open, true, &top(5.1, 10.1)).unwrap();
    assert!(matches!(inf.snap, Snap::Endpoint(_)), "{inf:?}");
}

#[test]
fn infer_in_skips_a_hidden_outer_instance_in_the_rest() {
    let mut n = nested();
    let open = path(&[n.outer_a]);
    let t = tag(&mut n.m, "B", n.outer_b);
    hide(&mut n.m, t);
    for (x, y) in [(5.1, 10.1), (1.9, 10.1)] {
        let inf = infer_in(&n.m, &open, true, &top(x, y)).unwrap();
        assert!(matches!(inf.snap, Snap::OnFace(_)), "({x}, {y}): {inf:?}");
        assert_close(inf.point, v(x, y, 0.0));
    }
}

// -- material_in (the eyedropper) ------------------------------------------------------

fn red(m: &mut Model) -> MaterialId {
    let mut undo = UndoStack::new();
    create_material(
        m,
        Material::new("red", [1.0, 0.0, 0.0, 1.0]),
        &mut undo,
        "New",
    )
    .unwrap()
}

#[test]
fn the_eyedropper_sees_through_a_hidden_instance() {
    let mut m = Model::new();
    let red = red(&mut m);
    let mut n = nested_with(m, Some(red), None);
    let at_root = |m: &Model| material_in(m, &EditPath::root(), true, &top(4.0, 4.0));
    assert_eq!(at_root(&n.m), Some(Some(red)));
    let t = tag(&mut n.m, "Side", n.side);
    hide(&mut n.m, t);
    assert_eq!(at_root(&n.m), Some(None), "the unpainted ground");
    // From inside outer_a, `side` is in the rest of the model.
    let open = path(&[n.outer_a]);
    assert_eq!(material_in(&n.m, &open, true, &top(4.0, 4.0)), Some(None));
    show(&mut n.m, t);
    assert_eq!(
        material_in(&n.m, &open, true, &top(4.0, 4.0)),
        Some(Some(red))
    );
}

#[test]
fn the_eyedropper_sees_through_a_hidden_nested_instance() {
    let mut m = Model::new();
    let red = red(&mut m);
    // outer_b is painted; its unpainted nested square takes that material.
    let mut n = nested_with(m, None, Some(red));
    assert_eq!(
        material_in(&n.m, &EditPath::root(), true, &top(6.0, 11.0)),
        Some(Some(red))
    );
    let t = tag(&mut n.m, "Inner", n.nested);
    hide(&mut n.m, t);
    assert_eq!(
        material_in(&n.m, &EditPath::root(), true, &top(6.0, 11.0)),
        Some(None)
    );
    // Inside the open outer_a, the hidden nested square is skipped too.
    let open = path(&[n.outer_a]);
    assert_eq!(material_in(&n.m, &open, false, &top(16.0, 11.0)), None);
    assert_eq!(material_in(&n.m, &open, true, &top(16.0, 11.0)), Some(None));
}

// -- box_select ----------------------------------------------------------------------

fn select(m: &Model, a: DVec2, b: DVec2, mode: BoxMode) -> Vec<Entity> {
    box_select(m, BodyId::Root, &top_project, a, b, mode)
}

#[test]
fn a_window_box_skips_a_hidden_instance() {
    let mut n = nested();
    let (a, b) = (s(2.5, 2.5), s(5.5, 5.5));
    assert_eq!(
        select(&n.m, a, b, BoxMode::Window),
        vec![Entity::Instance(n.side)]
    );
    let t = tag(&mut n.m, "Side", n.side);
    hide(&mut n.m, t);
    assert_eq!(select(&n.m, a, b, BoxMode::Window), Vec::<Entity>::new());
    show(&mut n.m, t);
    assert_eq!(
        select(&n.m, a, b, BoxMode::Window),
        vec![Entity::Instance(n.side)]
    );
}

#[test]
fn a_crossing_box_skips_a_hidden_instance() {
    let mut n = nested();
    // Right edge of `side` (x = 5), away from any other instance.
    let (a, b) = (s(5.5, 3.5), s(4.5, 4.5));
    let got = select(&n.m, a, b, BoxMode::Crossing);
    assert_eq!(got, vec![Entity::Face(n.ground), Entity::Instance(n.side)]);
    let t = tag(&mut n.m, "Side", n.side);
    hide(&mut n.m, t);
    assert_eq!(
        select(&n.m, a, b, BoxMode::Crossing),
        vec![Entity::Face(n.ground)]
    );
}

#[test]
fn a_window_box_ignores_hidden_geometry_inside_an_instance() {
    let mut n = nested();
    // Around outer_b's own square only; its nested square sticks out.
    let (a, b) = (s(-0.5, 9.5), s(2.5, 12.5));
    assert_eq!(select(&n.m, a, b, BoxMode::Window), Vec::<Entity>::new());
    let t = tag(&mut n.m, "Inner", n.nested);
    hide(&mut n.m, t);
    assert_eq!(
        select(&n.m, a, b, BoxMode::Window),
        vec![Entity::Instance(n.outer_b)]
    );
}

#[test]
fn a_crossing_box_ignores_hidden_geometry_inside_an_instance() {
    let mut n = nested();
    // Over the left edge of outer_b's nested square (x = 5) only.
    let (a, b) = (s(5.5, 10.5), s(4.5, 11.5));
    assert!(select(&n.m, a, b, BoxMode::Crossing).contains(&Entity::Instance(n.outer_b)));
    let t = tag(&mut n.m, "Inner", n.nested);
    hide(&mut n.m, t);
    assert_eq!(
        select(&n.m, a, b, BoxMode::Crossing),
        vec![Entity::Face(n.ground)]
    );
}

#[test]
fn an_instance_with_only_hidden_contents_is_never_box_selected() {
    let mut m = Model::new();
    let inner = square_def(&mut m, "inner", 1.0);
    let wrapper = define(&mut m, "wrapper");
    let hidden = place(
        &mut m,
        BodyId::Definition(wrapper),
        inner,
        at(1.0, 1.0, 0.0),
    );
    let w = place(&mut m, BodyId::Root, wrapper, DMat4::IDENTITY);
    let (a, b) = (s(-5.0, -5.0), s(10.0, 10.0));
    for mode in [BoxMode::Window, BoxMode::Crossing] {
        assert_eq!(select(&m, a, b, mode), vec![Entity::Instance(w)]);
    }
    let t = tag(&mut m, "Inner", hidden);
    hide(&mut m, t);
    for mode in [BoxMode::Window, BoxMode::Crossing] {
        assert_eq!(select(&m, a, b, mode), Vec::<Entity>::new(), "{mode:?}");
    }
}

#[test]
fn box_select_in_a_definition_skips_its_hidden_instances() {
    let mut n = nested();
    let body = BodyId::Definition(n.m.root().instance(n.outer_a).unwrap().definition);
    let (a, b) = (s(14.5, 9.5), s(17.5, 12.5));
    let got = box_select(&n.m, body, &top_project, a, b, BoxMode::Window);
    assert_eq!(got, vec![Entity::Instance(n.nested)]);
    let t = tag(&mut n.m, "Inner", n.nested);
    hide(&mut n.m, t);
    let got = box_select(&n.m, body, &top_project, a, b, BoxMode::Window);
    assert_eq!(got, Vec::<Entity>::new());
}

// -- instance_segments (the selection outline) -----------------------------------------

#[test]
fn the_selection_outline_leaves_out_hidden_nested_instances() {
    let mut n = nested();
    assert_eq!(instance_segments(&n.m, BodyId::Root, n.outer_b).len(), 8);
    let t = tag(&mut n.m, "Inner", n.nested);
    hide(&mut n.m, t);
    let segs = instance_segments(&n.m, BodyId::Root, n.outer_b);
    assert_eq!(segs.len(), 4);
    for (a, b) in segs {
        for p in [a, b] {
            assert!(p.x <= 2.0 + 1e-9 && p.y >= 10.0 - 1e-9, "{p:?}");
        }
    }
    show(&mut n.m, t);
    assert_eq!(instance_segments(&n.m, BodyId::Root, n.outer_b).len(), 8);
}

#[test]
fn the_outline_of_a_hidden_instance_itself_is_still_given() {
    let mut n = nested();
    let t = tag(&mut n.m, "Side", n.side);
    hide(&mut n.m, t);
    assert_eq!(instance_segments(&n.m, BodyId::Root, n.side).len(), 4);
}
