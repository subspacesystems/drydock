// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Toolbar layout: which toolbars exist, which tools each holds and in what order, which
//! are shown, and where each sits (docked along a window edge or floating). Kept in a
//! text file in the config folder between runs. Drawing the toolbars is elsewhere.
//!
//! Behaviour: `docs/specs/m6-toolbar-layout.md`.

use crate::input::Tool;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// The default toolbars' names, one per `chrome::TOOL_GROUPS` entry, same order.
pub const GROUP_NAMES: [&str; 5] = [
    "Select",
    "Drawing",
    "Modification",
    "Construction",
    "Camera",
];

/// First line of the layout file.
pub const HEADER: &str = "modeler toolbars 1";

/// Where a floating toolbar goes when `add_toolbar` makes one.
pub const NEW_TOOLBAR_POS: (f32, f32) = (120.0, 120.0);

/// A window edge a toolbar can dock along.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// Where a toolbar sits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    /// Along `edge`; toolbars on one edge are ordered by `slot` (0 first: leftmost on the
    /// top and bottom edges, topmost on the side edges).
    Docked { edge: Edge, slot: u32 },
    /// A free-standing toolbar whose top-left corner is at (`x`, `y`) points.
    Floating { x: f32, y: f32 },
}

/// One toolbar.
#[derive(Clone, Debug, PartialEq)]
pub struct Toolbar {
    pub name: String,
    /// Buttons in order; each tool at most once.
    pub tools: Vec<Tool>,
    pub visible: bool,
    pub placement: Placement,
}

/// All toolbars. Indices into `toolbars()` stay put: no method reorders the list except
/// `add_toolbar` (appends) and `remove_toolbar` (shifts the ones after it down by one).
#[derive(Clone, Debug, PartialEq)]
pub struct ToolbarLayout {
    toolbars: Vec<Toolbar>,
}

impl Default for ToolbarLayout {
    /// One toolbar per tool group (`GROUP_NAMES`, `chrome::TOOL_GROUPS`), all shown,
    /// docked along the top edge in group order.
    fn default() -> ToolbarLayout {
        unimplemented!()
    }
}

impl ToolbarLayout {
    /// The toolbars, in list order.
    pub fn toolbars(&self) -> &[Toolbar] {
        &self.toolbars
    }

    /// Show or hide toolbar `index`; out of range does nothing.
    pub fn set_visible(&mut self, index: usize, visible: bool) {
        let _ = (index, visible);
        unimplemented!()
    }

    /// Indices of the shown toolbars docked along `edge`, in slot order.
    pub fn docked(&self, edge: Edge) -> Vec<usize> {
        let _ = edge;
        unimplemented!()
    }

    /// Indices of the shown floating toolbars, in list order.
    pub fn floating(&self) -> Vec<usize> {
        unimplemented!()
    }

    /// Dock toolbar `index` along `edge` so it is at `position` in `docked(edge)` (the end
    /// when `position` is past it), and show it.
    pub fn dock(&mut self, index: usize, edge: Edge, position: usize) {
        let _ = (index, edge, position);
        unimplemented!()
    }

    /// Make toolbar `index` float with its top-left corner at (`x`, `y`), and show it.
    pub fn float(&mut self, index: usize, x: f32, y: f32) {
        let _ = (index, x, y);
        unimplemented!()
    }

    /// Put `tool` at `at` in toolbar `index` (the end when `at` is past it); a tool already
    /// on that toolbar moves there instead of appearing twice.
    pub fn add_tool(&mut self, index: usize, tool: Tool, at: usize) {
        let _ = (index, tool, at);
        unimplemented!()
    }

    /// Take the button at `at` off toolbar `index`.
    pub fn remove_tool(&mut self, index: usize, at: usize) {
        let _ = (index, at);
        unimplemented!()
    }

    /// Move toolbar `index`'s button at `from` to `to` (the end when `to` is past it).
    pub fn move_tool(&mut self, index: usize, from: usize, to: usize) {
        let _ = (index, from, to);
        unimplemented!()
    }

    /// Append a new empty, shown toolbar floating at `NEW_TOOLBAR_POS`; returns its index.
    pub fn add_toolbar(&mut self, name: &str) -> usize {
        let _ = name;
        unimplemented!()
    }

    /// Delete toolbar `index`.
    pub fn remove_toolbar(&mut self, index: usize) {
        let _ = index;
        unimplemented!()
    }

    /// Back to `ToolbarLayout::default()`.
    pub fn reset(&mut self) {
        *self = ToolbarLayout::default();
    }

    /// The layout from the file's text; the default layout when the text isn't a layout.
    pub fn parse(text: &str) -> ToolbarLayout {
        let _ = text;
        unimplemented!()
    }

    /// The file's text for this layout.
    pub fn to_text(&self) -> String {
        unimplemented!()
    }

    /// The layout stored in `file`; the default when the file is missing or unreadable.
    pub fn load(file: &Path) -> ToolbarLayout {
        std::fs::read_to_string(file)
            .map(|text| ToolbarLayout::parse(&text))
            .unwrap_or_default()
    }

    /// Store the layout in `file`, creating its folder when needed.
    pub fn save(&self, file: &Path) -> io::Result<()> {
        let folder = file.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(folder)?;
        let temp = folder.join(".toolbars.tmp");
        std::fs::write(&temp, self.to_text())?;
        std::fs::rename(&temp, file)
    }
}

/// The word that stands for `tool` in the layout file.
pub fn tool_key(tool: Tool) -> &'static str {
    let _ = tool;
    unimplemented!()
}

/// The tool a layout-file word stands for.
pub fn tool_from_key(key: &str) -> Option<Tool> {
    let _ = key;
    unimplemented!()
}

/// The file the layout is kept in: `toolbars` in `recent::config_dir`.
pub fn toolbars_file(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    crate::recent::config_dir(var).map(|dir| dir.join("toolbars"))
}
