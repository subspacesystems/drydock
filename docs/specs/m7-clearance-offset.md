# m7-clearance-offset: grow or shrink a flat-faced solid by a clearance (kernel)

Implement `app/crates/modeler-kernel/src/offset.rs` (skeleton on main: `OffsetError`,
`OffsetReport`, the constants `SAME_PLANE_DOT`, `RANK_TOL`, `RESIDUAL_REL`, and
`offset_positions`, `offset_body`, `offset_bodies` with `unimplemented!()` bodies). Change
only that file. Acceptance tests: `app/crates/modeler-kernel/tests/offset.rs` and
`tests/offset_props.rs`, which share the helpers in `tests/offset_common/mod.rs`.

Two printed parts that must fit together (a lid and a box, a peg and a hole) need a gap of
about 0.2 mm between them. The fit check (`m7-clearance`) shows where the gap is too small;
this task makes it bigger. **Grow** moves every face of a part out of the material by a
distance `d`; **shrink** moves every face into the material by `d`. A box 10 x 20 x 30 mm
grown by 0.2 mm becomes 10.4 x 20.4 x 30.4 mm with the same center. The menu items come
later (`m7-clearance-offset-ui`); this task is the geometry only.

Only parts with flat faces and no curves are handled. Anything this can't do it refuses with
a reason and leaves the part exactly as it was.

## How it works

Every face stays the same face: same id, same loops, same edges and vertices. Only vertex
positions change. Each face's plane moves `distance` along the face's **outward normal**, and
each vertex moves to where the moved planes of its faces meet. Positive `distance` grows,
negative shrinks.

A body that passes the solid check (`check_body(...).is_solid()`) holds nothing but closed
shells: no loose edges, no stray geometry, no nested groups or components. Moving its
vertices this way keeps every face flat, so there is nothing to split or merge and no other
geometry for the moved faces to stick to. The checks below refuse every case where the move
would break the solid instead.

## `offset_positions(ctx, distance)`

Computes where each vertex goes and runs every check. `ctx` is not changed. Returns a map
from every vertex id of `ctx` to its new position, or the first error in this order:

1. **Distance.** `distance` is zero (either sign), NaN or infinite: `BadDistance`.
2. **Solid.** `report = check_geometry(ctx)`; when `!report.is_solid()`:
   `NotSolid(Box::new(report))`.
3. **Curves.** `ctx.curve_count() > 0`: `Curved(ctx.curve_count())`. (A circle's segments
   would offset, but its curve record wouldn't describe the result; that's for a later task.)
4. **Outward normals.** For each face, its normal is `ctx.face_plane(f)`'s normal, negated
   when `f` is in `report.reversed_faces`. Faces whose fronts point the wrong way still move
   out of the material, and they keep their loops as they are.
