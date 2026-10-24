// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Copy: take a snapshot of a selection (the clipboard) and put copies of it into a body,
//! stuck to the geometry there, as one undo step. Used by Move with copy and by
//! Cut / Copy / Paste.
//!
//! Behaviour: `docs/specs/m4-copy.md`.

use std::collections::{BTreeMap, BTreeSet};

use glam::{DMat4, DVec3};

use crate::component::unique_definition_name;
use crate::explode::{ExplodeError, explode_inner};
use crate::model::{
    Body, BodyId, DefinitionId, DefinitionInfo, FaceMaterials, Instance, MaterialId, Model,
    ModelEdit, ModelError, ModelRecord, ModelViolation, UndoStack,
};
use crate::paint::unique_material_name;
use crate::select::Entity;
use crate::topology::{EdgeId, EdgeUse, Face, Loop, TopoError};

/// Why a copy, extract or insert was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum CopyError {
    /// Nothing is selected, or the clipboard holds nothing.
    Empty,
    /// The body doesn't have this edge, face or instance (the first one in selection order).
    Missing(Entity),
    /// The matrix is not finite, not affine, or (nearly) singular.
    BadMatrix,
    /// The model refused a record (also: the body doesn't exist, or a cycle of instances).
    Model(ModelError),
    /// The result would add these `Model::validate` violations.
    Invalid(Vec<ModelViolation>),
}

/// A self-contained snapshot of a selection: its geometry, face materials and instances in
/// the source body's coordinates, with every definition and material they reach.
#[derive(Clone, Debug, PartialEq)]
pub struct Clipboard {
    model: Model,
}

impl Clipboard {
    /// The snapshot as a model: its root holds the copied selection; its definitions and
    /// materials are exact copies of the source's, under the source's ids.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Whether the snapshot has no edge, face or instance.
    pub fn is_empty(&self) -> bool {
        let root = self.model.root();
        root.geometry().edge_count() == 0
            && root.geometry().face_count() == 0
            && root.instances().next().is_none()
    }

    /// The smallest axis-aligned box (min, max) around every copied vertex and every vertex
    /// the copied instances show, in the source body's coordinates; `None` if there are none.
    pub fn bounds(&self) -> Option<(DVec3, DVec3)> {
        let root = self.model.root();
        let mut selection = Vec::new();
        selection.extend(root.geometry().edges().map(|(id, _)| Entity::Edge(id)));
        selection.extend(root.geometry().faces().map(|(id, _)| Entity::Face(id)));
        selection.extend(root.instances().map(|(id, _)| Entity::Instance(id)));
        crate::tools::scale::bounds(&self.model, BodyId::Root, &selection)
            .map(|bounds| (bounds.min, bounds.max))
    }
}

fn has_entity(body: &Body, entity: Entity) -> bool {
    match entity {
        Entity::Edge(id) => body.geometry().edge(id).is_some(),
        Entity::Face(id) => body.geometry().face(id).is_some(),
        Entity::Instance(id) => body.instance(id).is_some(),
    }
}

fn copy_loop(loop_: &Loop, edges: &BTreeMap<EdgeId, EdgeId>) -> Loop {
    Loop {
        uses: loop_
            .uses
            .iter()
            .map(|use_| EdgeUse {
                edge: edges[&use_.edge],
                reversed: use_.reversed,
            })
            .collect(),
    }
}

fn copy_face(face: &Face, edges: &BTreeMap<EdgeId, EdgeId>) -> Face {
    Face {
        outer: copy_loop(&face.outer, edges),
        inner: face.inner.iter().map(|l| copy_loop(l, edges)).collect(),
    }
}

fn add_loop_edges(loop_: &Loop, edges: &mut BTreeSet<EdgeId>) {
    edges.extend(loop_.uses.iter().map(|use_| use_.edge));
}

fn reachable_definitions(
    model: &Model,
    roots: impl IntoIterator<Item = DefinitionId>,
) -> BTreeSet<DefinitionId> {
    let mut found = BTreeSet::new();
    let mut stack: Vec<DefinitionId> = roots.into_iter().collect();
    while let Some(definition) = stack.pop() {
        if !found.insert(definition) {
            continue;
        }
        if let Some(body) = model.body(BodyId::Definition(definition)) {
            stack.extend(body.instances().map(|(_, instance)| instance.definition));
        }
    }
    found
}

