// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Side panel layout: which panels are open, which are docked in the left or right column
//! (and in what order, collapsed or not), which float as windows, and how wide each column
//! is. Kept in a text file in the config folder between runs. Drawing is elsewhere.
//!
//! Behaviour: `docs/specs/m6-dock-layout.md`.

use eframe::egui::{Pos2, Rect};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// First line of the layout file.
pub const HEADER: &str = "modeler panels 1";
/// Column width limits and default, in points.
pub const MIN_WIDTH: f32 = 160.0;
pub const MAX_WIDTH: f32 = 600.0;
pub const DEFAULT_WIDTH: f32 = 240.0;
/// Smallest floating panel, in points.
pub const MIN_FLOAT_SIZE: (f32, f32) = (120.0, 80.0);
/// How close to a window edge (points) a dragged panel must be to dock there.
pub const EDGE_ZONE: f32 = 40.0;

/// A panel that can dock or float.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelId {
    EntityInfo,
    Materials,
    Components,
    Shadows,
}

/// Every panel, in this order.
pub const PANELS: [PanelId; 4] = [
    PanelId::EntityInfo,
    PanelId::Materials,
    PanelId::Components,
    PanelId::Shadows,
];

/// A docking column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

/// Where a panel sits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// In the `side` column; panels in a column are ordered by `slot` (0 at the top).
    Docked { side: Side, slot: u32 },
    /// A window with its top-left corner at (`x`, `y`) and size `w` x `h`, in points.
    Floating { x: f32, y: f32, w: f32, h: f32 },
}

/// One panel's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    pub id: PanelId,
    pub open: bool,
    /// Docked panels can be collapsed to their title row.
    pub collapsed: bool,
    pub place: Place,
}

/// Where a dragged panel would go if dropped now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropTarget {
    Dock { side: Side, position: usize },
    Float,
}

/// All panels and both column widths.
#[derive(Clone, Debug, PartialEq)]
pub struct DockLayout {
    panels: Vec<Panel>,
    widths: [f32; 2],
}

impl Default for DockLayout {
    /// Entity Info and Materials open in the right column (in that order), Components and
    /// Shadows closed but docked under them; both columns `DEFAULT_WIDTH`; nothing
    /// collapsed.
    fn default() -> DockLayout {
        let panels = PANELS
            .iter()
            .enumerate()
            .map(|(slot, &id)| Panel {
                id,
                open: slot < 2,
                collapsed: false,
                place: Place::Docked {
                    side: Side::Right,
                    slot: slot as u32,
                },
            })
            .collect();
        DockLayout {
            panels,
            widths: [DEFAULT_WIDTH; 2],
        }
    }
}

fn side_index(side: Side) -> usize {
    match side {
        Side::Left => 0,
        Side::Right => 1,
    }
}

fn parse_side(word: &str) -> Option<Side> {
    match word {
        "left" => Some(Side::Left),
        "right" => Some(Side::Right),
        _ => None,
    }
}

