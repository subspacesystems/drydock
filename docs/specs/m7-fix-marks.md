# m7-fix-marks: problem edges drawn over the selection outline

Found in the M7 sign-off. A selected open box shows the orange selection outline on every
edge, and nothing marks its open edges (Entity Info says "Open edges 4"). The marks' edge
tint lives inside the 3D scene (`marked_scene` -> `tint_problems`), but the selection
outline is painted afterwards by egui over the viewport image, so it hides the tint.

## What to build

- `overlay::Style::Problem`: a new style for an open, loose or shared edge of the selected
  part.
- `overlay::problem_stroke() -> ([u8; 4], f32)`: the stroke of that style. Colour is
  `modeler_dae::PROBLEM_COLOR` as sRGB bytes (each channel x 255, rounded); width 4.0
  points (wider than `Style::Selected`, 3.0).
- `app.rs` `overlay_style`: `Style::Problem` takes its colour and width from
  `problem_stroke()` (already wired in the skeleton).
- `solid_ui::mark_primitives(marks: &SolidMarks) -> Vec<Primitive>`: one
  `Primitive::Line { a, b, style: Style::Problem }` per entry of `marks.segments`, same
  order. Segments are already world space: no transform. Triangles are ignored (faces keep
  their scene tint). Empty marks give an empty list.
- `app.rs`, in the viewport painting right after the `overlay::selection(...)` primitives
  are painted: when `self.solid_marks.marks()` is `Some`, paint `mark_primitives(marks)` with
  `paint_primitives`, so the problem lines sit on top of the selection outline.
- Keep the scene line tint (`marked_scene`) as it is.

Do not edit the tests. `mark_primitives` and `problem_stroke` are `unimplemented!()` on main.

## Tests

`crates/modeler/tests/fix_marks.rs` (pure): empty marks, one line per segment with ends and
style, triangles alone give no lines, the stroke colour and width. The app.rs wiring is
checked by the orchestrator in the real window after merge: an STL box with the top missing,
selected, shows the 4 rim edges magenta over the orange outline.

## Where to look

`overlay::selection`, `overlay::Primitive`, `overlay::Style`, `overlay_style` and
`paint_primitives` in `app.rs`, `solid_ui::marked_scene`, `MarkTracker::marks`,
`modeler_dae::SolidMarks`, `modeler_dae::PROBLEM_COLOR`.
