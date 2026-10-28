# m6-dialogs

Tyrel's round-3 feedback: dialogs should follow the Windows guidelines. Text box labels
end with a colon; the buttons sit at the bottom right on one line with a gap above; the
default button is highlighted and Enter presses it; buttons are larger with visible borders.

## Shared helpers (`app/crates/modeler/src/dialog.rs`, skeleton on main)
- `field_label(label)`: trailing whitespace removed, one ":" at the end (unchanged if present).
- `default_fill(visuals)`: `visuals.selection.bg_fill`.
- `button_row(ui, buttons)`: adds `ROW_GAP` (12 pt) of space, then the buttons on one line
  right-aligned, left to right in the order given. Each is at least `BUTTON_MIN_SIZE`
  (88 x 28 pt); the `default` one is filled with `default_fill`; disabled ones are inert.
  Returns the clicked index, or the index of the enabled default button when Enter is
  pressed this frame. Escape is the caller's.

## Style (`look::style`)
- `widgets.inactive.bg_stroke`: width at least 1, an opaque colour (the noninteractive stroke
  colour is fine), so buttons show a border at rest.
- `button_padding.y` at least 4; `interact_size.y` stays within 20 to 28 (existing test).

## Geolocation
- `geo_fields(ui, form) -> bool` and `geo_fields_with_globe(ui, form, view) -> bool`: the
  heading, optional globe and hint, the four fields and the error line, **without** an Apply
  button; true when Enter was pressed in a field. Field labels use `field_label`
  ("Latitude (deg):" ...).
- Keep `geo_panel` and `geo_panel_with_globe` working as today for their tests (fields plus
  an Apply button, built from the new functions and `button_row` with only Apply), apart
  from the colons.
- The Geolocation window (`sun_windows.rs`) draws `geo_fields_with_globe`, then one
  `button_row` of Apply (default, enabled only when the form parses) and Close. Apply, Enter
  in a field, or Enter with no field focused applies a valid form and closes the window;
  Close closes without applying.

## Other dialogs (no new tests; I check them in the real window)
Use `field_label` for text box labels and `button_row` for the buttons, Enter = default,
Escape = cancel where it already is:
- Make Group / Make Component prompt (`chrome.rs`): "Name:", OK default, Cancel.
- About modeler and Model Info (`chrome.rs`): Close in a right-aligned row. Model Info's
  "Geolocation..." button gets `BUTTON_MIN_SIZE` too.
- Shadows window Close (`sun_windows.rs`).
- Materials edit form and Purge confirmation (`materials_panel.rs`, `app.rs`): OK/Cancel rows,
  labels with colons. Existing tests that look for these buttons by label must keep passing.
- Leave the Customize Toolbars window alone (`m6-toolbar-customize-3` owns it).

## Tests
`tests/dialogs.rs` (new); `geo_panel.rs`, `geo_globe.rs`, `sun_windows.rs` only changed the
four field labels to end with ":".
