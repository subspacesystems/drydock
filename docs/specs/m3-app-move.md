# m3-app-move: Move tool in the editor

The editor (`app/crates/modeler/src/editor.rs`) gets a Move tool that drives the kernel's
`modeler_kernel::tools::translate::MoveTool` (read `docs/specs/m3-tool-move.md`: stages, typed
distances, last move). The editor turns cursor events into points and keeps the selection,
messages and measurement box. No window code here: toolbar, keys and drawing come in a later
task. Tests: `app/crates/modeler/tests/editor_move.rs`. The other editor tests must keep
passing.

Only `editor.rs` changes. `Tool::Move`, the `translate: MoveTool` field, `Editor::move_tool`
(done) and `unimplemented!` arms are on main; fill them in. Root body only, like the other
tools. Read `docs/specs/m3-app-rotate.md`: Move follows the same shape (selection from the
entity under the first click, typed text, retype right after, idle on Esc/undo/redo/tool
switch), without a protractor plane and without plane locks.

## Points

Run inference as the Line tool does for a free point (`Probe::anchor` = `translate.from()`,
`lock` = `None`; no Shift or arrow locks). The point is `inf.point`; the hint is `inf`. No
inference: hint `None`, nothing is sent to the tool.

`pointer_move`: the tool gets `Input::Move(point)`.

## Clicks

`click(.., count)`: `count` is ignored.

1. At `From`, if the editor's selection is empty: pick the entity under the cursor as the
   Select tool would (count 1, no modifiers) and make it the selection. Nothing under the
   cursor: message "Select something to move", return false, nothing else happens.
2. The tool gets `Input::Click(point)` with the editor's selection and a `Target` on the root
   body with the editor's undo stack and unit.
3. `Ok` clears the message; `Err` sets it (wording below).

Returns true exactly when an undo step was pushed; then the selection is pruned (entities that
no longer exist go).

## Typed distance

`text(s)` takes text for Move with the same rules as the drawing tools. `Enter` with typed text
sends `Input::Typed(text)` (selection and target as for a click), then clears the text; `Ok`
clears the message, `Err` sets it. This covers typing while moving and typing right after a move
(the kernel tool changes the last move: it undoes it and moves again, so the undo count stays
the same). Returns true when the model changed: an undo step was pushed, or `last_offset` was
`Some` before and is `Some` with a different value after. Then prune.

## Messages

| Error | Message |
| --- | --- |
| `Parse(_)` | "Not a length" |
| `NotStarted` | "Click a start point first" |
| `NoDirection` | "Move the cursor to show the direction" |
| `Empty` | "Select something to move" |
| `NotFinite`, `Transform(_)` | "Can't move that" |

## Other keys and tool switches

- Esc (`Cancel`): the move tool gets `Input::Cancel`; typed text and message cleared. The model
  is unchanged.
- Undo / Redo: the move tool gets `Input::Cancel` first, then as for the other tools.
- `set_tool` (to any tool): the move tool is replaced by a new one (the last move is
  forgotten).
- Arrow keys and `draw_plane()`: nothing for Move (`draw_plane()` is `None`).

## Output

- `measurement()`: label `"Distance"`, lengths formatted like Push/Pull's (`units::format_length`
  with the editor's unit, 2 decimals). Value: while moving with a cursor, the length of
  `offset()`; else, if `last_offset(undo)` is `Some(o)`, the length of `o`; else `""`.
- `preview()`: the tool's `preview()` segments first. Then, while moving with a cursor, the
  geometry that would move, translated by `matrix()`: every selected edge and every edge of the
  outer and inner loops of every selected face, each edge once, and the segments of every
  selected instance (`select::instance_segments`). Collect these segments when the from point is
  placed, as Rotate does with `collect_rotate_segments`. Otherwise only the tool's segments.
- `move_tool()`: `Some(&translate)` while the tool is Move (on main).
- `hint()`: as in Points.
