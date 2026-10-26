// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Components panel: the model's component definitions with drawn thumbnails, a search
//! box and a list / tiles switch. Clicking one asks to place an instance of it. Kept apart
//! from `App` so tests can drive it with `egui_kittest`; the panel returns what the user
//! asked for and the app applies it through the editor.
//!
//! Behaviour: `docs/specs/m6-components-panel.md`.

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Vec2, pos2, vec2};
use modeler_kernel::model::{BodyId, DefinitionId, Model};

use crate::icons::{self, Icon};

/// Side of a thumbnail in the list, in points.
pub const LIST_THUMB: f32 = 28.0;
/// Side of a tile's thumbnail, in points.
pub const TILE_THUMB: f32 = 64.0;

/// What the panel remembers between frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComponentsPanel {
    /// The search box's text.
    pub search: String,
    /// Tiles instead of a list.
    pub tiles: bool,
}

/// What the user asked for in the panel this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentAction {
    /// Start placing an instance of this definition.
    Place(DefinitionId),
    /// The Import button was clicked: the app asks for a file.
    Import,
}

/// Read the `.dae` or `.modeler` file at `path` as the source of an imported component:
/// its kernel model and the name for the definition (the file name without its extension).
/// `Err` is a one-line message for the user that names the file: the file can't be read, or
/// it has no usable model.
pub fn read_component_source(path: &std::path::Path) -> Result<(Model, String), String> {
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let document =
        crate::document::Document::open(path).map_err(|error| error.replace(['\r', '\n'], " "))?;
    let Some(model) = document.kernel else {
        let message = document.kernel_error.unwrap_or_else(|| {
            format!(
                "{}: no model",
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string())
            )
        });
        return Err(message.replace(['\r', '\n'], " "));
    };
    Ok((model, name))
}

/// The definitions the panel lists for `search`: components only (not groups) whose name
/// contains `search` ignoring case, ordered by name ignoring case, then by id.
pub fn listed(model: &Model, search: &str) -> Vec<DefinitionId> {
    let query = search.to_lowercase();
    let mut out: Vec<DefinitionId> = model
        .definitions()
        .filter(|(_, info)| !info.group && info.name.to_lowercase().contains(&query))
        .map(|(id, _)| id)
        .collect();
    out.sort_by(|a, b| {
        let name = |id: &DefinitionId| {
            model
                .definition(*id)
                .map_or_else(String::new, |info| info.name.to_lowercase())
        };
        name(a).cmp(&name(b)).then_with(|| a.cmp(b))
    });
    out
}

