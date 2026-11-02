// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Curves: arcs and circles kept through edits (`docs/specs/m7-curves.md`).

use std::collections::BTreeSet;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

use drydock_kernel::curve::{self, Curve, MAX_CURVE_SEGMENTS};
use drydock_kernel::erase::erase;
use drydock_kernel::model::*;
use drydock_kernel::movemerge::transform_sticky;
use drydock_kernel::pushpull::push_pull;
use drydock_kernel::select::Entity;
use drydock_kernel::tools::arc::ArcTool;
use drydock_kernel::tools::arc3::Arc3Tool;
use drydock_kernel::tools::circle::CircleTool;
use drydock_kernel::tools::plane::DrawPlane;
use drydock_kernel::tools::*;
use drydock_kernel::topology::*;
use drydock_kernel::transform::transform;
use drydock_kernel::units::Unit;
use glam::{DMat4, DVec3};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[track_caller]
fn assert_near(a: f64, b: f64) {
    assert!(near(a, b), "{a} != {b}");
}

#[track_caller]
fn assert_close(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-9, "{a:?} != {b:?}");
}

fn xy() -> DrawPlane {
    DrawPlane {
        u: DVec3::X,
        v: DVec3::Y,
    }
}

fn typed(s: &str) -> Input {
    Input::Typed(s.to_string())
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

    fn target(&mut self) -> Target<'_> {
        Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: Unit::Meter,
        }
    }

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    fn curves(&self) -> Vec<(CurveId, Curve)> {
        self.ctx().curves().map(|(id, c)| (id, c.clone())).collect()
    }

    #[track_caller]
    fn only_curve(&self) -> (CurveId, Curve) {
        let all = self.curves();
        assert_eq!(all.len(), 1, "{all:?}");
        all[0].clone()
    }

    fn circle_in(&mut self, plane: DrawPlane, center: DVec3, rim: DVec3, segments: usize) {
        let mut t = CircleTool::new(plane);
        let mut tg = self.target();
        t.handle(typed(&format!("{segments}s")), &mut tg).unwrap();
        t.handle(Input::Click(center), &mut tg).unwrap();
        t.handle(Input::Click(rim), &mut tg).unwrap();
    }

    /// A circle in the XY plane, `segments` segments, first vertex at `center + (r, 0, 0)`.
    fn circle(&mut self, center: DVec3, r: f64, segments: usize) -> (CurveId, Curve) {
        self.circle_in(xy(), center, center + v(r, 0.0, 0.0), segments);
        self.only_curve()
    }

    fn line(&mut self, a: DVec3, b: DVec3) {
        draw_segments(&mut self.target(), &[(a, b)], "Line").unwrap();
    }

    fn position(&self, vertex: VertexId) -> DVec3 {
        self.ctx().vertex(vertex).unwrap().position
    }

    fn all_entities(&self) -> Vec<Entity> {
        let c = self.ctx();
        c.edges()
            .map(|(e, _)| Entity::Edge(e))
            .chain(c.faces().map(|(f, _)| Entity::Face(f)))
            .collect()
    }

    fn transform_all(&mut self, m: DMat4) {
        let sel = self.all_entities();
        transform(
            &mut self.model,
            BodyId::Root,
            &sel,
            m,
            &mut self.undo,
            "Move",
        )
        .unwrap();
    }

    fn erase(&mut self, edges: &[EdgeId]) {
        let sel: Vec<Entity> = edges.iter().map(|e| Entity::Edge(*e)).collect();
        erase(&mut self.model, BodyId::Root, &sel, &mut self.undo, "Erase").unwrap();
    }

    #[track_caller]
    fn valid(&self) {
        assert_eq!(self.model.validate(), Vec::new());
        for (id, c) in self.ctx().curves() {
            assert!(
                curve::fits(self.ctx(), c),
                "curve {id:?} doesn't fit: {c:?}"
            );
        }
    }
}

/// Every chain vertex is on the curve's circle and the chain runs from `start` to `end`.
#[track_caller]
fn assert_on_circle(w: &World, c: &Curve) {
    for vx in &c.vertices {
        let p = w.position(*vx);
        let d = p - c.center;
        assert!(d.dot(c.normal).abs() < 1e-9, "{p:?} off the plane");
        assert_near(d.length(), c.radius);
    }
    assert_close(w.position(c.vertices[0]), c.point_at(c.start));
    assert_close(w.position(*c.vertices.last().unwrap()), c.point_at(c.end));
}

fn curve_by_hand(radius: f64, start: f64, end: f64, closed: bool) -> Curve {
    let vertices = if closed {
        vec![VertexId(0), VertexId(1), VertexId(2), VertexId(0)]
    } else {
        vec![VertexId(0), VertexId(1), VertexId(2)]
    };
    Curve {
        center: DVec3::ZERO,
        normal: DVec3::Z,
        x_axis: DVec3::X,
        radius,
        start,
        end,
        vertices,
    }
}

fn sagitta(r: f64, sweep: f64, n: usize) -> f64 {
    2.0 * r * (sweep / (4.0 * n as f64)).sin().powi(2)
}

// ---------- drawing ----------

