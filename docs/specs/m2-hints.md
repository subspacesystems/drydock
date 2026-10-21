# m2-hints: inference feedback in the window

Tyrel's report: while drawing there is no sign of an axis lock or of what the cursor snapped to. The inference already knows both (`Inference::snap`, `Inference::axis`); the window only draws a small gray or colored square.

## Logic: `modeler/src/hints.rs` (skeleton on main; tests in `modeler/tests/hints.rs`)
- `axis_color(axis)`: index 0 red `[220,50,47]`, 1 green `[40,160,60]`, 2 blue `[50,90,220]`, anything else gray `[160,160,160]`.
- `snap_color(snap)`: the colors `hint_color` in `app.rs` uses today (endpoint `[40,200,80]`; midpoint and center `[60,200,220]`; on edge `[220,70,60]`; on face `[70,130,230]`; on axis, ground and along `[160,160,160]`).
- `snap_name(snap)`: `Endpoint`, `Midpoint`, `Center`, `On Edge`, `On Face`, `On Ground`, `Locked Direction` (for `Along`), and for `OnAxis(i)` `On Red Axis` / `On Green Axis` / `On Blue Axis` (`On ? Axis` for an index above 2).
- `hint_label(inference)`:
  - `axis == None`: text is `snap_name(snap)`, color `snap_color(snap)`.
  - `axis == Some(a)` and the snap is `OnAxis(a)`: text is `snap_name(snap)`, color `axis_color(a)`.
  - `axis == Some(a)` otherwise (the point lies on the axis through the anchor while snapped to something else): text is `"<snap_name> on <Red|Green|Blue|?> Axis"`, e.g. `Endpoint on Red Axis`, color `axis_color(a)`.

## Window (not unit tested; `modeler/src/app.rs`)
- Replace `hint_color` by `hints::snap_color` / `hint_label`.
- In the overlay: draw the marker square in the label's color, and the label text in a small rounded box offset about (14, 14) px from the marker (keep it inside the viewport rect).
- When the hint has an axis, also draw a thin line (1.5 px, the axis color) from the tool's anchor to the hint point. If the editor doesn't expose the anchor, take the first point of `editor.preview()`; draw nothing if there is none.
- While a direction lock is active (`editor.lock()` is `Some`), the label shows the lock: prefix the text with `Locked: `.
- Touch only `hints.rs`, `app.rs`. Check with `harness/bin/gui-session` (Line tool: move along red/green/blue, over an endpoint) and read the screenshots.
