// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Curves carried through groups, components, explode, copy/paste, import and push/pull caps
//! (`docs/specs/m7-curves-carry.md`).

use std::collections::BTreeSet;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

use glam::{DMat4, DQuat, DVec3};
use modeler_kernel::component::make_component;
use modeler_kernel::copy::{copy_selection, extract, import_component, insert};
use modeler_kernel::curve::{self, Curve};
use modeler_kernel::explode::explode;
use modeler_kernel::model::*;
use modeler_kernel::pushpull::push_pull;
use modeler_kernel::select::Entity;
use modeler_kernel::tools::*;
use modeler_kernel::topology::*;
use modeler_kernel::transform::transform;
use modeler_kernel::units::Unit;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

#[track_caller]
fn assert_close(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-9, "{a:?} != {b:?}");
}

#[track_caller]
fn assert_near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

/// Same circle and angles (vertices not compared).
#[track_caller]
fn assert_same_circle(a: &Curve, b: &Curve) {
    assert_close(a.center, b.center);
    assert_close(a.normal, b.normal);
    assert_close(a.x_axis, b.x_axis);
    assert_near(a.radius, b.radius);
    assert_near(a.start, b.start);
    assert_near(a.end, b.end);
}

/// `n` points of the circle (`center`, radius `r`, in the plane of `u`, `v`), counter-clockwise
/// about `u × v`, the first at angle `turn`.
fn ring(center: DVec3, u: DVec3, w: DVec3, r: f64, n: usize, turn: f64) -> Vec<DVec3> {
    (0..n)
        .map(|k| {
            let a = turn + TAU * k as f64 / n as f64;
            center + (u * a.cos() + w * a.sin()) * r
        })
        .collect()
}

/// `n + 1` points of the arc in the XY plane from `from` to `to` (radians).
fn arc_xy(center: DVec3, r: f64, from: f64, to: f64, n: usize) -> Vec<DVec3> {
    (0..=n)
        .map(|k| {
            let a = from + (to - from) * k as f64 / n as f64;
            center + v(a.cos(), a.sin(), 0.0) * r
        })
        .collect()
}

fn similarity(axis: DVec3, angle: f64, scale: f64, mirror: bool, t: DVec3) -> DMat4 {
    let flip = if mirror {
        DMat4::from_scale(v(-1.0, 1.0, 1.0))
    } else {
        DMat4::IDENTITY
    };
    DMat4::from_translation(t)
        * DMat4::from_quat(DQuat::from_axis_angle(axis.normalize(), angle))
        * flip
        * DMat4::from_scale(DVec3::splat(scale))
}

struct World {
    model: Model,
    undo: UndoStack,
}

impl World {
    fn new() -> Self {
        World {
            model: Model::new(),
            undo: UndoStack::new(),
        }
    }

    fn target(&mut self, body: BodyId) -> Target<'_> {
        Target {
            model: &mut self.model,
            body,
            undo: &mut self.undo,
            unit: Unit::Meter,
        }
    }

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    fn ctx_of(&self, body: BodyId) -> &Context {
        self.model.body(body).unwrap().geometry()
    }

    fn curves_of(&self, body: BodyId) -> Vec<(CurveId, Curve)> {
        self.ctx_of(body)
            .curves()
            .map(|(id, c)| (id, c.clone()))
            .collect()
    }

    fn curves(&self) -> Vec<(CurveId, Curve)> {
        self.curves_of(BodyId::Root)
    }

    /// Draw a closed curve through `points` in `body` (a circle also makes its face).
    fn draw_closed(&mut self, body: BodyId, points: &[DVec3]) -> CurveId {
        let (_, id) = draw_curve(&mut self.target(body), points, true, "Circle").unwrap();
        id.expect("the points make a curve")
    }

    fn draw_open(&mut self, body: BodyId, points: &[DVec3]) -> CurveId {
        let (_, id) = draw_curve(&mut self.target(body), points, false, "Arc").unwrap();
        id.expect("the points make a curve")
    }

    /// A 24-segment circle in the root's XY plane; returns its id and curve.
    fn circle(&mut self, center: DVec3, r: f64) -> (CurveId, Curve) {
        let id = self.draw_closed(BodyId::Root, &ring(center, DVec3::X, DVec3::Y, r, 24, 0.0));
        (id, self.ctx().curve(id).unwrap().clone())
    }

    fn line(&mut self, a: DVec3, b: DVec3) {
        draw_segments(&mut self.target(BodyId::Root), &[(a, b)], "Line").unwrap();
    }

    fn everything(&self, body: BodyId) -> Vec<Entity> {
        let c = self.ctx_of(body);
        c.edges()
            .map(|(e, _)| Entity::Edge(e))
            .chain(c.faces().map(|(f, _)| Entity::Face(f)))
            .chain(
                self.model
                    .body(body)
                    .unwrap()
                    .instances()
                    .map(|(i, _)| Entity::Instance(i)),
            )
            .collect()
    }

    fn make(&mut self, selection: &[Entity], group: bool) -> InstanceId {
        make_component(
            &mut self.model,
            BodyId::Root,
            selection,
            DefinitionInfo {
                name: String::new(),
                group,
            },
            &mut self.undo,
            if group {
                "Make Group"
            } else {
                "Make Component"
            },
        )
        .unwrap()
    }

    fn instance(&self, id: InstanceId) -> Instance {
        self.model.root().instance(id).unwrap().clone()
    }

    fn definition_of(&self, id: InstanceId) -> BodyId {
        BodyId::Definition(self.instance(id).definition)
    }

    fn explode(&mut self, id: InstanceId) -> Vec<Entity> {
        explode(&mut self.model, BodyId::Root, id, &mut self.undo, "Explode").unwrap()
    }

    fn set_transform(&mut self, id: InstanceId, m: DMat4) {
        let current = self.instance(id).transform;
        transform(
            &mut self.model,
            BodyId::Root,
            &[Entity::Instance(id)],
            m * current.inverse(),
            &mut self.undo,
            "Move",
        )
        .unwrap();
        let now = self.instance(id).transform;
        for (a, b) in now.to_cols_array().iter().zip(m.to_cols_array()) {
            assert!((a - b).abs() < 1e-12, "{now:?} != {m:?}");
        }
    }

    fn pull(&mut self, face: FaceId, distance: f64) -> modeler_kernel::pushpull::PushPulled {
        push_pull(
            &mut self.model,
            BodyId::Root,
            face,
            distance,
            &mut self.undo,
            "Push/Pull",
        )
        .unwrap()
    }

    fn position(&self, body: BodyId, vertex: VertexId) -> DVec3 {
        self.ctx_of(body).vertex(vertex).unwrap().position
    }

    fn chain_points(&self, body: BodyId, c: &Curve) -> Vec<DVec3> {
        c.vertices.iter().map(|x| self.position(body, *x)).collect()
    }

    /// The model validates and every curve of every body fits.
    #[track_caller]
    fn valid(&self) {
        assert_eq!(self.model.validate(), Vec::new());
        let bodies = std::iter::once(BodyId::Root)
            .chain(self.model.definitions().map(|(d, _)| BodyId::Definition(d)));
        for body in bodies {
            let ctx = self.ctx_of(body);
            for (id, c) in ctx.curves() {
                assert!(curve::fits(ctx, c), "{body:?} curve {id:?} doesn't fit");
            }
        }
    }

    fn face_with_center(&self, p: DVec3) -> FaceId {
        let ctx = self.ctx();
        ctx.faces()
            .find(|(_, f)| {
                let pts = ctx.loop_points(&f.outer);
                let c = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
                c.distance(p) < 1e-6
            })
            .map(|(id, _)| id)
            .expect("a face centered there")
    }
}

