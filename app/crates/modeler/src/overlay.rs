// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Overlay primitives for the M3 tools: what the window paints over the model besides the
//! tools' own previews. Pure world-space data; the window projects and strokes it.
//!
//! Behaviour: `docs/specs/m3-ui-overlay.md`.

use glam::{DMat4, DVec3};
use modeler_kernel::model::{BodyId, Model};
use modeler_kernel::select::Entity;
use modeler_kernel::tape::{Guide, Guides};
use modeler_kernel::tools::plane::DrawPlane;
use modeler_kernel::tools::scale::{Bounds, Handle, handles};
use modeler_kernel::topology::{Context, FaceId};

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
    let _ = (model, body, transform, selection);
    unimplemented!()
}

/// `primitives` with every point mapped through `transform`; styles unchanged.
pub fn transformed(primitives: Vec<Primitive>, transform: DMat4) -> Vec<Primitive> {
    let _ = (primitives, transform);
    unimplemented!()
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
