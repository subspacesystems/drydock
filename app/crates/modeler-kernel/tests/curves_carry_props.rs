// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Invariants of curves carried through groups, explode, copy and push/pull
//! (`docs/specs/m7-curves-carry.md`).

use std::f64::consts::TAU;

use glam::{DMat4, DQuat, DVec3};
use modeler_kernel::component::make_component;
use modeler_kernel::copy::copy_selection;
use modeler_kernel::curve::{self, Curve};
use modeler_kernel::explode::explode;
use modeler_kernel::geom::plane_basis;
use modeler_kernel::model::*;
use modeler_kernel::pushpull::push_pull;
use modeler_kernel::select::Entity;
use modeler_kernel::tools::*;
use modeler_kernel::topology::*;
use modeler_kernel::units::Unit;
use proptest::prelude::*;

#[derive(Clone, Debug)]
struct Shape {
    center: DVec3,
    normal: DVec3,
    radius: f64,
    /// `None`: a circle; `Some(radians)`: an arc of that sweep.
    sweep: Option<f64>,
    segments: usize,
    /// Angle of the first vertex in the drawing plane.
    turn: f64,
}

impl Shape {
    fn points(&self) -> Vec<DVec3> {
        let (u, v) = plane_basis(self.normal);
        let at = |a: f64| self.center + (u * a.cos() + v * a.sin()) * self.radius;
        match self.sweep {
            None => (0..self.segments)
                .map(|k| at(self.turn + TAU * k as f64 / self.segments as f64))
                .collect(),
            Some(sweep) => (0..=self.segments)
                .map(|k| at(self.turn + sweep * k as f64 / self.segments as f64))
                .collect(),
        }
    }
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
        0.05..3.0f64,
        prop_oneof![
            1 => Just(None),
            1 => (0.4..6.0f64).prop_map(Some),
        ],
        3usize..40,
        0.0..TAU,
    )
        .prop_map(|((x, y, z), normal, radius, sweep, segments, turn)| Shape {
            center: DVec3::new(x, y, z),
            normal,
            radius,
            sweep,
            segments,
            turn,
        })
}

/// A circle in the XY plane, for push/pull.
fn flat_circle() -> impl Strategy<Value = Shape> {
    (
        (-5.0..5.0f64, -5.0..5.0f64, -2.0..2.0f64),
        0.05..3.0f64,
        3usize..40,
        0.0..TAU,
    )
        .prop_map(|((x, y, z), radius, segments, turn)| Shape {
            center: DVec3::new(x, y, z),
            normal: DVec3::Z,
            radius,
            sweep: None,
            segments,
            turn,
        })
}