/// The curve `c` (in `from`'s coordinates) moved by `m`, compared with `got` point by point.
#[track_caller]
fn assert_moved(c: &Curve, m: DMat4, got: &Curve) {
    let size = 1.0 + m.transform_point3(c.center).length();
    for k in 0..=8 {
        let a = c.start + c.sweep() * k as f64 / 8.0;
        let want = m.transform_point3(c.point_at(a));
        let have = got.point_at(a);
        assert!(
            want.distance(have) < 1e-9 * size,
            "angle {a}: {have:?} != {want:?}"
        );
    }
    assert_near(got.start, c.start);
    assert_near(got.end, c.end);
}

fn by_hand() -> Curve {
    Curve {
        center: v(1.0, 2.0, 3.0),
        normal: DVec3::Z,
        x_axis: DVec3::X,
        radius: 2.0,
        start: 0.25,
        end: 0.25 + 1.5,
        vertices: vec![VertexId(4), VertexId(5), VertexId(6)],
    }
}

// ---------- Curve::transformed ----------

#[test]
fn transformed_by_identity_is_the_same_curve() {
    let c = by_hand();
    assert_eq!(c.transformed(DMat4::IDENTITY), Some(c));
}

#[test]
fn transformed_by_translation_moves_the_center_only() {
    let c = by_hand();
    let t = c
        .transformed(DMat4::from_translation(v(5.0, -1.0, 2.0)))
        .unwrap();
    assert_close(t.center, v(6.0, 1.0, 5.0));
    assert_eq!(t.normal, c.normal);
    assert_eq!(t.x_axis, c.x_axis);
    assert_eq!(t.radius, c.radius);
    assert_eq!(t.start, c.start);
    assert_eq!(t.end, c.end);
    assert_eq!(t.vertices, c.vertices);
}

#[test]
fn transformed_by_rotation_maps_every_point() {
    let c = by_hand();
    let m = similarity(v(1.0, 2.0, -0.5), 1.1, 1.0, false, v(0.3, 0.0, -2.0));
    let t = c.transformed(m).unwrap();
    assert_moved(&c, m, &t);
    assert_near(t.radius, 2.0);
    assert_close(t.normal, m.transform_vector3(DVec3::Z).normalize());
    assert_close(t.x_axis, m.transform_vector3(DVec3::X).normalize());
}

#[test]
fn transformed_by_uniform_scale_scales_the_radius() {
    let c = by_hand();
    let m = DMat4::from_scale(DVec3::splat(2.5));
    let t = c.transformed(m).unwrap();
    assert_near(t.radius, 5.0);
    assert_close(t.center, v(2.5, 5.0, 7.5));
    assert_close(t.normal, DVec3::Z);
    assert_moved(&c, m, &t);
}

#[test]
fn transformed_by_mirror_turns_the_normal_and_keeps_the_angles() {
    let c = by_hand();
    let m = DMat4::from_scale(v(-1.0, 1.0, 1.0));
    let t = c.transformed(m).unwrap();
    assert_close(t.normal, -DVec3::Z);
    assert_close(t.x_axis, -DVec3::X);
    assert_close(t.y_axis(), DVec3::Y);
    assert_eq!(t.start, c.start);
    assert_eq!(t.end, c.end);
    assert_moved(&c, m, &t);
}

#[test]
fn transformed_by_mirror_scale_and_rotation_maps_every_point() {
    let c = by_hand();
    let m = similarity(v(0.0, 1.0, 1.0), -2.4, 0.3, true, v(7.0, 7.0, 7.0));
    let t = c.transformed(m).unwrap();
    assert_moved(&c, m, &t);
    assert_near(t.radius, 0.6);
    assert_near(t.normal.length(), 1.0);
    assert_near(t.x_axis.length(), 1.0);
    assert!(t.normal.dot(t.x_axis).abs() < 1e-12);
}

