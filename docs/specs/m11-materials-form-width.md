# m11-materials-form-width: the Materials form keeps the right dock's width

Sign-off finding (present since the Materials form was added): in the Materials panel,
docked right, clicking New... widens the dock by about 12 px at once, and each click into a
field or scroll widens it further (1040 px -> ~920 -> ~690 in the measured session). The
Opacity number and the Texture field then run past the dock's right edge. The Edit form
behaves the same.

## Cause
The form's rows are sized from `ui.available_width()` or `f32::INFINITY` while the
resizable dock panel grows to fit whatever a row asks for. The Opacity row computes the
slider width as the room minus the value box (`interact_size.x`) and one item spacing, but
the slider's value box with its padding and the spacing before it take more than that, so the
row is a little too wide; the dock grows to fit, the next frame computes from the larger room,
and the error repeats. The Texture field and the Name field use `desired_width(INFINITY)`
in a layout that is then widened by that row.

## Required behaviour
- Opening the New or Edit form, clicking into each field, dragging or typing in it, and
  scrolling the panel never change the dock's width (within 1 px).
- Every form widget (Name, Opacity slider and its number, Texture image field, Width and
  Height numbers, OK, Cancel) lies inside the dock's rect, also with a very long texture path
  typed in.
- A slider row must reserve everything the row needs (the value box with its padding and
  all item spacings, plus the scroll bar's width when the list scrolls), not only the box's
  minimum width. Do not fix it by a magic constant that fits only one font: size the widgets
  from `ui.spacing()` values (`interact_size`, `item_spacing`, `button_padding`,
  `scroll.bar_width` etc.) or lay the row out so that the leftover width goes to the
  slider last.
- Appearance otherwise unchanged; the existing `materials_*.rs` tests stay green.

## Tests (already written, do not edit)
`app/crates/drydock/tests/materials_form_width.rs`: the panel inside a resizable right
`egui::Panel` with a `ScrollArea` (280 px, 40 materials so the list scrolls). Five tests: dock
width after opening the form, after clicking each field, after scrolling, for the Edit form,
and with a long texture path; each also checks that the text, number and slider widgets and
buttons lie inside the dock. They fail today (280 -> ~292 on opening, ~300 after clicks).
A trial change to the Opacity row alone (reserving about `3 * item_spacing + 16` instead of
`item_spacing`) made all five pass, so that row is the first place to fix; replace the
constant by spacing-derived values.

## Where to look
`materials_panel` (the `ScrollArea` with id salt `materials-list`, the form's rows: Name,
Color, Opacity, Texture image, Width/Height), `MaterialForm`, `look::slider`,
`dialog::field_label`, `dialog::button_row`, `units_ui::length_drag`; test helpers `harness`,
`settle`, `assert_inside`, `click_every_field` in `materials_form_width.rs`. Similar fixes:
`docs/specs/m10-tags-rename-width.md`.
