# m10-brand-theme: brand colors as egui Visuals

Supersedes `m6-native-look.md` for colors and corners (fonts are `m10-brand-fonts`; do
not touch them, nor `theme_from`, which still returns Light for `None`).

Source: `design/brand/README.md` ("Colors in the app" and "Shapes") and
`design/brand/tokens.json`.

## What to build (all in `app/crates/drydock/src/look.rs`)

1. `palette(theme) -> Palette` (struct and signature exist on main as a skeleton): fill
   in every field with the token's value for that theme (aliases resolved). Values are in
   the README tables and in `tests/look.rs::palette_holds_the_brand_tokens`.
   `accent_soft` is translucent: `Color32::from_rgba_unmultiplied(31, 213, 255, 41)` dark,
   `(0, 115, 156, 31)` light. No grid color: the app has no grid.
2. `style(theme)` builds its `Visuals` from the palette, no stock egui colors left:
   - `panel_fill` = surface; `window_fill` = surface_raised; `extreme_bg_color` = bg_sunken;
     `faint_bg_color` = bg; `code_bg_color` = bg_sunken.
   - `window_stroke` and `noninteractive.bg_stroke` = border (1 px); `inactive.bg_stroke` =
     border_control.
   - `fg_stroke` of every widget state = ink; `weak_text_color = Some(ink_muted)`.
   - hovered and active: `bg_fill` and `weak_bg_fill` = accent_soft, `bg_stroke` = accent.
     inactive: `weak_bg_fill` = surface, `bg_fill` = surface_raised (so hover differs).
     noninteractive: both fills surface. open: both fills surface_raised, stroke
     border_control.
   - `selection.bg_fill` = accent_soft, `selection.stroke` = accent, `hyperlink_color` =
     accent, `warn_fg_color` = warning, `error_fg_color` = danger.
   - Corners: all widget states 4; `menu_corner_radius` and `window_corner_radius` 8.
   - Flat: `window_shadow = Shadow::NONE`; keep egui's `popup_shadow`.
   - Spacing stays as it is.
3. Update the doc comments of `style` (the old "at most 3 points" wording goes).

## Tests

`app/crates/drydock/tests/look.rs` is already in the spec commit and cannot be changed:
corner radius 4/8, flat shadows, palette equals tokens, the Visuals mapping, WCAG contrast
(4.5:1 for ink and ink_muted on panel, window, faint and extreme fills; ink on accent_soft
over surface), and no Color32 outside the palette. `tests/dialogs.rs` must still pass
(default button fill equals `selection.bg_fill` and differs from an ordinary button's).
Run `harness/bin/run-tests -p drydock`.

## Where to look

- `show look::Palette`, `show look::palette`, `show look::style`, `show look::theme_from`
- `tests/look.rs`: `palette_holds_the_brand_tokens`, `the_visuals_use_no_stock_egui_colours`
- `tests/dialogs.rs`: `the_default_fill_is_not_an_ordinary_buttons`
- `design/brand/README.md`, "Colors in the app" table
