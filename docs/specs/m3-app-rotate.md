# m3-app-rotate: Rotate tool in the editor

The editor (`app/crates/modeler/src/editor.rs`) gets a Rotate tool that drives the kernel's
`modeler_kernel::tools::rotate::RotateTool` (read `docs/specs/m3-tool-rotate.md`: stages,
snapping, typed angles, last rotation). The editor picks the protractor's plane, turns cursor
events into points, and keeps the selection, messages and measurement box. No window code
here: toolbar, key and protractor drawing come in a later task. Tests:
`app/crates/modeler/tests/editor_rotate.rs`. The other editor tests must keep passing.

Only `editor.rs` changes. `Tool::Rotate`, the `rotate: RotateTool` field (created on the
ground plane), `Editor::rotate_tool` (done) and `unimplemented!` arms are on main; fill them
in. Root body only, like the other tools. Read `docs/specs/m2-app-shapes.md` ("Draw plane",
"Plane lock", "Pointer events with a shape tool"): Rotate reuses those rules, with the rotate
tool in place of the shape tool, "idle" meaning `stage() == Center` and the anchor being
`center()`.

## Protractor plane

While the rotate tool is at `Center`, every pointer event (move or click) first picks its
plane exactly as a shape tool picks its draw plane (plane lock axis, else the face under the
ray, else the ground; flipped to face the viewer; `plane_for_normal`), and gives it to the
tool with `set_plane`. Once a center is placed, the plane stays until the rotation ends.

`draw_plane()` with `Tool::Rotate`: `Some(rotate.plane())`.

Plane lock: `key(Lock(i))` with `Tool::Rotate`, `i < 3`, at `Center`: toggles the lock as for
the shape tools. At `Start` or `Angle`, or `i >= 3`: nothing. `plane_lock()` returns it with
`Tool::Rotate` too. Cleared by `Cancel` and `set_tool`, kept after a rotation.

## Points

Run inference as the shape tools do, with `Probe::anchor` = `rotate.center()` (no Shift lock,
probe `lock` is `None`). At `Center` the point is `inf.point`. At `Start` and `Angle`, if
`inf.snap` is `OnFace`, `Ground` or `Along`, the point is where the ray meets the protractor
plane through the center (same formula and fallbacks as a shape tool's plane through its
anchor); other snaps keep `inf.point`. The hint is `inf` with `point` replaced by that point.
No inference: hint `None`, nothing is sent to the tool.

`pointer_move`: the tool gets `Input::Move(point)`.

## Clicks

`click(.., count)`: `count` is ignored.

1. At `Center`, if the editor's selection is empty: pick the entity under the cursor as the
   Select tool would (count 1, no modifiers) and make it the selection. Nothing under the
   cursor: message "Select something to rotate", return false, nothing else happens.
2. The tool gets `Input::Click(point)` with the editor's selection and a `Target` on the root
   body with the editor's undo stack and unit.
3. `Ok` clears the message; `Err` sets it (wording below).

Returns true exactly when an undo step was pushed; then the selection is pruned (entities that
no longer exist go).

## Typed angle

`text(s)` takes text for Rotate with the same rules as the drawing tools. `Enter` with typed
text sends `Input::Typed(text)` (selection and target as for a click), then clears the text;
`Ok` clears the message, `Err` sets it. This covers typing while rotating and typing right
after a rotation (the kernel tool changes the last rotation: it undoes it and rotates again,
so the undo count stays the same). Returns true when the model changed: an undo step was
pushed, or `last_angle` was `Some` before and is `Some` with a different value after. Then
prune.

## Messages

| Error | Message |
| --- | --- |
| `Parse(_)` | "Not an angle" |
| `NotStarted` | "Click the center first" |
| `Empty` | "Select something to rotate" |
| `NotFinite`, `Transform(_)` | "Can't rotate that" |

## Other keys and tool switches

- Esc (`Cancel`): the rotate tool gets `Input::Cancel`; typed text, plane lock and message
  cleared. The model is unchanged.
- Undo / Redo: the rotate tool gets `Input::Cancel` first, then as for the other tools.
- `set_tool` (to any tool): the rotate tool is replaced by a new one on the ground plane (so
  the last rotation is forgotten).
- Arrow keys: the plane lock above.

## Output

- `measurement()`: label `"Angle"`. Value: at `Angle`, `format!("{:.1}°", angle)`; else, if
  `last_angle(undo)` is `Some(a)`, `format!("{:.1}°", a)`; else `""`.
- `preview()`: the tool's `preview()` segments first, in its order. Then, while at `Angle`,
  the geometry that would move, mapped by `matrix()`: every selected edge and every edge of
  the outer and inner loops of every selected face, each edge once (as the segment between
  its two end positions), and the segments of every selected instance
  (`select::instance_segments`). `preview` has no model, so collect these segments when the
  center is placed (the model can't change before the rotation ends: undo and redo end it).
  Other stages: only the tool's segments.
- `rotate_tool()`: `Some(&rotate)` while the tool is Rotate (on main).
- `hint()`: as in Points.