#[test]
fn transformed_keeps_vertices_of_a_closed_curve() {
    let mut c = by_hand();
    c.vertices = vec![VertexId(1), VertexId(2), VertexId(3), VertexId(1)];
    c.end = c.start + TAU;
    let t = c.transformed(DMat4::from_rotation_z(0.7)).unwrap();
    assert_eq!(t.vertices, c.vertices);
    assert_eq!(t.end - t.start, TAU);
}

#[test]
fn transformed_by_non_uniform_scale_is_none() {
    let c = by_hand();
    assert_eq!(c.transformed(DMat4::from_scale(v(2.0, 1.0, 1.0))), None);
    // Even a stretch along the normal, which leaves the circle a circle: carry re-fits that.
    assert_eq!(c.transformed(DMat4::from_scale(v(1.0, 1.0, 2.0))), None);
}

#[test]
fn transformed_by_shear_is_none() {
    let c = by_hand();
    let mut m = DMat4::IDENTITY;
    m.y_axis.x = 0.5;
    assert_eq!(c.transformed(m), None);
}

#[test]
fn transformed_by_singular_or_not_finite_matrix_is_none() {
    let c = by_hand();
    assert_eq!(c.transformed(DMat4::from_scale(DVec3::ZERO)), None);
    assert_eq!(
        c.transformed(DMat4::from_translation(v(f64::NAN, 0.0, 0.0))),
        None
    );
    let mut m = DMat4::IDENTITY;
    m.x_axis.x = f64::INFINITY;
    assert_eq!(c.transformed(m), None);
}

#[test]
fn transformed_allows_a_tiny_rounding_error() {
    let c = by_hand();
    let mut m = DMat4::from_rotation_y(0.3);
    m.x_axis.x *= 1.0 + 1e-13;
    let t = c.transformed(m).unwrap();
    assert_near(t.radius, 2.0);
}

// ---------- curve::carry ----------

/// Copy the root's geometry into a new definition moved by `m`, the way the operations do,
/// and carry `curves` with it. Returns the definition body, the carry result and the log.
fn copy_into_definition(
    w: &mut World,
    m: DMat4,
    curves: &[Curve],
    drop_vertex: Option<VertexId>,
) -> (BodyId, Vec<Option<CurveId>>, ModelEdit) {
    let source = w.model.root().geometry().clone();
    let mut edit = ModelEdit::new();
    let def = w.model.add_definition(
        DefinitionInfo {
            name: "Copy".into(),
            group: false,
        },
        &mut edit,
    );
    let body = BodyId::Definition(def);
    let result = w
        .model
        .edit_geometry(body, &mut edit, |ctx, log| {
            let mut map = std::collections::BTreeMap::new();
            for (id, vertex) in source.vertices() {
                map.insert(
                    id,
                    ctx.insert_vertex(m.transform_point3(vertex.position), log),
                );
            }
            for (_, edge) in source.edges() {
                ctx.insert_edge(map[&edge.ends[0]], map[&edge.ends[1]], log)
                    .unwrap();
            }
            if let Some(gone) = drop_vertex {
                map.remove(&gone);
            }
            curve::carry(ctx, curves, m, |_, x| map.get(&x).copied(), log)
        })
        .unwrap();
    (body, result, edit)
}

fn curve_records(edit: &ModelEdit) -> usize {
    edit.records
        .iter()
        .filter(|r| {
            matches!(
                r,
                ModelRecord::Geometry(_, Record::AddCurve(..) | Record::RemoveCurve(..))
            )
        })
        .count()
}

#[test]
fn carry_puts_the_moved_curve_on_the_mapped_chain() {
    let mut w = World::new();
    let (_, c) = w.circle(v(1.0, 1.0, 0.0), 2.0);
    let m = similarity(v(1.0, 0.0, 1.0), 0.8, 1.5, false, v(10.0, 0.0, 0.0));
    let (body, result, edit) = copy_into_definition(&mut w, m, std::slice::from_ref(&c), None);
    assert_eq!(result.len(), 1);
    let id = result[0].expect("carried");
    let got = w.ctx_of(body).curve(id).unwrap().clone();
    assert_moved(&c, m, &got);
    assert_near(got.radius, 3.0);
    assert!(got.is_closed());
    assert_eq!(got.vertices.len(), c.vertices.len());
    for (a, b) in c.vertices.iter().zip(&got.vertices) {
        assert_close(
            m.transform_point3(w.position(BodyId::Root, *a)),
            w.position(body, *b),
        );
    }
    assert_eq!(curve_records(&edit), 1);
    w.valid();
}

#[test]
fn carry_skips_a_curve_whose_chain_does_not_map() {
    let mut w = World::new();
    let (_, c) = w.circle(DVec3::ZERO, 1.0);
    let gone = c.vertices[5];
    let (body, result, edit) = copy_into_definition(
        &mut w,
        DMat4::IDENTITY,
        std::slice::from_ref(&c),
        Some(gone),
    );
    assert_eq!(result, vec![None]);
    assert_eq!(w.ctx_of(body).curve_count(), 0);
    assert_eq!(curve_records(&edit), 0);
}

