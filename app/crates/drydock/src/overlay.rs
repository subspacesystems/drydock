// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Overlay primitives for the M3 tools: what the window paints over the model besides the
//! tools' own previews. Pure world-space data; the window projects and strokes it.
//!
//! Behaviour: `docs/specs/m3-ui-overlay.md`.

use drydock_kernel::geom;
use drydock_kernel::model::{BodyId, Model};
use drydock_kernel::select::{Entity, instance_segments};
use drydock_kernel::tape::{Guide, Guides};
use drydock_kernel::tess;
use drydock_kernel::tools::plane::DrawPlane;
use drydock_kernel::tools::scale::{Bounds, Handle, handles};
use drydock_kernel::topology::{Context, FaceId};
use drydock_view::Camera;
use glam::{DMat4, DVec3};

/// Box edges shorter than this are not drawn.
const MIN_EDGE: f64 = 1e-9;

/// How the window should draw a primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// The face Push/Pull would act on.
    FaceHighlight,
    /// The bounding box of the Scale tool.
    BoundsBox,
    /// A scale handle.
    Handle,
    /// The scale handle under the cursor.
    HandleHover,
    /// The scale handle being dragged.
    HandleGrabbed,
    /// The protractor's circle.
    ProtractorRing,
    /// A 15 degree mark on the protractor.
    ProtractorTick,
    /// A protractor arm.
    ProtractorArm,
    /// A guide line.
    Guide,
    /// A guide point.
    GuidePoint,
    /// A selected edge, or an edge of a selected instance.
    Selected,
    /// One triangle of a selected face: filled as a tint, no outline.
    SelectedFace,
    /// An open, loose or shared edge of the selected part (`docs/specs/m7-fix-marks.md`).
    Problem,
    /// Half of an axis line from the origin: `axis` 0 red, 1 green, 2 blue; the positive
    /// half is drawn solid, the negative half dashed (`docs/specs/m4-ui-axes.md`).
    Axis { axis: usize, positive: bool },
}

/// Stroke of [`Style::Problem`]: `drydock_dae::PROBLEM_COLOR` as sRGB bytes (each channel
/// times 255, rounded) and the width in points (4.0, wider than `Style::Selected`).
pub fn problem_stroke() -> ([u8; 4], f32) {
    let [r, g, b, a] = drydock_dae::PROBLEM_COLOR;
    let byte = |c: f32| (c * 255.0).round() as u8;
    ([byte(r), byte(g), byte(b), byte(a)], 4.0)
}

/// One thing to draw, in world coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    Line {
        a: DVec3,
        b: DVec3,
        style: Style,
    },
    /// A closed outline (the last point joins the first).
    Polygon {
        points: Vec<DVec3>,
        style: Style,
    },
    /// A small square or cross of fixed pixel size, centred on `at`.
    Marker {
        at: DVec3,
        style: Style,
    },
}

/// Outlines of `face`: the outer loop first, then each hole, all `Style::FaceHighlight`.
pub fn face_highlight(ctx: &Context, face: FaceId) -> Vec<Primitive> {
    let Some(face) = ctx.face(face) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(1 + face.inner.len());
    out.push(Primitive::Polygon {
        points: ctx.loop_points(&face.outer),
        style: Style::FaceHighlight,
    });
    for hole in &face.inner {
        out.push(Primitive::Polygon {
            points: ctx.loop_points(hole),
            style: Style::FaceHighlight,
        });
    }
    out
}

