// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Box and connected selection (`docs/specs/m2-select-box.md`).

use glam::{DMat4, DVec2, DVec3};
use modeler_kernel::model::*;
use modeler_kernel::select::*;
use modeler_kernel::topology::*;

fn p(x: f64, y: f64) -> DVec3 {
    DVec3::new(x, y, 0.0)
}

fn s(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

/// Looking straight down: one pixel per meter, screen x = world x, screen y = world y.
fn top(q: DVec3) -> Option<DVec2> {
    Some(DVec2::new(q.x, q.y))
}

/// Draw the closed polygon `pts` into `body` with sticky edges.
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

fn segment(m: &mut Model, body: BodyId, a: DVec3, b: DVec3) {
    let mut log = ModelEdit::new();
    m.edit_geometry(body, &mut log, |ctx, edit| {
        ctx.add_edge(a, b, edit).unwrap();
    })
    .unwrap();
}

fn square(x0: f64, y0: f64, size: f64) -> Vec<DVec3> {
    vec![
        p(x0, y0),
        p(x0 + size, y0),
        p(x0 + size, y0 + size),
        p(x0, y0 + size),
    ]
}

fn root(m: &Model) -> &Context {
    m.root().geometry()
}

fn edge(ctx: &Context, a: DVec3, b: DVec3) -> Entity {
    let va = ctx.find_vertex(a, 1e-9).expect("vertex a");
    let vb = ctx.find_vertex(b, 1e-9).expect("vertex b");
    Entity::Edge(ctx.edge_between(va, vb).expect("edge"))
}

/// The face whose outer loop has exactly the vertices at `pts` (any order).
fn face(ctx: &Context, pts: &[DVec3]) -> Entity {
    let mut want: Vec<VertexId> = pts
        .iter()
        .map(|q| ctx.find_vertex(*q, 1e-9).expect("vertex"))
        .collect();
    want.sort();
    for (id, f) in ctx.faces() {
        let mut have = ctx.loop_vertices(&f.outer);
        have.sort();
        if have == want {
            return Entity::Face(id);
        }
    }
    panic!("no face with outer loop {pts:?}");
}

fn all_edges(ctx: &Context) -> Vec<Entity> {
    ctx.edges().map(|(id, _)| Entity::Edge(id)).collect()
}

fn sorted(mut v: Vec<Entity>) -> Vec<Entity> {
    v.sort();
    v.dedup();
    v
}

fn select(m: &Model, a: DVec2, b: DVec2, mode: BoxMode) -> Vec<Entity> {
    box_select(m, BodyId::Root, &top, a, b, mode)
}

/// A model with one 2 m square face at the origin.
fn square_model() -> Model {
    let mut m = Model::new();
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 2.0));
    assert_eq!(root(&m).face_count(), 1);
    m
}

fn instance(definition: DefinitionId, offset: DVec3) -> Instance {
    Instance {
        definition,
        transform: DMat4::from_translation(offset),
        name: String::new(),
        material: None,
    }
}

fn definition(m: &mut Model, name: &str) -> DefinitionId {
    let mut log = ModelEdit::new();
    m.add_definition(
        DefinitionInfo {
            name: name.to_string(),
            group: false,
        },
        &mut log,
    )
}

fn place(m: &mut Model, body: BodyId, definition: DefinitionId, offset: DVec3) -> InstanceId {
    let mut log = ModelEdit::new();
    m.add_instance(body, instance(definition, offset), &mut log)
        .unwrap()
}

// -- BoxMode::from_drag ---------------------------------------------------------------------

#[test]
fn drag_direction_picks_the_mode() {
    assert_eq!(
        BoxMode::from_drag(s(0.0, 0.0), s(5.0, 5.0)),
        BoxMode::Window
    );
    assert_eq!(
        BoxMode::from_drag(s(0.0, 5.0), s(5.0, 0.0)),
        BoxMode::Window
    );
    assert_eq!(
        BoxMode::from_drag(s(5.0, 0.0), s(0.0, 5.0)),
        BoxMode::Crossing
    );
    assert_eq!(
        BoxMode::from_drag(s(5.0, 5.0), s(0.0, 0.0)),
        BoxMode::Crossing
    );
    assert_eq!(
        BoxMode::from_drag(s(3.0, 0.0), s(3.0, 9.0)),
        BoxMode::Window
    );
    assert_eq!(
        BoxMode::from_drag(s(f64::NAN, 0.0), s(3.0, 9.0)),
        BoxMode::Window
    );
    assert_eq!(
        BoxMode::from_drag(s(3.0, 0.0), s(f64::NAN, 9.0)),
        BoxMode::Window
    );
}

