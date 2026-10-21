// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! `UndoStack::state` (`docs/specs/m2-undo-state.md`).

use glam::DVec3;
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::units::Unit;

/// Draw one edge from the origin to (x, 0, 0): one undo step.
fn edge(model: &mut Model, undo: &mut UndoStack, x: f64) {
    let mut target = Target {
        model,
        body: BodyId::Root,
        undo,
        unit: Unit::Meter,
    };
    draw_segments(
        &mut target,
        &[(DVec3::ZERO, DVec3::new(x, 0.0, 0.0))],
        "edge",
    )
    .expect("edge draws");
}

#[test]
fn empty_stack_is_state_zero() {
    assert_eq!(UndoStack::new().state(), 0);
}

#[test]
fn a_push_changes_the_state() {
    let (mut model, mut undo) = (Model::new(), UndoStack::new());
    edge(&mut model, &mut undo, 1.0);
    assert_ne!(undo.state(), 0);
}

#[test]
fn every_push_gets_a_new_state() {
    let (mut model, mut undo) = (Model::new(), UndoStack::new());
    edge(&mut model, &mut undo, 1.0);
    let first = undo.state();
    edge(&mut model, &mut undo, 2.0);
    assert_ne!(undo.state(), first);
}

#[test]
fn undo_returns_to_the_earlier_state_and_redo_back() {
    let (mut model, mut undo) = (Model::new(), UndoStack::new());
    edge(&mut model, &mut undo, 1.0);
    let first = undo.state();
    edge(&mut model, &mut undo, 2.0);
    let second = undo.state();
    undo.undo(&mut model).unwrap();
    assert_eq!(undo.state(), first);
    undo.redo(&mut model).unwrap();
    assert_eq!(undo.state(), second);
    undo.undo(&mut model).unwrap();
    undo.undo(&mut model).unwrap();
    assert_eq!(undo.state(), 0);
}

#[test]
fn a_new_edit_after_undo_never_reuses_a_state() {
    let (mut model, mut undo) = (Model::new(), UndoStack::new());
    edge(&mut model, &mut undo, 1.0);
    edge(&mut model, &mut undo, 2.0);
    let dropped = undo.state();
    undo.undo(&mut model).unwrap();
    edge(&mut model, &mut undo, 3.0);
    assert_ne!(undo.state(), dropped);
    // The redo branch is gone; the dropped state can't come back.
    assert_eq!(undo.redo(&mut model).unwrap(), None);
    assert_ne!(undo.state(), dropped);
}

#[test]
fn redrawing_existing_geometry_keeps_the_state() {
    let (mut model, mut undo) = (Model::new(), UndoStack::new());
    edge(&mut model, &mut undo, 1.0);
    let before = undo.state();
    edge(&mut model, &mut undo, 1.0); // empty edit: no undo step
    assert_eq!(undo.state(), before);
}
