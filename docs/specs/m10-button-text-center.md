# m10-button-text-center: button labels centered vertically

Feedback: in a button taller than its text, the label sits too high. Model Info's Close
(`dialog::BUTTON_MIN_SIZE`, 88x28) has about 5 px above the text and 11 below; small buttons are fine.

## Cause
In a right-to-left layout (the dialog `button_row`), egui 0.36 places the label at the top of the
button's rect when the rect is taller than the label. The same happens wherever
`egui::Button::min_size` is used inside such a layout.

## Do
- Fix it centrally: either the layout/alignment in `dialog::button_row`, or by making the
  button's content centered itself. `look::sized_button(ui, text, min_size)` (skeleton on main,
  currently a plain egui button) must return a button of at least `min_size` whose label is
  centered vertically and horizontally in any layout. `look::primary_button_sized` must do the same.
- Use `look::sized_button` for the plain dialog buttons in `button_row` and for the
  "Geolocation..." button in `app.rs` (the only other `min_size` user; grep to confirm).
- Keep the row's behavior: order, right edge, gap above, default fill, Enter, enabled state, width shrinking.
- Small buttons (`ui.button`) stay as they are.

## Tests
`crates/drydock/tests/button_text_center.rs` (given, do not edit): the painted galley's vertical
center is within 1 px of the button rect's center for dialog buttons (both themes, primary and
plain), `primary_button_sized`, `sized_button`, in right-to-left and top-down layouts. The existing
`dialogs.rs`, `primary_buttons.rs` tests must keep passing.

## Where to look
`dialog::button_row`, `dialog::BUTTON_MIN_SIZE`, `look::primary_button_sized`, `look::sized_button`,
`look::primary_button`, the Geolocation button near `app.rs:3165`; tests: `dialogs.rs`, `primary_buttons.rs`.
