// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Problem marks for 3D printing: the edges and faces of one group or component that keep
//! it from being a solid, in world space, and a scene copy with them drawn in
//! [`PROBLEM_COLOR`].
//!
//! Behaviour: `docs/specs/m7-solid-ui.md`.

use std::collections::HashMap;

use glam::{DMat4, DVec3};
use modeler_kernel::geom::{polygon_normal, to_plane_2d};
use modeler_kernel::model as km;
use modeler_kernel::tess::triangulate;

use crate::{Batch, FlatScene, LineBatch, Material, Triangle};

/// Color of the material [`tint_problems`] adds, for faces and edges alike.
pub const PROBLEM_COLOR: [f32; 4] = [0.85, 0.1, 0.6, 1.0];

/// How close a scene triangle's corners or a scene line's ends must be to a mark's for
/// [`tint_problems`] to take it, in meters.
pub const MARK_TOLERANCE_M: f64 = 1e-5;

/// What is wrong with one group or component, in world space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolidMarks {
    /// Every triangle of the reversed and internal faces.
    pub triangles: Vec<Triangle>,
    /// The open, loose and shared edges.
    pub segments: Vec<[DVec3; 2]>,
}

impl SolidMarks {
    /// No triangles and no segments.
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty() && self.segments.is_empty()
    }
}

/// The marks of `instance` (an instance in `body`): `check_geometry` of its definition's own
/// geometry (groups and components nested in it are not looked into), the triangles of its
/// `reversed_faces` and `internal_faces` and the end points of its `open_edges`,
/// `loose_edges` and `shared_edges`, placed by the instance's transform and then by `outer`.
/// The triangles are the ones `kernel_scene` draws for those faces. Empty when the instance
/// or its definition doesn't exist or nothing is wrong.
pub fn solid_marks(
    model: &km::Model,
    body: km::BodyId,
    instance: km::InstanceId,
    outer: DMat4,
) -> SolidMarks {
    let Some(instance) = model.body(body).and_then(|body| body.instance(instance)) else {
        return SolidMarks::default();
    };
    let Some(definition_body) = model.body(km::BodyId::Definition(instance.definition)) else {
        return SolidMarks::default();
    };
    let ctx = definition_body.geometry();
    let report = modeler_kernel::solid::check_geometry(ctx);
    let world = outer * instance.transform;

    let mut marks = SolidMarks::default();
    let faces: std::collections::BTreeSet<_> = report
        .reversed_faces
        .iter()
        .chain(report.internal_faces.iter())
        .copied()
        .collect();
    for face_id in faces {
        let Some(face) = ctx.face(face_id) else {
            continue;
        };
        let outer_points = ctx.loop_points(&face.outer);
        let normal = polygon_normal(&outer_points);
        if normal == DVec3::ZERO {
            continue;
        }
        let holes: Vec<Vec<DVec3>> = face.inner.iter().map(|l| ctx.loop_points(l)).collect();
        let outer_2d = to_plane_2d(&outer_points, normal);
        let holes_2d: Vec<Vec<glam::DVec2>> =
            holes.iter().map(|hole| to_plane_2d(hole, normal)).collect();
        let points: Vec<DVec3> = outer_points
            .iter()
            .chain(holes.iter().flatten())
            .copied()
            .collect();
        for [a, b, c] in triangulate(&outer_2d, &holes_2d) {
            let (Some(a), Some(b), Some(c)) = (points.get(a), points.get(b), points.get(c)) else {
                continue;
            };
            marks.triangles.push([
                world.transform_point3(*a),
                world.transform_point3(*b),
                world.transform_point3(*c),
            ]);
        }
    }

    for edge_id in report
        .open_edges
        .iter()
        .chain(report.loose_edges.iter())
        .chain(report.shared_edges.iter())
    {
        let Some(edge) = ctx.edge(*edge_id) else {
            continue;
        };
        let (Some(a), Some(b)) = (ctx.vertex(edge.ends[0]), ctx.vertex(edge.ends[1])) else {
            continue;
        };
        marks.segments.push([
            world.transform_point3(a.position),
            world.transform_point3(b.position),
        ]);
    }
    marks
}

type Cell = (i64, i64, i64);

fn cell(point: DVec3) -> Cell {
    (
        (point.x / MARK_TOLERANCE_M).floor() as i64,
        (point.y / MARK_TOLERANCE_M).floor() as i64,
        (point.z / MARK_TOLERANCE_M).floor() as i64,
    )
}

fn close(a: DVec3, b: DVec3) -> bool {
    (a.x - b.x).abs() <= MARK_TOLERANCE_M
        && (a.y - b.y).abs() <= MARK_TOLERANCE_M
        && (a.z - b.z).abs() <= MARK_TOLERANCE_M
}

const PERMUTATIONS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

fn triangle_matches(scene: &[DVec3; 3], mark: &Triangle) -> bool {
    PERMUTATIONS
        .iter()
        .any(|perm| (0..3).all(|i| close(scene[i], mark[perm[i]])))
}

fn segment_matches(scene: &[DVec3; 2], mark: &[DVec3; 2]) -> bool {
    (close(scene[0], mark[0]) && close(scene[1], mark[1]))
        || (close(scene[0], mark[1]) && close(scene[1], mark[0]))
}

fn scene_point(position: &[f32; 3]) -> DVec3 {
    DVec3::new(
        f64::from(position[0]),
        f64::from(position[1]),
        f64::from(position[2]),
    )
}

