// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! How a texture lies on a face: a map from body points to texture space, the default
//! placement, texture coordinates, fitting a map to given coordinates, and setting maps as
//! one undo step.
//!
//! Behaviour: `docs/specs/m4-texture-model.md`.

use glam::{DAffine2, DMat4, DVec2, DVec3, DVec4};

use crate::model::{BodyId, Model, Texture, UndoStack};
use crate::paint::{PaintError, Side};
use crate::topology::FaceId;

/// An affine map from a point of the body (meters) to texture space (meters):
/// `s = u · (p, 1)` and `t = v · (p, 1)`. Texture space has the image's bottom left corner at
/// `(0, 0)`, `s` to the right and `t` up; one copy of the image covers `Texture::size`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureMap {
    pub u: DVec4,
    pub v: DVec4,
}

impl TextureMap {
    /// The default placement on a side whose outward normal is `normal` (any length).
    pub fn default_for(normal: DVec3) -> TextureMap {
        let _ = normal;
        unimplemented!()
    }

    /// The texture-space point `(s, t)` of body point `p`.
    pub fn apply(&self, p: DVec3) -> DVec2 {
        let _ = p;
        unimplemented!()
    }

    /// Texture coordinates of `p` for `texture`: `apply(p) / texture.size`.
    pub fn uv(&self, texture: &Texture, p: DVec3) -> DVec2 {
        let _ = (texture, p);
        unimplemented!()
    }

    /// This map followed by `a` in texture space: `(s, t)` becomes `a(s, t)`.
    pub fn then(&self, a: DAffine2) -> TextureMap {
        let _ = a;
        unimplemented!()
    }

    /// The map for geometry moved by `m`: the moved point `m · p` gets what `p` had.
    /// `None` when `m` has a non-finite entry or `|det| < 1e-12`.
    pub fn transformed(&self, m: DMat4) -> Option<TextureMap> {
        let _ = m;
        unimplemented!()
    }

    /// The map that sends each of the planar `points` closest to its `st` (least squares in
    /// the points' plane), with no change along the plane's normal. `None` when the lengths
    /// differ, a number isn't finite, or the points don't span a plane.
    pub fn fit(points: &[DVec3], st: &[DVec2]) -> Option<TextureMap> {
        let _ = (points, st);
        unimplemented!()
    }

    pub fn is_finite(&self) -> bool {
        self.u.is_finite() && self.v.is_finite()
    }
}

/// The map in use on `side` of `face` in `body`: the stored one, else the default for that
/// side's outward normal. `None` when the body or the face doesn't exist.
pub fn side_map(model: &Model, body: BodyId, face: FaceId, side: Side) -> Option<TextureMap> {
    let _ = (model, body, face, side);
    unimplemented!()
}

/// Store `map` (`None` = back to the default) on the given side of each face, as one undo
/// step named `label`. Returns how many sides changed; no step when that is zero.
pub fn set_texture_maps(
    model: &mut Model,
    body: BodyId,
    changes: &[(FaceId, Side, Option<TextureMap>)],
    undo: &mut UndoStack,
    label: &str,
) -> Result<usize, PaintError> {
    let _ = (model, body, changes, undo, label);
    unimplemented!()
}
