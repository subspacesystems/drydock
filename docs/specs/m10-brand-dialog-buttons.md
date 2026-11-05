# m10-brand-dialog-buttons: primary dialog buttons in the action colour

Builds on `m10-brand-theme` (`look::palette`, `look::style`) and `m10-brand-fonts`; those
merge first. Source: `design/brand/README.md`, "Colors in the app": primary buttons use
`action` (violet) with an `on-action` label; links, focus and selection stay `accent`.

## What to build

1. `look.rs` (skeletons on main): fill in
   - `theme_of(&Visuals) -> Theme`: `Dark` when `visuals.dark_mode`, else `Light`.
   - `primary_hover_fill(theme)`: `palette(theme).action` blended 10% toward white, per
     channel, `round(c + (255 - c) * 0.1)`.
   - `primary_button(ui, text) -> Response`: an `egui::Button` with fill `action`
     (`primary_hover_fill` while hovered or pressed), label `text` in `on_action`, size at
     least `ui.spacing().interact_size`. Theme from `theme_of(ui.visuals())`. Do it inside
     `ui.scope` by setting `weak_bg_fill` and `fg_stroke.color` of the inactive, hovered and
     active widget visuals, so the focus ring and the border keep the style's accent. Do
     **not** use `FontFamily::medium()` in the button (tests that don't install the brand
     fonts would panic with an unbound family).
   - Add a sized variant (`primary_button_sized(ui, text, min_size)`, `pub`) that
     `primary_button` calls; `button_row` needs it for its 88 x 28 minimum.
2. `dialog.rs::button_row`: a button is **primary** when `default` is true and its label is
   neither `"Cancel"` nor `"Close"`. A primary button is drawn with `primary_button_sized`
   (inside `ui.add_enabled_ui(button.enabled, ..)`, so a disabled one is inert), at the
   width `button_row` already computes. Every other button is drawn as before, including a
   default `"Cancel"` (still filled with `default_fill`: `confirm_dialog` makes Cancel the
   default and its action button is not violet). Clicks, the returned index and Enter work
   as before.
3. This makes these dialogs' confirming buttons primary with no change to their code
   (they already pass `default: true`): name prompt "OK" (`chrome.rs`), materials "OK" (two)
   and "Purge" (`materials_panel.rs`), Export "OK" (`mesh_export.rs`), mesh units "OK"
   (`mesh_import.rs`), 3D Text "OK" (`text_tool.rs`), Geolocation "Apply" (`geo_panel.rs`
   `apply_row`, `sun_windows.rs`). "Close" buttons (`chrome.rs` x2, `sun_windows.rs`) stay
   ordinary.
4. Direct `ui.button` calls in `app.rs` become `look::primary_button`: in `unsaved_dialog`
   "Save"; in the file dialog (`SaveDialog`) the confirming button `choose` ("Save" or
   "Export") and "Replace". Discard, Cancel and everything else there stay `ui.button`.
   Leave `components_panel.rs`'s "Import" (a panel button, not a dialog) and
   `print_setup.rs` ("Check fit" and the bed presets, which confirm nothing) alone.

## Tests

`tests/primary_buttons.rs` (in the spec commit, read-only) reads the painted shapes of the
last frame in both themes: `primary_button` fill `action`, label `on_action`, hover fill,
contrast of at least 4.5:1; Export, mesh units and 3D Text dialogs have a violet OK and an
ordinary Cancel; a default Apply is violet and a Close is not; a default Cancel or Close
and a row without a default have no violet button; `confirm_dialog` has none. The old
`tests/dialogs.rs` and `tests/dialogs_enter.rs` must still pass. Run
`harness/bin/run-tests -p drydock`.

## Where to look

- `show look::primary_button`, `show look::primary_hover_fill`, `show look::theme_of`,
  `show look::palette`, `show look::style`
- `show dialog::button_row`, `show dialog::default_fill`, `show dialog::confirm_dialog`
- `show unsaved_dialog` and `SaveDialog` (`outline app.rs`, around the "Replace" and
  `choose` buttons)
- tests: `painted`, `assert_primary`, `assert_ordinary` in `tests/primary_buttons.rs`;
  `row` in `tests/dialogs.rs`