fn parse_flag(word: &str) -> Option<bool> {
    match word {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn slot_of(panel: &Panel) -> u32 {
    match panel.place {
        Place::Docked { slot, .. } => slot,
        Place::Floating { .. } => u32::MAX,
    }
}

impl DockLayout {
    /// The panel's state.
    pub fn panel(&self, id: PanelId) -> &Panel {
        self.panels
            .iter()
            .find(|panel| panel.id == id)
            .expect("the layout holds one panel per id")
    }

    fn panel_mut(&mut self, id: PanelId) -> &mut Panel {
        self.panels
            .iter_mut()
            .find(|panel| panel.id == id)
            .expect("the layout holds one panel per id")
    }

    /// Renumber the slots on both sides by slot, ties in `PANELS` order.
    fn renumber(&mut self) {
        for side in [Side::Left, Side::Right] {
            let mut indices: Vec<usize> = (0..self.panels.len())
                .filter(
                    |&i| matches!(self.panels[i].place, Place::Docked { side: s, .. } if s == side),
                )
                .collect();
            indices.sort_by_key(|&i| (slot_of(&self.panels[i]), i));
            for (slot, i) in indices.into_iter().enumerate() {
                self.panels[i].place = Place::Docked {
                    side,
                    slot: slot as u32,
                };
            }
        }
    }

    pub fn is_open(&self, id: PanelId) -> bool {
        self.panel(id).open
    }

    /// Open or close a panel; where it sits is kept.
    pub fn set_open(&mut self, id: PanelId, open: bool) {
        self.panel_mut(id).open = open;
        self.renumber();
    }

    /// Show a panel: open it and expand it. Used by the View menu.
    pub fn reveal(&mut self, id: PanelId) {
        let panel = self.panel_mut(id);
        panel.open = true;
        panel.collapsed = false;
    }

    pub fn toggle(&mut self, id: PanelId) {
        let open = self.is_open(id);
        self.set_open(id, !open);
    }

    pub fn set_collapsed(&mut self, id: PanelId, collapsed: bool) {
        self.panel_mut(id).collapsed = collapsed;
    }

    /// The open panels docked in `side`'s column, top to bottom.
    pub fn docked(&self, side: Side) -> Vec<PanelId> {
        let mut panels: Vec<&Panel> = self
            .panels
            .iter()
            .filter(|panel| {
                panel.open && matches!(panel.place, Place::Docked { side: s, .. } if s == side)
            })
            .collect();
        panels.sort_by_key(|panel| slot_of(panel));
        panels.into_iter().map(|panel| panel.id).collect()
    }

    /// The open floating panels, in `PANELS` order.
    pub fn floating(&self) -> Vec<PanelId> {
        self.panels
            .iter()
            .filter(|panel| panel.open && matches!(panel.place, Place::Floating { .. }))
            .map(|panel| panel.id)
            .collect()
    }

    /// Dock `id` in `side`'s column at `position` in `docked(side)` (the end when past it),
    /// and open it.
    pub fn dock(&mut self, id: PanelId, side: Side, position: usize) {
        let id_index = self
            .panels
            .iter()
            .position(|panel| panel.id == id)
            .expect("the layout holds one panel per id");

        let mut full: Vec<usize> = (0..self.panels.len())
            .filter(|&i| i != id_index)
            .filter(|&i| matches!(self.panels[i].place, Place::Docked { side: s, .. } if s == side))
            .collect();
        full.sort_by_key(|&i| (slot_of(&self.panels[i]), i));

        let open: Vec<usize> = full
            .iter()
            .copied()
            .filter(|&i| self.panels[i].open)
            .collect();
        let at = position.min(open.len());
        let insert = if at < open.len() {
            full.iter()
                .position(|&i| i == open[at])
                .expect("the open panel is in the column")
        } else {
            full.len()
        };
        full.insert(insert, id_index);

        for (slot, i) in full.into_iter().enumerate() {
            self.panels[i].place = Place::Docked {
                side,
                slot: slot as u32,
            };
        }
        self.panels[id_index].open = true;
        self.renumber();
    }

    /// Float `id` as a window covering `rect` (size raised to `MIN_FLOAT_SIZE`), and open
    /// it.
    pub fn float(&mut self, id: PanelId, rect: Rect) {
        let x = rect.min.x;
        let y = rect.min.y;
        let w = rect.width();
        let h = rect.height();
        if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) {
            return;
        }
        let panel = self.panel_mut(id);
        panel.open = true;
        panel.place = Place::Floating {
            x,
            y,
            w: w.max(MIN_FLOAT_SIZE.0),
            h: h.max(MIN_FLOAT_SIZE.1),
        };
        self.renumber();
    }

    /// The `side` column's width.
    pub fn width(&self, side: Side) -> f32 {
        self.widths[side_index(side)]
    }

    /// Set the `side` column's width, clamped to `MIN_WIDTH..=MAX_WIDTH`; not a number is
    /// ignored.
    pub fn set_width(&mut self, side: Side, width: f32) {
        if width.is_nan() {
            return;
        }
        self.widths[side_index(side)] = width.clamp(MIN_WIDTH, MAX_WIDTH);
    }

    /// The column a panel dragged to `pointer` would dock in, in a window covering
    /// `screen`; `None` means it would float.
    pub fn drop_side(&self, screen: Rect, pointer: Pos2) -> Option<Side> {
        let reach = |side: Side| {
            if self.docked(side).is_empty() {
                EDGE_ZONE
            } else {
                self.width(side).max(EDGE_ZONE)
            }
        };
        let in_left = pointer.x <= screen.left() + reach(Side::Left);
        let in_right = pointer.x >= screen.right() - reach(Side::Right);
        match (in_left, in_right) {
            (true, false) => Some(Side::Left),
            (false, true) => Some(Side::Right),
            (true, true) => {
                let to_left = pointer.x - screen.left();
                let to_right = screen.right() - pointer.x;
                Some(if to_left <= to_right {
                    Side::Left
                } else {
                    Side::Right
                })
            }
            (false, false) => None,
        }
    }

    /// Back to `DockLayout::default()`.
    pub fn reset(&mut self) {
        *self = DockLayout::default();
    }

    /// The layout from the file's text; the default layout when the text isn't a layout.
    pub fn parse(text: &str) -> DockLayout {
        let mut lines = text.lines();
        match lines.next() {
            Some(line) if line == HEADER => {}
            _ => return DockLayout::default(),
        }

        let mut layout = DockLayout::default();
        for line in lines {
            let fields: Vec<&str> = line.split_whitespace().collect();
            match fields.first().copied() {
                Some("width") => {
                    if fields.len() != 3 {
                        continue;
                    }
                    let Some(side) = parse_side(fields[1]) else {
                        continue;
                    };
                    let Ok(width) = fields[2].parse::<f32>() else {
                        continue;
                    };
                    if !width.is_finite() {
                        continue;
                    }
                    layout.set_width(side, width);
                }
                Some("panel") => {
                    if fields.len() < 5 {
                        continue;
                    }
                    let Some(id) = PANELS
                        .iter()
                        .copied()
                        .find(|id| panel_key(*id) == fields[1])
                    else {
                        continue;
                    };
                    let Some(open) = parse_flag(fields[2]) else {
                        continue;
                    };
                    let Some(collapsed) = parse_flag(fields[3]) else {
                        continue;
                    };
                    let place = match fields[4] {
                        "left" | "right" => {
                            if fields.len() != 6 {
                                continue;
                            }
                            let side = parse_side(fields[4]).expect("side is left or right");
                            let Ok(slot) = fields[5].parse::<u32>() else {
                                continue;
                            };
                            Place::Docked { side, slot }
                        }
                        "float" => {
                            if fields.len() != 9 {
                                continue;
                            }
                            let mut values = [0.0f32; 4];
                            let mut ok = true;
                            for (value, field) in values.iter_mut().zip(&fields[5..9]) {
                                match field.parse::<f32>() {
                                    Ok(number) if number.is_finite() => *value = number,
                                    _ => {
                                        ok = false;
                                        break;
                                    }
                                }
                            }
                            if !ok {
                                continue;
                            }
                            Place::Floating {
                                x: values[0],
                                y: values[1],
                                w: values[2].max(MIN_FLOAT_SIZE.0),
                                h: values[3].max(MIN_FLOAT_SIZE.1),
                            }
                        }
                        _ => continue,
                    };
                    let panel = layout.panel_mut(id);
                    panel.open = open;
                    panel.collapsed = collapsed;
                    panel.place = place;
                }
                _ => {}
            }
        }
        layout.renumber();
        layout
    }

    /// The file's text for this layout.
    pub fn to_text(&self) -> String {
        use std::fmt::Write;

        let mut text = String::new();
        let _ = writeln!(text, "{HEADER}");
        let _ = writeln!(text, "width left {}", self.width(Side::Left));
        let _ = writeln!(text, "width right {}", self.width(Side::Right));
        for panel in &self.panels {
            let key = panel_key(panel.id);
            let open = if panel.open { 1 } else { 0 };
            let collapsed = if panel.collapsed { 1 } else { 0 };
            match panel.place {
                Place::Docked { side, slot } => {
                    let side = match side {
                        Side::Left => "left",
                        Side::Right => "right",
                    };
                    let _ = writeln!(text, "panel {key} {open} {collapsed} {side} {slot}");
                }
                Place::Floating { x, y, w, h } => {
                    let _ = writeln!(text, "panel {key} {open} {collapsed} float {x} {y} {w} {h}");
                }
            }
        }
        text
    }

    /// The layout stored in `file`; the default when the file is missing or unreadable.
    pub fn load(file: &Path) -> DockLayout {
        std::fs::read_to_string(file)
            .map(|text| DockLayout::parse(&text))
            .unwrap_or_default()
    }

    /// Store the layout in `file`, creating its folder when needed.
    pub fn save(&self, file: &Path) -> io::Result<()> {
        let folder = file.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(folder)?;
        let temp = folder.join(".panels.tmp");
        std::fs::write(&temp, self.to_text())?;
        std::fs::rename(&temp, file)
    }
}

/// Where in a column a panel dropped at height `pointer_y` goes, given the vertical
/// middles of the column's panels (top to bottom, the dragged one left out): the number
/// of middles above the pointer.
pub fn drop_position(middles: &[f32], pointer_y: f32) -> usize {
    middles.iter().filter(|&&middle| middle < pointer_y).count()
}

/// The panel's title.
pub fn title(id: PanelId) -> &'static str {
    match id {
        PanelId::EntityInfo => "Entity Info",
        PanelId::Materials => "Materials",
        PanelId::Components => "Components",
        PanelId::Shadows => "Shadows",
    }
}

