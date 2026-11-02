// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Print mesh: the solid groups and components of a model as world-space triangles in
//! millimeters, ready for the STL and 3MF writers.
//!
//! Behaviour: `docs/specs/m7-print-mesh.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;

use drydock_kernel::curve::MIN_CHORD_TOL;
use drydock_kernel::geom;
use drydock_kernel::model::{BodyId, DefinitionId, Instance, InstanceId, MaterialId, Model};
use drydock_kernel::solid::{self, SolidReport};
use drydock_kernel::tess;
use drydock_kernel::topology::{Context, EdgeId, Face, Loop, VertexId};
use glam::{DMat4, DVec2, DVec3};

/// One triangle, corners counter-clockwise seen from outside the solid, in millimeters.
pub type Triangle = [DVec3; 3];

/// Default chord tolerance in millimeters.
pub const DEFAULT_CHORD_TOLERANCE_MM: f64 = 0.01;

/// What to export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintScope<'a> {
    /// Every group and component placed in the model root.
    Model,
    /// Only these instances of the model root (with everything nested in them). Ids that
    /// aren't instances of the root are ignored.
    Instances(&'a [InstanceId]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrintOptions {
    /// How far a re-segmented arc or circle may stray from the true curve, in millimeters.
    pub chord_tolerance_mm: f64,
}

impl Default for PrintOptions {
    fn default() -> Self {
        PrintOptions {
            chord_tolerance_mm: DEFAULT_CHORD_TOLERANCE_MM,
        }
    }
}

/// One exported solid.
#[derive(Clone, Debug, PartialEq)]
pub struct PrintObject {
    pub name: String,
    /// sRGB, straight alpha; `None` when nothing in the object is painted.
    pub color: Option<[f32; 4]>,
    pub triangles: Vec<Triangle>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SkipReason {
    /// No faces and nothing nested in it.
    NoGeometry,
    /// The solid check found problems; the report says which.
    NotSolid(SolidReport),
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkipReason::NoGeometry => f.write_str("has no faces"),
            SkipReason::NotSolid(report) => {
                let mut parts = Vec::new();
                let mut count = |n: usize, what: &str| {
                    if n > 0 {
                        parts.push(format!("{n} {what}{}", if n == 1 { "" } else { "s" }));
                    }
                };
                count(report.open_edges.len(), "open edge");
                count(report.loose_edges.len(), "loose edge");
                count(report.shared_edges.len(), "shared edge");
                count(report.internal_faces.len(), "internal face");
                if parts.is_empty() {
                    parts.push("no closed surface".to_string());
                }
                write!(f, "is not a solid: {}", parts.join(", "))
            }
        }
    }
}

/// A group or component that was left out, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Skipped {
    pub name: String,
    pub reason: SkipReason,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrintMesh {
    pub objects: Vec<PrintObject>,
    pub skipped: Vec<Skipped>,
}

