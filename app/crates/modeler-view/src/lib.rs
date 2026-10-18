// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Viewport camera: orbit, pan, zoom, zoom extents and standard views.
//!
//! Behaviour: `docs/specs/m0-camera.md`. World space is meters, Z up, right-handed.
//! Screen space: pixels from the viewport's top-left corner, y down. NDC: x right,
//! y up, both -1..1 across the viewport.

use std::f64::consts::FRAC_PI_2;

use glam::{DMat4, DVec2, DVec3, DVec4};

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
        self.target + self.distance * self.offset()
    }

    /// Unit vectors: forward (eye to target), right, up. Never degenerate, including
    /// straight-down and straight-up views.
    pub fn basis(&self) -> (DVec3, DVec3, DVec3) {
        let forward = -self.offset();
        let right = DVec3::new(-self.yaw.sin(), self.yaw.cos(), 0.0);
        let up = right.cross(forward);
        (forward, right, up)
    }

    /// World to view (right-handed, camera looks down -Z).
    pub fn view_matrix(&self) -> DMat4 {
        let (forward, right, up) = self.basis();
        let eye = self.eye();
        DMat4::from_cols(
            DVec4::new(right.x, up.x, -forward.x, 0.0),
            DVec4::new(right.y, up.y, -forward.y, 0.0),
            DVec4::new(right.z, up.z, -forward.z, 0.0),
            DVec4::new(-right.dot(eye), -up.dot(eye), forward.dot(eye), 1.0),
        )
    }

    /// View to clip, depth 0..1 (wgpu convention). Near and far planes enclose
    /// `scene` (center, radius) when given.
    pub fn projection_matrix(&self, aspect: f64, scene: Option<(DVec3, f64)>) -> DMat4 {
        let (forward, _, _) = self.basis();
        let eye = self.eye();
        match self.projection {
            Projection::Perspective => {
                let (mut near, mut far) = match scene {
                    Some((center, radius)) => {
                        let d = (center - eye).dot(forward);
                        let far = d + radius;
                        let near = (d - radius).max(far / 10_000.0);
                        (near, far)
                    }
                    None => (self.distance / 1000.0, self.distance * 1000.0),
                };
                near = near.max(1e-4);
                far = far.max(1e-4);
                let f = 1.0 / (self.fov_y / 2.0).tan();
                DMat4::from_cols(
                    DVec4::new(f / aspect, 0.0, 0.0, 0.0),
                    DVec4::new(0.0, f, 0.0, 0.0),
                    DVec4::new(0.0, 0.0, far / (near - far), -1.0),
                    DVec4::new(0.0, 0.0, near * far / (near - far), 0.0),
                )
            }
            Projection::Parallel => {
                let (near, far) = match scene {
                    Some((center, radius)) => {
                        let d = (center - eye).dot(forward);
                        (d - radius - 1e-3, d + radius + 1e-3)
                    }
                    None => (-self.distance * 1000.0, self.distance * 1000.0),
                };
                let height = 2.0 * self.distance * (self.fov_y / 2.0).tan();
                DMat4::from_cols(
                    DVec4::new(2.0 / (height * aspect), 0.0, 0.0, 0.0),
                    DVec4::new(0.0, 2.0 / height, 0.0, 0.0),
                    DVec4::new(0.0, 0.0, 1.0 / (near - far), 0.0),
                    DVec4::new(0.0, 0.0, near / (near - far), 1.0),
                )
            }
        }
    }

    /// World point to NDC (x, y) and view depth (distance along forward). `None` when
    /// the point is behind a perspective camera.
    pub fn project(&self, p: DVec3, aspect: f64) -> Option<(DVec2, f64)> {
        let (forward, _, _) = self.basis();
        let depth = (p - self.eye()).dot(forward);
        if self.projection == Projection::Perspective && depth <= 0.0 {
            return None;
        }
        let clip = self.projection_matrix(aspect, None) * self.view_matrix() * p.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        Some((DVec2::new(ndc.x, ndc.y), depth))
    }

    /// The point on the plane through the target, facing the camera, that appears at
    /// `ndc`.
    pub fn target_plane_point(&self, ndc: DVec2, aspect: f64) -> DVec3 {
        let (_, right, up) = self.basis();
        let half_height = self.distance * (self.fov_y / 2.0).tan();
        self.target + right * (ndc.x * half_height * aspect) + up * (ndc.y * half_height)
    }

    /// World units per pixel on the target plane, for a viewport `height_px` tall.
    pub fn units_per_pixel(&self, height_px: f64) -> f64 {
        2.0 * self.distance * (self.fov_y / 2.0).tan() / height_px
    }

    /// Orbit about the target by a mouse drag of (dx, dy) pixels.
    pub fn orbit(&mut self, dx: f64, dy: f64) {
        self.yaw -= (dx * ORBIT_DEGREES_PER_PIXEL).to_radians();
        self.pitch += (dy * ORBIT_DEGREES_PER_PIXEL).to_radians();
        self.pitch = self.pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
    }

    /// Pan by a mouse drag of (dx, dy) pixels in a viewport `height_px` tall.
    pub fn pan(&mut self, dx: f64, dy: f64, height_px: f64) {
        let (_, right, up) = self.basis();
        self.target += (-dx * right + dy * up) * self.units_per_pixel(height_px);
    }

    /// Zoom by `steps` (positive = in) toward the point under `cursor_ndc`.
    pub fn zoom(&mut self, steps: f64, cursor_ndc: DVec2, aspect: f64) {
        let p = self.target_plane_point(cursor_ndc, aspect);
        let old = self.distance;
        let new = (old * ZOOM_STEP_FACTOR.powf(steps)).clamp(MIN_DISTANCE, MAX_DISTANCE);
        self.distance = new;
        let factor = new / old;
        self.target = p + (self.target - p) * factor;
    }

    /// Keep the orientation; center and fit `bounds` in a viewport of `aspect`
    /// (width / height).
    pub fn zoom_extents(&mut self, bounds: &Bounds, aspect: f64) {
        self.target = (bounds.min + bounds.max) * 0.5;
        let mut radius = (bounds.max - bounds.min).length() * 0.5;
        if radius < 1e-6 {
            radius = 1.0;
        }
        let tan_half = (self.fov_y / 2.0).tan();
        match self.projection {
            Projection::Perspective => {
                let fov_x = 2.0 * (aspect * tan_half).atan();
                let fit = self.fov_y.min(fov_x) / 2.0;
                self.distance = radius / fit.sin();
            }
            Projection::Parallel => {
                let height = if aspect >= 1.0 {
                    2.0 * radius
                } else {
                    2.0 * radius / aspect
                };
                self.distance = height / (2.0 * tan_half);
            }
        }
        self.distance = self.distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// Set yaw and pitch for a standard view. Target and distance are unchanged.
    pub fn set_standard_view(&mut self, view: StandardView) {
        let (yaw, pitch): (f64, f64) = match view {
            StandardView::Top => (-90.0, 90.0),
            StandardView::Bottom => (-90.0, -90.0),
            StandardView::Front => (-90.0, 0.0),
            StandardView::Back => (90.0, 0.0),
            StandardView::Left => (180.0, 0.0),
            StandardView::Right => (0.0, 0.0),
            StandardView::Iso => (-45.0, (1.0 / 3f64.sqrt()).asin().to_degrees()),
        };
        self.yaw = yaw.to_radians();
        self.pitch = pitch.to_radians();
    }

    fn offset(&self) -> DVec3 {
        DVec3::new(
            self.pitch.cos() * self.yaw.cos(),
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
        )
    }
}
