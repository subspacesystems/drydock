// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Showing the active tool's cursor in the viewport. The images and hotspots
//! are in `cursors`; here they become `egui::CustomCursorImage`s, which the vendored
//! egui-winit turns into an OS cursor.
//!
//! Behaviour: `docs/specs/m12-cursor-set.md`.

use std::sync::OnceLock;

use crate::cursors;
use crate::input::Tool;
use eframe::egui;

/// egui documents `PlatformOutput::cursor_icon` and `cursor_image` as sticky between
/// frames, but its `end_pass` drains them from the context. Keep a copy in the context
/// output so the last request remains there for consumers and the next frame.
struct StickyCursorOutput;

impl egui::plugin::Plugin for StickyCursorOutput {
    fn debug_name(&self) -> &'static str {
        "sticky_cursor_output"
    }

    fn output_hook(&mut self, ctx: &egui::Context, output: &mut egui::FullOutput) {
        ctx.output_mut(|requested| {
            requested.cursor_icon = output.platform_output.cursor_icon;
            requested.cursor_image = output.platform_output.cursor_image.clone();
        });
    }
}

/// The tool whose cursor shows: Orbit, Pan and Zoom included. `middle_down` is the middle
/// button held over the viewport (Orbit, or Pan with `shift`); the Orbit tool with `shift`
/// held pans, so it shows Pan. Otherwise `tool` itself.
pub fn effective_tool(tool: Tool, middle_down: bool, shift: bool) -> Tool {
    if middle_down {
        if shift { Tool::Pan } else { Tool::Orbit }
    } else if tool == Tool::Orbit && shift {
        Tool::Pan
    } else {
        tool
    }
}

/// The cache slot for one tool's decoded cursor.
fn cache(tool: Tool) -> &'static OnceLock<Option<egui::CustomCursorImage>> {
    static ORBIT: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static PAN: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static ZOOM: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static SELECT: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static LINE: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static RECTANGLE: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static CIRCLE: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static ARC: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static ARC3: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static PUSH_PULL: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static ROTATE: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static MOVE: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static SCALE: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static TAPE: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static ERASE: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static PAINT: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static OFFSET: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();
    static FOLLOW_ME: OnceLock<Option<egui::CustomCursorImage>> = OnceLock::new();

    match tool {
        Tool::Orbit => &ORBIT,
        Tool::Pan => &PAN,
        Tool::Zoom => &ZOOM,
        Tool::Select => &SELECT,
        Tool::Line => &LINE,
        Tool::Rectangle => &RECTANGLE,
        Tool::Circle => &CIRCLE,
        Tool::Arc => &ARC,
        Tool::Arc3 => &ARC3,
        Tool::PushPull => &PUSH_PULL,
        Tool::Rotate => &ROTATE,
        Tool::Move => &MOVE,
        Tool::Scale => &SCALE,
        Tool::Tape => &TAPE,
        Tool::Erase => &ERASE,
        Tool::Paint => &PAINT,
        Tool::Offset => &OFFSET,
        Tool::FollowMe => &FOLLOW_ME,
    }
}

/// The tool's cursor decoded from `cursors::image_png` with `cursors::hotspot`; `None` for
/// Select. Decoded once per tool and kept: two calls return the same `Arc` (egui-winit
/// caches the OS cursor by its address).
pub fn cursor_image(tool: Tool) -> Option<egui::CustomCursorImage> {
    cache(tool)
        .get_or_init(|| {
            let png = cursors::image_png(tool)?;
            let image = image::load_from_memory(png).ok()?.to_rgba8();
            let (x, y) = cursors::hotspot(tool)?;
            Some(egui::CustomCursorImage {
                rgba: std::sync::Arc::from(image.into_raw()),
                size: [cursors::CURSOR_SIZE as u16; 2],
                hotspot: [x as u16, y as u16],
            })
        })
        .clone()
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
    ctx.add_plugin(StickyCursorOutput);

    if let Some(grip) = grip {
        ctx.set_cursor_image(None);
        ctx.set_cursor_icon(grip);
        return;
    }

    match cursor_image(effective_tool(tool, middle_down, shift)) {
        Some(image) => {
            ctx.set_cursor_icon(egui::CursorIcon::Default);
            ctx.set_cursor_image(Some(image));
        }
        None => {
            ctx.set_cursor_image(None);
            ctx.set_cursor_icon(egui::CursorIcon::Default);
        }
    }
}