#[test]
fn carry_refits_when_the_matrix_keeps_the_circle_round() {
    let mut w = World::new();
    let (_, c) = w.circle(v(1.0, 0.0, 0.0), 1.0);
    // Uniform in the circle's plane, stretched along its normal: still a circle.
    let m = DMat4::from_scale(v(2.0, 2.0, 3.0));
    let (body, result, _) = copy_into_definition(&mut w, m, std::slice::from_ref(&c), None);
    let got = w.ctx_of(body).curve(result[0].unwrap()).unwrap().clone();
    assert_near(got.radius, 2.0);
    assert_close(got.center, v(2.0, 0.0, 0.0));
    assert_close(got.normal, DVec3::Z);
    assert_eq!(got.start, c.start);
    assert_close(w.position(body, got.vertices[0]), got.point_at(got.start));
    w.valid();
}

#[test]
fn carry_skips_a_curve_that_would_be_an_ellipse() {
    let mut w = World::new();
    let (_, c) = w.circle(DVec3::ZERO, 1.0);
    let m = DMat4::from_scale(v(2.0, 1.0, 1.0));
    let (body, result, _) = copy_into_definition(&mut w, m, std::slice::from_ref(&c), None);
    assert_eq!(result, vec![None]);
    assert_eq!(w.ctx_of(body).curve_count(), 0);
    assert_eq!(w.ctx_of(body).edge_count(), 24);
}

#[test]
fn carry_never_puts_two_curves_on_one_edge() {
    let mut w = World::new();
    let (_, c) = w.circle(DVec3::ZERO, 1.0);
    let (body, result, _) =
        copy_into_definition(&mut w, DMat4::IDENTITY, &[c.clone(), c.clone()], None);
    assert!(result[0].is_some());
    assert_eq!(result[1], None);
    assert_eq!(w.ctx_of(body).curve_count(), 1);

    // Carrying onto a curve that is already there adds nothing either.
    let mut edit = ModelEdit::new();
    let again = w
        .model
        .edit_geometry(BodyId::Root, &mut edit, |ctx, log| {
            curve::carry(
                ctx,
                std::slice::from_ref(&c),
                DMat4::IDENTITY,
                |_, x| Some(x),
                log,
            )
        })
        .unwrap();
    assert_eq!(again, vec![None]);
    assert_eq!(w.ctx().curve_count(), 1);
    assert_eq!(curve_records(&edit), 0);
}

#[test]
fn carry_answers_each_curve_in_order() {
    let mut w = World::new();
    let (_, a) = w.circle(DVec3::ZERO, 1.0);
    let (_, b) = w.circle(v(5.0, 0.0, 0.0), 1.0);
    let m = DMat4::from_translation(v(0.0, 9.0, 0.0));
    let gone = a.vertices[3];
    let (body, result, _) = copy_into_definition(&mut w, m, &[a.clone(), b.clone()], Some(gone));
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], None);
    let got = w.ctx_of(body).curve(result[1].unwrap()).unwrap().clone();
    assert_close(got.center, v(5.0, 9.0, 0.0));
}

#[test]
fn carry_of_nothing_changes_nothing() {
    let mut w = World::new();
    let (body, result, edit) = copy_into_definition(&mut w, DMat4::IDENTITY, &[], None);
    assert!(result.is_empty());
    assert_eq!(w.ctx_of(body).curve_count(), 0);
    assert_eq!(curve_records(&edit), 0);
}

// ---------- make group / component ----------

#[test]
fn make_group_of_a_circle_moves_the_curve_into_the_group() {
    let mut w = World::new();
    let (_, c) = w.circle(v(3.0, 4.0, 0.0), 2.0);
    let before = w.chain_points(BodyId::Root, &c);
    let sel = w.everything(BodyId::Root);
    let inst = w.make(&sel, true);
    assert_eq!(w.ctx().curve_count(), 0);
    let def = w.definition_of(inst);
    let inside = w.curves_of(def);
    assert_eq!(inside.len(), 1);
    let got = &inside[0].1;
    let t = w.instance(inst).transform;
    // Shown through the instance, the curve is where it was.
    let shown = got.transformed(t).unwrap();
    assert_same_circle(&shown, &c);
    for (p, x) in before.iter().zip(&got.vertices) {
        assert_close(t.transform_point3(w.position(def, *x)), *p);
    }
    assert_eq!(w.undo.undo_count(), 2);
    w.valid();
}

#[test]
fn make_component_carries_an_arc() {
    let mut w = World::new();
    let id = w.draw_open(BodyId::Root, &arc_xy(v(1.0, 0.0, 2.0), 1.5, 0.3, 2.5, 10));
    let c = w.ctx().curve(id).unwrap().clone();
    let sel = w.everything(BodyId::Root);
    let inst = w.make(&sel, false);
    assert_eq!(w.ctx().curve_count(), 0);
    let def = w.definition_of(inst);
    let inside = w.curves_of(def);
    assert_eq!(inside.len(), 1);
    assert!(!inside[0].1.is_closed());
    let shown = inside[0].1.transformed(w.instance(inst).transform).unwrap();
    assert_same_circle(&shown, &c);
    w.valid();
}

#[test]
fn make_group_of_part_of_a_curve_leaves_the_curve_in_place() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0);
    let edges = curve::curve_edges(w.ctx(), id);
    let sel: Vec<Entity> = edges[..12].iter().map(|e| Entity::Edge(*e)).collect();
    let inst = w.make(&sel, true);
    // The edges stay in the root because the disc still uses them.
    assert_eq!(w.curves(), vec![(id, c)]);
    let def = w.definition_of(inst);
    assert_eq!(w.ctx_of(def).edge_count(), 12);
    assert_eq!(w.ctx_of(def).curve_count(), 0);
    w.valid();
}

