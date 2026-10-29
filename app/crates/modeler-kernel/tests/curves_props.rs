// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Invariants of curves through edits (`docs/specs/m7-curves.md`).

use std::f64::consts::TAU;

use glam::{DMat4, DQuat, DVec3};
use modeler_kernel::curve::{self, Curve, MAX_CURVE_SEGMENTS};
use modeler_kernel::erase::erase;
use modeler_kernel::format::{from_text, to_text};
use modeler_kernel::geom::plane_basis;
use modeler_kernel::model::*;
use modeler_kernel::select::Entity;
use modeler_kernel::tools::arc::ArcTool;
use modeler_kernel::tools::circle::CircleTool;
use modeler_kernel::tools::plane::DrawPlane;
use modeler_kernel::tools::*;
use modeler_kernel::topology::*;
use modeler_kernel::transform::transform;
use modeler_kernel::units::Unit;
use proptest::prelude::*;

#[derive(Clone, Debug)]
struct Shape {
    center: DVec3,
    normal: DVec3,
    radius: f64,
    /// `None`: a circle; `Some(degrees)`: an arc of that sweep.
    sweep: Option<f64>,
    segments: usize,
    /// Angle of the first vertex in the drawing plane.
    turn: f64,
}

fn unit() -> impl Strategy<Value = DVec3> {
    (0.0..TAU, -1.0f64..1.0).prop_map(|(a, z)| {
        let r = (1.0 - z * z).sqrt();
        DVec3::new(r * a.cos(), r * a.sin(), z)
    })
}

fn shape() -> impl Strategy<Value = Shape> {
    (
        (-5.0..5.0f64, -5.0..5.0f64, -5.0..5.0f64),
        unit(),
        0.05..5.0f64,
        prop_oneof![
            1 => Just(None),
            1 => (20.0..350.0f64).prop_map(|d| Some((d * 8.0).round() / 8.0)),
        ],
        2usize..40,
        0.0..TAU,
    )
        .prop_map(|((x, y, z), normal, radius, sweep, segments, turn)| Shape {
            center: DVec3::new(x, y, z),
            normal,
            radius,
            sweep,
            segments: if sweep.is_none() {
                segments.max(3)
            } else {
                segments
            },
            turn,
        })
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

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    fn draw(&mut self, s: &Shape) {
        let (u, v) = plane_basis(s.normal);
        let plane = DrawPlane { u, v };
        let rim = s.center + (u * s.turn.cos() + v * s.turn.sin()) * s.radius;
        let mut tg = Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: Unit::Meter,
        };
        let segs = Input::Typed(format!("{}s", s.segments));
        match s.sweep {
            None => {
                let mut t = CircleTool::new(plane);
                t.handle(segs, &mut tg).unwrap();
                t.handle(Input::Click(s.center), &mut tg).unwrap();
                t.handle(Input::Click(rim), &mut tg).unwrap();
            }
            Some(deg) => {
                let mut t = ArcTool::new(plane);
                t.handle(segs, &mut tg).unwrap();
                t.handle(Input::Click(s.center), &mut tg).unwrap();
                t.handle(Input::Click(rim), &mut tg).unwrap();
                t.handle(Input::Typed(format!("{deg}")), &mut tg).unwrap();
            }
        }
    }

    fn only_curve(&self) -> (CurveId, Curve) {
        let all: Vec<(CurveId, Curve)> = self.ctx().curves().map(|(i, c)| (i, c.clone())).collect();
        assert_eq!(all.len(), 1, "{all:?}");
        all[0].clone()
    }

    fn everything(&self) -> Vec<Entity> {
        self.ctx()
            .edges()
            .map(|(e, _)| Entity::Edge(e))
            .chain(self.ctx().faces().map(|(f, _)| Entity::Face(f)))
            .collect()
    }

    fn sound(&self) -> Result<(), TestCaseError> {
        prop_assert_eq!(self.model.validate(), Vec::new());
        for (_, c) in self.ctx().curves() {
            prop_assert!(curve::fits(self.ctx(), c));
        }
        Ok(())
    }
}

/// A similarity: rotation, uniform scale, optional mirror, translation.
fn similarity() -> impl Strategy<Value = (DMat4, f64, bool)> {
    (
        unit(),
        -3.2..3.2f64,
        0.2..5.0f64,
        any::<bool>(),
        (-4.0..4.0f64, -4.0..4.0f64, -4.0..4.0f64),
    )
        .prop_map(|(axis, angle, scale, mirror, (x, y, z))| {
            let flip = if mirror {
                DMat4::from_scale(DVec3::new(-1.0, 1.0, 1.0))
            } else {
                DMat4::IDENTITY
            };
            let m = DMat4::from_translation(DVec3::new(x, y, z))
                * DMat4::from_quat(DQuat::from_axis_angle(axis, angle))
                * flip
                * DMat4::from_scale(DVec3::splat(scale));
            (m, scale, mirror)
        })
}

