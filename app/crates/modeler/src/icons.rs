// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The app's own icons, drawn as vector shapes so they scale to any size and take the
//! theme's text color. No image files: every icon is a few lines, circles and paths.
//!
//! Behaviour: `docs/specs/m6-icons.md`.

use crate::input::Tool;
use eframe::egui::{self, Color32, Rect, Response, Shape, Ui};

/// Side of an icon button in points (the icon is drawn inside, with a margin).
pub const BUTTON_SIZE: f32 = 28.0;

/// Something the UI shows as an icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    /// A tool's toolbar button.
    Tool(Tool),
    /// Materials panel: show the list.
    List,
    /// Materials panel: show large tiles.
    Tiles,
    /// Materials panel: new material.
    New,
    /// Materials panel: edit the current material.
    Edit,
    /// Materials panel: purge unused materials.
    Purge,
}

/// Every tool, in `chrome::TOOL_BUTTONS` order.
pub const TOOLS: [Tool; 16] = [
    Tool::Select,
    Tool::Line,
    Tool::Rectangle,
    Tool::Circle,
    Tool::Arc,
    Tool::Arc3,
    Tool::PushPull,
    Tool::Move,
    Tool::Rotate,
    Tool::Scale,
    Tool::Tape,
    Tool::Erase,
    Tool::Paint,
    Tool::Orbit,
    Tool::Pan,
    Tool::Zoom,
];

/// Every icon: the tools in `TOOLS` order, then List, Tiles, New, Edit, Purge.
pub fn all() -> Vec<Icon> {
    unimplemented!()
}

/// The shapes that draw `icon` inside `rect` in `color`. Only lines, paths, circles,
/// rectangles and beziers (no text, no textures); every color used is `color`, possibly
/// with a lower alpha.
pub fn shapes(icon: Icon, rect: Rect, color: Color32) -> Vec<Shape> {
    let _ = (icon, rect, color);
    unimplemented!()
}

/// Draw `icon` inside `rect`.
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    painter.extend(shapes(icon, rect, color));
}

/// A square `BUTTON_SIZE` button showing `icon`, drawn selected when `selected`. Screen
/// readers and tests find it by `label`; hovering shows `tooltip`.
pub fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    tooltip: &str,
    selected: bool,
) -> Response {
    let _ = (ui, icon, label, tooltip, selected);
    unimplemented!()
}
