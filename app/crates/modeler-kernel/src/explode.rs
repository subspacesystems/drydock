// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Explode: put an instance's contents into the body that holds it, stuck to the geometry
//! there, and remove the instance, as one undo step.
//!
//! Behaviour: `docs/specs/m4-explode.md`.

use std::collections::{BTreeMap, BTreeSet};

use glam::{DMat4, DVec3};

use crate::geom::{self, LINEAR_TOL};
use crate::model::{
    BodyId, DefinitionId, FaceMaterials, Instance, InstanceId, Model, ModelEdit, ModelError,
    ModelViolation, UndoStack,
};
use crate::movemerge::{interior_point, restick};
use crate::select::Entity;
use crate::texture::moved_maps;
use crate::topology::{Context, EdgeId, EdgeUse, Face, FaceId, Loop, TopoError, VertexId};

/// Why an explode was refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum ExplodeError {
    /// The body has no such instance.
    MissingInstance(InstanceId),
    /// The instance's transform has a non-finite entry or isn't invertible.
    BadTransform,
    /// The model refused a record (also: the body doesn't exist).
    Model(ModelError),
    /// The result would add these `Model::validate` violations.
    Invalid(Vec<ModelViolation>),
}

/// A face from before the explode, or a copy of one, used to classify the faces sticking
/// leaves behind and to recover their orientation and materials.
struct RefFace {
    materials: FaceMaterials,
    normal: DVec3,
    outer: Vec<DVec3>,
    inner: Vec<Vec<DVec3>>,
}

impl RefFace {
    fn new(materials: FaceMaterials, outer: Vec<DVec3>, inner: Vec<Vec<DVec3>>) -> Self {
        let normal = geom::fit_plane(&outer)
            .map(|plane| plane.normal)
            .unwrap_or(DVec3::ZERO);
        Self {
            materials,
            normal,
            outer,
            inner,
        }
    }

    fn contains(&self, p: DVec3) -> bool {
        let Some(plane) = geom::fit_plane(&self.outer) else {
            return false;
        };
        if plane.signed_distance(p).abs() > LINEAR_TOL {
            return false;
        }
        let n = plane.normal;
        let q = geom::to_plane_2d(&[p], n)[0];
        let outer = geom::to_plane_2d(&self.outer, n);
        if geom::point_in_polygon_2d(q, &outer, LINEAR_TOL) != geom::Containment::Inside {
            return false;
        }
        self.inner.iter().all(|hole| {
            let hole = geom::to_plane_2d(hole, n);
            geom::point_in_polygon_2d(q, &hole, LINEAR_TOL) == geom::Containment::Outside
        })
    }
}

fn reversed_loop(l: &Loop) -> Loop {
    Loop {
        uses: l
            .uses
            .iter()
            .rev()
            .map(|u| EdgeUse {
                edge: u.edge,
                reversed: !u.reversed,
            })
            .collect(),
    }
}

fn reversed_face(face: &Face) -> Face {
    Face {
        outer: reversed_loop(&face.outer),
        inner: face.inner.iter().rev().map(reversed_loop).collect(),
    }
}

fn swap_materials(materials: FaceMaterials) -> FaceMaterials {
    FaceMaterials {
        front: materials.back,
        back: materials.front,
        front_map: materials.back_map,
        back_map: materials.front_map,
    }
}

fn rollback(model: &mut Model, edit: &ModelEdit) {
    let _ = model.apply_edit(&edit.inverse());
}

fn topo<T>(result: Result<Result<T, TopoError>, ModelError>) -> Result<T, ExplodeError> {
    result
        .map_err(ExplodeError::Model)?
        .map_err(|error| ExplodeError::Model(ModelError::Topo(error)))
}

fn bad_transform(matrix: &DMat4) -> bool {
    matrix
        .to_cols_array()
        .iter()
        .any(|value| !value.is_finite())
        || matrix.determinant().abs() < 1e-12
}

