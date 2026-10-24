// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! World-space draw batches for the renderer (meters, Z up, right-handed).

use std::collections::BTreeMap;

use glam::{DMat4, DVec3};

use crate::{GeometryInstance, Material, Model, Node, Primitive, PrimitiveKind, UpAxis};

/// Axis-aligned bounding box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// Triangles that share one material, in world space.
#[derive(Clone, Debug, Default)]
pub struct Batch {
    /// Index into [`FlatScene::materials`]; `None` draws with the default material.
    pub material: Option<usize>,
    pub positions: Vec<[f32; 3]>,
    /// Unit length, same length as `positions`. Generated from the triangles when the
    /// file has none.
    pub normals: Vec<[f32; 3]>,
    /// Same length as `positions`, or empty when the material has no texture.
    pub texcoords: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// Line segments that share one material, in world space.
#[derive(Clone, Debug, Default)]
pub struct LineBatch {
    pub material: Option<usize>,
    /// Segment end points, two per segment.
    pub positions: Vec<[f32; 3]>,
}

/// Everything the renderer needs, with every instance expanded.
#[derive(Clone, Debug, Default)]
pub struct FlatScene {
    pub materials: Vec<Material>,
    pub batches: Vec<Batch>,
    pub lines: Vec<LineBatch>,
    /// Bounds of every triangle and line vertex; `None` for an empty scene.
    pub bounds: Option<Aabb>,
}

impl FlatScene {
    pub fn triangle_count(&self) -> usize {
        self.batches.iter().map(|b| b.indices.len() / 3).sum()
    }

    pub fn line_count(&self) -> usize {
        self.lines.iter().map(|l| l.positions.len() / 2).sum()
    }

    /// The nearest point where the ray from `origin` along `dir` hits a triangle of the
    /// scene (either side), in front of `origin`. Lines don't count. Behaviour:
    /// `docs/specs/m4-orbit-pivot.md`.
    pub fn raycast(&self, origin: DVec3, dir: DVec3) -> Option<DVec3> {
        if !origin.is_finite() || !dir.is_finite() || dir.length_squared() == 0.0 {
            return None;
        }
        let mut nearest: Option<f64> = None;
        for batch in &self.batches {
            let point = |index: u32| {
                batch
                    .positions
                    .get(index as usize)
                    .map(|p| DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2])))
            };
            for triangle in batch.indices.as_chunks::<3>().0 {
                let (Some(a), Some(b), Some(c)) =
                    (point(triangle[0]), point(triangle[1]), point(triangle[2]))
                else {
                    continue;
                };
                let edge1 = b - a;
                let edge2 = c - a;
                let h = dir.cross(edge2);
                let det = edge1.dot(h);
                if det == 0.0 || !det.is_finite() {
                    continue;
                }
                let inv_det = 1.0 / det;
                let s = origin - a;
                let u = inv_det * s.dot(h);
                if u < 0.0 || u > 1.0 {
                    continue;
                }
                let q = s.cross(edge1);
                let v = inv_det * dir.dot(q);
                if v < 0.0 || u + v > 1.0 {
                    continue;
                }
                let t = inv_det * edge2.dot(q);
                if t > 0.0 && nearest.is_none_or(|nearest| t < nearest) {
                    nearest = Some(t);
                }
            }
        }
        nearest.map(|t| origin + dir * t)
    }

    /// Every place the ray from `origin` along `dir` crosses a triangle of the scene (either
    /// side) in front of `origin`, as ray parameters `t > 0` (the point is `origin + dir *
    /// t`), sorted ascending. Lines don't count. Behaviour: `docs/specs/m4-zoom-through.md`.
    pub fn ray_hits(&self, origin: DVec3, dir: DVec3) -> Vec<f64> {
        let _ = (origin, dir);
        unimplemented!("m4-zoom-through")
    }
}

/// Expand every instance into world space.
pub fn flatten(model: &Model) -> FlatScene {
    let frame = frame_matrix(model.up_axis);
    let scale = DMat4::from_scale(DVec3::splat(model.unit_meters));

    let mut expander = Expander {
        model,
        frame,
        scale,
        triangles: BTreeMap::new(),
        loose_triangles: Batch::default(),
        lines: BTreeMap::new(),
        loose_lines: LineBatch::default(),
    };
    expander.expand_node(&model.scene, DMat4::IDENTITY);

    let mut batches: Vec<Batch> = expander
        .triangles
        .into_values()
        .filter(|batch| !batch.positions.is_empty())
        .collect();
    if !expander.loose_triangles.positions.is_empty() {
        batches.push(expander.loose_triangles);
    }

    let mut lines: Vec<LineBatch> = expander
        .lines
        .into_values()
        .filter(|batch| !batch.positions.is_empty())
        .collect();
    if !expander.loose_lines.positions.is_empty() {
        lines.push(expander.loose_lines);
    }

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
        materials: model.materials.clone(),
        batches,
        lines,
        bounds: any.then_some(Aabb { min, max }),
    }
}

