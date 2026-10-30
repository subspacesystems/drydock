// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Triangulation of planar faces for drawing.
//!
//! Behaviour: `docs/specs/m1-kernel-scene.md`.

use glam::{DVec2, DVec3};

use crate::geom::{
    Containment, SegmentHit, point_in_polygon_2d, segment_intersection, signed_area_2d,
};

/// Triangles covering the polygon `outer` minus the `holes`. A triangle is three indices
/// into the points listed in order: `outer`, then each hole in turn. Every triangle runs
/// counter-clockwise (or is flat); no new points are added.
pub fn triangulate(outer: &[DVec2], holes: &[Vec<DVec2>]) -> Vec<[usize; 3]> {
    if outer.len() < 3 {
        return Vec::new();
    }

    // Points are numbered outer first, then every hole in order (short holes included,
    // even though they take no part in the triangulation).
    let mut points: Vec<DVec2> =
        Vec::with_capacity(outer.len() + holes.iter().map(Vec::len).sum::<usize>());
    points.extend_from_slice(outer);
    let mut hole_ranges = Vec::new();
    for hole in holes {
        let base = points.len();
        points.extend_from_slice(hole);
        if hole.len() >= 3 {
            hole_ranges.push((base, hole.len()));
        }
    }

    let (lo, hi) = bounding_box(outer);
    let diagonal = lo.distance(hi);
    let area_eps = 1e-12 * diagonal * diagonal;
    let length_eps = 1e-12 * diagonal;

    // The merged polygon runs counter-clockwise; holes run clockwise.
    let mut polygon: Vec<usize> = (0..outer.len()).collect();
    if signed_area_2d(outer) < 0.0 {
        polygon.reverse();
    }

    let mut hole_indices: Vec<Vec<usize>> = Vec::with_capacity(hole_ranges.len());
    for (base, len) in hole_ranges {
        let mut indices: Vec<usize> = (base..base + len).collect();
        let hole_points: Vec<DVec2> = indices.iter().map(|&i| points[i]).collect();
        if signed_area_2d(&hole_points) > 0.0 {
            indices.reverse();
        }
        hole_indices.push(indices);
    }

    let hole_polygons: Vec<Vec<DVec2>> = hole_indices
        .iter()
        .map(|indices| indices.iter().map(|&i| points[i]).collect())
        .collect();

    // Join each hole to the polygon with a bridge. `virtual_vertex[i]` marks the extra
    // copies of the bridge end points, so new bridges attach to the original vertices.
    let mut virtual_vertex = vec![false; polygon.len()];
    for hole in &hole_indices {
        let Some((at, start)) = find_bridge(
            &polygon,
            &virtual_vertex,
            hole,
            &points,
            outer,
            &hole_polygons,
            length_eps,
        ) else {
            continue;
        };

        let anchor = polygon[at];
        let mut inserted: Vec<(usize, bool)> = Vec::with_capacity(hole.len() + 2);
        for k in 0..hole.len() {
            inserted.push((hole[(start + k) % hole.len()], false));
        }
        inserted.push((hole[start], true));
        inserted.push((anchor, true));

        let mut merged = Vec::with_capacity(polygon.len() + inserted.len());
        let mut merged_virtual = Vec::with_capacity(merged.len());
        merged.extend_from_slice(&polygon[..=at]);
        merged_virtual.extend_from_slice(&virtual_vertex[..=at]);
        for (index, virtual_copy) in inserted {
            merged.push(index);
            merged_virtual.push(virtual_copy);
        }
        merged.extend_from_slice(&polygon[at + 1..]);
        merged_virtual.extend_from_slice(&virtual_vertex[at + 1..]);
        polygon = merged;
        virtual_vertex = merged_virtual;
    }

    ear_clip(polygon, &points, area_eps)
}

