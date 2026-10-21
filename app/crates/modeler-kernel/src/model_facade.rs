// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The public model API: the base model plus a per-model identity and revision.
//!
//! Behaviour: `docs/specs/m2-infer-cache.md`. The base implementation lives in `model.rs`;
//! this module adds the revision counter that pick and snap caches are keyed on.

use std::sync::atomic::{AtomicU64, Ordering};

pub use crate::model_base::{
    Body, BodyId, DefinitionId, DefinitionInfo, FaceMaterials, Instance, InstanceId, Material,
    MaterialId, ModelEdit, ModelError, ModelRecord, ModelViolation,
};

static NEXT_MODEL_ID: AtomicU64 = AtomicU64::new(1);

fn next_model_id() -> u64 {
    NEXT_MODEL_ID.fetch_add(1, Ordering::Relaxed)
}

/// A whole model. Equality compares the root, definitions and materials, not identity or
/// revision. A clone is a separate model: it keeps the revision but takes a new identity.
pub struct Model {
    inner: crate::model_base::Model,
    id: u64,
    revision: u64,
}

impl Clone for Model {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            id: next_model_id(),
            revision: self.revision,
        }
    }
}

impl Default for Model {
    fn default() -> Self {
        Self {
            inner: crate::model_base::Model::default(),
            id: next_model_id(),
            revision: 0,
        }
    }
}

impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.inner, f)
    }
}

impl Model {
    pub fn new() -> Self {
        Self::default()
    }

    /// A counter that changes whenever the root, a definition or an instance changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The identity and revision used to key a cached flattening of this model.
    pub(crate) fn cache_key(&self) -> (u64, u64) {
        (self.id, self.revision)
    }

    fn bump(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn root(&self) -> &Body {
        self.inner.root()
    }

    pub fn body(&self, id: BodyId) -> Option<&Body> {
        self.inner.body(id)
    }

    pub fn definition(&self, id: DefinitionId) -> Option<&DefinitionInfo> {
        self.inner.definition(id)
    }

    pub fn definitions(&self) -> impl Iterator<Item = (DefinitionId, &DefinitionInfo)> {
        self.inner.definitions()
    }

    pub fn material(&self, id: MaterialId) -> Option<&Material> {
        self.inner.material(id)
    }

    pub fn materials(&self) -> impl Iterator<Item = (MaterialId, &Material)> {
        self.inner.materials()
    }

    pub fn instances_of(&self, definition: DefinitionId) -> Vec<(BodyId, InstanceId)> {
        self.inner.instances_of(definition)
    }

    pub fn validate(&self) -> Vec<ModelViolation> {
        self.inner.validate()
    }

    pub fn add_material(&mut self, material: Material, log: &mut ModelEdit) -> MaterialId {
        let before = log.records.len();
        let id = self.inner.add_material(material, log);
        if log.records.len() != before {
            self.bump();
        }
        id
    }

    pub fn set_material(
        &mut self,
        id: MaterialId,
        material: Material,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        self.inner.set_material(id, material, log)?;
        self.bump();
        Ok(())
    }

    pub fn remove_material(
        &mut self,
        id: MaterialId,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        self.inner.remove_material(id, log)?;
        self.bump();
        Ok(())
    }

    pub fn add_definition(&mut self, info: DefinitionInfo, log: &mut ModelEdit) -> DefinitionId {
        let before = log.records.len();
        let id = self.inner.add_definition(info, log);
        if log.records.len() != before {
            self.bump();
        }
        id
    }

    pub fn set_definition(
        &mut self,
        id: DefinitionId,
        info: DefinitionInfo,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        self.inner.set_definition(id, info, log)?;
        self.bump();
        Ok(())
    }

    pub fn remove_definition(
        &mut self,
        id: DefinitionId,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        self.inner.remove_definition(id, log)?;
        self.bump();
        Ok(())
    }

    pub fn add_instance(
        &mut self,
        body: BodyId,
        instance: Instance,
        log: &mut ModelEdit,
    ) -> Result<InstanceId, ModelError> {
        let id = self.inner.add_instance(body, instance, log)?;
        self.bump();
        Ok(id)
    }

    pub fn set_instance(
        &mut self,
        body: BodyId,
        id: InstanceId,
        instance: Instance,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        self.inner.set_instance(body, id, instance, log)?;
        self.bump();
        Ok(())
    }

    pub fn remove_instance(
        &mut self,
        body: BodyId,
        id: InstanceId,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        self.inner.remove_instance(body, id, log)?;
        self.bump();
        Ok(())
    }

    pub fn set_face_materials(
        &mut self,
        body: BodyId,
        face: crate::topology::FaceId,
        materials: FaceMaterials,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        self.inner.set_face_materials(body, face, materials, log)?;
        self.bump();
        Ok(())
    }

    pub fn edit_geometry<R>(
        &mut self,
        body: BodyId,
        log: &mut ModelEdit,
        f: impl FnOnce(&mut crate::topology::Context, &mut crate::topology::Edit) -> R,
    ) -> Result<R, ModelError> {
        let result = self.inner.edit_geometry(body, log, f)?;
        self.bump();
        Ok(result)
    }

    pub fn apply(&mut self, record: &ModelRecord) -> Result<(), ModelError> {
        self.inner.apply(record)?;
        self.bump();
        Ok(())
    }

    pub fn apply_edit(&mut self, edit: &ModelEdit) -> Result<(), ModelError> {
        self.inner.apply_edit(edit)?;
        self.bump();
        Ok(())
    }
}

/// Labelled edits that can be undone and redone in order.
#[derive(Clone, Debug, Default)]
pub struct UndoStack {
    inner: crate::model_base::UndoStack,
}

impl UndoStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an edit that has been applied. Empty edits are ignored. Clears the redo list.
    pub fn push(&mut self, label: &str, edit: ModelEdit) {
        self.inner.push(label, edit);
    }

    /// Undo the last edit; returns its label, or `None` when there is nothing to undo.
    pub fn undo(&mut self, model: &mut Model) -> Result<Option<String>, ModelError> {
        let result = self.inner.undo(&mut model.inner)?;
        if result.is_some() {
            model.bump();
        }
        Ok(result)
    }

    /// Redo the last undone edit; returns its label, or `None` when there is nothing to redo.
    pub fn redo(&mut self, model: &mut Model) -> Result<Option<String>, ModelError> {
        let result = self.inner.redo(&mut model.inner)?;
        if result.is_some() {
            model.bump();
        }
        Ok(result)
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.inner.undo_label()
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.inner.redo_label()
    }

    pub fn state(&self) -> u64 {
        self.inner.state()
    }

    pub fn undo_count(&self) -> usize {
        self.inner.undo_count()
    }

    pub fn redo_count(&self) -> usize {
        self.inner.redo_count()
    }
}
