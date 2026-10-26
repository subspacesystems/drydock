// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Components panel: the model's component definitions with drawn thumbnails, a search
//! box and a list / tiles switch. Clicking one asks to place an instance of it. Kept apart
//! from `App` so tests can drive it with `egui_kittest`; the panel returns what the user
//! asked for and the app applies it through the editor.
//!
//! Behaviour: `docs/specs/m6-components-panel.md`.

use eframe::egui;
use modeler_kernel::model::{DefinitionId, Model};

/// Side of a thumbnail in the list, in points.
pub const LIST_THUMB: f32 = 28.0;
/// Side of a tile's thumbnail, in points.
pub const TILE_THUMB: f32 = 64.0;

/// What the panel remembers between frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComponentsPanel {
    /// The search box's text.
    pub search: String,
    /// Tiles instead of a list.
    pub tiles: bool,
}

/// What the user asked for in the panel this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentAction {
    /// Start placing an instance of this definition.
    Place(DefinitionId),
}

/// The definitions the panel lists for `search`: components only (not groups) whose name
/// contains `search` ignoring case, ordered by name ignoring case, then by id.
pub fn listed(model: &Model, search: &str) -> Vec<DefinitionId> {
    let _ = (model, search);
    unimplemented!("m6-components-panel")
}

/// The definition's edges seen from the standard isometric direction, as segments inside
/// the unit square (y down), scaled uniformly so the larger extent is exactly 1 and
/// centred on the other axis. Empty when the definition has no edges.
pub fn thumbnail_segments(model: &Model, definition: DefinitionId) -> Vec<[egui::Pos2; 2]> {
    let _ = (model, definition);
    unimplemented!("m6-components-panel")
}

/// Draw the panel.
pub fn components_panel(
    ui: &mut egui::Ui,
    panel: &mut ComponentsPanel,
    model: &Model,
) -> Option<ComponentAction> {
    let _ = (ui, panel, model);
    unimplemented!("m6-components-panel")
}
