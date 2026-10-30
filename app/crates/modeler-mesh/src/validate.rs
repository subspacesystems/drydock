// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Own mesh validator (`docs/specs/m7-roundtrip.md`): is a triangle list something a slicer
//! will print? Independent of the kernel's solid check, so the round-trip tests can tell
//! whether an exported or re-imported mesh is sound.

use crate::print_mesh::Triangle;

/// What `validate` found. Counts are of triangles or of undirected edges.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshReport {
    /// Triangles given.
    pub triangles: usize,
    /// Distinct points after welding.
    pub vertices: usize,
    /// Triangles with two corners welded together, or a sliver whose area is under 1e-9 of
    /// the square of its longest edge. They take no part in the edge counts or the volume.
    pub degenerate_triangles: usize,
    /// Edges used by exactly one (non-degenerate) triangle.
    pub open_edges: usize,
    /// Edges used by three or more triangles.
    pub non_manifold_edges: usize,
    /// Edges used by exactly two triangles that run the same way along it (one of the two
    /// is wound the wrong way round).
    pub inconsistent_edges: usize,
    /// Signed volume of the non-degenerate triangles, cubic millimeters like the input;
    /// positive when the surface faces outward.
    pub volume: f64,
}

impl MeshReport {
    /// At least one triangle, none of the counts above non-zero, and `volume > 0`.
    pub fn is_valid(&self) -> bool {
        unimplemented!("m7-roundtrip")
    }
}

/// Checks `triangles` after welding points: two points are the same when they round to the
/// same multiple of `weld_mm` in all three coordinates (`(c / weld_mm).round()`). The
/// volume is computed from the original coordinates, not the welded ones.
pub fn validate(triangles: &[Triangle], weld_mm: f64) -> MeshReport {
    let _ = (triangles, weld_mm);
    unimplemented!("m7-roundtrip")
}
