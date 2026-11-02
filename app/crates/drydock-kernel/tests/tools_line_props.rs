// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Properties of `draw_segments` and the line tool (`docs/specs/m2-tool-line.md`).

use drydock_kernel::model::*;
use drydock_kernel::tools::line::LineTool;
use drydock_kernel::tools::*;
use drydock_kernel::units::Unit;
use glam::DVec3;
use proptest::prelude::*;

/// Points on a coarse grid in a small box, so segments often meet, cross and close faces.
fn grid_point() -> impl Strategy<Value = DVec3> {
    (0i32..4, 0i32..4, 0i32..2).prop_map(|(x, y, z)| DVec3::new(x as f64, y as f64, z as f64))
}

fn any_point() -> impl Strategy<Value = DVec3> {
    (-5.0f64..5.0, -5.0f64..5.0, -5.0f64..5.0).prop_map(|(x, y, z)| DVec3::new(x, y, z))
}

#[derive(Clone, Debug)]
enum Op {
    Move(DVec3),
    Click(DVec3),
    Typed(f64),
    Cancel,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => grid_point().prop_map(Op::Move),
        5 => grid_point().prop_map(Op::Click),
        1 => (-3i32..4).prop_map(|n| Op::Typed(n as f64)),
        1 => Just(Op::Cancel),
    ]
}

fn undo_depth(undo: &UndoStack, model: &Model) -> usize {
    let mut u = undo.clone();
    let mut m = model.clone();
    let mut n = 0;
    while u.undo(&mut m).unwrap().is_some() {
        n += 1;
    }
    n
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Every call either fails and changes nothing, or adds exactly one undo step that takes
    /// the model back (none when every segment is already there: an empty edit is not pushed).
    #[test]
    fn draw_segments_is_one_step_or_nothing(
        first in prop::collection::vec((grid_point(), grid_point()), 0..6),
        second in prop::collection::vec((grid_point(), grid_point()), 1..6),
    ) {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let mut t = Target { model: &mut model, body: BodyId::Root, undo: &mut undo, unit: Unit::Meter };
        let _ = draw_segments(&mut t, &first, "A");
        let before = model.clone();
        let depth = undo_depth(&undo, &model);
        let mut t = Target { model: &mut model, body: BodyId::Root, undo: &mut undo, unit: Unit::Meter };
        match draw_segments(&mut t, &second, "B") {
            Ok(chains) => {
                prop_assert_eq!(chains.len(), second.len());
                prop_assert!(model.validate().is_empty());
                if model == before {
                    prop_assert_eq!(undo_depth(&undo, &model), depth);
                    return Ok(());
                }
                prop_assert_eq!(undo_depth(&undo, &model), depth + 1);
                prop_assert_eq!(undo.undo_label(), Some("B"));
                let after = model.clone();
                undo.undo(&mut model).unwrap();
                prop_assert_eq!(&model, &before);
                undo.redo(&mut model).unwrap();
                prop_assert_eq!(&model, &after);
            }
            Err(_) => {
                prop_assert_eq!(&model, &before);
                prop_assert_eq!(undo_depth(&undo, &model), depth);
            }
        }
    }

    /// Random sessions: the model stays valid, each input adds at most one "Line" step, the
    /// anchor is always a vertex of the model (after the first edge), and undoing everything
    /// empties the model.
    #[test]
    fn line_sessions_stay_valid_and_undo_to_empty(ops in prop::collection::vec(op(), 1..30)) {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let mut tool = LineTool::new();
        for o in ops {
            let input = match o {
                Op::Move(p) => Input::Move(p),
                Op::Click(p) => Input::Click(p),
                Op::Typed(x) => Input::Typed(format!("{x}")),
                Op::Cancel => Input::Cancel,
            };
            let depth = undo_depth(&undo, &model);
            let before = model.clone();
            let mut t = Target { model: &mut model, body: BodyId::Root, undo: &mut undo, unit: Unit::Meter };
            let result = tool.handle(input, &mut t);
            let now = undo_depth(&undo, &model);
            if result.is_err() {
                prop_assert_eq!(&model, &before);
                prop_assert_eq!(now, depth);
            } else {
                prop_assert!(now == depth || now == depth + 1);
                if now == depth + 1 {
                    prop_assert_eq!(undo.undo_label(), Some("Line"));
                    if let Some(a) = tool.anchor() {
                        let ctx = model.root().geometry();
                        prop_assert!(ctx.vertices().any(|(_, v)| v.position == a), "anchor {:?} is not a vertex", a);
                    }
                } else {
                    prop_assert_eq!(&model, &before);
                }
            }
            prop_assert!(model.validate().is_empty());
            if let Some((s, c)) = tool.preview() {
                prop_assert_eq!(Some(s), tool.anchor());
                prop_assert!((tool.length().unwrap() - s.distance(c)).abs() < 1e-12);
            } else {
                prop_assert_eq!(tool.length(), None);
            }
        }
        while undo.undo(&mut model).unwrap().is_some() {}
        prop_assert_eq!(model, Model::new());
    }

    /// Typed lengths on an empty model land exactly `len` from the start, toward the cursor.
    #[test]
    fn typed_length_lands_at_distance(
        s in any_point(),
        c in any_point(),
        len in prop_oneof![-10.0f64..-0.001, 0.001f64..10.0],
    ) {
        prop_assume!(s.distance(c) > 1e-3);
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let mut tool = LineTool::new();
        let mut t = Target { model: &mut model, body: BodyId::Root, undo: &mut undo, unit: Unit::Meter };
        tool.handle(Input::Click(s), &mut t).unwrap();
        tool.handle(Input::Move(c), &mut t).unwrap();
        tool.handle(Input::Typed(format!("{len}")), &mut t).unwrap();
        let end = tool.anchor().unwrap();
        prop_assert!((end.distance(s) - len.abs()).abs() < 1e-9);
        let u = (c - s).normalize();
        prop_assert!((end - s).dot(u) * len.signum() > 0.0);
        prop_assert!((end - s).cross(u).length() < 1e-9);
    }
}