#[test]
fn the_circle_tool_makes_a_closed_curve() {
    let mut w = World::new();
    w.circle_in(xy(), v(1.0, 2.0, 0.0), v(4.0, 2.0, 0.0), 24);
    let (id, c) = w.only_curve();
    assert_close(c.center, v(1.0, 2.0, 0.0));
    assert_near(c.radius, 3.0);
    assert_close(c.normal, DVec3::Z);
    assert_close(c.x_axis, DVec3::X);
    assert_close(c.y_axis(), DVec3::Y);
    assert_eq!(c.start, 0.0);
    assert_near(c.end, TAU);
    assert_near(c.sweep(), TAU);
    assert!(c.is_closed());
    assert_eq!(c.vertices.len(), 25);
    assert_eq!(c.vertices[0], c.vertices[24]);
    assert_close(w.position(c.vertices[0]), v(4.0, 2.0, 0.0));
    assert_close(w.position(c.vertices[6]), v(1.0, 5.0, 0.0));
    assert_on_circle(&w, &c);

    let edges = curve::curve_edges(w.ctx(), id);
    assert_eq!(edges.len(), 24);
    let all: BTreeSet<EdgeId> = w.ctx().edges().map(|(e, _)| e).collect();
    assert_eq!(edges.iter().copied().collect::<BTreeSet<_>>(), all);
    for e in &edges {
        assert_eq!(curve::curve_of_edge(w.ctx(), *e), Some(id));
    }
    // the edges come in chain order
    let first = w.ctx().edge(edges[0]).unwrap().ends;
    assert!(first.contains(&c.vertices[0]) && first.contains(&c.vertices[1]));
    assert_eq!(w.undo.undo_count(), 1);
    assert_eq!(w.undo.undo_label(), Some("Circle"));
    w.valid();
}

#[test]
fn a_circle_in_a_vertical_plane_has_that_plane_normal() {
    let mut w = World::new();
    let plane = DrawPlane {
        u: DVec3::Y,
        v: DVec3::Z,
    };
    w.circle_in(plane, v(0.0, 0.0, 1.0), v(0.0, 2.0, 1.0), 12);
    let (_, c) = w.only_curve();
    assert_close(c.normal, plane.normal());
    assert_close(c.x_axis, DVec3::Y);
    assert_near(c.radius, 2.0);
    assert_eq!(c.vertices.len(), 13);
    assert_on_circle(&w, &c);
    w.valid();
}

#[test]
fn the_arc_tool_makes_an_open_curve() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    {
        let mut tg = w.target();
        t.handle(Input::Click(DVec3::ZERO), &mut tg).unwrap();
        t.handle(Input::Click(v(2.0, 0.0, 0.0)), &mut tg).unwrap();
        t.handle(Input::Click(v(0.0, 7.0, 0.0)), &mut tg).unwrap();
    }
    let (id, c) = w.only_curve();
    assert!(!c.is_closed());
    assert_eq!(c.vertices.len(), 13);
    assert_close(c.center, DVec3::ZERO);
    assert_near(c.radius, 2.0);
    assert_close(c.normal, DVec3::Z);
    assert_close(c.x_axis, DVec3::X);
    assert_eq!(c.start, 0.0);
    assert_near(c.end, FRAC_PI_2);
    assert_close(w.position(c.vertices[0]), v(2.0, 0.0, 0.0));
    assert_close(w.position(c.vertices[12]), v(0.0, 2.0, 0.0));
    assert_eq!(curve::curve_edges(w.ctx(), id).len(), 12);
    assert_on_circle(&w, &c);
    assert_eq!(w.undo.undo_count(), 1);
    w.valid();
}

#[test]
fn a_full_turn_arc_is_a_closed_curve() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    {
        let mut tg = w.target();
        t.handle(typed("6s"), &mut tg).unwrap();
        t.handle(Input::Click(DVec3::ZERO), &mut tg).unwrap();
        t.handle(Input::Click(v(1.0, 0.0, 0.0)), &mut tg).unwrap();
        t.handle(typed("360"), &mut tg).unwrap();
    }
    let (_, c) = w.only_curve();
    assert!(c.is_closed());
    assert_eq!(c.vertices.len(), 7);
    assert_near(c.sweep(), TAU);
    assert_on_circle(&w, &c);
    w.valid();
}

#[test]
fn a_one_segment_arc_is_not_a_curve() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    {
        let mut tg = w.target();
        t.handle(typed("1s"), &mut tg).unwrap();
        t.handle(Input::Click(DVec3::ZERO), &mut tg).unwrap();
        t.handle(Input::Click(v(1.0, 0.0, 0.0)), &mut tg).unwrap();
        t.handle(Input::Click(v(0.0, 1.0, 0.0)), &mut tg).unwrap();
    }
    assert_eq!(w.ctx().edge_count(), 1);
    assert_eq!(w.ctx().curve_count(), 0);
    assert_eq!(w.undo.undo_count(), 1);
}

#[test]
fn a_two_segment_arc_is_a_curve() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    {
        let mut tg = w.target();
        t.handle(typed("2s"), &mut tg).unwrap();
        t.handle(Input::Click(DVec3::ZERO), &mut tg).unwrap();
        t.handle(Input::Click(v(1.0, 0.0, 0.0)), &mut tg).unwrap();
        t.handle(typed("300"), &mut tg).unwrap();
    }
    let (_, c) = w.only_curve();
    assert_eq!(c.vertices.len(), 3);
    assert_near(c.end, 300f64.to_radians());
    w.valid();
}