fn mapped_face(face: &Face, edges: &BTreeMap<EdgeId, EdgeId>) -> Face {
    let mapped_loop = |l: &Loop| Loop {
        uses: l
            .uses
            .iter()
            .filter_map(|u| {
                edges.get(&u.edge).copied().map(|edge| EdgeUse {
                    edge,
                    reversed: u.reversed,
                })
            })
            .collect(),
    };
    Face {
        outer: mapped_loop(&face.outer),
        inner: face.inner.iter().map(mapped_loop).collect(),
    }
}

fn edge_points(ctx: &Context, edge: EdgeId) -> Option<[DVec3; 2]> {
    let edge = ctx.edge(edge)?;
    Some([
        ctx.vertex(edge.ends[0])?.position,
        ctx.vertex(edge.ends[1])?.position,
    ])
}

fn lies_on_segment(a: DVec3, b: DVec3, segment: [DVec3; 2]) -> bool {
    geom::point_segment_distance(a, segment[0], segment[1]) <= LINEAR_TOL
        && geom::point_segment_distance(b, segment[0], segment[1]) <= LINEAR_TOL
}

fn empty_group(
    model: &mut Model,
    definition: DefinitionId,
    edit: &mut ModelEdit,
) -> Result<(), ExplodeError> {
    let body = BodyId::Definition(definition);
    let (instances, faces, materials, edges, vertices) = {
        let Some(target) = model.body(body) else {
            return Err(ExplodeError::Model(ModelError::MissingBody(body)));
        };
        let ctx = target.geometry();
        (
            target.instances().map(|(id, _)| id).collect::<Vec<_>>(),
            ctx.faces().map(|(id, _)| id).collect::<Vec<_>>(),
            target
                .face_material_entries()
                .map(|(face, _)| face)
                .collect::<Vec<_>>(),
            ctx.edges().map(|(id, _)| id).collect::<Vec<_>>(),
            ctx.vertices().map(|(id, _)| id).collect::<Vec<_>>(),
        )
    };

    for id in instances {
        model
            .remove_instance(body, id, edit)
            .map_err(ExplodeError::Model)?;
    }
    for face in materials {
        model
            .set_face_materials(body, face, FaceMaterials::default(), edit)
            .map_err(ExplodeError::Model)?;
    }
    model
        .clear_guides(body, edit)
        .map_err(ExplodeError::Model)?;
    topo(model.edit_geometry(body, edit, |ctx, log| {
        for face in faces {
            ctx.delete_face(face, log)?;
        }
        for edge in edges {
            ctx.delete_edge(edge, log)?;
        }
        for vertex in vertices {
            ctx.delete_vertex(vertex, log)?;
        }
        Ok(())
    }))?;
    model
        .remove_definition(definition, edit)
        .map_err(ExplodeError::Model)
}

