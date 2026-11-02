// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Move tool state machine (`docs/specs/m3-tool-move.md`).

use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use drydock_kernel::tools::translate::*;
use drydock_kernel::tools::{Input, Target};
use drydock_kernel::topology::FaceId;
use drydock_kernel::transform::TransformError;
use drydock_kernel::units::{ParseError, Unit};
use glam::DVec3;

mod transform_common;
use transform_common::*;

/// A model with a unit square A at x 1..2, y 0..1, z 0, and a unit square B at x 4..5.
struct World {
    model: Model,
    undo: UndoStack,
    a: FaceId,
    b: FaceId,
}

impl World {
    fn new() -> World {
        let mut model = Model::new();
        let ids = mesh(
            &mut model,
            &[
                v(1.0, 0.0, 0.0),
                v(2.0, 0.0, 0.0),
                v(2.0, 1.0, 0.0),
                v(1.0, 1.0, 0.0),
                v(4.0, 0.0, 0.0),
                v(5.0, 0.0, 0.0),
                v(5.0, 1.0, 0.0),
                v(4.0, 1.0, 0.0),
            ],
            &[&[0, 1, 2, 3], &[4, 5, 6, 7]],
        );
        World {
            model,
            undo: UndoStack::new(),
            a: ids[0],
            b: ids[1],
        }
    }

    fn sel(&self) -> Vec<Entity> {
        vec![Entity::Face(self.a)]
    }

    fn run(&mut self, tool: &mut MoveTool, input: Input) -> Result<(), MoveError> {
        let sel = self.sel();
        self.run_with(tool, input, &sel)
    }

    fn run_with(
        &mut self,
        tool: &mut MoveTool,
        input: Input,
        sel: &[Entity],
    ) -> Result<(), MoveError> {
        let mut target = Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: Unit::Meter,
        };
        tool.handle(input, sel, &mut target)
    }
}

const A: [DVec3; 4] = [
    DVec3::new(1.0, 0.0, 0.0),
    DVec3::new(2.0, 0.0, 0.0),
    DVec3::new(2.0, 1.0, 0.0),
    DVec3::new(1.0, 1.0, 0.0),
];
const B: [DVec3; 4] = [
    DVec3::new(4.0, 0.0, 0.0),
    DVec3::new(5.0, 0.0, 0.0),
    DVec3::new(5.0, 1.0, 0.0),
    DVec3::new(4.0, 1.0, 0.0),
];

fn shifted(pts: &[DVec3], by: DVec3) -> Vec<DVec3> {
    pts.iter().map(|&p| p + by).collect()
}

fn both(a: &[DVec3], b: &[DVec3]) -> Vec<DVec3> {
    a.iter().chain(b).copied().collect()
}

fn assert_positions(model: &Model, expected: &[DVec3]) {
    let got = positions(model);
    assert_eq!(got.len(), expected.len(), "{got:?} vs {expected:?}");
    for e in expected {
        assert!(
            got.iter().any(|g| g.distance(*e) < 1e-9),
            "{e:?} missing from {got:?}"
        );
    }
}

fn assert_vec(got: Option<DVec3>, want: DVec3) {
    let got = got.expect("a vector");
    assert!(got.distance(want) < 1e-9, "{got:?} != {want:?}");
}

fn typed(s: &str) -> Input {
    Input::Typed(s.to_string())
}

/// From the origin to (0, 1, 0) with the cursor there: the tool is left at `To`.
fn start(w: &mut World, tool: &mut MoveTool) {
    w.run(tool, Input::Click(DVec3::ZERO)).unwrap();
    w.run(tool, Input::Move(v(0.0, 1.0, 0.0))).unwrap();
    assert_eq!(tool.stage(), MoveStage::To);
}

// -- stages ----------------------------------------------------------------------------

#[test]
fn a_new_tool_waits_for_the_from_point() {
    let tool = MoveTool::new();
    assert_eq!(tool.stage(), MoveStage::From);
    assert_eq!(tool.from(), None);
    assert_eq!(tool.offset(), None);
    assert_eq!(tool.matrix(), None);
    assert_eq!(tool.last_offset(&UndoStack::new()), None);
    assert!(tool.preview().is_empty());
}

