# m2-app-shapes: rectangle, circle and arc tools in the editor

Module `modeler/src/editor.rs` and the key map in `modeler/src/input.rs` (skeleton on main: `Tool::{Rectangle, Circle, Arc}`, `Editor::draw_plane`, `Editor::plane_lock`, `plane_for_normal`). Change only those two files (add private fields and helpers; keep public signatures). No new dependencies. Never panic. Builds on `m2-app-editor` (read `docs/specs/m2-app-editor.md`; everything there still holds for `Select` and `Line`) and `m2-tool-shapes` (`RectTool`, `CircleTool`, `ArcTool`, `DrawPlane`; read `docs/specs/m2-tool-shapes.md`, including its last section: `plane`, `set_plane`, `ArcTool::placed`).

"Shape tool" means `Rectangle`, `Circle` or `Arc`. "Drawing tool" now means `Line` or a shape tool. `TOL`, `AXES`, the root body and probes are as in `m2-app-editor`.

## `input.rs`
New keys, plain only (other modifier sets give `None`): `R` gives `SelectTool(Rectangle)`, `C` gives `SelectTool(Circle)`, `A` gives `SelectTool(Arc)`. `drag` returns `None` for a left drag with a shape tool (already on main). Everything else is unchanged.

## `plane_for_normal(n)`
`n` unit length. If `|n.z| >= 1 − 1e-9`: `u = X`, `v = n × X`. Else `u = unit(Z × n)`, `v = n × u`. So `u × v = n`, `u` is horizontal for any non-horizontal plane, and the ground (`n = Z`) gives `u = X`, `v = Y`.

## Tool objects
The editor owns one `RectTool`, one `CircleTool` and one `ArcTool`, created in `new()` on the ground plane (`plane_for_normal(Z)`) and kept for the editor's life, so the circle and arc segment counts survive switching tools. "The shape tool goes idle" means it gets `Input::Cancel`. Everywhere `m2-app-editor` says the line tool goes idle (`set_tool`, `Cancel`, `Undo`, `Redo`), all three shape tools go idle too. The anchor of a shape tool is its `anchor()`.

## Draw plane
While the active shape tool is idle, every pointer event (move or click) first picks its plane, using that event's ray `(origin, dir)`:
1. If the plane lock is axis `i`: `n = AXES[i]`.
2. Else if `infer::ray_face(root geometry, origin, dir)` hits face `f` and `face_plane(f)` exists: `n` = its normal.
3. Else `n = Z`.
Then if `n · dir > 0`, `n = −n` (the plane faces the viewer). The tool gets `set_plane(plane_for_normal(n))`. While the tool is drawing, its plane stays.

`draw_plane()`: with a shape tool, that tool's `plane()`; else `None`.

## Plane lock
`key(Lock(i))` with a shape tool, `i < 3`, and the tool idle: if the plane lock is axis `i`, clear it, else set it to `i`. While drawing, or `i >= 3`: nothing. `plane_lock()` returns it with a shape tool, else `None`. It is cleared by `Cancel` and `set_tool`, and kept after a shape is drawn. With a shape tool `lock()` is `None`, and there is no Shift lock (the probe's `lock` is `None`).

## Pointer events with a shape tool
Build the probe as for `Line`, with `anchor` = the shape tool's anchor and `lock` = `None`; `inf = infer(root geometry, probe)`.
- `inf` is `None`: the hint becomes `None` (move) and nothing else happens; a click returns false.
- Idle: pick the plane (above). The hint is `inf`. The point is `inf.point`.
- Drawing (anchor `a`, plane normal `n`): if `inf.snap` is `OnFace`, `Ground` or `Along`, intersect the ray with the plane through `a`: `t = (a − origin) · n / (dir · n)`; when `|dir · n| > 1e-9`, `t > 0` and the hit is finite, the point is `origin + dir · t`; otherwise `inf.point`. Other snaps keep `inf.point`. The hint is `inf` with `point` replaced by that point.
- Move: the tool gets `Input::Move(point)`. Click: the tool gets `Input::Click(point)` with a `Target` as for `Line`; `Ok` clears the message, `Err` sets it (same wording as `m2-app-editor`). `count` is ignored. Returns true exactly when an undo step was pushed; then the selection is pruned.

## Typed text
`text(s)` takes text for every drawing tool (same rules as for `Line`). `Enter` sends `Input::Typed(text)` to the active drawing tool, then clears the text; `Ok` clears the message, `Err` sets it; returns true exactly when an undo step was pushed (then prune). The plane lock stays. So `8s` then Enter sets the circle's or arc's segments, even while idle.

## Output with a shape tool
- `preview()`: rectangle `[c0, c1, c2, c3]` as `(c0,c1), (c1,c2), (c2,c3), (c3,c0)`; circle points `p` as `(p[i], p[(i+1) % n])`; arc points as `(p[i], p[i+1])`; no preview: empty.
- `measurement()`, with `f(x) = units::format_length(x, unit, 2)`:
  - `Rectangle`: `("Dimensions", "<f(w)>, <f(h)>")` from `size()`, else `("Dimensions", "")`.
  - `Circle`: idle: `("Segments", segments as a decimal number)`; drawing: `("Radius", f(radius()))`, or `""` without a radius.
  - `Arc`: `placed()` 0: `("Segments", segments)`; 1: `("Radius", f(radius()) or "")`; 2: `("Angle", format!("{:.1}°", sweep in degrees))`, or `""` without a sweep.
- `hint()`, `message()`, `selection()`, `undo_stack()`, `tool()`: as before.

## Tests
`tests/editor_shapes.rs` (top view as in `tests/editor.rs`, plus a front view for drawing on a wall), `tests/editor_shapes_props.rs` (random events over every tool never panic and keep the model valid; `plane_for_normal` is orthonormal with `u × v = n`), `tests/input_shapes.rs` (key table).
