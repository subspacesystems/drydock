// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Own mesh validator (`docs/specs/m7-roundtrip.md`): is a triangle list something a slicer
//! will print? Independent of the kernel's solid check, so the round-trip tests can tell
//! whether an exported or re-imported mesh is sound.

use std::collections::{HashMap, HashSet};

use glam::DVec3;

use crate::print_mesh::Triangle;

type Key = (i64, i64, i64);

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
        self.triangles > 0
            && self.degenerate_triangles == 0
            && self.open_edges == 0
            && self.non_manifold_edges == 0
            && self.inconsistent_edges == 0
            && self.volume > 0.0
    }
}

/// Checks `triangles` after welding points: two points are the same when they round to the
/// same multiple of `weld_mm` in all three coordinates (`(c / weld_mm).round()`). The
/// volume is computed from the original coordinates, not the welded ones.
pub fn validate(triangles: &[Triangle], weld_mm: f64) -> MeshReport {
    let key = |c: &DVec3| -> Key {
        (
            (c.x / weld_mm).round() as i64,
            (c.y / weld_mm).round() as i64,
            (c.z / weld_mm).round() as i64,
        )
    };
    let mut vertices: HashSet<Key> = HashSet::new();
    let mut edges: HashMap<(Key, Key), (usize, usize)> = HashMap::new();
    let mut degenerate_triangles = 0usize;
    let mut volume = 0.0f64;

    for t in triangles {
        let k = [key(&t[0]), key(&t[1]), key(&t[2])];
        for x in &k {
            vertices.insert(*x);
        }

        let (a, b, c) = (t[0], t[1], t[2]);
        let mut degenerate = k[0] == k[1] || k[0] == k[2] || k[1] == k[2];
        if !degenerate {
            let twice_area = (b - a).cross(c - a).length();
            let longest = (b - a)
                .length_squared()
                .max((c - a).length_squared())
                .max((c - b).length_squared());
            degenerate = twice_area * 0.5 < 1e-9 * longest;
        }
        if degenerate {
            degenerate_triangles += 1;
            continue;
        }

        let mut add_edge = |i: usize, j: usize| {
            let (lo, hi, low_to_high) = if k[i] < k[j] {
                (k[i], k[j], true)
            } else {
                (k[j], k[i], false)
            };
            let slot = edges.entry((lo, hi)).or_default();
            if low_to_high {
                slot.0 += 1;
            } else {
                slot.1 += 1;
            }
        };
        add_edge(0, 1);
        add_edge(1, 2);
        add_edge(2, 0);
        volume += a.dot(b.cross(c)) / 6.0;
    }

    let mut report = MeshReport {
        triangles: triangles.len(),
        vertices: vertices.len(),
        degenerate_triangles,
        volume,
        ..Default::default()
    };
    for (lo, hi) in edges.values() {
        let total = lo + hi;
        if total == 1 {
            report.open_edges += 1;
        } else if total >= 3 {
            report.non_manifold_edges += 1;
        } else if *lo != *hi {
            report.inconsistent_edges += 1;
        }
    }
    report
}