#[test]
fn two_clicks_move_the_selection_one_undo_step() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    w.run(&mut tool, Input::Move(v(0.5, 0.5, 0.0))).unwrap();
    assert_eq!(tool.stage(), MoveStage::From);

    w.run(&mut tool, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(tool.stage(), MoveStage::To);
    assert_eq!(tool.from(), Some(v(1.0, 0.0, 0.0)));
    assert_eq!(tool.offset(), None);

    w.run(&mut tool, Input::Move(v(1.0, 2.0, 0.5))).unwrap();
    assert_vec(tool.offset(), v(0.0, 2.0, 0.5));
    assert_eq!(
        tool.matrix(),
        Some(glam::DMat4::from_translation(v(0.0, 2.0, 0.5)))
    );
    assert_positions(&w.model, &both(&A, &B));
    assert_eq!(w.undo.undo_count(), 0);

    w.run(&mut tool, Input::Click(v(1.0, 2.0, 0.5))).unwrap();
    assert_positions(&w.model, &both(&shifted(&A, v(0.0, 2.0, 0.5)), &B));
    assert_eq!(tool.stage(), MoveStage::From);
    assert_eq!(tool.from(), None);
    assert_eq!(tool.offset(), None);
    assert_vec(tool.last_offset(&w.undo), v(0.0, 2.0, 0.5));
    assert_eq!(w.undo.undo_count(), 1);

    assert_eq!(w.undo.undo(&mut w.model).unwrap().as_deref(), Some("Move"));
    assert_positions(&w.model, &both(&A, &B));
}

#[test]
fn the_preview_is_a_line_from_the_start_to_the_cursor() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    w.run(&mut tool, Input::Click(v(1.0, 1.0, 0.0))).unwrap();
    assert!(tool.preview().is_empty());
    w.run(&mut tool, Input::Move(v(3.0, 1.0, 0.0))).unwrap();
    assert_eq!(tool.preview(), vec![(v(1.0, 1.0, 0.0), v(3.0, 1.0, 0.0))]);
    w.run(&mut tool, Input::Move(v(1.0, 1.0, 0.0))).unwrap();
    assert!(tool.preview().is_empty());
}

#[test]
fn clicking_the_from_point_again_moves_nothing() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    w.run(&mut tool, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    w.run(&mut tool, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(tool.stage(), MoveStage::To);
    assert_positions(&w.model, &both(&A, &B));
    assert_eq!(w.undo.undo_count(), 0);
}

#[test]
fn an_empty_selection_cannot_start() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    assert_eq!(
        w.run_with(&mut tool, Input::Click(DVec3::ZERO), &[]),
        Err(MoveError::Empty)
    );
    assert_eq!(tool.stage(), MoveStage::From);
}

#[test]
fn points_that_are_not_finite_are_refused() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    let bad = v(f64::NAN, 0.0, 0.0);
    assert_eq!(
        w.run(&mut tool, Input::Click(bad)),
        Err(MoveError::NotFinite)
    );
    assert_eq!(tool.stage(), MoveStage::From);
    w.run(&mut tool, Input::Click(DVec3::ZERO)).unwrap();
    w.run(&mut tool, Input::Move(v(0.0, 1.0, 0.0))).unwrap();
    w.run(&mut tool, Input::Move(bad)).unwrap();
    assert_vec(tool.offset(), v(0.0, 1.0, 0.0));
    assert_eq!(
        w.run(&mut tool, Input::Click(bad)),
        Err(MoveError::NotFinite)
    );
    assert_eq!(tool.stage(), MoveStage::To);
}

#[test]
fn cancel_drops_the_move() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    start(&mut w, &mut tool);
    w.run(&mut tool, Input::Cancel).unwrap();
    assert_eq!(tool.stage(), MoveStage::From);
    assert_eq!(tool.from(), None);
    assert_eq!(tool.offset(), None);
    assert!(tool.preview().is_empty());
    assert_positions(&w.model, &both(&A, &B));
    assert_eq!(w.undo.undo_count(), 0);
}

