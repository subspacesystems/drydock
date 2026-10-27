# m6-icons-2: icons that sit in the middle of their square

Milestone M6 (UI), feedback round. Crate `modeler`: `src/icons.rs` only.

## Problem
Several tool icons are drawn off to one side or poke out of their square, so toolbars look ragged: the Arc sits high in its button, the 3-Point Arc low, the Pan hand left, the Paint Bucket and Eraser off centre, and the extrude arrow icon is wider than the square. Tyrel also finds the Extrude, Rotate, Scale, Tape Measure, Eraser, Paint Bucket and Pan pictures unclear.

## What to change
Redraw these tool icons in `shapes` (your own drawing, same rules as `docs/specs/m6-icons.md`: lines, paths, circles, rectangles, curves; one color; stroke grows with the icon):
- `Tool::PushPull` (the user-facing name becomes Extrude in a later task): a box seen from a corner (three visible faces, the top one lightly filled) with an arrow rising from the top face.
- `Tool::Rotate`: a circular arrow of about three quarters of a circle with a clear arrowhead, no center dot.
- `Tool::Scale`: a small filled square in the lower-left corner of a larger outlined square, and a two-headed diagonal arrow across them.
- `Tool::Tape`: a flat tape-measure case (rounded rectangle) with a short tape sticking out of one side that ends in a hook, and a few ticks along the tape.
- `Tool::Erase`: a tilted eraser block, half filled, with a short line under it.
- `Tool::Paint`: a bucket seen from the side, tipped, with a drop falling from its lip.
- `Tool::Pan`: an open hand with four fingers and a thumb, upright.
- `Tool::Arc` and `Tool::Arc3`: same pictures as now, moved so the drawing is centred.

Other icons may change only if needed; they already pass the new tests.

## Acceptance (new tests in `tests/icons_2.rs`; `tests/icons.rs` stays as is and must keep passing)
- For every icon in `icons::all()`, drawn in a 32-point square: the ink's bounding box is centred within 1 point of the square's centre (both axes), and no side of it is longer than 30 points.
- Every tool icon covers at least 6% of a 32-point square, and at a 20-point square at least 5%.
- The seven redrawn tools plus the two arcs differ from every other icon in at least 80 of 1024 pixels (32 points).
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
- Reviewer looks at the icons large and at toolbar size.
