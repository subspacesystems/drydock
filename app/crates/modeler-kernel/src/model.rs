// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! A whole model: the root body, component and group definitions with their own bodies,
//! instances placing definitions with a transform, and materials.
//!
//! Behaviour: `docs/specs/m1-model.md`, guides: `docs/specs/m3-guides-model.md`. Like `topology`, every change is a `ModelRecord`
//! that `ModelRecord::inverse` undoes; `UndoStack` keeps labelled `ModelEdit`s.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use glam::{DMat4, DVec2};

use crate::sun::{Geolocation, ShadowSettings};
use crate::tape::{Guide, GuideId, Guides};
use crate::texture::TextureMap;
use crate::topology::{Context, Edit, FaceId, Record, TopoError, Violation};

/// A good guide has only finite numbers and, for a line, a unit-length direction.
fn guide_is_good(guide: &Guide) -> bool {
    match guide {
        Guide::Line { point, dir } => {
            point.is_finite() && dir.is_finite() && (dir.length() - 1.0).abs() <= 1e-9
        }
        Guide::Point(point) => point.is_finite(),
    }
}

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
    /// The texture image, if any.
    pub texture: Option<Texture>,
}

/// A material's texture image and its size in the model.
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    /// Image file, as a path relative to the model file.
    pub path: String,
    /// Width and height in meters that one copy of the image covers; both finite and > 0.
    pub size: DVec2,
}

impl Texture {
    pub fn new(path: &str, size: DVec2) -> Texture {
        Texture {
            path: path.to_string(),
            size,
        }
    }
}

/// A texture of `path` covering 1 m by 1 m.
impl From<&str> for Texture {
    fn from(path: &str) -> Texture {
        Texture::new(path, DVec2::ONE)
    }
}

/// Whether the material's texture size (if any) is finite and positive on both axes.
pub(crate) fn texture_size_is_good(material: &Material) -> bool {
    material.texture.as_ref().is_none_or(|texture| {
        texture.size.x.is_finite()
            && texture.size.x > 0.0
            && texture.size.y.is_finite()
            && texture.size.y > 0.0
    })
}

impl Material {
    /// A plain color material without a texture.
    pub fn new(name: &str, color: [f32; 4]) -> Material {
        Material {
            name: name.to_string(),
            color,
            texture: None,
        }
    }
}

/// The materials painted on the two sides of a face, and how a texture lies on each side.
/// `None` is the default material, and for a map the default placement
/// (`docs/specs/m4-texture-model.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FaceMaterials {
    pub front: Option<MaterialId>,
    pub back: Option<MaterialId>,
    pub front_map: Option<TextureMap>,
    pub back_map: Option<TextureMap>,
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

