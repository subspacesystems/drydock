# m6-icons: the app's own icons

Milestone M6 (UI). Crate `modeler`: `src/icons.rs`.

## Goal
Toolbars and panel buttons show icons instead of words. The icons are ours: drawn in code as a few vector shapes, so they scale to any size, follow the theme's text color in light and dark mode, and need no image files or third-party art. This task only makes the icons and an icon button; later tasks put them in the toolbars and the Materials panel.

## Already on main
`src/icons.rs`: `Icon` (`Tool(Tool)`, `List`, `Tiles`, `New`, `Edit`, `Purge`), `TOOLS`, `BUTTON_SIZE`, `paint` (done), and stubs `all`, `shapes`, `icon_button`. `Tool` derives `Hash`.

## `all()`
Every icon: `Icon::Tool(t)` for each `t` in `TOOLS` order, then `List`, `Tiles`, `New`, `Edit`, `Purge`.

## `shapes(icon, rect, color)`
- Draws the icon to fill the largest square centered in `rect` (keep a small margin, about 5%). Everything stays inside `rect` (a stroke may poke out by at most 1 point).
- Only `LineSegment`, `Path`, `Circle`, `Ellipse`, `Rect`, `QuadraticBezier`, `CubicBezier` (and `Vec` of them). No text, meshes, textures or callbacks.
- One color: strokes in `color`; fills may use `color` at lower alpha (e.g. `color.gamma_multiply(0.35)`) for a two-tone look. No other colors.
- Stroke width grows with the icon (about 1/14 of the side works well; at least 1 point), so a 64-point icon looks like a 32-point one scaled up.
- Each icon must read clearly at 20 points and be unlike every other icon. Suggested pictures (your own drawing; don't copy any other program's icons):
  - Select: an arrow pointer. Line: a diagonal segment with dots on its ends. Rectangle: a rectangle outline. Circle: a circle with a center dot and a radius. Arc: a half circle on two end dots with a line from the chord's middle to the bulge. 3-Point Arc: a shallow arc with dots at its start, middle and end.
  - Push/Pull: a flat face seen at an angle with an arrow rising from it. Move: four arrows pointing out from the center. Rotate: a nearly closed circular arrow around a dot. Scale: a small square in the corner of a large one with a diagonal arrow out of it.
  - Tape Measure: a round case with a tape running out of it, with tick marks. Erase: a tilted eraser block. Paint Bucket: a tipped bucket with a drop.
  - Orbit: a tilted orbit (ellipse) with an arrowhead around a filled dot. Pan: an open hand. Zoom: a magnifying glass.
  - List: three rows, each a dot and a line. Tiles: four squares in a 2 x 2 grid. New: a plus. Edit: a pencil. Purge: a waste bin.

## `icon_button(ui, icon, label, tooltip, selected)`
- Allocates exactly `BUTTON_SIZE` x `BUTTON_SIZE`, senses clicks, and returns that response with `tooltip` as hover text.
- Accessible info: a button labelled `label`, with its selected state (`WidgetInfo::selected(WidgetType::Button, enabled, selected, label)`), so tests find it with `get_by_label(label)` and see it toggled.
- Look: the icon a few points inside the square, in the style's foreground color for the button's state (`ui.style().interact_selectable(&response, selected)` gives fill, stroke and text color); a background fill when selected or hovered, nothing behind it otherwise.

## Acceptance
- `cargo test -p modeler --test icons`: order of `all`; every icon draws inside its rect and covers 3-60% of a 32-point square; plain shapes in one color; any two icons differ in at least 40 of 1024 pixels at 32 points; icons move and scale with their rect; the button clicks, is `BUTTON_SIZE` square, reports selected, shows its tooltip.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
- Reviewer looks at the icons drawn large and at toolbar size (a screenshot in a later task) for clarity.
