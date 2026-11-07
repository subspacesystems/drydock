# m10-color-picker-srgb

The model stores colors (Model Info background, sky, ground; material colors) as sRGB
components 0..1, and the renderer linearizes them, so the viewport draws #A8C8E8 exactly.
The color buttons use `color_edit_button_rgb`, which egui reads as linear, so the swatch
shows #D4E5F5 for that sky, the picker reads wrong numbers, and a picked color is stored
and drawn differently. Edit them as sRGB.

## Do
- `app/crates/drydock/src/color_ui.rs` (skeleton on main): implement
  - `srgb_to_bytes([f32;3]) -> [u8;3]`: `round(c*255)` clamped to 0..=255.
  - `bytes_to_srgb([u8;3]) -> [f32;3]`: `b/255`.
  - `srgb_color_button(ui, &mut [f32;3]) -> bool`: convert to bytes, show
    `ui.color_edit_button_srgb(&mut bytes)`; only when the u8 triple differs from the one
    shown, write `bytes_to_srgb` back and return true. Otherwise leave the f32 color
    untouched (opening the picker must not move a color that is not an exact /255).
- `background_ui::background_section`: use `srgb_color_button` for the three buttons
  (same logic as before: report `Some` only when it returns true).
- `materials_panel.rs` Color row (~line 277-283): use it on `form.color`.
- Do not edit `tests/background_model_info.rs`; it must keep passing.

## Tests
`tests/color_picker_srgb.rs`: byte round trip, rounding and clamping, no report without
interaction, no drift on an inexact color, swatch fills for default sky and ground equal
their sRGB bytes.

## Where to look
`show background_section`, `show Background`, `show DEFAULT_SKY`, the Color row in
`materials_panel.rs` (`show channel_to_byte` for the existing conversion style),
`tests/background_model_info.rs` (the `section` harness helper).
