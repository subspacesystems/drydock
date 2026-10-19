# m1-kernel-scene: draw kernel models

Milestone M1, kernel task. Two files:
- `app/crates/modeler-kernel/src/tess.rs`: implement `triangulate`.
- `app/crates/modeler-dae/src/scene.rs`: implement `kernel_scene`.

Don't change public signatures or public types; add private helpers and private modules freely. Dependencies: what the two crates already have (`glam`, std; `modeler-dae` also has `modeler-kernel`). No panics on any input. You may make private helpers in `modeler-dae/src/flat.rs` `pub(crate)` to reuse them.

Why: the app renders `FlatScene`s. With `kernel_scene`, a model loaded from the native file (or converted from a DAE with `to_kernel`) can be drawn by the existing renderer.

## `triangulate(outer, holes) -> Vec<[usize; 3]>`
Points are numbered in order: `outer[0..n0]` are `0..n0`, then the first hole's points, and so on.

Valid input (what a valid kernel face gives): `outer` is a simple polygon with at least 3 points, either winding. Each hole is a simple polygon with at least 3 points, either winding, strictly inside `outer`; holes don't touch each other or `outer` and aren't nested. Consecutive points may be collinear (an edge split by a vertex gives a 180 degree corner), any number in a row.

For valid input with `n` points in total and `h` holes, the result:
1. has exactly `n + 2h - 2` triangles, each made of three distinct indices `< n`, and uses every point;
2. every triangle is counter-clockwise: its signed area is `>= -1e-9 * d * d`, where `d` is the diagonal of the bounding box of `outer` (flat triangles are allowed where points are collinear);
3. the triangles' areas add up to the area of `outer` minus the holes (relative error `<= 1e-9`);
4. covers exactly the region: every point inside `outer` and outside every hole, farther than `1e-6 * d` from every polygon edge, lies in at least one triangle; every point outside `outer` or inside a hole, that far from every edge, lies in none.

(2 to 4 together mean the triangles tile the region without overlaps.)

Other input: never panic, never return an index `>= n`. `outer` with fewer than 3 points gives no triangles. A hole with fewer than 3 points is ignored (its points are unused, and it doesn't count in `h`).

Speed: a 1000-point outer polygon with 10 holes of 40 points each must triangulate in well under a second in a debug build (the test allows 5 s). So no `O(n^3)` work; ear clipping with holes joined to the outer polygon by bridge edges (`O(n^2)`) is fine.

## `kernel_scene(model) -> FlatScene`
### Materials
`materials[i]` is the i-th kernel material in id order (`model.materials()`), converted to a document `Material`: `id` is the decimal kernel id (`"7"`), `name` the kernel name, `color` is `[r, g, b, 1.0]`, `opacity` is the kernel `a`, `texture: None` (kernel faces have no texture coordinates yet, so textured materials draw in their color), `unlit: false`. A batch's `material` is the index `i`.

### Walk
Bodies are expanded depth first from the root with `world = DMat4::IDENTITY` and inherited material `None`. In each body: its faces in id order, then its edges in id order, then its instances in id order. An instance expands its definition's body with `world * instance.transform` and inherited material `instance.material.or(inherited)` (the innermost instance with a material wins). Stop expanding below 64 levels of nesting (a valid model has no cycles, but never recurse without bound). An instance of a definition that doesn't exist is skipped.

### Faces
For a face with outer loop points `P` (from `loop_points`) and hole points `H1, H2, ...`, in the body's coordinates:
- `n = polygon_normal(P)`. If `n` is zero, or any point after `world.transform_point3` is not finite, skip the face.
- Project every loop with `to_plane_2d(_, n)`, `triangulate(outer_2d, holes_2d)`. The triangles are counter-clockwise seen from the `+n` side.
- Positions: `world.transform_point3(p)` as `f32`, for the outer points then the hole points (no new points, so `positions.len()` grows by the face's point count per drawn side).
- World normal `m`: `n` through the inverse transpose of `world`'s upper 3x3, normalised. If `world`'s upper 3x3 has a negative determinant (a mirror), reverse each triangle (swap its last two indices) so it stays counter-clockwise seen from `m`.
- Materials: `front = face_materials.front.or(inherited)`, `back = face_materials.back.or(inherited)`. A material id that the model doesn't have counts as `None`.
- Front side: the triangles, with every normal `m`, go into the batch of `front`.
- Back side: only when `back` is `Some`: the same points again with every triangle reversed and every normal `-m`, into the batch of `back`. (When `back` is `None` the renderer draws the front triangles' back sides.)

### Edges
Each edge becomes one segment from `ends[0]` to `ends[1]`, both through `world` as `f32`, in a single `LineBatch` with `material: None`. Skip a segment with a non-finite end.

### Output
- `batches`: one per material index that got triangles, in increasing index order, then the `None` batch if it got triangles. No empty batches. `texcoords` stay empty. Within a batch, faces appear in walk order.
- `lines`: the one line batch, or empty when there are no segments.
- `bounds`: min/max over every position in `batches` and `lines`, `None` when both are empty (same rule as `flatten`).

## Tests (fixed; don't edit)
- `modeler-kernel/tests/tess.rs`: triangles, squares, collinear runs, concave shapes (L, comb, spiral), clockwise input, one and several holes, holes next to concave corners, degenerate input, a speed case.
- `modeler-kernel/tests/tess_props.rs` (proptest): random star-shaped polygons with extra collinear points and random holes; checks 1 to 4 above.
- `modeler-dae/tests/kernel_scene.rs`: models built in code (one face, materials front and back, holes, instances with translation, rotation, non-uniform scale and mirror, nested instance materials, missing definitions, edges, bounds, order of batches) and the `tables.dae` fixture through `to_kernel`: area per material equals `flatten`'s (except the default material).
- `modeler-dae/tests/kernel_scene_corpus.rs`: runs only with `MODELER_CORPUS` (the oracle sets it; you can't run it). For every corpus model, `kernel_scene(to_kernel(..))` draws the same area per material as `flatten` and has the same bounds.
