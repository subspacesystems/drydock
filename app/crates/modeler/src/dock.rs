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
