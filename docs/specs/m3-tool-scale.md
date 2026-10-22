# m3-tool-scale: Scale tool state machine (kernel)

`app/crates/modeler-kernel/src/tools/scale.rs`: a `ScaleTool` that scales a selection from one
of the handles on its bounding box, by the cursor or by typed factors. Like `tools/rotate.rs`,
no UI: the editor works out which handle is under the mouse and calls `grab`, feeds the
cursor's points, clicks and typed text in as `Input`s, and draws `preview`. Wiring it into the
editor is a later task. Tests: `app/crates/modeler-kernel/tests/tool_scale.rs`.

Only `scale.rs` changes. The types, constants and signatures are on main; fill in the bodies
(add private fields and helpers as you like; `Bounds::center`, `Bounds::point`, `Handle::new`
and `Handle::opposite` are done). The scale itself is
`transform::transform(model, body, selection, matrix, undo, "Scale")`
(`docs/specs/m3-transform.md`).

## Boxes and handles

`bounds(model, body, selection)`: the smallest axis-aligned box holding every position of
`transform::moved_vertices` for the selection and both ends of every segment of
`select::instance_segments` for each selected instance. Unknown entities add nothing. No
points at all (empty selection, or only unknown entities, or a missing body): `None`.

An axis is **live** when the box is longer than `LINEAR_TOL` along it.

`handles(bounds)`: every `Handle` with components in {-1, 0, 1}, not all 0, whose non-zero
components are all on live axes; sorted. A solid box has 26 (8 corners, 12 edge middles, 6
face middles); a box flat in z has the 8 with `z == 0`; a box along one axis has 2; a box with
no live axis has none.

A handle's **axes** are its non-zero components. A handle is a **corner** when its axes are
exactly the live axes and there are at least two of them.

## State

A new tool is at `Pick` with no last scale, `about_center` and `invert_uniform` off. The two
switches (`set_about_center`, `set_invert_uniform`) can change at any time and keep their
value across drags.

While scaling (`Drag`) the tool holds the box and handle from `grab` and the current factors
along x, y and z. From those:

- **uniform** = (the handle is a corner) XOR `invert_uniform`.
- **fixed point** `f`: the box center when `about_center`, else `box.point(handle.opposite())`.
- `g = box.point(handle)`, `d = g - f`.
- **matrix**: translate by `-f`, scale by the factors, translate by `+f`.

## `grab(handle, model, body, selection)`

Checks in order, each refusal leaving everything unchanged: empty `selection`: `Err(Empty)`;
`bounds` is `None`: `Err(Empty)`; `handle` is not in `handles(bounds)`: `Err(NoHandle)`.

Otherwise: stage `Drag` with that box and handle, factors (1, 1, 1), no cursor point yet, and
the last scale forgotten. A grab while already at `Drag` starts over the same way.

## Cursor factors

From a point `p`:

- uniform: `t = (p - f)·d / (d·d)`; factors (t, t, t).
- not uniform: on each of the handle's axes `i`, `(p_i - f_i) / d_i`; 1 on the other axes.

The tool remembers the last cursor point of the drag (from `Move` or `Click`). When a switch
changes while at `Drag` and there is such a point, the factors are worked out again from it
with the new uniform / fixed point; with no cursor point yet the factors stay as they are.

## Inputs

`handle(input, selection, target)`:

### Pick

- `Move(_)`, `Click(_)`, `Cancel`: nothing, `Ok`.
- `Typed(s)`: change the last scale (see below), else `Err(NotStarted)`.

### Drag

- `Move(p)`: `p` not finite: ignored, `Ok`. Otherwise set the factors from `p`.
- `Click(p)`: `p` not finite: `Err(NotFinite)`. Otherwise set the factors from `p` (they stay
  set even if the click is refused below). Every factor within `FACTOR_TOL` of 1: `Ok`, still
  scaling, model unchanged. Any factor with absolute value below `MIN_FACTOR`:
  `Err(ZeroFactor)`, still scaling. Otherwise commit.
- `Typed(s)`: factors from text (below), using this drag's handle and uniform. Every factor
  within `FACTOR_TOL` of 1: `Ok`, nothing changes. Otherwise commit them.
- `Cancel`: back to `Pick`, model unchanged.

## Factors from text

1. `parse_factors(s)`; failure: `Err(Parse(..))`.
2. Count: uniform takes exactly 1 value `v`, giving (v, v, v). Not uniform takes 1 value
   (put on every one of the handle's axes) or exactly as many values as the handle has axes
   (put on its axes in x, y, z order); the other axes get 1. Any other count:
   `Err(WrongCount)`.
3. Any value with absolute value below `MIN_FACTOR`: `Err(ZeroFactor)`.

Negative factors are fine: they mirror (the transform keeps faces facing out).

## Commit

`transform(target.model, target.body, selection, matrix, target.undo, "Scale")`.

- Refused: `Err(Transform(e))`; the tool keeps its stage, box, handle and factors.
- Done: stage `Pick`. Remember the **last scale**: fixed point, handle, uniform, factors, and
  `target.undo.undo_count()` right after.

## Typed factors after a commit

At `Pick`, `Typed(s)` changes the last scale when there is one and the undo stack still shows
it on top: `undo_count()` equals the remembered count and `redo_count()` is 0. Otherwise
`Err(NotStarted)`.

Factors from text as above, with the last scale's handle and uniform. All within `FACTOR_TOL`
of 1: `Ok`, nothing changes. Empty selection: `Err(Empty)`. Otherwise undo the last scale
(`UndoStack::undo`) and scale `selection` by the new factors about the last fixed point: one
undo step in place of the old one. If the new scale is refused, redo the old one
(`UndoStack::redo`) so the model and the stack are as before, and return `Err(Transform(e))`.
On success the last scale has the new factors and the new count.

## Queries

All `None` unless at `Drag`: `handle_grabbed()`, `uniform()`, `fixed_point()`, `factors()`,
`matrix()`, and `preview()`: the box after the matrix, as a `Bounds` with `min <= max` on each
axis (componentwise min and max of the mapped `min` and `max` corners).

`last_factors(undo)`: the last scale's factors while typed factors could still change it (as
above: stage `Pick`, `undo` shows the remembered count and no redo); else `None`.

## `parse_factors`

Split on `,`, trim each piece. The whole text blank: `ParseError::Empty`. Every piece must
parse as a finite `f64` (Rust's `str::parse`); an empty piece or anything else:
`ParseError::Invalid`. Any number of pieces parses (the count is checked by the tool).
