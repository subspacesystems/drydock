# m4-texture-model: texture size and per-face texture placement (kernel)

Implement the skeletons on main:

- `TextureMap`, `side_map` and `set_texture_maps` in `app/crates/modeler-kernel/src/texture.rs`;
- the new checks in `Model::apply` (`app/crates/modeler-kernel/src/model.rs`) and in
  `paint::create_material` / `paint::edit_material`;
- native format version 3 in `app/crates/modeler-kernel/src/format.rs`, and `docs/format.md`
  to match.

Tests: `tests/texture.rs` and `tests/texture_format.rs` in the kernel crate. Existing tests
must keep passing (main already relaxed the two "version is 2" tests and moved the
"unsupported version" example to 99).

## Why

A material can carry an image. To draw it, every point of a face needs texture
coordinates, and the user needs to place the image on a face (move, turn, scale it). Imported
models bring their own coordinates, which today are lost on the first edit. This task adds
the data and the math; drawing (`m4-texture-scene`) and the UI come later.

## Data (already on main)

- `Material::texture: Option<Texture>`; `Texture { path, size }`. `size` is the width and
  height in meters that one copy of the image covers. `Texture::from(&str)` is a 1 m × 1 m
  texture.
- `FaceMaterials` gains `front_map` and `back_map: Option<TextureMap>`. `None` is the default
  placement. Because maps live in `FaceMaterials`, everything that already copies a face's
  materials to new faces (splits, push/pull, moves) copies the maps too; nothing to change
  there. A `FaceMaterials` is the default (no entry stored) only when all four fields are
  `None`.

## `TextureMap`

`u`, `v` are rows of an affine map from body coordinates (meters) to texture space (meters):
`s = u.x·p.x + u.y·p.y + u.z·p.z + u.w`, likewise `t` with `v`. Texture space: `(0, 0)` is the
image's bottom left corner, `s` goes right, `t` up, and the image repeats every
`size.x` in `s` and `size.y` in `t`.

- `apply(p)`: `(s, t)`.
- `uv(texture, p)`: `apply(p) / texture.size` (component-wise): the texture coordinates
  (1 = one image width or height; bottom left origin, `v` up).
- `then(a)`: the map `p ↦ a(apply(p))` (`a` is a `DAffine2` acting on `(s, t)`). This is how
  the image is moved, turned and scaled on a face.
- `transformed(m)`: the map for geometry moved by `m`, so that `transformed(m).apply(m·p)`
  equals `apply(p)`: rows times `m⁻¹`. `None` when `m` has a non-finite entry or
  `|det m| < 1e-12`.
- `is_finite()`: all eight numbers finite (on main).

### Default placement: `default_for(normal)`

The image seen from the side the normal points to stands upright and reads left to right:

1. `n` = `normal` normalized. If `normal` is zero or not finite, use `+z`.
2. `h = Z − (Z·n) n` (`Z` = `+z`). If `|h| ≥ 1e-6`, the up direction is `V = h / |h|` (the
   steepest way up the plane). Otherwise the face is horizontal and
   `V = normalize(Y − (Y·n) n)` (`Y` = `+y`).
3. `U = V × n` (right, seen from the front of that side).
4. `u = (U, 0)`, `v = (V, 0)`: the body origin maps to `(0, 0)`.

Examples: `+z` gives `u = (1,0,0,0)`, `v = (0,1,0,0)`; `−z` gives `u = (−1,0,0,0)`,
`v = (0,1,0,0)`; `+x` gives `u = (0,1,0,0)`, `v = (0,0,1,0)`; `−y` gives `u = (1,0,0,0)`,
`v = (0,0,1,0)`. Always `U`, `V` orthonormal, both perpendicular to `n`, `U × V = n`.

### Fitting: `fit(points, st)`

For imports: the map that best sends the points of one planar face to their given texture
points.

- `None` when `points.len() != st.len()`, there are fewer than 3 points, any number is not
  finite, `geom::polygon_normal(points)` is zero (for example all points on one line), or
  the least-squares system below is singular.
