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
        let toolbars = crate::chrome::TOOL_GROUPS
            .iter()
            .enumerate()
            .map(|(slot, group)| Toolbar {
                name: GROUP_NAMES[slot].to_string(),
                tools: group.to_vec(),
                visible: true,
                placement: Placement::Docked {
                    edge: Edge::Top,
                    slot: slot as u32,
                },
            })
            .collect();
        ToolbarLayout { toolbars }
    }
}

impl ToolbarLayout {
    /// The toolbars, in list order.
    pub fn toolbars(&self) -> &[Toolbar] {
        &self.toolbars
    }

    fn edge_members(&self, edge: Edge) -> Vec<usize> {
        let mut members: Vec<(u32, usize)> = self
            .toolbars
            .iter()
            .enumerate()
            .filter_map(|(index, toolbar)| match toolbar.placement {
                Placement::Docked { edge: e, slot } if e == edge => Some((slot, index)),
                _ => None,
            })
            .collect();
        members.sort_by_key(|&(slot, _)| slot);
        members.into_iter().map(|(_, index)| index).collect()
    }

    fn renumber_edge(&mut self, edge: Edge) {
        for (slot, index) in self.edge_members(edge).into_iter().enumerate() {
            if let Placement::Docked { slot: s, .. } = &mut self.toolbars[index].placement {
                *s = slot as u32;
            }
        }
    }

