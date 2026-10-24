# m4-fix-copy-speed: copying in large models without freezing

Milestone M4, from Tyrel's report: Move with copy (Ctrl) on a dining chair in a
large imported house froze the program ("Not Responding") when he clicked to place it.

## Why it happens today

Measured on that house (190 bodies, 158 000 edges; the room body that holds the chairs has
11 800 edges and 4 800 faces, the largest body 28 000 edges; imported bodies hold hundreds
of defects such as duplicate faces and crossing edges):

1. `copy::insert` (behind Move-copy and Paste) calls `Model::validate` before and after the
   edit. `Context::validate` compares every pair of vertices, edges and faces, so one
   `Model::validate` of the house takes several minutes. A 3 000-edge body alone takes
   2.6 s in a release build. `make_component` and `explode` do the same, and Move, Rotate,
   Scale and Push/Pull validate the body they change.
2. With a fast `validate` the copy still takes 5 s and then *fails*: `insert` puts the copy
   into a temporary group and explodes it, and the explode's clean-up passes visit every
   face of the target body (about F² containment tests) and re-insert some untouched faces
   under new ids. A pre-existing defect on such a face then looks new, and the copy is
   rolled back ("Can't copy that").
3. Copying a group whose own geometry has defects fails the same way: the group's copy is a
   new definition carrying the same defects, and they count as new.

## Part 1: `Context::validate` without all-pairs loops

`Context::validate` (`crates/modeler-kernel/src/topology.rs`) returns **exactly the same
list** as today, in the same order, for every context. Only the running time changes.

Today's order, which stays:

1. `StrayVertex` in vertex id order.
2. `DuplicateVertex(a, b)` for every pair of vertices within `geom::LINEAR_TOL` (distance
   `<=`), `a < b`, sorted by `(a, b)`.
3. `DuplicateEdge(a, b)` for every pair of edges with the same two ends (either order),
   `a < b`, sorted by `(a, b)`.
