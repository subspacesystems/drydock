# m4-texture-follow: texture placement moves with its faces (kernel)

Implement `texture::moved_maps` (skeleton on main, `app/crates/modeler-kernel/src/texture.rs`)
and call it from the operations that move faces:

- `transform::transform` (`src/transform.rs`);
- `movemerge::transform_sticky`, both its attached and its detached path (`src/movemerge.rs`);
- `component::make_component` (`src/component.rs`);
- `explode::explode` (`src/explode.rs`).

Tests: `tests/texture_follow.rs` in the kernel crate. Existing tests must keep passing.

## Why

A face's texture placement (`FaceMaterials::front_map` / `back_map`, see
`docs/specs/m4-texture-model.md`) is stored in the coordinates of the body that holds the
face. Today a move, rotate, scale or flip moves the face's points but leaves the map where it
was, so the image slides across the face (or, for a face on its default placement, is
projected afresh from the new position). Making a component re-bases the points into the
definition (minus its origin) without re-basing the maps, and exploding an instance puts the
points at `M·p` with the maps unchanged. The user expects the image to stay glued to the face
in all of these.

## `moved_maps(model, materials, front_normal, m) -> FaceMaterials`

`materials` belong to a face whose points move by the affine matrix `m`. `front_normal` is the
outward normal of the face's front side before the move (any length; the back side's normal is
its opposite). Returns `materials` with the two maps replaced:

- If `m` can't be inverted (`TextureMap::transformed` would return `None`: a non-finite entry
  or `|det| < 1e-12`), return `materials` unchanged.
- For each side:
  - a stored map `map` becomes `map.transformed(m)`;
  - no stored map, and the side's material exists in `model` and has a texture: the side's
    default placement before the move, `TextureMap::default_for(normal of that side)`,
    carried along: `default_for(..).transformed(m)`. It is now stored;
  - otherwise (no material, an untextured material, or an id the model doesn't have): no map
    (`None`). Untextured geometry never gains maps, so files that had none stay at their
    old format version.
- `front` and `back` (the material ids) are returned as they were.

After this, every point `p` of a side that had a stored map or a texture keeps its texture
point: `new.apply(m·p) == old.apply(p)`, where `old` is the side's map before the move
(stored or default) and `new` is the stored map afterwards.

## Where it is applied

In each operation, decide the moved faces' new materials from the state **before** any point
moves (that's where the normals come from), and record the change in the operation's own
`ModelEdit`, so it is part of the same undo step and undo/redo restore the maps exactly.
Only set materials that actually change.

### `transform` and the attached path of `transform_sticky`

A face moves with the matrix when every vertex of all its loops is among the moved vertices
(the same test the code already uses to decide which faces flip under a mirror). For each such
face: new materials = `moved_maps(model, old materials, normal before, matrix)`. Set them
before the faces are flipped and before re-sticking, so the flips and `restick` carry the new
maps to the faces they make (both already copy whole `FaceMaterials`).

A face that moves only in part (some of its vertices stay) keeps its map as it is: the image
stays put on the stretched face. Moving instances changes only instance transforms; maps inside
definitions are not touched.

A selection of edges only moves the faces they enclose completely, by the rule above.

### Detached path of `transform_sticky`

The faces it removes and rebuilds already remember their materials (`OldFace`). Store
`moved_maps(..)` of each face's materials there, with the face's normal before the move, so
the rebuilt faces get maps that followed.

### `make_component`

The definition's points are the selection's points minus `origin` (the box minimum), and the
instance is placed at `translate(origin)`. Each copied face's materials go into the definition
as `moved_maps(model, materials, normal in the parent, translate(-origin))`, so the image is
where it was when seen through the instance.

### `explode`

Each definition face is copied to `M·p` (`M` = the instance transform). The copy's materials
are the source's with the instance material on sides that have none (as now); then apply
`moved_maps(model, those materials, the source face's normal in the definition, M)`. So:

- stored maps and textured sides keep their texture points in the parent;
- a side painted only by the instance's material, when that material has a texture, gets the
  placement the instance showed: the definition face's default placement for that side,
  carried by `M`. (Drawing an instance's material on a face without its own uses the face's
  map in definition coordinates; `m4-texture-scene` relies on this.)

A mirrored instance already gets its copied faces reversed so the front stays outward; the
front's materials and map stay on the front. The later re-orientation step swaps maps along
with materials (`swap_materials`), which needs no change.

## Out of scope

Push/pull (the pushed face keeps its map as stored; follow-up if Tyrel wants it), drawing
(`m4-texture-scene`), the texture position tool (`m4-app-texture-position`).

## Tests (`tests/texture_follow.rs`)

- `moved_maps`: stored maps carried; a textured side without a map gets its default carried
  (back side: opposite normal); untextured, absent and unknown materials store nothing; a
  singular or non-finite matrix changes nothing; mirrors carry maps.
- `transform`: a textured cube moved by a rotate-scale-move, mirrored, or moved by selecting
  only its edges keeps every corner's texture point; untextured faces store no maps (also via
  `transform_sticky`); a face moved in part keeps its map; undo and redo restore the materials
  exactly; moving an instance leaves its definition's maps alone.
- `transform_sticky`: a free cube; a square moved until it shares an edge with another
  (re-stick runs; the other square stays unpainted); a rectangle pulled loose from a cube corner
  (detached path).
- `make_component`: a textured cube away from the origin keeps its texture points in definition
  coordinates; untextured faces store no maps.
- `explode`: turned and scaled, and mirrored, instances keep their texture points; a textured
  instance material lands with the placement it showed on both sides; a plain instance material
  stores no maps.
- Properties: random affine matrices (with mirrors) and random paint (untextured, textured
  default, textured with a moved image) for a sticky transform and for explode; make component
  then explode gives back the same texture points.
