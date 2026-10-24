// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Texture position handles in the viewport: one copy of the image outlined on the face,
//! with a scale grip at each corner and a turn grip on a short stem above the top edge. The
//! editor drives them (`Editor::start_texture_handles`); this module holds the geometry,
//! snapping helpers and overlay primitives so tests can check them without a window.
//!
//! Behaviour: `docs/specs/m4-app-texture-handles.md`, reworked by
//! `docs/specs/m4-texture-handles-2.md`.

use eframe::egui;
use glam::{DVec2, DVec3};
use modeler_kernel::paint::Side;
use modeler_kernel::texture::TextureMap;
use modeler_kernel::topology::FaceId;

use crate::overlay::{Primitive, Style};

/// How close (pixels) the cursor must be to a corner grip to grab it.
pub const GRIP_PX: f64 = 8.0;

/// What a click grabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    /// Inside the outlined image copy: the texture slides with the cursor.
    Slide,
    /// The grip above the top edge: the texture turns about the image copy's center.
    Turn,
    /// Corner `i` (index into `TextureHandles::corners`): the texture scales about the
    /// opposite corner `(i + 2) % 4`.
    Scale(usize),
}

impl Grip {
    /// The mouse cursor to show while this grip is hovered or held.
    pub fn cursor(self) -> egui::CursorIcon {
        unimplemented!("m4-texture-handles-2")
    }
}

/// How close (pixels) a dragged point must come to a face vertex or edge to snap to it.
pub const SNAP_PX: f64 = 10.0;

/// How close (radians, 3 degrees) a turn must come to lining the image up with a face edge
/// to snap to it.
pub const SNAP_ANGLE: f64 = 3.0 * std::f64::consts::PI / 180.0;

/// How far above the top edge the turn grip sits, as a fraction of the image copy's height.
pub const TURN_STEM: f64 = 0.25;

/// What a snapped point landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    Vertex,
    Edge,
}

/// The handles as they should be drawn now.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureHandles {
    pub face: FaceId,
    pub side: Side,
    /// One copy of the image on the face's plane, open-body coordinates, in this order:
    /// bottom left, bottom right, top right, top left.
    pub corners: [DVec3; 4],
    /// The grip under the cursor while nothing is held.
    pub hover: Option<Grip>,
    /// The grip being moved.
    pub held: Option<Grip>,
}

/// The point of the plane through `point` with normal `normal` whose texture point under
/// `map` is `st`. `None` when the map doesn't cover the plane one-to-one (|det| < 1e-12 in
/// the plane) or a number isn't finite.
pub fn plane_point_at(map: &TextureMap, point: DVec3, normal: DVec3, st: DVec2) -> Option<DVec3> {
    if !map.is_finite() || !point.is_finite() || !normal.is_finite() || !st.is_finite() {
        return None;
    }
    let length = normal.length();
    if !length.is_finite() || length == 0.0 {
        return None;
    }
    let n = normal / length;
    let helper = if n.z.abs() < 0.9 { DVec3::Z } else { DVec3::Y };
    let e1 = helper.cross(n).normalize();
    let e2 = n.cross(e1).normalize();

    let base = map.apply(point);
    let a1 = DVec2::new(map.u.truncate().dot(e1), map.v.truncate().dot(e1));
    let a2 = DVec2::new(map.u.truncate().dot(e2), map.v.truncate().dot(e2));
    let det = a1.x * a2.y - a2.x * a1.y;
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    let rhs = st - base;
    let a = (rhs.x * a2.y - a2.x * rhs.y) / det;
    let b = (a1.x * rhs.y - rhs.x * a1.y) / det;
    let result = point + a * e1 + b * e2;
    result.is_finite().then_some(result)
}

