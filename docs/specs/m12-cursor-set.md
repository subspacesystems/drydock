# m12-cursor-set: show the active tool's cursor in the viewport

Part of M12 (Tyrel). `m12-cursor-art` made the images and hotspots (`drydock::cursors`); this task shows them.

## What exists
- `drydock::cursors`: `image_png(tool)`, `hotspot(tool)` (both `None` for Select), `eyedropper_png`.
- `drydock::cursor_set` (skeleton, `unimplemented!()`): `effective_tool`, `cursor_image`, `apply`.
- The vendored egui-winit already turns `egui::PlatformOutput::cursor_image` into an OS cursor (`apply_cursor`, cached by the `Arc` address of `rgba`). Egui keeps the last `cursor_image` between frames, so it must be set (or cleared) explicitly. Prefer no vendor change; if one proves necessary, record it in `app/vendor/egui-winit/PATCHES.md`.
- `app.rs` (viewport input, near the `set_cursor_icon` calls) sets only `CursorIcon::None` while the eyedropper is armed, and the grip cursor (`Grip::cursor`) over scale/rotate/move grips.

## To do
1. `cursor_set::cursor_image(tool)`: decode `cursors::image_png(tool)` (the `image` crate is already a dependency) to straight RGBA, hotspot from `cursors::hotspot`, size 32x32 as `[u16; 2]`. Keep each decoded image in a `OnceLock`/static so repeat calls return the same `Arc<[u8]>`. Select: `None`.
2. `effective_tool(tool, middle_down, shift)`: middle held gives Orbit (Pan with shift); the Orbit tool with shift held gives Pan; anything else is `tool`. The shift rule matches how the app already reads these drags (`input.rs`).
3. `apply(ctx, tool, grip, middle_down, shift)`:
   - a grip cursor wins: `set_cursor_icon(grip)` and `set_cursor_image(None)`;
   - otherwise the effective tool's image via `set_cursor_image(Some(..))` (and `CursorIcon::Default` as the fallback icon); for Select, `set_cursor_image(None)` and `CursorIcon::Default`.
4. In `app.rs`, call `apply` from the viewport code in place of the `Grip::cursor` block, only while the pointer is over the viewport (`response.contains_pointer()`), with `middle_down` from the pointer's middle button (`ui.input(|i| i.pointer.middle_down())`) and `shift` from the modifiers. The armed-eyedropper block stays as it is (its own drawn cursor) and takes precedence. Outside the viewport nothing sets an image, so panels and menus keep the arrow: when the pointer is not over the viewport, call `ctx.set_cursor_image(None)` once so a leftover tool image is dropped.
5. Do not change tool behaviour or the drawn eyedropper; no new icons.

## Tests
`tests/cursor_set.rs` (kittest, in-process): Select has none and requests the arrow; every other tool has a 32x32 image equal to its PNG with the table hotspot on a non-transparent pixel; images decode once (same `Arc`); `apply` requests exactly that image per tool; different tools differ; a grip wins and clears the image; the image does not stick over frames (Line, Select, Rotate, grip); the effective-tool table and `apply` with middle/shift.

The OS cursor itself cannot be read back under Xvfb (screenshots do not include it), so there is no real-window test; the sign-off session checks the wiring in the real window.

## Where to look
- `show Tool` (`src/input.rs`), `show Grip::cursor` (`src/texture_handles.rs`)
- `show apply_cursor` in `app/vendor/egui-winit/src/lib.rs`; egui's `Context::set_cursor_image`, `CustomCursorImage`
- `show cursors::hotspot`, `cursors::image_png`
- `app.rs`: search `set_cursor_icon` and `eyedropper_armed` (viewport input fn, ~line 1595)
- `show window_icon::ICON_PNG` for an `include_bytes!`/decode example