struct Expander<'a> {
    model: &'a Model,
    frame: DMat4,
    scale: DMat4,
    triangles: BTreeMap<usize, Batch>,
    loose_triangles: Batch,
    lines: BTreeMap<usize, LineBatch>,
    loose_lines: LineBatch,
}

impl Expander<'_> {
    fn expand_node(&mut self, node: &Node, parent: DMat4) {
        let world = parent * node.transform;
        for instance in &node.geometry {
            self.expand_geometry(instance, world);
        }
        for child in &node.children {
            self.expand_node(child, world);
        }
        for &definition in &node.instances {
            self.expand_node(&self.model.definitions[definition].node, world);
        }
    }

    fn expand_geometry(&mut self, instance: &GeometryInstance, world: DMat4) {
        let mesh = &self.model.meshes[instance.mesh];
        let transform = self.scale * self.frame * world;
        let mirror = transform.determinant() < 0.0;
        let normal_matrix = transform
            .try_inverse()
            .map(|inverse| inverse.transpose())
            .unwrap_or(DMat4::IDENTITY);

        for primitive in &mesh.primitives {
            let material = primitive
                .material_symbol
                .as_deref()
                .and_then(|symbol| instance.material_for(symbol));
            let has_texture = material
                .and_then(|index| self.model.materials.get(index))
                .is_some_and(|material| material.texture.is_some());

            match primitive.kind {
                PrimitiveKind::Triangles => {
                    self.expand_triangles(
                        primitive,
                        material,
                        has_texture,
                        transform,
                        normal_matrix,
                        mirror,
                    );
                }
                PrimitiveKind::Lines => {
                    self.expand_lines(primitive, material, transform);
                }
            }
        }
    }

    fn expand_triangles(
        &mut self,
        primitive: &Primitive,
        material: Option<usize>,
        has_texture: bool,
        transform: DMat4,
        normal_matrix: DMat4,
        mirror: bool,
    ) {
        let order = if mirror {
            [0usize, 2, 1]
        } else {
            [0usize, 1, 2]
        };
        let base = match material {
            Some(index) => self.triangles.entry(index).or_insert_with(|| Batch {
                material: Some(index),
                ..Batch::default()
            }),
            None => &mut self.loose_triangles,
        };

        for triangle in primitive.indices.chunks(3) {
            if triangle.len() != 3 {
                continue;
            }
            let source = [
                triangle[0] as usize,
                triangle[1] as usize,
                triangle[2] as usize,
            ];
            if source
                .iter()
                .any(|&index| index >= primitive.positions.len())
            {
                continue;
            }
            let local = source.map(|index| dvec(primitive.positions[index]));
            let points = local.map(|point| transform.transform_point3(point));
            let face = face_normal(points[order[0]], points[order[1]], points[order[2]]);
            let first = base.positions.len() as u32;

            for &corner in &order {
                let index = source[corner];
                let normal = if primitive.normals.is_empty() {
                    face
                } else {
                    let raw = primitive
                        .normals
                        .get(index)
                        .copied()
                        .map(dvec)
                        .unwrap_or(DVec3::ZERO);
                    normalize_or(normal_matrix.transform_vector3(raw), face)
                };
                base.positions.push(points[corner].as_vec3().to_array());
                base.normals.push(normal.as_vec3().to_array());
                if has_texture {
                    let uv = primitive
                        .texcoords
                        .get(index)
                        .copied()
                        .unwrap_or([0.0, 0.0]);
                    base.texcoords.push(uv);
                }
            }

            base.indices
                .extend_from_slice(&[first, first + 1, first + 2]);
        }
    }

    fn expand_lines(&mut self, primitive: &Primitive, material: Option<usize>, transform: DMat4) {
        let base = match material {
            Some(index) => self.lines.entry(index).or_insert_with(|| LineBatch {
                material: Some(index),
                ..LineBatch::default()
            }),
            None => &mut self.loose_lines,
        };
        for line in primitive.indices.chunks(2) {
            if line.len() != 2 {
                continue;
            }
            for &index in line {
                let Some(position) = primitive.positions.get(index as usize) else {
                    continue;
                };
                let point = transform.transform_point3(dvec(*position));
                base.positions.push(point.as_vec3().to_array());
            }
        }
    }
}

fn dvec(value: [f32; 3]) -> DVec3 {
    DVec3::new(value[0] as f64, value[1] as f64, value[2] as f64)
}

pub(crate) fn frame_matrix(up: UpAxis) -> DMat4 {
    match up {
        UpAxis::Z => DMat4::IDENTITY,
        UpAxis::Y => DMat4::from_cols_array(&[
            1.0, 0.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, -1.0, 0.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ]),
        UpAxis::X => DMat4::from_cols_array(&[
            0.0, 0.0, 1.0, 0.0, //
            -1.0, 0.0, 0.0, 0.0, //
            0.0, -1.0, 0.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ]),
    }
}

fn normalize_or(value: DVec3, fallback: DVec3) -> DVec3 {
    if value.length_squared() > 1e-24 {
        value.normalize()
    } else {
        fallback
    }
}

fn face_normal(a: DVec3, b: DVec3, c: DVec3) -> DVec3 {
    normalize_or((b - a).cross(c - a), DVec3::Z)
}