    fn renumber_all(&mut self) {
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            self.renumber_edge(edge);
        }
    }

    /// Show or hide toolbar `index`; out of range does nothing.
    pub fn set_visible(&mut self, index: usize, visible: bool) {
        if let Some(toolbar) = self.toolbars.get_mut(index) {
            toolbar.visible = visible;
        }
    }

    /// Indices of the shown toolbars docked along `edge`, in slot order.
    pub fn docked(&self, edge: Edge) -> Vec<usize> {
        self.edge_members(edge)
            .into_iter()
            .filter(|&index| self.toolbars[index].visible)
            .collect()
    }

    /// Indices of the shown floating toolbars, in list order.
    pub fn floating(&self) -> Vec<usize> {
        self.toolbars
            .iter()
            .enumerate()
            .filter_map(|(index, toolbar)| {
                (toolbar.visible && matches!(toolbar.placement, Placement::Floating { .. }))
                    .then_some(index)
            })
            .collect()
    }

    /// Dock toolbar `index` along `edge` so it is at `position` in `docked(edge)` (the end
    /// when `position` is past it), and show it.
    pub fn dock(&mut self, index: usize, edge: Edge, position: usize) {
        if index >= self.toolbars.len() {
            return;
        }
        let old_edge = match self.toolbars[index].placement {
            Placement::Docked { edge: e, .. } => Some(e),
            Placement::Floating { .. } => None,
        };
        let mut members = self.edge_members(edge);
        members.retain(|&member| member != index);
        let mut shown = 0;
        let mut insert_at = members.len();
        for (at, &member) in members.iter().enumerate() {
            if self.toolbars[member].visible {
                if shown == position {
                    insert_at = at;
                    break;
                }
                shown += 1;
            }
        }
        members.insert(insert_at, index);
        self.toolbars[index].visible = true;
        for (slot, &member) in members.iter().enumerate() {
            self.toolbars[member].placement = Placement::Docked {
                edge,
                slot: slot as u32,
            };
        }
        if let Some(old_edge) = old_edge
            && old_edge != edge
        {
            self.renumber_edge(old_edge);
        }
    }

    /// Make toolbar `index` float with its top-left corner at (`x`, `y`), and show it.
    pub fn float(&mut self, index: usize, x: f32, y: f32) {
        if index >= self.toolbars.len() {
            return;
        }
        let old_edge = match self.toolbars[index].placement {
            Placement::Docked { edge, .. } => Some(edge),
            Placement::Floating { .. } => None,
        };
        self.toolbars[index].visible = true;
        self.toolbars[index].placement = Placement::Floating { x, y };
        if let Some(edge) = old_edge {
            self.renumber_edge(edge);
        }
    }

    /// Put `tool` at `at` in toolbar `index` (the end when `at` is past it); a tool already
    /// on that toolbar moves there instead of appearing twice.
    pub fn add_tool(&mut self, index: usize, tool: Tool, at: usize) {
        let Some(toolbar) = self.toolbars.get_mut(index) else {
            return;
        };
        toolbar.tools.retain(|&existing| existing != tool);
        let at = at.min(toolbar.tools.len());
        toolbar.tools.insert(at, tool);
    }

    /// Take the button at `at` off toolbar `index`.
    pub fn remove_tool(&mut self, index: usize, at: usize) {
        if let Some(toolbar) = self.toolbars.get_mut(index)
            && at < toolbar.tools.len()
        {
            toolbar.tools.remove(at);
        }
    }

    /// Move toolbar `index`'s button at `from` to `to` (the end when `to` is past it).
    pub fn move_tool(&mut self, index: usize, from: usize, to: usize) {
        let Some(toolbar) = self.toolbars.get_mut(index) else {
            return;
        };
        if from >= toolbar.tools.len() {
            return;
        }
        let tool = toolbar.tools.remove(from);
        let to = to.min(toolbar.tools.len());
        toolbar.tools.insert(to, tool);
    }

    /// Append a new empty, shown toolbar floating at `NEW_TOOLBAR_POS`; returns its index.
    pub fn add_toolbar(&mut self, name: &str) -> usize {
        let cleaned: String = name
            .chars()
            .map(|c| if c == '\r' || c == '\n' { ' ' } else { c })
            .collect();
        let base = match cleaned.trim() {
            "" => "Toolbar".to_string(),
            trimmed => trimmed.to_string(),
        };
        let mut candidate = base.clone();
        let mut suffix = 2;
        while self.toolbars.iter().any(|t| t.name == candidate) {
            candidate = format!("{base} {suffix}");
            suffix += 1;
        }
        let index = self.toolbars.len();
        self.toolbars.push(Toolbar {
            name: candidate,
            tools: Vec::new(),
            visible: true,
            placement: Placement::Floating {
                x: NEW_TOOLBAR_POS.0,
                y: NEW_TOOLBAR_POS.1,
            },
        });
        index
    }

    /// Delete toolbar `index`.
    pub fn remove_toolbar(&mut self, index: usize) {
        if index >= self.toolbars.len() {
            return;
        }
        let old_edge = match self.toolbars[index].placement {
            Placement::Docked { edge, .. } => Some(edge),
            Placement::Floating { .. } => None,
        };
        self.toolbars.remove(index);
        if let Some(edge) = old_edge {
            self.renumber_edge(edge);
        }
    }

    /// Back to `ToolbarLayout::default()`.
    pub fn reset(&mut self) {
        *self = ToolbarLayout::default();
    }

    /// The layout from the file's text; the default layout when the text isn't a layout.
    pub fn parse(text: &str) -> ToolbarLayout {
        let mut lines = text
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line));
        match lines.next() {
            Some(line) if line == HEADER => {}
            _ => return ToolbarLayout::default(),
        }
        let mut layout = ToolbarLayout {
            toolbars: Vec::new(),
        };
        let mut pending = Pending::None;
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("toolbar") {
                if !field_break(rest) {
                    pending = Pending::None;
                    continue;
                }
                match parse_toolbar(rest) {
                    Some(toolbar) => {
                        layout.toolbars.push(toolbar);
                        pending = Pending::Kept;
                    }
                    None => pending = Pending::Bad,
                }
            } else if let Some(rest) = line.strip_prefix("tools") {
                if !field_break(rest) {
                    pending = Pending::None;
                    continue;
                }
                if pending == Pending::Kept
                    && let Some(toolbar) = layout.toolbars.last_mut()
                {
                    for key in rest.split_whitespace() {
                        if let Some(tool) = tool_from_key(key)
                            && !toolbar.tools.contains(&tool)
                        {
                            toolbar.tools.push(tool);
                        }
                    }
                }
                pending = Pending::None;
            } else {
                pending = Pending::None;
            }
        }
        layout.renumber_all();
        layout
    }

    /// The file's text for this layout.
    pub fn to_text(&self) -> String {
        let mut text = format!("{HEADER}\n");
        for toolbar in &self.toolbars {
            let visible = u8::from(toolbar.visible);
            match toolbar.placement {
                Placement::Docked { edge, slot } => {
                    text.push_str(&format!(
                        "toolbar {visible} {} {slot} {}\n",
                        edge_key(edge),
                        toolbar.name
                    ));
                }
                Placement::Floating { x, y } => {
                    text.push_str(&format!(
                        "toolbar {visible} float {x} {y} {}\n",
                        toolbar.name
                    ));
                }
            }
            text.push_str("tools");
            for &tool in &toolbar.tools {
                text.push(' ');
                text.push_str(tool_key(tool));
            }
            text.push('\n');
        }
        text
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

#[derive(Clone, Copy, PartialEq)]
enum Pending {
    None,
    Kept,
    Bad,
}

fn field_break(rest: &str) -> bool {
    rest.is_empty() || rest.starts_with(|c: char| c.is_whitespace())
}

fn next_token(text: &str) -> Option<(&str, &str)> {
    let text = text.trim_start();
    if text.is_empty() {
        return None;
    }
    let end = text.find(|c: char| c.is_whitespace()).unwrap_or(text.len());
    Some((&text[..end], &text[end..]))
}

fn parse_toolbar(text: &str) -> Option<Toolbar> {
    let (visible, rest) = next_token(text)?;
    let visible = match visible {
        "1" => true,
        "0" => false,
        _ => return None,
    };
    let (kind, rest) = next_token(rest)?;
    let (placement, name) = if kind == "float" {
        let (x, rest) = next_token(rest)?;
        let (y, rest) = next_token(rest)?;
        let x: f32 = x.parse().ok()?;
        let y: f32 = y.parse().ok()?;
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        (Placement::Floating { x, y }, rest.trim())
    } else {
        let edge = edge_from_key(kind)?;
        let (slot, rest) = next_token(rest)?;
        let slot: u32 = slot.parse().ok()?;
        (Placement::Docked { edge, slot }, rest.trim())
    };
    if name.is_empty() {
        return None;
    }
    Some(Toolbar {
        name: name.to_string(),
        tools: Vec::new(),
        visible,
        placement,
    })
}

fn edge_key(edge: Edge) -> &'static str {
    match edge {
        Edge::Top => "top",
        Edge::Bottom => "bottom",
        Edge::Left => "left",
        Edge::Right => "right",
    }
}

