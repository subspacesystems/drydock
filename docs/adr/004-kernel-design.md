# ADR-004: Geometry kernel design

Status: accepted (M1). Owner: orchestrator.

## Model
- Crate `modeler-kernel`. A `Context` (the root model, or the inside of one group/component definition) owns arenas of vertices, edges and faces, addressed by generational ids (`VertexId`, `EdgeId`, `FaceId`). Ids are never reused within a session, so undo records stay valid.
- Vertex: a point. Edge: two distinct vertices, no direction. Face: one outer loop and zero or more inner loops, each a closed cycle of oriented edge uses. The plane is derived from the outer loop, not stored. Inner loops run clockwise seen from the front. A vertex no edge uses is invalid (operations remove orphans). A face's front side follows the loop's counter-clockwise order seen from the front; material per side lives on the face.
- Each definition (group or component) has its own context; instances reference a definition with a transform, so all instances of a component share one context (see Model layer). Geometry in different contexts never merges ("sticky" only within one context).
- All lengths are meters (f64). `geom::LINEAR_TOL` (10 um) is the single identity tolerance.

## Model layer (`model.rs`)
- `Model` = a root `Body` plus definitions (`DefinitionInfo { name, group }`), each with its own `Body`. A body holds one `Context`, its instances (definition + `DMat4` transform + name + optional material) and face materials. A group is a definition meant to have exactly one instance (checked by `validate`, not by `apply`). Instance ids are unique across the model. Instances may not form cycles.
- Face materials (front/back) live in a side map in the body keyed by `FaceId`, not in `Face`. Entries stay when the face is removed. Ids are never reused, so undoing the removal brings the paint back with no extra records. Stale entries count as material uses, and saving writes only entries for existing faces.
- `Material`: name, sRGB color with straight alpha (opacity), optional texture path relative to the model file. How texture coordinates map onto kernel faces is decided in `m1-dae-to-kernel`.
- Every change is a `ModelRecord` (`Geometry(body, Record)` wraps topology records) with an inverse. `UndoStack` holds labelled `ModelEdit`s. `edit_geometry` runs a closure on a body's `Context` and wraps its topology `Edit`, so kernel operations stay `Context`-only.

## Invariants (checked by `Context::validate()`, used by every property test)
1. Every edge has two distinct vertices; no two edges share the same vertex pair; no two vertices coincide within tolerance.
2. No two edges overlap or cross except at a shared vertex (T-junctions are split).
3. Each loop is a closed simple cycle; each face is planar within tolerance; inner loops lie strictly inside the outer loop and don't touch each other.
4. An edge may border any number of faces (non-manifold models from DAE import are legal); the kernel stores the face uses and never assumes two.
5. Edge to face and face to edge back references agree.
6. No two coplanar faces overlap in area (they may share edges and vertices only). Checked by `Context::face_overlaps()`, not `validate()`: imported models can hold overlapping decals, so this is an invariant of sticky editing, asserted by the sticky property tests.

## Operations (each returns an `Edit` that can be applied in reverse)
- `add_edge(a, b)`: add vertices as needed; split existing edges the new edge crosses or touches (and split the new edge at existing vertices); then create faces for the new minimal closed coplanar regions, and split an existing face when the new edge cuts across it (see Face finding).
- `erase_edge(e)`: remove the edge and every face using it, except that two coplanar faces sharing it merge (see Face finding).
- `add_face(points)` for direct polygon creation (used by import).
- Undo/redo: every mutation yields a list of primitive records (added/removed vertex, edge, face); the undo stack stores them and replays reversed. No operation mutates without a record.

## Native file (`format.rs`)
- Versioned line-based UTF-8 text, defined in `docs/format.md`. Ids and numbers round-trip exactly (shortest `Display` floats); the reader rebuilds the model through `Model::apply`, so references are checked by the same code as undo. Save writes `<name>.tmp`, syncs it and renames it over the target.

