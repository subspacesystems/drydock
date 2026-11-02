# Native model file, version 6

The native file stores a whole `Model` (`drydock-kernel`, `src/model.rs`): materials,
definitions, and for the root and every definition its geometry (vertices, edges, faces,
curves),
face materials, instances and guides, plus the model's geolocation, shadow settings and length format. Code: `drydock_kernel::format`. Owner: orchestrator.

Design goals: exact round trip (ids and every number come back bit for bit), readable and
diffable, no dependencies, easy to extend. Lengths are meters.

## Text
- UTF-8, one record per line. Lines end with `\n` (a `\r\n` ending is read the same).
- A line is a list of tokens separated by spaces or tabs. A token is either bare (no spaces,
  tabs or `"`) or a quoted string.
- Quoted string: `"` ... `"`, followed by a space, a tab or the end of the line. Inside,
  `\\` is a backslash, `\"` a quote, `\n` `\r` `\t` the usual control characters, and
  `\u{H}` the character with hex code `H` (1 to 6 hex digits). Any other `\` sequence is an
  error. Every other character stands for itself. Writers escape `\`, `"`, newline,
  carriage return and tab as above, and the other characters U+0000 to U+001F and U+007F
  as `\u{h}` (lowercase hex, no leading zeros).
- Blank lines (only spaces or tabs) and lines whose first non-blank character is `#` are
  ignored everywhere except the first line.