fn bounding_box(points: &[DVec2]) -> (DVec2, DVec2) {
    let mut lo = DVec2::splat(f64::INFINITY);
    let mut hi = DVec2::splat(f64::NEG_INFINITY);
    for &p in points {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    (lo, hi)
}

fn find_bridge(
    polygon: &[usize],
    virtual_vertex: &[bool],
    hole: &[usize],
    points: &[DVec2],
    outer: &[DVec2],
    holes: &[Vec<DVec2>],
    length_eps: f64,
) -> Option<(usize, usize)> {
    let mut candidates: Vec<(f64, usize, usize)> = Vec::with_capacity(polygon.len() * hole.len());
    for (position, &anchor) in polygon.iter().enumerate() {
        if virtual_vertex[position] {
            continue;
        }
        for (start, &target) in hole.iter().enumerate() {
            let distance = points[anchor].distance_squared(points[target]);
            candidates.push((distance, position, start));
        }
    }
    candidates.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });

    for (_, position, start) in candidates {
        if bridge_is_visible(
            polygon, hole, position, start, points, outer, holes, length_eps,
        ) {
            return Some((position, start));
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn bridge_is_visible(
    polygon: &[usize],
    hole: &[usize],
    position: usize,
    start: usize,
    points: &[DVec2],
    outer: &[DVec2],
    holes: &[Vec<DVec2>],
    length_eps: f64,
) -> bool {
    let a = points[polygon[position]];
    let b = points[hole[start]];
    if !a.is_finite() || !b.is_finite() || a.distance(b) <= length_eps {
        return false;
    }

    // The bridge must leave the anchor through the polygon's interior wedge.
    let before = points[polygon[(position + polygon.len() - 1) % polygon.len()]];
    let after = points[polygon[(position + 1) % polygon.len()]];
    let incoming = a - before;
    let outgoing = after - a;
    let to_target = b - a;
    let at_vertex = orientation(before, a, after);
    let left_incoming = incoming.perp_dot(to_target);
    let left_outgoing = outgoing.perp_dot(to_target);
    let in_wedge = if at_vertex >= 0.0 {
        left_incoming >= -length_eps && left_outgoing >= -length_eps
    } else {
        left_incoming >= -length_eps || left_outgoing >= -length_eps
    };
    if !in_wedge {
        return false;
    }

    let clears = |c: DVec2, d: DVec2| {
        matches!(
            segment_intersection(
                DVec3::new(a.x, a.y, 0.0),
                DVec3::new(b.x, b.y, 0.0),
                DVec3::new(c.x, c.y, 0.0),
                DVec3::new(d.x, d.y, 0.0),
                length_eps,
            ),
            SegmentHit::None
        ) || intersects_only_at_ends(a, b, c, d, length_eps)
    };

    for k in 0..polygon.len() {
        let c = points[polygon[k]];
        let d = points[polygon[(k + 1) % polygon.len()]];
        if !clears(c, d) {
            return false;
        }
    }
    for other in holes {
        for k in 0..other.len() {
            let c = other[k];
            let d = other[(k + 1) % other.len()];
            if !clears(c, d) {
                return false;
            }
        }
    }

    let middle = (a + b) * 0.5;
    if point_in_polygon_2d(middle, outer, length_eps) != Containment::Inside {
        return false;
    }
    for hole in holes {
        if point_in_polygon_2d(middle, hole, length_eps) != Containment::Outside {
            return false;
        }
    }
    true
}

/// Whether segments `a`-`b` and `c`-`d` meet only at one of the bridge's end points.
fn intersects_only_at_ends(a: DVec2, b: DVec2, c: DVec2, d: DVec2, eps: f64) -> bool {
    let hit = segment_intersection(
        DVec3::new(a.x, a.y, 0.0),
        DVec3::new(b.x, b.y, 0.0),
        DVec3::new(c.x, c.y, 0.0),
        DVec3::new(d.x, d.y, 0.0),
        eps,
    );
    match hit {
        SegmentHit::None => true,
        SegmentHit::Point { ta, .. } => {
            let point = a + (b - a) * ta;
            point.distance(a) <= eps * 4.0 || point.distance(b) <= eps * 4.0
        }
        SegmentHit::Overlap { .. } => false,
    }
}

fn orientation(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    (b - a).perp_dot(c - a)
}

fn ear_clip(polygon: Vec<usize>, points: &[DVec2], area_eps: f64) -> Vec<[usize; 3]> {
    let n = polygon.len();
    if n < 3 {
        return Vec::new();
    }

    let mut previous: Vec<usize> = (0..n).map(|i| (i + n - 1) % n).collect();
    let mut next: Vec<usize> = (0..n).map(|i| (i + 1) % n).collect();
    let mut active = vec![true; n];
    let mut remaining = n;
    let mut triangles = Vec::with_capacity(n - 2);
    let mut current = 0usize;
    let mut misses = 0usize;

    while remaining > 3 {
        let p = previous[current];
        let q = next[current];
        if active[current]
            && p != current
            && q != current
            && let Some(triangle) = ear_at(
                p, current, q, &active, &polygon, points, area_eps, &previous, &next,
            )
        {
            triangles.push(triangle);
            next[p] = q;
            previous[q] = p;
            active[current] = false;
            remaining -= 1;
            current = q;
            misses = 0;
            continue;
        }

        current = next[current];
        misses += 1;
        if misses >= remaining {
            // Degenerate or invalid input: make progress by clipping a vertex anyway,
            // preferring one that leaves a real triangle so flat ears don't hide the rest.
            let mut chosen = None;
            let mut flat = None;
            for i in 0..n {
                if active[i] {
                    let p = previous[i];
                    let q = next[i];
                    let (a, b, c) = (polygon[p], polygon[i], polygon[q]);
                    if a != b && b != c && a != c {
                        let cross = orientation(points[a], points[b], points[c]);
                        if cross.abs() > area_eps {
                            chosen = Some((i, p, q, a, b, c));
                            break;
                        }
                        if flat.is_none() {
                            flat = Some((i, p, q, a, b, c));
                        }
                    }
                }
            }
            let Some((i, p, q, a, b, c)) = chosen.or(flat) else {
                break;
            };
            let cross = orientation(points[a], points[b], points[c]);
            triangles.push(if cross >= 0.0 { [a, b, c] } else { [a, c, b] });
            next[p] = q;
            previous[q] = p;
            active[i] = false;
            remaining -= 1;
            current = q;
            misses = 0;
        }
    }

    let mut ring = Vec::with_capacity(3);
    let mut i = (0..n).find(|&i| active[i]).unwrap_or(0);
    for _ in 0..3 {
        if !active[i] {
            break;
        }
        ring.push(polygon[i]);
        i = next[i];
    }
    if ring.len() == 3 {
        let (a, b, c) = (ring[0], ring[1], ring[2]);
        let cross = orientation(points[a], points[b], points[c]);
        if cross.abs() > area_eps {
            triangles.push(if cross >= 0.0 { [a, b, c] } else { [a, c, b] });
        }
    }

    triangles
}

#[allow(clippy::too_many_arguments)]
fn ear_at(
    p: usize,
    current: usize,
    q: usize,
    active: &[bool],
    polygon: &[usize],
    points: &[DVec2],
    area_eps: f64,
    previous: &[usize],
    next: &[usize],
) -> Option<[usize; 3]> {
    let a = polygon[p];
    let b = polygon[current];
    let c = polygon[q];
    if a == b || b == c || a == c {
        return None;
    }

    let (pa, pb, pc) = (points[a], points[b], points[c]);
    let cross = orientation(pa, pb, pc);
    if cross <= area_eps {
        return None;
    }
    if contains_point(
        pa, pb, pc, active, polygon, points, area_eps, a, b, c, previous, next,
    ) {
        return None;
    }
    // With holes joined by bridges, an ear must not reach across a bridge edge.
    for (i, &is_active) in active.iter().enumerate() {
        if !is_active {
            continue;
        }
        let j = next[i];
        if !active[j] || i == p || i == q || j == p || j == q {
            continue;
        }
        if !intersects_only_at_ends(pa, pc, points[polygon[i]], points[polygon[j]], area_eps) {
            return None;
        }
    }

    Some([a, b, c])
}

#[allow(clippy::too_many_arguments)]
fn contains_point(
    a: DVec2,
    b: DVec2,
    c: DVec2,
    active: &[bool],
    polygon: &[usize],
    points: &[DVec2],
    area_eps: f64,
    ia: usize,
    ib: usize,
    ic: usize,
    previous: &[usize],
    next: &[usize],
) -> bool {
    for (i, &is_active) in active.iter().enumerate() {
        if !is_active {
            continue;
        }
        // Only reflex vertices can block an ear; convex ones may lie on its boundary.
        let p = previous[i];
        let n = next[i];
        if orientation(points[polygon[p]], points[polygon[i]], points[polygon[n]]) >= -area_eps {
            continue;
        }
        let index = polygon[i];
        if index == ia || index == ib || index == ic {
            continue;
        }
        let p = points[index];
        // Copies of a triangle corner are not obstacles.
        if p == a || p == b || p == c {
            continue;
        }
        if orientation(a, b, p) >= -area_eps
            && orientation(b, c, p) >= -area_eps
            && orientation(c, a, p) >= -area_eps
        {
            return true;
        }
    }
    false
}