pub(crate) fn explode_inner(
    model: &mut Model,
    body: BodyId,
    instance: InstanceId,
    edit: &mut ModelEdit,
) -> Result<Vec<Entity>, ExplodeError> {
    let data = model
        .body(body)
        .and_then(|target| target.instance(instance))
        .cloned()
        .ok_or(ExplodeError::MissingInstance(instance))?;
    let definition = data.definition;
    let material = data.material;
    let group = model
        .definition(definition)
        .map(|info| info.group)
        .unwrap_or(false);

    let (
        source_vertices,
        source_edges,
        source_faces,
        source_instances,
        old_faces,
        copied_faces,
        copied_segments,
    ) = {
        let Some(source) = model.body(BodyId::Definition(definition)) else {
            return Err(ExplodeError::Model(ModelError::MissingDefinition(
                definition,
            )));
        };
        let src = source.geometry();
        let negative = data.transform.determinant() < 0.0;

        let vertices: Vec<(VertexId, DVec3)> = src
            .vertices()
            .map(|(id, vertex)| (id, data.transform.transform_point3(vertex.position)))
            .collect();
        let edges: Vec<(EdgeId, VertexId, VertexId)> = src
            .edges()
            .map(|(id, edge)| (id, edge.ends[0], edge.ends[1]))
            .collect();
        let faces: Vec<(Face, FaceMaterials)> = src
            .faces()
            .map(|(id, face)| {
                let base = source.face_materials(id);
                let materials = FaceMaterials {
                    front: base.front.or(material),
                    back: base.back.or(material),
                    ..base
                };
                let normal = geom::polygon_normal(&src.loop_points(&face.outer));
                (
                    face.clone(),
                    moved_maps(model, materials, normal, data.transform),
                )
            })
            .collect();

        let segments: Vec<[DVec3; 2]> = src
            .edges()
            .filter_map(|(id, _)| edge_points(src, id))
            .map(|[a, b]| {
                [
                    data.transform.transform_point3(a),
                    data.transform.transform_point3(b),
                ]
            })
            .collect();

        let mut copied = Vec::with_capacity(faces.len());
        for (face, materials) in &faces {
            let mut outer: Vec<DVec3> = src
                .loop_points(&face.outer)
                .into_iter()
                .map(|p| data.transform.transform_point3(p))
                .collect();
            let mut inner: Vec<Vec<DVec3>> = face
                .inner
                .iter()
                .map(|l| {
                    src.loop_points(l)
                        .into_iter()
                        .map(|p| data.transform.transform_point3(p))
                        .collect()
                })
                .collect();
            if negative {
                outer.reverse();
                for hole in &mut inner {
                    hole.reverse();
                }
            }
            copied.push(RefFace::new(*materials, outer, inner));
        }

        let instances: Vec<Instance> = source
            .instances()
            .map(|(_, instance)| Instance {
                definition: instance.definition,
                transform: data.transform * instance.transform,
                name: instance.name.clone(),
                material: instance.material.or(material),
            })
            .collect();

        let Some(target) = model.body(body) else {
            return Err(ExplodeError::Model(ModelError::MissingBody(body)));
        };
        let ctx = target.geometry();
        let old: Vec<RefFace> = ctx
            .faces()
            .map(|(id, face)| {
                RefFace::new(
                    target.face_materials(id),
                    ctx.loop_points(&face.outer),
                    face.inner.iter().map(|l| ctx.loop_points(l)).collect(),
                )
            })
            .collect();

        (vertices, edges, faces, instances, old, copied, segments)
    };

    let mut vertex_map: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    let mut edge_map: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    topo(model.edit_geometry(body, edit, |ctx, log| {
        for (old, position) in source_vertices {
            let new = ctx.insert_vertex(position, log);
            vertex_map.insert(old, new);
        }
        for (old, a, b) in source_edges {
            let Some(a) = vertex_map.get(&a).copied() else {
                return Err(TopoError::MissingVertex(a));
            };
            let Some(b) = vertex_map.get(&b).copied() else {
                return Err(TopoError::MissingVertex(b));
            };
            let new = ctx.insert_edge(a, b, log)?;
            edge_map.insert(old, new);
        }
        for (face, _) in &source_faces {
            let mut mapped = mapped_face(face, &edge_map);
            if data.transform.determinant() < 0.0 {
                mapped = reversed_face(&mapped);
            }
            ctx.insert_face(mapped, log)?;
        }
        Ok(())
    }))?;

    let copied_vertices: BTreeSet<VertexId> = vertex_map.values().copied().collect();

    let mut added_instances = Vec::with_capacity(source_instances.len());
    for source in source_instances {
        let id = model
            .add_instance(body, source, edit)
            .map_err(ExplodeError::Model)?;
        added_instances.push(id);
    }

    model
        .remove_instance(body, instance, edit)
        .map_err(ExplodeError::Model)?;

    restick(model, body, &copied_vertices, edit).map_err(ExplodeError::Model)?;

    // Drop the faces sticking made that don't cover an old or copied face.
    let unwanted: Vec<FaceId> = {
        let Some(target) = model.body(body) else {
            return Err(ExplodeError::Model(ModelError::MissingBody(body)));
        };
        let ctx = target.geometry();
        ctx.faces()
            .filter_map(|(id, _)| {
                let point = interior_point(ctx, id)?;
                let known = old_faces.iter().any(|face| face.contains(point))
                    || copied_faces.iter().any(|face| face.contains(point));
                (!known).then_some(id)
            })
            .collect()
    };
    topo(model.edit_geometry(body, edit, |ctx, log| {
        for face in unwanted {
            ctx.delete_face(face, log)?;
        }
        Ok(())
    }))?;

    // Orient the faces like the old or copied face they cover, and give them its materials.
    let plans: Vec<(FaceId, Face, bool, FaceMaterials)> = {
        let Some(target) = model.body(body) else {
            return Err(ExplodeError::Model(ModelError::MissingBody(body)));
        };
        let ctx = target.geometry();
        ctx.faces()
            .filter_map(|(id, face)| {
                let point = interior_point(ctx, id)?;
                let copied = copied_faces.iter().find(|source| source.contains(point));
                let old = old_faces.iter().find(|source| source.contains(point));
                let orientation = copied.or(old)?;

                let (material_source, materials) = match copied {
                    Some(source) if source.materials != FaceMaterials::default() => {
                        (Some(source), source.materials)
                    }
                    Some(source) => match old {
                        Some(source) => (Some(source), source.materials),
                        None => (Some(source), FaceMaterials::default()),
                    },
                    None => match old {
                        Some(source) => (Some(source), source.materials),
                        None => (None, FaceMaterials::default()),
                    },
                };
                let materials = match material_source {
                    Some(source) if source.normal.dot(orientation.normal) < 0.0 => {
                        swap_materials(materials)
                    }
                    _ => materials,
                };
                let reverse = ctx
                    .face_plane(id)
                    .is_some_and(|plane| plane.normal.dot(orientation.normal) < 0.0);
                Some((id, face.clone(), reverse, materials))
            })
            .collect()
    };

    let placed = topo(model.edit_geometry(body, edit, |ctx, log| {
        let mut out = Vec::with_capacity(plans.len());
        for (id, face, reverse, materials) in &plans {
            if *reverse {
                ctx.delete_face(*id, log)?;
                let new = ctx.insert_face(reversed_face(face), log)?;
                out.push((new, *materials));
            } else {
                out.push((*id, *materials));
            }
        }
        Ok(out)
    }))?;
    for (face, materials) in placed {
        model
            .set_face_materials(body, face, materials, edit)
            .map_err(ExplodeError::Model)?;
    }

    if group {
        empty_group(model, definition, edit)?;
    }

    let Some(target) = model.body(body) else {
        return Err(ExplodeError::Model(ModelError::MissingBody(body)));
    };
    let ctx = target.geometry();
    let mut result = BTreeSet::new();
    for (id, _) in ctx.edges() {
        let Some([a, b]) = edge_points(ctx, id) else {
            continue;
        };
        if copied_segments
            .iter()
            .any(|segment| lies_on_segment(a, b, *segment))
        {
            result.insert(Entity::Edge(id));
        }
    }
    for (id, _) in ctx.faces() {
        let Some(point) = interior_point(ctx, id) else {
            continue;
        };
        if copied_faces.iter().any(|face| face.contains(point)) {
            result.insert(Entity::Face(id));
        }
    }
    for id in added_instances {
        result.insert(Entity::Instance(id));
    }
    Ok(result.into_iter().collect())
}

/// Explode `instance` of `body`: its definition's geometry, face materials and nested
/// instances go into `body` where the instance showed them. Returns the entities of `body`
/// that came from it, sorted.
pub fn explode(
    model: &mut Model,
    body: BodyId,
    instance: InstanceId,
    undo: &mut UndoStack,
    label: &str,
) -> Result<Vec<Entity>, ExplodeError> {
    if model.body(body).is_none() {
        return Err(ExplodeError::Model(ModelError::MissingBody(body)));
    }
    let Some(data) = model
        .body(body)
        .and_then(|target| target.instance(instance))
    else {
        return Err(ExplodeError::MissingInstance(instance));
    };
    if bad_transform(&data.transform) {
        return Err(ExplodeError::BadTransform);
    }

    let before = model.validate();
    let mut edit = ModelEdit::new();
    match explode_inner(model, body, instance, &mut edit) {
        Ok(entities) => {
            let added: Vec<ModelViolation> = model
                .validate()
                .into_iter()
                .filter(|violation| !before.contains(violation))
                .collect();
            if !added.is_empty() {
                rollback(model, &edit);
                return Err(ExplodeError::Invalid(added));
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
