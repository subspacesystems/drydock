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

use crate::overlay::Primitive;

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
    let _ = (map, point, normal, st);
    unimplemented!("m4-app-texture-handles")
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
    let _ = (map, size, point, normal, center);
    unimplemented!("m4-app-texture-handles")
}

/// Overlay primitives for `handles`, open-body coordinates: the outline as a
/// `Style::BoundsBox` polygon, a `Style::Handle` marker at the pin, and markers at the Turn
/// and Scale corners styled `HandleGrabbed` when held, `HandleHover` when hovered, else
/// `Handle`.
pub fn primitives(handles: &TextureHandles) -> Vec<Primitive> {
    let _ = handles;
    unimplemented!("m4-app-texture-handles")
}
