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
6. Once `m1-sticky-faces` lands: no two coplanar faces overlap in area (they may share edges and vertices only). Add this to `validate()` there, so the property tests catch a face built over existing ones.

## Operations (each returns an `Edit` that can be applied in reverse)
- `add_edge(a, b)`: add vertices as needed; split existing edges the new edge crosses or touches (and split the new edge at existing vertices); then create faces for the new minimal closed coplanar regions, and split an existing face when the new edge cuts across it (see Face finding).
- `erase_edge(e)`: remove the edge and every face using it, except that two coplanar faces sharing it merge (see Face finding).
- `add_face(points)` for direct polygon creation (used by import).
- Undo/redo: every mutation yields a list of primitive records (added/removed vertex, edge, face); the undo stack stores them and replays reversed. No operation mutates without a record.

## Native file (`format.rs`)
- Versioned line-based UTF-8 text, defined in `docs/format.md`. Ids and numbers round-trip exactly (shortest `Display` floats); the reader rebuilds the model through `Model::apply`, so references are checked by the same code as undo. Save writes `<name>.tmp`, syncs it and renames it over the target.

## Face finding (`m1-sticky-faces`, design draft)
- Work per affected plane `P`. Planes come from each edge the operation inserted: the plane of every existing face containing it, and the plane it spans with each non-collinear edge at either end. Planes are deduplicated with `LINEAR_TOL`.
- In `P`: take every edge whose ends lie within tolerance of `P` and drop bridges (edges on no cycle: dangling spurs, links between loops). These edges just lie on faces. Project to 2D with `plane_basis` and trace the boundary walks of the planar arrangement (turn to the next edge clockwise at each vertex). Walks with positive area are region outlines. Negative walks are outer contours of connected components; each one becomes a hole of the smallest outline (from another component) that contains it.
- A region can become a face only if its outline and holes are simple. A region touching itself at a vertex can't, and a face that would have to be split into one is left alone.
- Region `R` lies inside an old face `F` (`F` in `P`) when the midpoint of `R`'s first outline edge that isn't one of `F`'s edges is inside `F`. If all of `R`'s edges are `F`'s edges, `R` is inside `F` when `R` crosses its first edge with `F`'s interior on the same side.
- `add_edge`: a region with a new edge on its outline or holes, inside no old face, becomes a new face. An old face with a new-edge region inside it is replaced by all the regions inside it, keeping its orientation, provided every one of them can be a face.
- Orientation of a new face: opposite use of a shared edge with an existing face (lowest id), so solids come out consistently oriented. With no shared face, the front faces +Z, then +Y, then +X: the first of `n.z`, `n.y`, `n.x` that isn't ~0 is positive. Horizontal faces therefore face up. This is easy to change if Tyrel wants ground faces to face down.
- Sticky reports where each new face came from (`(new, Option<old>)`), so the model layer copies face materials to split pieces.
- `erase_edge` on an edge shared by exactly two coplanar faces with the same orientation merges them: after the erase, the region of `P` containing the edge's midpoint becomes one face, oriented like the lower-id face. Otherwise the behaviour stays as in `m1-sticky-edges`: faces whose outer loop uses the edge go, and holes it borders close.
- Known limit: regions whose outline touches itself (for example a triangle inside a face that touches the face's edge at one vertex) get no face.

## Task order
`m1-geom` (math) -> `m1-topology` (arenas, ids, validate, primitive records) -> `m1-sticky-edges` (snap, split crossings, T-junctions and overlaps, erase; no face creation) -> `m1-sticky-faces` (faces from closed coplanar loops, splitting a face, holes, merging coplanar faces on erase). In parallel after topology: `m1-model` (definitions, instances, materials, model records, undo stack; replaces `m1-undo`) -> `m1-format` (native file, `docs/format.md`) and `m1-dae-to-kernel` (merge triangles into polygons, keep hierarchy).