fn edge_from_key(key: &str) -> Option<Edge> {
    match key {
        "top" => Some(Edge::Top),
        "bottom" => Some(Edge::Bottom),
        "left" => Some(Edge::Left),
        "right" => Some(Edge::Right),
        _ => None,
    }
}

/// The word that stands for `tool` in the layout file.
pub fn tool_key(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "select",
        Tool::Line => "line",
        Tool::Rectangle => "rectangle",
        Tool::Circle => "circle",
        Tool::Arc => "arc",
        Tool::Arc3 => "arc3",
        Tool::PushPull => "pushpull",
        Tool::Move => "move",
        Tool::Rotate => "rotate",
        Tool::Scale => "scale",
        Tool::Tape => "tape",
        Tool::Erase => "erase",
        Tool::Paint => "paint",
        Tool::Orbit => "orbit",
        Tool::Pan => "pan",
        Tool::Zoom => "zoom",
    }
}

/// The tool a layout-file word stands for.
pub fn tool_from_key(key: &str) -> Option<Tool> {
    Some(match key {
        "select" => Tool::Select,
        "line" => Tool::Line,
        "rectangle" => Tool::Rectangle,
        "circle" => Tool::Circle,
        "arc" => Tool::Arc,
        "arc3" => Tool::Arc3,
        "pushpull" => Tool::PushPull,
        "move" => Tool::Move,
        "rotate" => Tool::Rotate,
        "scale" => Tool::Scale,
        "tape" => Tool::Tape,
        "erase" => Tool::Erase,
        "paint" => Tool::Paint,
        "orbit" => Tool::Orbit,
        "pan" => Tool::Pan,
        "zoom" => Tool::Zoom,
        _ => return None,
    })
}

/// The file the layout is kept in: `toolbars` in `recent::config_dir`.
pub fn toolbars_file(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    crate::recent::config_dir(var).map(|dir| dir.join("toolbars"))
}