#[test]
fn three_point_arcs_run_either_way() {
    // counter-clockwise about +Z from (1,0,0) over (0,1,0) to (-1,0,0)
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    {
        let mut tg = w.target();
        t.handle(Input::Click(v(1.0, 0.0, 0.0)), &mut tg).unwrap();
        t.handle(Input::Click(v(-1.0, 0.0, 0.0)), &mut tg).unwrap();
        t.handle(Input::Click(v(0.0, 1.0, 0.0)), &mut tg).unwrap();
    }
    let (_, c) = w.only_curve();
    assert_close(c.center, DVec3::ZERO);
    assert_near(c.radius, 1.0);
    assert_close(c.normal, DVec3::Z);
    assert_close(c.x_axis, DVec3::X);
    assert_eq!(c.start, 0.0);
    assert_near(c.end, PI);
    assert_eq!(c.vertices.len(), 13);
    assert_on_circle(&w, &c);
    w.valid();

    // through (0,-1,0) the chain runs clockwise about +Z, so the normal is -Z
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    {
        let mut tg = w.target();
        t.handle(Input::Click(v(1.0, 0.0, 0.0)), &mut tg).unwrap();
        t.handle(Input::Click(v(-1.0, 0.0, 0.0)), &mut tg).unwrap();
        t.handle(Input::Click(v(0.0, -1.0, 0.0)), &mut tg).unwrap();
    }
    let (_, c) = w.only_curve();
    assert_close(c.normal, -DVec3::Z);
    assert_close(c.x_axis, DVec3::X);
    assert_near(c.end, PI);
    assert_close(w.position(c.vertices[6]), v(0.0, -1.0, 0.0));
    assert_on_circle(&w, &c);
    w.valid();
}

#[test]
fn draw_curve_returns_the_curve_and_draws_anyway_when_there_is_none() {
    let mut w = World::new();
    let pts: Vec<DVec3> = (0..8)
        .map(|k| {
            let a = TAU * k as f64 / 8.0;
            v(a.cos(), a.sin(), 0.0)
        })
        .collect();
    let (chains, id) = draw_curve(&mut w.target(), &pts, true, "Circle").unwrap();
    assert_eq!(chains.len(), 8);
    let id = id.expect("a curve");
    assert!(w.ctx().curve(id).unwrap().is_closed());
    assert_eq!(w.undo.undo_count(), 1);

    // three points on a line: drawn, but no curve
    let mut w = World::new();
    let line = [v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(2.0, 0.0, 0.0)];
    let (chains, id) = draw_curve(&mut w.target(), &line, false, "Arc").unwrap();
    assert_eq!(chains.len(), 2);
    assert_eq!(id, None);
    assert_eq!(w.ctx().edge_count(), 2);
    assert_eq!(w.ctx().curve_count(), 0);
    assert_eq!(w.undo.undo_count(), 1);
}

#[test]
fn a_circle_drawn_across_a_line_keeps_its_curve() {
    let mut w = World::new();
    w.line(v(-3.0, 0.1, 0.0), v(3.0, 0.1, 0.0));
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 24);
    // the line splits two chords, so two steps are runs of two edges
    assert_eq!(curve::curve_edges(w.ctx(), id).len(), 26);
    assert!(c.is_closed());
    assert_on_circle(&w, &c);
    w.valid();
}

#[test]
fn undo_and_redo_of_a_drawn_circle() {
    let mut w = World::new();
    let (_, c) = w.circle(DVec3::ZERO, 1.0, 24);
    let after = w.model.clone();
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.ctx().curve_count(), 0);
    assert_eq!(w.ctx().edge_count(), 0);
    w.undo.redo(&mut w.model).unwrap();
    assert_eq!(w.model, after);
    assert_eq!(w.only_curve().1, c);
}

// ---------- add_curve, fit, fits ----------

#[test]
fn add_curve_refuses_what_is_not_a_curve() {
    let mut w = World::new();
    let pts: Vec<DVec3> = (0..6)
        .map(|k| {
            let a = TAU * k as f64 / 6.0;
            v(a.cos(), a.sin(), 0.0)
        })
        .collect();
    let segs: Vec<(DVec3, DVec3)> = (0..6).map(|k| (pts[k], pts[(k + 1) % 6])).collect();
    draw_segments(&mut w.target(), &segs, "Lines").unwrap();
    assert_eq!(w.ctx().curve_count(), 0);

    let mut log = ModelEdit::new();
    let before = w.model.clone();
    let refused = w
        .model
        .edit_geometry(BodyId::Root, &mut log, |ctx, log| {
            vec![
                // fewer than three points
                curve::add_curve(ctx, &pts[..2], false, log),
                // a point with no vertex
                curve::add_curve(ctx, &[pts[0], pts[1], v(5.0, 5.0, 0.0)], false, log),
                // two points with no edge between them
                curve::add_curve(ctx, &[pts[0], pts[2], pts[4]], true, log),
            ]
        })
        .unwrap();
    assert_eq!(refused, vec![None, None, None]);
    assert!(log.is_empty());
    assert_eq!(w.model, before);

    // the hexagon is a circle; once it is one, it can't be made a second time
    let mut log = ModelEdit::new();
    let made = w
        .model
        .edit_geometry(BodyId::Root, &mut log, |ctx, log| {
            let first = curve::add_curve(ctx, &pts, true, log);
            let again = curve::add_curve(ctx, &pts, true, log);
            let arc = curve::add_curve(ctx, &pts[1..4], false, log);
            (first, again, arc)
        })
        .unwrap();
    assert!(made.0.is_some());
    assert_eq!(made.1, None);
    assert_eq!(made.2, None);
    assert_eq!(w.ctx().curve_count(), 1);
    w.valid();
}

