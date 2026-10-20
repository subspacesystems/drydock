// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! How a click or a finished selection box changes the current selection. Pure; the app keeps
//! the selection and turns the keyboard state into `ClickMods`.
//!
//! Behaviour: `docs/specs/m2-select-click.md`.

use crate::select::Entity;

/// The modifier keys held during a click or box drag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClickMods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// What the modifiers ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectOp {
    /// The picked entities become the selection.
    Replace,
    /// The picked entities are added.
    Add,
    /// Picked entities in the selection leave it; the others join it.
    Toggle,
    /// The picked entities leave the selection.
    Remove,
}

impl ClickMods {
    /// The operation these modifiers select.
    pub fn op(self) -> SelectOp {
        unimplemented!("m2-select-click")
    }
}

/// The selection after applying `mods` to `current` with the `picked` entities (a click's hit,
/// or a box's result). Sorted, no duplicates. `current` may be unsorted or hold duplicates.
pub fn apply(current: &[Entity], picked: &[Entity], mods: ClickMods) -> Vec<Entity> {
    let _ = (current, picked, mods);
    unimplemented!("m2-select-click")
}

/// `apply` for a click: `hit` is the entity under the cursor, `None` for empty space.
pub fn apply_click(current: &[Entity], hit: Option<Entity>, mods: ClickMods) -> Vec<Entity> {
    let _ = (current, hit, mods);
    unimplemented!("m2-select-click")
}