// -- window ---------------------------------------------------------------------------------

#[test]
fn window_around_everything_selects_edges_and_face() {
    let m = square_model();
    let ctx = root(&m);
    let mut want = all_edges(ctx);
    want.push(face(ctx, &square(0.0, 0.0, 2.0)));
    let got = select(&m, s(-1.0, -1.0), s(3.0, 3.0), BoxMode::Window);
    assert_eq!(got, sorted(want));
    assert_eq!(got.len(), 5);
}

#[test]
fn window_boundary_is_inclusive() {
    let m = square_model();
    let got = select(&m, s(0.0, 0.0), s(2.0, 2.0), BoxMode::Window);
    assert_eq!(got.len(), 5);
}

#[test]
fn window_over_part_selects_only_whole_entities() {
    let m = square_model();
    let ctx = root(&m);
    let got = select(&m, s(-1.0, -1.0), s(1.0, 3.0), BoxMode::Window);
    assert_eq!(got, vec![edge(ctx, p(0.0, 0.0), p(0.0, 2.0))]);
}

#[test]
fn window_inside_a_face_selects_nothing() {
    let m = square_model();
    assert_eq!(
        select(&m, s(0.5, 0.5), s(1.5, 1.5), BoxMode::Window),
        vec![]
    );
}

#[test]
fn corners_in_any_order() {
    let m = square_model();
    let want = select(&m, s(-1.0, -1.0), s(1.0, 3.0), BoxMode::Window);
    assert_eq!(
        select(&m, s(1.0, 3.0), s(-1.0, -1.0), BoxMode::Window),
        want
    );
    assert_eq!(
        select(&m, s(-1.0, 3.0), s(1.0, -1.0), BoxMode::Window),
        want
    );
    assert_eq!(
        select(&m, s(1.0, -1.0), s(-1.0, 3.0), BoxMode::Window),
        want
    );
}

// -- crossing -------------------------------------------------------------------------------

#[test]
fn crossing_over_part_selects_touched_entities() {
    let m = square_model();
    let ctx = root(&m);
    let got = select(&m, s(-1.0, -1.0), s(1.0, 3.0), BoxMode::Crossing);
    let want = sorted(vec![
        edge(ctx, p(0.0, 0.0), p(0.0, 2.0)),
        edge(ctx, p(0.0, 0.0), p(2.0, 0.0)),
        edge(ctx, p(0.0, 2.0), p(2.0, 2.0)),
        face(ctx, &square(0.0, 0.0, 2.0)),
    ]);
    assert_eq!(got, want);
}

#[test]
fn crossing_inside_a_face_selects_the_face() {
    let m = square_model();
    let ctx = root(&m);
    assert_eq!(
        select(&m, s(0.5, 0.5), s(1.5, 1.5), BoxMode::Crossing),
        vec![face(ctx, &square(0.0, 0.0, 2.0))]
    );
}

#[test]
fn crossing_outside_selects_nothing() {
    let m = square_model();
    assert_eq!(
        select(&m, s(3.0, 3.0), s(4.0, 4.0), BoxMode::Crossing),
        vec![]
    );
    assert_eq!(
        select(&m, s(-2.0, 0.5), s(-0.5, 1.5), BoxMode::Crossing),
        vec![]
    );
}

#[test]
fn crossing_touching_an_edge_exactly_selects_it() {
    let m = square_model();
    let ctx = root(&m);
    // The box's left side lies on the square's right edge.
    let got = select(&m, s(2.0, 0.5), s(3.0, 1.5), BoxMode::Crossing);
    assert_eq!(
        got,
        sorted(vec![
            edge(ctx, p(2.0, 0.0), p(2.0, 2.0)),
            face(ctx, &square(0.0, 0.0, 2.0)),
        ])
    );
}

#[test]
fn crossing_selects_an_edge_passing_through() {
    let mut m = Model::new();
    segment(&mut m, BodyId::Root, p(0.0, 0.0), p(10.0, 0.0));
    let ctx = root(&m);
    let e = edge(ctx, p(0.0, 0.0), p(10.0, 0.0));
    assert_eq!(
        select(&m, s(4.0, -1.0), s(6.0, 1.0), BoxMode::Crossing),
        vec![e]
    );
    assert_eq!(
        select(&m, s(4.0, -1.0), s(6.0, 1.0), BoxMode::Window),
        vec![]
    );
    // A diagonal edge passing by the box's corner without touching it.
    let mut m = Model::new();
    segment(&mut m, BodyId::Root, p(0.0, 3.0), p(3.0, 0.0));
    assert_eq!(
        select(&m, s(0.0, 0.0), s(1.0, 1.0), BoxMode::Crossing),
        vec![]
    );
    assert_eq!(
        select(&m, s(0.0, 0.0), s(1.0, 2.5), BoxMode::Crossing).len(),
        1
    );
}

