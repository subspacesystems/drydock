# m3-tool-rotate: Rotate tool state machine (kernel)

`app/crates/modeler-kernel/src/tools/rotate.rs`: a `RotateTool` that turns the cursor's inferred
points, clicks and typed angles into rotations of a selection. Like the drawing tools in
`tools/`, no UI: the editor runs inference, feeds `Input`s in and draws `preview`. Wiring it into
the editor is a later task. Tests: `app/crates/modeler-kernel/tests/tool_rotate.rs`.

Only `rotate.rs` changes. The types, constants and signatures are on main; fill in the bodies
(add private fields and helpers as you like). The rotation itself is
`transform::transform(model, body, selection, matrix, undo, "Rotate")`
(`docs/specs/m3-transform.md`).

## Plane and angles

The protractor lies in the tool's `DrawPlane` through the center; its normal `n = u × v` is the
rotation axis. A positive angle turns counter-clockwise about `n` (right-hand rule: `u` toward
`v` is +90°). Angles in the API and in typed text are degrees.

Points from `Input::Move` and `Input::Click` are first projected into the plane through the
center (`DrawPlane::project`). An **arm** is such a projected point minus the center; arms of
length `LINEAR_TOL` or less have no direction.

`set_plane` changes the plane only while the stage is `Center`; later it is ignored.

## Stages

`stage()` is `Center` (waiting for the protractor's center), `Start` (waiting for the start of
the angle) or `Angle` (rotating). A new tool is at `Center` with no last rotation.

### Center

- `Move(p)`: nothing.
- `Click(p)`: `p` not finite: `Err(NotFinite)`. Empty selection: `Err(Empty)`. Otherwise the
  center is `p`, stage `Start`, and the last rotation is forgotten.
- `Typed(s)`: change the last rotation (see Typed angles), else `Err(NotStarted)`.
- `Cancel`: nothing.

### Start

- `Move(p)`: remember the cursor arm.
- `Click(p)`: `p` not finite: `Err(NotFinite)`. An arm with no direction: `Ok`, nothing
  changes. Otherwise the start direction is the unit arm, the start radius is its length,
  stage `Angle`, angle 0.
- `Typed(_)`: `Err(NotStarted)`, nothing changes.
- `Cancel`: back to `Center`, center forgotten.

### Angle

- `Move(p)`: set the angle from the arm (see Cursor angle). An arm with no direction keeps the
  previous angle.
- `Click(p)`: `p` not finite: `Err(NotFinite)`. Set the angle as for `Move`. If the angle is
  within `ANGLE_TOL_DEG` of 0: `Ok`, still rotating, the model unchanged. Otherwise commit it.
- `Typed(s)`: parse with `parse_angle` (`Err(Parse(..))` on failure, nothing changes). The
  angle is `|s|` in the direction the cursor shows (the sign of the current angle; positive
  when the current angle is 0), flipped when `s` is negative. Within `ANGLE_TOL_DEG` of 0:
  `Ok`, nothing changes. Otherwise commit it.
- `Cancel`: back to `Center`, the model unchanged.

## Cursor angle

The raw angle is the signed angle from the start direction to the arm about `n`, in
(-180, 180]. If it is within `SNAP_WINDOW_DEG` of a whole multiple of `SNAP_STEP_DEG`
(15°), it becomes that multiple (and -180 becomes 180). Typed angles never snap.

## Commit

The matrix is: translate by `-center`, rotate by the angle about `n`, translate by `+center`
(`matrix()` returns this for the current angle while at `Angle`, else `None`). Call
`transform(target.model, target.body, selection, matrix, target.undo, "Rotate")`.

- Refused: `Err(Transform(e))`; the tool keeps its stage, center, start and angle.
- Done: stage `Center`, center, start and cursor forgotten. Remember the **last rotation**:
  center, axis, angle, and `target.undo.undo_count()` right after.

## Typed angles after a commit

At `Center`, `Typed(s)` changes the last rotation when there is one and the undo stack still
shows it on top: `undo_count()` equals the remembered count and `redo_count()` is 0.
Otherwise `Err(NotStarted)`.

Parse `s` (`Err(Parse(..))` on failure). The new angle is `|s|` in the direction of the last
angle, flipped when `s` is negative. Within `ANGLE_TOL_DEG` of 0: `Ok`, nothing changes. Empty
selection: `Err(Empty)`. Otherwise undo the last rotation (`UndoStack::undo`), then rotate
`selection` by the new angle about the same center and axis. The result is one undo step in
place of the old one. If the new rotation is refused, redo the old one (`UndoStack::redo`) so
the model and the stack are as before, and return `Err(Transform(e))`. On success the last
rotation has the new angle and the new count.

## Queries

- `center()`: the center at `Start` and `Angle`, else `None`.
- `angle()`: the current angle at `Angle`, else `None`.
- `last_angle(undo)`: the last rotation's angle while a typed angle could still change it (as
  above: stage `Center`, `undo` shows the remembered count and no redo); else `None`.
- `preview()`: `Center`: empty. `Start`: one segment from the center to the center plus the
  cursor arm, if the cursor arm has a direction; else empty. `Angle`: two segments, `(c, c +
  r·s)` and `(c, c + r·R·s)`, with `c` the center, `r` the start radius, `s` the start direction
  and `R` the current rotation.

## `parse_angle`

Trim white space. An optional trailing `°` or `deg` (then trim again). The rest must parse as a
finite `f64` (Rust's `str::parse`, so `+45`, `-7.5`, `1e1` are fine). Nothing left:
`ParseError::Empty`; anything else that fails: `ParseError::Invalid`.