/// The definition's edges seen from the standard isometric direction, as segments inside
/// the unit square (y down), scaled uniformly so the larger extent is exactly 1 and
/// centred on the other axis. Empty when the definition has no edges.
pub fn thumbnail_segments(model: &Model, definition: DefinitionId) -> Vec<[egui::Pos2; 2]> {
    let Some(body) = model.body(BodyId::Definition(definition)) else {
        return Vec::new();
    };
    let geometry = body.geometry();
    let mut projected = Vec::new();
    for (_, edge) in geometry.edges() {
        let (Some(a), Some(b)) = (geometry.vertex(edge.ends[0]), geometry.vertex(edge.ends[1]))
        else {
            continue;
        };
        projected.push((project(a.position), project(b.position)));
    }
    if projected.is_empty() {
        return Vec::new();
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (a, b) in &projected {
        for &(x, y) in &[*a, *b] {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    let width = max_x - min_x;
    let height = max_y - min_y;
    let largest = width.max(height);
    let scale = if largest > 0.0 { 1.0 / largest } else { 1.0 };
    let offset_x = if width < largest {
        (1.0 - width * scale) / 2.0
    } else {
        0.0
    };
    let offset_y = if height < largest {
        (1.0 - height * scale) / 2.0
    } else {
        0.0
    };
    let to_screen = |(x, y): (f64, f64)| {
        pos2(
            ((x - min_x) * scale + offset_x) as f32,
            ((y - min_y) * scale + offset_y) as f32,
        )
    };
    projected
        .into_iter()
        .map(|(a, b)| [to_screen(a), to_screen(b)])
        .collect()
}

/// Project a model point with z up onto the screen (y down).
fn project(point: glam::DVec3) -> (f64, f64) {
    let angle = std::f64::consts::PI / 6.0;
    let (sin, cos) = angle.sin_cos();
    (
        (point.x - point.y) * cos,
        -(point.z + (point.x + point.y) * sin),
    )
}

/// Draw the panel.
pub fn components_panel(
    ui: &mut egui::Ui,
    panel: &mut ComponentsPanel,
    model: &Model,
) -> Option<ComponentAction> {
    ui.heading("Components");

    ui.add(
        egui::TextEdit::singleline(&mut panel.search)
            .hint_text("Search")
            .desired_width(f32::INFINITY),
    );

    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        if icons::icon_button(ui, Icon::List, "List", "Show as a list", !panel.tiles).clicked() {
            panel.tiles = false;
        }
        if icons::icon_button(ui, Icon::Tiles, "Tiles", "Show as tiles", panel.tiles).clicked() {
            panel.tiles = true;
        }
        if ui.button("Import").clicked() {
            action = Some(ComponentAction::Import);
        }
    });

    let entries = listed(model, &panel.search);
    let has_components = model.definitions().any(|(_, info)| !info.group);
    let tiles = panel.tiles;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if entries.is_empty() {
                ui.label(if has_components {
                    "No matches"
                } else {
                    "No components"
                });
                return;
            }
            let color = ui.visuals().text_color();
            if tiles {
                ui.horizontal_wrapped(|ui| {
                    for id in &entries {
                        let Some(info) = model.definition(*id) else {
                            continue;
                        };
                        let segments = thumbnail_segments(model, *id);
                        if tile(ui, &info.name, &segments, color).clicked() && action.is_none() {
                            action = Some(ComponentAction::Place(*id));
                        }
                    }
                });
            } else {
                for id in &entries {
                    let Some(info) = model.definition(*id) else {
                        continue;
                    };
                    let segments = thumbnail_segments(model, *id);
                    if list_row(ui, &info.name, &segments, color).clicked() && action.is_none() {
                        action = Some(ComponentAction::Place(*id));
                    }
                }
            }
        });
    action
}

/// One list row: a square thumbnail followed by the name.
fn list_row(ui: &mut egui::Ui, name: &str, segments: &[[Pos2; 2]], color: Color32) -> Response {
    let enabled = ui.is_enabled();
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, LIST_THUMB), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));

    let thumb = Rect::from_min_size(
        pos2(rect.left(), rect.center().y - LIST_THUMB / 2.0),
        Vec2::splat(LIST_THUMB),
    );
    paint_thumbnail(ui.painter(), thumb, segments, color);

    let spacing = ui.spacing().item_spacing.x;
    let text = Rect::from_min_max(pos2(thumb.right() + spacing, rect.top()), rect.max);
    paint_name(ui, text, name, color);

    response
}

/// One tile: a square thumbnail with the name under it.
fn tile(ui: &mut egui::Ui, name: &str, segments: &[[Pos2; 2]], color: Color32) -> Response {
    let enabled = ui.is_enabled();
    let spacing = ui.spacing().item_spacing.y;
    let text_height = ui.text_style_height(&egui::TextStyle::Button);
    let height = TILE_THUMB + spacing + text_height;
    let (rect, response) = ui.allocate_exact_size(vec2(TILE_THUMB, height), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));

    let thumb = Rect::from_min_size(rect.min, Vec2::splat(TILE_THUMB));
    paint_thumbnail(ui.painter(), thumb, segments, color);

    let text = Rect::from_min_max(pos2(rect.left(), thumb.bottom() + spacing), rect.max);
    paint_name(ui, text, name, color);

    response
}

/// Draw unit-square segments inside `rect`.
fn paint_thumbnail(painter: &egui::Painter, rect: Rect, segments: &[[Pos2; 2]], color: Color32) {
    let size = rect.size();
    let map = |p: Pos2| rect.min + vec2(p.x * size.x, p.y * size.y);
    let stroke = Stroke::new(1.0, color);
    for [a, b] in segments {
        painter.line_segment([map(*a), map(*b)], stroke);
    }
}

/// Draw `name` on one line, centred vertically and truncated to `rect`'s width.
fn paint_name(ui: &egui::Ui, rect: Rect, name: &str, color: Color32) {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let mut job = egui::text::LayoutJob::simple_singleline(name.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width().max(0.0));
    let galley = ui.painter().layout_job(job);
    let pos = pos2(rect.left(), rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(pos, galley, color);
}