/// A similarity: rotation, uniform scale, optional mirror, translation far from the source.
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
            let m = DMat4::from_translation(DVec3::new(100.0 + x, y, z))
                * DMat4::from_quat(DQuat::from_axis_angle(axis, angle))
                * flip
                * DMat4::from_scale(DVec3::splat(scale));
            (m, scale, mirror)
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

    fn draw(&mut self, s: &Shape) -> Option<CurveId> {
        let mut tg = Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: Unit::Meter,
        };
        draw_curve(&mut tg, &s.points(), s.sweep.is_none(), "Curve")
            .ok()
            .and_then(|(_, id)| id)
    }

    fn curves(&self) -> Vec<(CurveId, Curve)> {
        self.ctx().curves().map(|(i, c)| (i, c.clone())).collect()
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

fn close(a: DVec3, b: DVec3, tol: f64) -> bool {
    a.distance(b) <= tol
}

fn same_circle(a: &Curve, b: &Curve, size: f64) -> Result<(), TestCaseError> {
    prop_assert!(close(a.center, b.center, 1e-9 * size), "{:?} vs {:?}", a, b);
    prop_assert!(close(a.normal, b.normal, 1e-9), "{:?} vs {:?}", a, b);
    prop_assert!(close(a.x_axis, b.x_axis, 1e-9), "{:?} vs {:?}", a, b);
    prop_assert!(
        (a.radius - b.radius).abs() <= 1e-9 * size,
        "{:?} vs {:?}",
        a,
        b
    );
    prop_assert!((a.start - b.start).abs() <= 1e-9);
    prop_assert!((a.end - b.end).abs() <= 1e-9);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// A similarity moves every point of the circle with the matrix, keeps the angles and the
    /// vertices, and leaves a unit normal and x axis at right angles.
    #[test]
    fn transformed_maps_every_point(s in shape(), (m, scale, _) in similarity()) {
        let c = Curve {
            center: s.center,
            normal: s.normal,
            x_axis: plane_basis(s.normal).0,
            radius: s.radius,
            start: s.turn,
            end: s.turn + s.sweep.unwrap_or(TAU),
            vertices: vec![VertexId(1), VertexId(2), VertexId(3)],
        };
        let t = c.transformed(m).expect("a similarity");
        prop_assert_eq!(&t.vertices, &c.vertices);
        prop_assert_eq!(t.start, c.start);
        prop_assert_eq!(t.end, c.end);
        prop_assert!((t.radius - c.radius * scale).abs() <= 1e-9 * c.radius * scale);
        prop_assert!((t.normal.length() - 1.0).abs() <= 1e-12);
        prop_assert!((t.x_axis.length() - 1.0).abs() <= 1e-12);
        prop_assert!(t.normal.dot(t.x_axis).abs() <= 1e-12);
        let size = 1.0 + m.transform_point3(c.center).length();
        for k in 0..=12 {
            let a = c.start + c.sweep() * k as f64 / 12.0;
            prop_assert!(close(t.point_at(a), m.transform_point3(c.point_at(a)), 1e-9 * size));
        }
    }

    /// Make group, then explode: the curve comes back as it was, on vertices where its old
    /// ones were. Undo of both steps gives the model back exactly.
    #[test]
    fn group_then_explode_returns_the_curve(s in shape(), group in any::<bool>()) {
        let mut w = World::new();
        prop_assume!(w.draw(&s).is_some());
        let before_model = w.model.clone();
        let original = w.curves();
        prop_assert_eq!(original.len(), 1);
        let c = original[0].1.clone();
        let points: Vec<DVec3> = c
            .vertices
            .iter()
            .map(|x| w.ctx().vertex(*x).unwrap().position)
            .collect();
        let sel = w.everything();
        let inst = make_component(
            &mut w.model,
            BodyId::Root,
            &sel,
            DefinitionInfo { name: String::new(), group },
            &mut w.undo,
            "Make Group",
        )
        .unwrap();
        prop_assert_eq!(w.ctx().curve_count(), 0);
        let def = w.model.root().instance(inst).unwrap().definition;
        prop_assert_eq!(
            w.model.body(BodyId::Definition(def)).unwrap().geometry().curve_count(),
            1
        );
        explode(&mut w.model, BodyId::Root, inst, &mut w.undo, "Explode").unwrap();
        w.sound()?;
        let back = w.curves();
        prop_assert_eq!(back.len(), 1);
        let got = &back[0].1;
        same_circle(got, &c, 1.0 + c.center.length())?;
        prop_assert_eq!(got.vertices.len(), c.vertices.len());
        for (x, p) in got.vertices.iter().zip(&points) {
            prop_assert!(close(w.ctx().vertex(*x).unwrap().position, *p, 1e-9 * (1.0 + p.length())));
        }
        w.undo.undo(&mut w.model).unwrap();
        w.undo.undo(&mut w.model).unwrap();
        prop_assert_eq!(&w.model, &before_model);
    }

    /// Copy with a random similarity: the copy gets the original's curve moved by the matrix
    /// (as `Curve::transformed`), the original keeps its own, and undo is exact.
    #[test]
    fn copy_moves_the_curve_with_the_matrix(s in shape(), (m, scale, mirror) in similarity()) {
        let mut w = World::new();
        let Some(id) = w.draw(&s) else {
            return Err(TestCaseError::reject("no curve"));
        };
        let c = w.ctx().curve(id).unwrap().clone();
        let before = w.model.clone();
        let sel = w.everything();
        copy_selection(&mut w.model, BodyId::Root, &sel, m, &mut w.undo, "Copy").unwrap();
        w.sound()?;
        let all = w.curves();
        prop_assert_eq!(all.len(), 2);
        prop_assert_eq!(w.ctx().curve(id), Some(&c));
        let copy = &all.iter().find(|(i, _)| *i != id).unwrap().1;
        let size = 1.0 + m.transform_point3(c.center).length();
        prop_assert!(close(copy.center, m.transform_point3(c.center), 1e-9 * size));
        prop_assert!((copy.radius - c.radius * scale).abs() <= 1e-9 * size);
        let sign = if mirror { -1.0 } else { 1.0 };
        prop_assert!(close(copy.normal, m.transform_vector3(c.normal).normalize() * sign, 1e-9));
        prop_assert!(close(copy.x_axis, m.transform_vector3(c.x_axis).normalize(), 1e-9));
        prop_assert!((copy.start - c.start).abs() <= 1e-9);
        prop_assert!((copy.end - c.end).abs() <= 1e-9);
        prop_assert_eq!(copy.is_closed(), c.is_closed());
        w.undo.undo(&mut w.model).unwrap();
        prop_assert_eq!(&w.model, &before);
    }

    /// Push/pull a lone disc by `d`: the base keeps its curve and the cap gets one whose
    /// center is `d` along the face normal, with the same radius, frame and angles.
    #[test]
    fn push_pull_gives_a_curve_at_each_end(s in flat_circle(), d in prop_oneof![-5.0..-0.01f64, 0.01..5.0f64]) {
        let mut w = World::new();
        let Some(id) = w.draw(&s) else {
            return Err(TestCaseError::reject("no curve"));
        };
        let c = w.ctx().curve(id).unwrap().clone();
        let face = w.ctx().faces().next().unwrap().0;
        let n = w.ctx().face_plane(face).unwrap().normal;
        let before = w.model.clone();
        push_pull(&mut w.model, BodyId::Root, face, d, &mut w.undo, "Push/Pull").unwrap();
        w.sound()?;
        let all = w.curves();
        prop_assert_eq!(all.len(), 2);
        prop_assert_eq!(w.ctx().curve(id), Some(&c));
        let cap = &all.iter().find(|(i, _)| *i != id).unwrap().1;
        prop_assert!(close(cap.center, c.center + n * d, 1e-9 * (1.0 + c.center.length() + d.abs())));
        prop_assert!(close(cap.normal, c.normal, 1e-12));
        prop_assert!(close(cap.x_axis, c.x_axis, 1e-12));
        prop_assert_eq!(cap.radius, c.radius);
        prop_assert_eq!(cap.start, c.start);
        prop_assert_eq!(cap.end, c.end);
        prop_assert_eq!(cap.vertices.len(), c.vertices.len());
        w.undo.undo(&mut w.model).unwrap();
        prop_assert_eq!(&w.model, &before);
    }
}
