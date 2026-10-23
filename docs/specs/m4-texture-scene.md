# m4-texture-scene: draw kernel textures; DAE import keeps texture placement

Change two existing functions in `app/crates/modeler-dae`:

- `kernel_scene` (`src/scene.rs`): textured materials are drawn with texture coordinates;
- `to_kernel` (`src/kernel.rs`): each imported face keeps the texture placement the file gave
  its triangles.

No new public API. The kernel's `TextureMap` (`modeler_kernel::texture`, see
`docs/specs/m4-texture-model.md`) already holds everything: `default_for`, `fit`, `apply`,
`uv`.

Tests: `tests/kernel_texture.rs` and `tests/kernel_texture_corpus.rs` (corpus test, runs when
`MODELER_CORPUS` is set; the oracle sets it). Two assertions in `tests/kernel_scene.rs`
changed with this task: a textured kernel material now reaches the scene with its path, and
`assert_facing` accepts a batch with one texture coordinate per position. Every other test
must keep passing.

## Why

Today an imported model is drawn textured until its first edit; then the app draws
`kernel_scene`, which drops every texture. After this task the kernel model carries each
face's placement and the kernel scene draws it, so an import looks the same before and after
edits, and materials painted in the app show their image.

## `kernel_scene`

### Materials

`Material::texture` in the scene is the kernel material's `texture.path` as a `PathBuf`
(`None` when the kernel material has no texture). Everything else stays as it is.

### Texture coordinates

A batch whose material has a texture has exactly one texture coordinate per position. Every
other batch (an untextured material, or the default batch) has none, as now.

Each side of a face is drawn into the batch of its material (the face's own material on that
side, else the inherited instance material, as now). When that material has a texture, the
side's vertices get texture coordinates `map.uv(texture, p)`, where

- `p` is the vertex in the face's own body coordinates (before the instance transforms), so
  the image moves, turns and scales with the instance;
- `map` is the side's stored map (`FaceMaterials::front_map` for the front,
  `back_map` for the back), else `TextureMap::default_for(n)` with `n` the side's outward
  normal in body coordinates: `polygon_normal` of the outer loop for the front, its negation
  for the back. This is the same rule as `texture::side_map`, and it applies also when the
  material comes from the instance (the face has no material on that side).

Store the coordinates as `f32` in the same order as the positions appended for that side.
Front and back of one face may go into the same batch (same material); each side's vertices
get that side's coordinates.

## `to_kernel`

### Texture coordinates of a triangle

A file triangle has texture coordinates when the material it is bound to has a texture and
its primitive has a texture coordinate for each of its three corners. Its texture-space
points are `st = uv × texture.size` per corner (import makes every texture 1 m × 1 m, so in
practice `st = uv`). Its map is `TextureMap::fit` of its three corner positions (in model
coordinates: meters, Z up, the same welded positions the faces are built from) and its three
`st`. A triangle without texture coordinates, or whose fit is `None`, has no map.

When a reversed twin gives a triangle its back material (as now), the twin's map becomes the
triangle's back map. A triangle's own map is its front map.

### Merging triangles into faces

Two neighbouring triangles may join the same face (all the existing rules still hold) only
when they look the same:

- the same front material and the same back material, and
- for each side, both have no map, or both have one and the maps agree: at each of the six
  corners `p` of the two triangles, `x = a.apply(p)` and `y = b.apply(p)` differ by at most
  `1e-3 × max(1, |x.s|, |x.t|, |y.s|, |y.t|)` in `s` and in `t`.

So a seam in the texture (coordinates that jump between two triangles) keeps them apart, a
textured triangle never merges with an unplaced one, and noise far below a pixel doesn't
split a face.

### The face's materials

A face built from a patch gets the patch's first triangle's materials and maps. A triangle
that ends up as its own face (the fallback when a patch can't become one face) keeps its own.
As before, nothing is stored for a face whose `FaceMaterials` is the default.

## Corpus check

`tests/kernel_texture_corpus.rs`: for each corpus model, the kernel scene of the imported
model has the same texture path per material as `flatten`, a coordinate for every vertex of
each textured batch (none elsewhere), all finite; and for at least 97 % of the textured
vertices `flatten` draws, the kernel scene has a vertex of the same material at the same
place (within `1e-5` of the model size) with the same texture coordinates (within
`1e-3 × max(1, |uv|)`). The share is not 100 % because the exported files hold some
duplicate triangles (the same corners twice, with different materials or coordinates), and
the kernel keeps one of them. Measured with a reference implementation: 98.3 % to 100 % per
model.

## Out of scope

Drawing changes in `modeler-render` (it already draws `texcoords` with the material's image),
positioning textures in the UI (`m4-app-texture-position`), export.
