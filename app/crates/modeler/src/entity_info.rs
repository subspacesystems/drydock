// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Entity Info panel: a description of the whole selection in the open body (what it
//! is, its length, area, size and materials, and for one group or component its names),
//! with fields to rename the definition and the instance. Kept apart from `App` so tests
//! can drive it with `egui_kittest`; the panel returns what the user asked for and the app
//! applies it through the editor.
//!
//! It always describes the whole selection and lives in a docked panel or its own window,
//! never at the click position in the viewport.
//!
//! Behaviour: `docs/specs/m6-entity-info.md`.

use eframe::egui;
use glam::DVec3;
use modeler_kernel::model::{BodyId, DefinitionId, InstanceId, MaterialId, Model};
use modeler_kernel::select::Entity;
use modeler_kernel::units::Unit;

/// What the selection is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfoKind {
    Nothing,
    Edges,
    Faces,
    Groups,
    Components,
    Mixed,
}

/// A value over several entities: all the same, or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shared<T> {
    Same(T),
    Varies,
}

/// The names and material of the one selected group or component.
#[derive(Clone, Debug, PartialEq)]
pub struct InstanceInfo {
    pub id: InstanceId,
    pub definition: DefinitionId,
    pub group: bool,
    pub definition_name: String,
    pub instance_name: String,
    pub material: Option<MaterialId>,
    /// Instances of the definition in the whole model.
    pub copies: usize,
}

/// What the panel shows for a selection.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityInfo {
    pub kind: InfoKind,
    /// Selected entities the body has, by kind.
    pub edges: usize,
    pub faces: usize,
    pub instances: usize,
    /// Total length of the edges, in meters, when only edges are selected.
    pub length: Option<f64>,
    /// Total area of the faces (holes left out), in square meters, when only faces are
    /// selected.
    pub area: Option<f64>,
    /// The faces' front and back materials (`None` inside = the default material), when
    /// only faces are selected.
    pub front: Option<Shared<Option<MaterialId>>>,
    pub back: Option<Shared<Option<MaterialId>>>,
    /// Extent along the body's x, y and z of the box around the selection, when it has one.
    pub size: Option<DVec3>,
    /// When the selection is exactly one instance.
    pub instance: Option<InstanceInfo>,
}

/// Describe `selection` in `body`.
pub fn describe(model: &Model, body: BodyId, selection: &[Entity]) -> EntityInfo {
    let _ = (model, body, selection);
    unimplemented!()
}

/// The panel's title line for `info`: "No Selection", "Edge", "3 Edges", ...
pub fn title(info: &EntityInfo) -> String {
    let _ = info;
    unimplemented!()
}

/// An area in square meters as text in `unit`'s square.
pub fn format_area(square_meters: f64, unit: Unit) -> String {
    let _ = (square_meters, unit);
    unimplemented!()
}

/// A material's name for the panel: "Default" for `None`, "Varies", or the material's
/// name (`"?"` when the model doesn't have it).
pub fn material_label(model: &Model, material: Shared<Option<MaterialId>>) -> String {
    let _ = (model, material);
    unimplemented!()
}

/// What the user asked for in the panel this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum InfoAction {
    RenameDefinition(DefinitionId, String),
    RenameInstance(InstanceId, String),
}

/// The panel's state between frames: the name fields' text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntityInfoPanel {
    pub definition_name: String,
    pub instance_name: String,
    /// The instance and the names the fields were last filled from.
    pub shown: Option<(InstanceId, String, String)>,
}

/// Draw the panel into `ui` for `selection` in `body`, lengths in `unit`. `Some` on the
/// frame the user renames something.
pub fn entity_info_panel(
    ui: &mut egui::Ui,
    panel: &mut EntityInfoPanel,
    model: &Model,
    body: BodyId,
    selection: &[Entity],
    unit: Unit,
) -> Option<InfoAction> {
    let _ = (ui, panel, model, body, selection, unit);
    unimplemented!()
}