// -- typed distances -------------------------------------------------------------------

#[test]
fn a_typed_distance_moves_along_the_cursor_direction() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    start(&mut w, &mut tool);
    w.run(&mut tool, typed("3")).unwrap();
    assert_positions(&w.model, &both(&shifted(&A, v(0.0, 3.0, 0.0)), &B));
    assert_eq!(tool.stage(), MoveStage::From);
    assert_vec(tool.last_offset(&w.undo), v(0.0, 3.0, 0.0));
}

#[test]
fn a_negative_typed_distance_flips_the_direction() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    start(&mut w, &mut tool);
    w.run(&mut tool, typed("-2")).unwrap();
    assert_positions(&w.model, &both(&shifted(&A, v(0.0, -2.0, 0.0)), &B));
}

#[test]
fn typed_distances_use_the_target_unit_and_suffixes() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    start(&mut w, &mut tool);
    w.run(&mut tool, typed("50cm")).unwrap();
    assert_positions(&w.model, &both(&shifted(&A, v(0.0, 0.5, 0.0)), &B));
}

#[test]
fn typed_junk_is_refused_and_nothing_changes() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    start(&mut w, &mut tool);
    assert_eq!(
        w.run(&mut tool, typed("abc")),
        Err(MoveError::Parse(ParseError::Invalid))
    );
    assert_eq!(
        w.run(&mut tool, typed("")),
        Err(MoveError::Parse(ParseError::Empty))
    );
    assert_eq!(tool.stage(), MoveStage::To);
    assert_eq!(w.undo.undo_count(), 0);
}

#[test]
fn a_typed_distance_needs_a_direction() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    w.run(&mut tool, Input::Click(DVec3::ZERO)).unwrap();
    assert_eq!(w.run(&mut tool, typed("1")), Err(MoveError::NoDirection));
    w.run(&mut tool, Input::Move(DVec3::ZERO)).unwrap();
    assert_eq!(w.run(&mut tool, typed("1")), Err(MoveError::NoDirection));
    assert_eq!(tool.stage(), MoveStage::To);
    assert_positions(&w.model, &both(&A, &B));
}

#[test]
fn a_typed_zero_moves_nothing() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    start(&mut w, &mut tool);
    w.run(&mut tool, typed("0")).unwrap();
    assert_eq!(tool.stage(), MoveStage::To);
    assert_eq!(w.undo.undo_count(), 0);
}

#[test]
fn a_distance_cannot_be_typed_before_any_move() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    assert_eq!(w.run(&mut tool, typed("1")), Err(MoveError::NotStarted));
}

// -- typed distances after a move ------------------------------------------------------

#[test]
fn a_distance_typed_after_a_move_replaces_it() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    start(&mut w, &mut tool);
    w.run(&mut tool, Input::Click(v(0.0, 1.0, 0.0))).unwrap();
    assert_eq!(w.undo.undo_count(), 1);

    w.run(&mut tool, typed("4")).unwrap();
    assert_positions(&w.model, &both(&shifted(&A, v(0.0, 4.0, 0.0)), &B));
    assert_eq!(w.undo.undo_count(), 1);
    assert_vec(tool.last_offset(&w.undo), v(0.0, 4.0, 0.0));
    w.run(&mut tool, typed("-1")).unwrap();
    assert_positions(&w.model, &both(&shifted(&A, v(0.0, -1.0, 0.0)), &B));
    assert_eq!(w.undo.undo_count(), 1);
    assert_eq!(w.undo.undo(&mut w.model).unwrap().as_deref(), Some("Move"));
    assert_positions(&w.model, &both(&A, &B));
}

#[test]
fn a_retype_is_refused_once_the_undo_stack_moved_on() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    start(&mut w, &mut tool);
    w.run(&mut tool, Input::Click(v(0.0, 1.0, 0.0))).unwrap();
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(tool.last_offset(&w.undo), None);
    assert_eq!(w.run(&mut tool, typed("2")), Err(MoveError::NotStarted));
    assert_positions(&w.model, &both(&A, &B));
}

