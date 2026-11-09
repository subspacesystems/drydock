// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Showing the active tool's cursor in the viewport. The images and hotspots
//! are in `cursors`; here they become `egui::CustomCursorImage`s, which the vendored
//! egui-winit turns into an OS cursor.
//!
//! Behaviour: `docs/specs/m12-cursor-set.md`.

use crate::input::Tool;
use eframe::egui;

/// The tool whose cursor shows: Orbit, Pan and Zoom included. `middle_down` is the middle
/// button held over the viewport (Orbit, or Pan with `shift`); the Orbit tool with `shift`
/// held pans, so it shows Pan. Otherwise `tool` itself.
pub fn effective_tool(tool: Tool, middle_down: bool, shift: bool) -> Tool {
    let _ = (tool, middle_down, shift);
    unimplemented!("m12-cursor-set")
}

/// The tool's cursor decoded from `cursors::image_png` with `cursors::hotspot`; `None` for
/// Select. Decoded once per tool and kept: two calls return the same `Arc` (egui-winit
/// caches the OS cursor by its address).
pub fn cursor_image(tool: Tool) -> Option<egui::CustomCursorImage> {
    let _ = tool;
    unimplemented!("m12-cursor-set")
}

/// Ask for this frame's viewport cursor, to be called while the pointer is over the
/// viewport. `grip` is the cursor of a scale/rotate/move grip under or held by the pointer
/// (`Grip::cursor`); it wins and no image is set. Otherwise the cursor image of
/// `effective_tool(tool, middle_down, shift)`, or for Select the arrow
/// (`CursorIcon::Default`, image cleared). Egui keeps the last cursor image between frames,
/// so every call sets the image explicitly (`Context::set_cursor_image`, `None` included).
pub fn apply(
    ctx: &egui::Context,
    tool: Tool,
    grip: Option<egui::CursorIcon>,
    middle_down: bool,
    shift: bool,
) {
    let _ = (ctx, tool, grip, middle_down, shift);
    unimplemented!("m12-cursor-set")
}
