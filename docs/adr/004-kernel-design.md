# ADR-004: Geometry kernel design

Status: accepted (M1). Owner: orchestrator.

## Model
- Crate `modeler-kernel`. A `Context` (the root model, or the inside of one group/component definition) owns arenas of vertices, edges and faces, addressed by generational ids (`VertexId`, `EdgeId`, `FaceId`). Ids are never reused within a session, so undo records stay valid.
- Vertex: a point. Edge: two distinct vertices, no direction. Face: one outer loop and zero or more inner loops, each a closed cycle of oriented edge uses. The plane is derived from the outer loop, not stored. Inner loops run clockwise seen from the front. A vertex no edge uses is invalid (operations remove orphans). A face's front side follows the loop's counter-clockwise order seen from the front; material per side lives on the face.
- Groups and component instances are separate contexts with a transform. Geometry in different contexts never merges ("sticky" only within one context).
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

## Operations (each returns an `Edit` that can be applied in reverse)
- `add_edge(a, b)`: add vertices as needed; split existing edges the new edge crosses or touches (and split the new edge at existing vertices); then create faces for every new closed coplanar loop, and split an existing face when the new edge cuts across it.
- `erase_edge(e)`: remove the edge and every face using it.
- `add_face(points)` for direct polygon creation (used by import).
- Undo/redo: every mutation yields a list of primitive records (added/removed vertex, edge, face); the undo stack stores them and replays reversed. No operation mutates without a record.

## Task order
`m1-geom` (math) -> `m1-topology` (arenas, ids, validate, primitive records) -> `m1-sticky-edges` (snap, split crossings, T-junctions and overlaps, erase; no face creation) -> `m1-sticky-faces` (faces from closed coplanar loops, splitting a face, holes, merging coplanar faces on erase). In parallel after topology: `m1-model` (definitions, instances, materials, model records, undo stack; replaces `m1-undo`) -> `m1-format` (native file, `docs/format.md`) and `m1-dae-to-kernel` (merge triangles into polygons, keep hierarchy).