/// The highlight for `selection`, entities of `body`, drawn in world coordinates: every
/// point is mapped through `transform` (body to world; the open instance's placement while
/// editing inside one). Missing entities are skipped. Behaviour:
/// `docs/specs/m4-fix-edit-overlays.md`.
pub fn selection(
    model: &Model,
    body: BodyId,
    transform: DMat4,
    selection: &[Entity],
) -> Vec<Primitive> {
    let Some(target) = model.body(body) else {
        return Vec::new();
    };
    let ctx = target.geometry();
    let mut out = Vec::new();
    for entity in selection {
        match *entity {
            Entity::Edge(id) => {
                let Some(edge) = ctx.edge(id) else {
                    continue;
                };
                let (Some(a), Some(b)) = (ctx.vertex(edge.ends[0]), ctx.vertex(edge.ends[1]))
                else {
                    continue;
                };
                out.push(Primitive::Line {
                    a: transform.transform_point3(a.position),
                    b: transform.transform_point3(b.position),
                    style: Style::Selected,
                });
            }
            Entity::Face(id) => {
                let Some(face) = ctx.face(id) else {
                    continue;
                };
                let Some(plane) = ctx.face_plane(id) else {
                    continue;
                };
                let normal = plane.normal;
                let outer = ctx.loop_points(&face.outer);
                let holes: Vec<Vec<DVec3>> = face
                    .inner
                    .iter()
                    .map(|hole| ctx.loop_points(hole))
                    .collect();
                let outer_2d = geom::to_plane_2d(&outer, normal);
                let holes_2d: Vec<Vec<_>> = holes
                    .iter()
                    .map(|hole| geom::to_plane_2d(hole, normal))
                    .collect();
                let points: Vec<DVec3> = outer
                    .iter()
                    .chain(holes.iter().flatten())
                    .copied()
                    .collect();
                for triangle in tess::triangulate(&outer_2d, &holes_2d) {
                    out.push(Primitive::Polygon {
                        points: triangle
                            .iter()
                            .map(|&i| transform.transform_point3(points[i]))
                            .collect(),
                        style: Style::SelectedFace,
                    });
                }
            }
            Entity::Instance(id) => {
                for (a, b) in instance_segments(model, body, id) {
                    out.push(Primitive::Line {
                        a: transform.transform_point3(a),
                        b: transform.transform_point3(b),
                        style: Style::Selected,
                    });
                }
            }
        }
    }
    out
}

/// `primitives` with every point mapped through `transform`; styles unchanged.
pub fn transformed(primitives: Vec<Primitive>, transform: DMat4) -> Vec<Primitive> {
    primitives
        .into_iter()
        .map(|primitive| match primitive {
            Primitive::Line { a, b, style } => Primitive::Line {
                a: transform.transform_point3(a),
                b: transform.transform_point3(b),
                style,
            },
            Primitive::Polygon { points, style } => Primitive::Polygon {
                points: points
                    .into_iter()
                    .map(|p| transform.transform_point3(p))
                    .collect(),
                style,
            },
            Primitive::Marker { at, style } => Primitive::Marker {
                at: transform.transform_point3(at),
                style,
            },
        })
        .collect()
}

fn push_box_edge(out: &mut Vec<Primitive>, seen: &mut Vec<(DVec3, DVec3)>, a: DVec3, b: DVec3) {
    if a.distance(b) < MIN_EDGE {
        return;
    }
    let (lo, hi) = (a.min(b), a.max(b));
    if seen.iter().any(|(s, e)| *s == lo && *e == hi) {
        return;
    }
    seen.push((lo, hi));
    out.push(Primitive::Line {
        a,
        b,
        style: Style::BoundsBox,
    });
}

