# m3-app-pushpull: Push/Pull tool in the editor

The editor (`app/crates/modeler/src/editor.rs`) gets a Push/Pull tool on top of the kernel's
`modeler_kernel::pushpull::push_pull` (read `docs/specs/m3-pushpull.md` for what one push does;
the editor only picks the face and the distance). No window code here: toolbar, key and
drawing come in a later task. Tests: `app/crates/modeler/tests/editor_pushpull.rs`. The other
editor tests must keep passing.

Only `editor.rs` changes. `Tool::PushPull` and the stubs `Editor::pushpull_face`,
`Editor::pushpull_distance` and the `unimplemented!` arms are already on main; fill them in.
Work on the root body (`BodyId::Root`) only, like the other tools.

## States

The tool is **idle** or **pushing** a face. Pushing remembers the face id, the start point (where
the ray through the starting click met the face) and the face's unit front normal `n` at that
time.

- `pushpull_face()`: idle: the face of the root body under the cursor at the last
  `pointer_move` (nearest face hit by the cursor ray, `infer::ray_face`), else `None`.
  Pushing: the face being pushed, wherever the cursor is. `None` for any other tool.
- `pushpull_distance()`: pushing: the current signed distance along `n`; idle or other tools:
  `None`.

## Cursor to distance

While pushing, each `pointer_move` sets the distance:

1. Run the editor's usual inference at the cursor (the same as the Line tool's, no anchor). If
   it snaps to an endpoint or a midpoint (`Snap::Endpoint`, `Snap::Midpoint`) at point `p`, the
   distance is `(p - start) · n`. A taller point anywhere in the model can set the height.
2. Otherwise take the point on the line `start + t·n` closest to the cursor ray; the distance is
   that `t`. If the ray is parallel to the line, keep the previous distance.

`hint()` while pushing is an inference whose point is `start + distance·n` (snap kind and axis
as you like). Idle, the hint is `None`.

`preview()` while pushing with distance `d`: for each loop of the face (outer and holes), each
edge `a`-`b` moved by `d·n`, plus one segment from each loop vertex `a` to `a + d·n`. For a quad:
8 segments. Idle: empty.

## Clicks

`click(.., count)`:

- **count 1, idle**: if a face is under the cursor, start pushing it (distance 0). Returns
  false. Nothing under the cursor: nothing happens.
- **count 1, pushing**: commit with the current distance (see Commit). A distance within
  `LINEAR_TOL` of zero does nothing (still pushing, no message).
- **count 2 or more**: drop any push in progress. If there is a last distance (from an earlier
  commit with this editor) and a face is under the cursor, push/pull that face by the last
  signed distance (Commit). Otherwise nothing happens. A double click arrives as a count-1 click
  (which starts a push) and then a count-2 click at the same spot.

Return value: true when the model changed.

## Commit

`push_pull(model, BodyId::Root, face, distance, undo, "Push/Pull")` with the editor's undo
stack. On success: back to idle, the message cleared, the typed text cleared, and remember
**last** = (face, signed distance, the undo stack's `undo_count()` right after). Prune the
selection as the other tools do (entities that no longer exist go). On a refusal: set a message
(e.g. "Can't push/pull that"), the model and undo stack are unchanged, and the tool keeps its
state (still pushing, if it was).

## Typed distance (`text` and Enter)

`text()` takes number-like text for this tool as for the drawing tools. Enter with typed text
`s`, parsed as a length in the editor's unit (`units::parse_length` as the Line tool does;
"Not a length" message on a parse error):

- **Pushing**: the distance is `|s|` in the direction the cursor shows (the sign of the current
  distance; along `n` when the current distance is zero), flipped when `s` is negative. Commit.
- **Idle, right after a commit**: if `last` exists and the undo stack's `undo_count()` still
  equals the count remembered with it and `redo_count()` is 0, change that push: undo it, then
  push/pull the same face by `|s|` in the direction of the last distance (flipped when `s` is
  negative). The result is one undo step in place of the old one. If the new push is refused,
  redo the old one so the model is as before, and set a message. On success `last` is updated.
- **Idle otherwise**: nothing (returns false).

The typed text is cleared on Enter in all cases.

## Other keys

- Esc: drop the push in progress and the typed text; the model is unchanged.
- Undo/Redo: as for the other tools; they also drop a push in progress.
- `set_tool` (any tool) drops a push in progress.
- Arrow keys do nothing for this tool.

## Measurement box

`measurement()` for this tool: label `"Distance"`. Value: pushing: the absolute current
distance formatted with `units::format_length(.., self.unit, 2)`; idle with a last distance: the
absolute last distance, same format; otherwise the empty string.
