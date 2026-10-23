// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Paint: put a material on one side of faces and on instances; add, edit and purge
//! materials, each as one undo step.
//!
//! Behaviour: `docs/specs/m4-paint.md`.

use std::collections::BTreeSet;

use crate::model::{
    texture_size_is_good, BodyId, FaceMaterials, Material, MaterialId, Model, ModelEdit,
    ModelError, UndoStack,
};
use crate::select::Entity;
use crate::topology::FaceId;

/// The side of a face that paint goes on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Front,
    Back,
}

/// Why a paint or material operation was refused. The model and the undo stack are
/// unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaintError {
    /// Nothing to paint.
    Empty,
    /// The body doesn't have this edge, face or instance (the first one in target order).
    Missing(Entity),
    /// A color channel is not finite or outside 0..=1.
    BadColor,
    /// The model refused a record (also: missing body or material).
    Model(ModelError),
}

fn good_color(color: [f32; 4]) -> bool {
    color
        .iter()
        .all(|c| c.is_finite() && (0.0..=1.0).contains(c))
}

fn name_with_base(base: &str, taken: impl Fn(&str) -> bool) -> String {
    let base = base.trim();
    let base = if base.is_empty() { "Material" } else { base };
    if !taken(base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base} {n}");
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Paint `material` (`None` = the default material) on `side` of every face in `targets`
/// and on every instance in `targets`; edges are skipped. Returns how many entities
/// changed; pushes one undo step named `label` when that is more than zero.
pub fn paint(
    model: &mut Model,
    body: BodyId,
    targets: &[Entity],
    side: Side,
    material: Option<MaterialId>,
    undo: &mut UndoStack,
    label: &str,
) -> Result<usize, PaintError> {
    if targets.is_empty() {
        return Err(PaintError::Empty);
    }
    let current = model
        .body(body)
        .ok_or(PaintError::Model(ModelError::MissingBody(body)))?;
    if let Some(id) = material
        && model.material(id).is_none()
    {
        return Err(PaintError::Model(ModelError::MissingMaterial(id)));
    }
    for &entity in targets {
        let present = match entity {
            Entity::Edge(e) => current.geometry().edge(e).is_some(),
            Entity::Face(f) => current.geometry().face(f).is_some(),
            Entity::Instance(i) => current.instance(i).is_some(),
        };
        if !present {
            return Err(PaintError::Missing(entity));
        }
    }

    let mut faces: BTreeSet<FaceId> = BTreeSet::new();
    let mut instances = BTreeSet::new();
    for &entity in targets {
        match entity {
            Entity::Edge(_) => {}
            Entity::Face(f) => {
                faces.insert(f);
            }
            Entity::Instance(i) => {
                instances.insert(i);
            }
        }
    }

    let mut log = ModelEdit::new();
    let outcome = (|| -> Result<usize, PaintError> {
        let mut changed = 0;
        for &face in &faces {
            let from = model
                .body(body)
                .map(|b| b.face_materials(face))
                .unwrap_or_default();
            let to = match side {
                Side::Front => FaceMaterials {
                    front: material,
                    ..from
                },
                Side::Back => FaceMaterials {
                    back: material,
                    ..from
                },
            };
            if to == from {
                continue;
            }
            model
                .set_face_materials(body, face, to, &mut log)
                .map_err(PaintError::Model)?;
            changed += 1;
        }
        for &instance in &instances {
            let from = model.body(body).and_then(|b| b.instance(instance)).cloned();
            let Some(mut updated) = from else {
                return Err(PaintError::Missing(Entity::Instance(instance)));
            };
            if updated.material == material {
                continue;
            }
            updated.material = material;
            model
                .set_instance(body, instance, updated, &mut log)
                .map_err(PaintError::Model)?;
            changed += 1;
        }
        Ok(changed)
    })();

    match outcome {
        Ok(changed) => {
            if changed > 0 {
                undo.push(label, log);
            }
            Ok(changed)
        }
        Err(error) => {
            let _ = model.apply_edit(&log.inverse());
            Err(error)
        }
    }
}

/// The material on `side` of a face, or on an instance (`side` ignored). Outer `None`: the
/// entity is an edge or isn't in the body.
pub fn sample(
    model: &Model,
    body: BodyId,
    entity: Entity,
    side: Side,
) -> Option<Option<MaterialId>> {
    let body = model.body(body)?;
    match entity {
        Entity::Face(f) => {
            let _ = body.geometry().face(f)?;
            let materials = body.face_materials(f);
            Some(match side {
                Side::Front => materials.front,
                Side::Back => materials.back,
            })
        }
        Entity::Instance(i) => Some(body.instance(i)?.material),
        Entity::Edge(_) => None,
    }
}

/// `base` if no material has that name, else `base 2`, `base 3`, ... (the first free one).
/// An empty or blank `base` counts as `Material`.
pub fn unique_material_name(model: &Model, base: &str) -> String {
    name_with_base(base, |name| {
        model.materials().any(|(_, material)| material.name == name)
    })
}

/// Add `material` under a unique name (see [`unique_material_name`]) as one undo step.
pub fn create_material(
    model: &mut Model,
    material: Material,
    undo: &mut UndoStack,
    label: &str,
) -> Result<MaterialId, PaintError> {
    if !good_color(material.color) {
        return Err(PaintError::BadColor);
    }
    if !texture_size_is_good(&material) {
        return Err(PaintError::Model(ModelError::BadTexture));
    }
    let mut material = material;
    material.name = unique_material_name(model, &material.name);
    let mut log = ModelEdit::new();
    let id = model.add_material(material, &mut log);
    undo.push(label, log);
    Ok(id)
}

/// Replace material `id` with `material` as one undo step (none when nothing changes). A
/// name used by another material is made unique the same way.
pub fn edit_material(
    model: &mut Model,
    id: MaterialId,
    material: Material,
    undo: &mut UndoStack,
    label: &str,
) -> Result<(), PaintError> {
    let stored = model
        .material(id)
        .cloned()
        .ok_or(PaintError::Model(ModelError::MissingMaterial(id)))?;
    if !good_color(material.color) {
        return Err(PaintError::BadColor);
    }
    if !texture_size_is_good(&material) {
        return Err(PaintError::Model(ModelError::BadTexture));
    }
    let mut material = material;
    material.name = name_with_base(&material.name, |name| {
        model
            .materials()
            .any(|(other, material)| other != id && material.name == name)
    });
    if material == stored {
        return Ok(());
    }
    let mut log = ModelEdit::new();
    match model.set_material(id, material, &mut log) {
        Ok(()) => {
            undo.push(label, log);
            Ok(())
        }
        Err(error) => {
            let _ = model.apply_edit(&log.inverse());
            Err(PaintError::Model(error))
        }
    }
}

/// Remove every material that no existing face side and no instance uses, as one undo
/// step (none when nothing goes). Returns the removed ids, ascending.
pub fn purge_materials(
    model: &mut Model,
    undo: &mut UndoStack,
    label: &str,
) -> Result<Vec<MaterialId>, PaintError> {
    let bodies: Vec<BodyId> = std::iter::once(BodyId::Root)
        .chain(model.definitions().map(|(id, _)| BodyId::Definition(id)))
        .collect();

    let mut used = BTreeSet::new();
    for &body_id in &bodies {
        let Some(body) = model.body(body_id) else {
            continue;
        };
        for (_, instance) in body.instances() {
            if let Some(material) = instance.material {
                used.insert(material);
            }
        }
        for (face, materials) in body.face_material_entries() {
            if body.geometry().face(face).is_none() {
                continue;
            }
            if let Some(material) = materials.front {
                used.insert(material);
            }
            if let Some(material) = materials.back {
                used.insert(material);
            }
        }
    }

    let unused: Vec<MaterialId> = model
        .materials()
        .filter(|(id, _)| !used.contains(id))
        .map(|(id, _)| id)
        .collect();
    if unused.is_empty() {
        return Ok(Vec::new());
    }
    let unused_set: BTreeSet<MaterialId> = unused.iter().copied().collect();

    let mut log = ModelEdit::new();
    let outcome = (|| -> Result<(), ModelError> {
        for &body_id in &bodies {
            let entries: Vec<(FaceId, FaceMaterials)> = {
                let body = model.body(body_id).expect("body exists");
                body.face_material_entries()
                    .filter(|(face, _)| body.geometry().face(*face).is_none())
                    .collect()
            };
            for (face, from) in entries {
                let mut to = from;
                if let Some(material) = to.front
                    && unused_set.contains(&material)
                {
                    to.front = None;
                }
                if let Some(material) = to.back
                    && unused_set.contains(&material)
                {
                    to.back = None;
                }
                if to != from {
                    model.set_face_materials(body_id, face, to, &mut log)?;
                }
            }
        }
        for &id in &unused {
            model.remove_material(id, &mut log)?;
        }
        Ok(())
    })();

    match outcome {
        Ok(()) => {
            undo.push(label, log);
            Ok(unused)
        }
        Err(error) => {
            let _ = model.apply_edit(&log.inverse());
            Err(PaintError::Model(error))
        }
    }
}
