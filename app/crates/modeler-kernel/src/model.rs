// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! A whole model: the root body, component and group definitions with their own bodies,
//! instances placing definitions with a transform, and materials.
//!
//! Behaviour: `docs/specs/m1-model.md`. Like `topology`, every change is a `ModelRecord`
//! that `ModelRecord::inverse` undoes; `UndoStack` keeps labelled `ModelEdit`s.

use std::collections::BTreeMap;

use glam::DMat4;

use crate::topology::{Context, Edit, FaceId, Record, TopoError, Violation};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DefinitionId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstanceId(pub u64);

/// Where geometry and instances live: the model root or the inside of a definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BodyId {
    Root,
    Definition(DefinitionId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    /// sRGB color, straight alpha; alpha is the opacity (1 = opaque).
    pub color: [f32; 4],
    /// Image file of the texture, if any, as a path relative to the model file.
    pub texture: Option<String>,
}

/// The materials painted on the two sides of a face. `None` is the default material.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FaceMaterials {
    pub front: Option<MaterialId>,
    pub back: Option<MaterialId>,
}

/// A placed copy of a definition's body.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub definition: DefinitionId,
    /// Maps the definition's coordinates into the containing body's.
    pub transform: DMat4,
    pub name: String,
    /// Paints faces inside the instance whose side has no material of its own.
    pub material: Option<MaterialId>,
}

/// A definition's name and kind. A group is a definition meant to have exactly one instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefinitionInfo {
    pub name: String,
    pub group: bool,
}

/// Geometry, instances and face materials of the root or of one definition.
///
/// Face material entries are keyed by face id and are kept when the face goes away, so
/// undoing the removal brings the material back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Body {
    geometry: Context,
    instances: BTreeMap<InstanceId, Instance>,
    face_materials: BTreeMap<FaceId, FaceMaterials>,
}

impl Body {
    pub fn geometry(&self) -> &Context {
        &self.geometry
    }

    pub fn instance(&self, id: InstanceId) -> Option<&Instance> {
        self.instances.get(&id)
    }

    /// Instances in ascending id order.
    pub fn instances(&self) -> impl Iterator<Item = (InstanceId, &Instance)> {
        self.instances.iter().map(|(id, i)| (*id, i))
    }

    /// The face's materials; the default (both `None`) when it has no entry.
    pub fn face_materials(&self, face: FaceId) -> FaceMaterials {
        self.face_materials.get(&face).copied().unwrap_or_default()
    }

    /// Non-default face material entries in ascending face id order, including entries for
    /// faces that no longer exist.
    pub fn face_material_entries(&self) -> impl Iterator<Item = (FaceId, FaceMaterials)> + '_ {
        self.face_materials.iter().map(|(f, m)| (*f, *m))
    }

    /// True when the body has no geometry, no instances and no face material entries.
    pub fn is_empty(&self) -> bool {
        self.geometry == Context::new()
            && self.instances.is_empty()
            && self.face_materials.is_empty()
    }
}

/// One primitive change to a model. `Remove*` and `Set*` records carry the old data.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelRecord {
    Geometry(BodyId, Record),
    AddMaterial(MaterialId, Material),
    RemoveMaterial(MaterialId, Material),
    SetMaterial {
        id: MaterialId,
        from: Material,
        to: Material,
    },
    AddDefinition(DefinitionId, DefinitionInfo),
    RemoveDefinition(DefinitionId, DefinitionInfo),
    SetDefinition {
        id: DefinitionId,
        from: DefinitionInfo,
        to: DefinitionInfo,
    },
    AddInstance(BodyId, InstanceId, Instance),
    RemoveInstance(BodyId, InstanceId, Instance),
    SetInstance {
        body: BodyId,
        id: InstanceId,
        from: Instance,
        to: Instance,
    },
    SetFaceMaterials {
        body: BodyId,
        face: FaceId,
        from: FaceMaterials,
        to: FaceMaterials,
    },
}

impl ModelRecord {
    /// The record that undoes this one.
    pub fn inverse(&self) -> ModelRecord {
        unimplemented!("m1-model")
    }
}

/// An ordered list of model records, applied first to last.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelEdit {
    pub records: Vec<ModelRecord>,
}

impl ModelEdit {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The edit that undoes this one: inverted records in reverse order.
    pub fn inverse(&self) -> ModelEdit {
        unimplemented!("m1-model")
    }
}

/// Why a model record or operation could not be applied. The model is unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    /// An `Add*` record names an id that is present.
    IdInUse,
    MissingBody(BodyId),
    MissingMaterial(MaterialId),
    MissingDefinition(DefinitionId),
    MissingInstance(BodyId, InstanceId),
    /// A `Remove*` or `Set*` record doesn't match the stored data.
    Mismatch,
    /// A geometry record failed in the body's context.
    Topo(TopoError),
    /// The material is still used by an instance or a face material entry.
    MaterialInUse(MaterialId),
    /// The definition still has instances somewhere.
    DefinitionInUse(DefinitionId),
    /// The definition's body still has geometry, instances or face material entries.
    DefinitionNotEmpty(DefinitionId),
    /// The instance would make a definition contain itself.
    Cycle,
}

/// A broken model invariant found by `Model::validate`.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelViolation {
    Geometry(BodyId, Violation),
    /// The instance's transform has a non-finite entry or isn't invertible.
    BadTransform(BodyId, InstanceId),
    /// A group definition with other than one instance (the count found).
    GroupInstances(DefinitionId, usize),
}

/// A whole model. Equality compares the root, definitions and materials, not id counters.
#[derive(Clone, Debug, Default)]
pub struct Model {
    root: Body,
    definitions: BTreeMap<DefinitionId, (DefinitionInfo, Body)>,
    materials: BTreeMap<MaterialId, Material>,
}

impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
            && self.definitions == other.definitions
            && self.materials == other.materials
    }
}

impl Model {
    pub fn new() -> Self {
        Self::default()
    }

    // -- queries --------------------------------------------------------------------

    pub fn root(&self) -> &Body {
        &self.root
    }

    pub fn body(&self, id: BodyId) -> Option<&Body> {
        match id {
            BodyId::Root => Some(&self.root),
            BodyId::Definition(d) => self.definitions.get(&d).map(|(_, b)| b),
        }
    }

    pub fn definition(&self, id: DefinitionId) -> Option<&DefinitionInfo> {
        self.definitions.get(&id).map(|(i, _)| i)
    }

    /// Definitions in ascending id order.
    pub fn definitions(&self) -> impl Iterator<Item = (DefinitionId, &DefinitionInfo)> {
        self.definitions.iter().map(|(id, (i, _))| (*id, i))
    }

    pub fn material(&self, id: MaterialId) -> Option<&Material> {
        self.materials.get(&id)
    }

    /// Materials in ascending id order.
    pub fn materials(&self) -> impl Iterator<Item = (MaterialId, &Material)> {
        self.materials.iter().map(|(id, m)| (*id, m))
    }

    /// Every instance of the definition, as (containing body, instance), ascending.
    pub fn instances_of(&self, definition: DefinitionId) -> Vec<(BodyId, InstanceId)> {
        let _ = definition;
        unimplemented!("m1-model")
    }

    /// Every broken invariant, in a deterministic order. Empty when the model is valid.
    pub fn validate(&self) -> Vec<ModelViolation> {
        unimplemented!("m1-model")
    }

    // -- operations (each appends its records to `log`) --------------------------------

    pub fn add_material(&mut self, material: Material, log: &mut ModelEdit) -> MaterialId {
        let _ = (material, log);
        unimplemented!("m1-model")
    }

    pub fn set_material(
        &mut self,
        id: MaterialId,
        material: Material,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let _ = (id, material, log);
        unimplemented!("m1-model")
    }

    pub fn remove_material(
        &mut self,
        id: MaterialId,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let _ = (id, log);
        unimplemented!("m1-model")
    }

    pub fn add_definition(&mut self, info: DefinitionInfo, log: &mut ModelEdit) -> DefinitionId {
        let _ = (info, log);
        unimplemented!("m1-model")
    }

    pub fn set_definition(
        &mut self,
        id: DefinitionId,
        info: DefinitionInfo,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let _ = (id, info, log);
        unimplemented!("m1-model")
    }

    pub fn remove_definition(
        &mut self,
        id: DefinitionId,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let _ = (id, log);
        unimplemented!("m1-model")
    }

    pub fn add_instance(
        &mut self,
        body: BodyId,
        instance: Instance,
        log: &mut ModelEdit,
    ) -> Result<InstanceId, ModelError> {
        let _ = (body, instance, log);
        unimplemented!("m1-model")
    }

    pub fn set_instance(
        &mut self,
        body: BodyId,
        id: InstanceId,
        instance: Instance,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let _ = (body, id, instance, log);
        unimplemented!("m1-model")
    }

    pub fn remove_instance(
        &mut self,
        body: BodyId,
        id: InstanceId,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let _ = (body, id, log);
        unimplemented!("m1-model")
    }

    pub fn set_face_materials(
        &mut self,
        body: BodyId,
        face: FaceId,
        materials: FaceMaterials,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let _ = (body, face, materials, log);
        unimplemented!("m1-model")
    }

    /// Run `f` on the body's geometry with a fresh topology `Edit`, then append each of its
    /// records to `log` as `ModelRecord::Geometry(body, record)`.
    pub fn edit_geometry<R>(
        &mut self,
        body: BodyId,
        log: &mut ModelEdit,
        f: impl FnOnce(&mut Context, &mut Edit) -> R,
    ) -> Result<R, ModelError> {
        let _ = (body, log, f);
        unimplemented!("m1-model")
    }

    // -- records ------------------------------------------------------------------------

    /// Apply one record. On error, nothing changes.
    pub fn apply(&mut self, record: &ModelRecord) -> Result<(), ModelError> {
        let _ = record;
        unimplemented!("m1-model")
    }

    /// Apply every record in order. On the first error, undo the records already applied
    /// and return it, leaving the model as it was.
    pub fn apply_edit(&mut self, edit: &ModelEdit) -> Result<(), ModelError> {
        let _ = edit;
        unimplemented!("m1-model")
    }
}

/// Labelled edits that can be undone and redone in order.
#[derive(Clone, Debug, Default)]
pub struct UndoStack {}

impl UndoStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an edit that has been applied. Empty edits are ignored. Clears the redo list.
    pub fn push(&mut self, label: &str, edit: ModelEdit) {
        let _ = (label, edit);
        unimplemented!("m1-model")
    }

    /// Undo the last edit; returns its label, or `None` when there is nothing to undo.
    pub fn undo(&mut self, model: &mut Model) -> Result<Option<String>, ModelError> {
        let _ = model;
        unimplemented!("m1-model")
    }

    /// Redo the last undone edit; returns its label, or `None` when there is nothing to redo.
    pub fn redo(&mut self, model: &mut Model) -> Result<Option<String>, ModelError> {
        let _ = model;
        unimplemented!("m1-model")
    }

    pub fn undo_label(&self) -> Option<&str> {
        unimplemented!("m1-model")
    }

    pub fn redo_label(&self) -> Option<&str> {
        unimplemented!("m1-model")
    }
}
