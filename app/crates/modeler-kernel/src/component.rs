// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Make component / make group: move a selection of a body into a new definition and put
//! one instance of it in its place, as one undo step.
//!
//! Behaviour: `docs/specs/m4-make-component.md`.

use std::collections::{BTreeMap, BTreeSet};

use glam::{DMat4, DVec3};

use crate::model::{
    BodyId, DefinitionInfo, FaceMaterials, Instance, InstanceId, Model, ModelEdit, ModelError,
    ModelViolation, UndoStack,
};
use crate::select::Entity;
use crate::tools::scale;
use crate::topology::{EdgeId, EdgeUse, Face, FaceId, Loop, TopoError, VertexId};

/// Why making a component was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum ComponentError {
    /// Nothing is selected.
    Empty,
    /// The body doesn't have this edge, face or instance (the first one in selection order).
    Missing(Entity),
    /// The model refused a record (also: the body doesn't exist).
    Model(ModelError),
    /// The result would add these `Model::validate` violations.
    Invalid(Vec<ModelViolation>),
}

/// The definition name to use for `base`: trimmed; blank becomes `Group` or `Component`
/// (by `group`); then `"<name> <n>"`, smallest `n >= 2`, while a definition has that name.
pub fn unique_definition_name(model: &Model, base: &str, group: bool) -> String {
    let trimmed = base.trim();
    let base = if trimmed.is_empty() {
        if group { "Group" } else { "Component" }
    } else {
        trimmed
    };
    let taken = |name: &str| model.definitions().any(|(_, info)| info.name == name);
    if !taken(base) {
        return base.to_string();
    }
    let mut n = 2u64;
    loop {
        let candidate = format!("{base} {n}");
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

fn rollback(model: &mut Model, edit: &ModelEdit) {
    let _ = model.apply_edit(&edit.inverse());
}

fn copy_loop(l: &Loop, edges: &BTreeMap<EdgeId, EdgeId>) -> Loop {
    Loop {
        uses: l
            .uses
            .iter()
            .map(|u| EdgeUse {
                edge: edges[&u.edge],
                reversed: u.reversed,
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

/// Move `selection` out of `body` into a new definition described by `info` (name made
/// unique) and place one instance of it where the selection was. Returns the instance.
pub fn make_component(
    model: &mut Model,
    body: BodyId,
    selection: &[Entity],
    info: DefinitionInfo,
    undo: &mut UndoStack,
    label: &str,
) -> Result<InstanceId, ComponentError> {
    if selection.is_empty() {
        return Err(ComponentError::Empty);
    }
    if model.body(body).is_none() {
        return Err(ComponentError::Model(ModelError::MissingBody(body)));
    }
    for &entity in selection {
        let exists = model.body(body).is_some_and(|target| match entity {
            Entity::Edge(id) => target.geometry().edge(id).is_some(),
            Entity::Face(id) => target.geometry().face(id).is_some(),
            Entity::Instance(id) => target.instance(id).is_some(),
        });
        if !exists {
            return Err(ComponentError::Missing(entity));
        }
    }

    let (faces, edges, vertices, face_data, edge_data, vertex_data, material_data, instance_data) = {
        let target = model.body(body).expect("body checked above");
        let ctx = target.geometry();
        let mut faces = BTreeSet::new();
        let mut edge_ids = BTreeSet::new();
        let mut instances = BTreeSet::new();
        for &entity in selection {
            match entity {
                Entity::Face(id) => {
                    faces.insert(id);
                }
                Entity::Edge(id) => {
                    edge_ids.insert(id);
                }
                Entity::Instance(id) => {
                    instances.insert(id);
                }
            }
        }
        for &id in &faces {
            let face = ctx.face(id).expect("face checked above");
            for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
                edge_ids.extend(l.uses.iter().map(|u| u.edge));
            }
        }
        let mut vertex_ids = BTreeSet::new();
        for &id in &edge_ids {
            let edge = ctx.edge(id).expect("edge checked above");
            vertex_ids.extend(edge.ends);
        }
        let face_data: Vec<(FaceId, Face)> = faces
            .iter()
            .filter_map(|&id| ctx.face(id).map(|f| (id, f.clone())))
            .collect();
        let edge_data: Vec<(EdgeId, [VertexId; 2])> = edge_ids
            .iter()
            .filter_map(|&id| ctx.edge(id).map(|e| (id, e.ends)))
            .collect();
        let vertex_data: Vec<(VertexId, DVec3)> = vertex_ids
            .iter()
            .filter_map(|&id| ctx.vertex(id).map(|v| (id, v.position)))
            .collect();
        let material_data: Vec<(FaceId, FaceMaterials)> = face_data
            .iter()
            .map(|&(id, _)| (id, target.face_materials(id)))
            .filter(|(_, m)| *m != FaceMaterials::default())
            .collect();
        let instance_data: Vec<(InstanceId, Instance)> = instances
            .iter()
            .filter_map(|&id| target.instance(id).map(|i| (id, i.clone())))
            .collect();
        (
            faces,
            edge_ids,
            vertex_ids,
            face_data,
            edge_data,
            vertex_data,
            material_data,
            instance_data,
        )
    };

    let origin = scale::bounds(model, body, selection).map_or(DVec3::ZERO, |b| b.min);
    let transform = DMat4::from_translation(origin);
    let inverse = DMat4::from_translation(-origin);

    let before = model.validate();
    let mut edit = ModelEdit::new();
    let definition = model.add_definition(
        DefinitionInfo {
            name: unique_definition_name(model, &info.name, info.group),
            group: info.group,
        },
        &mut edit,
    );
    let inside = BodyId::Definition(definition);

    let mut face_map = BTreeMap::new();
    let result = model.edit_geometry(inside, &mut edit, |ctx, log| {
        let mut vertex_map = BTreeMap::new();
        for &(id, position) in &vertex_data {
            vertex_map.insert(id, ctx.insert_vertex(position - origin, log));
        }
        let mut edge_map = BTreeMap::new();
        for &(id, ends) in &edge_data {
            let a = vertex_map[&ends[0]];
            let b = vertex_map[&ends[1]];
            edge_map.insert(id, ctx.insert_edge(a, b, log)?);
        }
        for (id, face) in &face_data {
            face_map.insert(*id, ctx.insert_face(copy_face(face, &edge_map), log)?);
        }
        Ok::<(), TopoError>(())
    });
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            rollback(model, &edit);
            return Err(ComponentError::Model(ModelError::Topo(error)));
        }
        Err(error) => {
            rollback(model, &edit);
            return Err(ComponentError::Model(error));
        }
    }

    for &(old, materials) in &material_data {
        let new = face_map[&old];
        if let Err(error) = model.set_face_materials(inside, new, materials, &mut edit) {
            rollback(model, &edit);
            return Err(ComponentError::Model(error));
        }
    }

    for &(id, ref instance) in &instance_data {
        let mut copy = instance.clone();
        copy.transform = inverse * copy.transform;
        if let Err(error) = model.add_instance(inside, copy, &mut edit) {
            rollback(model, &edit);
            return Err(ComponentError::Model(error));
        }
        if let Err(error) = model.remove_instance(body, id, &mut edit) {
            rollback(model, &edit);
            return Err(ComponentError::Model(error));
        }
    }

    let result = model.edit_geometry(body, &mut edit, |ctx, log| {
        for &id in &faces {
            ctx.delete_face(id, log)?;
        }
        for &id in &edges {
            if ctx.faces_of_edge(id).is_empty() {
                ctx.delete_edge(id, log)?;
            }
        }
        for &id in &vertices {
            if ctx.edges_of_vertex(id).is_empty() {
                ctx.delete_vertex(id, log)?;
            }
        }
        Ok::<(), TopoError>(())
    });
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            rollback(model, &edit);
            return Err(ComponentError::Model(ModelError::Topo(error)));
        }
        Err(error) => {
            rollback(model, &edit);
            return Err(ComponentError::Model(error));
        }
    }

    let instance = match model.add_instance(
        body,
        Instance {
            definition,
            transform,
            name: String::new(),
            material: None,
        },
        &mut edit,
    ) {
        Ok(id) => id,
        Err(error) => {
            rollback(model, &edit);
            return Err(ComponentError::Model(error));
        }
    };

    let invalid: Vec<ModelViolation> = model
        .validate()
        .into_iter()
        .filter(|v| !before.contains(v))
        .collect();
    if !invalid.is_empty() {
        rollback(model, &edit);
        return Err(ComponentError::Invalid(invalid));
    }

    undo.push(label, edit);
    Ok(instance)
}
