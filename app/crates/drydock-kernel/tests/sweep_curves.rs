// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Curves carried through `sweep_face` (`docs/specs/m9-sweep-curves.md`).

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use drydock_kernel::curve::{self, Curve};
use drydock_kernel::format;
use drydock_kernel::model::*;
use drydock_kernel::shapes;
use drydock_kernel::sweep::SweepPath;
use drydock_kernel::sweepface::*;
use drydock_kernel::topology::*;
use glam::DVec3;

mod pushpull_common;
use pushpull_common::*;
mod sweep_face_common;
use sweep_face_common::*;

const X: DVec3 = DVec3::X;
const Y: DVec3 = DVec3::Y;
const Z: DVec3 = DVec3::Z;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-7
}

fn assert_vec(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-7, "{a:?} != {b:?}");
}

fn parallel(a: DVec3, b: DVec3) -> bool {
    a.normalize().cross(b.normalize()).length() < 1e-7
}

/// Make the chain through `pts` (already vertices of the root body) a curve.
fn add_curve(m: &mut Model, pts: &[DVec3], closed: bool) -> CurveId {
    let mut edit = ModelEdit::new();
    m.edit_geometry(BodyId::Root, &mut edit, |c, log| {
        curve::add_curve(c, pts, closed, log)
    })
    .unwrap()
    .expect("the points make a curve")
}

/// A face through `pts` (one loop) with a curve on all of it, closed.
fn curved_face(m: &mut Model, pts: &[DVec3]) -> (FaceId, CurveId) {
    let idx: Vec<usize> = (0..pts.len()).collect();
    let f = mesh(m, pts, &[&[&idx]])[0];
    (f, add_curve(m, pts, true))
}

/// Loose edges through `pts` in the root body (closing back when `closed`), as a path whose
/// vertices and edges are the body's own.
fn body_path(m: &mut Model, pts: &[DVec3], closed: bool) -> SweepPath {
    let mut edit = ModelEdit::new();
    m.edit_geometry(BodyId::Root, &mut edit, |c, log| {
        let vs: Vec<VertexId> = pts
            .iter()
            .map(|p| {
                c.find_vertex(*p, 1e-9)
                    .unwrap_or_else(|| c.insert_vertex(*p, log))
            })
            .collect();
        let n = vs.len();
        let count = if closed { n } else { n - 1 };
        let edges: Vec<EdgeId> = (0..count)
            .map(|i| c.insert_edge(vs[i], vs[(i + 1) % n], log).unwrap())
            .collect();
        SweepPath {
            vertices: vs,
            points: pts.to_vec(),
            edges,
            closed,
        }
    })
    .unwrap()
}

/// `body_path` with a curve on all of it.
fn curve_path(m: &mut Model, pts: &[DVec3], closed: bool) -> (SweepPath, CurveId) {
    let p = body_path(m, pts, closed);
    let id = add_curve(m, pts, closed);
    (p, id)
}

fn curves(c: &Context) -> Vec<(CurveId, Curve)> {
    c.curves().map(|(id, cv)| (id, cv.clone())).collect()
}

/// The curves of `c` whose ids aren't in `before`, ascending id.
fn new_curves(c: &Context, before: &[(CurveId, Curve)]) -> Vec<(CurveId, Curve)> {
    curves(c)
        .into_iter()
        .filter(|(id, _)| before.iter().all(|(b, _)| b != id))
        .collect()
}

fn steps(cv: &Curve) -> usize {
    cv.vertices.len() - 1
}

/// Every curve fits, and no edge belongs to two curves.
fn assert_curves_sound(c: &Context) {
    let mut seen = std::collections::BTreeSet::new();
    for (id, cv) in c.curves() {
        assert!(curve::fits(c, cv), "curve {id:?} doesn't fit");
        let edges = curve::curve_edges(c, id);
        assert_eq!(edges.len(), steps(cv), "curve {id:?}");
        for e in edges {
            assert!(seen.insert(e), "edge {e:?} is on two curves");
        }
    }
}

