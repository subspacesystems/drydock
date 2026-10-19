# m1-dae-to-kernel: imported documents become kernel models

Milestone M1, kernel task. Crate `app/crates/modeler-dae`, file `src/kernel.rs`. Implement `to_kernel`. Don't change public signatures or public types; add private helpers and private modules freely. You may make `flat::frame_matrix` `pub(crate)` and reuse it. Don't edit anything in `modeler-kernel`. Dependencies: what `modeler-dae` already has (`glam`, `roxmltree`, `modeler-kernel`, std). No panics on any input `Model`.

Build the result with the public API of `modeler_kernel::model::Model` (`add_material`, `add_definition`, `add_instance`, `set_face_materials`, `edit_geometry`) and, inside `edit_geometry`, the `Context` primitives (`insert_vertex`, `insert_edge`, `insert_face`, `delete_face`, `face_violation`). The undo log they fill is thrown away. Any `ModelError` they return is `Err(ConvertError::Model(e))`.

## Frame
`C = frame_matrix(up_axis) * DMat4::from_scale(DVec3::splat(unit_meters))` (the same frame `flatten` uses). Every point `p` read from a mesh is stored as `C.transform_point3(p)` (positions converted `f32` -> `f64` first). Every transform `T` built below is stored as `C * T * C.inverse()`.

## Materials and definitions (in this order, before any body)
1. Each document material, in order -> `add_material`: `name` is the material's name, or its id when the name is empty; `color` is `[r, g, b, opacity]` (the rgb of `color`, alpha replaced by `opacity`); `texture` is the path's `to_string_lossy()`. So the n-th document material is the n-th kernel material in id order.
2. Each `definitions[i]`, in order -> `add_definition` with the definition's name (its id when the name is empty) and `group: false`. Call the result `D[i]`.

## Bodies
Then fill the body of `D[0]`, `D[1]`, ... from `definitions[i].node`'s contents, then the root body from `scene`'s contents. The contents of a node `N` going into body `B`:
- `N.geometry` -> geometry of `B` (next section).
- Each child `K` of `N`, in order:
  - `K` has no geometry, no children and exactly one instance ref `d` -> `add_instance` in `B`: definition `D[d]`, transform `C * K.transform * definitions[d].node.transform * C^-1`, name `K.name`, material `None`.
  - Nothing in `K`'s subtree (`K` and its descendants) has geometry or instance refs -> skipped.
  - Otherwise `K` is a group: `add_definition` (name `K.name`, `group: true`), then its contents go into the new body (recursively, so nested groups get higher ids), then `add_instance` in `B` of the new definition with transform `C * K.transform * C^-1`, name `K.name`, material `None`.
- Each instance ref `d` in `N.instances`, in order -> `add_instance` in `B`: `D[d]`, transform `C * definitions[d].node.transform * C^-1`, name empty, material `None`.
- `N.transform` itself is not used here (whoever places `N` applies it).

A cycle of definitions instancing each other comes back from `add_instance` as `ModelError::Cycle`.

## Geometry of one body
Inputs: for each geometry instance of `N` in order, each primitive of its mesh in order. A `Triangles` primitive gives triangles (index triples in order); a `Lines` primitive gives segments (index pairs in order). A triangle's material is `instance.material_for(symbol)` mapped to the kernel material (`None` when the primitive has no symbol or it isn't bound). Indices out of range: skip that triangle or segment.

1. **Weld.** Triangles or segments with a non-finite converted coordinate are dropped. Every other point, in reading order, goes to the slot whose position is closest to it within `LINEAR_TOL` (on a tie, the earlier slot), or opens a new slot at that point. A slot's position is the first point that opened it. Use a hash grid or similar; `05-house` has 310k triangles and must convert in seconds, so no linear scans.
2. **Drop degenerate triangles**: two corners in one slot, `polygon_normal` of the three slot positions is zero, or `polygon_is_simple_2d(&to_plane_2d(&pts, normal), LINEAR_TOL)` is false. Drop segments whose ends share a slot.
3. **Twins.** Go through the remaining triangles in order. A triangle with the same three slots as an earlier kept triangle is dropped. If it runs the opposite way round and that kept triangle has no twin yet, it becomes the twin: the kept triangle's back material is this triangle's material. Every kept triangle has `FaceMaterials { front: its material, back: its twin's material or None }`.
4. **Joinable edges.** An unordered slot pair `{a, b}` joins triangles `t` and `u` when exactly two kept triangles use it (`t` and `u`), they run along it in opposite directions, no kept segment has the same two slots, and `t` and `u` have equal `FaceMaterials`.
5. **Patches.** Go through the kept triangles in order; each one not yet in a patch is a seed with plane `P` through its first corner with normal `polygon_normal` of its corners. Its patch is the set of triangles reachable from the seed through joinable edges, passing only through triangles that are in no earlier patch, have all three corners within `LINEAR_TOL / 2` of `P`, and whose `polygon_normal` has a positive dot product with `P`'s normal.
6. **Patch boundary.** The patch's directed triangle edges (`a->b`, `b->c`, `c->a`, triangles in patch order) whose slot pair is not joinable between two triangles of the patch. If any slot starts more than one boundary edge, the patch falls back (below). Otherwise trace loops: start at the first unused boundary edge, follow the unique edge starting where the last one ended, until back at the start. Project each loop with `to_plane_2d` along `P`'s normal. Exactly one loop must have positive `signed_area_2d`, else fall back. That loop is the outer loop, the others are holes, in tracing order.
7. **Face.** Get or create the edges (below) of the outer loop then the holes, in loop order, and `insert_face`. If `face_violation` reports anything for it, `delete_face` and fall back.
8. **Fallback**: each triangle of the patch, in patch order, becomes its own face (outer loop `a->b`, `b->c`, `c->a`).
9. **Segments**: after all patches, get or create an edge for each kept segment.
10. **Materials**: then `set_face_materials` for each face whose materials aren't both `None`.

Get or create: a slot becomes a kernel vertex (`insert_vertex` at the slot position) the first time an edge needs it; an unordered slot pair becomes one kernel edge the first time a loop or segment needs it, with `ends` in the order of that first use. A loop's use of an edge is reversed when it runs from `ends[1]` to `ends[0]`. Slots no edge needs never become vertices (the centre of a triangle fan disappears).

Results this gives: faces face the same way as their triangles; a flat polygon triangulated any way becomes one face; neighbouring faces share edges and vertices; the world area per material is the same as `flatten`'s, minus dropped triangles. Crossing edges in the document are not repaired.

## Tests (fixed; don't edit)
- `tests/kernel.rs`: small documents built in code (one quad, fan, hole, cube, materials, segments, bends, twins, duplicates, degenerate, welding, units and axes, components, groups, skipped nodes, cycles, pinched patch fallback) and the `tables.dae` fixture.
- `tests/kernel_props.rs` (proptest): random height-field grids with gaps, materials, diagonals, segments, twins, units and up axes: `validate` is clean, area per front and back material is preserved, a flat full single-material grid is one face.
- `tests/kernel_corpus.rs`: runs only with `MODELER_CORPUS` (the oracle sets it; you can't run it). Every corpus model converts in seconds; front plus back area per material matches what `flatten` draws (the corpus is exported two-sided); faces are far fewer than triangles; small bodies validate with no violations other than `EdgesIntersect`.
