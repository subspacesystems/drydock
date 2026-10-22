# m3-tool-move: Move tool state machine (kernel)

`app/crates/modeler-kernel/src/tools/translate.rs`: a `MoveTool` that turns the cursor's inferred
points, clicks and typed distances into moves of a selection. Like the other tools in `tools/`,
no UI: the editor runs inference, feeds `Input`s in and draws `preview`. Wiring it into the
editor is a later task. Tests: `app/crates/modeler-kernel/tests/tool_move.rs`.

Only `translate.rs` changes. The types and signatures are on main; fill in the bodies (add
private fields and helpers as you like; drop the `allow(dead_code)` once they are used). The move
itself is `movemerge::transform_sticky(model, body, selection, translation, undo, "Move")`, so
moved geometry welds and splits against the rest of the body (`docs/specs/m3-move-merge.md`).
Lengths are metres in the model; typed text goes through `units::parse_length(text, target.unit)`
(`Err(Parse(..))` on failure).

A point is finite when all three coordinates are. The **offset** is a point minus the from
point; an offset of length `LINEAR_TOL` or less has no direction.

## Stages

`stage()` is `From` (waiting for the point to move from) or `To` (moving). A new tool is at
`From` with no last move.

### From

- `Move(p)`: nothing.
- `Click(p)`: `p` not finite: `Err(NotFinite)`. Empty selection: `Err(Empty)`. Otherwise the
  from point is `p`, no cursor yet, stage `To`, and the last move is forgotten.
- `Typed(s)`: change the last move (see Typed distances after a move).
- `Cancel`: nothing.

### To

- `Move(p)`: a finite `p` becomes the cursor; a non-finite one is ignored (`Ok`).
- `Click(p)`: `p` not finite: `Err(NotFinite)`. Otherwise `p` becomes the cursor. If the offset
  from the from point has no direction: `Ok`, still at `To`, nothing changes. Otherwise commit it.
- `Typed(s)`: parse (`Err(Parse(..))`, nothing changes). The direction is that of the cursor
  offset; no cursor yet or no direction: `Err(NoDirection)`. A value within `LINEAR_TOL` of 0:
  `Ok`, nothing changes. Otherwise commit `direction * value` (a negative value goes the other
  way).
- `Cancel`: back to `From`, the from point and cursor forgotten, model unchanged.

## Commit

Call `transform_sticky` with the translation by the offset, label `"Move"`.

- Refused: `Err(Transform(e))`; the tool keeps its stage, from point and cursor.
- Done: stage `From`, from point and cursor forgotten. Remember the **last move**: the offset
  and `target.undo.undo_count()` right after.

## Typed distances after a move

At `From`, `Typed(s)` changes the last move when there is one and the undo stack still shows it
on top: `undo_count()` equals the remembered count and `redo_count()` is 0. Otherwise
`Err(NotStarted)`.

Parse `s` (`Err(Parse(..))`). A value within `LINEAR_TOL` of 0: `Ok`, nothing changes. Empty
selection: `Err(Empty)`. Otherwise undo the last move (`UndoStack::undo`), then move `selection`
by `unit(last offset) * value` (negative reverses). The result is one undo step in place of the
old one. If the new move is refused, redo the old one (`UndoStack::redo`) so the model and the
stack are as before, and return `Err(Transform(e))`. On success the last move has the new offset
and the new count.

## Queries

- `from()`: the from point at `To`, else `None`.
- `offset()`: at `To` once a cursor exists, cursor minus from point; else `None`.
- `last_offset(undo)`: the last move's offset while a typed distance could still change it
  (stage `From`, `undo` shows the remembered count and no redo); else `None`.
- `matrix()`: at `To` with a cursor, the translation by `offset()`; else `None`.
- `preview()`: at `To` with a cursor farther than `LINEAR_TOL` from the from point, one segment
  `(from, cursor)`; else empty.

Not in this task: copy modifier, axis locks (the editor passes locked points in through
inference).