#[test]
fn make_group_of_a_circles_edges_copies_the_curve_and_keeps_it() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0);
    let sel: Vec<Entity> = curve::curve_edges(w.ctx(), id)
        .into_iter()
        .map(Entity::Edge)
        .collect();
    let inst = w.make(&sel, true);
    // The face is still there, so the edges and the curve stay in the root ...
    assert_eq!(w.curves(), vec![(id, c.clone())]);
    // ... and the group got a copy.
    let def = w.definition_of(inst);
    let inside = w.curves_of(def);
    assert_eq!(inside.len(), 1);
    let shown = inside[0].1.transformed(w.instance(inst).transform).unwrap();
    assert_same_circle(&shown, &c);
    w.valid();
}

#[test]
fn make_group_takes_only_the_selected_curves() {
    let mut w = World::new();
    let (_, a) = w.circle(DVec3::ZERO, 1.0);
    let (b_id, b) = w.circle(v(5.0, 0.0, 0.0), 1.0);
    let face = w.face_with_center(DVec3::ZERO);
    let mut sel = vec![Entity::Face(face)];
    sel.extend(
        curve::curve_edges(w.ctx(), w.curves()[0].0)
            .into_iter()
            .map(Entity::Edge),
    );
    let inst = w.make(&sel, true);
    assert_eq!(w.curves(), vec![(b_id, b)]);
    let def = w.definition_of(inst);
    let inside = w.curves_of(def);
    assert_eq!(inside.len(), 1);
    let shown = inside[0].1.transformed(w.instance(inst).transform).unwrap();
    assert_same_circle(&shown, &a);
    w.valid();
}

#[test]
fn make_group_undo_and_redo_are_exact() {
    let mut w = World::new();
    w.circle(v(1.0, 2.0, 0.0), 1.0);
    w.draw_open(BodyId::Root, &arc_xy(v(6.0, 0.0, 0.0), 1.0, 0.0, PI, 8));
    let before = w.model.clone();
    let sel = w.everything(BodyId::Root);
    w.make(&sel, true);
    let after = w.model.clone();
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.model, before);
    w.undo.redo(&mut w.model).unwrap();
    assert_eq!(w.model, after);
    w.valid();
}

// ---------- explode ----------

#[test]
fn group_then_explode_gives_the_curve_back() {
    let mut w = World::new();
    let (_, c) = w.circle(v(3.0, -1.0, 0.5), 2.0);
    let before = w.chain_points(BodyId::Root, &c);
    let sel = w.everything(BodyId::Root);
    let inst = w.make(&sel, true);
    w.explode(inst);
    let all = w.curves();
    assert_eq!(all.len(), 1);
    let got = &all[0].1;
    assert_same_circle(got, &c);
    assert_eq!(got.vertices.len(), c.vertices.len());
    for (p, x) in before.iter().zip(&got.vertices) {
        assert_close(w.position(BodyId::Root, *x), *p);
    }
    w.valid();
}

#[test]
fn explode_a_moved_component_moves_its_curves() {
    let mut w = World::new();
    w.circle(DVec3::ZERO, 1.0);
    w.draw_open(BodyId::Root, &arc_xy(v(4.0, 0.0, 0.0), 1.0, -0.5, 2.0, 9));
    let sel = w.everything(BodyId::Root);
    let inst = w.make(&sel, false);
    let def = w.definition_of(inst);
    let inside = w.curves_of(def);
    assert_eq!(inside.len(), 2);
    let m = similarity(v(0.3, -1.0, 2.0), 2.2, 1.7, false, v(20.0, 5.0, -3.0));
    w.set_transform(inst, m);
    w.explode(inst);
    // The component's definition keeps its own curves.
    assert_eq!(w.curves_of(def), inside);
    let got = w.curves();
    assert_eq!(got.len(), 2);
    for (_, c) in &inside {
        let want = c.transformed(m).unwrap();
        let found = got
            .iter()
            .find(|(_, g)| g.center.distance(want.center) < 1e-6)
            .expect("a moved copy");
        assert_moved(c, m, &found.1);
        assert_eq!(found.1.is_closed(), c.is_closed());
    }
    w.valid();
}

#[test]
fn explode_a_mirrored_instance_keeps_the_curve() {
    let mut w = World::new();
    let (_, c) = w.circle(v(1.0, 1.0, 0.0), 1.0);
    let sel = w.everything(BodyId::Root);
    let inst = w.make(&sel, false);
    let def = w.definition_of(inst);
    let inside = w.curves_of(def)[0].1.clone();
    let m = DMat4::from_translation(v(10.0, 0.0, 0.0)) * DMat4::from_scale(v(-1.0, 1.0, 1.0));
    w.set_transform(inst, m);
    w.explode(inst);
    let all = w.curves();
    assert_eq!(all.len(), 1);
    assert_moved(&inside, m, &all[0].1);
    assert_close(all[0].1.normal, -c.normal);
    w.valid();
}

#[test]
fn explode_a_stretched_instance_leaves_plain_edges() {
    let mut w = World::new();
    w.circle(DVec3::ZERO, 1.0);
    let sel = w.everything(BodyId::Root);
    let inst = w.make(&sel, false);
    let def = w.definition_of(inst);
    w.set_transform(inst, DMat4::from_scale(v(2.0, 1.0, 1.0)));
    w.explode(inst);
    assert_eq!(w.ctx().curve_count(), 0);
    assert_eq!(w.ctx().edge_count(), 24);
    assert_eq!(w.ctx_of(def).curve_count(), 1);
    w.valid();
}

#[test]
fn explode_an_instance_scaled_in_its_plane_refits_the_curve() {
    let mut w = World::new();
    let (_, c) = w.circle(DVec3::ZERO, 1.0);
    let sel = w.everything(BodyId::Root);
    let inst = w.make(&sel, true);
    w.set_transform(
        inst,
        DMat4::from_scale(v(2.0, 2.0, 1.0)) * w.instance(inst).transform,
    );
    w.explode(inst);
    let all = w.curves();
    assert_eq!(all.len(), 1);
    let got = &all[0].1;
    assert_near(got.radius, 2.0);
    assert_close(got.center, DVec3::ZERO);
    assert_close(got.normal, c.normal);
    assert_eq!(got.start, c.start);
    w.valid();
}