- Ids are decimal `u64`. Real numbers are decimal floating point (`f64`, colors `f32`) and
  must be finite. Writers print the shortest text that reads back as the same number
  (Rust's `Display` for `f64`/`f32`, so `1`, `0.5`, `-0`, `0.1`).
- `-` (bare) means "none" where a material or texture may be absent.

## Records
The first line is the header `drydock-model 1` (the word, one space, the version). A reader
also accepts the older word `modeler-model` with the same version, and refuses a file whose
first line is neither `drydock-model <u32>` nor `modeler-model <u32>`, or a version it
doesn't know. Writers always write `drydock-model`. The rename does not change the version
number: a model that needed version 4 before still says version 4. Native files are
`.drydock`; older `.modeler` files still open.

Then, one per line:

| Record | Meaning |
|---|---|
| `material <id> <r> <g> <b> <a> <name> <texture>` | sRGB color with straight alpha (alpha is opacity); `name` is a string; `texture` is a string (image path relative to the model file) or `-`; in version 3 a texture is followed by its width and height in meters |
| `material <id> <r> <g> <b> <a> <name> <texture> <width> <height>` | (version 3) the material's texture and the size one copy of it covers, in meters; both finite and `> 0` |
| `definition <id> <kind> <name>` | `kind` is `component` or `group`; the new definition's body is empty |
| `body root` / `body <definition id>` | the following geometry, paint and instance records belong to this body; each body appears at most once |
| `vertex <id> <x> <y> <z>` | a vertex |
| `edge <id> <v1> <v2>` | an edge from `v1` to `v2` (order kept) |
| `face <id> <loop> [\| <loop>]...` | the first loop is the outer loop, each one after a `\|` token is a hole; a loop is a list of edge uses `+<edge id>` (forward) or `-<edge id>` (reversed), possibly empty |
| `curve <id> <cx> <cy> <cz> <nx> <ny> <nz> <xx> <xy> <xz> <radius> <start> <end> <v0> <v1> ...` | (version 6) an arc or circle kept as such (`drydock_kernel::curve::Curve`, `docs/specs/m7-curves.md`): center, unit plane normal, unit x axis, radius, start and end angles in radians, then the vertex ids of its chain in order (at least 3; a circle ends with its first id again) |
| `paint <face id> <front> <back>` | the face's materials, each a material id or `-` |
| `texmap <face id> <side> <u.x> <u.y> <u.z> <u.w> <v.x> <v.y> <v.z> <v.w>` | (version 3) the texture map of one side of the face in the current body; `side` is `front` or `back`; the face must exist |
| `instance <id> <definition> <material> <m0> ... <m15> <name>` | a placed definition; `material` is an id or `-`; the 16 numbers are the transform in column-major order (`DMat4::to_cols_array`); `name` is a string |
| `guide <id> line <px> <py> <pz> <dx> <dy> <dz>` | (version 2) an infinite guide line through `p` with unit direction `d` |
| `guide <id> point <x> <y> <z>` | (version 2) a guide point |
| `geolocation <latitude> <longitude> <north angle> <utc offset>` | (version 4) where the model is (`drydock_kernel::sun::Geolocation`): degrees north, degrees east, degrees from +Y to true north clockwise seen from above, hours ahead of UTC; at most once, anywhere before `end` |
| `sun <year> <month> <day> <hour> <minute> <shadows> <sun shading> <light> <dark> <on ground>` | (version 4) the shadow settings (`ShadowSettings`): local date and time as integers, then `on`/`off`, `on`/`off`, two reals in 0..=1, `on`/`off`; at most once, anywhere before `end` |
| `units decimal <unit> <decimals>` / `units architectural <denominator>` | (version 5) the model's length format (`drydock_kernel::units::LengthFormat`, `docs/specs/m6-units.md`): `<unit>` one of `mm cm m in ft`, `<decimals>` 0 to 6; or architectural with `<denominator>` one of 1 2 4 8 16 32 64; at most once, anywhere before `end` |
| `end` | last record; after it only blank lines may follow |

Every record is applied as soon as it is read, so everything it names must already exist
(the material for a paint or instance, the definition for a `body` or an instance, the
vertices of an edge, the edges of a face, the face of a paint or texmap). Ids keep their
values. A `guide` record in a version 1 file is an unknown record; a `texmap` record in a
version 1 or 2 file is too, a `geolocation` or `sun` record in a version 1 to 3 file, and a
`units` record in a version 1 to 4 file, and a `curve` record in a version 1 to 5 file.
A `curve` record needs at least 17 tokens, finite numbers and `u64` ids; its vertices must
exist in the current body (else it is rejected with the first missing one), and the curve
must fit them (`curve::fits`: vertices on the circle in order from `start` to `end`, each
step joined by an edge or a straight run of edges), else it is a bad field.
A second `geolocation`, `sun` or `units` record is a bad field; so is a `units` record with
the wrong token count, an unknown kind or unit, or a number that isn't an unsigned integer.
Values the model refuses (out of range, an invalid date, 7 decimals, a 1/3 fraction) are
rejected with `BadSettings`.

In versions 1 and 2 the material record has exactly 8 tokens and a texture's size reads as
`1` by `1` meters. In version 3 a material with a texture has exactly 10 tokens; without
one it has 8 and the texture token is `-`. Any other shape is a bad field. A size the
model refuses (non-finite or not positive) is rejected.

## What writers produce
Exactly this order, one space between tokens, every line ending in `\n`:
1. the header, with the lowest version that holds the model: `6` when a body has a curve;
   else `5` when the length format differs from its default (inches, 2 places); else `4` when the geolocation or
   the shadow settings differ from their defaults; else `3` when a material's texture
   size isn't exactly `1` by `1`, or an existing face has a stored texture map; else `2`
   when a body has guides; else `1`;
2. `geolocation`, then `sun`, then `units`, each only when it differs from its default;
3. `material` records, ascending id; in version 3 a texture also writes its width and
   height;
4. `definition` records, ascending id;
5. `body root`, then `body <id>` for every definition in ascending id (written even when
   the body is empty); inside each body: `vertex` records, then `edge`, `face`, `curve`,
   `paint`, `texmap`, `instance` and `guide` records, each in ascending id; `texmap` is written for
   each existing face with a stored map, `front` before `back`;
6. `end`.

`paint` is written only for faces that exist and have a material on either side.
`texmap` is written only for faces that exist (face material entries left behind by
removed faces are dropped, and their maps with them). No comments or blank lines.

## Saving
The writer never leaves a half-written file under the target name: it writes the whole
text to `<file name>.tmp` in the same directory, flushes it to disk, then renames it over
the target. If anything fails, the temporary file is removed and the target is untouched.

## Versions
- 1 (M1): everything here except `guide`.
- 2 (M3): adds `guide` records.
- 3 (M4): adds texture sizes to `material` records and `texmap` records.
- 4 (M5): adds `geolocation` and `sun` records.
- 5 (M6): adds the `units` record.
- 6 (M7): adds `curve` records.

A later version may add records; a reader for version N reads every version up to N.
