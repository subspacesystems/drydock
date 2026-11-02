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
        if self.alt {
            SelectOp::Remove
        } else if self.ctrl {
            SelectOp::Toggle
        } else if self.shift {
            SelectOp::Add
        } else {
            SelectOp::Replace
        }
    }
}

/// The selection after applying `mods` to `current` with the `picked` entities (a click's hit,
/// or a box's result). Sorted, no duplicates. `current` may be unsorted or hold duplicates.
pub fn apply(current: &[Entity], picked: &[Entity], mods: ClickMods) -> Vec<Entity> {
    use std::collections::BTreeSet;

    let current: BTreeSet<Entity> = current.iter().copied().collect();
    let picked: BTreeSet<Entity> = picked.iter().copied().collect();

    match mods.op() {
        SelectOp::Replace => picked.into_iter().collect(),
        SelectOp::Add => current.union(&picked).copied().collect(),
        SelectOp::Toggle => current.symmetric_difference(&picked).copied().collect(),
        SelectOp::Remove => current.difference(&picked).copied().collect(),
    }
}

/// `apply` for a click: `hit` is the entity under the cursor, `None` for empty space.
pub fn apply_click(current: &[Entity], hit: Option<Entity>, mods: ClickMods) -> Vec<Entity> {
    match hit {
        Some(entity) => apply(current, &[entity], mods),
        None => apply(current, &[], mods),
    }
}
