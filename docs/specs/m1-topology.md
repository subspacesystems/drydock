# m1-topology: vertices, edges, faces, records and validation

Milestone M1. Crate `app/crates/modeler-kernel`, file `src/topology.rs`. Implement every `unimplemented!("m1-topology")`. Don't change public signatures or types; add private fields (to `Context`) and private helpers freely. `src/geom.rs` is done (use it, don't edit it). Dependencies: `glam` only. No panics on any input.

Design background: `docs/adr/004-kernel-design.md`. A `Context` holds vertices, edges and faces in `BTreeMap`s keyed by ids. The primitives below keep references consistent but do **no** geometry checks: tests build broken geometry on purpose and `validate` must find it.

## Ids
- Each kind (vertex, edge, face) has its own counter. The first id a new context hands out is `0`, then `1`, `2`, ...
- Ids are never reused: removing an entity (or undoing its creation) doesn't lower the counter. Applying an `Add*` record whose id is at or above the counter moves the counter to `id + 1`.
- `Context` equality (already written) ignores counters and any private caches you add. `Clone` must copy them.

## Records and edits
- `Record::inverse`: `AddX(id, data)` <-> `RemoveX(id, data)`; `MoveVertex { id, from, to }` -> `MoveVertex { id, from: to, to: from }`.
- `Edit::inverse`: the inverse of each record, last record first.
- `Context::apply(record)` (on error, nothing changes):
  - `AddVertex`: id present -> `IdInUse`.
  - `AddEdge`: id present -> `IdInUse`; an end vertex missing -> `MissingVertex(first missing in ends order)`; both ends equal -> `SameVertex(v)`.
  - `AddFace`: id present -> `IdInUse`; any edge in any loop missing -> `MissingEdge(first missing, outer loop first, then inner loops in order)`.
  - `RemoveVertex`: missing -> `MissingVertex`; stored data differs -> `Mismatch`; an edge uses it -> `VertexInUse`.
  - `RemoveEdge`: missing -> `MissingEdge`; differs -> `Mismatch`; a face uses it -> `EdgeInUse`.
  - `RemoveFace`: missing -> `MissingFace`; differs -> `Mismatch`.
  - `MoveVertex`: missing -> `MissingVertex`; current position != `from` (exact) -> `Mismatch`; else set to `to`.
  - Check in the order listed for each record.
- `apply_edit`: applies records in order; on the first error, undoes the records already applied (in reverse) and returns that error. The context ends equal (`==`) to how it started; counters may have moved.

## Primitives
Each allocates the next id (if it creates something), applies the record through the same rules as `apply`, and pushes the record onto `log` only on success.
- `insert_vertex(p)` -> new id (never fails).
- `insert_edge(a, b)` stores `ends: [a, b]` as given. Errors as `AddEdge`. A failed call does not use up an id.
- `insert_face(face)`: errors as `AddFace`. Doesn't check that loops close or are planar.
- `delete_vertex`, `delete_edge`, `delete_face`: build the `Remove*` record from the stored data; errors as above (`Missing*`, `VertexInUse`, `EdgeInUse`).
- `set_vertex_position(v, p)`: `MoveVertex { id: v, from: current, to: p }`; `MissingVertex` if absent.

## Queries
- `edges_of_vertex`, `faces_of_edge`: ascending ids, no duplicates (a face lists once even if it used an edge twice); empty for unknown ids. Keep private adjacency maps updated in `apply` so these don't scan everything (not tested for speed, but `m1-sticky` calls them a lot).
- `edge_between(a, b)`: lowest-id edge with ends `{a, b}` in either order.
- `find_vertex(p, tol)`: closest vertex with distance `<= tol`; ties go to the lowest id; `None` if none.
- `use_start(u)`: `ends[0]` of the edge, or `ends[1]` when `reversed`. `loop_vertices`: `use_start` of each use (unknown edges skipped). `loop_points`: their positions.
- `face_plane(f)`: `geom::fit_plane(loop_points(outer))`.

## validate
Returns every violation found, in this order: all vertex checks, then edge checks, then each face in ascending id order, then duplicate faces. Within a check, pairs are reported in ascending order of (lower id, higher id), lower id first. Uses `geom::LINEAR_TOL` throughout.
1. `StrayVertex(v)`: no edge uses `v`. Ascending.
2. `DuplicateVertex(a, b)`: positions within `LINEAR_TOL` (distance `<=`). Every such pair.
3. `DuplicateEdge(a, b)`: same unordered end pair. Every such pair.
4. `EdgesIntersect(a, b)`: for every other pair of edges (skip pairs already reported as duplicates), use `geom::segment_intersection` with `LINEAR_TOL`:
   - Edges sharing exactly one vertex: fine if the result is `None`, or a `Point` whose parameters are exactly the shared vertex's end on each edge (`0.0` or `1.0`). Anything else (overlap, or a point elsewhere) is a violation.
   - Edges sharing no vertex: any `Point` or `Overlap` is a violation.
5. For each face (ascending), check its loops; stop at the first failing step for that face, so each face reports at most one violation:
   1. `OpenLoop`: some loop (outer or inner) has fewer than 3 uses, or a use's end vertex isn't the next use's start vertex (wrapping last to first).
   2. `LoopNotSimple`: some loop visits a vertex twice, or some edge appears more than once anywhere in the face's loops.
   3. `NonPlanarFace`: `face_plane` is `None`, or any point of any loop is farther than `LINEAR_TOL` from it.
   4. `LoopNotSimple`: some loop, projected with `geom::to_plane_2d(points, plane.normal)`, fails `geom::polygon_is_simple_2d(_, LINEAR_TOL)`.
   5. `HoleWinding`: an inner loop's projected signed area is not negative (the outer loop's is positive by construction).
   6. `HoleOutside`: some vertex of an inner loop is not `Inside` the projected outer loop (`point_in_polygon_2d` with `LINEAR_TOL`).
   7. `HolesTouch`: for two inner loops, some vertex of one is not `Outside` the other.
6. `DuplicateFace(a, b)`: two faces whose outer loops use the same set of edge ids. Every such pair.

Faces sharing an edge are fine, however many there are (non-manifold geometry is legal). Collinear neighbouring edges in a loop are fine.

## Tests (fixed; don't edit)
`tests/topology.rs` (primitives, records, queries, one test per violation) and `tests/topology_props.rs` (proptest: random build-and-delete sequences undo back to empty and redo to the same state; ids are never reused; a failing record in an edit leaves the context unchanged; grids of disjoint squares and boxes validate clean).
