// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Texture position handles in the viewport: one copy of the image outlined on the face,
//! with a pin at its bottom left corner, a turn grip at its bottom right and a scale grip at
//! its top right. The editor drives them (`Editor::start_texture_handles`); this module holds
//! the geometry and the overlay primitives so tests can check them without a window.
//!
//! Behaviour: `docs/specs/m4-app-texture-handles.md`.

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
    /// The bottom right corner: the texture turns about the pin.
    Turn,
    /// The top right corner: the texture scales about the pin.
    Scale,
}

/// The handles as they should be drawn now.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureHandles {
    pub face: FaceId,
    pub side: Side,
    /// One copy of the image on the face's plane, open-body coordinates, in this order:
    /// the pin (image bottom left), bottom right (Turn grip), top right (Scale grip), top
    /// left.
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

/// Overlay primitives for `handles`, open-body coordinates: the outline as a
/// `Style::BoundsBox` polygon, a `Style::Handle` marker at the pin, and markers at the Turn
/// and Scale corners styled `HandleGrabbed` when held, `HandleHover` when hovered, else
/// `Handle`.
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
            style: style(Grip::Scale),
        },
    ]
}
