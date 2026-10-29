# m7-import-stl: read STL files into the kernel

Implement `app/crates/modeler-mesh/src/stl.rs` (`read`, `StlError`'s `Display`) and
`to_kernel` / `import_stl` in `app/crates/modeler-mesh/src/lib.rs` (skeletons on main).
Change only those two files. Acceptance tests: `app/crates/modeler-mesh/tests/stl_read.rs`,
`tests/stl_kernel.rs` and `tests/stl_props.rs`.

STL is the usual file for 3D printing: a list of triangles, no units, no materials. This task
reads it; export (`m7-export-stl`) and the File menu items come later.

## The two STL forms

**Binary**: an 80-byte header (any bytes, ignored), a little-endian `u32` triangle count `n`,
then `n` records of 50 bytes: a normal (3 × `f32`, ignored), the three corners (3 × 3 ×
`f32`, x y z each), and a 2-byte attribute (ignored). All numbers little-endian.

**ASCII**: words separated by any whitespace; keywords are case-insensitive:

```
solid [name]
  facet normal nx ny nz
    outer loop
      vertex x y z
      vertex x y z
      vertex x y z
    endloop
  endfacet
  ...
endsolid [name]
solid ...            (more solids may follow)
```

## `stl::read(bytes, default_name)`

1. **Which form.** With `bytes.len() >= 84`, let `n` be the `u32` at offset 80. If
   `bytes.len() == 84 + 50 * n` the file is binary, whatever its header says (binary
   headers often start with `solid` too). Otherwise, if the bytes (after leading ASCII
   whitespace) start with `solid` in any case, it is ASCII. Otherwise, with
   `bytes.len() >= 84` it is `Truncated { expected: 84 + 50 * n, actual: bytes.len() }`,
   and with fewer bytes `NotStl`. Compute `84 + 50 * n` in `usize` without overflow
   (`n` up to `u32::MAX`).
2. **Binary**: one object named `default_name` holding the `n` triangles in file order.
   Corners are kept bit for bit (`f32`), including the corner order. Normals and attributes
   are ignored (the corner order defines the outside).
3. **ASCII**: an ASCII file that isn't valid UTF-8 is `NotStl`. Parse line by line, keeping
   1-based line numbers (a final newline doesn't start another line).
   - `solid` starts an object; its name is the rest of that line after the keyword,
     trimmed; empty → `default_name`. The object ends at `endsolid` (anything after it on
     that line is ignored) or at end of file.
   - Inside an object, each `facet` must be followed by `normal` and three numbers, then
     `outer loop`, exactly three `vertex x y z` lines, `endloop` and `endfacet`. Keywords
     may share a line or be split across lines as long as the order holds.
   - Numbers parse as `f32` with Rust's `str::parse` (so `1`, `-2.5`, `1e-3`, `4.0E+02`
     work). A number that doesn't parse, or isn't finite (`nan`, `inf`), is a syntax error.
   - Anything out of order is `Syntax { line, message }` with the line of the word where
     the error was found: a missing or unexpected keyword, a fourth `vertex` (its line), an
     `endloop` after fewer than three vertices (its line), text outside a solid. End of
     file inside a facet is a syntax error at the last line.
   - Blank lines anywhere are fine. A solid with no facets is an object with no triangles.
4. A file with zero triangles in total is still `Ok` (objects may be empty).

`StlError`'s `Display`: `not an STL file`, `binary STL is N bytes, expected M` (N actual,
M expected), `line L: <message>`.

## `to_kernel(objects, unit_meters)`

Build a `modeler_dae::Model` and convert it with `modeler_dae::to_kernel`, so welding,
merging coplanar triangles into faces, degenerate-triangle handling and validation are the
DAE import's:

- `unit_meters` as given, `up_axis: UpAxis::Z`, no materials, definitions or cameras;
- one `Mesh` per object that has at least one triangle (`id` and `name` = the object's
  name), holding one `Triangles` primitive: positions = the corners in order (three per
  triangle), indices `0..3n`, no normals, no texture coordinates, no material symbol;
- the scene node (identity transform, empty name) gets one child per such object, in
  object order: the object's name, identity transform, one `GeometryInstance` of its mesh
  with no bindings, no children, no instances.

`to_kernel` makes each child a group in the root. Objects with no triangles make no group.
Errors from `modeler_dae::to_kernel` are returned as they are.

## `import_stl(path, unit_meters)`

Read the file, `stl::read` it with the file stem as `default_name` (`"Mesh"` if the path has
no stem), then `to_kernel`. Errors are one line naming the file: `"<path>: <io error>"`,
`"<path>: <StlError>"`, or `"<path>: <ConvertError as {:?}>"`, with the path from
`Path::display`.

## Not in scope

3MF (`m7-import-3mf`), the File menu and units choice (`m7-import-ui`; callers pass
`unit_meters`, 0.001 for the usual millimeters), STL export.
