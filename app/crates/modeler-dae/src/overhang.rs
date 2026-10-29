// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Overhang tint: triangles that face down more steeply than a printer can bridge, moved
//! into one batch drawn in [`OVERHANG_COLOR`].
//!
//! Behaviour: `docs/specs/m7-overhang.md`.

use glam::DVec3;

use crate::{Batch, FlatScene, Material};

/// The overhang angle the app starts with, in degrees from vertical.
pub const DEFAULT_OVERHANG_DEGREES: f64 = 45.0;

/// Color of the material [`tint_overhangs`] adds.
pub const OVERHANG_COLOR: [f32; 4] = [0.9, 0.2, 0.15, 1.0];

/// Whether a surface with front normal `normal` (any length) leans past `max_degrees` from
/// vertical while facing down.
pub fn is_overhang(normal: DVec3, max_degrees: f64) -> bool {
    if !normal.is_finite() || normal.length_squared() == 0.0 {
        return false;
    }
    let max_degrees = if max_degrees.is_nan() {
        90.0
    } else {
        max_degrees.clamp(0.0, 90.0)
    };
    let n = normal.normalize();
    -n.z > max_degrees.to_radians().sin() + 1e-9
}

fn position(value: &[f32; 3]) -> DVec3 {
    DVec3::new(
        f64::from(value[0]),
        f64::from(value[1]),
        f64::from(value[2]),
    )
}

/// A copy of `scene` with every overhanging triangle that isn't on the bed moved into one
/// batch drawn in [`OVERHANG_COLOR`].
pub fn tint_overhangs(scene: &FlatScene, max_degrees: f64) -> FlatScene {
    let bed = scene.bounds.map(|bounds| f64::from(bounds.min[2]));

    let mut tinted = Vec::with_capacity(scene.batches.len());
    let mut any = false;
    for batch in &scene.batches {
        let mut flags = Vec::with_capacity(batch.indices.len() / 3);
        for triangle in batch.indices.as_chunks::<3>().0 {
            let (Some(a), Some(b), Some(c)) = (
                batch.positions.get(triangle[0] as usize),
                batch.positions.get(triangle[1] as usize),
                batch.positions.get(triangle[2] as usize),
            ) else {
                flags.push(false);
                continue;
            };
            let (a, b, c) = (position(a), position(b), position(c));
            let normal = (b - a).cross(c - a);
            let on_bed = bed.is_some_and(|bed| {
                let top = bed + 1e-4;
                a.z <= top && b.z <= top && c.z <= top
            });
            let flag = is_overhang(normal, max_degrees) && !on_bed;
            any |= flag;
            flags.push(flag);
        }
        tinted.push(flags);
    }

    if !any {
        return scene.clone();
    }

    let mut materials = scene.materials.clone();
    materials.push(Material {
        id: "overhang".to_owned(),
        name: "Overhang".to_owned(),
        color: OVERHANG_COLOR,
        texture: None,
        opacity: 1.0,
        unlit: false,
    });
    let material = Some(scene.materials.len());

    let mut batches = Vec::with_capacity(scene.batches.len() + 1);
    let mut overhangs = Batch {
        material,
        ..Batch::default()
    };
    for (batch, flags) in scene.batches.iter().zip(tinted) {
        let mut kept = Batch {
            material: batch.material,
            positions: batch.positions.clone(),
            normals: batch.normals.clone(),
            texcoords: batch.texcoords.clone(),
            indices: Vec::new(),
        };
        for (triangle, tinted) in batch.indices.as_chunks::<3>().0.iter().zip(flags) {
            if tinted {
                for &index in triangle {
                    let index = index as usize;
                    let first = overhangs.positions.len() as u32;
                    overhangs.positions.push(batch.positions[index]);
                    overhangs
                        .normals
                        .push(batch.normals.get(index).copied().unwrap_or([0.0; 3]));
                    overhangs.indices.push(first);
                }
            } else {
                kept.indices.extend_from_slice(triangle);
            }
        }
        if !kept.indices.is_empty() {
            batches.push(kept);
        }
    }
    batches.push(overhangs);

    FlatScene {
        materials,
        batches,
        lines: scene.lines.clone(),
        bounds: scene.bounds,
    }
}