fn outer_edges(c: &Context, f: FaceId) -> Vec<EdgeId> {
    c.face(f)
        .unwrap()
        .outer
        .uses
        .iter()
        .map(|u| u.edge)
        .collect()
}

/// The 12-gon of radius 2 round the z axis on the ground, first corner on +x.
fn ring12() -> Vec<DVec3> {
    shapes::circle(v(0.0, 0.0, 0.0), Z, X, 2.0, 12)
}

/// The quarter arc of radius 2 round the z axis on the ground, from +x to +y, in 6 steps.
fn quarter_arc() -> Vec<DVec3> {
    shapes::arc(v(0.0, 0.0, 0.0), Z, X, 2.0, 0.0, FRAC_PI_2, 6)
}

/// A square in the plane y = 0 (the plane through the z axis and +x): x from 2.2 to 2.4,
/// z from 0.1 to 0.3.
fn radial_square(m: &mut Model) -> FaceId {
    let pts = rect_points(v(0.0, 0.0, 0.0), X, Z, 2.2, 2.4, 0.1, 0.3);
    mesh(m, &pts, &[&[&[0, 1, 2, 3]]])[0]
}

/// A 12-segment circle of radius 0.2 in the plane y = 0, centred at (2.5, 0, 0.5).
fn radial_circle(m: &mut Model) -> (FaceId, CurveId) {
    let pts = shapes::circle(v(2.5, 0.0, 0.5), Y, X, 0.2, 12);
    curved_face(m, &pts)
}

// -- curves of the profile ---------------------------------------------------------------

#[test]
fn a_circle_on_a_straight_path_reappears_on_the_end_cap() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let pts = shapes::circle(v(0.0, 0.0, 0.0), -X, Y, 0.3, 12);
    let (f, id) = curved_face(&mut m, &pts);
    let old = ctx(&m).curve(id).unwrap().clone();
    let before = curves(ctx(&m));
    let s = sweep(&mut m, &mut undo, f, &straight_path(1.0)).unwrap();
    let c = ctx(&m);
    assert_curves_sound(c);
    assert_eq!(c.curve(id), Some(&old));
    let new = new_curves(c, &before);
    assert_eq!(new.len(), 1);
    let (nid, cv) = &new[0];
    assert!(*nid > id);
    assert!(cv.is_closed());
    assert_eq!(steps(cv), 12);
    assert!(close(cv.radius, 0.3));
    assert_vec(cv.center, v(1.0, 0.0, 0.0));
    assert!(parallel(cv.normal, X));
    assert_eq!(cv.start, old.start);
    let cap = outer_edges(c, s.end.unwrap());
    let edges = curve::curve_edges(c, *nid);
    assert_eq!(edges.len(), 12);
    assert!(edges.iter().all(|e| cap.contains(e)));
}

fn straight_path(len: f64) -> SweepPath {
    path(&[v(0.0, 0.0, 0.0), v(len, 0.0, 0.0)], false)
}

#[test]
fn a_mitred_corner_is_no_circle_but_the_end_cap_is() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let pts = shapes::circle(v(0.0, 0.0, 0.0), -X, Y, 0.3, 12);
    let (f, _) = curved_face(&mut m, &pts);
    let before = curves(ctx(&m));
    let p = path(
        &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0)],
        false,
    );
    sweep(&mut m, &mut undo, f, &p).unwrap();
    let c = ctx(&m);
    assert_curves_sound(c);
    let new = new_curves(c, &before);
    assert_eq!(new.len(), 1, "{new:?}");
    let cv = &new[0].1;
    assert_vec(cv.center, v(1.0, 1.0, 0.0));
    assert!(parallel(cv.normal, Y));
    assert!(close(cv.radius, 0.3));
}