4. `EdgesIntersect(a, b)`, `a < b`, sorted by `(a, b)`: every pair that is not a duplicate
   pair and whose `geom::segment_intersection(a0, a1, b0, b1, LINEAR_TOL)` (called with the
   lower id's edge first, exactly as today) is bad by today's rule (no shared end: any hit;
   one shared end: anything but the single point at that shared end; two shared ends:
   never). Edges whose end vertex is missing are skipped.
5. The face checks, one per face, in face id order (unchanged).
6. `DuplicateFace(a, b)` for every pair of faces whose outer loops use the same set of
   edges, `a < b`, sorted by `(a, b)`.

The acceptance tests hold today's implementation as `all_pairs` and compare against it on
hand-made and random contexts. Keep the tolerance comparisons (`<=`), the argument order of
`segment_intersection`, and the results for contexts with non-finite positions.

How: replace each all-pairs loop with a near-linear search; the exact test for each
candidate pair stays the same.

- Duplicate vertices: a hash grid. With cell side `2 * LINEAR_TOL`, two points within
  `LINEAR_TOL` fall in the same or neighbouring cells, so each point checks the 27 cells
  around its own. Collect `(i, j)` index pairs, sort, map to ids.
- Duplicate edges: a hash map keyed by the sorted end pair.
- Edge intersections: a broad phase over the edges' bounding boxes, each grown by
  `LINEAR_TOL` on every side. Use a bounding-volume tree (split at the median center along
  the axis of largest spread, small leaves) or another structure that stays fast when many
  edges are long and parallel (see the tests). Edges whose box isn't finite are paired with
  every other edge instead, so the result still equals the reference. Sort the candidate
  pairs, drop repeats and duplicate-edge pairs, then run today's exact test.
- Duplicate faces: a hash map keyed by the outer loop's edge set.

`std::collections::HashMap` is fine (the output is sorted, so hash order never shows).

## Part 2: edits check and touch only what they change

### `Model::validate_bodies` (`crates/modeler-kernel/src/model.rs`, skeleton on main)

```rust
pub fn validate_bodies(&self, bodies: &[BodyId]) -> Vec<ModelViolation>
```

Exactly the violations `validate()` reports that belong to `bodies`, in `validate()`'s
order: `Geometry(b, _)` and `BadTransform(b, _)` for each listed body `b`, and
`GroupInstances(d, _)` for each group `d` whose body `BodyId::Definition(d)` is listed.
Bodies that don't exist are skipped. It must not look at unlisted bodies' geometry (that is
the point: the tests time it next to a large unrelated body).

### The before/after checks

- `copy::insert`: `before` and the check after use `validate_bodies(&[body])`. Definitions
  that `insert` creates are raw copies of the clipboard's definitions (`copy_body_into`), so
  they can't hold anything their source didn't; don't check their geometry.
- `component::make_component`: before `validate_bodies(&[body])`, after
  `validate_bodies(&[body, BodyId::Definition(new definition)])`.
- `explode::explode`: before and after `validate_bodies(&[body])`.

The comparison stays "violations after that weren't there before".

### Copying instances without exploding

In `insert_inner` (`crates/modeler-kernel/src/copy.rs`): when the clipboard's root body has
no geometry (no vertices, edges or faces; only instances), don't make the temporary group
and don't explode. Add each clipboard root instance to `body` directly:

- `definition`: mapped through `definition_map` (so a component instance copied within the
  same model keeps its definition, and every copy follows edits of it);
- `transform`: `matrix * instance.transform`;
- `material`: mapped through `material_map`; `name` unchanged.

Return the new instances as `Entity::Instance`, sorted. The body's geometry is not touched
at all. When the clipboard root has geometry, keep today's path.

### Explode leaves faces away from the copied geometry alone

In `explode_inner` (`crates/modeler-kernel/src/explode.rs`): remember the body's face ids
before the copied geometry goes in, and the bounding box of the copied geometry (all copied
edge end points, after the instance transform) grown by `LINEAR_TOL`. The two clean-up
passes after `restick` (dropping unwanted faces, and orienting faces and giving them
materials) only visit:

- faces that are new (not in the remembered set), and
- remembered faces whose outer loop's bounding box meets the copied box.

Every other face keeps its id, loops, orientation and materials. With no copied geometry
(an instance whose definition holds only instances) the box is empty and no remembered face
is visited. Everything else in `explode_inner` (restick, nested instances, the group
clean-up, the returned entities) stays as it is.

## Acceptance tests

`crates/modeler-kernel/tests/validate_speed.rs` (part 1):

- Hand-made contexts for each violation kind (duplicate vertices either side of possible
  cell walls, near-touching T-junctions, very long edges, duplicate and reversed faces, a
  non-planar face, non-finite positions): `validate` equals `all_pairs`.
- A proptest (512 cases) on random lattice contexts with points nudged by about the
  tolerance: `validate` equals `all_pairs`.
- Speed, each on a thread with a 20 s limit (debug build): a 100 × 100 grid of squares,
  6000 long parallel edges, two families of 3000 long edges in different planes. All three
  are valid. The reference implementation takes 0.5–2 s for each in a debug build.

`crates/modeler-kernel/tests/copy_local.rs` (part 2), with a deliberately broken body
(duplicate face, crossing edges, a self-crossing face, a bent face):

- Copying an instance in the broken body succeeds, returns one new instance with the same
  definition, name and material and transform `matrix * original`; the body's geometry and
  face materials are unchanged; no definition is added; undo restores the model.
- Two copies of a component share its definition.
- Pasting an instance into another broken body keeps that body's geometry.
- Copying a group whose geometry is broken makes a new group with the same geometry.
- Faces and an instance copied together: six faces and the instance (same definition,
  transform `matrix * original`), sorted result.
- Exploding next to broken geometry keeps every earlier face (id, loops, materials) and
  every earlier violation.
- Exploding an instance whose definition holds only instances changes no geometry.
- `validate_bodies` equals `validate()` filtered to the listed bodies; all bodies give
  `validate()`; unknown bodies give nothing.
- Copy, make component and two explodes in a small body finish within 10 s next to an
  unplaced definition with a 90 × 90 grid of edges.

`crates/modeler-dae/tests/copy_corpus.rs` (oracle only; skipped without `MODELER_CORPUS`):
in the corpus house, copying the first chair instance in the body under the root's instance
succeeds within 60 s (the reference takes about 2.5 s in a debug build), keeps the chair's
definition and leaves the body's geometry unchanged.

Every existing kernel test must stay green (sticky, move, push/pull, copy, explode,
component, format all go through `validate`).
