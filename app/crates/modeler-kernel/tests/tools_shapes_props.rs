// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Properties of the shape tools (`docs/specs/m2-tool-shapes.md`).

use glam::DVec3;
use modeler_kernel::model::*;
use modeler_kernel::tools::arc::ArcTool;
use modeler_kernel::tools::circle::CircleTool;
use modeler_kernel::tools::plane::DrawPlane;
use modeler_kernel::tools::rectangle::RectTool;
use modeler_kernel::tools::*;
use modeler_kernel::units::Unit;
use proptest::prelude::*;

fn pt() -> impl Strategy<Value = DVec3> {
    (0i32..5, 0i32..5, 0i32..2).prop_map(|(x, y, z)| DVec3::new(x as f64, y as f64, z as f64))
}

#[derive(Clone, Debug)]
enum Op {
    Move(DVec3),
    Click(DVec3),
    Typed(String),
    Cancel,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => pt().prop_map(Op::Move),
        5 => pt().prop_map(Op::Click),
        2 => prop_oneof![
            Just("1".to_string()),
            Just("2,1".to_string()),
            Just("-1,2".to_string()),
            Just("45".to_string()),
            Just("-90".to_string()),
            Just("6s".to_string()),
            Just("x".to_string()),
            Just("0".to_string()),
        ].prop_map(Op::Typed),
        1 => Just(Op::Cancel),
    ]
}

fn depth(undo: &UndoStack, model: &Model) -> usize {
    let mut u = undo.clone();
    let mut m = model.clone();
    let mut n = 0;
    while u.undo(&mut m).unwrap().is_some() {
        n += 1;
    }
    n
}

fn counts(model: &Model) -> (usize, usize, usize) {
    let c = model.body(BodyId::Root).unwrap().geometry();
    (c.vertices().count(), c.edges().count(), c.faces().count())
}

fn plane() -> DrawPlane {
    DrawPlane {
        u: DVec3::X,
        v: DVec3::Y,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Any input either fails and changes nothing, or adds at most one undo step; undoing
    /// every step leaves the empty model. No input panics.
    #[test]
    fn every_input_is_one_step_or_nothing(which in 0usize..3, ops in prop::collection::vec(op(), 0..25)) {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let mut rect = RectTool::new(plane());
        let mut circle = CircleTool::new(plane());
        let mut arc = ArcTool::new(plane());
        for o in ops {
            let input = match o {
                Op::Move(p) => Input::Move(p),
                Op::Click(p) => Input::Click(p),
                Op::Typed(s) => Input::Typed(s),
                Op::Cancel => Input::Cancel,
            };
            let before_counts = counts(&model);
            let before_depth = depth(&undo, &model);
            let mut t = Target { model: &mut model, body: BodyId::Root, undo: &mut undo, unit: Unit::Meter };
            let result = match which {
                0 => rect.handle(input, &mut t),
                1 => circle.handle(input, &mut t),
                _ => arc.handle(input, &mut t),
            };
            let after_depth = depth(&undo, &model);
            if result.is_err() {
                prop_assert_eq!(counts(&model), before_counts);
                prop_assert_eq!(after_depth, before_depth);
            } else {
                prop_assert!(after_depth == before_depth || after_depth == before_depth + 1);
            }
        }
        let mut u = undo.clone();
        let mut m = model.clone();
        while u.undo(&mut m).unwrap().is_some() {}
        prop_assert_eq!(counts(&m), (0, 0, 0));
    }
}
