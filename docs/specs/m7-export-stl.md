# m7-export-stl: binary STL writer

Writes the print mesh (`m7-print-mesh`) as binary STL. Pure functions in
`modeler-mesh/src/stl_write.rs`; the skeleton (signatures, doc comments, errors) is on main.
Acceptance tests: `app/crates/modeler-mesh/tests/stl_write.rs`.

## Format
- 80-byte header, u32 little-endian triangle count, then 50-byte records: normal (3 x f32),
  three corners (9 x f32), attribute u16 = 0.
- STL has no unit: write the values as given. The print mesh is already in mm.
- Header: `"modeler STL: "` followed by the object name, cut at 80 bytes on a character
  boundary, padded with zeros. It must never start with `solid`, because other readers
  take that for ASCII.
- Normal: `(b - a) x (c - a)` in f64, normalized, then cast to f32. A degenerate
  triangle (zero-length cross product) gets a zero normal. Corners are cast f64 -> f32.
- Corner order is kept as given.

## `encode(name, triangles) -> Vec<u8>`
The bytes above. An empty list gives a valid 84-byte file (the callers refuse empty input).

## `sanitize_name(name) -> String`
Letters and digits (any script, `char::is_alphanumeric`), `-`, `_` and space are kept; every other
character, including `.`, `/` and `\`, becomes `_`. Trim spaces at both ends, keep at most 100
characters, and use `"part"` if nothing is left.

## `write_separate(objects, dir) -> Result<Vec<PathBuf>, StlWriteError>`
- Objects with no triangles are skipped. If none has any, return `NothingToWrite` before
  touching the disk (no folder, no file).
- Create `dir` and its parents. Each remaining object goes to `<stem>.stl`; the stem is
  `sanitize_name(name)`. If the stem (compared ignoring case) is already used by an earlier
  object, try `<base>-2`, `<base>-3`, ... until one is free, so no two files share a name
  even on a case-insensitive disk. Overwrite existing files.
- Returns the paths in object order (without the skipped ones). Disk errors become
  `Io(message)`.

## `write_combined(objects, path) -> Result<(), StlWriteError>`
All triangles of all objects, in object order, in one file; the header carries the name of the
first object that has triangles. `NothingToWrite` as above (no file). Disk errors are `Io`.
The parent folder is not created.

## Out of scope
ASCII STL, colors (the attribute stays 0), units choice and curve tolerance (`m7-export-ui`),
the file dialogs, 3MF.

## Where to look
- `show encode`, `show sanitize_name`, `show write_separate`, `show write_combined`
  (skeletons in `stl_write.rs`).
- `show PrintObject` and `show Triangle` in `print_mesh.rs` (`Triangle = [DVec3; 3]`, mm).
- `stl.rs`: `read_binary`, `binary_len` for the layout the reader expects.
- Tests: `print_common::{box_group, place, volume, bounds}` and `modeler_mesh::import_stl` for the round trip.