#[test]
fn add_curve_closes_a_chain_that_ends_where_it_starts() {
    let mut w = World::new();
    let mut pts: Vec<DVec3> = (0..5)
        .map(|k| {
            let a = TAU * k as f64 / 5.0;
            v(2.0 * a.cos(), 2.0 * a.sin(), 1.0)
        })
        .collect();
    pts.push(pts[0]);
    let segs: Vec<(DVec3, DVec3)> = pts.windows(2).map(|p| (p[0], p[1])).collect();
    draw_segments(&mut w.target(), &segs, "Lines").unwrap();
    let mut log = ModelEdit::new();
    let id = w
        .model
        .edit_geometry(BodyId::Root, &mut log, |ctx, log| {
            curve::add_curve(ctx, &pts, false, log)
        })
        .unwrap()
        .unwrap();
    let c = w.ctx().curve(id).unwrap().clone();
    assert!(c.is_closed());
    assert_eq!(c.vertices.len(), 6);
    assert_close(c.center, v(0.0, 0.0, 1.0));
    assert_near(c.radius, 2.0);
    assert_close(c.normal, DVec3::Z);
    assert_eq!(c.start, 0.0);
    assert_near(c.end, TAU);
    w.valid();
}

#[test]
fn fit_gives_back_the_drawn_circle() {
    let mut w = World::new();
    w.circle_in(xy(), v(1.0, -1.0, 0.5), v(1.0, 1.5, 0.5), 16);
    let (_, c) = w.only_curve();
    let again = curve::fit(w.ctx(), &c.vertices, 0.0).unwrap();
    assert_close(again.center, c.center);
    assert_close(again.normal, c.normal);
    assert_close(again.x_axis, DVec3::Y);
    assert_near(again.radius, 2.5);
    assert_eq!(again.start, 0.0);
    assert_near(again.end, TAU);
    assert_eq!(again.vertices, c.vertices);

    // a start angle turns the x axis back by that angle
    let turned = curve::fit(w.ctx(), &c.vertices, 1.0).unwrap();
    assert_eq!(turned.start, 1.0);
    assert_near(turned.end, 1.0 + TAU);
    let x = DVec3::Y * 1f64.cos() - DVec3::Z.cross(DVec3::Y) * 1f64.sin();
    assert_close(turned.x_axis, x);
    assert_close(turned.point_at(1.0), w.position(c.vertices[0]));
    assert!(curve::fits(w.ctx(), &turned));

    // an open part of the chain
    let part = curve::fit(w.ctx(), &c.vertices[2..7], 0.0).unwrap();
    assert!(!part.is_closed());
    assert_near(part.end, 4.0 * TAU / 16.0);
    assert_close(part.center, c.center);
    assert!(curve::fits(w.ctx(), &part));
}

#[test]
fn fit_refuses_lines_short_chains_and_missing_vertices() {
    let mut w = World::new();
    let line = [v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(2.0, 0.0, 0.0)];
    draw_segments(
        &mut w.target(),
        &[(line[0], line[1]), (line[1], line[2])],
        "L",
    )
    .unwrap();
    let ids: Vec<VertexId> = line
        .iter()
        .map(|p| w.ctx().find_vertex(*p, 1e-9).unwrap())
        .collect();
    assert_eq!(curve::fit(w.ctx(), &ids, 0.0), None);
    assert_eq!(curve::fit(w.ctx(), &ids[..2], 0.0), None);

    let mut w = World::new();
    let (_, c) = w.circle(DVec3::ZERO, 1.0, 8);
    assert_eq!(
        curve::fit(w.ctx(), &[c.vertices[0], c.vertices[1]], 0.0),
        None
    );
    let mut gone = c.vertices.clone();
    gone[3] = VertexId(9999);
    assert_eq!(curve::fit(w.ctx(), &gone, 0.0), None);
    assert_eq!(curve::fit(w.ctx(), &c.vertices, f64::NAN), None);
    // reversed: a curve again, about the opposite normal
    let mut back = c.vertices.clone();
    back.reverse();
    let r = curve::fit(w.ctx(), &back, 0.0).unwrap();
    assert_close(r.normal, -DVec3::Z);
}

#[test]
fn fits_checks_numbers_order_and_steps() {
    let mut w = World::new();
    let (_, c) = w.circle(DVec3::ZERO, 1.0, 12);
    let ctx = w.ctx();
    assert!(curve::fits(ctx, &c));

    let mut bad = c.clone();
    bad.radius = 1.1;
    assert!(!curve::fits(ctx, &bad), "vertices off the circle");
    let mut bad = c.clone();
    bad.center = v(0.0, 0.0, 0.01);
    assert!(!curve::fits(ctx, &bad), "vertices off the plane");
    let mut bad = c.clone();
    bad.normal = v(0.0, 0.0, 2.0);
    assert!(!curve::fits(ctx, &bad), "normal not unit");
    let mut bad = c.clone();
    bad.x_axis = v(1.0, 0.0, 0.0001).normalize();
    assert!(!curve::fits(ctx, &bad), "x axis not in the plane");
    let mut bad = c.clone();
    bad.end = bad.start + PI;
    assert!(!curve::fits(ctx, &bad), "a closed curve sweeps a full turn");
    let mut bad = c.clone();
    bad.start = 0.3;
    bad.end = 0.3 + TAU;
    assert!(!curve::fits(ctx, &bad), "first vertex not at start");
    let mut bad = c.clone();
    bad.radius = f64::NAN;
    assert!(!curve::fits(ctx, &bad));
    let mut bad = c.clone();
    bad.vertices.swap(3, 4);
    assert!(!curve::fits(ctx, &bad), "angles must increase");
    let mut bad = c.clone();
    bad.vertices.remove(5);
    assert!(!curve::fits(ctx, &bad), "no edge for the step");
    let mut bad = c.clone();
    bad.vertices[5] = VertexId(9999);
    assert!(!curve::fits(ctx, &bad), "missing vertex");
    let mut bad = c.clone();
    bad.vertices.truncate(3);
    bad.vertices.push(bad.vertices[0]);
    assert!(!curve::fits(ctx, &bad));

    // an open run of the circle
    let mut part = c.clone();
    part.vertices = c.vertices[0..5].to_vec();
    part.end = 4.0 * TAU / 12.0;
    assert!(curve::fits(ctx, &part));
    part.end = 4.0 * TAU / 12.0 + TAU;
    assert!(
        !curve::fits(ctx, &part),
        "an open curve sweeps less than a turn"
    );
    let mut two = c.clone();
    two.vertices = c.vertices[0..2].to_vec();
    two.end = TAU / 12.0;
    assert!(!curve::fits(ctx, &two), "a curve has at least two steps");
}