#[test]
fn an_arc_on_the_profile_outline_is_carried() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    // A half disc: the arc's seven points, closed by the diameter.
    let pts = shapes::arc(v(0.0, 0.0, 0.0), -X, Y, 0.3, 0.0, PI, 6);
    let idx: Vec<usize> = (0..pts.len()).collect();
    let f = mesh(&mut m, &pts, &[&[&idx]])[0];
    let id = add_curve(&mut m, &pts, false);
    let old = ctx(&m).curve(id).unwrap().clone();
    let before = curves(ctx(&m));
    sweep(&mut m, &mut undo, f, &straight_path(1.0)).unwrap();
    let c = ctx(&m);
    assert_curves_sound(c);
    assert_eq!(c.curve(id), Some(&old));
    let new = new_curves(c, &before);
    assert_eq!(new.len(), 1);
    let cv = &new[0].1;
    assert!(!cv.is_closed());
    assert_eq!(steps(cv), 6);
    assert_vec(cv.center, v(1.0, 0.0, 0.0));
    assert!(close(cv.radius, 0.3));
    assert_eq!(cv.start, old.start);
    assert!(close(cv.sweep(), PI));
}

#[test]
fn a_round_hole_is_carried_through_a_tube() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let o = v(0.0, 0.0, 0.0);
    let mut pts = rect_points(o, Z, Y, -0.5, 0.5, -0.5, 0.5).to_vec();
    let hole = shapes::circle(o, -X, Y, 0.2, 12);
    pts.extend(hole.iter().copied());
    let hole_idx: Vec<usize> = (4..16).rev().collect();
    let f = mesh(&mut m, &pts, &[&[&[0, 1, 2, 3], &hole_idx]])[0];
    add_curve(&mut m, &hole, true);
    let before = curves(ctx(&m));
    let s = sweep(&mut m, &mut undo, f, &straight_path(2.0)).unwrap();
    let c = ctx(&m);
    assert_curves_sound(c);
    let new = new_curves(c, &before);
    assert_eq!(new.len(), 1);
    let (nid, cv) = &new[0];
    assert_vec(cv.center, v(2.0, 0.0, 0.0));
    assert!(close(cv.radius, 0.2));
    let end = c.face(s.end.unwrap()).unwrap();
    let hole_edges: Vec<EdgeId> = end.inner[0].uses.iter().map(|u| u.edge).collect();
    assert!(
        curve::curve_edges(c, *nid)
            .iter()
            .all(|e| hole_edges.contains(e))
    );
}

#[test]
fn a_ring_round_a_regular_polygon_carries_the_circle_to_every_corner() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let (f, id) = radial_circle(&mut m);
    let old = ctx(&m).curve(id).unwrap().clone();
    let before = curves(ctx(&m));
    // Bare points: the path is no curve, so nothing runs along it.
    let s = sweep(&mut m, &mut undo, f, &path(&ring12(), true)).unwrap();
    assert_eq!(s.start, None);
    let c = ctx(&m);
    assert_curves_sound(c);
    // The profile stood at the first corner, so its curve is the first section's.
    assert_eq!(c.curve(id), Some(&old));
    let new = new_curves(c, &before);
    assert_eq!(new.len(), 11);
    for (k, (_, cv)) in new.iter().enumerate() {
        let a = TAU * (k + 1) as f64 / 12.0;
        assert_vec(cv.center, v(2.5 * a.cos(), 2.5 * a.sin(), 0.5));
        assert!(cv.is_closed());
        assert!(close(cv.radius, 0.2));
        assert!(parallel(cv.normal, v(-a.sin(), a.cos(), 0.0)));
    }
}

