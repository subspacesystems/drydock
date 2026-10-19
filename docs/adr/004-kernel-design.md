# ADR-004: Geometry kernel design

Status: accepted (M1). Owner: orchestrator.

## Model
- Crate `modeler-kernel`. A `Context` (the root model, or the inside of one group/component definition) owns arenas of vertices, edges and faces, addressed by generational ids (`VertexId`, `EdgeId`, `FaceId`). Ids are never reused within a session, so undo records stay valid.
- Vertex: a point. Edge: two distinct vertices, no direction. Face: one outer loop and zero or more inner loops, each a closed cycle of oriented edge uses. The plane is derived from the outer loop, not stored. Inner loops run clockwise seen from the front. A vertex no edge uses is invalid (operations remove orphans). A face's front side follows the loop's counter-clockwise order seen from the front; material per side lives on the face.
- Groups and component instances are separate contexts with a transform. Geometry in different contexts never merges ("sticky" only within one context).
- All lengths are meters (f64). `geom::LINEAR_TOL` (10 um) is the single identity tolerance.

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
`m1-geom` (math) -> `m1-topology` (arenas, ids, validate, primitive records) -> `m1-sticky` (add/erase edge, splitting, face creation; kernel task, orchestrator-reviewed) -> `m1-undo` -> `m1-format` (native file, `docs/format.md`) -> `m1-dae-to-kernel` (merge triangles into polygons, keep hierarchy).
