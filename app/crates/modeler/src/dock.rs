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
        unimplemented!()
    }
}

impl DockLayout {
    /// The panel's state.
    pub fn panel(&self, id: PanelId) -> &Panel {
        let _ = id;
        unimplemented!()
    }

    pub fn is_open(&self, id: PanelId) -> bool {
        self.panel(id).open
    }

    /// Open or close a panel; where it sits is kept.
    pub fn set_open(&mut self, id: PanelId, open: bool) {
        let _ = (id, open);
        unimplemented!()
    }

    pub fn toggle(&mut self, id: PanelId) {
        let open = self.is_open(id);
        self.set_open(id, !open);
    }

    pub fn set_collapsed(&mut self, id: PanelId, collapsed: bool) {
        let _ = (id, collapsed);
        unimplemented!()
    }

    /// The open panels docked in `side`'s column, top to bottom.
    pub fn docked(&self, side: Side) -> Vec<PanelId> {
        let _ = side;
        unimplemented!()
    }

    /// The open floating panels, in `PANELS` order.
    pub fn floating(&self) -> Vec<PanelId> {
        unimplemented!()
    }

    /// Dock `id` in `side`'s column at `position` in `docked(side)` (the end when past it),
    /// and open it.
    pub fn dock(&mut self, id: PanelId, side: Side, position: usize) {
        let _ = (id, side, position);
        unimplemented!()
    }

    /// Float `id` as a window covering `rect` (size raised to `MIN_FLOAT_SIZE`), and open
    /// it.
    pub fn float(&mut self, id: PanelId, rect: Rect) {
        let _ = (id, rect);
        unimplemented!()
    }

    /// The `side` column's width.
    pub fn width(&self, side: Side) -> f32 {
        let _ = side;
        unimplemented!()
    }

    /// Set the `side` column's width, clamped to `MIN_WIDTH..=MAX_WIDTH`; not a number is
    /// ignored.
    pub fn set_width(&mut self, side: Side, width: f32) {
        let _ = (side, width);
        unimplemented!()
    }

    /// The column a panel dragged to `pointer` would dock in, in a window covering
    /// `screen`; `None` means it would float.
    pub fn drop_side(&self, screen: Rect, pointer: Pos2) -> Option<Side> {
        let _ = (screen, pointer);
        unimplemented!()
    }

    /// Back to `DockLayout::default()`.
    pub fn reset(&mut self) {
        *self = DockLayout::default();
    }

    /// The layout from the file's text; the default layout when the text isn't a layout.
    pub fn parse(text: &str) -> DockLayout {
        let _ = text;
        unimplemented!()
    }

    /// The file's text for this layout.
    pub fn to_text(&self) -> String {
        unimplemented!()
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
    let _ = (middles, pointer_y);
    unimplemented!()
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