/// The word that stands for the panel in the layout file.
pub fn panel_key(id: PanelId) -> &'static str {
    match id {
        PanelId::EntityInfo => "entity-info",
        PanelId::Materials => "materials",
        PanelId::Components => "components",
        PanelId::Shadows => "shadows",
    }
}

/// The file the layout is kept in: `panels` in `recent::config_dir`.
pub fn panels_file(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    crate::recent::config_dir(var).map(|dir| dir.join("panels"))
}

/// Height of a panel's title row, in points.
pub const HEADER_HEIGHT: f32 = 24.0;

/// The glyph drawn on a header button.
#[derive(Clone, Copy)]
enum HeaderIcon {
    Collapse,
    Expand,
    Close,
}

/// What a panel's title row reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PanelEvent {
    /// The collapse button was clicked.
    ToggleCollapsed(PanelId),
    /// The close button was clicked.
    Close(PanelId),
    /// A drag that started on the title ended with the pointer at `pos`.
    Dropped { id: PanelId, pos: Pos2 },
}

/// Draw the title row of panel `id`: the title (`title(id)`) as a drag handle, then, when
/// `docked`, a collapse button, then a close button. `collapsed` only changes the
/// collapse button's label.
pub fn panel_header(
    ui: &mut eframe::egui::Ui,
    id: PanelId,
    collapsed: bool,
    docked: bool,
) -> Option<PanelEvent> {
    use eframe::egui::{Align, Align2, FontId, Layout, Sense, Stroke, WidgetInfo, WidgetType};

    const BUTTON_WIDTH: f32 = 22.0;
    let name = title(id);
    let mut event = None;

    // A fixed-height row: `with_layout` alone would take all the remaining height and
    // centre the header in it.
    let row = eframe::egui::vec2(ui.available_width(), HEADER_HEIGHT);
    ui.allocate_ui_with_layout(row, Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let buttons = if docked { 2 } else { 1 };
        let reserved = buttons as f32 * (BUTTON_WIDTH + ui.spacing().item_spacing.x);
        let title_width = (ui.available_width() - reserved).max(BUTTON_WIDTH);
        let (title_rect, title_response) = ui.allocate_exact_size(
            eframe::egui::vec2(title_width, HEADER_HEIGHT),
            Sense::click_and_drag(),
        );
        let enabled = ui.is_enabled();
        let handle_label = format!("Move {name} panel");
        title_response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &handle_label));
        let text_color = ui.visuals().text_color();
        ui.painter().text(
            title_rect.left_center() + eframe::egui::vec2(4.0, 0.0),
            Align2::LEFT_CENTER,
            name,
            FontId::proportional(13.0),
            text_color,
        );
        if title_response.dragged() {
            ui.ctx().data_mut(|data| {
                data.insert_temp(eframe::egui::Id::new("dock-dragging"), id);
            });
        }
        if title_response.drag_stopped()
            && let Some(pos) = title_response.interact_pointer_pos()
        {
            event = Some(PanelEvent::Dropped { id, pos });
        }

        let button = |ui: &mut eframe::egui::Ui, icon: HeaderIcon, label: String| {
            let (rect, response) = ui.allocate_exact_size(
                eframe::egui::vec2(BUTTON_WIDTH, HEADER_HEIGHT),
                Sense::click(),
            );
            let enabled = ui.is_enabled();
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &label));
            let color = ui.visuals().text_color();
            if response.hovered() {
                ui.painter()
                    .rect_filled(rect, 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
            }
            let center = rect.center();
            let stroke = Stroke::new(1.5, color);
            let reach = 4.0;
            match icon {
                HeaderIcon::Collapse => {
                    ui.painter()
                        .line_segment([center + eframe::egui::vec2(-reach, -2.0), center], stroke);
                    ui.painter()
                        .line_segment([center, center + eframe::egui::vec2(reach, -2.0)], stroke);
                }
                HeaderIcon::Expand => {
                    ui.painter()
                        .line_segment([center + eframe::egui::vec2(-2.0, -reach), center], stroke);
                    ui.painter()
                        .line_segment([center, center + eframe::egui::vec2(-2.0, reach)], stroke);
                }
                HeaderIcon::Close => {
                    ui.painter().line_segment(
                        [
                            center + eframe::egui::vec2(-reach, -reach),
                            center + eframe::egui::vec2(reach, reach),
                        ],
                        stroke,
                    );
                    ui.painter().line_segment(
                        [
                            center + eframe::egui::vec2(-reach, reach),
                            center + eframe::egui::vec2(reach, -reach),
                        ],
                        stroke,
                    );
                }
            }
            if response.clicked() {
                ui.painter().rect_stroke(
                    rect,
                    3.0,
                    Stroke::new(1.0, color),
                    eframe::egui::StrokeKind::Inside,
                );
            }
            response
        };

        if docked {
            let (icon, label) = if collapsed {
                (HeaderIcon::Expand, format!("Expand {name}"))
            } else {
                (HeaderIcon::Collapse, format!("Collapse {name}"))
            };
            if button(ui, icon, label).clicked() {
                event = Some(PanelEvent::ToggleCollapsed(id));
            }
        }
        if button(ui, HeaderIcon::Close, format!("Close {name}")).clicked() {
            event = Some(PanelEvent::Close(id));
        }
    });

    event
}