#[test]
fn explode_onto_the_same_circle_adds_no_second_curve() {
    let mut w = World::new();
    w.circle(DVec3::ZERO, 1.0);
    let sel = w.everything(BodyId::Root);
    let inst = w.make(&sel, false);
    // The same circle again, in the root, right where the instance shows its own.
    let (id, c) = w.circle(DVec3::ZERO, 1.0);
    w.explode(inst);
    let all = w.curves();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].0, id);
    assert_same_circle(&all[0].1, &c);
    w.valid();
}

#[test]
fn explode_where_lines_cross_the_circle_keeps_the_curve() {
    let mut w = World::new();
    let (_, c) = w.circle(DVec3::ZERO, 1.0);
    let sel = w.everything(BodyId::Root);
    let inst = w.make(&sel, true);
    // A line in the root that crosses two of the circle's chords.
    w.line(v(-3.0, 0.05, 0.0), v(3.0, 0.05, 0.0));
    w.explode(inst);
    let all = w.curves();
    assert_eq!(all.len(), 1);
    assert_same_circle(&all[0].1, &c);
    assert!(curve::curve_edges(w.ctx(), all[0].0).len() >= 24);
    w.valid();
}

#[test]
fn explode_undo_and_redo_are_exact() {
    let mut w = World::new();
    w.circle(DVec3::ZERO, 1.0);
    let sel = w.everything(BodyId::Root);
    let inst = w.make(&sel, true);
    w.set_transform(inst, DMat4::from_rotation_x(FRAC_PI_2));
    let before = w.model.clone();
    w.explode(inst);
    let after = w.model.clone();
    assert_eq!(w.undo.undo_label(), Some("Explode"));
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.model, before);
    w.undo.redo(&mut w.model).unwrap();
    assert_eq!(w.model, after);
    w.valid();
}

// ---------- copy, paste, import ----------

#[test]
fn copy_a_disc_copies_its_curve() {
    let mut w = World::new();
    let (id, c) = w.circle(v(1.0, 0.0, 0.0), 1.0);
    let sel = w.everything(BodyId::Root);
    let m = DMat4::from_translation(v(0.0, 5.0, 0.0));
    copy_selection(&mut w.model, BodyId::Root, &sel, m, &mut w.undo, "Copy").unwrap();
    let all = w.curves();
    assert_eq!(all.len(), 2);
    assert_eq!(w.ctx().curve(id), Some(&c));
    let copy = all.iter().find(|(i, _)| *i != id).unwrap();
    assert_moved(&c, m, &copy.1);
    assert_eq!(w.undo.undo_count(), 2);
    w.valid();
}

#[test]
fn paste_with_rotation_scale_and_mirror() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0);
    let clip = extract(&w.model, BodyId::Root, &w.everything(BodyId::Root)).unwrap();
    let m = similarity(v(1.0, 1.0, 0.0), 0.9, 0.5, true, v(-8.0, 3.0, 1.0));
    insert(&mut w.model, BodyId::Root, &clip, m, &mut w.undo, "Paste").unwrap();
    let all = w.curves();
    assert_eq!(all.len(), 2);
    let copy = all.iter().find(|(i, _)| *i != id).unwrap();
    assert_moved(&c, m, &copy.1);
    assert_near(copy.1.radius, 0.5);
    w.valid();
}

#[test]
fn the_clipboard_holds_the_curve() {
    let mut w = World::new();
    let (_, c) = w.circle(v(2.0, 2.0, 0.0), 1.0);
    let clip = extract(&w.model, BodyId::Root, &w.everything(BodyId::Root)).unwrap();
    let root = clip.model().root().geometry();
    let all: Vec<&Curve> = root.curves().map(|(_, c)| c).collect();
    assert_eq!(all.len(), 1);
    assert_same_circle(all[0], &c);
    assert!(curve::fits(root, all[0]));
}

#[test]
fn copy_a_circles_edges_without_the_face() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0);
    let sel: Vec<Entity> = curve::curve_edges(w.ctx(), id)
        .into_iter()
        .map(Entity::Edge)
        .collect();
    let m = DMat4::from_translation(v(4.0, 0.0, 0.0));
    copy_selection(&mut w.model, BodyId::Root, &sel, m, &mut w.undo, "Copy").unwrap();
    let all = w.curves();
    assert_eq!(all.len(), 2);
    let copy = all.iter().find(|(i, _)| *i != id).unwrap();
    assert_moved(&c, m, &copy.1);
    w.valid();
}

#[test]
fn copy_part_of_a_curve_gives_plain_edges() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0);
    let edges = curve::curve_edges(w.ctx(), id);
    let sel: Vec<Entity> = edges[..10].iter().map(|e| Entity::Edge(*e)).collect();
    let m = DMat4::from_translation(v(4.0, 0.0, 0.0));
    copy_selection(&mut w.model, BodyId::Root, &sel, m, &mut w.undo, "Copy").unwrap();
    assert_eq!(w.curves(), vec![(id, c)]);
    w.valid();
}

