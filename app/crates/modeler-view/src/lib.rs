// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Viewport camera: orbit, pan, zoom, zoom extents and standard views.
//!
//! Behaviour: `docs/specs/m0-camera.md`. World space is meters, Z up, right-handed.
//! Screen space: pixels from the viewport's top-left corner, y down. NDC: x right,
//! y up, both -1..1 across the viewport.

use glam::{DMat4, DVec2, DVec3};

/// Degrees of orbit per pixel dragged.
pub const ORBIT_DEGREES_PER_PIXEL: f64 = 0.5;
/// Each zoom step in scales the distance to the target by this factor.
pub const ZOOM_STEP_FACTOR: f64 = 0.8;
/// Smallest and largest allowed distance to the target, in meters.
pub const MIN_DISTANCE: f64 = 1e-4;
pub const MAX_DISTANCE: f64 = 1e7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Projection {
    Perspective,
    /// Parallel (orthographic). The visible height equals what a perspective camera
    /// would see at the target's distance, so switching keeps the target framed.
    Parallel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandardView {
    Top,
    Bottom,
    Front,
    Back,
    Left,
    Right,
    Iso,
}

/// Bounding box in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: DVec3,
    pub max: DVec3,
}

/// An orbiting camera: a target point, a distance, and two angles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// The point the camera looks at and orbits around.
    pub target: DVec3,
    /// Distance from the eye to the target, meters.
    pub distance: f64,
    /// Radians about +Z. The eye sits in direction (cos pitch cos yaw, cos pitch sin yaw,
    /// sin pitch) from the target.
    pub yaw: f64,
    /// Radians, -pi/2 (from below) to pi/2 (from above), inclusive.
    pub pitch: f64,
    /// Vertical field of view, radians.
    pub fov_y: f64,
    pub projection: Projection,
}

impl Default for Camera {
    /// Iso view of a 10 m cube at the origin, perspective, 35 degree field of view.
    fn default() -> Self {
        let mut c = Camera {
            target: DVec3::ZERO,
            distance: 10.0,
            yaw: 0.0,
            pitch: 0.0,
            fov_y: 35f64.to_radians(),
            projection: Projection::Perspective,
        };
        c.set_standard_view(StandardView::Iso);
        c.zoom_extents(
            &Bounds {
                min: DVec3::splat(-5.0),
                max: DVec3::splat(5.0),
            },
            1.0,
        );
        c
    }
}

impl Camera {
    /// Eye position in world space.
    pub fn eye(&self) -> DVec3 {
        unimplemented!("m0-camera")
    }

    /// Unit vectors: forward (eye to target), right, up. Never degenerate, including
    /// straight-down and straight-up views.
    pub fn basis(&self) -> (DVec3, DVec3, DVec3) {
        unimplemented!("m0-camera")
    }

    /// World to view (right-handed, camera looks down -Z).
    pub fn view_matrix(&self) -> DMat4 {
        unimplemented!("m0-camera")
    }

    /// View to clip, depth 0..1 (wgpu convention). Near and far planes enclose
    /// `scene` (center, radius) when given.
    pub fn projection_matrix(&self, aspect: f64, scene: Option<(DVec3, f64)>) -> DMat4 {
        let _ = (aspect, scene);
        unimplemented!("m0-camera")
    }

    /// World point to NDC (x, y) and view depth (distance along forward). `None` when
    /// the point is behind a perspective camera.
    pub fn project(&self, p: DVec3, aspect: f64) -> Option<(DVec2, f64)> {
        let _ = (p, aspect);
        unimplemented!("m0-camera")
    }

    /// The point on the plane through the target, facing the camera, that appears at
    /// `ndc`.
    pub fn target_plane_point(&self, ndc: DVec2, aspect: f64) -> DVec3 {
        let _ = (ndc, aspect);
        unimplemented!("m0-camera")
    }

    /// World units per pixel on the target plane, for a viewport `height_px` tall.
    pub fn units_per_pixel(&self, height_px: f64) -> f64 {
        let _ = height_px;
        unimplemented!("m0-camera")
    }

    /// Orbit about the target by a mouse drag of (dx, dy) pixels.
    pub fn orbit(&mut self, dx: f64, dy: f64) {
        let _ = (dx, dy);
        unimplemented!("m0-camera")
    }

    /// Pan by a mouse drag of (dx, dy) pixels in a viewport `height_px` tall.
    pub fn pan(&mut self, dx: f64, dy: f64, height_px: f64) {
        let _ = (dx, dy, height_px);
        unimplemented!("m0-camera")
    }

    /// Zoom by `steps` (positive = in) toward the point under `cursor_ndc`.
    pub fn zoom(&mut self, steps: f64, cursor_ndc: DVec2, aspect: f64) {
        let _ = (steps, cursor_ndc, aspect);
        unimplemented!("m0-camera")
    }

    /// Keep the orientation; center and fit `bounds` in a viewport of `aspect`
    /// (width / height).
    pub fn zoom_extents(&mut self, bounds: &Bounds, aspect: f64) {
        let _ = (bounds, aspect);
        unimplemented!("m0-camera")
    }

    /// Set yaw and pitch for a standard view. Target and distance are unchanged.
    pub fn set_standard_view(&mut self, view: StandardView) {
        let _ = view;
        unimplemented!("m0-camera")
    }
}