#[test]
fn validate_reports_a_curve_that_no_longer_fits() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 12);
    // Context::apply doesn't reconcile; validate must notice.
    let mut ctx = w.ctx().clone();
    let p = ctx.vertex(c.vertices[3]).unwrap().position;
    ctx.apply(&Record::MoveVertex {
        id: c.vertices[3],
        from: p,
        to: p * 1.2,
    })
    .unwrap();
    assert!(ctx.validate().contains(&Violation::BadCurve(id)));
}

#[test]
fn curve_records_invert_and_check_ids() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 12);
    let mut ctx = w.ctx().clone();
    assert_eq!(
        Record::AddCurve(id, c.clone()).inverse(),
        Record::RemoveCurve(id, c.clone())
    );
    assert_eq!(
        ctx.apply(&Record::AddCurve(id, c.clone())),
        Err(TopoError::IdInUse)
    );
    let mut other = c.clone();
    other.radius = 2.0;
    assert_eq!(
        ctx.apply(&Record::RemoveCurve(id, other)),
        Err(TopoError::Mismatch)
    );
    let mut log = Edit::new();
    ctx.delete_curve(id, &mut log).unwrap();
    assert_eq!(ctx.curve_count(), 0);
    assert_eq!(
        ctx.delete_curve(id, &mut log),
        Err(TopoError::MissingCurve(id))
    );
    ctx.apply_edit(&log.inverse()).unwrap();
    assert_eq!(ctx.curve(id), Some(&c));
    assert_eq!(&ctx, w.ctx());
}

// ---------- keeping curves through edits ----------

#[test]
fn moving_the_circle_moves_its_center() {
    let mut w = World::new();
    let (id, c) = w.circle(v(1.0, 1.0, 0.0), 2.0, 24);
    let t = v(3.0, -2.0, 5.0);
    w.transform_all(DMat4::from_translation(t));
    let (id2, moved) = w.only_curve();
    assert_eq!(id2, id);
    assert_eq!(moved.vertices, c.vertices);
    assert_close(moved.center, c.center + t);
    assert_close(moved.normal, c.normal);
    assert_close(moved.x_axis, c.x_axis);
    assert_near(moved.radius, c.radius);
    assert_near(moved.start, c.start);
    assert_near(moved.end, c.end);
    assert_on_circle(&w, &moved);
    w.valid();

    // undo puts the curve back exactly, redo moves it again exactly
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.only_curve().1, c);
    w.undo.redo(&mut w.model).unwrap();
    assert_eq!(w.only_curve().1, moved);
}

#[test]
fn sticky_move_keeps_the_curve() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 24);
    let sel = w.all_entities();
    let t = v(0.0, 0.0, 2.0);
    transform_sticky(
        &mut w.model,
        BodyId::Root,
        &sel,
        DMat4::from_translation(t),
        &mut w.undo,
        "Move",
    )
    .unwrap();
    let (id2, moved) = w.only_curve();
    assert_eq!(id2, id);
    assert_close(moved.center, t);
    assert_near(moved.radius, 1.0);
    assert_on_circle(&w, &moved);
    w.valid();
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.only_curve().1, c);
}

#[test]
fn rotating_about_its_own_axis_turns_the_x_axis_and_keeps_the_angles() {
    let mut w = World::new();
    let center = v(2.0, 0.0, 0.0);
    let (_, c) = w.circle(center, 1.0, 24);
    let m = DMat4::from_translation(center)
        * DMat4::from_rotation_z(FRAC_PI_2)
        * DMat4::from_translation(-center);
    w.transform_all(m);
    let (_, r) = w.only_curve();
    assert_close(r.center, center);
    assert_close(r.normal, DVec3::Z);
    assert_close(r.x_axis, DVec3::Y);
    assert_eq!(r.start, c.start);
    assert_near(r.end, c.end);
    assert_eq!(r.vertices, c.vertices);
    assert_on_circle(&w, &r);
    w.valid();
}