#[test]
fn copy_a_group_copies_the_curve_inside() {
    let mut w = World::new();
    let (_, c) = w.circle(DVec3::ZERO, 1.0);
    let sel = w.everything(BodyId::Root);
    let inst = w.make(&sel, true);
    let m = DMat4::from_translation(v(3.0, 0.0, 0.0));
    let copied = copy_selection(
        &mut w.model,
        BodyId::Root,
        &[Entity::Instance(inst)],
        m,
        &mut w.undo,
        "Copy",
    )
    .unwrap();
    let Entity::Instance(other) = copied[0] else {
        panic!("{copied:?}");
    };
    let a = w.definition_of(inst);
    let b = w.definition_of(other);
    assert_ne!(a, b);
    for (body, id) in [(a, inst), (b, other)] {
        let inside = w.curves_of(body);
        assert_eq!(inside.len(), 1, "{body:?}");
        let shown = inside[0].1.transformed(w.instance(id).transform).unwrap();
        let want = if id == inst {
            c.clone()
        } else {
            c.transformed(m).unwrap()
        };
        assert_same_circle(&shown, &want);
    }
    w.valid();
}

#[test]
fn copy_undo_and_redo_are_exact() {
    let mut w = World::new();
    w.circle(DVec3::ZERO, 1.0);
    let sel = w.everything(BodyId::Root);
    let before = w.model.clone();
    let m = DMat4::from_translation(v(0.0, 0.0, 3.0)) * DMat4::from_rotation_x(1.0);
    copy_selection(&mut w.model, BodyId::Root, &sel, m, &mut w.undo, "Copy").unwrap();
    let after = w.model.clone();
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.model, before);
    w.undo.redo(&mut w.model).unwrap();
    assert_eq!(w.model, after);
    w.valid();
}

#[test]
fn import_component_carries_curves_at_every_level() {
    let mut src = World::new();
    let (_, outer) = src.circle(DVec3::ZERO, 2.0);
    let (_, inner) = src.circle(v(10.0, 0.0, 0.0), 1.0);
    let face = src.face_with_center(v(10.0, 0.0, 0.0));
    let mut sel = vec![Entity::Face(face)];
    sel.extend(
        curve::curve_edges(src.ctx(), src.curves()[1].0)
            .into_iter()
            .map(Entity::Edge),
    );
    src.make(&sel, false);
    assert_eq!(src.ctx().curve_count(), 1);

    let mut w = World::new();
    let def = import_component(&mut w.model, &src.model, "Part", &mut w.undo, "Import").unwrap();
    let body = BodyId::Definition(def);
    let top = w.curves_of(body);
    assert_eq!(top.len(), 1);
    assert_same_circle(&top[0].1, &outer);
    let (_, nested) = w.model.body(body).unwrap().instances().next().unwrap();
    let nested_body = BodyId::Definition(nested.definition);
    let deep = w.curves_of(nested_body);
    assert_eq!(deep.len(), 1);
    let shown = deep[0].1.transformed(nested.transform).unwrap();
    assert_same_circle(&shown, &inner);
    assert_eq!(w.ctx().curve_count(), 0);
    w.valid();
}

// ---------- push/pull ----------

/// The cap's curve: the one whose vertices are all on face `cap`.
fn curve_on_face(w: &World, cap: FaceId) -> (CurveId, Curve) {
    let ctx = w.ctx();
    let face = ctx.face(cap).unwrap();
    let mut on: BTreeSet<VertexId> = ctx.loop_vertices(&face.outer).into_iter().collect();
    for hole in &face.inner {
        on.extend(ctx.loop_vertices(hole));
    }
    let found: Vec<(CurveId, Curve)> = w
        .curves()
        .into_iter()
        .filter(|(_, c)| c.vertices.iter().all(|x| on.contains(x)))
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    found[0].clone()
}

#[test]
fn pull_a_disc_gives_a_cylinder_with_a_curve_at_each_end() {
    let mut w = World::new();
    let (id, c) = w.circle(v(1.0, 1.0, 0.0), 1.0);
    let face = w.ctx().faces().next().unwrap().0;
    let n = w.ctx().face_plane(face).unwrap().normal;
    let done = w.pull(face, 2.0);
    let all = w.curves();
    assert_eq!(all.len(), 2);
    assert_eq!(w.ctx().curve(id), Some(&c));
    let (cap_id, cap) = curve_on_face(&w, done.cap);
    assert_ne!(cap_id, id);
    assert_close(cap.center, c.center + n * 2.0);
    assert_close(cap.normal, c.normal);
    assert_close(cap.x_axis, c.x_axis);
    assert_eq!(cap.radius, c.radius);
    assert_eq!(cap.start, c.start);
    assert_eq!(cap.end, c.end);
    assert!(cap.is_closed());
    assert_eq!(cap.vertices.len(), c.vertices.len());
    // The cap's edges are its steps; the sides' new edges belong to no curve.
    let cap_edges: BTreeSet<EdgeId> = curve::curve_edges(w.ctx(), cap_id).into_iter().collect();
    assert_eq!(cap_edges.len(), 24);
    for side in &done.sides {
        for u in &w.ctx().face(*side).unwrap().outer.uses {
            let owner = curve::curve_of_edge(w.ctx(), u.edge);
            assert!(owner.is_none() || owner == Some(id) || owner == Some(cap_id));
        }
    }
    let vertical = w
        .ctx()
        .edges()
        .filter(|(e, _)| curve::curve_of_edge(w.ctx(), *e).is_none())
        .count();
    assert_eq!(vertical, 24);
    w.valid();
}

#[test]
fn push_a_lone_disc_backwards() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 2.0);
    let face = w.ctx().faces().next().unwrap().0;
    let n = w.ctx().face_plane(face).unwrap().normal;
    let done = w.pull(face, -1.5);
    assert_eq!(w.ctx().curve(id), Some(&c));
    let (_, cap) = curve_on_face(&w, done.cap);
    assert_close(cap.center, c.center - n * 1.5);
    assert_eq!(cap.radius, c.radius);
    assert_eq!(w.ctx().curve_count(), 2);
    w.valid();
}

