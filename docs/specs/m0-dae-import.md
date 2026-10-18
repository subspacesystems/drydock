# m0-dae-import: COLLADA 1.4 reader

Milestone M0. Crate `app/crates/modeler-dae`. Implement `import_str`, `import_file` (already written) and `flatten` behind the API in `src/lib.rs` and `src/flat.rs`. Don't change the public types; add private modules freely.

Source of truth for the format: the Khronos COLLADA 1.4.1 specification. Work from it and from this spec only.

## Dependencies
`roxmltree` for XML (already in `Cargo.toml`), `glam` for math. No others.

## Reading (`import_str`)

Namespace `http://www.collada.org/2005/11/COLLADASchema`; also accept documents with no namespace. Root must be `<COLLADA>`, else `ImportError::Invalid`. Malformed XML is `ImportError::Xml`.

**Asset.** `<asset><unit meter="..">` sets `unit_meters` (default 1.0). `<asset><up_axis>` is `X_UP`, `Y_UP` or `Z_UP` (default `Y_UP`).

**Geometry.** Each `<library_geometries><geometry>` becomes one `Mesh` (id, name; missing attributes become empty strings), in document order. Only `<mesh>` content is read; other geometry types (splines, etc.) give a mesh with no primitives. A `<mesh>` may be empty (`<mesh/>`) or have no `<vertices>`: that's a mesh with no primitives, not an error.

- `<source>` holds a `<float_array>` and a `<technique_common><accessor count stride>`; element *i* is `stride` floats starting at `offset + i*stride` (accessor `offset` default 0). Read the first 3 (positions, normals) or 2 (texture coordinates) params of each element.
- `<vertices>` has inputs: `POSITION` (required), and optionally `NORMAL` and `TEXCOORD`. These apply to every primitive that uses the `VERTEX` semantic.
- Primitives: `<triangles>`, `<lines>`, and `<polylist>` (with `<vcount>`; each polygon with n >= 3 vertices becomes n-2 triangles as a fan from its first vertex; polygons with fewer than 3 are skipped). `<polygons>`, `<linestrips>`, `<tristrips>`, `<trifans>` are not needed: skip them.
- Each primitive input has `semantic`, `source`, `offset` and optional `set`. The `<p>` list is interleaved: each vertex uses (max offset + 1) integers, and input *k* reads the integer at its offset. Inputs: `VERTEX` (expands to the `<vertices>` inputs), `NORMAL`, `TEXCOORD` (use the input with the lowest `set`, or the first if none has a set). Ignore other semantics.
- Build `Primitive` re-indexed: one output vertex per distinct combination of (position index, normal index, texcoord index) in first-use order; `indices` point at those. `normals`/`texcoords` are empty when the primitive has none.
- An index outside its source, or a `<p>` whose length isn't a whole number of primitives, is `ImportError::Invalid` naming the geometry id.
- `material` attribute -> `material_symbol`.

**Materials.** Each `<library_materials><material>` becomes one `Material`, in document order, using the effect its `<instance_effect url>` names. From the effect's `<profile_COMMON><technique>`, use the first shading element present (`lambert`, `phong`, `blinn` or `constant`):

- `color`: the `<diffuse><color>` RGBA (for `constant`: `<emission><color>`). If the diffuse is a `<texture texture="X">`, color is white (1,1,1,1) and `texture` is the image found by following X -> `<newparam sid=X><sampler2D><source>S` -> `<newparam sid=S><surface><init_from>IMG` -> `<library_images><image id=IMG><init_from>path`. Also accept a `<texture>` whose attribute names an image id directly. Defaults when absent: color (0.8, 0.8, 0.8, 1), no texture.
- `texture` path: percent-decode it (`%20` -> space), strip a `file://` prefix, and if relative join it to `base_dir`. Don't check that the file exists.
- `opacity` (COLLADA common profile rules): with `<transparent opaque="A_ONE">` (the default mode) opacity = alpha of the transparent color x transparency; with `opaque="RGB_ZERO"` opacity = 1 - luminance(transparent rgb) x transparency, where luminance = 0.212671 r + 0.715160 g + 0.072169 b. `<transparency><float>` defaults to 1. With no `<transparent>` element, or when `<transparent>` holds a `<texture>` (alpha comes from the image at render time), opacity = 1. Clamp to 0..1.
- `unlit`: true for `constant`.