/// Geometry, instances, face materials and guides of the root or of one definition.
///
/// Face material entries are keyed by face id and are kept when the face goes away, so
/// undoing the removal brings the material back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Body {
    geometry: Context,
    instances: BTreeMap<InstanceId, Instance>,
    face_materials: BTreeMap<FaceId, FaceMaterials>,
    guides: Guides,
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

    /// The body's guides (construction lines and points), ascending id.
    pub fn guides(&self) -> &Guides {
        &self.guides
    }

    /// True when the body has no geometry, no instances, no face material entries and no
    /// guides.
    pub fn is_empty(&self) -> bool {
        self.geometry == Context::new()
            && self.instances.is_empty()
            && self.face_materials.is_empty()
            && self.guides.is_empty()
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
    AddGuide(BodyId, GuideId, Guide),
    RemoveGuide(BodyId, GuideId, Guide),
    /// Behaviour: `docs/specs/m5-geo-model.md`.
    SetGeolocation {
        from: Geolocation,
        to: Geolocation,
    },
    SetShadows {
        from: ShadowSettings,
        to: ShadowSettings,
    },
    /// Behaviour: `docs/specs/m6-units.md`.
    SetLengthFormat {
        from: crate::units::LengthFormat,
        to: crate::units::LengthFormat,
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
            ModelRecord::AddGuide(body, id, guide) => ModelRecord::RemoveGuide(*body, *id, *guide),
            ModelRecord::RemoveGuide(body, id, guide) => ModelRecord::AddGuide(*body, *id, *guide),
            ModelRecord::SetGeolocation { from, to } => ModelRecord::SetGeolocation {
                from: *to,
                to: *from,
            },
            ModelRecord::SetLengthFormat { from, to } => ModelRecord::SetLengthFormat {
                from: *to,
                to: *from,
            },
            ModelRecord::SetShadows { from, to } => ModelRecord::SetShadows {
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
    MissingGuide(BodyId, GuideId),
    /// A guide with a non-finite number, or a guide line whose direction isn't unit length.
    BadGuide,
    /// A material's texture size has a non-finite or non-positive entry.
    BadTexture,
    /// A face's texture map has a non-finite entry.
    BadTextureMap,
    /// A geolocation or shadow setting is non-finite or out of range
    /// (`docs/specs/m5-geo-model.md`).
    BadSettings,
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

/// A whole model. Equality compares the root, definitions, materials, geolocation and
/// shadow settings, not id counters.
#[derive(Clone, Debug, Default)]
pub struct Model {
    root: Body,
    definitions: BTreeMap<DefinitionId, (DefinitionInfo, Body)>,
    materials: BTreeMap<MaterialId, Material>,
    next_material: u64,
    next_definition: u64,
    next_instance: u64,
    id: ModelId,
    revision: u64,
    geolocation: Geolocation,
    shadows: ShadowSettings,
}

/// A process-unique identity; a clone takes a fresh one, so caches keyed on it never
/// mistake a clone for its original.
#[derive(Debug)]
struct ModelId(u64);

static NEXT_MODEL_ID: AtomicU64 = AtomicU64::new(1);

impl Default for ModelId {
    fn default() -> Self {
        Self(NEXT_MODEL_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl Clone for ModelId {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
            && self.definitions == other.definitions
            && self.materials == other.materials
            && self.geolocation == other.geolocation
            && self.shadows == other.shadows
    }
}

fn geolocation_is_good(g: &Geolocation) -> bool {
    g.latitude.is_finite()
        && (-90.0..=90.0).contains(&g.latitude)
        && g.longitude.is_finite()
        && (-180.0..=180.0).contains(&g.longitude)
        && g.north_angle.is_finite()
        && (-360.0..=360.0).contains(&g.north_angle)
        && g.utc_offset.is_finite()
        && (-12.0..=14.0).contains(&g.utc_offset)
}

fn shadow_settings_are_good(s: &ShadowSettings) -> bool {
    s.time.is_valid()
        && s.light.is_finite()
        && (0.0..=1.0).contains(&s.light)
        && s.dark.is_finite()
        && (0.0..=1.0).contains(&s.dark)
}

impl Model {
    pub fn new() -> Self {
        Self::default()
    }

    /// A counter that changes whenever the root, a definition or an instance changes.
    /// Behaviour: `docs/specs/m2-infer-cache.md`.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Where the model is on Earth (`docs/specs/m5-geo-model.md`).
    pub fn geolocation(&self) -> Geolocation {
        self.geolocation
    }

    /// How the sun lights the model (`docs/specs/m5-geo-model.md`).
    pub fn shadow_settings(&self) -> ShadowSettings {
        self.shadows
    }

    /// How the model shows lengths (`docs/specs/m6-units.md`).
    pub fn length_format(&self) -> crate::units::LengthFormat {
        unimplemented!("m6-units")
    }

    /// Replace the length format as one record in `log`; `BadSettings` (nothing changes)
    /// when it isn't valid (`docs/specs/m6-units.md`).
    pub fn set_length_format(
        &mut self,
        format: crate::units::LengthFormat,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let _ = (format, log);
        unimplemented!("m6-units")
    }

    /// Replace the geolocation as one record in `log`; `BadSettings` (nothing changes) when
    /// a field is non-finite or out of range.
    pub fn set_geolocation(
        &mut self,
        geolocation: Geolocation,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        if !geolocation_is_good(&geolocation) {
            return Err(ModelError::BadSettings);
        }
        if geolocation == self.geolocation {
            return Ok(());
        }
        let record = ModelRecord::SetGeolocation {
            from: self.geolocation,
            to: geolocation,
        };
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    /// Replace the shadow settings as one record in `log`; `BadSettings` (nothing changes)
    /// when a field is non-finite or out of range.
    pub fn set_shadow_settings(
        &mut self,
        settings: ShadowSettings,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        if !shadow_settings_are_good(&settings) {
            return Err(ModelError::BadSettings);
        }
        if settings == self.shadows {
            return Ok(());
        }
        let record = ModelRecord::SetShadows {
            from: self.shadows,
            to: settings,
        };
        self.apply(&record)?;
        log.records.push(record);
        Ok(())
    }

    /// The identity and revision that key a cached flattening of this model (or anything
    /// else derived from it, like the Entity Info description).
    pub fn cache_key(&self) -> (u64, u64) {
        (self.id.0, self.revision)
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

    /// The violations `validate` reports that belong to `bodies`: geometry and transform
    /// problems of those bodies, and the instance count of groups whose body is listed. Same
    /// order as `validate`; unknown bodies are skipped. Behaviour:
    /// `docs/specs/m4-fix-copy-speed.md`.
    pub fn validate_bodies(&self, bodies: &[BodyId]) -> Vec<ModelViolation> {
        let listed: std::collections::BTreeSet<BodyId> = bodies.iter().copied().collect();
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

        if listed.contains(&BodyId::Root) {
            check_body(BodyId::Root, &self.root, &mut out);
        }
        for (d, (_, b)) in &self.definitions {
            let id = BodyId::Definition(*d);
            if listed.contains(&id) {
                check_body(id, b, &mut out);
            }
        }
        for (d, (info, _)) in &self.definitions {
            if info.group && listed.contains(&BodyId::Definition(*d)) {
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

    /// Add a definition with an exact clone of `body`, keeping every id inside it and moving
    /// the model's instance counter past the clone's instance ids. Used by the clipboard.
    pub(crate) fn add_definition_with_body(
        &mut self,
        id: DefinitionId,
        info: DefinitionInfo,
        body: &Body,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let record = ModelRecord::AddDefinition(id, info);
        self.apply(&record)?;
        log.records.push(record);
        self.clone_body_into(BodyId::Definition(id), body, log)
    }

    /// Append records that put an exact clone of `source` into the (empty) body `target`.
    pub(crate) fn clone_body_into(
        &mut self,
        target: BodyId,
        source: &Body,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        for (id, vertex) in source.geometry.vertices() {
            let record = ModelRecord::Geometry(target, Record::AddVertex(id, *vertex));
            self.apply(&record)?;
            log.records.push(record);
        }
        for (id, edge) in source.geometry.edges() {
            let record = ModelRecord::Geometry(target, Record::AddEdge(id, *edge));
            self.apply(&record)?;
            log.records.push(record);
        }
        for (id, face) in source.geometry.faces() {
            let record = ModelRecord::Geometry(target, Record::AddFace(id, face.clone()));
            self.apply(&record)?;
            log.records.push(record);
        }
        for (face, materials) in &source.face_materials {
            let record = ModelRecord::SetFaceMaterials {
                body: target,
                face: *face,
                from: FaceMaterials::default(),
                to: *materials,
            };
            self.apply(&record)?;
            log.records.push(record);
        }
        for (id, instance) in &source.instances {
            let record = ModelRecord::AddInstance(target, *id, instance.clone());
            self.apply(&record)?;
            log.records.push(record);
        }
        for (id, guide) in source.guides.iter() {
            let record = ModelRecord::AddGuide(target, id, *guide);
            self.apply(&record)?;
            log.records.push(record);
        }
        Ok(())
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

    /// Add a guide to the body. Errors: `MissingBody`, `BadGuide`.
    pub fn add_guide(
        &mut self,
        body: BodyId,
        guide: Guide,
        log: &mut ModelEdit,
    ) -> Result<GuideId, ModelError> {
        let current = self.body(body).ok_or(ModelError::MissingBody(body))?;
        if !guide_is_good(&guide) {
            return Err(ModelError::BadGuide);
        }
        let id = {
            let mut probe = current.guides().clone();
            probe.add(guide)
        };
        let record = ModelRecord::AddGuide(body, id, guide);
        self.apply(&record)?;
        log.records.push(record);
        Ok(id)
    }

    /// Remove one guide; returns it. Errors: `MissingBody`, `MissingGuide`.
    pub fn remove_guide(
        &mut self,
        body: BodyId,
        id: GuideId,
        log: &mut ModelEdit,
    ) -> Result<Guide, ModelError> {
        let guide = *self
            .body(body)
            .ok_or(ModelError::MissingBody(body))?
            .guides()
            .get(id)
            .ok_or(ModelError::MissingGuide(body, id))?;
        let record = ModelRecord::RemoveGuide(body, id, guide);
        self.apply(&record)?;
        log.records.push(record);
        Ok(guide)
    }

    /// Remove every guide of the body; returns them, ascending id. Errors: `MissingBody`.
    pub fn clear_guides(
        &mut self,
        body: BodyId,
        log: &mut ModelEdit,
    ) -> Result<Vec<(GuideId, Guide)>, ModelError> {
        let guides: Vec<(GuideId, Guide)> = self
            .body(body)
            .ok_or(ModelError::MissingBody(body))?
            .guides()
            .iter()
            .map(|(id, guide)| (id, *guide))
            .collect();
        for (id, guide) in &guides {
            let record = ModelRecord::RemoveGuide(body, *id, *guide);
            self.apply(&record)?;
            log.records.push(record);
        }
        Ok(guides)
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
        self.revision = self.revision.wrapping_add(1);
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
        self.apply_record(record)?;
        if !matches!(
            record,
            ModelRecord::SetGeolocation { .. } | ModelRecord::SetShadows { .. }
        ) {
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }

    fn apply_record(&mut self, record: &ModelRecord) -> Result<(), ModelError> {
        match record {
            ModelRecord::Geometry(body, r) => {
                let b = self.body_mut(*body).ok_or(ModelError::MissingBody(*body))?;
                b.geometry.apply(r).map_err(ModelError::Topo)
            }
            ModelRecord::AddMaterial(id, material) => {
                if !texture_size_is_good(material) {
                    return Err(ModelError::BadTexture);
                }
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
                if !texture_size_is_good(to) {
                    return Err(ModelError::BadTexture);
                }
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
                if to.front_map.is_some_and(|map| !map.is_finite())
                    || to.back_map.is_some_and(|map| !map.is_finite())
                {
                    return Err(ModelError::BadTextureMap);
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
            ModelRecord::AddGuide(body, id, guide) => {
                let b = self.body_mut(*body).ok_or(ModelError::MissingBody(*body))?;
                if !guide_is_good(guide) {
                    return Err(ModelError::BadGuide);
                }
                if b.guides.get(*id).is_some() {
                    return Err(ModelError::IdInUse);
                }
                b.guides.insert(*id, *guide);
                Ok(())
            }
            ModelRecord::RemoveGuide(body, id, guide) => {
                let b = self.body_mut(*body).ok_or(ModelError::MissingBody(*body))?;
                let stored = b
                    .guides
                    .get(*id)
                    .ok_or(ModelError::MissingGuide(*body, *id))?;
                if stored != guide {
                    return Err(ModelError::Mismatch);
                }
                b.guides.remove(*id);
                Ok(())
            }
            ModelRecord::SetGeolocation { from, to } => {
                if !geolocation_is_good(to) {
                    return Err(ModelError::BadSettings);
                }
                if *from != self.geolocation {
                    return Err(ModelError::Mismatch);
                }
                self.geolocation = *to;
                Ok(())
            }
            ModelRecord::SetLengthFormat { .. } => unimplemented!("m6-units"),
            ModelRecord::SetShadows { from, to } => {
                if !shadow_settings_are_good(to) {
                    return Err(ModelError::BadSettings);
                }
                if *from != self.shadows {
                    return Err(ModelError::Mismatch);
                }
                self.shadows = *to;
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

#[derive(Clone, Debug)]
struct UndoEntry {
    state: u64,
    label: String,
    edit: ModelEdit,
}

/// Labelled edits that can be undone and redone in order.
#[derive(Clone, Debug, Default)]
pub struct UndoStack {
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
    next_state: u64,
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
        self.next_state = self.next_state.wrapping_add(1);
        self.undo.push(UndoEntry {
            state: self.next_state,
            label: label.to_string(),
            edit,
        });
        self.redo.clear();
    }

    /// Undo the last edit; returns its label, or `None` when there is nothing to undo.
    pub fn undo(&mut self, model: &mut Model) -> Result<Option<String>, ModelError> {
        let Some(entry) = self.undo.pop() else {
            return Ok(None);
        };
        if let Err(error) = model.apply_edit(&entry.edit.inverse()) {
            self.undo.push(entry);
            return Err(error);
        }
        let label = entry.label.clone();
        self.redo.push(entry);
        Ok(Some(label))
    }

    /// Redo the last undone edit; returns its label, or `None` when there is nothing to redo.
    pub fn redo(&mut self, model: &mut Model) -> Result<Option<String>, ModelError> {
        let Some(entry) = self.redo.pop() else {
            return Ok(None);
        };
        if let Err(error) = model.apply_edit(&entry.edit) {
            self.redo.push(entry);
            return Err(error);
        }
        let label = entry.label.clone();
        self.undo.push(entry);
        Ok(Some(label))
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|entry| entry.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|entry| entry.label.as_str())
    }

    /// Identifies the model state this stack has reached. Two calls return the same
    /// value only if the same sequence of edits is applied: 0 when nothing is applied,
    /// otherwise a value unique to the newest applied edit (each pushed edit gets a
    /// fresh one; undo then redo returns to the earlier value; a push after an undo
    /// never reuses a value).
    pub fn state(&self) -> u64 {
        self.undo.last().map_or(0, |entry| entry.state)
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