#[test]
fn pull_a_d_shape_carries_the_arc_only() {
    let mut w = World::new();
    let id = w.draw_open(BodyId::Root, &arc_xy(DVec3::ZERO, 1.0, 0.0, PI, 12));
    let c = w.ctx().curve(id).unwrap().clone();
    w.line(v(-1.0, 0.0, 0.0), v(1.0, 0.0, 0.0));
    assert_eq!(w.ctx().face_count(), 1);
    let face = w.ctx().faces().next().unwrap().0;
    let n = w.ctx().face_plane(face).unwrap().normal;
    let done = w.pull(face, 0.75);
    assert_eq!(w.ctx().curve_count(), 2);
    let (_, cap) = curve_on_face(&w, done.cap);
    assert!(!cap.is_closed());
    assert_close(cap.center, c.center + n * 0.75);
    assert_eq!(cap.start, c.start);
    assert_eq!(cap.end, c.end);
    w.valid();
}

/// A 4 x 4 x 2 box on the ground with a circle of radius 1 drawn on its top face.
fn box_with_circle_on_top(w: &mut World) -> (CurveId, Curve, FaceId) {
    let corners = [
        v(0.0, 0.0, 0.0),
        v(4.0, 0.0, 0.0),
        v(4.0, 4.0, 0.0),
        v(0.0, 4.0, 0.0),
    ];
    let segs: Vec<(DVec3, DVec3)> = (0..4).map(|i| (corners[i], corners[(i + 1) % 4])).collect();
    draw_segments(&mut w.target(BodyId::Root), &segs, "Rectangle").unwrap();
    let base = w.ctx().faces().next().unwrap().0;
    let up = w.ctx().face_plane(base).unwrap().normal.z;
    w.pull(base, -2.0 * up);
    let top_z = 2.0;
    let pts = ring(v(2.0, 2.0, top_z), DVec3::X, DVec3::Y, 1.0, 24, 0.0);
    let id = w.draw_closed(BodyId::Root, &pts);
    let c = w.ctx().curve(id).unwrap().clone();
    let disc = w.face_with_center(v(2.0, 2.0, top_z));
    (id, c, disc)
}

#[test]
fn push_a_hole_into_a_box() {
    let mut w = World::new();
    let (id, c, disc) = box_with_circle_on_top(&mut w);
    assert_eq!(w.ctx().curve_count(), 1);
    let n = w.ctx().face_plane(disc).unwrap().normal;
    assert_close(n, DVec3::Z);
    let done = w.pull(disc, -1.0);
    assert_eq!(w.ctx().curve(id), Some(&c));
    assert_eq!(w.ctx().curve_count(), 2);
    let (_, cap) = curve_on_face(&w, done.cap);
    assert_close(cap.center, v(2.0, 2.0, 1.0));
    assert_near(cap.radius, 1.0);
    w.valid();
}

#[test]
fn push_a_hole_through_a_box() {
    let mut w = World::new();
    let (id, c, disc) = box_with_circle_on_top(&mut w);
    let done = w.pull(disc, -2.0);
    // The cap met the bottom and became a hole in it; the curve stays on the hole's edges.
    assert_eq!(w.ctx().curve(id), Some(&c));
    let all = w.curves();
    assert_eq!(all.len(), 2);
    let bottom = all.iter().find(|(i, _)| *i != id).unwrap();
    assert_close(bottom.1.center, v(2.0, 2.0, 0.0));
    assert!(w.ctx().face(done.cap).unwrap().inner.len() == 1);
    w.valid();
}

#[test]
fn pull_a_face_without_curves_adds_none() {
    let mut w = World::new();
    let (_, c) = w.circle(v(10.0, 0.0, 0.0), 1.0);
    let corners = [
        v(0.0, 0.0, 0.0),
        v(2.0, 0.0, 0.0),
        v(2.0, 2.0, 0.0),
        v(0.0, 2.0, 0.0),
    ];
    let segs: Vec<(DVec3, DVec3)> = (0..4).map(|i| (corners[i], corners[(i + 1) % 4])).collect();
    draw_segments(&mut w.target(BodyId::Root), &segs, "Rectangle").unwrap();
    let square = w.face_with_center(v(1.0, 1.0, 0.0));
    w.pull(square, 1.0);
    let all = w.curves();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].1, c);
    w.valid();
}

#[test]
fn pull_the_cap_again_slides_its_curve() {
    let mut w = World::new();
    let (_, c) = w.circle(DVec3::ZERO, 1.0);
    let face = w.ctx().faces().next().unwrap().0;
    let n = w.ctx().face_plane(face).unwrap().normal;
    let done = w.pull(face, 1.0);
    let (cap_id, _) = curve_on_face(&w, done.cap);
    let again = w.pull(done.cap, 0.5);
    assert_eq!(again.cap, done.cap);
    assert!(again.sides.is_empty());
    assert_eq!(w.ctx().curve_count(), 2);
    let moved = w.ctx().curve(cap_id).unwrap();
    assert_close(moved.center, c.center + n * 1.5);
    assert_near(moved.radius, 1.0);
    w.valid();
}

#[test]
fn push_pull_undo_and_redo_are_exact() {
    let mut w = World::new();
    w.circle(DVec3::ZERO, 1.0);
    let face = w.ctx().faces().next().unwrap().0;
    let before = w.model.clone();
    w.pull(face, 3.0);
    let after = w.model.clone();
    assert_eq!(w.undo.undo_label(), Some("Push/Pull"));
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.model, before);
    w.undo.redo(&mut w.model).unwrap();
    assert_eq!(w.model, after);
    w.valid();
}
