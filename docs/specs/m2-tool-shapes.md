# m2-tool-shapes: rectangle, circle and arc tools

Module `modeler-kernel/src/tools/` (skeletons on main: `rectangle.rs`, `circle.rs`, `arc.rs`; `plane.rs` is done). Change only those three files (add private fields and helpers; keep public signatures). No new dependencies. Never panic. Builds on `m2-tool-line` (`Input`, `Target`, `ToolError`, `draw_segments`) and `m2-shapes` (`shapes::{rectangle, circle, arc, sweep_angle}`); read those specs for the terms. Same conventions as `LineTool`: the app passes in inferred points; "nothing changes" means model, undo stack and tool state are as before the call; a non-finite `Move` is ignored (`Ok`); a non-finite `Click` is `Err(Geometry(StickyError::NotFinite))`; `Move` while idle is ignored; `Cancel` goes idle (`Ok`, model untouched).

`DrawPlane { u, v }` is given to `new`. The shape lives in the plane through the first clicked point; later points are measured in that plane (components along `u` and `v` only; the part along the normal is dropped). `TOL` = `geom::LINEAR_TOL`. When a tool finishes a shape it goes idle with no cursor. If `draw_segments` fails, return its error; the tool stays as it was before the call.

Every shape is drawn with one `draw_segments` call, so it is one undo step, labelled `"Rectangle"`, `"Circle"` or `"Arc"`.

## Segment count (circle and arc)
`Typed(text)`, where the trimmed text ends in `s` or `S` and the rest is a whole number `n`, sets the segment count to `n`, in any state (idle too), and nothing else happens (`Ok`). Allowed `n`: 3 to 360 for the circle, 1 to 360 for the arc; otherwise `Err(Parse(ParseError::Invalid))` and the count is unchanged. Defaults: circle 24, arc 12. `segments()` reads it. The count survives finishing a shape and `Cancel`.

## `RectTool`
- `anchor()`: the first corner `a` while drawing, else `None`.
- `preview()`: `shapes::rectangle(a, cursor, u, v)` while drawing with a cursor; else `None`. `size()`: `(|du|, |dv|)` of it, exactly when `preview()` is `Some`.
- **Click(p)**: idle: `a = p`, no cursor. Drawing: `shapes::rectangle(a, p, u, v)`; `None` (too thin): `Ok`, nothing changes; else draw its four corners as a closed loop of four edges `c0→c1, c1→c2, c2→c3, c3→c0`.
- **Typed(text)**: idle: `Err(NotStarted)` (text not parsed). Drawing: `units::parse_lengths(text, target.unit)`, a parse error is `Err(Parse(e))`; any count other than 1 or 2 values is `Err(Parse(Invalid))`. The quadrant comes from the cursor: `su = -1` if the cursor's `du < 0`, else `+1`; same for `sv` (no cursor: both `+1`). Two values `w, h`: the other corner is `a + u·(w·su) + v·(h·sv)` (a negative value flips that side). One value `w`: height is the cursor's `|dv|`; with no cursor or `|dv| <= TOL`, `Err(NoDirection)`. If the resulting rectangle is too thin (`shapes::rectangle` is `None`): `Err(NoDirection)`, nothing changes. Otherwise draw as for a click.
- Nothing changes on any error.

## `CircleTool`
- `anchor()`: the center while drawing. `radial(p)` below: the vector from the center to `p` in the plane, `None` when its length is at most `TOL`.
- `preview()`: while drawing with a cursor whose `radial` exists: `shapes::circle(center, normal, radial, |radial|, segments)` (so the first point is under the cursor). `radius()`: `|radial|`, exactly when `preview()` is `Some`.
- **Click(p)**: idle: center `= p`. Drawing: `radial(p)` is `None`: `Ok`, nothing changes; else draw that circle as a closed loop of `segments` edges (`pts[i] → pts[(i+1) % n]`).
- **Typed(text)** (not a segment count): idle: `Err(NotStarted)`. Drawing: `parse_length`; error `Err(Parse(e))`; the radius is its absolute value, at most `TOL` is `Err(NoDirection)`. The circle's `x_dir` is the cursor's `radial` if there is one, else `u`. Draw it.

## `ArcTool`
Three placed points, all in the plane: center `c`, start `s`, then the end direction. Stages: idle; *center placed*; *start placed*.
- `anchor()`: `c` in the first stage, `s` in the second, else `None`.
- `radius()`: stage 1 with a cursor: `|radial|` if it exists; stage 2: `|s - c|`; else `None`.
- `sweep()`: stage 2 with a cursor whose `radial` exists: `shapes::sweep_angle(c, normal, s, c + radial)` when it is at least `1e-9`, else `None`. Counter-clockwise about the normal, in `(0, 2π)`.
- `preview()`: stage 2, `sweep()` is `Some(w)`: `shapes::arc(c, normal, s - c, |s - c|, 0.0, w, segments)` (`segments + 1` points, the first at `s`). Else `None`.
- **Click(p)**: idle: `c = p`. Stage 1: `radial(p)` is `None`: `Ok`, nothing changes; else `s = c + radial(p)`, no cursor. Stage 2: the sweep to `p` as for `sweep()`; if there is none (point on the center, or sweep under `1e-9`): `Ok`, nothing changes; else draw the arc: the points of `shapes::arc` as consecutive edges `pts[i] → pts[i+1]`.
- **Typed(text)** (not a segment count): idle: `Err(NotStarted)`. Stage 1: a length as for the circle's radius (same errors); `s = c + x·r` with `x` the unit cursor `radial`, else `u`; goes to stage 2 (no cursor), nothing drawn. Stage 2: a number of degrees (trim, then an optional trailing `°`), counter-clockwise positive, negative clockwise; not a number, zero, or `|deg| > 360`: `Err(Parse(Invalid))`, nothing changes. Draw the arc with sweep `deg` in radians (at 360 the last point is the first; keep that edge).
- `Cancel` from any stage goes idle.
