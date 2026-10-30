# m7-export-3mf: 3MF writer

Implement `app/crates/modeler-mesh/src/threemf_write.rs` (`crc32`, `zip_stored`, `encode`,
`write`; types and skeleton are on main). Change only that file. Acceptance tests:
`app/crates/modeler-mesh/tests/threemf_write.rs`, with fixtures from `tests/print_common`.

Input is the `PrintObject`s from `print_mesh` (millimeters, world space, Z up). No new
dependencies: the ZIP is written by hand, entries stored (no compression), CRC-32 computed
by your own table-driven function.

## `crc32(bytes) -> u32`
Standard ZIP CRC-32 (reflected, polynomial 0xEDB88320, init and final xor 0xFFFFFFFF).

## `zip_stored(entries) -> Vec<u8>`
Local file header (`PK\x03\x04`), data, for each entry in order; then the central directory
(`PK\x01\x02`), then the end record (`PK\x05\x06`, 22 bytes, no comment). Per entry: version
needed and made-by 20, flags 0 (no data descriptor), method 0, time 0, date 0x0021
(1980-01-01), CRC-32 and sizes filled in both headers, no extra field, no comment, attributes
0. Entry names are UTF-8 as given. Output depends only on the input.

## `encode(objects)`
1. Keep objects that have at least one usable triangle. A triangle is unusable when a
   corner is not finite, or after welding two of its corners share a vertex. None left:
   `NothingToWrite`.
2. Per object, convert each corner to `f32` and weld corners with identical `f32` values
   (`-0.0` equals `0.0`) into one vertex list, numbered in order of first use. Triangles
   keep their order and corner order (the winding).
3. Colors: quantize each `Some(color)` to `#RRGGBBAA` (uppercase hex; each component
   `(c * 255).round()` clamped to 0..=255, NaN as 0). Distinct strings, in order of first
   use, become the `<base>` children of one `<basematerials>`; none colored means no
   `<basematerials>` at all.
4. The package has exactly three entries in this order:
   - `[Content_Types].xml`: namespace `http://schemas.openxmlformats.org/package/2006/content-types`,
     `Default` for `rels` (`application/vnd.openxmlformats-package.relationships+xml`) and
     `model` (`application/vnd.ms-package.3dmanufacturing-3dmodel+xml`).
   - `_rels/.rels`: one `Relationship` with `Target="/3D/3dmodel.model"`, `Id="rel0"`,
     `Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"`.
   - `3D/3dmodel.model`: XML declaration, then
     `<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">`
     with `<resources>` and `<build>`. Resource ids: the `<basematerials id="1">` first
     when present, then the objects, numbered consecutively after it (from 1 when there are
     no colors). Each `<object id type="model" name>` holds `<mesh>` with `<vertices>`
     (`<vertex x y z/>`, f32 `Display` text) and `<triangles>` (`<triangle v1 v2 v3/>`),
     plus `pid` (the basematerials id) and `pindex` (index of its base) when it has a
     color. `<build>` has one `<item objectid>` per object, no transform. An empty name
     is written as `Object`; names are XML-escaped (`& < > " '`).

## `write(objects, path)`
`encode`, then write the bytes to `path` (parent folder not created). `NothingToWrite`
leaves no file; a disk error is `Io` with the system's message.

## Out of scope
Compression, the units choice and curve tolerance (`m7-export-ui`), file dialogs, STL.

## Where to look
- `show zip_parts` and `show entry_data` in `threemf.rs` (what the reader expects of the
  archive), `show parse_model_part` for the model XML it accepts.
- `show PrintObject` and `show Triangle` in `print_mesh.rs` (`Triangle = [DVec3; 3]`, mm).
- `stl_write.rs`: `write_combined` for the file-writing and error-mapping pattern.
- Tests: `print_common::{box_group, cylinder_group, place, volume, bounds}`,
  `modeler_mesh::import_3mf` for the round trip.