#[test]
fn a_new_from_click_forgets_the_last_move() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    start(&mut w, &mut tool);
    w.run(&mut tool, Input::Click(v(0.0, 1.0, 0.0))).unwrap();
    w.run(&mut tool, Input::Click(DVec3::ZERO)).unwrap();
    w.run(&mut tool, Input::Cancel).unwrap();
    assert_eq!(tool.last_offset(&w.undo), None);
    assert_eq!(w.run(&mut tool, typed("2")), Err(MoveError::NotStarted));
}

#[test]
fn a_retype_of_zero_or_junk_changes_nothing() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    start(&mut w, &mut tool);
    w.run(&mut tool, Input::Click(v(0.0, 1.0, 0.0))).unwrap();
    w.run(&mut tool, typed("0")).unwrap();
    assert_eq!(
        w.run(&mut tool, typed("x")),
        Err(MoveError::Parse(ParseError::Invalid))
    );
    assert_positions(&w.model, &both(&shifted(&A, v(0.0, 1.0, 0.0)), &B));
    assert_vec(tool.last_offset(&w.undo), v(0.0, 1.0, 0.0));
}

#[test]
fn a_retype_with_an_empty_selection_is_refused() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    start(&mut w, &mut tool);
    w.run(&mut tool, Input::Click(v(0.0, 1.0, 0.0))).unwrap();
    assert_eq!(
        w.run_with(&mut tool, typed("2"), &[]),
        Err(MoveError::Empty)
    );
    assert_positions(&w.model, &both(&shifted(&A, v(0.0, 1.0, 0.0)), &B));
    assert_eq!(w.undo.undo_count(), 1);
}

// -- refusals --------------------------------------------------------------------------

#[test]
fn a_refused_move_keeps_the_tool_and_the_model() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    let ghost = Entity::Face(FaceId(999));
    w.run_with(&mut tool, Input::Click(DVec3::ZERO), &[ghost])
        .unwrap();
    let got = w.run_with(&mut tool, Input::Click(v(0.0, 1.0, 0.0)), &[ghost]);
    assert_eq!(
        got,
        Err(MoveError::Transform(TransformError::MissingEntity(ghost)))
    );
    assert_eq!(tool.stage(), MoveStage::To);
    assert_positions(&w.model, &both(&A, &B));
    assert_eq!(w.undo.undo_count(), 0);
}

#[test]
fn a_refused_retype_restores_the_old_move() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    start(&mut w, &mut tool);
    w.run(&mut tool, Input::Click(v(0.0, 1.0, 0.0))).unwrap();
    let ghost = Entity::Face(FaceId(999));
    let got = w.run_with(&mut tool, typed("2"), &[ghost]);
    assert!(matches!(got, Err(MoveError::Transform(_))), "{got:?}");
    assert_positions(&w.model, &both(&shifted(&A, v(0.0, 1.0, 0.0)), &B));
    assert_eq!(w.undo.undo_count(), 1);
    assert_eq!(w.undo.redo_count(), 0);
    assert_vec(tool.last_offset(&w.undo), v(0.0, 1.0, 0.0));
}

// -- sticky ----------------------------------------------------------------------------

#[test]
fn a_move_welds_onto_the_geometry_it_meets() {
    let mut w = World::new();
    let mut tool = MoveTool::new();
    // A's right edge (x 2) lands on B's left edge (x 4): they share vertices afterwards.
    w.run(&mut tool, Input::Click(DVec3::ZERO)).unwrap();
    w.run(&mut tool, Input::Click(v(2.0, 0.0, 0.0))).unwrap();
    assert_positions(
        &w.model,
        &[
            v(3.0, 0.0, 0.0),
            v(4.0, 0.0, 0.0),
            v(4.0, 1.0, 0.0),
            v(3.0, 1.0, 0.0),
            v(5.0, 0.0, 0.0),
            v(5.0, 1.0, 0.0),
        ],
    );
    assert_eq!(w.model.root().geometry().faces().count(), 2);
    assert!(w.model.root().geometry().validate().is_empty());
    assert!(w.model.root().geometry().face(w.b).is_some());
    w.undo.undo(&mut w.model).unwrap();
    assert_positions(&w.model, &both(&A, &B));
}