/// Definitions with every definition their instances place before them.
fn definition_order(model: &Model, roots: &BTreeSet<DefinitionId>) -> Vec<DefinitionId> {
    fn visit(
        model: &Model,
        definition: DefinitionId,
        seen: &mut BTreeSet<DefinitionId>,
        order: &mut Vec<DefinitionId>,
    ) {
        if !seen.insert(definition) {
            return;
        }
        if let Some(body) = model.body(BodyId::Definition(definition)) {
            for (_, instance) in body.instances() {
                visit(model, instance.definition, seen, order);
            }
        }
        order.push(definition);
    }

    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    for &definition in roots {
        visit(model, definition, &mut seen, &mut order);
    }
    order
}

fn used_materials(body: &Body) -> BTreeSet<MaterialId> {
    let mut used = BTreeSet::new();
    for (_, materials) in body.face_material_entries() {
        used.extend(materials.front);
        used.extend(materials.back);
    }
    used.extend(
        body.instances()
            .filter_map(|(_, instance)| instance.material),
    );
    used
}

fn topo<T>(result: Result<Result<T, TopoError>, ModelError>) -> Result<T, ModelError> {
    result?.map_err(ModelError::Topo)
}

/// Snapshot `selection` of `body`. The model is not changed.
pub fn extract(model: &Model, body: BodyId, selection: &[Entity]) -> Result<Clipboard, CopyError> {
    if selection.is_empty() {
        return Err(CopyError::Empty);
    }
    let Some(source) = model.body(body) else {
        return Err(CopyError::Model(ModelError::MissingBody(body)));
    };
    for &entity in selection {
        if !has_entity(source, entity) {
            return Err(CopyError::Missing(entity));
        }
    }

    let mut faces = BTreeSet::new();
    let mut edges = BTreeSet::new();
    let mut instances = BTreeSet::new();
    for &entity in selection {
        match entity {
            Entity::Face(id) => {
                faces.insert(id);
            }
            Entity::Edge(id) => {
                edges.insert(id);
            }
            Entity::Instance(id) => {
                instances.insert(id);
            }
        }
    }
    for &face in &faces {
        if let Some(face) = source.geometry().face(face) {
            add_loop_edges(&face.outer, &mut edges);
            for hole in &face.inner {
                add_loop_edges(hole, &mut edges);
            }
        }
    }
    let mut vertices = BTreeSet::new();
    for &edge in &edges {
        if let Some(edge) = source.geometry().edge(edge) {
            vertices.extend(edge.ends);
        }
    }

    let definitions = reachable_definitions(
        model,
        instances
            .iter()
            .filter_map(|&id| source.instance(id).map(|instance| instance.definition)),
    );

    let mut materials = BTreeSet::new();
    for &face in &faces {
        let face_materials = source.face_materials(face);
        materials.extend(face_materials.front);
        materials.extend(face_materials.back);
    }
    for &id in &instances {
        if let Some(instance) = source.instance(id) {
            materials.extend(instance.material);
        }
    }
    for &definition in &definitions {
        if let Some(definition_body) = model.body(BodyId::Definition(definition)) {
            materials.extend(used_materials(definition_body));
        }
    }

    let mut clipboard = Model::new();
    let mut log = ModelEdit::new();
    for &id in &materials {
        if let Some(material) = model.material(id) {
            let record = ModelRecord::AddMaterial(id, material.clone());
            clipboard.apply(&record).map_err(CopyError::Model)?;
            log.records.push(record);
        }
    }
    for definition in definition_order(model, &definitions) {
        let Some(info) = model.definition(definition) else {
            return Err(CopyError::Model(ModelError::MissingDefinition(definition)));
        };
        let Some(source_body) = model.body(BodyId::Definition(definition)) else {
            return Err(CopyError::Model(ModelError::MissingDefinition(definition)));
        };
        clipboard
            .add_definition_with_body(definition, info.clone(), source_body, &mut log)
            .map_err(CopyError::Model)?;
    }

    let mut vertex_map = BTreeMap::new();
    let mut edge_map = BTreeMap::new();
    let mut face_map = BTreeMap::new();
    topo(clipboard.edit_geometry(BodyId::Root, &mut log, |ctx, log| {
        for &id in &vertices {
            let Some(vertex) = source.geometry().vertex(id) else {
                continue;
            };
            vertex_map.insert(id, ctx.insert_vertex(vertex.position, log));
        }
        for &id in &edges {
            let Some(edge) = source.geometry().edge(id) else {
                continue;
            };
            let a = vertex_map[&edge.ends[0]];
            let b = vertex_map[&edge.ends[1]];
            edge_map.insert(id, ctx.insert_edge(a, b, log)?);
        }
        for &id in &faces {
            let Some(face) = source.geometry().face(id) else {
                continue;
            };
            let new = ctx.insert_face(copy_face(face, &edge_map), log)?;
            face_map.insert(id, new);
        }
        Ok(())
    }))
    .map_err(CopyError::Model)?;

    for (&old, &new) in &face_map {
        let materials = source.face_materials(old);
        if materials != FaceMaterials::default() {
            clipboard
                .set_face_materials(BodyId::Root, new, materials, &mut log)
                .map_err(CopyError::Model)?;
        }
    }
    for &id in &instances {
        if let Some(instance) = source.instance(id) {
            clipboard
                .add_instance(BodyId::Root, instance.clone(), &mut log)
                .map_err(CopyError::Model)?;
        }
    }

    Ok(Clipboard { model: clipboard })
}

