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
        match self {
            ModelRecord::Geometry(body, r) => ModelRecord::Geometry(*body, r.inverse()),
            ModelRecord::AddMaterial(id, m) => ModelRecord::RemoveMaterial(*id, m.clone()),
            ModelRecord::RemoveMaterial(id, m) => ModelRecord::AddMaterial(*id, m.clone()),
            ModelRecord::SetMaterial { id, from, to } => ModelRecord::SetMaterial {
                id: *id,
                from: to.clone(),
                to: from.clone(),
            },
            ModelRecord::AddDefinition(id, info) => {
                ModelRecord::RemoveDefinition(*id, info.clone())
            }
            ModelRecord::RemoveDefinition(id, info) => {
                ModelRecord::AddDefinition(*id, info.clone())
            }
            ModelRecord::SetDefinition { id, from, to } => ModelRecord::SetDefinition {
                id: *id,
                from: to.clone(),
                to: from.clone(),
            },
            ModelRecord::AddInstance(body, id, inst) => {
                ModelRecord::RemoveInstance(*body, *id, inst.clone())
            }
            ModelRecord::RemoveInstance(body, id, inst) => {
                ModelRecord::AddInstance(*body, *id, inst.clone())
            }
            ModelRecord::SetInstance { body, id, from, to } => ModelRecord::SetInstance {
                body: *body,
                id: *id,
                from: to.clone(),
                to: from.clone(),
            },
            ModelRecord::SetFaceMaterials {
                body,
                face,
                from,
                to,
            } => ModelRecord::SetFaceMaterials {
                body: *body,
                face: *face,
                from: *to,
                to: *from,
            },
        }
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
        ModelEdit {
            records: self
                .records
                .iter()
                .rev()
                .map(ModelRecord::inverse)
                .collect(),
        }
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
    next_material: u64,
    next_definition: u64,
    next_instance: u64,
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

    fn body_mut(&mut self, id: BodyId) -> Option<&mut Body> {
        match id {
            BodyId::Root => Some(&mut self.root),
            BodyId::Definition(d) => self.definitions.get_mut(&d).map(|(_, b)| b),
        }
    }

    fn instance_location(&self, id: InstanceId) -> Option<BodyId> {
        if self.root.instance(id).is_some() {
            return Some(BodyId::Root);
        }
        self.definitions
            .iter()
            .find(|(_, (_, b))| b.instance(id).is_some())
            .map(|(d, _)| BodyId::Definition(*d))
    }

    fn definition_has_instances(&self, id: DefinitionId) -> bool {
        if self.root.instances().any(|(_, i)| i.definition == id) {
            return true;
        }
        self.definitions
            .values()
            .any(|(_, b)| b.instances().any(|(_, i)| i.definition == id))
    }

    fn material_in_use(&self, id: MaterialId) -> bool {
        let used_by_body = |b: &Body| {
            b.instances().any(|(_, i)| i.material == Some(id))
                || b.face_material_entries()
                    .any(|(_, fm)| fm.front == Some(id) || fm.back == Some(id))
        };
        used_by_body(&self.root) || self.definitions.values().any(|(_, b)| used_by_body(b))
    }

    /// Whether placing an instance of `definition` in `body` would close a cycle.
    fn would_cycle(&self, definition: DefinitionId, body: BodyId) -> bool {
        let BodyId::Definition(parent) = body else {
            return false;
        };
        if definition == parent {
            return true;
        }
        let mut stack = vec![definition];
        let mut seen = std::collections::BTreeSet::new();
        seen.insert(definition);
        while let Some(current) = stack.pop() {
            let Some((_, b)) = self.definitions.get(&current) else {
                continue;
            };
            for inst in b.instances.values() {
                if inst.definition == parent {
                    return true;
                }
                if seen.insert(inst.definition) {
                    stack.push(inst.definition);
                }
            }
        }
        false
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
        let mut out: Vec<(BodyId, InstanceId)> = self
            .root
            .instances()
            .filter(|(_, i)| i.definition == definition)
            .map(|(id, _)| (BodyId::Root, id))
            .collect();
        for (d, (_, b)) in &self.definitions {
            out.extend(
                b.instances()
                    .filter(|(_, i)| i.definition == definition)
                    .map(|(id, _)| (BodyId::Definition(*d), id)),
            );
        }
        out
    }

    /// Every broken invariant, in a deterministic order. Empty when the model is valid.
    pub fn validate(&self) -> Vec<ModelViolation> {
        let mut out = Vec::new();

        let check_body = |body_id: BodyId, body: &Body, out: &mut Vec<ModelViolation>| {
            for v in body.geometry.validate() {
                out.push(ModelViolation::Geometry(body_id, v));
            }
            for (id, inst) in body.instances() {
                let finite = inst.transform.to_cols_array().iter().all(|v| v.is_finite());
                let det = inst.transform.determinant();
                if !finite || det.abs() <= 1e-12 {
                    out.push(ModelViolation::BadTransform(body_id, id));
                }
            }
        };

        check_body(BodyId::Root, &self.root, &mut out);
        for (d, (_, b)) in &self.definitions {
            check_body(BodyId::Definition(*d), b, &mut out);
        }

        for (d, (info, _)) in &self.definitions {
            if info.group {
                let n = self.instances_of(*d).len();
                if n != 1 {
                    out.push(ModelViolation::GroupInstances(*d, n));
                }
            }
        }

        out
    }

    // -- operations (each appends its records to `log`) --------------------------------

    pub fn add_material(&mut self, material: Material, log: &mut ModelEdit) -> MaterialId {
        let id = MaterialId(self.next_material);
        let record = ModelRecord::AddMaterial(id, material);
        if self.apply(&record).is_ok() {
            log.records.push(record);
        }
        id
    }

    pub fn set_material(
        &mut self,
        id: MaterialId,
        material: Material,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let from = self
            .materials
            .get(&id)
            .ok_or(ModelError::MissingMaterial(id))?
            .clone();
        let record = ModelRecord::SetMaterial {
            id,
            from,
            to: material,
        };
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    pub fn remove_material(
        &mut self,
        id: MaterialId,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let data = self
            .materials
            .get(&id)
            .ok_or(ModelError::MissingMaterial(id))?
            .clone();
        let record = ModelRecord::RemoveMaterial(id, data);
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    pub fn add_definition(&mut self, info: DefinitionInfo, log: &mut ModelEdit) -> DefinitionId {
        let id = DefinitionId(self.next_definition);
        let record = ModelRecord::AddDefinition(id, info);
        if self.apply(&record).is_ok() {
            log.records.push(record);
        }
        id
    }

    pub fn set_definition(
        &mut self,
        id: DefinitionId,
        info: DefinitionInfo,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let from = self
            .definitions
            .get(&id)
            .map(|(i, _)| i.clone())
            .ok_or(ModelError::MissingDefinition(id))?;
        let record = ModelRecord::SetDefinition { id, from, to: info };
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    pub fn remove_definition(
        &mut self,
        id: DefinitionId,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let info = self
            .definitions
            .get(&id)
            .map(|(i, _)| i.clone())
            .ok_or(ModelError::MissingDefinition(id))?;
        let record = ModelRecord::RemoveDefinition(id, info);
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    pub fn add_instance(
        &mut self,
        body: BodyId,
        instance: Instance,
        log: &mut ModelEdit,
    ) -> Result<InstanceId, ModelError> {
        let id = InstanceId(self.next_instance);
        let record = ModelRecord::AddInstance(body, id, instance);
        self.apply(&record)?;
        log.records.push(record);
        Ok(id)
    }

    pub fn set_instance(
        &mut self,
        body: BodyId,
        id: InstanceId,
        instance: Instance,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let from = self
            .body(body)
            .ok_or(ModelError::MissingBody(body))?
            .instance(id)
            .cloned()
            .ok_or(ModelError::MissingInstance(body, id))?;
        let record = ModelRecord::SetInstance {
            body,
            id,
            from,
            to: instance,
        };
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    pub fn remove_instance(
        &mut self,
        body: BodyId,
        id: InstanceId,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let data = self
            .body(body)
            .ok_or(ModelError::MissingBody(body))?
            .instance(id)
            .cloned()
            .ok_or(ModelError::MissingInstance(body, id))?;
        let record = ModelRecord::RemoveInstance(body, id, data);
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    pub fn set_face_materials(
        &mut self,
        body: BodyId,
        face: FaceId,
        materials: FaceMaterials,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let from = self
            .body(body)
            .ok_or(ModelError::MissingBody(body))?
            .face_materials(face);
        let record = ModelRecord::SetFaceMaterials {
            body,
            face,
            from,
            to: materials,
        };
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    /// Run `f` on the body's geometry with a fresh topology `Edit`, then append each of its
    /// records to `log` as `ModelRecord::Geometry(body, record)`.
    pub fn edit_geometry<R>(
        &mut self,
        body: BodyId,
        log: &mut ModelEdit,
        f: impl FnOnce(&mut Context, &mut Edit) -> R,
    ) -> Result<R, ModelError> {
        let b = self.body_mut(body).ok_or(ModelError::MissingBody(body))?;
        let mut edit = Edit::new();
        let result = f(&mut b.geometry, &mut edit);
        log.records.extend(
            edit.records
                .into_iter()
                .map(|record| ModelRecord::Geometry(body, record)),
        );
        Ok(result)
    }

    // -- records ------------------------------------------------------------------------

    /// Apply one record. On error, nothing changes.
    pub fn apply(&mut self, record: &ModelRecord) -> Result<(), ModelError> {
        match record {
            ModelRecord::Geometry(body, r) => {
                let b = self.body_mut(*body).ok_or(ModelError::MissingBody(*body))?;
                b.geometry.apply(r).map_err(ModelError::Topo)
            }
            ModelRecord::AddMaterial(id, material) => {
                if self.materials.contains_key(id) {
                    return Err(ModelError::IdInUse);
                }
                self.materials.insert(*id, material.clone());
                self.next_material = self.next_material.max(id.0.saturating_add(1));
                Ok(())
            }
            ModelRecord::RemoveMaterial(id, material) => {
                let stored = self
                    .materials
                    .get(id)
                    .ok_or(ModelError::MissingMaterial(*id))?;
                if stored != material {
                    return Err(ModelError::Mismatch);
                }
                if self.material_in_use(*id) {
                    return Err(ModelError::MaterialInUse(*id));
                }
                self.materials.remove(id);
                Ok(())
            }
            ModelRecord::SetMaterial { id, from, to } => {
                let stored = self
                    .materials
                    .get(id)
                    .ok_or(ModelError::MissingMaterial(*id))?;
                if stored != from {
                    return Err(ModelError::Mismatch);
                }
                self.materials.insert(*id, to.clone());
                Ok(())
            }
            ModelRecord::AddDefinition(id, info) => {
                if self.definitions.contains_key(id) {
                    return Err(ModelError::IdInUse);
                }
                self.definitions
                    .insert(*id, (info.clone(), Body::default()));
                self.next_definition = self.next_definition.max(id.0.saturating_add(1));
                Ok(())
            }
            ModelRecord::RemoveDefinition(id, info) => {
                let stored = self
                    .definitions
                    .get(id)
                    .map(|(i, _)| i)
                    .ok_or(ModelError::MissingDefinition(*id))?;
                if stored != info {
                    return Err(ModelError::Mismatch);
                }
                if self.definition_has_instances(*id) {
                    return Err(ModelError::DefinitionInUse(*id));
                }
                let empty = self.definitions.get(id).is_some_and(|(_, b)| b.is_empty());
                if !empty {
                    return Err(ModelError::DefinitionNotEmpty(*id));
                }
                self.definitions.remove(id);
                Ok(())
            }
            ModelRecord::SetDefinition { id, from, to } => {
                let stored = self
                    .definitions
                    .get(id)
                    .map(|(i, _)| i)
                    .ok_or(ModelError::MissingDefinition(*id))?;
                if stored != from {
                    return Err(ModelError::Mismatch);
                }
                if let Some((info, _)) = self.definitions.get_mut(id) {
                    *info = to.clone();
                }
                Ok(())
            }
            ModelRecord::AddInstance(body, id, instance) => {
                if self.body(*body).is_none() {
                    return Err(ModelError::MissingBody(*body));
                }
                if self.instance_location(*id).is_some() {
                    return Err(ModelError::IdInUse);
                }
                if !self.definitions.contains_key(&instance.definition) {
                    return Err(ModelError::MissingDefinition(instance.definition));
                }
                if let Some(material) = instance.material
                    && !self.materials.contains_key(&material)
                {
                    return Err(ModelError::MissingMaterial(material));
                }
                if self.would_cycle(instance.definition, *body) {
                    return Err(ModelError::Cycle);
                }
                if let Some(b) = self.body_mut(*body) {
                    b.instances.insert(*id, instance.clone());
                }
                self.next_instance = self.next_instance.max(id.0.saturating_add(1));
                Ok(())
            }
            ModelRecord::RemoveInstance(body, id, instance) => {
                let stored = self
                    .body(*body)
                    .ok_or(ModelError::MissingBody(*body))?
                    .instance(*id)
                    .ok_or(ModelError::MissingInstance(*body, *id))?;
                if stored != instance {
                    return Err(ModelError::Mismatch);
                }
                if let Some(b) = self.body_mut(*body) {
                    b.instances.remove(id);
                }
                Ok(())
            }
            ModelRecord::SetInstance { body, id, from, to } => {
                let stored = self
                    .body(*body)
                    .ok_or(ModelError::MissingBody(*body))?
                    .instance(*id)
                    .ok_or(ModelError::MissingInstance(*body, *id))?;
                if stored != from {
                    return Err(ModelError::Mismatch);
                }
                if !self.definitions.contains_key(&to.definition) {
                    return Err(ModelError::MissingDefinition(to.definition));
                }
                if let Some(material) = to.material
                    && !self.materials.contains_key(&material)
                {
                    return Err(ModelError::MissingMaterial(material));
                }
                if self.would_cycle(to.definition, *body) {
                    return Err(ModelError::Cycle);
                }
                if let Some(b) = self.body_mut(*body) {
                    b.instances.insert(*id, to.clone());
                }
                Ok(())
            }
            ModelRecord::SetFaceMaterials {
                body,
                face,
                from,
                to,
            } => {
                let current = self
                    .body(*body)
                    .ok_or(ModelError::MissingBody(*body))?
                    .face_materials(*face);
                if current != *from {
                    return Err(ModelError::Mismatch);
                }
                if let Some(material) = to.front
                    && !self.materials.contains_key(&material)
                {
                    return Err(ModelError::MissingMaterial(material));
                }
                if let Some(material) = to.back
                    && !self.materials.contains_key(&material)
                {
                    return Err(ModelError::MissingMaterial(material));
                }
                if let Some(b) = self.body_mut(*body) {
                    if *to == FaceMaterials::default() {
                        b.face_materials.remove(face);
                    } else {
                        b.face_materials.insert(*face, *to);
                    }
                }
                Ok(())
            }
        }
    }

    /// Apply every record in order. On the first error, undo the records already applied
    /// and return it, leaving the model as it was.
    pub fn apply_edit(&mut self, edit: &ModelEdit) -> Result<(), ModelError> {
        for (applied, record) in edit.records.iter().enumerate() {
            if let Err(error) = self.apply(record) {
                for done in edit.records[..applied].iter().rev() {
                    let _ = self.apply(&done.inverse());
                }
                return Err(error);
            }
        }
        Ok(())
    }
}

/// Labelled edits that can be undone and redone in order.
#[derive(Clone, Debug, Default)]
pub struct UndoStack {
    undo: Vec<(String, ModelEdit)>,
    redo: Vec<(String, ModelEdit)>,
}

impl UndoStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an edit that has been applied. Empty edits are ignored. Clears the redo list.
    pub fn push(&mut self, label: &str, edit: ModelEdit) {
        if edit.is_empty() {
            return;
        }
        self.undo.push((label.to_string(), edit));
        self.redo.clear();
    }

    /// Undo the last edit; returns its label, or `None` when there is nothing to undo.
    pub fn undo(&mut self, model: &mut Model) -> Result<Option<String>, ModelError> {
        let Some(entry) = self.undo.pop() else {
            return Ok(None);
        };
        if let Err(error) = model.apply_edit(&entry.1.inverse()) {
            self.undo.push(entry);
            return Err(error);
        }
        let label = entry.0.clone();
        self.redo.push(entry);
        Ok(Some(label))
    }

    /// Redo the last undone edit; returns its label, or `None` when there is nothing to redo.
    pub fn redo(&mut self, model: &mut Model) -> Result<Option<String>, ModelError> {
        let Some(entry) = self.redo.pop() else {
            return Ok(None);
        };
        if let Err(error) = model.apply_edit(&entry.1) {
            self.redo.push(entry);
            return Err(error);
        }
        let label = entry.0.clone();
        self.undo.push(entry);
        Ok(Some(label))
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|(label, _)| label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|(label, _)| label.as_str())
    }

    /// Identifies the model state this stack has reached. Two calls return the same
    /// value only if the same sequence of edits is applied: 0 when nothing is applied,
    /// otherwise a value unique to the newest applied edit (each pushed edit gets a
    /// fresh one; undo then redo returns to the earlier value; a push after an undo
    /// never reuses a value).
    pub fn state(&self) -> u64 {
        unimplemented!("m2-undo-state")
    }

    /// Number of edits that can be undone.
    pub fn undo_count(&self) -> usize {
        self.undo.len()
    }

    /// Number of edits that can be redone.
    pub fn redo_count(&self) -> usize {
        self.redo.len()
    }
}