#[test]
fn rotating_an_arc_tilts_its_plane() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    {
        let mut tg = w.target();
        t.handle(Input::Click(v(1.0, 1.0, 0.0)), &mut tg).unwrap();
        t.handle(Input::Click(v(1.0, 3.0, 0.0)), &mut tg).unwrap();
        t.handle(typed("135"), &mut tg).unwrap();
    }
    let (_, a) = w.only_curve();
    assert_close(a.x_axis, DVec3::Y);
    w.transform_all(DMat4::from_rotation_x(FRAC_PI_2));
    let (_, r) = w.only_curve();
    assert_close(r.center, v(1.0, 0.0, 1.0));
    assert_close(r.normal, v(0.0, -1.0, 0.0));
    assert_close(r.x_axis, DVec3::Z);
    assert_near(r.radius, 2.0);
    assert_eq!(r.start, a.start);
    assert_near(r.end, 135f64.to_radians());
    assert_on_circle(&w, &r);
    w.valid();
}

#[test]
fn uniform_scale_scales_the_radius() {
    let mut w = World::new();
    let (_, c) = w.circle(v(1.0, 0.0, 0.0), 0.5, 24);
    w.transform_all(DMat4::from_scale(DVec3::splat(3.0)));
    let (_, s) = w.only_curve();
    assert_close(s.center, v(3.0, 0.0, 0.0));
    assert_near(s.radius, 1.5);
    assert_close(s.normal, c.normal);
    assert_close(s.x_axis, c.x_axis);
    assert_on_circle(&w, &s);
    w.valid();
}

#[test]
fn a_mirror_flips_the_normal_and_keeps_the_angles() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    {
        let mut tg = w.target();
        t.handle(Input::Click(v(1.0, 0.0, 0.0)), &mut tg).unwrap();
        t.handle(Input::Click(v(2.0, 0.0, 0.0)), &mut tg).unwrap();
        t.handle(typed("90"), &mut tg).unwrap();
    }
    let (_, a) = w.only_curve();
    w.transform_all(DMat4::from_scale(v(-1.0, 1.0, 1.0)));
    let (_, m) = w.only_curve();
    assert_close(m.center, v(-1.0, 0.0, 0.0));
    assert_close(m.normal, -DVec3::Z);
    assert_close(m.x_axis, -DVec3::X);
    assert_eq!(m.start, a.start);
    assert_near(m.end, a.end);
    assert_close(w.position(m.vertices[0]), v(-2.0, 0.0, 0.0));
    assert_on_circle(&w, &m);
    w.valid();
}

#[test]
fn stretching_one_way_makes_plain_edges() {
    let mut w = World::new();
    w.circle(DVec3::ZERO, 1.0, 24);
    let edges = w.ctx().edge_count();
    w.transform_all(DMat4::from_scale(v(1.5, 1.0, 1.0)));
    assert_eq!(w.ctx().curve_count(), 0);
    assert_eq!(w.ctx().edge_count(), edges);
    w.valid();
    // undo brings the curve back
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.ctx().curve_count(), 1);
    w.valid();
}

#[test]
fn stretching_along_the_normal_changes_nothing() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 24);
    w.transform_all(DMat4::from_scale(v(1.0, 1.0, 3.0)));
    assert_eq!(w.only_curve(), (id, c));
}

#[test]
fn moving_one_edge_makes_plain_edges() {
    let mut w = World::new();
    let (id, _) = w.circle(DVec3::ZERO, 1.0, 24);
    let e = curve::curve_edges(w.ctx(), id)[0];
    let out = v(7.5f64.to_radians().cos(), 7.5f64.to_radians().sin(), 0.0) * 0.2;
    transform(
        &mut w.model,
        BodyId::Root,
        &[Entity::Edge(e)],
        DMat4::from_translation(out),
        &mut w.undo,
        "Move",
    )
    .unwrap();
    assert_eq!(w.ctx().curve_count(), 0);
    assert_eq!(w.ctx().edge_count(), 24);
    w.valid();
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.ctx().curve_count(), 1);
}

#[test]
fn a_line_across_a_chord_keeps_the_curve() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 24);
    // a diameter through the middles of steps 0 and 12 splits both chords and the face
    let d = v(7.5f64.to_radians().cos(), 7.5f64.to_radians().sin(), 0.0);
    w.line(d * -2.0, d * 2.0);
    assert_eq!(w.only_curve(), (id, c.clone()));
    let edges = curve::curve_edges(w.ctx(), id);
    assert_eq!(edges.len(), 26);
    for e in &edges {
        assert_eq!(curve::curve_of_edge(w.ctx(), *e), Some(id));
    }
    // the diameter's own edges are not part of the curve
    let all: BTreeSet<EdgeId> = w.ctx().edges().map(|(e, _)| e).collect();
    let mine: BTreeSet<EdgeId> = edges.iter().copied().collect();
    for e in all.difference(&mine) {
        assert_eq!(curve::curve_of_edge(w.ctx(), *e), None);
    }
    assert_eq!(w.ctx().face_count(), 2);
    w.valid();

    // erasing half of a split chord breaks that step: the rest is an arc
    let half = edges[0];
    w.erase(&[half]);
    let (_, arc) = w.only_curve();
    assert!(!arc.is_closed());
    assert_eq!(arc.vertices.len(), 24);
    assert_eq!(arc.vertices[0], c.vertices[1]);
    assert_eq!(arc.vertices[23], c.vertices[0]);
    w.valid();
}

#[test]
fn a_line_from_a_curve_vertex_changes_nothing() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 24);
    w.line(v(1.0, 0.0, 0.0), v(3.0, 0.0, 0.0));
    w.line(v(1.0, 0.0, 0.0), v(-1.0, 0.0, 0.0));
    assert_eq!(w.only_curve(), (id, c));
    assert_eq!(curve::curve_edges(w.ctx(), id).len(), 24);
    w.valid();
}