/// The Scale tool's box and handles.
pub fn scale_handles(
    bounds: &Bounds,
    hover: Option<Handle>,
    grabbed: Option<Handle>,
) -> Vec<Primitive> {
    let min = [bounds.min.x, bounds.min.y, bounds.min.z];
    let max = [bounds.max.x, bounds.max.y, bounds.max.z];
    let mut out = Vec::new();
    let mut seen = Vec::new();
    for axis in 0..3 {
        let others: Vec<usize> = (0..3).filter(|&i| i != axis).collect();
        for side_a in 0..2 {
            for side_b in 0..2 {
                let mut a = min;
                let mut b = min;
                b[axis] = max[axis];
                for (side, other) in [(side_a, others[0]), (side_b, others[1])] {
                    let value = if side == 0 { min[other] } else { max[other] };
                    a[other] = value;
                    b[other] = value;
                }
                push_box_edge(
                    &mut out,
                    &mut seen,
                    DVec3::from_array(a),
                    DVec3::from_array(b),
                );
            }
        }
    }
    for handle in handles(bounds) {
        let style = if grabbed == Some(handle) {
            Style::HandleGrabbed
        } else if hover == Some(handle) {
            Style::HandleHover
        } else {
            Style::Handle
        };
        out.push(Primitive::Marker {
            at: bounds.point(handle),
            style,
        });
    }
    out
}

fn ring_point(center: DVec3, plane: DrawPlane, radius: f64, degrees: f64) -> DVec3 {
    let t = degrees.to_radians();
    center + radius * (t.cos() * plane.u + t.sin() * plane.v)
}

/// The Rotate tool's protractor.
pub fn protractor(
    center: DVec3,
    plane: DrawPlane,
    radius: f64,
    start: Option<DVec3>,
    angle: Option<f64>,
) -> Vec<Primitive> {
    if !radius.is_finite() || radius <= 0.0 {
        return Vec::new();
    }
    let point = |degrees: f64| ring_point(center, plane, radius, degrees);
    let mut out = Vec::with_capacity(50);
    for k in 0..24 {
        out.push(Primitive::Line {
            a: point(15.0 * k as f64),
            b: point(15.0 * (k + 1) as f64),
            style: Style::ProtractorRing,
        });
    }
    for k in 0..24 {
        let p = point(15.0 * k as f64);
        out.push(Primitive::Line {
            a: center + 0.9 * (p - center),
            b: p,
            style: Style::ProtractorTick,
        });
    }
    let Some(start) = start.filter(|s| s.is_finite()) else {
        return out;
    };
    let normal = plane.normal();
    let flat = start - normal * start.dot(normal);
    let length = flat.length();
    if !length.is_finite() || length == 0.0 {
        return out;
    }
    let start_dir = flat / length;
    out.push(Primitive::Line {
        a: center,
        b: center + radius * start_dir,
        style: Style::ProtractorArm,
    });
    if let Some(angle) = angle.filter(|a| a.is_finite()) {
        let t = angle.to_radians();
        let dir = start_dir * t.cos() + normal.cross(start_dir) * t.sin();
        out.push(Primitive::Line {
            a: center,
            b: center + radius * dir,
            style: Style::ProtractorArm,
        });
    }
    out
}

/// The model's guides, lines clipped to a stretch around `around`.
pub fn guides(guides: &Guides, around: DVec3, half_length: f64) -> Vec<Primitive> {
    let long_enough = half_length.is_finite() && half_length > 0.0;
    let mut out = Vec::new();
    for (_, guide) in guides.iter() {
        match *guide {
            Guide::Point(p) => out.push(Primitive::Marker {
                at: p,
                style: Style::GuidePoint,
            }),
            Guide::Line { point, dir } => {
                if !long_enough {
                    continue;
                }
                let c = point + dir * dir.dot(around - point);
                out.push(Primitive::Line {
                    a: c - dir * half_length,
                    b: c + dir * half_length,
                    style: Style::Guide,
                });
            }
        }
    }
    out
}