#[test]
fn zero_size_box() {
    let mut m = Model::new();
    segment(&mut m, BodyId::Root, p(0.0, 0.0), p(10.0, 0.0));
    // A point on the edge.
    assert_eq!(
        select(&m, s(5.0, 0.0), s(5.0, 0.0), BoxMode::Crossing).len(),
        1
    );
    // A vertical line across it.
    assert_eq!(
        select(&m, s(5.0, -1.0), s(5.0, 1.0), BoxMode::Crossing).len(),
        1
    );
    // A horizontal line covering it.
    assert_eq!(
        select(&m, s(-1.0, 0.0), s(11.0, 0.0), BoxMode::Window).len(),
        1
    );
    assert_eq!(
        select(&m, s(5.0, 1.0), s(5.0, 1.0), BoxMode::Crossing),
        vec![]
    );
}

#[test]
fn crossing_in_a_hole_skips_the_holed_face() {
    // A 6 m square with a 2 m square drawn inside: the outer face gets a hole and the inner
    // square becomes its own face.
    let mut m = Model::new();
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 6.0));
    polygon(&mut m, BodyId::Root, &square(2.0, 2.0, 2.0));
    let ctx = root(&m);
    assert_eq!(ctx.face_count(), 2);
    let inner = face(ctx, &square(2.0, 2.0, 2.0));
    let outer = face(ctx, &square(0.0, 0.0, 6.0));
    assert_eq!(
        select(&m, s(2.5, 2.5), s(3.5, 3.5), BoxMode::Crossing),
        vec![inner]
    );
    assert_eq!(
        select(&m, s(0.5, 0.5), s(1.5, 1.5), BoxMode::Crossing),
        vec![outer]
    );
    // A window around the inner square takes its face and edges, not the ring.
    let got = select(&m, s(1.5, 1.5), s(4.5, 4.5), BoxMode::Window);
    assert_eq!(got.len(), 5);
    assert!(got.contains(&inner));
    assert!(!got.contains(&outer));
    // The ring's window needs its outer loop and its hole inside.
    let got = select(&m, s(-1.0, -1.0), s(7.0, 7.0), BoxMode::Window);
    assert_eq!(got.len(), 10);
}

// -- visibility, bad input ------------------------------------------------------------------

#[test]
fn entities_with_unseen_points_are_never_selected() {
    let mut m = Model::new();
    segment(&mut m, BodyId::Root, p(0.0, 0.0), DVec3::new(0.0, 0.0, 2.0));
    segment(&mut m, BodyId::Root, p(5.0, 0.0), p(6.0, 0.0));
    let ctx = root(&m);
    let flat = edge(ctx, p(5.0, 0.0), p(6.0, 0.0));
    let near = |q: DVec3| (q.z < 1.0).then(|| DVec2::new(q.x, q.y));
    let not_finite = |q: DVec3| Some(DVec2::new(if q.z < 1.0 { q.x / 0.0 } else { q.x }, q.y));
    for mode in [BoxMode::Window, BoxMode::Crossing] {
        let got = box_select(&m, BodyId::Root, &near, s(-9.0, -9.0), s(9.0, 9.0), mode);
        assert_eq!(got, vec![flat], "{mode:?}");
        let got = box_select(
            &m,
            BodyId::Root,
            &not_finite,
            s(-9.0, -9.0),
            s(9.0, 9.0),
            mode,
        );
        assert_eq!(got, vec![], "{mode:?}");
    }
}

#[test]
fn bad_box_or_body_selects_nothing() {
    let m = square_model();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for mode in [BoxMode::Window, BoxMode::Crossing] {
            assert_eq!(select(&m, s(bad, -1.0), s(3.0, 3.0), mode), vec![]);
            assert_eq!(select(&m, s(-1.0, -1.0), s(3.0, bad), mode), vec![]);
        }
    }
    let missing = BodyId::Definition(DefinitionId(999));
    let got = box_select(
        &m,
        missing,
        &top,
        s(-9.0, -9.0),
        s(9.0, 9.0),
        BoxMode::Crossing,
    );
    assert_eq!(got, vec![]);
}

// -- instances ------------------------------------------------------------------------------

/// A definition holding one 1 m square face, placed once at x = 10.
fn instance_model() -> (Model, DefinitionId, InstanceId) {
    let mut m = Model::new();
    let d = definition(&mut m, "tile");
    polygon(&mut m, BodyId::Definition(d), &square(0.0, 0.0, 1.0));
    let i = place(&mut m, BodyId::Root, d, DVec3::new(10.0, 0.0, 0.0));
    (m, d, i)
}

