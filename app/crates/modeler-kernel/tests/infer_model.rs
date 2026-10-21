// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Picking and snapping on geometry inside instances (`docs/specs/m2-pick-imported.md`).

use glam::{DMat4, DQuat, DVec2, DVec3};
use modeler_kernel::infer::*;
use modeler_kernel::model::*;
use modeler_kernel::select::{Entity, instance_segments};

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

/// Probe for the top view with the cursor over world point (x, y). Radius 5 px (0.5 m).
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

fn define(m: &mut Model) -> DefinitionId {
    let mut log = ModelEdit::new();
    m.add_definition(
        DefinitionInfo {
            name: "part".into(),
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

/// A 2 m square (0,0)-(2,2) at height `z` in a new definition.
fn square_def(m: &mut Model, z: f64) -> DefinitionId {
    let d = define(m);
    polygon(m, BodyId::Definition(d), &square(0.0, 0.0, z, 2.0));
    d
}

fn at(x: f64, y: f64, z: f64) -> DMat4 {
    DMat4::from_translation(v(x, y, z))
}

/// Root with one instance of a 2 m square, moved to x = 10.
fn one_instance() -> (Model, InstanceId) {
    let mut m = Model::new();
    let d = square_def(&mut m, 0.0);
    let i = place(&mut m, BodyId::Root, d, at(10.0, 0.0, 0.0));
    (m, i)
}

// -- pick_model ----------------------------------------------------------------------

#[test]
fn pick_instance_face() {
    let (m, i) = one_instance();
    assert_eq!(pick_model(&m, &top(11.0, 1.0)), Some(Entity::Instance(i)));
}

#[test]
fn pick_instance_edge() {
    let (m, i) = one_instance();
    // On the left edge, and within the radius of it.
    assert_eq!(pick_model(&m, &top(10.0, 1.0)), Some(Entity::Instance(i)));
    assert_eq!(pick_model(&m, &top(9.8, 1.0)), Some(Entity::Instance(i)));
}

#[test]
fn pick_nothing_beside_the_instance() {
    let (m, _) = one_instance();
    assert_eq!(pick_model(&m, &top(5.0, 1.0)), None);
    assert_eq!(pick_model(&m, &top(13.0, 1.0)), None);
}

#[test]
fn pick_root_entities_as_before() {
    let (mut m, i) = one_instance();
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 0.0, 2.0));
    let root = m.root().geometry();
    let (face, _) = root.faces().next().unwrap();
    assert_eq!(pick_model(&m, &top(1.0, 1.0)), Some(Entity::Face(face)));
    let hit = pick_model(&m, &top(0.0, 1.0));
    assert!(matches!(hit, Some(Entity::Edge(_))), "{hit:?}");
    assert_eq!(pick_model(&m, &top(11.0, 1.0)), Some(Entity::Instance(i)));
    // Same answer as `pick` on the root alone.
    for (x, y) in [(1.0, 1.0), (0.0, 1.0), (5.0, 5.0), (-3.0, 0.0)] {
        assert_eq!(
            pick_model(&Model::new(), &top(x, y)),
            None,
            "empty model at {x},{y}"
        );
        let p = top(x, y);
        let root_only = pick(m.root().geometry(), &p);
        if root_only.is_some() {
            assert_eq!(pick_model(&m, &p), root_only);
        }
    }
}

#[test]
fn pick_the_nearer_of_root_and_instance_faces() {
    // The instance's square is 1 m above the root square, over the same spot.
    let mut m = Model::new();
    let d = square_def(&mut m, 0.0);
    let i = place(&mut m, BodyId::Root, d, at(0.0, 0.0, 1.0));
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 0.0, 2.0));
    assert_eq!(pick_model(&m, &top(1.0, 1.0)), Some(Entity::Instance(i)));

    // Now the instance is 1 m below: the root face is in front.
    let mut m = Model::new();
    let d = square_def(&mut m, 0.0);
    place(&mut m, BodyId::Root, d, at(0.0, 0.0, -1.0));
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 0.0, 2.0));
    let (face, _) = m.root().geometry().faces().next().unwrap();
    assert_eq!(pick_model(&m, &top(1.0, 1.0)), Some(Entity::Face(face)));
}

#[test]
fn pick_nearer_of_two_instances() {
    let mut m = Model::new();
    let d = square_def(&mut m, 0.0);
    let low = place(&mut m, BodyId::Root, d, at(0.0, 0.0, 0.0));
    let high = place(&mut m, BodyId::Root, d, at(0.0, 0.0, 2.0));
    assert_ne!(low, high);
    assert_eq!(pick_model(&m, &top(1.0, 1.0)), Some(Entity::Instance(high)));
}

