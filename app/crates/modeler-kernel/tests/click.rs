// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tests for `modeler_kernel::click` (spec: docs/specs/m2-select-click.md).

use modeler_kernel::click::{ClickMods, SelectOp, apply, apply_click};
use modeler_kernel::select::Entity;
use modeler_kernel::topology::{EdgeId, FaceId};

fn e(n: u64) -> Entity {
    Entity::Edge(EdgeId(n))
}
fn f(n: u64) -> Entity {
    Entity::Face(FaceId(n))
}
fn mods(shift: bool, ctrl: bool, alt: bool) -> ClickMods {
    ClickMods { shift, ctrl, alt }
}

const NONE: ClickMods = ClickMods {
    shift: false,
    ctrl: false,
    alt: false,
};
const SHIFT: ClickMods = ClickMods {
    shift: true,
    ctrl: false,
    alt: false,
};
const CTRL: ClickMods = ClickMods {
    shift: false,
    ctrl: true,
    alt: false,
};
const ALT: ClickMods = ClickMods {
    shift: false,
    ctrl: false,
    alt: true,
};

#[test]
fn default_mods_are_none() {
    assert_eq!(ClickMods::default(), NONE);
}

#[test]
fn op_table() {
    assert_eq!(NONE.op(), SelectOp::Replace);
    assert_eq!(SHIFT.op(), SelectOp::Add);
    assert_eq!(CTRL.op(), SelectOp::Toggle);
    assert_eq!(ALT.op(), SelectOp::Remove);
    assert_eq!(mods(true, true, false).op(), SelectOp::Toggle);
    assert_eq!(mods(true, false, true).op(), SelectOp::Remove);
    assert_eq!(mods(false, true, true).op(), SelectOp::Remove);
    assert_eq!(mods(true, true, true).op(), SelectOp::Remove);
}

#[test]
fn replace() {
    assert_eq!(apply(&[e(1), e(2)], &[f(3)], NONE), vec![f(3)]);
}

#[test]
fn replace_with_nothing_clears() {
    assert_eq!(apply(&[e(1), e(2)], &[], NONE), vec![]);
    assert_eq!(apply_click(&[e(1)], None, NONE), vec![]);
}

#[test]
fn add() {
    assert_eq!(
        apply(&[e(2), e(1)], &[f(1), e(2)], SHIFT),
        vec![e(1), e(2), f(1)]
    );
}

#[test]
fn toggle() {
    assert_eq!(apply(&[e(1), e(2)], &[e(2), e(3)], CTRL), vec![e(1), e(3)]);
}

#[test]
fn remove() {
    assert_eq!(
        apply(&[e(1), e(2), f(1)], &[e(2), e(9)], ALT),
        vec![e(1), f(1)]
    );
}

#[test]
fn modifiers_with_nothing_picked_keep_selection() {
    for m in [SHIFT, CTRL, ALT] {
        assert_eq!(apply(&[f(2), e(1), e(1)], &[], m), vec![e(1), f(2)]);
        assert_eq!(apply_click(&[f(2), e(1)], None, m), vec![e(1), f(2)]);
    }
}

#[test]
fn duplicates_are_folded() {
    assert_eq!(apply(&[e(1), e(1)], &[e(1), e(1)], CTRL), vec![]);
    assert_eq!(apply(&[], &[e(1), e(1)], CTRL), vec![e(1)]);
    assert_eq!(apply(&[], &[e(3), e(3), e(2)], NONE), vec![e(2), e(3)]);
}

#[test]
fn click_hit() {
    assert_eq!(apply_click(&[e(1)], Some(e(2)), NONE), vec![e(2)]);
    assert_eq!(apply_click(&[e(1)], Some(e(2)), SHIFT), vec![e(1), e(2)]);
    assert_eq!(apply_click(&[e(1), e(2)], Some(e(2)), CTRL), vec![e(1)]);
    assert_eq!(apply_click(&[e(1)], Some(e(2)), CTRL), vec![e(1), e(2)]);
    assert_eq!(apply_click(&[e(1), e(2)], Some(e(2)), ALT), vec![e(1)]);
    assert_eq!(apply_click(&[e(1)], Some(e(2)), ALT), vec![e(1)]);
}

#[test]
fn order_is_edges_then_faces() {
    assert_eq!(apply(&[f(1)], &[e(9)], SHIFT), vec![e(9), f(1)]);
}