fn neighbour_cells(point: DVec3) -> impl Iterator<Item = Cell> {
    let (x, y, z) = cell(point);
    (-1..=1).flat_map(move |dx| {
        (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| (x + dx, y + dy, z + dz)))
    })
}

/// A copy of `scene` with every triangle that matches a mark triangle (the same three corners
/// in any order, each within [`MARK_TOLERANCE_M`]) moved into one batch, and every line
/// segment that matches a mark segment (the same two ends in either order) moved out of its
/// line batch into one new line batch, both drawn in a material named "Problem" in
/// [`PROBLEM_COLOR`] that is added after the scene's own. When nothing matches (or `marks`
/// is empty) the copy is the scene unchanged, with no material added. Must stay fast for
/// scenes of 100 000 triangles and as many marks.
pub fn tint_problems(scene: &FlatScene, marks: &SolidMarks) -> FlatScene {
    if marks.is_empty() {
        return scene.clone();
    }

    let mut triangle_grid: HashMap<Cell, Vec<usize>> = HashMap::new();
    for (index, triangle) in marks.triangles.iter().enumerate() {
        for corner in triangle {
            triangle_grid.entry(cell(*corner)).or_default().push(index);
        }
    }
    let mut segment_grid: HashMap<Cell, Vec<usize>> = HashMap::new();
    for (index, segment) in marks.segments.iter().enumerate() {
        for end in segment {
            segment_grid.entry(cell(*end)).or_default().push(index);
        }
    }

    let mut triangle_flags: Vec<Vec<bool>> = Vec::with_capacity(scene.batches.len());
    let mut any_triangle = false;
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
            let corners = [scene_point(a), scene_point(b), scene_point(c)];
            let mut flag = false;
            'candidates: for cell in neighbour_cells(corners[0]) {
                if let Some(indices) = triangle_grid.get(&cell) {
                    for &index in indices {
                        if triangle_matches(&corners, &marks.triangles[index]) {
                            flag = true;
                            break 'candidates;
                        }
                    }
                }
            }
            any_triangle |= flag;
            flags.push(flag);
        }
        triangle_flags.push(flags);
    }

    let mut segment_flags: Vec<Vec<bool>> = Vec::with_capacity(scene.lines.len());
    let mut any_segment = false;
    for line in &scene.lines {
        let mut flags = Vec::with_capacity(line.positions.len() / 2);
        for ends in line.positions.chunks_exact(2) {
            let ends = [scene_point(&ends[0]), scene_point(&ends[1])];
            let mut flag = false;
            'candidates: for cell in neighbour_cells(ends[0]) {
                if let Some(indices) = segment_grid.get(&cell) {
                    for &index in indices {
                        if segment_matches(&ends, &marks.segments[index]) {
                            flag = true;
                            break 'candidates;
                        }
                    }
                }
            }
            any_segment |= flag;
            flags.push(flag);
        }
        segment_flags.push(flags);
    }

    if !any_triangle && !any_segment {
        return scene.clone();
    }

    let mut materials = scene.materials.clone();
    materials.push(Material {
        id: "problem".to_owned(),
        name: "Problem".to_owned(),
        color: PROBLEM_COLOR,
        texture: None,
        opacity: 1.0,
        unlit: false,
    });
    let material = Some(scene.materials.len());

    let mut batches = Vec::with_capacity(scene.batches.len() + 1);
    let mut problem_batch = Batch {
        material,
        ..Batch::default()
    };
    for (batch, flags) in scene.batches.iter().zip(triangle_flags) {
        let mut kept = Batch {
            material: batch.material,
            positions: batch.positions.clone(),
            normals: batch.normals.clone(),
            texcoords: batch.texcoords.clone(),
            indices: Vec::new(),
        };
        for (triangle, marked) in batch.indices.as_chunks::<3>().0.iter().zip(flags) {
            if marked {
                for &index in triangle {
                    let index = index as usize;
                    let first = problem_batch.positions.len() as u32;
                    problem_batch.positions.push(batch.positions[index]);
                    problem_batch
                        .normals
                        .push(batch.normals.get(index).copied().unwrap_or([0.0; 3]));
                    problem_batch.indices.push(first);
                }
            } else {
                kept.indices.extend_from_slice(triangle);
            }
        }
        if !kept.indices.is_empty() {
            batches.push(kept);
        }
    }
    if !problem_batch.indices.is_empty() {
        batches.push(problem_batch);
    }

    let mut lines = Vec::with_capacity(scene.lines.len() + 1);
    let mut problem_line = LineBatch {
        material,
        ..LineBatch::default()
    };
    for (line, flags) in scene.lines.iter().zip(segment_flags) {
        let mut kept = LineBatch {
            material: line.material,
            positions: Vec::new(),
        };
        for (segment, marked) in line.positions.chunks_exact(2).zip(flags) {
            if marked {
                problem_line.positions.extend_from_slice(segment);
            } else {
                kept.positions.extend_from_slice(segment);
            }
        }
        let used = line.positions.len() - line.positions.len() % 2;
        kept.positions.extend_from_slice(&line.positions[used..]);
        if !kept.positions.is_empty() {
            lines.push(kept);
        }
    }
    if !problem_line.positions.is_empty() {
        lines.push(problem_line);
    }

    FlatScene {
        materials,
        batches,
        lines,
        bounds: scene.bounds,
    }
}