## Face finding (`m1-sticky-faces`, spec `docs/specs/m1-sticky-faces.md`)
- Work per affected plane `P`. Planes come from each edge the operation inserted: the plane of every existing face containing it, and the plane it spans with each non-collinear edge at either end. Planes are deduplicated with `LINEAR_TOL`.
- In `P`: take every edge whose ends lie within tolerance of `P` and drop bridges (edges on no cycle: dangling spurs, links between loops). These edges just lie on faces. Project to 2D with `plane_basis` and trace the boundary walks of the planar arrangement (turn to the next edge clockwise at each vertex). Walks with positive area are region outlines. Negative walks are outer contours of connected components; each one becomes a hole of the smallest outline (from another component) that contains it.
- A region can become a face only if its outline and holes are simple. A region touching itself at a vertex can't, and a face that would have to be split into one is left alone.
- Region `R` lies inside an old face `F` (`F` in `P`) when the midpoint of `R`'s first outline edge that isn't one of `F`'s edges is inside `F`. If all of `R`'s edges are `F`'s edges, `R` is inside `F` when `R` crosses its first edge with `F`'s interior on the same side.
- `add_edge`: a region with a new edge on its outline or holes, inside no old face, becomes a new face. An old face with a new-edge region inside it is replaced by all the regions inside it, keeping its orientation, provided every one of them can be a face.
- Orientation of a new face: opposite use of a shared edge with an existing face (lowest id), so solids come out consistently oriented. With no shared face, the front faces +Z, then +Y, then +X: the first of `n.z`, `n.y`, `n.x` that isn't ~0 is positive. Horizontal faces therefore face up. This is easy to change if Tyrel wants ground faces to face down.
- Sticky reports where each new face came from (`(new, Option<old>)`), so the model layer copies face materials to split pieces.
- An old face whose only inside region has exactly its edges is left alone (no new id), so redrawing an edge or a spur changes nothing. Redrawing an edge of a loop that has no face restores the face (chain edges count whether inserted or reused). The merged face on erase keeps the lower id. New loops start at their lowest-id edge, so results are deterministic.
- Existing faces are never flipped: a box comes out consistent when drawn outward from one face, not in every drawing order (two faces created apart may disagree when a third joins them).
- `erase_edge` on an edge shared by exactly two coplanar faces with the same orientation merges them: after the erase, the region of `P` containing the edge's midpoint becomes one face, oriented like the lower-id face. Otherwise the behaviour stays as in `m1-sticky-edges`: faces whose outer loop uses the edge go, and holes it borders close.
- Known limit: regions whose outline touches itself (for example a triangle inside a face that touches the face's edge at one vertex) get no face.

## Task order
`m1-geom` (math) -> `m1-topology` (arenas, ids, validate, primitive records) -> `m1-sticky-edges` (snap, split crossings, T-junctions and overlaps, erase; no face creation) -> `m1-sticky-faces` (faces from closed coplanar loops, splitting a face, holes, merging coplanar faces on erase). In parallel after topology: `m1-model` (definitions, instances, materials, model records, undo stack; replaces `m1-undo`) -> `m1-format` (native file, `docs/format.md`) and `m1-dae-to-kernel` (merge triangles into polygons, keep hierarchy).

## DAE import (`m1-dae-to-kernel`)
- Lives in `modeler-dae` (`to_kernel`), which now depends on `modeler-kernel`; the kernel knows nothing about COLLADA. Built from topology primitives (`insert_*`), not sticky operations.
- Frame: every point goes through `C = frame(up) * scale(unit)` (same frame as `flatten`), every transform `T` becomes `C T C^-1`, so all bodies are in meters, Z up, and `LINEAR_TOL` means 10 um everywhere.
- Hierarchy: each `<library_nodes>` node is a component definition. In a node's contents, a child node holding only one `instance_node` (no geometry, no children) is an instance of that definition; a child holding nothing in its subtree is skipped (camera nodes); any other child is a group (definition with `group = true` and one instance). Instance refs sitting directly in a node's contents become instances too.
- Faces: weld points (closest earlier point within `LINEAR_TOL`), drop degenerate and duplicate triangles, fold an opposite-winding twin into the back material. Patches are edge-connected sets of triangles that share an edge used by exactly those two triangles in opposite directions, have equal front/back materials, no line segment on the shared edge, and lie within `LINEAR_TOL / 2` of the seed triangle's plane with the same facing. A patch's boundary becomes one face (outer loop + holes); if the boundary pinches or the face fails `face_violation`, the patch falls back to one face per triangle.
- Materials: color is `[r, g, b, opacity]`; texture keeps the resolved path as a string. Texture coordinates are dropped for now: the plan is a per-face affine map from plane coordinates to UV, added with format v2 when kernel models are rendered.
- Corpus findings (throwaway impl, 2026-10-19): the corpus is exported two-sided (about half the triangles are back twins), so per-material area only matches `flatten` as front plus back. Which twin is the front is "first in reading order"; for some materials (ground in 01) that puts the exporter's back side in front. Recheck by eye when kernel models are rendered. 01 also repeats a few faces in one component (dropped as duplicates). 0.19-0.33 faces per file triangle; 05-house converts in 27 s in a debug build.
- Not repaired: crossing or T-junction edges in the source (they show up as `EdgesIntersect` in `validate`), twins triangulated differently from their front, faces with slits. Recheck with the corpus when sticky faces land.
