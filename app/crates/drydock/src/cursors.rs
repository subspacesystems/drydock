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
    match tool {
        Tool::Select => None,
        Tool::Line => Some("line"),
        Tool::Rectangle => Some("rectangle"),
        Tool::Circle => Some("circle"),
        Tool::Arc => Some("arc"),
        Tool::Arc3 => Some("arc3"),
        Tool::PushPull => Some("pushpull"),
        Tool::Offset => Some("offset"),
        Tool::FollowMe => Some("followme"),
        Tool::Move => Some("move"),
        Tool::Rotate => Some("rotate"),
        Tool::Scale => Some("scale"),
        Tool::Tape => Some("tape"),
        Tool::Erase => Some("erase"),
        Tool::Paint => Some("paint"),
        Tool::Orbit => Some("orbit"),
        Tool::Pan => Some("pan"),
        Tool::Zoom => Some("zoom"),
    }
}

/// The tool's cursor as PNG bytes (embedded with `include_bytes!`); `None` for Select.
pub fn image_png(tool: Tool) -> Option<&'static [u8]> {
    match tool {
        Tool::Select => None,
        Tool::Line => Some(include_bytes!("../assets/cursors/line.png")),
        Tool::Rectangle => Some(include_bytes!("../assets/cursors/rectangle.png")),
        Tool::Circle => Some(include_bytes!("../assets/cursors/circle.png")),
        Tool::Arc => Some(include_bytes!("../assets/cursors/arc.png")),
        Tool::Arc3 => Some(include_bytes!("../assets/cursors/arc3.png")),
        Tool::PushPull => Some(include_bytes!("../assets/cursors/pushpull.png")),
        Tool::Offset => Some(include_bytes!("../assets/cursors/offset.png")),
        Tool::FollowMe => Some(include_bytes!("../assets/cursors/followme.png")),
        Tool::Move => Some(include_bytes!("../assets/cursors/move.png")),
        Tool::Rotate => Some(include_bytes!("../assets/cursors/rotate.png")),
        Tool::Scale => Some(include_bytes!("../assets/cursors/scale.png")),
        Tool::Tape => Some(include_bytes!("../assets/cursors/tape.png")),
        Tool::Erase => Some(include_bytes!("../assets/cursors/erase.png")),
        Tool::Paint => Some(include_bytes!("../assets/cursors/paint.png")),
        Tool::Orbit => Some(include_bytes!("../assets/cursors/orbit.png")),
        Tool::Pan => Some(include_bytes!("../assets/cursors/pan.png")),
        Tool::Zoom => Some(include_bytes!("../assets/cursors/zoom.png")),
    }
}

/// The clicking point `(x, y)` in image pixels, from the milestone's hotspot table;
/// `None` for Select.
pub fn hotspot(tool: Tool) -> Option<(u32, u32)> {
    match tool {
        Tool::Select => None,
        Tool::Line => Some((25, 7)),
        Tool::Rectangle => Some((25, 25)),
        Tool::Circle => Some((16, 16)),
        Tool::Arc => Some((16, 12)),
        Tool::Arc3 => Some((26, 22)),
        Tool::PushPull => Some((16, 22)),
        Tool::Offset => Some((22, 22)),
        Tool::FollowMe => Some((25, 7)),
        Tool::Move => Some((16, 16)),
        Tool::Rotate => Some((16, 16)),
        Tool::Scale => Some((27, 5)),
        Tool::Tape => Some((4, 16)),
        Tool::Erase => Some((9, 24)),
        Tool::Paint => Some((4, 22)),
        Tool::Orbit => Some((16, 16)),
        Tool::Pan => Some((16, 18)),
        Tool::Zoom => Some((13, 13)),
    }
}

/// The Eyedropper cursor (Paint with Alt held); it is not a toolbar tool. Image
/// `eyedropper.png`.
pub fn eyedropper_png() -> &'static [u8] {
    include_bytes!("../assets/cursors/eyedropper.png")
}

/// The Eyedropper's clicking point: its tip.
pub fn eyedropper_hotspot() -> (u32, u32) {
    (4, 28)
}