#[test]
fn a_ring_from_a_circle_part_way_along_a_side_has_no_section_circles() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    // In the plane through the z axis and the middle of the ring's first side.
    let a = TAU / 24.0;
    let out = v(a.cos(), a.sin(), 0.0);
    let pts = shapes::circle(out * 2.5 + Z * 0.5, v(-a.sin(), a.cos(), 0.0), out, 0.2, 12);
    let (f, _) = curved_face(&mut m, &pts);
    let before = m.clone();
    sweep(&mut m, &mut undo, f, &path(&ring12(), true)).unwrap();
    let c = ctx(&m);
    // Square to the side, the profile meets each corner's mitre at a slant: no section is a
    // circle, and the profile itself is gone.
    assert_eq!(c.curve_count(), 0);
    undo.undo(&mut m).unwrap();
    assert_eq!(m, before);
}

#[test]
fn a_refused_sweep_leaves_the_curves_alone() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let pts = shapes::circle(v(0.0, 0.0, 0.0), -X, Y, 0.1, 12);
    let (f, _) = curved_face(&mut m, &pts);
    // Crosses the side edge the sweep would draw from the circle's point on +y.
    let mut edit = ModelEdit::new();
    m.edit_geometry(BodyId::Root, &mut edit, |c, log| {
        let a = c.insert_vertex(v(1.0, 0.1, -1.0), log);
        let b = c.insert_vertex(v(1.0, 0.1, 1.0), log);
        c.insert_edge(a, b, log).unwrap();
    })
    .unwrap();
    let before = m.clone();
    let r = sweep(&mut m, &mut undo, f, &straight_path(2.0));
    assert!(
        matches!(r, Err(SweepError::Invalid(Violation::EdgesIntersect(..)))),
        "{r:?}"
    );
    assert_eq!(m, before);
    assert_eq!(undo.undo_count(), 0);
}

// -- curves along the path ---------------------------------------------------------------

#[test]
fn a_profile_swept_along_an_arc_leaves_an_arc_through_each_corner() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let (p, path_curve) = curve_path(&mut m, &quarter_arc(), false);
    let old = ctx(&m).curve(path_curve).unwrap().clone();
    let f = radial_square(&mut m);
    let corners = ctx(&m).loop_vertices(&ctx(&m).face(f).unwrap().outer);
    let before = curves(ctx(&m));
    sweep(&mut m, &mut undo, f, &p).unwrap();
    let c = ctx(&m);
    assert_curves_sound(c);
    assert_eq!(c.curve(path_curve), Some(&old));
    let new = new_curves(c, &before);
    assert_eq!(new.len(), 4);
    for ((_, cv), corner) in new.iter().zip(&corners) {
        // From the profile's own corner through the five mitres; the end cap, square to the
        // last step, is off the circle.
        assert_eq!(cv.vertices[0], *corner);
        let at = c.vertex(*corner).unwrap().position;
        assert!(!cv.is_closed());
        assert_eq!(steps(cv), 5);
        assert!(parallel(cv.normal, Z));
        assert_vec(cv.center, v(0.0, 0.0, at.z));
        assert!(close(cv.radius, at.x));
        assert!(close(cv.sweep(), 5.0 * TAU / 24.0));
    }
}

#[test]
fn level_sides_that_merge_split_the_section_circles_into_arcs() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let (p, _) = curve_path(&mut m, &quarter_arc(), false);
    // A hexagon in the plane y = 0 with a level top and bottom side: those sides of
    // neighbouring steps are coplanar and merge, erasing the sections' level edges.
    let pts = shapes::circle(v(2.5, 0.0, 0.5), Y, X, 0.2, 6);
    let (f, id) = curved_face(&mut m, &pts);
    let old = ctx(&m).curve(id).unwrap().clone();
    let before = curves(ctx(&m));
    sweep(&mut m, &mut undo, f, &p).unwrap();
    let c = ctx(&m);
    assert_curves_sound(c);
    // The profile is the start cap, whole.
    assert_eq!(c.curve(id), Some(&old));
    let new = new_curves(c, &before);
    let (sections, along): (Vec<_>, Vec<_>) = new.iter().partition(|(_, cv)| close(cv.radius, 0.2));
    // Each of the five mitres keeps two arcs of two steps; the end cap is no circle.
    assert_eq!(sections.len(), 10);
    for (_, cv) in &sections {
        assert!(!cv.is_closed());
        assert_eq!(steps(cv), 2);
        assert!(close(cv.sweep(), TAU / 3.0));
    }
    assert_eq!(along.len(), 6);
    assert!(along.iter().all(|(_, cv)| steps(cv) == 5));
}

