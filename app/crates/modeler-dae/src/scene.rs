// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Kernel model to world-space draw batches, so kernel models render like imports.
//!
//! Behaviour: `docs/specs/m1-kernel-scene.md`.

use std::collections::BTreeMap;

use glam::{DMat3, DMat4, DVec2, DVec3};
use modeler_kernel::geom::{polygon_normal, to_plane_2d};
use modeler_kernel::model as km;
use modeler_kernel::tess::triangulate;
use modeler_kernel::texture::TextureMap;

use crate::{Aabb, Batch, FlatScene, LineBatch, Material};

/// How deep instances may nest before the walk stops.
const MAX_DEPTH: usize = 64;

/// Expand every instance of `model` into triangles (faces) and line segments (edges) in
/// world space, ready for the renderer.
pub fn kernel_scene(model: &km::Model) -> FlatScene {
    let mut material_index = BTreeMap::new();
    let mut materials = Vec::new();
    let mut texture_sizes = Vec::new();
    for (index, (id, material)) in model.materials().enumerate() {
        material_index.insert(id, index);
        materials.push(Material {
            id: id.0.to_string(),
            name: material.name.clone(),
            color: [material.color[0], material.color[1], material.color[2], 1.0],
            texture: material
                .texture
                .as_ref()
                .map(|texture| std::path::PathBuf::from(&texture.path)),
            opacity: material.color[3],
            unlit: false,
        });
        texture_sizes.push(material.texture.as_ref().map(|texture| texture.size));
    }

    let mut draw = Draw {
        model,
        material_index,
        texture_sizes,
        batches: BTreeMap::new(),
        default_batch: Batch::default(),
        line: LineBatch::default(),
    };
    draw.walk(km::BodyId::Root, DMat4::IDENTITY, None, 0);

    let mut batches: Vec<Batch> = draw
        .batches
        .into_values()
        .filter(|batch| !batch.positions.is_empty())
        .collect();
    if !draw.default_batch.positions.is_empty() {
        batches.push(draw.default_batch);
    }
    let lines = if draw.line.positions.is_empty() {
        Vec::new()
    } else {
        vec![draw.line]
    };

    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut any = false;
    for position in batches
        .iter()
        .flat_map(|batch| batch.positions.iter())
        .chain(lines.iter().flat_map(|line| line.positions.iter()))
    {
        any = true;
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }

    FlatScene {
        materials,
        batches,
        lines,
        bounds: any.then_some(Aabb { min, max }),
    }
}

struct FaceDraw {
    outer: Vec<DVec3>,
    holes: Vec<Vec<DVec3>>,
    front: Option<usize>,
    back: Option<usize>,
    front_map: Option<(TextureMap, DVec2)>,
    back_map: Option<(TextureMap, DVec2)>,
}

struct Draw<'a> {
    model: &'a km::Model,
    material_index: BTreeMap<km::MaterialId, usize>,
    texture_sizes: Vec<Option<DVec2>>,
    batches: BTreeMap<usize, Batch>,
    default_batch: Batch,
    line: LineBatch,
}