/// Where a panel dropped with the pointer at `pointer` goes: into the column
/// `layout.drop_side(screen, pointer)` names, at `drop_position(middles, pointer.y)`, or
/// floating when that is `None`.
pub fn drop_destination(
    layout: &DockLayout,
    screen: Rect,
    pointer: Pos2,
    middles: &[f32],
) -> DropTarget {
    match layout.drop_side(screen, pointer) {
        Some(side) => DropTarget::Dock {
            side,
            position: drop_position(middles, pointer.y),
        },
        None => DropTarget::Float,
    }
}

/// Move `id` to `target`. Docking is `DockLayout::dock`. Floating puts the window's
/// top-left corner at `pointer`; it keeps the size it floated at before, and a panel that
/// was docked floats at `DEFAULT_WIDTH` x 300.
pub fn apply_drop(layout: &mut DockLayout, id: PanelId, target: DropTarget, pointer: Pos2) {
    match target {
        DropTarget::Dock { side, position } => layout.dock(id, side, position),
        DropTarget::Float => {
            let (w, h) = match layout.panel(id).place {
                Place::Floating { w, h, .. } => (w, h),
                Place::Docked { .. } => (DEFAULT_WIDTH, 300.0),
            };
            layout.float(id, Rect::from_min_size(pointer, eframe::egui::vec2(w, h)));
        }
    }
}

