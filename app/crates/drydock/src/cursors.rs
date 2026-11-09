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

fn name_of(tool: Tool) -> Option<&'static str> {
    Some(match tool {
        Tool::Orbit => "orbit",
        Tool::Pan => "pan",
        Tool::Zoom => "zoom",
        Tool::Select => return None,
        Tool::Line => "line",
        Tool::Rectangle => "rectangle",
        Tool::Circle => "circle",
        Tool::Arc => "arc",
        Tool::Arc3 => "arc3",
        Tool::PushPull => "pushpull",
        Tool::Rotate => "rotate",
        Tool::Move => "move",
        Tool::Scale => "scale",
        Tool::Tape => "tape",
        Tool::Erase => "erase",
        Tool::Paint => "paint",
        Tool::Offset => "offset",
        Tool::FollowMe => "followme",
    })
}

/// The icon file name (`"line"`, `"erase"`, ...) of a tool's cursor; `None` for Select.
pub fn file_name(tool: Tool) -> Option<&'static str> {
    name_of(tool)
}

/// The tool's cursor as PNG bytes (embedded with `include_bytes!`); `None` for Select.
pub fn image_png(tool: Tool) -> Option<&'static [u8]> {
    Some(match tool {
        Tool::Orbit => include_bytes!("../assets/cursors/orbit.png"),
        Tool::Pan => include_bytes!("../assets/cursors/pan.png"),
        Tool::Zoom => include_bytes!("../assets/cursors/zoom.png"),
        Tool::Select => return None,
        Tool::Line => include_bytes!("../assets/cursors/line.png"),
        Tool::Rectangle => include_bytes!("../assets/cursors/rectangle.png"),
        Tool::Circle => include_bytes!("../assets/cursors/circle.png"),
        Tool::Arc => include_bytes!("../assets/cursors/arc.png"),
        Tool::Arc3 => include_bytes!("../assets/cursors/arc3.png"),
        Tool::PushPull => include_bytes!("../assets/cursors/pushpull.png"),
        Tool::Rotate => include_bytes!("../assets/cursors/rotate.png"),
        Tool::Move => include_bytes!("../assets/cursors/move.png"),
        Tool::Scale => include_bytes!("../assets/cursors/scale.png"),
        Tool::Tape => include_bytes!("../assets/cursors/tape.png"),
        Tool::Erase => include_bytes!("../assets/cursors/erase.png"),
        Tool::Paint => include_bytes!("../assets/cursors/paint.png"),
        Tool::Offset => include_bytes!("../assets/cursors/offset.png"),
        Tool::FollowMe => include_bytes!("../assets/cursors/followme.png"),
    })
}

/// The clicking point `(x, y)` in image pixels, from the milestone's hotspot table;
/// `None` for Select.
pub fn hotspot(tool: Tool) -> Option<(u32, u32)> {
    Some(match tool {
        Tool::Orbit => (16, 16),
        Tool::Pan => (16, 18),
        Tool::Zoom => (13, 13),
        Tool::Select => return None,
        Tool::Line => (25, 7),
        Tool::Rectangle => (25, 25),
        Tool::Circle => (16, 16),
        Tool::Arc => (16, 12),
        Tool::Arc3 => (26, 22),
        Tool::PushPull => (16, 22),
        Tool::Rotate => (16, 16),
        Tool::Move => (16, 16),
        Tool::Scale => (27, 5),
        Tool::Tape => (4, 16),
        Tool::Erase => (9, 24),
        Tool::Paint => (4, 22),
        Tool::Offset => (22, 22),
        Tool::FollowMe => (25, 7),
    })
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