**Scene.** `<scene><instance_visual_scene url>` names the visual scene (if absent, use the first `<visual_scene>`; with no visual scene at all the root has no children). `Model::scene` is a root `Node` (identity transform, name = the visual scene's name or id) whose children are the scene's top-level `<node>`s.

- `Node.transform`: product, in document order, of the node's `<matrix>` (16 floats, row-major as written), `<translate>`, `<rotate>` (axis x y z, angle in degrees), `<scale>`. No transform elements = identity.
- `Node.name`: `name` attribute, else `id`, else empty.
- `<instance_geometry url="#id">` -> `GeometryInstance` with the mesh index and `bindings` from `<bind_material><technique_common><instance_material symbol target="#matid">`. An unknown geometry id is `Invalid`; an unknown material target is `Invalid`.
- `<instance_node url="#id">` -> index into `definitions`. Definitions are the direct `<node>` children of every `<library_nodes>`, in document order; nested nodes inside a definition are part of that definition. An unknown id is `Invalid`. A definition that (directly or indirectly) instances itself is `Invalid`.
- `<instance_camera url>` inside a scene node -> a `ViewCamera` with the node's world transform (product of transforms from the root, in file units/axes) and its `<optics><technique_common>` `<perspective><yfov>` (degrees; if only `xfov` is given use it) or `<orthographic><xmag>` (if only `ymag`, use it). Cameras in the order met, depth first.
- `<instance_light>`, `<extra>` and anything else: ignore.

## Flattening (`flatten`)

World space is meters, Z up, right-handed. A file point p maps to `C * W * p`, where W is the product of node transforms from the scene root down (through instanced definitions) and C converts the file frame:

- `Z_UP`: (x, y, z) -> (x, y, z)
- `Y_UP`: (x, y, z) -> (x, -z, y)
- `X_UP`: (x, y, z) -> (-y, -z, x)

then multiplies by `unit_meters`.

- Every instance is expanded. A definition instanced 5 times contributes its triangles 5 times. Never drop triangles, including degenerate ones: the triangle count must equal the sum over all expanded primitives.
- One `Batch` per material actually used (and one for `None` if any triangles have no bound material), ordered by material index with `None` last. A primitive's material is its instance's binding for its `material_symbol`; no symbol or no binding = `None`. `FlatScene::materials` is a copy of `Model::materials` (same indices).
- Normals: transform by the inverse transpose of the full transform and normalize. If a primitive has no normals, give each of its triangles its own three vertices carrying the triangle's face normal (right-hand rule, after transforming).
- Mirroring: when the full transform has a negative determinant, swap the 2nd and 3rd index of each triangle so the winding still matches the normals.
- `texcoords`: present (same length as positions) when the batch's material has a texture; vertices from primitives without texture coordinates get (0, 0). Empty when the material has no texture.
- Lines go to `LineBatch`es the same way (per material, `None` last), two positions per segment.
- `bounds`: min/max over every triangle and line vertex; `None` when there are none.

## Tests (fixed; don't edit)
- `tests/import.rs`, `tests/flatten.rs` with fixtures in `tests/fixtures/`.
- `tests/corpus.rs` runs only when `MODELER_CORPUS` is set (the test oracle sets it); it checks counts, bounds, materials and texture paths on real exported models. You can't run it locally; the fixtures cover the same constructs.

## Out of scope
Animation, skinning, controllers, physics, lights, `<polygons>` with holes, and building the editable geometry model (that's M1).
