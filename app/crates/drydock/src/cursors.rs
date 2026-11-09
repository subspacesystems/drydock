// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Cursor images for the tools: the tool's brand icon with a dark outline as a
//! 32x32 PNG, and the pixel inside it that is the clicking point. Select has no image and
//! keeps the OS arrow. The images are written from `design/brand/icons/cursors/` by the
//! ignored test `write_cursor_pngs` in `tests/cursor_art.rs`; the table below is in 32 px
//! image coordinates (the 20-unit icon grid times 1.6, rounded).
//!
//! Behaviour: `docs/specs/m12-cursor-art.md`.

use crate::input::Tool;

/// Side of every cursor image in pixels.
pub const CURSOR_SIZE: u32 = 32;

/// The icon file name (`"line"`, `"erase"`, ...) of a tool's cursor; `None` for Select.
pub fn file_name(tool: Tool) -> Option<&'static str> {
    let _ = tool;
    unimplemented!("m12-cursor-art")
}

/// The tool's cursor as PNG bytes (embedded with `include_bytes!`); `None` for Select.
pub fn image_png(tool: Tool) -> Option<&'static [u8]> {
    let _ = tool;
    unimplemented!("m12-cursor-art")
}

/// The clicking point `(x, y)` in image pixels, from the milestone's hotspot table;
/// `None` for Select.
pub fn hotspot(tool: Tool) -> Option<(u32, u32)> {
    let _ = tool;
    unimplemented!("m12-cursor-art")
}

/// The Eyedropper cursor (Paint with Alt held); it is not a toolbar tool. Image
/// `eyedropper.png`.
pub fn eyedropper_png() -> &'static [u8] {
    unimplemented!("m12-cursor-art")
}

/// The Eyedropper's clicking point: its tip.
pub fn eyedropper_hotspot() -> (u32, u32) {
    unimplemented!("m12-cursor-art")
}
