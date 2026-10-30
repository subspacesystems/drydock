# m7-clearance: find where two parts are too close

Printed parts that must fit together or move against each other need a gap. A printer can't
hold a gap much under a few tenths of a millimeter, so parts that come closer than the
clearance (0.2 mm by default) fuse. This task is the geometry behind the fit check: which
triangles of two parts are within the clearance of the other part, and a scene copy with
those triangles tinted. The menu, the clearance field and picking the two parts are
`m7-clearance-ui`.

Skeleton on main: `app/crates/modeler-dae/src/clearance.rs` (re-exported from the crate
root). File to change: that one. Acceptance tests: `app/crates/modeler-dae/tests/clearance.rs`.

All lengths are meters (the scene's unit); 0.2 mm is `0.0002`.

## `triangle_distance(a, b)`

The shortest distance between any point of triangle `a` and any point of triangle `b`
(closed triangles, not only their corners).

- 0 when they touch, share a corner or an edge, cross each other, overlap in a plane, or one
  lies inside the other in a plane.
- A triangle whose corners are collinear or equal counts as the segment or point they make.
- Compute it as the smallest of: the six corner-to-other-triangle distances, the nine
  edge-to-edge distances, and 0 if an edge of one pierces the other.
- Symmetric. Not meaningful for non-finite corners (the caller filters them).

## `close_triangles(a, b, clearance) -> FitReport`

`a` and `b` are the triangles of two parts. The report lists, ascending, the indices `i` in
`a` for which some triangle of `b` has `triangle_distance < clearance`, and likewise the
indices in `b`.

- The comparison is strict: a gap of exactly the clearance is not flagged. Touching and
  overlapping parts (distance 0) are flagged.
- `clearance` that is not greater than 0 (or NaN), or an empty list: empty report.
- A triangle with a non-finite corner is never flagged and never makes another one flagged.
- **No pair-by-pair walk.** Parts have tens of thousands of triangles. Use a bounding volume
  hierarchy (or a grid) over each list and only measure pairs whose boxes are within the
  clearance of each other; a triangle that has one close partner needs no further search.
  The acceptance tests compare 2 x 45 000 triangles and give it 10 s.
- The result must equal the pair-by-pair definition exactly (a test checks this on random
  soups).

## `instance_triangles(model, body, instance, outer)`

The triangles of one part, ready for `close_triangles`.

- `instance` is an instance in `body` (the root or a definition). Take its definition's
  faces and, recursively, the faces of the instances nested in it (depth limit 64, as
  `kernel_scene` has), each placed by the instance's transform (and the nested instances'
  transforms), then by `outer`.
- Triangulate every face the way `kernel_scene` does (`polygon_normal`, `to_plane_2d`,
  `tess::triangulate`, holes included; skip faces with no normal or non-finite points), so
  the triangles are the ones drawn. The order is the faces' order, then the nested
  instances'.
- `instance` not found in `body`, or `body` missing: empty.
- `outer` lets the app place a part that is inside an open group in world space (the edit
  path's transform).

## `tint_fit(scene, flagged)`

Returns a new scene; `scene` is not changed. A scene triangle **matches** a flagged triangle
when each of its three corners is within `MATCH_TOLERANCE_M` (1e-5) of a corner of the
flagged one, with the corners paired one to one, in any rotation and either winding (the
scene's corners are `f32`). Tests walk every batch's triangles in order, as
`tint_overhangs` does (`indices` in threes; a triangle with an index out of range never
matches).

- Walk each flagged triangle's neighbors through a hash grid (cell about 1 mm) or similar so
  that a scene of 45 000 triangles with 22 500 flagged takes well under 10 s. Flagged
  triangles with non-finite corners are ignored.
- Nothing matches: the result is an exact copy of `scene`.
- Otherwise it is built exactly as `tint_overhangs` builds its result: `materials` plus
  `Material { id: "fit", name: "Fit", color: FIT_COLOR, texture: None, opacity: 1.0,
  unlit: false }`; each source batch keeps its `material`, `positions`, `normals`,
  `texcoords` and its non-matching triangles' `indices`; batches left empty are dropped;
  one batch goes last with `material: Some(scene.materials.len())` and, for each matching
  triangle in walk order, its three vertices (positions and normals copied, `texcoords`
  empty, `indices` `0, 1, 2, ...`). `lines` and `bounds` are copied unchanged.
- A triangle is tinted once however often it is flagged; two identical scene triangles are
  both tinted.

## Not in scope

The View/Tools menu item, the clearance field in mm, picking the two parts, uploading the
tinted scene (`m7-clearance-ui`); moving faces to open the gap (`m7-clearance-offset`);
distances between a part and itself.