/// The corners (order as in [`TextureHandles::corners`]) of the image copy whose
/// texture-space cell `[k·size, (k+1)·size)` holds `center`'s texture point.
pub fn tile_corners(
    map: &TextureMap,
    size: DVec2,
    point: DVec3,
    normal: DVec3,
    center: DVec3,
) -> Option<[DVec3; 4]> {
    if !size.is_finite() || size.x <= 0.0 || size.y <= 0.0 {
        return None;
    }
    let c = map.apply(center);
    let base = DVec2::new(
        (c.x / size.x).floor() * size.x,
        (c.y / size.y).floor() * size.y,
    );
    let offsets = [
        DVec2::ZERO,
        DVec2::new(size.x, 0.0),
        size,
        DVec2::new(0.0, size.y),
    ];
    let mut corners = [DVec3::ZERO; 4];
    for (corner, offset) in corners.iter_mut().zip(offsets) {
        *corner = plane_point_at(map, point, normal, base + offset)?;
    }
    Some(corners)
}

/// The point of the turn grip: the middle of the top edge, pushed `TURN_STEM` of the
/// image copy's height further up. `corners` as in [`TextureHandles::corners`].
pub fn turn_grip_point(corners: &[DVec3; 4]) -> DVec3 {
    let _ = corners;
    unimplemented!("m4-texture-handles-2")
}

/// Where a dragged point snaps. `project` maps an open-body point to pixels (`None` when it
/// can't be shown), `cursor` is in pixels and `loops` are the face's loops (outer loop and
/// holes) as closed point lists. A vertex within `SNAP_PX` of the cursor wins (the nearest);
/// else the nearest point on a loop edge within `SNAP_PX` (nearest in pixels, the point on
/// the 3D edge at the same parameter); else `None`.
pub fn snap_point(
    project: &dyn Fn(DVec3) -> Option<DVec2>,
    cursor: DVec2,
    loops: &[Vec<DVec3>],
) -> Option<(DVec3, SnapKind)> {
    let _ = (project, cursor, loops);
    unimplemented!("m4-texture-handles-2")
}

/// Which image corner lands on a face vertex: among all pairs of a `corners` entry and a
/// `vertices` entry whose pixel distance is at most `SNAP_PX`, the closest pair as
/// `(corner index, vertex)` (ties: lower corner index, then earlier vertex).
pub fn snap_corner(
    project: &dyn Fn(DVec3) -> Option<DVec2>,
    corners: &[DVec3; 4],
    vertices: &[DVec3],
) -> Option<(usize, DVec3)> {
    let _ = (project, corners, vertices);
    unimplemented!("m4-texture-handles-2")
}

/// `angle` (radians, a rotation of texture space) adjusted so the image lines up with the
/// face: `edge_dirs` are the face's edge directions in texture space; after turning by `θ`
/// a direction at angle `φ` lies along the image's bottom or side edges when `θ + φ` is a
/// multiple of 90 degrees. Returns the nearest such `θ` when it is within `SNAP_ANGLE` of
/// `angle`, else `angle` itself. Zero or non-finite directions are ignored; a non-finite
/// `angle` is returned unchanged.
pub fn snap_turn(angle: f64, edge_dirs: &[DVec2]) -> f64 {
    let _ = (angle, edge_dirs);
    unimplemented!("m4-texture-handles-2")
}

/// The short help line for the status bar.
pub fn help_text(handles: &TextureHandles) -> &'static str {
    let _ = handles;
    unimplemented!("m4-texture-handles-2")
}

/// Overlay primitives for `handles`, open-body coordinates.
pub fn primitives(handles: &TextureHandles) -> Vec<Primitive> {
    let style = |grip: Grip| {
        if handles.held == Some(grip) {
            Style::HandleGrabbed
        } else if handles.hover == Some(grip) {
            Style::HandleHover
        } else {
            Style::Handle
        }
    };
    vec![
        Primitive::Polygon {
            points: handles.corners.to_vec(),
            style: Style::BoundsBox,
        },
        Primitive::Marker {
            at: handles.corners[0],
            style: Style::Handle,
        },
        Primitive::Marker {
            at: handles.corners[1],
            style: style(Grip::Turn),
        },
        Primitive::Marker {
            at: handles.corners[2],
            style: style(Grip::Scale(2)),
        },
    ]
}