#[test]
fn instances_are_selected_whole() {
    let (m, _, i) = instance_model();
    let want = vec![Entity::Instance(i)];
    assert_eq!(
        select(&m, s(9.0, -1.0), s(12.0, 2.0), BoxMode::Window),
        want
    );
    assert_eq!(
        select(&m, s(9.0, -1.0), s(10.5, 2.0), BoxMode::Window),
        vec![]
    );
    assert_eq!(
        select(&m, s(9.0, -1.0), s(10.5, 2.0), BoxMode::Crossing),
        want
    );
    // Inside its face.
    assert_eq!(
        select(&m, s(10.25, 0.25), s(10.75, 0.75), BoxMode::Crossing),
        want
    );
    // Where the definition's geometry is before the transform: nothing.
    assert_eq!(
        select(&m, s(-1.0, -1.0), s(2.0, 2.0), BoxMode::Crossing),
        vec![]
    );
}

#[test]
fn definition_body_selects_its_own_geometry() {
    let (m, d, _) = instance_model();
    let got = box_select(
        &m,
        BodyId::Definition(d),
        &top,
        s(-1.0, -1.0),
        s(2.0, 2.0),
        BoxMode::Window,
    );
    assert_eq!(got.len(), 5);
    assert!(got.iter().all(|e| !matches!(e, Entity::Instance(_))));
}

#[test]
fn nested_instances_compose_transforms() {
    let (mut m, tile, _) = instance_model();
    let row = definition(&mut m, "row");
    place(
        &mut m,
        BodyId::Definition(row),
        tile,
        DVec3::new(0.0, 5.0, 0.0),
    );
    let outer = place(&mut m, BodyId::Root, row, DVec3::new(20.0, 0.0, 0.0));
    // The tile inside the row inside the root sits at (20, 5) to (21, 6).
    assert_eq!(
        select(&m, s(19.5, 4.5), s(21.5, 6.5), BoxMode::Window),
        vec![Entity::Instance(outer)]
    );
    assert_eq!(
        select(&m, s(20.5, 5.5), s(20.6, 5.6), BoxMode::Crossing),
        vec![Entity::Instance(outer)]
    );
    assert_eq!(
        select(&m, s(19.5, -0.5), s(21.5, 1.5), BoxMode::Crossing),
        vec![]
    );
}

#[test]
fn rotated_instance() {
    let (mut m, tile, _) = instance_model();
    let mut log = ModelEdit::new();
    let turned = m
        .add_instance(
            BodyId::Root,
            Instance {
                definition: tile,
                transform: DMat4::from_translation(DVec3::new(0.0, 30.0, 0.0))
                    * DMat4::from_rotation_z(std::f64::consts::FRAC_PI_2),
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    // Turned a quarter around z, then moved: (-1, 30) to (0, 31).
    assert_eq!(
        select(&m, s(-1.1, 29.9), s(0.1, 31.1), BoxMode::Window),
        vec![Entity::Instance(turned)]
    );
    assert_eq!(
        select(&m, s(-0.1, 29.9), s(1.1, 31.1), BoxMode::Window),
        vec![]
    );
}

#[test]
fn empty_definition_instance_is_never_selected() {
    let mut m = Model::new();
    let empty = definition(&mut m, "empty");
    let holder = definition(&mut m, "holder");
    place(&mut m, BodyId::Definition(holder), empty, DVec3::ZERO);
    place(&mut m, BodyId::Root, empty, DVec3::ZERO);
    place(&mut m, BodyId::Root, holder, DVec3::ZERO);
    for mode in [BoxMode::Window, BoxMode::Crossing] {
        assert_eq!(select(&m, s(-9.0, -9.0), s(9.0, 9.0), mode), vec![]);
    }
}

#[test]
fn results_are_sorted_edges_faces_instances() {
    let (mut m, _, i) = instance_model();
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 2.0));
    let got = select(&m, s(-1.0, -1.0), s(12.0, 3.0), BoxMode::Window);
    assert_eq!(got.len(), 6);
    assert_eq!(got, sorted(got.clone()));
    assert_eq!(got[5], Entity::Instance(i));
    assert!(matches!(got[4], Entity::Face(_)));
}

// -- bounded (double click) -----------------------------------------------------------------

