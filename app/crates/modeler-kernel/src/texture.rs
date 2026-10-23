// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! How a texture lies on a face: a map from body points to texture space, the default
//! placement, texture coordinates, fitting a map to given coordinates, and setting maps as
//! one undo step.
//!
//! Behaviour: `docs/specs/m4-texture-model.md`.

use glam::{DAffine2, DMat4, DVec2, DVec3, DVec4};

use crate::geom;
use crate::model::{BodyId, FaceMaterials, Model, ModelEdit, Texture, UndoStack};
use crate::paint::{PaintError, Side};
use crate::select::Entity;
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
        let n = if normal.is_finite() {
            let length = normal.length();
            if length > 0.0 && length.is_finite() {
                normal / length
            } else {
                DVec3::Z
            }
        } else {
            DVec3::Z
        };

        let h = DVec3::Z - n * n.z;
        let v = if h.length() >= 1e-6 {
            h.normalize()
        } else {
            (DVec3::Y - n * n.y).normalize()
        };
        let u = v.cross(n);
        TextureMap {
            u: u.extend(0.0),
            v: v.extend(0.0),
        }
    }

    /// The texture-space point `(s, t)` of body point `p`.
    pub fn apply(&self, p: DVec3) -> DVec2 {
        let p = p.extend(1.0);
        DVec2::new(self.u.dot(p), self.v.dot(p))
    }

    /// Texture coordinates of `p` for `texture`: `apply(p) / texture.size`.
    pub fn uv(&self, texture: &Texture, p: DVec3) -> DVec2 {
        self.apply(p) / texture.size
    }

    /// This map followed by `a` in texture space: `(s, t)` becomes `a(s, t)`.
    pub fn then(&self, a: DAffine2) -> TextureMap {
        let x = a.matrix2.x_axis;
        let y = a.matrix2.y_axis;
        let t = a.translation;
        let combine = |first: DVec4, second: DVec4, offset: f64| {
            DVec4::new(
                x.x * first.x + y.x * second.x,
                x.x * first.y + y.x * second.y,
                x.x * first.z + y.x * second.z,
                x.x * first.w + y.x * second.w + offset,
            )
        };
        TextureMap {
            u: combine(self.u, self.v, t.x),
            v: DVec4::new(
                x.y * self.u.x + y.y * self.v.x,
                x.y * self.u.y + y.y * self.v.y,
                x.y * self.u.z + y.y * self.v.z,
                x.y * self.u.w + y.y * self.v.w + t.y,
            ),
        }
    }

    /// The map for geometry moved by `m`: the moved point `m · p` gets what `p` had.
    /// `None` when `m` has a non-finite entry or `|det| < 1e-12`.
    pub fn transformed(&self, m: DMat4) -> Option<TextureMap> {
        if !m.to_cols_array().iter().all(|value| value.is_finite()) || m.determinant().abs() < 1e-12
        {
            return None;
        }
        let inverse = m.inverse();
        let row = |row: DVec4| {
            let mut out = [0.0; 4];
            for (k, value) in out.iter_mut().enumerate() {
                *value = row.dot(inverse.col(k));
            }
            DVec4::from_array(out)
        };
        Some(TextureMap {
            u: row(self.u),
            v: row(self.v),
        })
    }

    /// The map that sends each of the planar `points` closest to its `st` (least squares in
    /// the points' plane), with no change along the plane's normal. `None` when the lengths
    /// differ, a number isn't finite, or the points don't span a plane.
    pub fn fit(points: &[DVec3], st: &[DVec2]) -> Option<TextureMap> {
        if points.len() != st.len()
            || points.len() < 3
            || points.iter().any(|p| !p.is_finite())
            || st.iter().any(|p| !p.is_finite())
        {
            return None;
        }
        let n = geom::polygon_normal(points);
        if n == DVec3::ZERO || !n.is_finite() {
            return None;
        }

        let basis = TextureMap::default_for(n);
        let e1 = basis.u.truncate();
        let e2 = basis.v.truncate();
        let p0 = points[0];

        let mut matrix = [[0.0f64; 3]; 3];
        let mut rhs_s = [0.0f64; 3];
        let mut rhs_t = [0.0f64; 3];
        for (&p, &q) in points.iter().zip(st) {
            let a = (p - p0).dot(e1);
            let b = (p - p0).dot(e2);
            let row = [a, b, 1.0];
            for i in 0..3 {
                for j in 0..3 {
                    matrix[i][j] += row[i] * row[j];
                }
                rhs_s[i] += row[i] * q.x;
                rhs_t[i] += row[i] * q.y;
            }
        }

        let s = solve3(matrix, rhs_s)?;
        let t = solve3(matrix, rhs_t)?;
        let u_xyz = e1 * s[0] + e2 * s[1];
        let v_xyz = e1 * t[0] + e2 * t[1];
        Some(TextureMap {
            u: DVec4::new(u_xyz.x, u_xyz.y, u_xyz.z, s[2] - u_xyz.dot(p0)),
            v: DVec4::new(v_xyz.x, v_xyz.y, v_xyz.z, t[2] - v_xyz.dot(p0)),
        })
    }

    pub fn is_finite(&self) -> bool {
        self.u.is_finite() && self.v.is_finite()
    }
}