5. **Vertex moves.** For each vertex `v`, in ascending id order:
   - Its faces are the faces of its edges (`edges_of_vertex`, then `faces_of_edge`), in
     ascending face id.
   - Keep their outward normals in that order, dropping a normal whose dot product with an
     already kept one is above `SAME_PLANE_DOT` (coplanar faces, such as a face split in
     two, count once).
   - Solve for the smallest move `m` with `n · m = distance` for the kept normals:
     - Basis: `b1` is the first kept normal. `b2` is the kept normal with the largest
       `|b1 × n|`, used only when that is above `RANK_TOL`. If there is a `b2`, `b3` is the
       kept normal with the largest `|(b1 × b2) · n|`, used only when that is above
       `RANK_TOL`. Ties go to the earlier normal.
     - One basis normal: `m = distance * b1`. Two: `m = a (b1 + b2)` with
       `a = distance / (1 + b1 · b2)`. Three: `m` solves the 3 x 3 system whose rows are
       `b1`, `b2`, `b3` and whose right side is `distance` in every row.
   - Check every kept normal (not only the basis): when any has
     `|n · m - distance| > RESIDUAL_REL * |distance|`, the planes don't meet in one point
     and `v` is unsupported. Otherwise its new position is the old one plus `m`.

   Three faces at a corner always meet in one point. Where four or more meet (the apex of a
   pyramid, an octahedron's corners) they meet only when the shape allows it: a square
   pyramid's apex moves straight up, a rectangular pyramid's apex can't follow all four
   faces and is unsupported. A vertex where only two distinct planes meet (on an edge that
   another edge splits) moves along both normals. A vertex shared by two shells that only
   touch there gets normals facing both ways and is unsupported.

   A vertex with no faces, or with a face that has no plane, is also unsupported. When any
   vertex is unsupported: `Unsupported(ids)`, every such vertex in ascending order.
6. **Collapse.** Compare each edge and face before and after the move:
   - An edge collapses when its new vector (end 1 minus end 0) is no longer than
     `geom::LINEAR_TOL`, or its dot product with the old vector is zero or less.
   - A face collapses when, for any of its loops (outer or inner), the new
     `geom::area_vector` of the loop's points is no longer than `1e-12`, or its dot product
     with the old area vector is zero or less.
   When anything collapses: `Collapse { edges, faces }`, both in ascending id order. This is
   shrinking a part past half its thickness, or growing until a hole or cavity closes.
   (A loop that reverses along both of its directions has only turned half a turn, so its
   area vector keeps its direction: a square hole that closes lists its rim edges and its
   walls, not the top and bottom faces it passes through.)
7. **Self-intersection.** Build the moved context (a clone of `ctx` with every vertex set
   to its new position). It's `SelfIntersect` when any of these holds:
   - `moved.validate()` is not empty (for example, a face's loop now crosses itself);
   - `moved.face_overlaps()` is not empty;
   - an edge pierces a face: for an edge whose ends are both not vertices of the face (any
     of its loops), with signed distances `da`, `db` of its ends from the face's plane,
     the candidate points are the crossing point when the ends are on opposite sides by
     more than `LINEAR_TOL` each, plus any end within `LINEAR_TOL` of the plane; the edge
     pierces the face when a candidate is not outside the face, that is, inside or on the
     boundary of the outer loop and not strictly inside any hole (`geom::to_plane_2d`,
     `geom::point_in_polygon_2d` with `LINEAR_TOL`). Skip pairs whose bounding boxes
     (grown by `LINEAR_TOL`) don't overlap;
   - `check_geometry(&moved)` is not solid, or has a different number of shells.

   This is growing until a slot or gap between two parts of the same body closes.

## `offset_body(model, body, distance, log)`

1. `BadDistance` as above, before anything else.
2. `MissingBody(body)` when the body doesn't exist.
3. `report = check_body(model, body)`; not solid (this also counts nested groups and
   components): `NotSolid(Box::new(report))`.
4. `offset_positions` on the body's geometry; pass its error on.
5. Set every vertex to its new position through `Model::edit_geometry` with
   `Context::set_vertex_position`, in ascending vertex id, so `log` gets exactly one
   `ModelRecord::Geometry(body, Record::MoveVertex { .. })` per vertex.
6. Return `OffsetReport { moved: number of vertices, volume_before: report.volume,
   volume_after: the volume of check_body after the move }`.

On any error nothing changes and nothing is added to `log`. Face materials and texture
placements stay as they are.

## `offset_bodies(model, bodies, distance, undo, label)`

For the Grow and Shrink menu items, which work on every selected group or component:

- Offset each body in the order given with `offset_body`, into one shared `ModelEdit`. Each
  body is all or nothing; one that fails is left alone and the others still go ahead.
- A body listed again is not offset again; its entry repeats the first result.
- Return one result per entry of `bodies`, in order.
- When at least one body changed, push the shared edit as one undo step named `label`.
  When none did (every body failed, or `bodies` is empty), push nothing.

The body is a definition, so every instance of a component changes together, as with any
edit inside a component. The distance is in the body's own coordinates; the UI task turns
the clearance into that.

## What the tests check

- Boxes: corners move by `d` along each axis, volume `(a + 2d)(b + 2d)(c + 2d)`, bounds, ids
  and loops kept, only `MoveVertex` records, other parts untouched, rotated boxes, a reversed
  face, two shells in one part.
- Vertex rules: a vertex on a split edge, square pyramid apex, octahedron, rectangular
  pyramid apex unsupported, two boxes sharing a corner unsupported.
- Holes and cavities: a hole through a block narrows when growing and widens when shrinking;
  a cavity shrinks when the part grows; both collapse when narrower than `2d`.
- Collapse and overlap: shrinking past half the thickness, by exactly half, leaving less than
  `LINEAR_TOL`, and just under half (works); a slot that stays open at 0.5 mm and closes at
  1.5 mm; two boxes 0.3 mm apart growing into each other (refused) and shrinking (fine).
- Refusals and their order: bad distances, missing body, open box, nested group, the root,
  a curved part, `NotSolid` before `Curved`, `Unsupported` before `Collapse`.
- `offset_bodies`: one undo step with the label, undo and redo, a failing part left alone,
  nothing pushed when all fail or the list is empty, a repeated body offset once.
- Properties: grown box volume and bounds; placed boxes and convex prisms come back to
  their old positions when grown and shrunk (or shrunk and grown) by the same `d`, with
  every face's vertices exactly `d` from its old plane; shrinking a box past half its
  thinnest side is refused and changes nothing; a cavity shrinks by `2d` when the part grows.

The test helper `offset` in `offset.rs` also checks, after every success, that the model is
valid and still solid and that `log.inverse()` gives back the model as it was; after every
error, that the model and the log are unchanged.

## Not in scope

- Curved parts (arcs and circles): refused with `Curved`.
- Rounded offsets: corners stay sharp (each face just moves).
- Splitting a vertex where the planes don't meet, or trimming a closed slot: refused.
- Menu items, the clearance field and status bar messages (`m7-clearance-offset-ui`).

## Where to look

- `offset.rs` skeleton: `OffsetError`, `OffsetReport`, `SAME_PLANE_DOT`, `RANK_TOL`,
  `RESIDUAL_REL`.
- Solid check: `check_geometry`, `check_body`, `SolidReport` (`is_solid`, `reversed_faces`,
  `shells`, `volume`).
- Context: `Context::face_plane`, `Context::edges_of_vertex`, `Context::faces_of_edge`,
  `Context::loop_points`, `Context::loop_vertices`, `Context::set_vertex_position`,
  `Context::validate`, `Context::face_overlaps`, `Context::curve_count`, `Face`, `Loop`.
- Geometry: `geom::LINEAR_TOL`, `geom::area_vector`, `geom::Plane` (`signed_distance`),
  `geom::to_plane_2d`, `geom::point_in_polygon_2d`, `geom::Containment`.
- Model: `Model::edit_geometry`, `Model::body`, `ModelEdit`, `UndoStack::push`.
- How a similar step-by-step kernel op is laid out: `repair_body` in `repair.rs`.
- Test helpers in `tests/offset_common/mod.rs`: `part`, `box_part`, `box_points`,
  `box_faces`, `prism`, `planes_moved_by`, `topology_of`.
