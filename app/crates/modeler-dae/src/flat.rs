// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! World-space draw batches for the renderer (meters, Z up, right-handed).

use crate::{Material, Model};

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
}

/// Expand every instance into world space.
pub fn flatten(model: &Model) -> FlatScene {
    let _ = model;
    unimplemented!("m0-dae-import")
}
