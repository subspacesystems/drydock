// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The app's brand icons from `design/brand/icons/`, drawn as SVG images tinted by the
//! theme. `svg` is public so cursors can reuse them.
//!
//! Behaviour: `docs/specs/m10-brand-icons.md`.

use crate::input::Tool;
use crate::look;
use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Ui, Vec2, vec2};

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
    /// Materials panel: pick a material from the model.
    Eyedropper,
}

/// Every tool, in `chrome::TOOL_BUTTONS` order.
pub const TOOLS: [Tool; 18] = [
    Tool::Select,
    Tool::Line,
    Tool::Rectangle,
    Tool::Circle,
    Tool::Arc,
    Tool::Arc3,
    Tool::PushPull,
    Tool::Offset,
    Tool::FollowMe,
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

/// Every icon: the tools in `TOOLS` order, then List, Tiles, New, Edit, Purge, Eyedropper.
pub fn all() -> Vec<Icon> {
    let mut icons: Vec<Icon> = TOOLS.iter().map(|tool| Icon::Tool(*tool)).collect();
    icons.extend([
        Icon::List,
        Icon::Tiles,
        Icon::New,
        Icon::Edit,
        Icon::Purge,
        Icon::Eyedropper,
    ]);
    icons
}

/// Side of the SVG artwork in its own units (`viewBox="0 0 20 20"`).
pub const SVG_SIZE: f32 = 20.0;

/// Height in points of the bar under the active tool's button.
pub const ACTIVE_BAR_HEIGHT: f32 = 2.0;

/// The file name stem of `icon` in `design/brand/icons/` (`"pushpull"`, `"arc3"`, `"list"`).
/// `docs/specs/m10-brand-icons.md`.
pub fn name(icon: Icon) -> &'static str {
    match icon {
        Icon::Tool(Tool::Select) => "select",
        Icon::Tool(Tool::Line) => "line",
        Icon::Tool(Tool::Rectangle) => "rectangle",
        Icon::Tool(Tool::Circle) => "circle",
        Icon::Tool(Tool::Arc) => "arc",
        Icon::Tool(Tool::Arc3) => "arc3",
        Icon::Tool(Tool::PushPull) => "pushpull",
        Icon::Tool(Tool::Offset) => "offset",
        Icon::Tool(Tool::FollowMe) => "followme",
        Icon::Tool(Tool::Move) => "move",
        Icon::Tool(Tool::Rotate) => "rotate",
        Icon::Tool(Tool::Scale) => "scale",
        Icon::Tool(Tool::Tape) => "tape",
        Icon::Tool(Tool::Erase) => "erase",
        Icon::Tool(Tool::Paint) => "paint",
        Icon::Tool(Tool::Orbit) => "orbit",
        Icon::Tool(Tool::Pan) => "pan",
        Icon::Tool(Tool::Zoom) => "zoom",
        Icon::List => "list",
        Icon::Tiles => "tiles",
        Icon::New => "new",
        Icon::Edit => "edit",
        Icon::Purge => "purge",
        Icon::Eyedropper => "eyedropper",
    }
}

/// The icon's SVG source, embedded with `include_bytes!` from `design/brand/icons/`.
pub fn svg(icon: Icon) -> &'static [u8] {
    match icon {
        Icon::Tool(Tool::Select) => include_bytes!("../../../../design/brand/icons/select.svg"),
        Icon::Tool(Tool::Line) => include_bytes!("../../../../design/brand/icons/line.svg"),
        Icon::Tool(Tool::Rectangle) => {
            include_bytes!("../../../../design/brand/icons/rectangle.svg")
        }
        Icon::Tool(Tool::Circle) => include_bytes!("../../../../design/brand/icons/circle.svg"),
        Icon::Tool(Tool::Arc) => include_bytes!("../../../../design/brand/icons/arc.svg"),
        Icon::Tool(Tool::Arc3) => include_bytes!("../../../../design/brand/icons/arc3.svg"),
        Icon::Tool(Tool::PushPull) => {
            include_bytes!("../../../../design/brand/icons/pushpull.svg")
        }
        Icon::Tool(Tool::Offset) => include_bytes!("../../../../design/brand/icons/offset.svg"),
        Icon::Tool(Tool::FollowMe) => {
            include_bytes!("../../../../design/brand/icons/followme.svg")
        }
        Icon::Tool(Tool::Move) => include_bytes!("../../../../design/brand/icons/move.svg"),
        Icon::Tool(Tool::Rotate) => include_bytes!("../../../../design/brand/icons/rotate.svg"),
        Icon::Tool(Tool::Scale) => include_bytes!("../../../../design/brand/icons/scale.svg"),
        Icon::Tool(Tool::Tape) => include_bytes!("../../../../design/brand/icons/tape.svg"),
        Icon::Tool(Tool::Erase) => include_bytes!("../../../../design/brand/icons/erase.svg"),
        Icon::Tool(Tool::Paint) => include_bytes!("../../../../design/brand/icons/paint.svg"),
        Icon::Tool(Tool::Orbit) => include_bytes!("../../../../design/brand/icons/orbit.svg"),
        Icon::Tool(Tool::Pan) => include_bytes!("../../../../design/brand/icons/pan.svg"),
        Icon::Tool(Tool::Zoom) => include_bytes!("../../../../design/brand/icons/zoom.svg"),
        Icon::List => include_bytes!("../../../../design/brand/icons/list.svg"),
        Icon::Tiles => include_bytes!("../../../../design/brand/icons/tiles.svg"),
        Icon::New => include_bytes!("../../../../design/brand/icons/new.svg"),
        Icon::Edit => include_bytes!("../../../../design/brand/icons/edit.svg"),
        Icon::Purge => include_bytes!("../../../../design/brand/icons/purge.svg"),
        Icon::Eyedropper => include_bytes!("../../../../design/brand/icons/eyedropper.svg"),
    }
}

