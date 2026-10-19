# Native model file, version 1

The native file stores a whole `Model` (`modeler-kernel`, `src/model.rs`): materials,
definitions, and for the root and every definition its geometry (vertices, edges, faces),
face materials and instances. Code: `modeler_kernel::format`. Owner: orchestrator.

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
The first line is the header `modeler-model 1` (the word, one space, the version). A reader
refuses a file whose first line is not `modeler-model <u32>`, and a version it doesn't know.

Then, one per line:

| Record | Meaning |
|---|---|
| `material <id> <r> <g> <b> <a> <name> <texture>` | sRGB color with straight alpha (alpha is opacity); `name` is a string; `texture` is a string (image path relative to the model file) or `-` |
| `definition <id> <kind> <name>` | `kind` is `component` or `group`; the new definition's body is empty |
| `body root` / `body <definition id>` | the following geometry, paint and instance records belong to this body; each body appears at most once |
| `vertex <id> <x> <y> <z>` | a vertex |
| `edge <id> <v1> <v2>` | an edge from `v1` to `v2` (order kept) |
| `face <id> <loop> [\| <loop>]...` | the first loop is the outer loop, each one after a `\|` token is a hole; a loop is a list of edge uses `+<edge id>` (forward) or `-<edge id>` (reversed), possibly empty |
| `paint <face id> <front> <back>` | the face's materials, each a material id or `-` |
| `instance <id> <definition> <material> <m0> ... <m15> <name>` | a placed definition; `material` is an id or `-`; the 16 numbers are the transform in column-major order (`DMat4::to_cols_array`); `name` is a string |
| `end` | last record; after it only blank lines may follow |

Every record is applied as soon as it is read, so everything it names must already exist
(the material for a paint or instance, the definition for a `body` or an instance, the
vertices of an edge, the edges of a face, the face of a paint). Ids keep their values.

## What writers produce
Exactly this order, one space between tokens, every line ending in `\n`:
1. the header;
2. `material` records, ascending id;
3. `definition` records, ascending id;
4. `body root`, then `body <id>` for every definition in ascending id (written even when
   the body is empty); inside each body: `vertex` records, then `edge`, `face`, `paint`
   and `instance` records, each in ascending id;
5. `end`.

`paint` is written only for faces that exist (face material entries left behind by removed
faces are dropped). No comments or blank lines.

## Saving
The writer never leaves a half-written file under the target name: it writes the whole
text to `<file name>.tmp` in the same directory, flushes it to disk, then renames it over
the target. If anything fails, the temporary file is removed and the target is untouched.

## Versions
- 1 (M1): this document.

A later version may add records; a reader for version N reads every version up to N.