fn bad_matrix(matrix: &DMat4) -> bool {
    if matrix
        .to_cols_array()
        .iter()
        .any(|value| !value.is_finite())
    {
        return true;
    }
    if matrix.x_axis.w != 0.0
        || matrix.y_axis.w != 0.0
        || matrix.z_axis.w != 0.0
        || matrix.w_axis.w != 1.0
    {
        return true;
    }
    matrix.determinant().abs() < 1e-9
}

fn mapped_materials(
    materials: FaceMaterials,
    map: &BTreeMap<MaterialId, MaterialId>,
) -> FaceMaterials {
    let mapped = |id: Option<MaterialId>| id.map(|id| map.get(&id).copied().unwrap_or(id));
    FaceMaterials {
        front: mapped(materials.front),
        back: mapped(materials.back),
        front_map: materials.front_map,
        back_map: materials.back_map,
    }
}

fn copy_body_into(
    model: &mut Model,
    target: BodyId,
    source: &Body,
    definitions: &BTreeMap<DefinitionId, DefinitionId>,
    materials: &BTreeMap<MaterialId, MaterialId>,
    edit: &mut ModelEdit,
) -> Result<(), ModelError> {
    let mut vertex_map = BTreeMap::new();
    let mut edge_map = BTreeMap::new();
    let mut face_map = BTreeMap::new();
    topo(model.edit_geometry(target, edit, |ctx, log| {
        for (id, vertex) in source.geometry().vertices() {
            vertex_map.insert(id, ctx.insert_vertex(vertex.position, log));
        }
        for (id, edge) in source.geometry().edges() {
            let a = vertex_map[&edge.ends[0]];
            let b = vertex_map[&edge.ends[1]];
            edge_map.insert(id, ctx.insert_edge(a, b, log)?);
        }
        for (id, face) in source.geometry().faces() {
            let new = ctx.insert_face(copy_face(face, &edge_map), log)?;
            face_map.insert(id, new);
        }
        Ok(())
    }))?;

    for (face, face_materials) in source.face_material_entries() {
        let Some(&new) = face_map.get(&face) else {
            continue;
        };
        let face_materials = mapped_materials(face_materials, materials);
        if face_materials != FaceMaterials::default() {
            model.set_face_materials(target, new, face_materials, edit)?;
        }
    }
    for (_, instance) in source.instances() {
        let instance = Instance {
            definition: definitions
                .get(&instance.definition)
                .copied()
                .unwrap_or(instance.definition),
            transform: instance.transform,
            name: instance.name.clone(),
            material: instance
                .material
                .map(|id| materials.get(&id).copied().unwrap_or(id)),
        };
        model.add_instance(target, instance, edit)?;
    }
    Ok(())
}

fn map_explode(error: ExplodeError) -> CopyError {
    match error {
        ExplodeError::Model(error) => CopyError::Model(error),
        ExplodeError::Invalid(violations) => CopyError::Invalid(violations),
        ExplodeError::MissingInstance(id) => {
            CopyError::Model(ModelError::MissingInstance(BodyId::Root, id))
        }
        ExplodeError::BadTransform => CopyError::BadMatrix,
    }
}