/// Two 2 m squares side by side sharing the edge x = 2, plus a separate segment far away.
fn two_squares() -> Model {
    let mut m = Model::new();
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 2.0));
    polygon(&mut m, BodyId::Root, &square(2.0, 0.0, 2.0));
    segment(&mut m, BodyId::Root, p(10.0, 10.0), p(11.0, 10.0));
    assert_eq!(root(&m).face_count(), 2);
    assert_eq!(root(&m).edge_count(), 8);
    m
}

#[test]
fn bounded_face_is_face_and_its_edges() {
    let m = two_squares();
    let ctx = root(&m);
    let left = face(ctx, &square(0.0, 0.0, 2.0));
    let want = sorted(vec![
        left,
        edge(ctx, p(0.0, 0.0), p(2.0, 0.0)),
        edge(ctx, p(2.0, 0.0), p(2.0, 2.0)),
        edge(ctx, p(2.0, 2.0), p(0.0, 2.0)),
        edge(ctx, p(0.0, 2.0), p(0.0, 0.0)),
    ]);
    assert_eq!(bounded(ctx, left), want);
}

#[test]
fn bounded_face_includes_hole_edges() {
    let mut m = Model::new();
    polygon(&mut m, BodyId::Root, &square(0.0, 0.0, 6.0));
    polygon(&mut m, BodyId::Root, &square(2.0, 2.0, 2.0));
    let ctx = root(&m);
    let ring = face(ctx, &square(0.0, 0.0, 6.0));
    let got = bounded(ctx, ring);
    assert_eq!(got.len(), 9);
    assert_eq!(got[8], ring);
}

#[test]
fn bounded_edge_is_edge_and_its_faces() {
    let m = two_squares();
    let ctx = root(&m);
    let shared = edge(ctx, p(2.0, 0.0), p(2.0, 2.0));
    let want = sorted(vec![
        shared,
        face(ctx, &square(0.0, 0.0, 2.0)),
        face(ctx, &square(2.0, 0.0, 2.0)),
    ]);
    assert_eq!(bounded(ctx, shared), want);
    let outside = edge(ctx, p(0.0, 0.0), p(0.0, 2.0));
    assert_eq!(
        bounded(ctx, outside),
        sorted(vec![outside, face(ctx, &square(0.0, 0.0, 2.0))])
    );
    let lone = edge(ctx, p(10.0, 10.0), p(11.0, 10.0));
    assert_eq!(bounded(ctx, lone), vec![lone]);
}

#[test]
fn bounded_instance_and_missing() {
    let m = two_squares();
    let ctx = root(&m);
    let i = Entity::Instance(InstanceId(7));
    assert_eq!(bounded(ctx, i), vec![i]);
    assert_eq!(bounded(ctx, Entity::Edge(EdgeId(999))), vec![]);
    assert_eq!(bounded(ctx, Entity::Face(FaceId(999))), vec![]);
}

// -- connected (triple click) ---------------------------------------------------------------

#[test]
fn connected_takes_everything_reachable() {
    let m = two_squares();
    let ctx = root(&m);
    let lone = edge(ctx, p(10.0, 10.0), p(11.0, 10.0));
    let mut want: Vec<Entity> = all_edges(ctx).into_iter().filter(|e| *e != lone).collect();
    want.push(face(ctx, &square(0.0, 0.0, 2.0)));
    want.push(face(ctx, &square(2.0, 0.0, 2.0)));
    let want = sorted(want);
    assert_eq!(want.len(), 9);
    assert_eq!(connected(ctx, face(ctx, &square(0.0, 0.0, 2.0))), want);
    assert_eq!(connected(ctx, edge(ctx, p(4.0, 0.0), p(4.0, 2.0))), want);
    assert_eq!(connected(ctx, lone), vec![lone]);
}

#[test]
fn connected_follows_vertices_not_just_faces() {
    // A square with a dangling segment off one corner, and a second segment off that.
    let mut m = square_model();
    segment(&mut m, BodyId::Root, p(2.0, 2.0), p(5.0, 5.0));
    segment(&mut m, BodyId::Root, p(5.0, 5.0), p(5.0, 9.0));
    let ctx = root(&m);
    let tail = edge(ctx, p(5.0, 5.0), p(5.0, 9.0));
    let got = connected(ctx, tail);
    assert_eq!(got.len(), 7);
    assert!(got.contains(&face(ctx, &square(0.0, 0.0, 2.0))));
}

#[test]
fn connected_instance_and_missing() {
    let m = two_squares();
    let ctx = root(&m);
    let i = Entity::Instance(InstanceId(3));
    assert_eq!(connected(ctx, i), vec![i]);
    assert_eq!(connected(ctx, Entity::Edge(EdgeId(999))), vec![]);
    assert_eq!(connected(ctx, Entity::Face(FaceId(999))), vec![]);
}
