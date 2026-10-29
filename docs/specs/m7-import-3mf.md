# m7-import-3mf: read 3MF files into the kernel

Implement `app/crates/modeler-mesh/src/threemf.rs` (`read`, `ThreeMfError`'s `Display`).
`import_3mf` in `lib.rs` is already written on main; change only `threemf.rs`. Acceptance
tests: `app/crates/modeler-mesh/tests/threemf_read.rs` and `tests/threemf_kernel.rs`.
`miniz_oxide` (inflate) and `roxmltree` are already dependencies of the crate. Do not add
any other dependency (no zip crate).

3MF is the modern 3D-printing file: a ZIP archive whose model part is XML with units,
named objects, meshes, and a build list. This task reads it; export and the File menu come later.

## The archive (ZIP)

All numbers little-endian. Read the archive through its central directory:

1. Find the end-of-central-directory record: signature bytes `50 4b 05 06`, searched
   backwards from the last 22 bytes, at most 65535 bytes of trailing comment. Not found (or
   fewer than 22 bytes) → `NotZip`. It holds the entry count (`u16` at offset 10) and the
   central directory offset (`u32` at offset 16).
2. Each central record (`50 4b 01 02`) holds: compression method (`u16` at 10), compressed
   size (`u32` at 20), uncompressed size (`u32` at 24), name length (`u16` at 28), extra
   length (`u16` at 30), comment length (`u16` at 32), local header offset (`u32` at 42),
   then the name (UTF-8) at 46. The next record follows the name, extra and comment. A wrong
   signature or any read past the end of the bytes is `Zip("...")`.
3. An entry's data starts after its local header (`50 4b 03 04`; name length `u16` at 26,
   extra length `u16` at 28, data at `30 + name + extra`). Take sizes from the central record
   (data descriptors don't matter). Method 0 is stored; method 8 is raw deflate, inflated with
   `miniz_oxide::inflate::decompress_to_vec_with_limit` limited to the declared uncompressed
   size; any other method is `Zip("unsupported compression method N")`. Bad deflate data, a
   result whose length differs from the declared size, or a declared size over 1 GiB are
   `Zip(...)`. Ignore CRCs. Entry names match exactly (case-sensitive, no path clean-up).

## Finding the model

Default model part: `3D/3dmodel.model`. If the archive has `_rels/.rels`, parse it as XML (an
XML failure is `Xml`); the first `Relationship` element (any depth, namespace ignored) whose
`Type` ends with `/3dmodel` names the part in `Target`, with leading `/` characters removed.
The part must be an archive entry, otherwise `MissingModel`. Match all elements by local
name, ignoring namespaces.

## The model part

Parse with `roxmltree` (failure → `Xml(message)`); non-UTF-8 → `Xml`.

- **Unit**: the root's `unit` attribute, default `millimeter`: `micron` 1e-6, `millimeter`
  1e-3, `centimeter` 1e-2, `inch` 0.0254, `foot` 0.3048, `meter` 1. Anything else is
  `Model("unknown unit \"furlong\"")` (the name in `{:?}` quotes).
- **Objects**: every `object` element has an `id` (missing → `Model`). Its geometry is its
  `mesh` (`vertices` of `vertex x y z`, `triangles` of `triangle v1 v2 v3` indexing the
  vertices from 0) and/or its `components` (`component objectid [transform]`). Numbers that
  don't parse or aren't finite → `Model("bad number \"nan\"")`. A triangle index past the
  vertex list → `Model("triangle references vertex 1, mesh has 1")` (index, vertex count). Missing
  attributes on vertex / triangle / component / item → a `Model` message.
- **Transform**: 12 numbers `m00 m01 m02 m10 m11 m12 m20 m21 m22 m30 m31 m32`, row-vector
  convention: `x' = x·m00 + y·m10 + z·m20 + m30`, `y' = x·m01 + y·m11 + z·m21 + m31`, `z'`
  likewise with the third column. Absent → identity. Not 12 numbers → `Model("transform needs 12 numbers")`.
  Compute in `f64`; the output triangles are `f32`.
- **Output objects**: one `MeshObject` per `build` `item` (any depth), in order, even when
  two items name the same object. Name: the item's object's `name` attribute, or
  `"Object <id>"`. An `objectid` naming no object → `Model` (the message contains the id).
  Triangles: that object's mesh triangles, then its components' triangles, depth first in
  order, each component's matrix applied before its parent's, the item's last (point → component → … → item).
  A shared object reached twice is flattened twice. An object chain that reaches itself →
  `Model("component cycle")`.
- **Mirroring**: when a combined matrix has a negative determinant, swap each triangle's
  second and third corner so it still faces outward.
- **No items at all**: every object that no other object uses as a component becomes one
  output object, in file order, identity transform.
- Everything else in the file (colors, materials, metadata, thumbnails, other parts) is ignored.

`ThreeMfError`'s `Display`: `not a 3MF file`, `bad 3MF archive: <m>`, `3MF has no model part`,
`bad 3MF model XML: <m>`, `bad 3MF model: <m>`.

## `import_3mf(path)`

Already written: reads the file and calls `read`, then `to_kernel` with the file's unit.
Errors are one line `"<path>: <message>"`.

## Not in scope

The File menu and units override (`m7-import-ui`), 3MF export, production-extension
(multi-file) 3MF, colors and materials.