fn insert_inner(
    model: &mut Model,
    body: BodyId,
    clipboard: &Clipboard,
    matrix: DMat4,
    edit: &mut ModelEdit,
) -> Result<Vec<Entity>, CopyError> {
    let clip = clipboard.model();
    let mut material_map = BTreeMap::new();
    for (id, material) in clip.materials() {
        let target = if model.material(id) == Some(material) {
            id
        } else if let Some((target, _)) = model.materials().find(|(_, other)| *other == material) {
            target
        } else {
            let mut material = material.clone();
            material.name = unique_material_name(model, &material.name);
            model.add_material(material, edit)
        };
        material_map.insert(id, target);
    }

    let definitions: BTreeSet<DefinitionId> = clip.definitions().map(|(id, _)| id).collect();
    let mut definition_map = BTreeMap::new();
    for definition in definition_order(clip, &definitions) {
        let Some(info) = clip.definition(definition) else {
            continue;
        };
        let Some(clip_body) = clip.body(BodyId::Definition(definition)) else {
            continue;
        };
        let materials_self = used_materials(clip_body)
            .iter()
            .all(|id| material_map.get(id).copied() == Some(*id));
        let definitions_self = clip_body.instances().all(|(_, instance)| {
            definition_map.get(&instance.definition).copied() == Some(instance.definition)
        });
        let reuse = !info.group
            && model.definition(definition) == Some(info)
            && model.body(BodyId::Definition(definition)) == Some(clip_body)
            && materials_self
            && definitions_self;
        if reuse {
            definition_map.insert(definition, definition);
        } else {
            let info = DefinitionInfo {
                name: unique_definition_name(model, &info.name, info.group),
                group: info.group,
            };
            let new = model.add_definition(info, edit);
            definition_map.insert(definition, new);
            copy_body_into(
                model,
                BodyId::Definition(new),
                clip_body,
                &definition_map,
                &material_map,
                edit,
            )
            .map_err(CopyError::Model)?;
        }
    }

    let holder = model.add_definition(
        DefinitionInfo {
            name: "Clipboard".into(),
            group: true,
        },
        edit,
    );
    copy_body_into(
        model,
        BodyId::Definition(holder),
        clip.root(),
        &definition_map,
        &material_map,
        edit,
    )
    .map_err(CopyError::Model)?;
    let instance = model
        .add_instance(
            body,
            Instance {
                definition: holder,
                transform: matrix,
                name: String::new(),
                material: None,
            },
            edit,
        )
        .map_err(CopyError::Model)?;
    explode_inner(model, body, instance, edit).map_err(map_explode)
}

fn rollback(model: &mut Model, edit: &ModelEdit) {
    let _ = model.apply_edit(&edit.inverse());
}

/// Put a copy of `clipboard` into `body`, each point at `matrix · p`, as one undo step
/// labelled `label`. Returns the entities of `body` that came from it, sorted.
pub fn insert(
    model: &mut Model,
    body: BodyId,
    clipboard: &Clipboard,
    matrix: DMat4,
    undo: &mut UndoStack,
    label: &str,
) -> Result<Vec<Entity>, CopyError> {
    if model.body(body).is_none() {
        return Err(CopyError::Model(ModelError::MissingBody(body)));
    }
    if clipboard.is_empty() {
        return Err(CopyError::Empty);
    }
    if bad_matrix(&matrix) {
        return Err(CopyError::BadMatrix);
    }

    let before = model.validate();
    let mut edit = ModelEdit::new();
    match insert_inner(model, body, clipboard, matrix, &mut edit) {
        Ok(entities) => {
            let added: Vec<ModelViolation> = model
                .validate()
                .into_iter()
                .filter(|violation| !before.contains(violation))
                .collect();
            if !added.is_empty() {
                rollback(model, &edit);
                return Err(CopyError::Invalid(added));
            }
            undo.push(label, edit);
            Ok(entities)
        }
        Err(error) => {
            rollback(model, &edit);
            Err(error)
        }
    }
}

/// `extract` then `insert` into the same body: copies `selection` of `body` to `matrix`
/// times where it is, as one undo step. Returns the copy's entities, sorted.
pub fn copy_selection(
    model: &mut Model,
    body: BodyId,
    selection: &[Entity],
    matrix: DMat4,
    undo: &mut UndoStack,
    label: &str,
) -> Result<Vec<Entity>, CopyError> {
    let clipboard = extract(model, body, selection)?;
    insert(model, body, &clipboard, matrix, undo, label)
}