#[test]
fn a_profile_square_to_the_first_step_leaves_shorter_arcs() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let arc = quarter_arc();
    let (p, _) = curve_path(&mut m, &arc, false);
    let d = (arc[1] - arc[0]).normalize();
    let u = d.cross(Z).normalize();
    let pts = rect_points(arc[0], u, Z, 0.2, 0.4, 0.1, 0.3);
    let f = mesh(&mut m, &pts, &[&[&[0, 1, 2, 3]]])[0];
    let before = curves(ctx(&m));
    sweep(&mut m, &mut undo, f, &p).unwrap();
    let c = ctx(&m);
    assert_curves_sound(c);
    let new = new_curves(c, &before);
    assert_eq!(new.len(), 4);
    for (_, cv) in &new {
        // The profile's own corners and the end cap are both off the circle.
        assert_eq!(steps(cv), 4);
        assert!(parallel(cv.normal, Z));
        assert!(cv.center.truncate().length() < 1e-7);
        assert!(close(cv.sweep(), 4.0 * TAU / 24.0));
    }
}

#[test]
fn a_straight_lead_in_does_not_join_the_arc() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let arc = quarter_arc();
    let mut pts = vec![v(2.0, -1.0, 0.0)];
    pts.extend(arc.iter().copied());
    let p = body_path(&mut m, &pts, false);
    add_curve(&mut m, &arc, false);
    let sq = rect_points(v(0.0, -1.0, 0.0), X, Z, 2.2, 2.4, 0.1, 0.3);
    let f = mesh(&mut m, &sq, &[&[&[0, 1, 2, 3]]])[0];
    let before = curves(ctx(&m));
    sweep(&mut m, &mut undo, f, &p).unwrap();
    let c = ctx(&m);
    assert_curves_sound(c);
    let new = new_curves(c, &before);
    assert_eq!(new.len(), 4);
    for (_, cv) in &new {
        // The mitre where the lead-in meets the arc and the end cap are off the circle.
        assert_eq!(steps(cv), 4);
        assert!(parallel(cv.normal, Z));
    }
}

#[test]
fn a_ring_round_a_circle_leaves_a_circle_through_each_corner() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let (p, path_curve) = curve_path(&mut m, &ring12(), true);
    let old = ctx(&m).curve(path_curve).unwrap().clone();
    let f = radial_square(&mut m);
    let corners = ctx(&m).loop_vertices(&ctx(&m).face(f).unwrap().outer);
    let at: Vec<DVec3> = corners
        .iter()
        .map(|v| ctx(&m).vertex(*v).unwrap().position)
        .collect();
    let before = curves(ctx(&m));
    sweep(&mut m, &mut undo, f, &p).unwrap();
    let c = ctx(&m);
    assert_curves_sound(c);
    assert_eq!(c.curve(path_curve), Some(&old));
    let new = new_curves(c, &before);
    assert_eq!(new.len(), 4);
    for (((_, cv), at), corner) in new.iter().zip(&at).zip(&corners) {
        assert_eq!(cv.vertices[0], *corner);
        assert!(cv.is_closed());
        assert_eq!(steps(cv), 12);
        assert!(parallel(cv.normal, Z));
        assert_vec(cv.center, v(0.0, 0.0, at.z));
        assert!(close(cv.radius, at.x));
        assert_eq!(cv.start, 0.0);
        // The first section is the profile's own: the face is gone, its corners stay.
        assert_vec(c.vertex(cv.vertices[0]).unwrap().position, *at);
    }
}

