# m1-geom: tolerance-aware geometry math

Milestone M1. Crate `app/crates/modeler-kernel`, file `src/geom.rs`. Implement every `unimplemented!("m1-geom")`. Don't change public signatures, types or constants; add private helpers freely. Dependencies: `glam` only. Every function is total: no panics on degenerate input, return the documented "nothing" value instead.

World is meters, Z up, right-handed. `LINEAR_TOL` (1e-5 m) is the usual `tol`; callers may pass other values.

## Planes and polygons (3D)
- `area_vector(pts)`: half the sum of cross products around the closed polygon, so its length is the area and its direction the normal (counter-clockwise seen from the normal side). Fewer than 3 points: zero. Take the sum relative to the first vertex so large coordinates (1e5 m) don't lose precision.
- `polygon_normal(pts)`: the unit vector along `area_vector`, or `DVec3::ZERO` if its length is below 1e-12.
- `fit_plane(pts)`: `Plane { point: average of the vertices, normal: polygon_normal }`; `None` if the normal is zero.
- `Plane::signed_distance(p)`: `normal . (p - point)`.
- `max_plane_deviation(pts, plane)`: the largest absolute signed distance (0 for no points).
- `is_planar(pts, tol)`: a fit plane exists and every point is within `tol` of it. A triangle with area is always planar; fewer than 3 points or zero area is not.

## Segments (3D)
- `point_segment_distance(p, a, b)`: shortest distance to the segment; `a == b` is a point.
- `segment_intersection(a0, a1, b0, b1, tol)`:
  - Either segment shorter than `tol`: `SegmentHit::None`.
  - Not collinear (b's endpoints not both within `tol` of the line through a): find the closest pair of points between the two segments. If they are within `tol`, `Point { ta, tb }` with the parameters of those points (0..=1 along `a0->a1` and `b0->b1`), else `None`. This covers crossings, T-junctions, shared endpoints and skew segments that nearly touch. Parallel segments that fail the collinear test are handled the same way (their closest approach is then at an end). The closest pair must consider the segment ends, not just the infinite lines (lines that cross past the end of a segment are not a hit).
  - Collinear: overlap interval of the two segments along a. Longer than `tol`: `Overlap { a_from, a_to, b_from, b_to }` with `a_from < a_to` and `b_from`/`b_to` the parameters on b of the points at `a_from`/`a_to` (so `b_from > b_to` when b runs the opposite way). Between -`tol` and `tol` long (end to end contact): `Point`. A gap bigger than `tol`: `None`.
  - Snapping: any returned parameter whose point is within `tol` (as a length along that segment) of an end is exactly `0.0` or `1.0`. The kernel relies on this to tell "at an endpoint" from "in the middle" without its own tolerance.
  - Symmetric: swapping `a` and `b` swaps `ta`/`tb` (and the overlap parameter pairs).

## Planar 2D helpers
- `plane_basis(normal)`: deterministic orthonormal `(u, v)` with `u x v == normal`. Suggested: helper axis X if `|normal.x| < 0.9` else Y, `u = helper x normal` normalised, `v = normal x u`.
- `to_plane_2d(pts, normal)`: `(p . u, p . v)` for each point, with the basis above (measured from the world origin; areas and shapes are preserved).
- `signed_area_2d(poly)`: shoelace; positive for counter-clockwise; 0 for fewer than 3 points.
- `point_in_polygon_2d(p, poly, tol)`: `Boundary` if `p` is within `tol` of any edge or vertex (checked first), else `Inside` or `Outside` by the even-odd rule. Works for concave polygons and either winding. Fewer than 3 vertices: `Outside`.
- `polygon_is_simple_2d(poly, tol)`: false for fewer than 3 vertices, any two consecutive vertices within `tol` (including last to first), `|area| <= tol^2`, or any pair of edges that meet other than consecutive edges meeting at their shared vertex only (crossing, touching, pinched vertex, overlapping spike). A collinear extra vertex in the middle of an edge is fine. Suggested: lift to z = 0 and use `segment_intersection`; for consecutive edges accept only a `Point` hit with exactly `ta == 1.0, tb == 0.0` (and the wrap-around pair with `ta == 0.0, tb == 1.0`).

## Tests (fixed; don't edit)
`tests/geom.rs` (examples for every function above) and `tests/geom_props.rs` (proptest: rigid motions keep polygons planar with a rotated normal, lifting a vertex breaks planarity, plane-2D area equals 3D area, `plane_basis` is a rotation, constructed crossings are found with the right parameters and are symmetric, lifted crossings are not hits, collinear overlaps agree at their end points).