/// Smallest body (below the header) of an expanded docked panel, in points.
pub const MIN_BODY: f32 = 60.0;

/// One docked panel in a column, for sizing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StackRow {
    pub collapsed: bool,
    /// Share of the free height this panel gets; kept between frames, not saved to the file.
    pub weight: f32,
}

/// Height of every row (header included) in a column `available` points tall.
pub fn stack_heights(available: f32, rows: &[StackRow]) -> Vec<f32> {
    let minimum = HEADER_HEIGHT + MIN_BODY;
    let mut heights = vec![HEADER_HEIGHT; rows.len()];
    let collapsed = rows.iter().filter(|row| row.collapsed).count();
    let mut remaining = available - collapsed as f32 * HEADER_HEIGHT;

    let mut expanded: Vec<usize> = (0..rows.len())
        .filter(|&index| !rows[index].collapsed)
        .collect();
    if expanded.is_empty() {
        return heights;
    }

    loop {
        let weight = |index: usize| {
            let weight = rows[index].weight;
            if weight > 0.0 { weight } else { 1.0 }
        };
        let total: f32 = expanded.iter().map(|&index| weight(index)).sum();
        let pinned: Vec<usize> = expanded
            .iter()
            .copied()
            .filter(|&index| remaining * weight(index) / total < minimum)
            .collect();
        if pinned.is_empty() {
            for &index in &expanded {
                heights[index] = remaining * weight(index) / total;
            }
            break;
        }
        for &index in &pinned {
            heights[index] = minimum;
            remaining -= minimum;
        }
        expanded.retain(|index| !pinned.contains(index));
        if expanded.is_empty() {
            break;
        }
    }
    heights
}

/// The user dragged the border under row `index` by `delta` points (positive is down).
pub fn drag_splitter(rows: &mut [StackRow], heights: &[f32], index: usize, delta: f32) {
    let minimum = HEADER_HEIGHT + MIN_BODY;
    if index + 1 >= rows.len() || index + 1 >= heights.len() {
        return;
    }
    if rows[index].collapsed || rows[index + 1].collapsed {
        return;
    }

    let top = heights[index];
    let bottom = heights[index + 1];
    let lower = minimum - top;
    let upper = bottom - minimum;
    let delta = if lower <= upper {
        delta.clamp(lower, upper)
    } else {
        0.0
    };
    let mut moved = heights.to_vec();
    moved[index] = top + delta;
    moved[index + 1] = bottom - delta;
    for (row, &height) in rows.iter_mut().zip(&moved) {
        if !row.collapsed {
            row.weight = height;
        }
    }
}