/// The image URI the icon is registered under in egui's loaders (ends in `.svg`).
pub fn uri(icon: Icon) -> String {
    format!("bytes://drydock-icon-{}.svg", name(icon))
}

/// Installs egui_extras' image loaders and registers every icon's bytes under `uri(icon)`.
/// Cheap and safe to call again; `icon_button` and `paint` call it themselves.
pub fn install(ctx: &egui::Context) {
    let installed = egui::Id::new("drydock-icon-loaders-installed");
    if ctx.data(|data| data.get_temp::<bool>(installed).unwrap_or(false)) {
        return;
    }
    ctx.data_mut(|data| data.insert_temp(installed, true));
    egui_extras::install_image_loaders(ctx);
    for icon in all() {
        ctx.include_bytes(uri(icon), svg(icon));
    }
}

/// Side of the eyedropper cursor in points.
pub const CURSOR_SIZE: f32 = 24.0;

/// Where to draw `Icon::Eyedropper` as the mouse cursor for a pointer at `pointer`: a
/// `CURSOR_SIZE` square placed so the pipette's tip is at the pointer
/// (`docs/specs/m6-eyedropper-2.md`).
pub fn eyedropper_cursor_rect(pointer: Pos2) -> Rect {
    let half = CURSOR_SIZE * 0.45;
    let tip = vec2(-0.72, 0.72);
    Rect::from_center_size(pointer - tip * half, Vec2::splat(CURSOR_SIZE))
}

/// Draw `icon` inside `rect`.
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let ctx = painter.ctx();
    install(ctx);
    let pixel_size = (rect.size() * ctx.pixels_per_point()).round();
    let size_hint = egui::load::SizeHint::Size {
        width: pixel_size.x.max(1.0) as u32,
        height: pixel_size.y.max(1.0) as u32,
        maintain_aspect_ratio: false,
    };
    let loaded = ctx.try_load_texture(&uri(icon), egui::TextureOptions::LINEAR, size_hint);
    if let Ok(egui::load::TexturePoll::Ready { texture }) = loaded {
        painter.image(
            texture.id,
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            color,
        );
    }
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
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(BUTTON_SIZE), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label)
    });

    let visuals = ui.style().interact_selectable(&response, selected);
    let palette = look::palette(look::theme_of(ui.visuals()));
    let color = if !enabled {
        visuals.fg_stroke.color
    } else if selected {
        palette.accent
    } else {
        palette.ink
    };

    if selected {
        ui.painter()
            .rect_filled(rect.shrink(0.1), 4.0, palette.accent_soft);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, visuals.weak_bg_fill);
    }

    install(ui.ctx());
    egui::Image::new(uri(icon))
        .tint(color)
        .paint_at(ui, rect.shrink(4.0));

    if selected && matches!(icon, Icon::Tool(_)) {
        let bar = Rect::from_min_max(
            Pos2::new(rect.min.x, rect.max.y - ACTIVE_BAR_HEIGHT),
            rect.max,
        );
        ui.painter().rect_filled(bar, 0.0, palette.product);
    }

    response.on_hover_text(tooltip)
}
