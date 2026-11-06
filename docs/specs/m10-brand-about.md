# m10-brand-about: Help > About with the brand lockup

The About window shows the horizontal "Subspace Drydock" lockup for the current
theme, then the version and "A Subspace Systems product.". No headline font: the lockup is
already outlines.

## Already on main
- `design/brand/logo/drydock-lockup-h-dark.svg` (light ink for dark backgrounds) and
  `drydock-lockup-h-light.svg` (the dark one with `#EEF7FF` and `#1FD5FF` changed to
  `#091126`; the orange `#FF8A3D` stays). Both are committed; do not edit them.
- Stubs in `chrome.rs`: `PRODUCT_LINE`, `LOCKUP_WIDTH` (341 pt), `lockup_svg`, `lockup_uri`,
  `install_lockups`.
- The SVG loader comes from `m10-brand-icons` (`egui_extras` with `svg`; `icons::install`
  calls `egui_extras::install_image_loaders`). This task builds on it.

## What to build (all in `chrome.rs`)
1. `lockup_svg(dark)`: `include_bytes!` of the dark or light file, path relative to
   `src/chrome.rs` (`../../../../design/brand/logo/...`, as `CONTRIBUTORS` does). No copies.
2. `lockup_uri(dark)`: two distinct strings ending `.svg`, e.g. `bytes://drydock-lockup-dark.svg`.
3. `install_lockups(ctx)`: `egui_extras::install_image_loaders(ctx)` and
   `ctx.include_bytes(uri, svg)` for both. Repeat calls must be harmless.
4. `about_lines()` becomes: `about_text()`, `PRODUCT_LINE`, the `License:` line, the
   `Copyright (c) 2026 Subspace Systems` line, `Contributors:`, then the contributor names
   (as now). The two old lines "Subspace Drydock" and "by Subspace Systems" go: the lockup
   replaces them. No line is empty.
5. `about_window`: call `install_lockups(ctx)`, then at the top of the window draw
   `egui::Image::new(lockup_uri(dark))` with
   `fit_to_exact_size(vec2(LOCKUP_WIDTH, LOCKUP_WIDTH * 748.0 / 3410.0))`, no tint, where
   `dark = ui.visuals().dark_mode` (read it every frame, so a theme switch while the window
   is open swaps the lockup). Then `ui.label` for each `about_lines()` entry, then the Close
   row as now. The window title stays "About Drydock". Give the image an alt text
   "Subspace Drydock". Exact size means egui rasterises at `LOCKUP_WIDTH * ppp` pixels:
   crisp at 2x.
6. Keep `about_text()`, `CONTRIBUTORS`, `VERSION`, `GIT_HASH` as they are (version still comes
   from the build's `DRYDOCK_VERSION`, which defaults to `CARGO_PKG_VERSION`).

## Tests (already written; do not edit)
- `tests/about_lockup.rs`: the two lockups equal the files, the light one equals the colour
  swap of the dark one; URIs; `install_lockups` twice registers both; `about_lines` order;
  in Light and Dark at 1x and 2x the About window has the title, version, product line and
  Close, exactly one untinted image of `LOCKUP_WIDTH` points whose texture is the theme's
  lockup (not the other's), has the real pixel width, the right ink colours (light ink on
  dark, navy on light, orange in both), and sits above the version; switching theme while
  open switches the image.
- `tests/menus.rs` and `tests/drydock_text.rs`: the existing About tests, updated for the new
  lines.

## Where to look
- `show chrome::about_window`, `show chrome::about_lines`, `show chrome::about_text`
- `show icons::install` (how the icons register bytes and install loaders)
- `show look::style`, `show look::palette` (themes; `ui.visuals().dark_mode` picks the lockup)
- egui: `Image::fit_to_exact_size`, `Image::alt_text`, `Context::include_bytes`
- Test helpers: `tests/about_lockup.rs` `open_about`, `quads`, `textures_of`, `colour_counts`