#[test]
fn pick_follows_scale_and_rotation() {
    // Scale 3, a quarter turn about Z, then a move: the square becomes 6 m wide at x 14-20.
    let mut m = Model::new();
    let d = square_def(&mut m, 0.0);
    let t = at(20.0, 0.0, 0.0)
        * DMat4::from_quat(DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2))
        * DMat4::from_scale(DVec3::splat(3.0));
    let i = place(&mut m, BodyId::Root, d, t);
    let inside = t.transform_point3(v(1.0, 1.0, 0.0));
    let outside = t.transform_point3(v(3.0, 1.0, 0.0));
    assert_eq!(
        pick_model(&m, &top(inside.x, inside.y)),
        Some(Entity::Instance(i))
    );
    assert_eq!(pick_model(&m, &top(outside.x, outside.y)), None);
    // The square's untransformed place is empty.
    assert_eq!(pick_model(&m, &top(1.0, 1.0)), None);
}

#[test]
fn pick_nested_instance_reports_the_root_instance() {
    // Root -> outer (moved to x = 10) -> inner (moved 5 m along x inside outer) -> square.
    let mut m = Model::new();
    let inner = square_def(&mut m, 0.0);
    let outer = define(&mut m);
    place(&mut m, BodyId::Definition(outer), inner, at(5.0, 0.0, 0.0));
    let i = place(&mut m, BodyId::Root, outer, at(10.0, 0.0, 0.0));
    assert_eq!(pick_model(&m, &top(16.0, 1.0)), Some(Entity::Instance(i)));
    assert_eq!(pick_model(&m, &top(11.0, 1.0)), None);
}

#[test]
fn pick_instance_edge_beats_nothing_but_not_a_nearer_face() {
    // An edge of the instance below a root face is hidden by it: the face is picked.
    let mut m = Model::new();
    let d = square_def(&mut m, 0.0);
    place(&mut m, BodyId::Root, d, at(0.0, 0.0, -1.0));
    polygon(&mut m, BodyId::Root, &square(-1.0, -1.0, 0.0, 4.0));
    let (face, _) = m.root().geometry().faces().next().unwrap();
    assert_eq!(pick_model(&m, &top(0.0, 1.0)), Some(Entity::Face(face)));
}

#[test]
fn pick_ignores_a_probe_that_is_not_finite() {
    let (m, _) = one_instance();
    let mut p = top(11.0, 1.0);
    p.origin = v(f64::NAN, 0.0, 0.0);
    assert_eq!(pick_model(&m, &p), None);
    assert!(infer_model(&m, &p).is_none());
}

// -- infer_model ---------------------------------------------------------------------

#[test]
fn infer_snaps_to_instance_endpoint() {
    let (m, _) = one_instance();
    let inf = infer_model(&m, &top(10.1, 0.1)).unwrap();
    assert!(matches!(inf.snap, Snap::Endpoint(_)), "{:?}", inf.snap);
    assert_close(inf.point, v(10.0, 0.0, 0.0));
}

#[test]
fn infer_snaps_to_instance_midpoint() {
    let (m, _) = one_instance();
    let inf = infer_model(&m, &top(11.1, 0.1)).unwrap();
    assert!(matches!(inf.snap, Snap::Midpoint(_)), "{:?}", inf.snap);
    assert_close(inf.point, v(11.0, 0.0, 0.0));
}

#[test]
fn infer_snaps_on_instance_edge() {
    let (m, _) = one_instance();
    // 4 px from the edge, more than the radius (5 px) from its midpoint and its end.
    let inf = infer_model(&m, &top(10.4, 0.5)).unwrap();
    assert!(matches!(inf.snap, Snap::OnEdge(_)), "{:?}", inf.snap);
    assert_close(inf.point, v(10.0, 0.5, 0.0));
}

#[test]
fn infer_on_instance_face_uses_its_height() {
    let mut m = Model::new();
    let d = square_def(&mut m, 0.0);
    place(&mut m, BodyId::Root, d, at(10.0, 0.0, 1.0));
    let inf = infer_model(&m, &top(11.3, 0.9)).unwrap();
    assert!(matches!(inf.snap, Snap::OnFace(_)), "{:?}", inf.snap);
    assert_close(inf.point, v(11.3, 0.9, 1.0));
}