/// The axes of the frame `transform` maps from: for each of red, green and blue (local x,
/// y, z), the positive then the negative half, each a line from the frame's origin
/// `half_length` long in world units. Behaviour: `docs/specs/m4-ui-axes.md`.
pub fn axes(transform: DMat4, half_length: f64) -> Vec<Primitive> {
    if !half_length.is_finite() || half_length <= 0.0 {
        return Vec::new();
    }
    let origin = transform.transform_point3(DVec3::ZERO);
    if !origin.is_finite() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(6);
    for (axis, unit) in [DVec3::X, DVec3::Y, DVec3::Z].into_iter().enumerate() {
        let mapped = transform.transform_vector3(unit);
        let length = mapped.length();
        if !length.is_finite() || length == 0.0 {
            continue;
        }
        let dir = mapped / length;
        out.push(Primitive::Line {
            a: origin,
            b: origin + dir * half_length,
            style: Style::Axis {
                axis,
                positive: true,
            },
        });
        out.push(Primitive::Line {
            a: origin,
            b: origin - dir * half_length,
            style: Style::Axis {
                axis,
                positive: false,
            },
        });
    }
    out
}

/// Width of a drawn axis in points.
pub const AXIS_WIDTH_PT: f32 = 1.5;

/// Dash and gap of a negative half axis in points.
pub const AXIS_DASH_PT: [f32; 2] = [6.0, 4.0];

/// The axes of `transform` as GPU lines, sized for `pixels_per_point`
/// (`docs/specs/m4-axes-screen.md`).
pub fn axis_lines(
    transform: DMat4,
    half_length: f64,
    pixels_per_point: f32,
) -> Vec<drydock_render::ExtraLine> {
    const COLORS: [[f32; 4]; 3] = [
        [200.0 / 255.0, 40.0 / 255.0, 40.0 / 255.0, 1.0],
        [40.0 / 255.0, 150.0 / 255.0, 40.0 / 255.0, 1.0],
        [40.0 / 255.0, 70.0 / 255.0, 210.0 / 255.0, 1.0],
    ];
    if !half_length.is_finite() || half_length <= 0.0 {
        return Vec::new();
    }
    let origin = transform.transform_point3(DVec3::ZERO);
    if !origin.is_finite() {
        return Vec::new();
    }
    let ppp = if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
        pixels_per_point
    } else {
        1.0
    };
    let width_px = AXIS_WIDTH_PT * ppp;
    let dash_px = [AXIS_DASH_PT[0] * ppp, AXIS_DASH_PT[1] * ppp];
    let mut out = Vec::new();
    for (axis, unit) in [DVec3::X, DVec3::Y, DVec3::Z].into_iter().enumerate() {
        let mapped = transform.transform_vector3(unit);
        let length = mapped.length();
        if !length.is_finite() || length == 0.0 {
            continue;
        }
        let dir = mapped / length;
        let color = COLORS[axis];
        out.push(drydock_render::ExtraLine {
            a: origin,
            b: origin + dir * half_length,
            color,
            width_px,
            dash_px: None,
        });
        out.push(drydock_render::ExtraLine {
            a: origin,
            b: origin - dir * half_length,
            color,
            width_px,
            dash_px: Some(dash_px),
        });
    }
    out
}

/// The part of segment `a`-`b` a perspective camera can draw: in front of its near
/// distance (`camera.distance / 1000` along the view direction). A parallel camera keeps the
/// whole segment. `None` when nothing is left. Behaviour: `docs/specs/m4-ui-axes.md`.
pub fn clip_in_front(camera: &Camera, a: DVec3, b: DVec3) -> Option<(DVec3, DVec3)> {
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    if camera.projection != drydock_view::Projection::Perspective {
        return Some((a, b));
    }
    let near = camera.distance / 1000.0;
    let (forward, _, _) = camera.basis();
    let eye = camera.eye();
    let depth_a = (a - eye).dot(forward);
    let depth_b = (b - eye).dot(forward);
    let keep_a = depth_a >= near;
    let keep_b = depth_b >= near;
    match (keep_a, keep_b) {
        (true, true) => Some((a, b)),
        (false, false) => None,
        (false, true) => {
            let t = (near - depth_a) / (depth_b - depth_a);
            Some((a + (b - a) * t, b))
        }
        (true, false) => {
            let t = (near - depth_a) / (depth_b - depth_a);
            Some((a, a + (b - a) * t))
        }
    }
}