#[test]
fn a_circle_round_a_circle_keeps_both_ways_of_circles_through_undo_and_redo() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let (p, path_curve) = curve_path(&mut m, &ring12(), true);
    let (f, profile_curve) = radial_circle(&mut m);
    let profile_vertices = ctx(&m).loop_vertices(&ctx(&m).face(f).unwrap().outer);
    let before = m.clone();
    let before_curves = curves(ctx(&m));
    sweep(&mut m, &mut undo, f, &p).unwrap();
    let after = m.clone();
    let c = ctx(&m);
    assert_curves_sound(c);
    assert_eq!(c.curve_count(), 25);
    assert!(c.curve(path_curve).is_some());
    assert!(c.curve(profile_curve).is_some());
    let new = new_curves(c, &before_curves);
    assert_eq!(new.len(), 23);
    // Section curves first, in sweep order, then one circle round the axis per profile
    // vertex, in the profile's loop order.
    for (k, (_, cv)) in new[..11].iter().enumerate() {
        let a = TAU * (k + 1) as f64 / 12.0;
        assert_vec(cv.center, v(2.5 * a.cos(), 2.5 * a.sin(), 0.5));
        assert!(close(cv.radius, 0.2));
    }
    for ((_, cv), pv) in new[11..].iter().zip(&profile_vertices) {
        assert!(cv.is_closed());
        assert!(parallel(cv.normal, Z));
        assert_eq!(cv.vertices[0], *pv);
    }

    undo.undo(&mut m).unwrap();
    assert_eq!(m, before);
    assert_eq!(curves(ctx(&m)), before_curves);
    undo.redo(&mut m).unwrap();
    assert_eq!(m, after);
}

#[test]
fn a_profile_corner_on_the_path_does_not_double_the_path_curve() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let (p, path_curve) = curve_path(&mut m, &ring12(), true);
    let old = ctx(&m).curve(path_curve).unwrap().clone();
    // A square standing on the ring's first corner, outside it, in that corner's mitre plane.
    let mut edit = ModelEdit::new();
    let f = m
        .edit_geometry(BodyId::Root, &mut edit, |c, log| {
            let mut vs = vec![p.vertices[0]];
            for q in [v(2.2, 0.0, 0.0), v(2.2, 0.0, 0.2), v(2.0, 0.0, 0.2)] {
                vs.push(c.insert_vertex(q, log));
            }
            let uses = (0..4)
                .map(|i| EdgeUse::forward(c.insert_edge(vs[i], vs[(i + 1) % 4], log).unwrap()))
                .collect();
            c.insert_face(
                Face {
                    outer: Loop { uses },
                    inner: Vec::new(),
                },
                log,
            )
            .unwrap()
        })
        .unwrap();
    let before = curves(ctx(&m));
    sweep(&mut m, &mut undo, f, &p).unwrap();
    let c = ctx(&m);
    assert_curves_sound(c);
    assert_eq!(c.curve(path_curve), Some(&old));
    // The corner on the path runs along the path's own edges: no second curve there.
    assert_eq!(new_curves(c, &before).len(), 3);
}

#[test]
fn swept_curves_survive_save_and_load() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let (p, _) = curve_path(&mut m, &ring12(), true);
    let (f, _) = radial_circle(&mut m);
    sweep(&mut m, &mut undo, f, &p).unwrap();
    let text = format::to_text(&m).unwrap();
    let back = format::from_text(&text).unwrap();
    let (a, b) = (curves(ctx(&m)), curves(ctx(&back)));
    assert_eq!(a.len(), b.len());
    for ((ia, ca), (ib, cb)) in a.iter().zip(&b) {
        assert_eq!(ia, ib);
        assert_eq!(ca.vertices, cb.vertices);
        assert_vec(ca.center, cb.center);
        assert!(close(ca.radius, cb.radius));
    }
    assert_curves_sound(ctx(&back));
}