fn close(a: DVec3, b: DVec3, tol: f64) -> bool {
    a.distance(b) <= tol
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// Drawn shapes are curves through their vertices, with the drawing plane's normal.
    #[test]
    fn drawn_shapes_are_curves(s in shape()) {
        let mut w = World::new();
        w.draw(&s);
        let (id, c) = w.only_curve();
        w.sound()?;
        prop_assert!(close(c.center, s.center, 1e-9));
        prop_assert!((c.radius - s.radius).abs() <= 1e-9);
        prop_assert!(close(c.normal, s.normal, 1e-9));
        prop_assert_eq!(c.start, 0.0);
        match s.sweep {
            None => {
                prop_assert!(c.is_closed());
                prop_assert_eq!(c.vertices.len(), s.segments + 1);
            }
            Some(deg) => {
                prop_assert!(!c.is_closed());
                prop_assert!((c.end - deg.to_radians()).abs() <= 1e-9);
                if s.segments >= 2 {
                    prop_assert_eq!(c.vertices.len(), s.segments + 1);
                }
            }
        }
        prop_assert_eq!(curve::curve_edges(w.ctx(), id).len(), c.vertices.len() - 1);
    }

    /// Rigid motions, uniform scales and mirrors carry the curve along: center and frame
    /// map with the matrix, the radius scales, the angles stay. Undo and redo are exact.
    #[test]
    fn similarities_carry_the_curve((s, (m, scale, mirror)) in (shape(), similarity())) {
        prop_assume!(s.sweep.is_none() || s.segments >= 2);
        let mut w = World::new();
        w.draw(&s);
        let (id, c) = w.only_curve();
        let sel = w.everything();
        transform(&mut w.model, BodyId::Root, &sel, m, &mut w.undo, "Move").unwrap();
        let (id2, t) = w.only_curve();
        prop_assert_eq!(id2, id);
        prop_assert_eq!(&t.vertices, &c.vertices);
        w.sound()?;

        let size = 1.0 + m.transform_point3(c.center).length();
        prop_assert!(close(t.center, m.transform_point3(c.center), 1e-9 * size));
        prop_assert!((t.radius - c.radius * scale).abs() <= 1e-9 * c.radius * scale);
        let sign = if mirror { -1.0 } else { 1.0 };
        let n = m.transform_vector3(c.normal).normalize() * sign;
        prop_assert!(close(t.normal, n, 1e-9), "{:?} vs {:?}", t.normal, n);
        let x = m.transform_vector3(c.x_axis).normalize();
        prop_assert!(close(t.x_axis, x, 1e-9), "{:?} vs {:?}", t.x_axis, x);
        prop_assert_eq!(t.start, c.start);
        prop_assert!((t.end - c.end).abs() <= 1e-9);

        w.undo.undo(&mut w.model).unwrap();
        prop_assert_eq!(&w.only_curve().1, &c);
        w.undo.redo(&mut w.model).unwrap();
        prop_assert_eq!(&w.only_curve().1, &t);
    }

    /// Stretching a circle in its plane makes plain edges; the edges stay. (Five or more
    /// points of an ellipse are never on one circle; three always are, so a triangle stays a
    /// curve, and that is fine.)
    #[test]
    fn stretching_in_the_plane_makes_plain_edges(
        s in shape().prop_map(|mut s| {
            s.sweep = None;
            s.segments = s.segments.max(5);
            s
        }),
        k in 1.25..3.0f64,
        along in 0.0..TAU,
    ) {
        let mut w = World::new();
        w.draw(&s);
        let (_, c) = w.only_curve();
        let edges = w.ctx().edge_count();
        let dir = (c.x_axis * along.cos() + c.y_axis() * along.sin()).normalize();
        let to = DQuat::from_rotation_arc(DVec3::X, dir);
        let m = DMat4::from_translation(c.center)
            * DMat4::from_quat(to)
            * DMat4::from_scale(DVec3::new(k, 1.0, 1.0))
            * DMat4::from_quat(to.inverse())
            * DMat4::from_translation(-c.center);
        let sel = w.everything();
        transform(&mut w.model, BodyId::Root, &sel, m, &mut w.undo, "Scale").unwrap();
        prop_assert_eq!(w.ctx().curve_count(), 0);
        prop_assert_eq!(w.ctx().edge_count(), edges);
        w.sound()?;
        w.undo.undo(&mut w.model).unwrap();
        prop_assert_eq!(&w.only_curve().1, &c);
    }

    /// Erasing any set of a circle's edges leaves one arc per run of two or more steps,
    /// each on the old circle, and undo brings the circle back.
    #[test]
    fn erasing_edges_leaves_runs(segments in 3usize..30, picks in proptest::collection::vec(0usize..30, 1..6)) {
        let s = Shape {
            center: DVec3::new(0.5, -0.5, 0.25),
            normal: DVec3::Z,
            radius: 1.5,
            sweep: None,
            segments,
            turn: 0.3,
        };
        let mut w = World::new();
        w.draw(&s);
        let (id, c) = w.only_curve();
        let before = w.model.clone();
        let edges = curve::curve_edges(w.ctx(), id);
        let mut gone = vec![false; segments];
        for p in &picks {
            gone[p % segments] = true;
        }
        let sel: Vec<Entity> = (0..segments)
            .filter(|i| gone[*i])
            .map(|i| Entity::Edge(edges[i]))
            .collect();
        erase(&mut w.model, BodyId::Root, &sel, &mut w.undo, "Erase").unwrap();
        w.sound()?;

        // expected runs of kept steps, around the circle
        let first_gone = gone.iter().position(|g| *g).unwrap();
        let mut runs = Vec::new();
        let mut len = 0;
        for k in 1..=segments {
            let i = (first_gone + k) % segments;
            if gone[i] {
                if len >= 2 {
                    runs.push(len);
                }
                len = 0;
            } else {
                len += 1;
            }
        }
        let mut got: Vec<usize> = w.ctx().curves().map(|(_, a)| a.vertices.len() - 1).collect();
        got.sort();
        runs.sort();
        prop_assert_eq!(got, runs);
        for (_, a) in w.ctx().curves() {
            prop_assert!(!a.is_closed());
            prop_assert_eq!(a.center, c.center);
            prop_assert_eq!(a.radius, c.radius);
            prop_assert_eq!(a.x_axis, c.x_axis);
            prop_assert!((a.sweep() - (a.vertices.len() - 1) as f64 * TAU / segments as f64).abs() <= 1e-9);
        }
        w.undo.undo(&mut w.model).unwrap();
        prop_assert_eq!(&w.model, &before);
    }

    /// Curves survive the native file bit for bit.
    #[test]
    fn curves_round_trip_through_the_file(a in shape(), b in shape()) {
        let mut w = World::new();
        w.draw(&a);
        let mut b = b;
        b.center += DVec3::new(20.0, 0.0, 0.0);
        w.draw(&b);
        let text = to_text(&w.model).unwrap();
        let back = from_text(&text).unwrap();
        prop_assert_eq!(&back, &w.model);
        prop_assert_eq!(to_text(&back).unwrap(), text);
    }

    /// `segments_for` is the smallest count within the tolerance; `points` lie on the arc.
    #[test]
    fn points_stay_within_the_tolerance(
        radius in 1e-3..1e3f64,
        start in -7.0..7.0f64,
        sweep in 0.01..TAU,
        closed in any::<bool>(),
        tol in 1e-6..10.0f64,
    ) {
        let sweep = if closed { TAU } else { sweep };
        let c = Curve {
            center: DVec3::new(1.0, 2.0, 3.0),
            normal: DVec3::new(0.0, 0.6, 0.8),
            x_axis: DVec3::X,
            radius,
            start,
            end: start + sweep,
            vertices: if closed {
                vec![VertexId(0), VertexId(1), VertexId(2), VertexId(0)]
            } else {
                vec![VertexId(0), VertexId(1), VertexId(2)]
            },
        };
        let min = if closed { 3 } else { 1 };
        let n = c.segments_for(tol);
        let sag = |n: usize| 2.0 * radius * (sweep / (4.0 * n as f64)).sin().powi(2);
        prop_assert!(n >= min);
        prop_assert!(n <= MAX_CURVE_SEGMENTS);
        prop_assert!(n == MAX_CURVE_SEGMENTS || sag(n) <= tol);
        prop_assert!(n == min || sag(n - 1) > tol);

        let pts = c.points(tol);
        prop_assert_eq!(pts.len(), n + 1);
        prop_assert_eq!(pts[0], c.point_at(start));
        if closed {
            prop_assert_eq!(pts[n], pts[0]);
        } else {
            prop_assert_eq!(pts[n], c.point_at(start + sweep));
        }
        for p in &pts {
            let d = *p - c.center;
            prop_assert!(d.dot(c.normal).abs() <= 1e-9 * radius.max(1.0));
            prop_assert!((d.length() - radius).abs() <= 1e-9 * radius.max(1.0));
        }
    }
}
