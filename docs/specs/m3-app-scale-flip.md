# m3-app-scale-flip: Scale tool and Flip in the editor

The editor (`app/crates/modeler/src/editor.rs`) gets a Scale tool that drives the kernel's
`modeler_kernel::tools::scale::ScaleTool` (read `docs/specs/m3-tool-scale.md`: handles, stages,
typed factors, last scale), and a `flip` method. The editor finds the handle under the cursor,
turns cursor events into points, and keeps the selection, messages and measurement box. No
window code here: toolbar, keys and drawing come in a later task. Tests:
`app/crates/modeler/tests/editor_scale.rs`. The other editor tests must keep passing.

Only `editor.rs` changes. `Tool::Scale`, the `scale`, `scale_hover` and `scale_segments`
fields, `Editor::scale_tool` and `Editor::scale_hover` (done), `flip` and `unimplemented!`
arms are on main; fill them in. Root body only, like the other tools. Read
`docs/specs/m3-app-move.md`: Scale follows the same shape (typed text, retype right after, idle
on Esc/undo/redo/tool switch). `takes_text` must include Scale.

## Handles

While the scale tool is at `Pick` and the selection is not empty, `bounds(model, Root,
selection)` gives the box and `handles(box)` its handles. The **hovered handle** is the one
whose projected position (`Viewport::project` of `box.point(handle)`) is nearest the cursor,
if that distance is at most `SNAP_RADIUS_PX`; ties go to the smaller `Handle`. Handles that
cannot be projected don't count. Nothing close, an empty selection, or no box: no hovered
handle.

## Pointer move

`pointer_move(.., mods)` with Scale:

1. `set_about_center(mods.ctrl)` and `set_invert_uniform(mods.shift)` on the tool, every event.
2. At `Pick`: `scale_hover` is the hovered handle; the hint is `None`.
3. At `Drag`: `scale_hover` is `None`. Run inference as the Move tool does, with `Probe::anchor`
   = the tool's `fixed_point()`, `lock` `None`. The hint is that inference; the tool gets
   `Input::Move(inf.point)`. No inference: hint `None`, nothing is sent.

`scale_hover()` is `None` for other tools.

## Clicks

`click(.., mods, count)`: `count` is ignored. The two switches are set from `mods` first, as in
pointer move.

- At `Pick`:
  - Empty selection: message "Select something to scale", return false.
  - Handle under the cursor (as above, worked out for this click): `grab` it with the
    editor's selection on the root body. `Ok`: clear the message, collect the preview segments
    (below), `scale_hover` becomes `None`. `Err(e)`: set the message (below). Returns false.
  - No handle under the cursor: message "Click a handle to scale", return false.
- At `Drag`: run inference as in pointer move; none: hint `None`, return false. Otherwise the
  tool gets `Input::Click(inf.point)` with the editor's selection and a `Target` on the root
  body with the editor's undo stack and unit. `Ok` clears the message; `Err` sets it.

Returns true exactly when an undo step was pushed; then the selection is pruned.

## Typed factors

`text(s)` takes text for Scale with the same rules as the drawing tools. `Enter` with typed
text sends `Input::Typed(text)` (selection and target as for a click), then clears the text;
`Ok` clears the message, `Err` sets it. This covers typing while scaling and typing right after
a scale (the kernel tool changes the last scale: it undoes it and scales again, so the undo
count stays the same). A typed scale from `Drag` ends the drag. Returns true when the model
changed: an undo step was pushed, or `last_factors` was `Some` before and is `Some` with a
different value after. Then prune.

## Messages

| Error | Message |
| --- | --- |
| `Parse(_)` | "Not a scale factor" |
| `WrongCount` | "Wrong number of factors" |
| `ZeroFactor` | "Can't scale to nothing" |
| `NotStarted` | "Click a handle first" |
| `Empty` | "Select something to scale" |
| `NoHandle` | "Click a handle to scale" |
| `NotFinite`, `Transform(_)` | "Can't scale that" |

## Other keys and tool switches

- Esc (`Cancel`): the scale tool gets `Input::Cancel`; typed text and message cleared; the
  preview segments dropped. The model is unchanged.
- Undo / Redo: the scale tool gets `Input::Cancel` first, then as for the other tools.
- `set_tool` (to any tool): the scale tool is replaced by a new one (switches off, last scale
  forgotten); `scale_hover` and the preview segments cleared (on main).
- Arrow keys and `draw_plane()`: nothing for Scale. `plane_lock()` is `None`.

## Output

- `measurement()`: label `"Scale"`. Value: at `Drag`, the tool's `factors()`; else
  `last_factors(undo)` if `Some`; else `""`. A factor triple is formatted `{:.3}` once when
  its three values are equal (within `1e-9`), else as `"x, y, z"` with each `{:.3}`, joined
  by `", "`.
- `preview()`: only at `Drag`. First the tool's `preview()` box as its 12 edges: its 8 corners
  numbered by bits (x = 1, y = 2, z = 4), then for each corner `i` in order and each bit `b`
  in x, y, z order with bit `b` clear in `i`, the segment from corner `i` to corner `i | b`.
  Then the geometry that would scale, mapped by `matrix()`: every selected edge and every edge
  of the outer and inner loops of every selected face, each edge once, and the segments of
  every selected instance (`select::instance_segments`). Collect these segments when the
  handle is grabbed, as Move does (`collect_rotate_segments`). Other stages: empty.
- `scale_tool()`: `Some(&scale)` while the tool is Scale (on main).
- `hint()`: as above.

## Flip

`flip(axis, model)` works with any tool. Axis 0, 1, 2 is x, y, z. It mirrors the selection
across the plane through the centre of its bounding box that is perpendicular to the axis:
matrix = translate by `-c`, scale by -1 on that axis, translate by `+c`, where `c` is the
centre of `bounds(model, Root, selection)`; run through `transform::transform(model, Root,
selection, matrix, undo, "Flip")` (faces keep facing out; one undo step).

- `axis >= 3`: false, nothing else happens (the message is untouched).
- The scale tool is at `Drag`: false, nothing happens.
- Empty selection or no box: message "Select something to flip", false.
- `transform` refused: message "Can't flip that", false.
- Done: message cleared, selection pruned, true.