impl Draw<'_> {
    fn material(&self, id: Option<km::MaterialId>) -> Option<usize> {
        id.and_then(|id| self.material_index.get(&id).copied())
    }

    fn walk(&mut self, body_id: km::BodyId, world: DMat4, inherited: Option<usize>, depth: usize) {
        if depth >= MAX_DEPTH {
            return;
        }

        let (faces, edges, instances) = {
            let Some(body) = self.model.body(body_id) else {
                return;
            };
            let geometry = body.geometry();

            let faces: Vec<FaceDraw> = geometry
                .faces()
                .map(|(id, face)| {
                    let materials = body.face_materials(id);
                    let outer = geometry.loop_points(&face.outer);
                    let normal = polygon_normal(&outer);
                    let front = self.material(materials.front).or(inherited);
                    let back = self.material(materials.back).or(inherited);
                    let side =
                        |material: Option<usize>, stored: Option<TextureMap>, normal: DVec3| {
                            let size = material.and_then(|index| self.texture_sizes[index])?;
                            let map = stored.unwrap_or_else(|| TextureMap::default_for(normal));
                            Some((map, size))
                        };
                    FaceDraw {
                        outer,
                        holes: face
                            .inner
                            .iter()
                            .map(|hole| geometry.loop_points(hole))
                            .collect(),
                        front,
                        back,
                        front_map: side(front, materials.front_map, normal),
                        back_map: side(back, materials.back_map, -normal),
                    }
                })
                .collect();

            let mut edges = Vec::new();
            for (_, edge) in geometry.edges() {
                let (Some(a), Some(b)) =
                    (geometry.vertex(edge.ends[0]), geometry.vertex(edge.ends[1]))
                else {
                    continue;
                };
                edges.push((a.position, b.position));
            }

            let instances: Vec<(km::DefinitionId, DMat4, Option<usize>)> = body
                .instances()
                .map(|(_, instance)| {
                    (
                        instance.definition,
                        instance.transform,
                        self.material(instance.material).or(inherited),
                    )
                })
                .collect();

            (faces, edges, instances)
        };

        for face in faces {
            draw_face(
                &mut self.batches,
                &mut self.default_batch,
                world,
                &face.outer,
                &face.holes,
                face.front,
                face.back,
                face.front_map,
                face.back_map,
            );
        }

        for (a, b) in edges {
            let a = world.transform_point3(a);
            let b = world.transform_point3(b);
            if !a.is_finite() || !b.is_finite() {
                continue;
            }
            self.line.positions.push(as_f32(a));
            self.line.positions.push(as_f32(b));
        }

        for (definition, transform, material) in instances {
            if self
                .model
                .body(km::BodyId::Definition(definition))
                .is_none()
            {
                continue;
            }
            self.walk(
                km::BodyId::Definition(definition),
                world * transform,
                material,
                depth + 1,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_face(
    batches: &mut BTreeMap<usize, Batch>,
    default_batch: &mut Batch,
    world: DMat4,
    outer: &[DVec3],
    holes: &[Vec<DVec3>],
    front: Option<usize>,
    back: Option<usize>,
    front_map: Option<(TextureMap, DVec2)>,
    back_map: Option<(TextureMap, DVec2)>,
) {
    let normal = polygon_normal(outer);
    if normal == DVec3::ZERO {
        return;
    }

    let world_outer: Vec<DVec3> = outer
        .iter()
        .map(|&point| world.transform_point3(point))
        .collect();
    let world_holes: Vec<Vec<DVec3>> = holes
        .iter()
        .map(|hole| {
            hole.iter()
                .map(|&point| world.transform_point3(point))
                .collect()
        })
        .collect();
    if world_outer
        .iter()
        .chain(world_holes.iter().flatten())
        .any(|point| !point.is_finite())
    {
        return;
    }

    let outer_2d = to_plane_2d(outer, normal);
    let holes_2d: Vec<Vec<DVec2>> = holes.iter().map(|hole| to_plane_2d(hole, normal)).collect();
    let triangles = triangulate(&outer_2d, &holes_2d);
    if triangles.is_empty() {
        return;
    }

    let positions: Vec<[f32; 3]> = world_outer
        .iter()
        .chain(world_holes.iter().flatten())
        .map(|&point| as_f32(point))
        .collect();

    let matrix = DMat3::from_mat4(world);
    let normal = world_normal(matrix, normal);
    let mirror = matrix.determinant() < 0.0;
    let front_triangles: Vec<[usize; 3]> = triangles
        .iter()
        .map(|&triangle| {
            if mirror {
                [triangle[0], triangle[2], triangle[1]]
            } else {
                triangle
            }
        })
        .collect();

    let body_positions: Vec<DVec3> = outer
        .iter()
        .chain(holes.iter().flatten())
        .copied()
        .collect();
    let texcoords = |side: Option<(TextureMap, DVec2)>| {
        side.map(|(map, size)| {
            body_positions
                .iter()
                .map(|&point| {
                    let uv = map.apply(point) / size;
                    [uv.x as f32, uv.y as f32]
                })
                .collect::<Vec<[f32; 2]>>()
        })
    };
    let front_texcoords = texcoords(front_map);

    append(
        batch(batches, default_batch, front),
        &positions,
        normal,
        &front_triangles,
        front_texcoords.as_deref(),
    );

    if let Some(back) = back {
        let back_triangles: Vec<[usize; 3]> =
            front_triangles.iter().map(|&[a, b, c]| [a, c, b]).collect();
        let back_texcoords = texcoords(back_map);
        append(
            batch(batches, default_batch, Some(back)),
            &positions,
            -normal,
            &back_triangles,
            back_texcoords.as_deref(),
        );
    }
}

fn batch<'a>(
    batches: &'a mut BTreeMap<usize, Batch>,
    default_batch: &'a mut Batch,
    material: Option<usize>,
) -> &'a mut Batch {
    match material {
        Some(index) => batches.entry(index).or_insert_with(|| Batch {
            material: Some(index),
            ..Batch::default()
        }),
        None => default_batch,
    }
}

fn append(
    batch: &mut Batch,
    positions: &[[f32; 3]],
    normal: DVec3,
    triangles: &[[usize; 3]],
    texcoords: Option<&[[f32; 2]]>,
) {
    let base = batch.positions.len() as u32;
    let normal = as_f32(normal);
    for (index, &position) in positions.iter().enumerate() {
        batch.positions.push(position);
        batch.normals.push(normal);
        if let Some(texcoords) = texcoords {
            batch.texcoords.push(texcoords[index]);
        }
    }
    for &[a, b, c] in triangles {
        batch.indices.push(base + a as u32);
        batch.indices.push(base + b as u32);
        batch.indices.push(base + c as u32);
    }
}

fn world_normal(matrix: DMat3, normal: DVec3) -> DVec3 {
    let inverse = matrix.try_inverse().map(|inverse| inverse.transpose());
    let transformed = inverse.map(|inverse| inverse * normal).unwrap_or(normal);
    if transformed.is_finite() && transformed.length_squared() > 1e-24 {
        transformed.normalize()
    } else {
        normal
    }
}

fn as_f32(point: DVec3) -> [f32; 3] {
    [point.x as f32, point.y as f32, point.z as f32]
}
