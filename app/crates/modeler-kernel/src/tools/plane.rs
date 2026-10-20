// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The plane a shape tool draws in.

use glam::DVec3;

/// Two unit, perpendicular directions. The normal is `u × v`. The plane passes through the
/// first point a tool is given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawPlane {
    pub u: DVec3,
    pub v: DVec3,
}

impl DrawPlane {
    pub fn normal(&self) -> DVec3 {
        self.u.cross(self.v)
    }

    /// `p` moved into the plane through `origin`, along the normal.
    pub fn project(&self, origin: DVec3, p: DVec3) -> DVec3 {
        let d = p - origin;
        origin + self.u * d.dot(self.u) + self.v * d.dot(self.v)
    }
}
