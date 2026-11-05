# m10-brand-icons: the 24 brand icons from SVG, tinted by the theme

Replaces the hand-drawn vector shapes in `icons.rs` with the brand SVGs in
`design/brand/icons/`, drawn through egui_extras' SVG loader, and gives the active tool its
marker.

## Already on main
- Dependencies `egui_extras` (feature `svg` only) and, for tests, `usvg`. Licenses were checked:
  all new crates are MIT and/or Apache-2.0. If `cargo deny` still objects, stop and say so in
  your final note; do not add an exception.
- Stubs in `app/crates/drydock/src/icons.rs`: `name`, `svg`, `uri`, `install`, `SVG_SIZE`,
  `ACTIVE_BAR_HEIGHT`.
- `look::palette(theme)` (from `m10-brand-theme`) with `ink`, `accent`, `accent_soft`, `product`.

## What to build (all in `icons.rs`, plus one line in `app.rs`)
1. `name(icon)`: the file stem: tool names lowercase (`select`, `line`, `rectangle`, `circle`,
   `arc`, `arc3`, `pushpull`, `offset`, `followme`, `move`, `rotate`, `scale`, `tape`, `erase`,
   `paint`, `orbit`, `pan`, `zoom`), then `list`, `tiles`, `new`, `edit`, `purge`, `eyedropper`.
2. `svg(icon)`: `include_bytes!("../../../../design/brand/icons/<name>.svg")`, one per icon, straight from
   `design/brand/icons/` (path relative to `src/icons.rs`; count the `..` to reach the repo
   root). No copies of the files. A match over `Icon` with 24 arms, or a const table.
3. `uri(icon)`: a unique string ending `.svg`, e.g. `bytes://drydock-icon-<name>.svg`.
4. `install(ctx)`: `egui_extras::install_image_loaders(ctx)` and `ctx.include_bytes(uri, svg)`
   for every icon. Idempotent and cheap on repeat (guard with a flag in `ctx.data`, or accept
   the repeat). Call it once in `App::new` (next to `look::apply`) **and** from `icon_button`
   and `paint` (so tests and other harnesses need no setup).
5. Drawing. Icons are white ink; tint them with `egui::Image` (`.tint(color)`) or an
   equivalent textured quad with that tint. The theme comes from
   `ui.visuals().dark_mode` / `painter.ctx().style().visuals.dark_mode` -> `look::Theme` ->
   `look::palette`. Ink colors:
   - normal: `palette.ink`;
   - selected (active): `palette.accent`;
   - disabled: keep what the code does now (the visuals' weak/disabled stroke color).
   - hovered, not selected: `ink` (the hover fill stays as now).
6. Pixel size. The image is `SVG_SIZE` (20) points square, centred in the button (the button is
   `BUTTON_SIZE` = 28, so that is `rect.shrink(4.0)`, as now). Give it the exact size
   (`fit_to_exact_size(vec2(20, 20))` or `SizeHint::Size` = 20 x ppp) so the loader rasterises
   at 20 x pixels_per_point pixels: 20 px at 1x, 40 px at 2x. Do not let egui round the size
   hint up to something else or scale a 20 px texture.
7. `paint(painter, rect, icon, color)` keeps its signature (the menu icons and the eyedropper
   cursor use it): a tinted image filling `rect`, size hint = rect size x ppp. Use
   `painter.ctx().try_load_texture(...)` and `painter.image(...)`, or `Image::paint_at`
   through a throwaway child ui if simpler. While the texture is pending, paint nothing.
8. Active tool marker in `icon_button` (a button with `selected`):
   - fill `rect_filled(button_rect, 4.0, palette.accent_soft)` (replaces the old weak_bg_fill
     for selected);
   - icon tint `palette.accent`;
   - **only when `icon` is `Icon::Tool(_)`**: a bar `ACTIVE_BAR_HEIGHT` (2.0) tall, full
     button width, its bottom on the button's bottom edge, filled `palette.product`. This is
     the only orange in the working UI. Selected `List`/`Tiles` get the fill and tint but no
     bar.
   - Not selected: no accent-soft fill, no bar.
9. Remove the old drawing: `Draw`, `shapes`, `rotate`, `arc_points` and the helpers, and the
   module doc line about "no image files". `eyedropper_cursor_rect`, `CURSOR_SIZE`, `TOOLS`,
   `all`, `BUTTON_SIZE`, `icon_button`, `paint` stay. Update the module doc to say the icons
   come from `design/brand/icons/` and `svg` is public so cursors can reuse them.
10. `chrome.rs` and `app.rs` call `icons::paint` with a color: they keep working. Check that
   the menu icon color there is still sensible (it passes `visuals.fg_stroke.color`).

## Tests (already written; do not edit)
- `tests/icons_svg.rs`: names and order; `svg` equals the brand file bytes; every SVG parses
  with usvg and is 20 x 20; unique `.svg` URIs; `install` twice, bytes registered; every icon
  drawn by `icon_button` at 1x and 2x in Light and Dark has a 20 x 20 pt textured quad tinted
  with `palette.ink`, whose texture is 20 x ppp pixels square and has inked pixels; active
  tool marker (accent tint, accent-soft fill, exactly one `product` bar, 2 pt tall, button
  width, bottom-aligned); inactive: neither; selected `List`: fill and accent but no bar;
  `paint` draws a tinted quad in the given rect.
- `tests/icons.rs`: the old button tests (label, click, size, selected state, tooltip). The
  old shape-geometry tests and `icons_2.rs` are removed with the shapes.
- Other tests that use icon buttons (`materials_icons.rs`, `toolbar_*`) must still pass.

## Notes
- The SVGs use a `mask` and `stroke-linejoin="miter"`; resvg handles both. `egui_extras`
  without `svg_text` has no text support: none of the icons has text.
- Texture meta size is read from `ctx.tex_manager()` in the test; the loader's cache entry
  for the size hint you pass is the one the test inspects, so draw with exactly 20 x ppp.

## Where to look
- `show icons::icon_button`, `show icons::paint`, `show icons::all`, `show icons::name`
- `show look::palette`, `show look::Palette`, `show look::Theme`, `show look::style`
- `show App::new` (the `look::apply` line, `app.rs:222`)
- `toolbars.rs` ~l.785: the one toolbar caller of `icon_button` (needs no change)
- egui_extras: `install_image_loaders`; egui: `Image::tint`, `Context::include_bytes`,
  `Context::try_load_texture`
- Test helpers: `tests/icons_svg.rs` `draw`, `quads`, `fills`
