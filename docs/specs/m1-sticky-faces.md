# m1-sticky-faces: faces from closed coplanar edges

Crate `modeler-kernel`. Change `src/sticky.rs` (`add_edge`, `erase_edge`) and implement
`Context::face_overlaps` in `src/topology.rs`. You may add private helpers or a new private
module (for example `src/faces.rs`). Don't change public signatures; `geom.rs` and the
tests are fixed.

Read first: `docs/adr/004-kernel-design.md` ("Face finding") and `docs/specs/m1-sticky-edges.md`.
Everything from sticky-edges still holds (snapping, splitting, rollback on error, every change
recorded in `log` so `log.inverse()` undoes it exactly).

## Terms
- Tolerance is `geom::LINEAR_TOL` everywhere.
- Two planes are the same when their unit normals are parallel (`|n1 . n2| >= 1 - 1e-9`) and a
  point of one is within tolerance of the other. A point or edge "lies in" plane `P` when it
  (both ends) is within tolerance of `P`.
- Canonical normal of a plane: the unit normal `n` whose first component of `z, y, x` with
  `|c| > 1e-9` is positive (so horizontal planes have `+Z`, vertical planes facing along `y`
  have `+Y`, the plane `x = c` has `+X`). Project points of `P` to 2D with
  `geom::plane_basis(n)` (counter-clockwise in 2D = counter-clockwise seen from `+n`).
- A face's front normal is `face_plane(f).normal` (from its outer loop order, CCW seen from
  the front). A face's interior lies to the left of every one of its loop uses seen from its
  front (holes run clockwise).

## The arrangement of a plane
For a plane `P`, take every edge of the context lying in `P`. Remove bridges: edges that are on
no cycle of this edge graph (dangling spurs, links between two loops). Bridges just lie on
faces or in empty space; they never bound one.

Trace the remaining edges as a planar graph in 2D. Each edge gives two directed walks (one per
direction). After arriving at vertex `v` from `u`, leave along the edge at `v` that comes
first turning **clockwise** from the direction `v -> u` (if `v -> u` is the only edge, it
can't be: no bridges remain). Each closed walk keeps its region on its left.
- Walks with positive signed 2D area are **outlines** of bounded regions.
- Walks with negative area are **contours**: the outer boundary of one connected piece of the
  graph. A contour becomes a **hole** of the smallest-area outline from a different connected
  piece that strictly contains it (test any vertex of the contour). A contour inside no outline
  bounds the unbounded region and is ignored.
- A **region** is one outline plus its holes. Its edges are the edges of all those walks.
- A region **can be a face** when its outline and each hole visit no vertex twice. (Two loops
  touching at a vertex give a walk that visits it twice: that region gets no face. Known limit,
  see ADR-004.)
- Region to face: the outline becomes the outer loop and each hole an inner loop, in walk
  order, so the front faces `+n`. To face `-n` instead, reverse every loop (order and each
  use's direction). Then rotate each loop so it starts at its use of the lowest-id edge. Inner
  loops are ordered by their lowest edge id.

## Old faces and regions
An old face `F` is a face whose plane is the same as `P`. Region `R` is **inside** `F` when:
- `R` has an outline edge that isn't one of `F`'s edges: take the first one in walk order; `R`
  is inside `F` when its midpoint is strictly inside `F` (inside the outer loop, not inside or
  on a hole loop; `geom::point_in_polygon_2d`).
- Otherwise (every outline edge of `R` is an edge of `F`): take `R`'s first outline use. `R`
  is inside `F` when `F` has its interior on the same side of that edge as `R` (`R`'s interior
  is on the left of its walk; `F`'s is on the left of its own use of the edge seen from `F`'s
  front, so compare directions in 2D with `F`'s front normal against `n`).

## `add_edge`
After the sticky-edges work, let `C` be the chain's edges (inserted or reused). If `C` is
empty nothing more happens.

Affected planes, in this order, deduplicated (keep the first): for each edge `e` of `C` in
chain order: the plane of every face whose plane `e` lies in (faces in id order); then, for
each end of `e` (first end first), each other edge at that end not collinear with `e` (edges in
id order; collinear: `|cross of unit directions| <= 1e-9`), the plane through both edges.

For each affected plane `P` in order, build the arrangement (using the context as it is at that
moment, including faces made for earlier planes) and:
1. **Split old faces.** For each old face `F` in id order: let `Rs` be the regions inside `F`.
   Skip `F` if no region of `Rs` has an edge in `C`, or if `Rs` is a single region with exactly
   `F`'s edge set (the face is already right). Skip it too if any region of `Rs` can't be a
   face. Otherwise remove `F` and add one face per region of `Rs` (in order of the region's
   lowest edge id), each with `F`'s front side. Report each as `(new, Some(F))`.
2. **New faces.** Each region that has an edge in `C`, is inside no old face (old faces of `P`
   before step 1), can be a face and isn't now a face (same edge set as an existing face) gets a
   new face, in order of the region's lowest edge id. Orientation: let `G` be the lowest-id
   face that exists now and uses one of the region's outline edges; the new face uses that edge
   in the opposite direction to `G` (so neighbours agree, and a box drawn outward from
   one face comes out consistently oriented). Existing faces are never flipped. With no such `G`, the front faces the canonical `+n`. Report `(new, None)`.

`EdgeChain::faces` lists every face added, in the order added. Faces removed in step 1 are not
listed (they are the `Some` origins). Faces only touched by sticky-edges' `split_edge` keep
their ids and aren't listed.

## `erase_edge`
If the edge is used by exactly two faces `A < B` (by id) whose planes are the same and whose
front normals point the same way (`dot > 0`), they merge:
- Remove `A`, `B` and the edge (and end vertices left without edges, as before).
- Build the arrangement of `A`'s plane. The region whose outline strictly contains the erased
  edge's midpoint, and that the midpoint is not inside a hole of, is the merged region.
- If that region exists and can be a face, add it with id `A` (re-add with the same id, as
  `split_edge` does) and `A`'s front side. Otherwise nothing is added (both faces are gone).

In every other case `erase_edge` behaves as in sticky-edges (faces whose outer loop uses the
edge go; a hole loop it borders is dropped from its face).

## `Context::face_overlaps`
Pairs `(a, b)`, `a < b`, sorted, of faces whose planes are the same (either facing) and whose
interiors share area. Faces sharing only edges or vertices don't overlap. Because valid edges
never cross, it is enough to test, for both orders: the midpoint of some loop edge of one face
is strictly inside the other (as in "inside" above), or the two outer loops use the same set of
edges. Brute force over pairs is fine; bucket by plane if you like.

## Tests
- `tests/sticky_faces.rs`: faces from loops (any order and direction), orientation, a line
  across a face splits it, a loop inside a face makes a hole and a filled face, redrawing an
  edge restores a missing face, spurs don't split, a box closes consistently oriented, merging
  on erase, the self-touching known limit, undo/redo of every step.
- `tests/sticky_faces_props.rs` (proptest): random unit-grid drawings with erases keep
  `validate` and `face_overlaps` clean, undo/redo exact, every face faces `+Z` with whole-cell
  area; a fully drawn grid (any order and directions) has exactly one face per cell; erasing
  random vertical inner edges of a full grid merges cells into strips and keeps the area.
- `tests/sticky_edges.rs` and `tests/sticky_edges_props.rs` are updated where closed loops now
  make faces.