#[test]
fn erasing_one_edge_leaves_an_arc() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 24);
    let before = w.model.clone();
    let e = curve::curve_edges(w.ctx(), id)[0];
    w.erase(&[e]);
    let (id2, a) = w.only_curve();
    assert_eq!(id2, id);
    assert!(!a.is_closed());
    assert_eq!(a.vertices.len(), 24);
    assert_eq!(a.vertices[..23], c.vertices[1..24]);
    assert_eq!(a.vertices[23], c.vertices[0]);
    assert_near(a.start, TAU / 24.0);
    assert_near(a.end, TAU);
    assert_eq!(a.center, c.center);
    assert_eq!(a.normal, c.normal);
    assert_eq!(a.x_axis, c.x_axis);
    assert_eq!(a.radius, c.radius);
    assert_on_circle(&w, &a);
    w.valid();

    let after = w.model.clone();
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.model, before);
    w.undo.redo(&mut w.model).unwrap();
    assert_eq!(w.model, after);
}

#[test]
fn erasing_two_edges_leaves_two_arcs() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 24);
    let edges = curve::curve_edges(w.ctx(), id);
    w.erase(&[edges[3], edges[15]]);
    let arcs = w.curves();
    assert_eq!(arcs.len(), 2);
    assert!(arcs.iter().any(|(i, _)| *i == id));
    let mut starts = Vec::new();
    for (_, a) in &arcs {
        assert!(!a.is_closed());
        assert_eq!(a.vertices.len(), 12);
        assert_near(a.sweep(), 11.0 * TAU / 24.0);
        assert_eq!(a.center, c.center);
        assert_eq!(a.x_axis, c.x_axis);
        assert_on_circle(&w, a);
        starts.push(a.vertices[0]);
    }
    starts.sort();
    let mut want = vec![c.vertices[4], c.vertices[16]];
    want.sort();
    assert_eq!(starts, want);
    w.valid();
}

#[test]
fn a_piece_of_one_step_is_not_a_curve() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 24);
    let edges = curve::curve_edges(w.ctx(), id);
    w.erase(&[edges[0], edges[2]]);
    let (_, a) = w.only_curve();
    assert_eq!(a.vertices.len(), 22);
    assert_eq!(a.vertices[0], c.vertices[3]);
    assert_near(a.start, 3.0 * TAU / 24.0);
    assert_near(a.end, TAU);
    // the lone step between them is a plain edge
    assert_eq!(curve::curve_of_edge(w.ctx(), edges[1]), None);
    w.valid();
}

#[test]
fn erasing_from_an_arc_splits_it_by_chain_order() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    {
        let mut tg = w.target();
        t.handle(typed("10s"), &mut tg).unwrap();
        t.handle(Input::Click(DVec3::ZERO), &mut tg).unwrap();
        t.handle(Input::Click(v(1.0, 0.0, 0.0)), &mut tg).unwrap();
        t.handle(typed("200"), &mut tg).unwrap();
    }
    let (id, a) = w.only_curve();
    let edges = curve::curve_edges(w.ctx(), id);
    w.erase(&[edges[4]]);
    let arcs = w.curves();
    assert_eq!(arcs.len(), 2);
    // the first run keeps the id
    let first = arcs.iter().find(|(i, _)| *i == id).unwrap().1.clone();
    assert_eq!(first.vertices, a.vertices[0..5].to_vec());
    assert_eq!(first.start, a.start);
    assert_near(first.end, 4.0 * 20f64.to_radians());
    let second = arcs.iter().find(|(i, _)| *i != id).unwrap().1.clone();
    assert_eq!(second.vertices, a.vertices[5..].to_vec());
    assert_near(second.start, 5.0 * 20f64.to_radians());
    assert_near(second.end, 200f64.to_radians());
    w.valid();
}

#[test]
fn erasing_the_last_two_steps_drops_the_curve() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    {
        let mut tg = w.target();
        t.handle(typed("3s"), &mut tg).unwrap();
        t.handle(Input::Click(DVec3::ZERO), &mut tg).unwrap();
        t.handle(Input::Click(v(1.0, 0.0, 0.0)), &mut tg).unwrap();
        t.handle(typed("90"), &mut tg).unwrap();
    }
    let (id, _) = w.only_curve();
    let edges = curve::curve_edges(w.ctx(), id);
    w.erase(&[edges[1]]);
    assert_eq!(w.ctx().curve_count(), 0);
    assert_eq!(w.ctx().edge_count(), 2);
    w.valid();
}

#[test]
fn push_pull_keeps_the_base_curve() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 24);
    let face = w.ctx().faces().next().unwrap().0;
    push_pull(
        &mut w.model,
        BodyId::Root,
        face,
        2.0,
        &mut w.undo,
        "Push/Pull",
    )
    .unwrap();
    assert_eq!(w.ctx().curve(id), Some(&c));
    w.valid();
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.only_curve(), (id, c));
}