/// The print mesh of `model` for `scope`.
pub fn print_mesh(model: &Model, scope: PrintScope, options: &PrintOptions) -> PrintMesh {
    let mut out = PrintMesh::default();
    let mut reports: BTreeMap<DefinitionId, SolidReport> = BTreeMap::new();
    for (id, instance) in model.root().instances() {
        if let PrintScope::Instances(ids) = scope
            && !ids.contains(&id)
        {
            continue;
        }
        let name = object_name(model, instance);
        walk(
            model,
            &mut out,
            &mut reports,
            instance,
            instance.transform,
            name,
            None,
            0,
            options,
        );
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn walk(
    model: &Model,
    out: &mut PrintMesh,
    reports: &mut BTreeMap<DefinitionId, SolidReport>,
    instance: &Instance,
    world: DMat4,
    name: String,
    inherited: Option<MaterialId>,
    depth: usize,
    options: &PrintOptions,
) {
    let definition = instance.definition;
    let Some(body) = model.body(BodyId::Definition(definition)) else {
        return;
    };
    let material = instance.material.or(inherited);
    let has_faces = body.geometry().face_count() > 0;
    let has_nested = body.instances().next().is_some();

    if !has_faces {
        if !has_nested {
            out.skipped.push(Skipped {
                name: name.clone(),
                reason: SkipReason::NoGeometry,
            });
        }
    } else {
        let report = match reports.get(&definition) {
            Some(report) => report.clone(),
            None => {
                let report = solid::check_geometry(body.geometry());
                reports.insert(definition, report.clone());
                report
            }
        };
        if report.is_solid() {
            out.objects.push(build_object(
                model,
                body,
                &report,
                world,
                name.clone(),
                material,
                options,
            ));
        } else {
            out.skipped.push(Skipped {
                name: name.clone(),
                reason: SkipReason::NotSolid(report),
            });
        }
    }

    if depth < 64 {
        for (_, child) in body.instances() {
            let child_name = object_name(model, child);
            walk(
                model,
                out,
                reports,
                child,
                world * child.transform,
                format!("{name}/{child_name}"),
                material,
                depth + 1,
                options,
            );
        }
    }
}

fn object_name(model: &Model, instance: &Instance) -> String {
    if !instance.name.is_empty() {
        return instance.name.clone();
    }
    if let Some(definition) = model.definition(instance.definition)
        && !definition.name.is_empty()
    {
        return definition.name.clone();
    }
    "Object".to_string()
}

fn build_object(
    model: &Model,
    body: &drydock_kernel::model::Body,
    report: &SolidReport,
    world: DMat4,
    name: String,
    inherited: Option<MaterialId>,
    options: &PrintOptions,
) -> PrintObject {
    let ctx = body.geometry();
    let tolerance = local_tolerance(options, world);
    let splits = split_points(ctx, tolerance);
    let reversed: BTreeSet<_> = report.reversed_faces.iter().copied().collect();
    let mirrored = world.determinant() < 0.0;
    let mut triangles = Vec::new();
    let mut areas: BTreeMap<MaterialId, f64> = BTreeMap::new();

    for (face_id, face) in ctx.faces() {
        let own = body.face_materials(face_id);
        let face_material = if reversed.contains(&face_id) {
            own.back
        } else {
            own.front
        }
        .or(inherited);

        let turn = reversed.contains(&face_id) ^ mirrored;
        for mut triangle in face_triangles(ctx, face, &splits) {
            if turn {
                triangle.swap(1, 2);
            }
            let placed = triangle.map(|p| world.transform_point3(p) * 1000.0);
            if let Some(material) = face_material {
                let area = (placed[1] - placed[0])
                    .cross(placed[2] - placed[0])
                    .length()
                    * 0.5;
                *areas.entry(material).or_insert(0.0) += area;
            }
            triangles.push(placed);
        }
    }

    let color = color_of(model, &areas);
    PrintObject {
        name,
        color,
        triangles,
    }
}

fn color_of(model: &Model, areas: &BTreeMap<MaterialId, f64>) -> Option<[f32; 4]> {
    let largest = areas.values().copied().fold(0.0f64, f64::max);
    let lowest = areas
        .iter()
        .filter(|(_, area)| **area >= largest - 1e-9 * largest.abs())
        .map(|(id, _)| *id)
        .next()?;
    model.material(lowest).map(|material| material.color)
}

fn local_tolerance(options: &PrintOptions, world: DMat4) -> f64 {
    let raw = options.chord_tolerance_mm;
    let meters = if raw.is_nan() || raw < MIN_CHORD_TOL * 1000.0 {
        MIN_CHORD_TOL
    } else {
        raw / 1000.0
    };
    if meters.is_infinite() {
        return f64::INFINITY;
    }
    let largest = [
        world.x_axis.truncate(),
        world.y_axis.truncate(),
        world.z_axis.truncate(),
    ]
    .into_iter()
    .map(DVec3::length)
    .fold(0.0f64, f64::max);
    if largest > 0.0 && largest.is_finite() {
        meters / largest
    } else {
        f64::INFINITY
    }
}

struct Split {
    start: VertexId,
    end: VertexId,
    points: Vec<DVec3>,
}

fn split_points(ctx: &Context, tolerance: f64) -> BTreeMap<EdgeId, Split> {
    let mut splits: BTreeMap<EdgeId, Split> = BTreeMap::new();
    for (_, curve) in ctx.curves() {
        for pair in curve.vertices.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            let Some(edge) = ctx.edge_between(start, end) else {
                continue;
            };
            if splits.contains_key(&edge) {
                continue;
            }
            let (Some(a), Some(b)) = (ctx.vertex(start), ctx.vertex(end)) else {
                continue;
            };
            let y_axis = curve.y_axis();
            let da = a.position - curve.center;
            let db = b.position - curve.center;
            let theta_a = da.dot(y_axis).atan2(da.dot(curve.x_axis));
            let theta_b = db.dot(y_axis).atan2(db.dot(curve.x_axis));
            let mut delta = theta_b - theta_a;
            while delta <= 0.0 {
                delta += TAU;
            }
            let k = split_count(curve.radius, delta, tolerance);
            let mut points = Vec::with_capacity(k.saturating_sub(1));
            for j in 1..k {
                points.push(curve.point_at(theta_a + delta * j as f64 / k as f64));
            }
            splits.insert(edge, Split { start, end, points });
        }
    }
    splits
}

fn split_count(radius: f64, delta: f64, tolerance: f64) -> usize {
    if !tolerance.is_finite() {
        return 1;
    }
    let mut k = 1;
    while k < 4096 {
        let half = delta / (2.0 * k as f64);
        if radius * (1.0 - half.cos()) <= tolerance {
            break;
        }
        k += 1;
    }
    k
}

fn refine_loop(
    ctx: &Context,
    lp: &Loop,
    splits: &BTreeMap<EdgeId, Split>,
) -> (Vec<Vec<DVec3>>, Vec<DVec3>) {
    let mut sides = Vec::new();
    for use_ in &lp.uses {
        let Some(start) = ctx.use_start(*use_) else {
            continue;
        };
        let Some(vertex) = ctx.vertex(start) else {
            continue;
        };
        let mut side = vec![vertex.position];
        if let Some(split) = splits.get(&use_.edge) {
            if start == split.start {
                side.extend(split.points.iter().copied());
            } else if start == split.end {
                side.extend(split.points.iter().rev().copied());
            }
        }
        sides.push(side);
    }
    let points = sides.iter().flatten().copied().collect();
    (sides, points)
}

fn face_triangles(ctx: &Context, face: &Face, splits: &BTreeMap<EdgeId, Split>) -> Vec<Triangle> {
    let outer_raw = ctx.loop_points(&face.outer);
    let normal = geom::polygon_normal(&outer_raw);
    if normal == DVec3::ZERO {
        return Vec::new();
    }

    let (outer_sides, outer) = refine_loop(ctx, &face.outer, splits);
    if outer.len() < 3 {
        return Vec::new();
    }
    let holes: Vec<Vec<DVec3>> = face
        .inner
        .iter()
        .map(|hole| refine_loop(ctx, hole, splits).1)
        .collect();

    let plane_point = outer[0];
    let flat = outer
        .iter()
        .chain(holes.iter().flatten())
        .all(|p| normal.dot(*p - plane_point).abs() <= 1e-9);

    if flat {
        return flat_triangles(&outer, &holes, normal);
    }

    if !holes.is_empty() {
        return flat_triangles(&outer, &holes, normal);
    }

    match outer_sides.len() {
        4 => {
            ladder_triangles(&outer_sides).unwrap_or_else(|| flat_triangles(&outer, &holes, normal))
        }
        3 => fan_triangles(&outer_sides).unwrap_or_else(|| flat_triangles(&outer, &holes, normal)),
        _ => flat_triangles(&outer, &holes, normal),
    }
}

fn flat_triangles(outer: &[DVec3], holes: &[Vec<DVec3>], normal: DVec3) -> Vec<Triangle> {
    let outer_2d = geom::to_plane_2d(outer, normal);
    let holes_2d: Vec<Vec<DVec2>> = holes
        .iter()
        .map(|hole| geom::to_plane_2d(hole, normal))
        .collect();
    let mut points = outer.to_vec();
    for hole in holes {
        points.extend_from_slice(hole);
    }
    tess::triangulate(&outer_2d, &holes_2d)
        .into_iter()
        .map(|[a, b, c]| [points[a], points[b], points[c]])
        .collect()
}

fn ladder_triangles(sides: &[Vec<DVec3>]) -> Option<Vec<Triangle>> {
    let split = |i: usize| sides[i].len() > 1;
    let s = if !split(1) && !split(3) {
        0
    } else if !split(0) && !split(2) {
        1
    } else {
        return None;
    };

    let mut a = sides[s].clone();
    a.push(sides[(s + 1) % 4][0]);
    let mut b_rev = sides[(s + 2) % 4].clone();
    b_rev.push(sides[(s + 3) % 4][0]);
    b_rev.reverse();
    let b = b_rev;

    let p = a.len() - 1;
    let q = b.len() - 1;
    let mut triangles = Vec::with_capacity(p + q);
    let (mut i, mut j) = (0, 0);
    while i < p || j < q {
        if i < p && (j == q || (i + 1) * q <= (j + 1) * p) {
            triangles.push([a[i], a[i + 1], b[j]]);
            i += 1;
        } else {
            triangles.push([a[i], b[j + 1], b[j]]);
            j += 1;
        }
    }
    Some(triangles)
}

fn fan_triangles(sides: &[Vec<DVec3>]) -> Option<Vec<Triangle>> {
    let split: Vec<usize> = (0..3).filter(|&i| sides[i].len() > 1).collect();
    let &[t] = split.as_slice() else {
        return None;
    };
    let apex = sides[(t + 2) % 3][0];
    let mut points = sides[t].clone();
    points.push(sides[(t + 1) % 3][0]);
    Some(
        (0..points.len() - 1)
            .map(|i| [points[i], points[i + 1], apex])
            .collect(),
    )
}