- The plane: normal `n = polygon_normal(points)`. Take an orthonormal basis `e1`, `e2` of the
  plane and coordinates `a_i = (p_i − p0)·e1`, `b_i = (p_i − p0)·e2` (`p0 = points[0]`).
- Solve least squares for `s ≈ α a + β b + γ` and the same for `t`. The returned map has
  `u.xyz = α e1 + β e2` and `u.w = γ − u.xyz·p0` (same for `v`), so it ignores how far a
  point is off the plane (`u.xyz·n = 0`).
- When the data is exactly affine (as for any planar face exported with a texture), the fit
  reproduces it (to about `1e-9` relative).

## `side_map(model, body, face, side)`

The stored map for that side when there is one, else `default_for` that side's outward
normal: the face normal `geom::polygon_normal` of its outer loop's points for `Side::Front`,
its negation for `Side::Back`. `None` when the body or the face doesn't exist.

## `set_texture_maps(model, body, changes, undo, label)`

Each change `(face, side, map)` stores `map` (`None`: back to the default) on that side,
keeping the face's materials and its other side's map. When the same face and side appear
more than once, the last one counts. Returns the number of `(face, side)` pairs whose stored
value changed; pushes one undo step named `label` when that is more than zero, nothing
otherwise.

Errors, checked in this order (model and undo stack unchanged):

- `PaintError::Empty` for no changes;
- `PaintError::Model(ModelError::MissingBody(body))`;
- `PaintError::Missing(Entity::Face(f))` for the first change, in order, whose face the body
  doesn't have;
- `PaintError::Model(ModelError::BadTextureMap)` when a map isn't finite.

## Model checks

- `AddMaterial` and `SetMaterial` records whose `to` material has a texture with a size
  entry that is not finite or not `> 0`: `ModelError::BadTexture`.
- `SetFaceMaterials` with a `to` map that isn't finite: `ModelError::BadTextureMap`.
- `paint::create_material` and `paint::edit_material` return
  `PaintError::Model(ModelError::BadTexture)` for such a texture (checked first after the
  color; nothing changes, no undo step). Note `Model::add_material` returns an id even when
  the record is refused; don't rely on it there.

## Native format version 3

`FORMAT_VERSION` becomes 3. Readers still read 1 and 2.

Header: the writer uses the lowest version that holds the model:
`3` when a material's texture size isn't exactly `(1, 1)` or an existing face has a stored
map (`front_map` or `back_map`); else `2` when a body has guides; else `1`. So every file an
older version wrote is still written byte for byte the same.

Records in version 3:

- `material <id> <r> <g> <b> <a> <name> -` (no texture, 8 tokens), or
  `material <id> <r> <g> <b> <a> <name> <texture> <width> <height>` (texture a quoted
  string, 10 tokens). Any other shape is `BadField`. A size the model refuses is
  `Rejected(BadTexture)`.
- In versions 1 and 2 the material record is unchanged (8 tokens) and a texture's size reads
  as `(1, 1)`.
- `texmap <face id> <side> <u.x> <u.y> <u.z> <u.w> <v.x> <v.y> <v.z> <v.w>` (11 tokens),
  `side` bare `front` or `back`: sets that side's map of the face in the current body,
  keeping the rest of its `FaceMaterials`. Before any `body`: `NoBody`. The face must exist:
  else `Rejected(Topo(MissingFace(id)))`. In versions 1 and 2, `UnknownRecord`. Another side
  word: `BadField`.

Writer, inside each body: `vertex`, `edge`, `face`, `paint`, `texmap`, `instance`, `guide`.
`paint` is written for existing faces with a material on either side (as before, since
entries without maps never had both `None`). `texmap` is written for existing faces, ascending
face id, `front` before `back`, one line per stored map. Numbers print as the other reals do
(shortest round-trip form), so the round trip is exact.

Update `docs/format.md`: the version history, the header rule, the two record shapes and
the writer order.

## Not in scope

- Moving maps along when faces are moved, rotated, made into components or exploded
  (`m4-texture-follow`).
- Drawing textures and keeping imported coordinates (`m4-texture-scene`), saving texture
  files elsewhere (`m4-texture-save`), the UI.