/// The map in use on `side` of `face` in `body`: the stored one, else the default for that
/// side's outward normal. `None` when the body or the face doesn't exist.
pub fn side_map(model: &Model, body: BodyId, face: FaceId, side: Side) -> Option<TextureMap> {
    let body = model.body(body)?;
    let face_data = body.geometry().face(face)?;
    let materials = body.face_materials(face);
    let stored = match side {
        Side::Front => materials.front_map,
        Side::Back => materials.back_map,
    };
    if let Some(map) = stored {
        return Some(map);
    }
    let mut normal = geom::polygon_normal(&body.geometry().loop_points(&face_data.outer));
    if side == Side::Back {
        normal = -normal;
    }
    Some(TextureMap::default_for(normal))
}

/// `materials` of a face whose geometry moves by `m`, with each side's map carried along so
/// every moved point keeps its texture coordinates. A stored map becomes
/// `map.transformed(m)`; a side without one whose material (in `model`) has a texture gets
/// the default placement for that side carried along (`front_normal` is the front's outward
/// normal before the move; the back uses its opposite). Sides with neither keep no map. The
/// material ids don't change, and nothing changes when `m` can't be inverted.
///
/// Behaviour: `docs/specs/m4-texture-follow.md`.
pub fn moved_maps(
    model: &Model,
    materials: FaceMaterials,
    front_normal: DVec3,
    m: DMat4,
) -> FaceMaterials {
    let _ = (model, materials, front_normal, m);
    unimplemented!("m4-texture-follow")
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
    if changes.is_empty() {
        return Err(PaintError::Empty);
    }
    let current =
        model
            .body(body)
            .ok_or(PaintError::Model(crate::model::ModelError::MissingBody(
                body,
            )))?;
    for &(face, _, _) in changes {
        if current.geometry().face(face).is_none() {
            return Err(PaintError::Missing(Entity::Face(face)));
        }
    }
    for &(_, _, map) in changes {
        if map.is_some_and(|map| !map.is_finite()) {
            return Err(PaintError::Model(crate::model::ModelError::BadTextureMap));
        }
    }

    let mut final_changes: Vec<(FaceId, Side, Option<TextureMap>)> = Vec::new();
    for &(face, side, map) in changes {
        if let Some(existing) = final_changes
            .iter_mut()
            .find(|(f, s, _)| *f == face && *s == side)
        {
            existing.2 = map;
        } else {
            final_changes.push((face, side, map));
        }
    }

    let mut log = ModelEdit::new();
    let mut changed = 0;
    for (face, side, map) in final_changes {
        let from = model
            .body(body)
            .expect("the body was checked")
            .face_materials(face);
        let to = match side {
            Side::Front => FaceMaterials {
                front_map: map,
                ..from
            },
            Side::Back => FaceMaterials {
                back_map: map,
                ..from
            },
        };
        if to == from {
            continue;
        }
        if let Err(error) = model.set_face_materials(body, face, to, &mut log) {
            let _ = model.apply_edit(&log.inverse());
            return Err(PaintError::Model(error));
        }
        changed += 1;
    }

    if changed > 0 {
        undo.push(label, log);
    }
    Ok(changed)
}

fn solve3(mut matrix: [[f64; 3]; 3], mut rhs: [f64; 3]) -> Option<[f64; 3]> {
    let scale = matrix
        .iter()
        .flatten()
        .fold(0.0f64, |largest, value| largest.max(value.abs()));
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let tolerance = scale * 1e-14;

    for column in 0..3 {
        let pivot = (column..3)
            .max_by(|&a, &b| matrix[a][column].abs().total_cmp(&matrix[b][column].abs()))
            .expect("a non-empty row range");
        if matrix[pivot][column].abs() <= tolerance {
            return None;
        }
        matrix.swap(column, pivot);
        rhs.swap(column, pivot);
        let divisor = matrix[column][column];
        for row in column + 1..3 {
            let factor = matrix[row][column] / divisor;
            if factor == 0.0 {
                continue;
            }
            let (before, after) = matrix.split_at_mut(row);
            let pivot = &before[column][column..];
            for (value, pivot) in after[0][column..].iter_mut().zip(pivot) {
                *value -= factor * pivot;
            }
            rhs[row] -= factor * rhs[column];
        }
    }

    let mut out = [0.0f64; 3];
    for row in (0..3).rev() {
        let mut value = rhs[row];
        for k in row + 1..3 {
            value -= matrix[row][k] * out[k];
        }
        out[row] = value / matrix[row][row];
    }
    out.iter().all(|value| value.is_finite()).then_some(out)
}