#[test]
fn infer_snaps_in_nested_and_transformed_instances() {
    let mut m = Model::new();
    let inner = square_def(&mut m, 0.0);
    let outer = define(&mut m);
    place(&mut m, BodyId::Definition(outer), inner, at(5.0, 0.0, 0.0));
    let t = at(10.0, 0.0, 0.0) * DMat4::from_scale(DVec3::splat(2.0));
    place(&mut m, BodyId::Root, outer, t);
    // The inner square's corner (2, 2) -> (7, 2) -> scaled and moved: (24, 4).
    let inf = infer_model(&m, &top(24.1, 4.1)).unwrap();
    assert!(matches!(inf.snap, Snap::Endpoint(_)), "{:?}", inf.snap);
    assert_close(inf.point, v(24.0, 4.0, 0.0));
}

#[test]
fn infer_ignores_instance_points_hidden_behind_a_root_face() {
    // The instance's corner is under a big root face.
    let mut m = Model::new();
    let d = square_def(&mut m, 0.0);
    place(&mut m, BodyId::Root, d, at(0.0, 0.0, -1.0));
    polygon(&mut m, BodyId::Root, &square(-1.0, -1.0, 0.0, 4.0));
    let inf = infer_model(&m, &top(0.1, 0.1)).unwrap();
    assert!(matches!(inf.snap, Snap::OnFace(_)), "{:?}", inf.snap);
    assert_close(inf.point, v(0.1, 0.1, 0.0));
}

#[test]
fn infer_root_snaps_win_ties_and_match_infer() {
    let (mut m, _) = one_instance();
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 0.0, 2.0));
    for (x, y) in [(0.1, 0.1), (1.0, 0.1), (1.0, 1.0), (7.0, 7.0), (-4.0, 0.3)] {
        let p = top(x, y);
        let root_only = infer(m.root().geometry(), &p);
        let both = infer_model(&m, &p);
        assert_eq!(
            root_only.map(|i| (i.snap, i.point)),
            both.map(|i| (i.snap, i.point)),
            "at {x},{y}"
        );
    }
}

#[test]
fn infer_model_without_instances_is_infer() {
    let mut m = Model::new();
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 0.0, 2.0));
    for (x, y) in [(0.1, 0.1), (1.0, 1.0), (7.0, 7.0)] {
        let p = top(x, y);
        assert_eq!(infer_model(&m, &p), infer(m.root().geometry(), &p));
    }
}

// -- instance_segments ---------------------------------------------------------------

type Seg = (DVec3, DVec3);

/// Segments as sorted pairs of rounded points, for comparing as sets.
fn normalized(segs: &[Seg]) -> Vec<[[i64; 3]; 2]> {
    let r = |p: DVec3| [p.x, p.y, p.z].map(|c| (c * 1e6).round() as i64);
    let mut out: Vec<[[i64; 3]; 2]> = segs
        .iter()
        .map(|&(a, b)| {
            let (a, b) = (r(a), r(b));
            if a <= b { [a, b] } else { [b, a] }
        })
        .collect();
    out.sort();
    out
}

#[test]
fn instance_segments_are_the_transformed_edges() {
    let (m, i) = one_instance();
    let got = instance_segments(&m, BodyId::Root, i);
    let sq = square(10.0, 0.0, 0.0, 2.0);
    let want: Vec<Seg> = (0..4).map(|k| (sq[k], sq[(k + 1) % 4])).collect();
    assert_eq!(normalized(&got), normalized(&want));
}

#[test]
fn instance_segments_include_nested_instances() {
    let mut m = Model::new();
    let inner = square_def(&mut m, 0.0);
    let outer = define(&mut m);
    polygon(
        &mut m,
        BodyId::Definition(outer),
        &[v(0.0, 0.0, 5.0), v(1.0, 0.0, 5.0)],
    );
    place(&mut m, BodyId::Definition(outer), inner, at(5.0, 0.0, 0.0));
    let t = at(10.0, 0.0, 0.0) * DMat4::from_scale(DVec3::splat(2.0));
    let i = place(&mut m, BodyId::Root, outer, t);
    let got = instance_segments(&m, BodyId::Root, i);
    // Outer's own edge (a closed two-point "polygon" draws one edge twice: one edge) plus the
    // four edges of the inner square at (5,0)-(7,2), scaled by 2 and moved by 10.
    let sq = square(20.0, 0.0, 0.0, 4.0);
    let mut want: Vec<Seg> = (0..4).map(|k| (sq[k], sq[(k + 1) % 4])).collect();
    want.push((v(10.0, 0.0, 10.0), v(12.0, 0.0, 10.0)));
    assert_eq!(normalized(&got), normalized(&want));
}

#[test]
fn instance_segments_of_missing_things_are_empty() {
    let (m, i) = one_instance();
    assert!(instance_segments(&m, BodyId::Root, InstanceId(i.0 + 99)).is_empty());
    assert!(instance_segments(&m, BodyId::Definition(DefinitionId(99)), i).is_empty());
}