#[test]
fn curves_in_a_definition_body() {
    let mut w = World::new();
    let mut log = ModelEdit::new();
    let def = w.model.add_definition(
        DefinitionInfo {
            name: "Part".to_string(),
            group: false,
        },
        &mut log,
    );
    let body = BodyId::Definition(def);
    let mut t = CircleTool::new(xy());
    {
        let mut tg = Target {
            model: &mut w.model,
            body,
            undo: &mut w.undo,
            unit: Unit::Meter,
        };
        t.handle(Input::Click(DVec3::ZERO), &mut tg).unwrap();
        t.handle(Input::Click(v(0.5, 0.0, 0.0)), &mut tg).unwrap();
    }
    let ctx = w.model.body(body).unwrap().geometry();
    assert_eq!(ctx.curve_count(), 1);
    assert_eq!(w.ctx().curve_count(), 0);
    let sel: Vec<Entity> = ctx.edges().map(|(e, _)| Entity::Edge(e)).collect();
    transform(
        &mut w.model,
        body,
        &sel,
        DMat4::from_translation(v(0.0, 0.0, 1.0)),
        &mut w.undo,
        "Move",
    )
    .unwrap();
    let ctx = w.model.body(body).unwrap().geometry();
    let c = ctx.curves().next().unwrap().1;
    assert_close(c.center, v(0.0, 0.0, 1.0));
    assert!(w.model.validate().is_empty());
}

#[test]
fn an_unrelated_edit_writes_no_curve_records() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 24);
    w.line(v(5.0, 0.0, 0.0), v(6.0, 0.0, 0.0));
    let far = w.ctx().find_vertex(v(6.0, 0.0, 0.0), 1e-9).unwrap();
    let mut log = ModelEdit::new();
    w.model
        .edit_geometry(BodyId::Root, &mut log, |ctx, log| {
            ctx.set_vertex_position(far, v(6.0, 1.0, 0.0), log).unwrap();
        })
        .unwrap();
    assert_eq!(log.records.len(), 1);
    // moving a curve vertex onto the spot it already has changes nothing either
    let p = w.position(c.vertices[2]);
    let mut log = ModelEdit::new();
    w.model
        .edit_geometry(BodyId::Root, &mut log, |ctx, log| {
            ctx.set_vertex_position(c.vertices[2], p, log).unwrap();
        })
        .unwrap();
    assert_eq!(log.records.len(), 1);
    assert_eq!(w.only_curve(), (id, c));
}

#[test]
fn a_rigid_move_rewrites_the_curve_under_the_same_id() {
    let mut w = World::new();
    let (id, c) = w.circle(DVec3::ZERO, 1.0, 6);
    let mut log = ModelEdit::new();
    w.model
        .edit_geometry(BodyId::Root, &mut log, |ctx, log| {
            for vx in &c.vertices[..6] {
                let p = ctx.vertex(*vx).unwrap().position;
                ctx.set_vertex_position(*vx, p + DVec3::Z, log).unwrap();
            }
        })
        .unwrap();
    let curve_records: Vec<&ModelRecord> = log
        .records
        .iter()
        .filter(|r| {
            matches!(
                r,
                ModelRecord::Geometry(_, Record::AddCurve(..) | Record::RemoveCurve(..))
            )
        })
        .collect();
    assert_eq!(curve_records.len(), 2);
    assert!(matches!(
        curve_records[0],
        ModelRecord::Geometry(_, Record::RemoveCurve(i, old)) if *i == id && *old == c
    ));
    assert!(matches!(
        curve_records[1],
        ModelRecord::Geometry(_, Record::AddCurve(i, _)) if *i == id
    ));
    assert_close(w.only_curve().1.center, DVec3::Z);
}

// ---------- segments for export ----------

#[test]
fn segments_for_a_tolerance() {
    let circle = curve_by_hand(1.0, 0.0, TAU, true);
    assert_eq!(circle.segments_for(0.01), 23);
    assert!(sagitta(1.0, TAU, 23) <= 0.01);
    assert!(sagitta(1.0, TAU, 22) > 0.01);
    // below the minimum tolerance, or not a number: 1 µm
    assert_eq!(circle.segments_for(0.0), 2222);
    assert_eq!(circle.segments_for(-1.0), 2222);
    assert_eq!(circle.segments_for(f64::NAN), 2222);
    assert_eq!(circle.segments_for(1e-7), 2222);
    // a closed curve has at least 3 segments, an open one at least 1
    assert_eq!(circle.segments_for(10.0), 3);
    let quarter = curve_by_hand(2.0, 0.0, FRAC_PI_2, false);
    assert_eq!(quarter.segments_for(1.0), 1);
    assert_eq!(quarter.segments_for(0.5), 2);
    // never more than the cap
    let huge = curve_by_hand(1e6, 0.0, TAU, true);
    assert_eq!(huge.segments_for(1e-6), MAX_CURVE_SEGMENTS);
}

#[test]
fn points_for_a_tolerance() {
    let mut arc = curve_by_hand(2.0, 0.5, 2.0, false);
    arc.center = v(1.0, 1.0, 1.0);
    arc.normal = DVec3::Y;
    arc.x_axis = DVec3::Z;
    let n = arc.segments_for(0.001);
    let pts = arc.points(0.001);
    assert_eq!(pts.len(), n + 1);
    assert_eq!(pts[0], arc.point_at(0.5));
    assert_eq!(pts[n], arc.point_at(2.0));
    for (k, p) in pts.iter().enumerate() {
        assert_close(*p, arc.point_at(0.5 + 1.5 * k as f64 / n as f64));
    }

    let circle = curve_by_hand(1.0, 1.0, 1.0 + TAU, true);
    let pts = circle.points(0.01);
    assert_eq!(pts.len(), 24);
    assert_eq!(pts[0], circle.point_at(1.0));
    assert_eq!(pts[23], pts[0]);
}
