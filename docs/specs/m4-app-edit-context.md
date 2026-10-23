# m4-app-edit-context: the editor works inside an open instance

The editor (`app/crates/modeler/src/editor.rs`) can open an instance for editing. While one
is open, every tool picks, infers and edits in that instance's definition, so all instances
of it change together. The kernel side is on main: read `docs/specs/m4-edit-path.md`
(`EditPath`, `infer::pick_in`, `infer::infer_in`). On main already: the `path`,
`hide_rest` fields and the methods `Editor::{path, open_instance, close_path, hide_rest,
set_hide_rest}` (unimplemented). Only `editor.rs` changes (plus whatever helper types you
add in the `modeler` crate). Tests: `app/crates/modeler/tests/editor_edit_context.rs`, and
`editor_instances.rs` (its double-click test changed). The other tests must keep passing.

## State

- `path()` is the open `EditPath` (starts as the root). `hide_rest()` starts false.
- `open_instance(model, instance)`: if the open body has that instance, push it on the path,
  clear the selection and return true; else return false and change nothing. Never changes
  the model or the undo stack.
- `close_path()`: pop the innermost step and clear the selection. At the root nothing happens
  (the selection stays).
- The path persists across tool changes. The undo stack is shared by all bodies.
- At the start of `pointer_move`, `click`, `box_select` and `key` the editor replaces its
  path by `path.valid_prefix(model)` (an erase, undo or outside edit may have removed an
  instance); when that shortened it, the selection is cleared as in `close_path`.

## Coordinates

Everything the editor stores and hands to kernel tools is in the **open body's coordinates**
(the body from `path.body`): the ray through the cursor is mapped through the inverse of
`path.transform`, `Target.body` is the open body, selection and box selection use that body,
and `project` maps a body point through `path.transform` and then the camera. Everything the
editor returns for drawing is in **world coordinates**: `hint().point`, `preview()`,
`lock()`-related segments, `measurement` are unchanged in meaning for the root, and inside an
open instance they are mapped by `path.transform` (directions and lengths shown as the user
sees them in the world; typed distances are in the open body's units, which is the same as
world units unless the instance is scaled — do not special-case scaling beyond this).
With the root open nothing changes from today.

## Behaviour with a non-root path

- **Pick / infer / hover** in every tool use `infer::pick_in` / `infer::infer_in` with the
  path and `rest = !hide_rest()`. A pick that lands on the rest of the model is `None`.
  A hint snapping to the rest of the model is allowed (that is the point of `rest`): its
  `point` is in world coordinates, and a click that uses it creates geometry at the
  corresponding body point.
- **Line, Rectangle, Circle, Arc, Arc3, Push/Pull, Move, Rotate, Scale, Tape, Erase, Paint,
  Select, box select** all act on the open body only: new edges and faces are added to its
  definition, erasing and painting use `Target.body` / the body argument of the kernel call,
  and so on. The root and other definitions stay untouched except through shared definitions
  (other instances of the same definition show the change; that is automatic).
- **Select** with count 1: a click that picks `None` (empty space or the rest of the model)
  clears the selection as before. A pick of an `Entity::Instance` inside the open body
  selects it (it is an instance of the open body).
- **Double click opens**: Select tool, count 2 and the pick is an `Entity::Instance` of the
  open body: `open_instance` on it (no other effect). This replaces "bounded" selection for
  instances; double click on a face or edge keeps working as before.
- **Double click outside closes**: Select tool, count 2 and the pick is `None` while the
  path is not the root: `close_path()`.
- **Esc closes**: `EditKey::Cancel` does what it does today; additionally, when the tool is
  Select, Orbit, Pan or Zoom and the path is not the root, `close_path()`. With another tool
  it never closes.
- **Hide rest**: with `set_hide_rest(true)` nothing outside the open body is picked or
  snapped to (`rest = false`). With false it is (and a pick on it is `None`).
- `prune` (dropping dead selection entries) checks the open body.

## Not in scope

- Drawing the rest of the model faded or hidden, menu items, name prompts
  (`m4-app-components`). The viewport scene is not changed here.
