# m7-print-mesh: solid groups and components as printable triangles

Implement `app/crates/modeler-mesh/src/print_mesh.rs` (`print_mesh` and `SkipReason`'s
`Display`; types and skeleton are on main). Change only that file. Acceptance tests:
`app/crates/modeler-mesh/tests/print_mesh.rs` and `tests/print_mesh_props.rs`, with the
fixtures in `tests/print_common/mod.rs`.

This is pure data: no file writing. The STL and 3MF writers (`m7-export-stl`,
`m7-export-3mf`) and the export dialog take the result. Kernel units are meters; the output
is millimeters.

## Which objects

An *object* is a group or component instance. `print_mesh(model, scope, options)` walks the
instances of the model root in ascending id order (`PrintScope::Instances(ids)` keeps only
those root instances whose id is in `ids`; unknown ids are ignored; `Model` keeps all).
Geometry drawn directly in the root, outside any instance, is not an object and is ignored.

For one instance `I` with definition body `D`, placed in the world by `world` (the product of
the instance transforms from the root down to `I`):

1. **Name.** The instance name; if empty, the definition's name; if that is empty,
   `"Object"`. An instance nested inside another gets `"<parent's full name>/<its name>"`.
2. **No faces of its own.** If `D` has no faces and no nested instances, `I` is skipped with
   `SkipReason::NoGeometry`. If it has no faces but has nested instances it is only a
   container: it is neither exported nor skipped.
3. **Faces of its own.** Run `solid::check_geometry` on `D`'s geometry (not `check_body`:
   nested instances don't make the faces non-solid). If `report.is_solid()`, `I` becomes a
   `PrintObject`; otherwise it is skipped with `SkipReason::NotSolid(report)`. Reversed faces
   don't prevent export, they are turned (below). Run the check once per definition, not per
   instance.
4. **Nested instances** of `D` are then walked the same way, in ascending id order, with
   `world * nested.transform`, whatever happened to `I` (a skipped parent doesn't hide its
   solid children). So an instance with faces and nested instances gives its own object
   first, then its children's. Stop descending at depth 64.

`PrintMesh::objects` and `skipped` are each in walk order (parent before children).

`Display` of `SkipReason`: `NoGeometry` is `has no faces`. `NotSolid` is `is not a solid: `
followed by the non-empty counts joined by `, `, in this order: open edges, loose edges,
shared edges, internal faces, each as `1 open edge` / `4 open edges` (singular for 1); when
none of those is non-zero (no closed surface at all) the list is `no closed surface`.

## Triangles

For every face of `D` (skip a face whose outer loop has a zero normal,
`geom::polygon_normal`):

1. **Corner lists.** Each use of the outer loop and of each hole is one *side*: its start
   vertex followed by the extra points of its edge (see Curves), in the direction of the use.
   The loop's point list is the sides' points in order. Without curves a side is just its
   start vertex.
2. **Flat faces.** If every point is within 1e-9 m of the plane through the first point with
   normal `n = polygon_normal(unrefined outer loop points)`, triangulate like the viewport
   does: `geom::to_plane_2d` for the outer loop and the holes, then `tess::triangulate`, and
   take the points by index (outer, then holes). This is the only path for a face without
   curves, and for caps, which stay flat when their circle is split.
3. **Walls** (a face whose split points leave its plane, e.g. the side of a cylinder). A
   flat triangulation of such a face collapses the extra points onto a line and cuts through
   the wall, so use these instead, only for faces without holes:
   - *Four sides, the sides before and after the first (or second) one not split*: a ladder
     between side `s` (points `A_0..A_p`, then the next side's start as `A_p`) and the
     opposite side read backwards (`B_0..B_q`, ending at the corner next to `A_p`'s). Start
     at `i = j = 0`; while `i < p` or `j < q`: if `i < p` and (`j == q` or
     `(i + 1) * q <= (j + 1) * p`) emit `[A_i, A_{i+1}, B_j]` and `i += 1`, else emit
     `[A_i, B_{j+1}, B_j]` and `j += 1`. Use `s = 0` (sides 0 and 2) when sides 1 and 3 aren't
     split, else `s = 1` when sides 0 and 2 aren't.
   - *Three sides, exactly one split*: a fan from the opposite corner: `[P_i, P_{i+1}, apex]`
     along the split side's points ending at its far corner.
   - Anything else: the flat way (step 2), even though it may cut the wall.
4. All triangles above run counter-clockwise seen from the face's front.
5. **Orientation.** Turn the triangle (swap its last two corners) when the face is in
   `report.reversed_faces`, and again when `world`'s linear part has a negative determinant
   (a mirror). Then the triangle faces away from the material for every instance.
6. **Place.** Each corner: `world.transform_point3(p) * 1000.0` (in that order, so an
   identity transform keeps a vertex's position times 1000 bit for bit).

Points are never merged or moved after this, so the faces of a closed solid share their
points exactly: every edge of the result is used by two triangles, once in each direction
(compared bit for bit).

## Curves

A *step* is two consecutive vertices of a kernel `Curve` (`ctx.curves()`) that are joined by
a single edge (`ctx.edge_between`). A step whose chord is split by other geometry, so that
there is no such edge, is left as it is. The first curve (lowest id) that has an edge as a
step owns it. Every other edge is not split.

For a step from vertex `a` to vertex `b` (in curve order) of curve `c`:

- `θa` is the angle of `a` on the circle, `atan2((a - center)·y_axis, (a - center)·x_axis)`;
  `Δ = θb - θa`, plus 2π until `Δ > 0`.
- The step is split into `k` equal angles, adding the `k - 1` points `c.point_at(θa + Δ j / k)`,
  `j = 1..k-1`, computed in the definition's coordinates and placed like every other point.
  Both faces on the edge get the same points (in opposite orders for opposite directions).
- `k` is the smallest value `>= 1` with sagitta `radius * (1 - cos(Δ / (2k))) <= tol`, at most
  4096. Curves are only ever refined: a step that already meets `tol` stays one edge, and
  points are never removed. The arc through the split points is then within `tol` of the
  true circle, so the printed chords stay within `tol`.
- `tol` is the world tolerance in meters, `chord_tolerance_mm / 1000`, divided by the
  largest column length of `world`'s linear part (so a stretched or scaled instance keeps the
  tolerance in the printed size; stretching only makes the result finer than needed). A
  `chord_tolerance_mm` that is NaN or below `MIN_CHORD_TOL * 1000` (the kernel's 1 µm) is read
  as 1 µm; +infinity means never split.

A smaller tolerance never gives fewer triangles.

## Colors

`PrintObject::color` is the front material (`Body::face_materials`) that covers the most world
area, or `None` when no triangle has a material.

- A face's material is its front material, or its back material when the face is in
  `reversed_faces` (the side that ends up outside); when that is `None`, the material of the
  closest instance that has one: the instance's own `material`, else the one inherited from the
  instances it is nested in (an inner instance's material wins).
- Area is the sum of the world triangle areas (after `world`, in any unit). Materials whose
  area is within a relative 1e-9 of the largest are tied; the lowest `MaterialId` wins.
- The color is `Material::color` as stored (`[r, g, b, a]`, sRGB, straight alpha).

## Notes

- Several instances of one definition are separate objects with their own transform.
- Volume: for objects without curves the signed volume of the triangles equals
  `report.volume * 1e9` (to 1e-9 relative).
- The throwaway reference implementation passes every test and is about 330 lines; a
  face-by-face walk with a few small helpers (`ladder`, a step-points function) is enough. No
  new dependencies.
