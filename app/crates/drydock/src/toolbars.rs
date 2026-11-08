// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Toolbar layout: which toolbars exist, which tools each holds and in what order, which
//! are shown, and where each sits (docked along a window edge or floating). Kept in a
//! text file in the config folder between runs. Drawing the toolbars is elsewhere.
//!
//! Behaviour: `docs/specs/m6-toolbar-layout.md`.

use crate::chrome::{TOOL_BUTTONS, menu_check_item, menu_item, tool_shortcut};
use crate::dialog::{DialogWindow, confirm_dialog, dialog_window, name_dialog};
use crate::icons::{self, Icon};
use crate::input::Tool;
use eframe::egui;
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
#[derive(Clone, Debug)]
pub struct ToolbarLayout {
    toolbars: Vec<Toolbar>,
}

impl PartialEq for ToolbarLayout {
    /// Equal up to the old-layout migration: a layout written before Follow Me existed is
    /// loaded with Follow Me after Offset, so it compares equal to that loaded form.
    fn eq(&self, other: &Self) -> bool {
        let mut left = self.toolbars.clone();
        let mut right = other.toolbars.clone();
        if let Some((index, at)) = follow_me_spot(&left) {
            left[index].tools.insert(at, Tool::FollowMe);
        }
        if let Some((index, at)) = follow_me_spot(&right) {
            right[index].tools.insert(at, Tool::FollowMe);
        }
        left == right
    }
}

/// Where to add Follow Me in the first toolbar holding Offset, when no toolbar holds Follow
/// Me (`docs/specs/m9-followme-button.md`, old toolbar layouts).
fn follow_me_spot(toolbars: &[Toolbar]) -> Option<(usize, usize)> {
    if toolbars
        .iter()
        .any(|toolbar| toolbar.tools.contains(&Tool::FollowMe))
    {
        return None;
    }
    toolbars.iter().enumerate().find_map(|(index, toolbar)| {
        toolbar
            .tools
            .iter()
            .position(|tool| *tool == Tool::Offset)
            .map(|at| (index, at + 1))
    })
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

    /// Rename toolbar `index`; true when its name changed.
    /// Behaviour: `docs/specs/m6-toolbar-customize-3.md`.
    pub fn rename_toolbar(&mut self, index: usize, name: &str) -> bool {
        let Some(current) = self.toolbars.get(index).map(|toolbar| toolbar.name.clone()) else {
            return false;
        };
        let cleaned: String = name
            .chars()
            .map(|c| if c == '\r' || c == '\n' { ' ' } else { c })
            .collect();
        let base = cleaned.trim();
        if base.is_empty() {
            return false;
        }
        let mut candidate = base.to_string();
        let mut suffix = 2;
        while self
            .toolbars
            .iter()
            .enumerate()
            .any(|(i, toolbar)| i != index && toolbar.name == candidate)
        {
            candidate = format!("{base} {suffix}");
            suffix += 1;
        }
        if candidate == current {
            return false;
        }
        self.toolbars[index].name = candidate;
        true
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
        if let Some((index, at)) = follow_me_spot(&layout.toolbars) {
            layout.toolbars[index].tools.insert(at, Tool::FollowMe);
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
        Tool::Offset => "offset",
        Tool::FollowMe => "followme",
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
        "offset" => Tool::Offset,
        "followme" => Tool::FollowMe,
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

/// Distance from a window edge, in points, within which dropping a toolbar docks it there.
pub const EDGE_ZONE: f32 = 48.0;

/// Length of a toolbar's grip along the toolbar, in points (its other side is
/// `icons::BUTTON_SIZE`).
pub const GRIP_LENGTH: f32 = 12.0;

/// Space left and right of the buttons in a toolbar column docked on the left or right edge,
/// in points.
pub const SIDE_MARGIN: f32 = 3.0;

/// The panel holding the toolbars docked on `edge`. Top and bottom as before; left and right
/// exactly as wide as a toolbar column (`icons::BUTTON_SIZE` plus `SIDE_MARGIN` each side),
/// not resizable (`docs/specs/m6-toolbar-side-width.md`).
pub fn edge_panel(edge: Edge, style: &egui::Style) -> egui::Panel {
    match edge {
        Edge::Top => egui::Panel::top("toolbars-top"),
        Edge::Bottom => egui::Panel::bottom("toolbars-bottom"),
        Edge::Left => egui::Panel::left("toolbars-left")
            .resizable(false)
            .exact_size(icons::BUTTON_SIZE + 2.0 * SIDE_MARGIN)
            .frame(
                egui::Frame::side_top_panel(style)
                    .inner_margin(egui::Margin::symmetric(SIDE_MARGIN as i8, 2)),
            ),
        Edge::Right => egui::Panel::right("toolbars-right")
            .resizable(false)
            .exact_size(icons::BUTTON_SIZE + 2.0 * SIDE_MARGIN)
            .frame(
                egui::Frame::side_top_panel(style)
                    .inner_margin(egui::Margin::symmetric(SIDE_MARGIN as i8, 2)),
            ),
    }
}

/// What a drawn toolbar reports for the frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolbarEvent {
    /// A tool button was clicked.
    Select(Tool),
    /// The grip of toolbar `index` was let go with the pointer at `pos`.
    Dropped { index: usize, pos: egui::Pos2 },
}

/// The edge of `window` a toolbar dropped at `pos` docks to: the nearest edge `pos` is
/// within `EDGE_ZONE` of (the first of top, bottom, left, right on a tie), or `None`
/// (the toolbar floats).
pub fn drop_target(pos: egui::Pos2, window: egui::Rect) -> Option<Edge> {
    let distances = [
        (Edge::Top, (pos.y - window.top()).max(0.0)),
        (Edge::Bottom, (window.bottom() - pos.y).max(0.0)),
        (Edge::Left, (pos.x - window.left()).max(0.0)),
        (Edge::Right, (window.right() - pos.x).max(0.0)),
    ];
    let mut best: Option<(Edge, f32)> = None;
    for (edge, distance) in distances {
        if distance <= EDGE_ZONE && best.is_none_or(|(_, nearest)| distance < nearest) {
            best = Some((edge, distance));
        }
    }
    best.map(|(edge, _)| edge)
}

/// Where a toolbar dropped at `pos` goes among the toolbars docked along `edge`.
/// `rects` are those toolbars' screen rectangles in docked order. The answer is the
/// number of rectangles whose centre is before `pos` along the edge (x for top and
/// bottom, y for left and right), so `0..=rects.len()`.
pub fn insertion_index(edge: Edge, rects: &[egui::Rect], pos: egui::Pos2) -> usize {
    let horizontal = matches!(edge, Edge::Top | Edge::Bottom);
    rects
        .iter()
        .filter(|rect| {
            let centre = if horizontal {
                rect.center().x
            } else {
                rect.center().y
            };
            let coordinate = if horizontal { pos.x } else { pos.y };
            centre < coordinate
        })
        .count()
}

/// The rectangle of size `size` an outline is drawn at to show a toolbar landing at
/// `index` among the `rects` docked along `edge` inside `window`.
/// Along the edge it starts where `rects[index]` starts, or where the last rectangle
/// ends when `index == rects.len()`. Across the edge it starts where `rects[0]` does.
/// With no rectangles it starts at the window's start of that edge (left for top and
/// bottom, top for left and right) and sits against the edge itself.
pub fn drop_outline(
    edge: Edge,
    rects: &[egui::Rect],
    index: usize,
    window: egui::Rect,
    size: egui::Vec2,
) -> egui::Rect {
    let along = match rects.get(index) {
        Some(rect) => match edge {
            Edge::Top | Edge::Bottom => rect.left(),
            Edge::Left | Edge::Right => rect.top(),
        },
        None => match rects.last() {
            Some(rect) => match edge {
                Edge::Top | Edge::Bottom => rect.right(),
                Edge::Left | Edge::Right => rect.bottom(),
            },
            None => match edge {
                Edge::Top | Edge::Bottom => window.left(),
                Edge::Left | Edge::Right => window.top(),
            },
        },
    };
    let across = match rects.first() {
        Some(rect) => match edge {
            Edge::Top | Edge::Bottom => rect.top(),
            Edge::Left | Edge::Right => rect.left(),
        },
        None => match edge {
            Edge::Top => window.top(),
            Edge::Bottom => window.bottom() - size.y,
            Edge::Left => window.left(),
            Edge::Right => window.right() - size.x,
        },
    };
    let min = match edge {
        Edge::Top | Edge::Bottom => egui::pos2(along, across),
        Edge::Left | Edge::Right => egui::pos2(across, along),
    };
    egui::Rect::from_min_size(min, size)
}

/// The tool button's tooltip: its name and shortcut, like `Line (L)`.
pub fn tool_tooltip(tool: Tool) -> String {
    let label = TOOL_BUTTONS
        .iter()
        .find(|(button, _)| *button == tool)
        .map(|(_, label)| *label)
        .unwrap_or("");
    format!("{label} ({})", tool_shortcut(tool))
}

/// What drawing one toolbar strip reported.
#[derive(Clone, Copy, Debug)]
pub struct ToolbarStripResponse {
    /// The tool button clicked or the grip dropped this frame.
    pub event: Option<ToolbarEvent>,
    /// The whole strip's screen rectangle.
    pub rect: egui::Rect,
    /// The grip is being dragged this frame.
    pub dragging: bool,
}

/// Draw `toolbar` (number `index` in its layout) as a row, or a column when `vertical`:
/// a grip, then one icon button per tool. `current` is drawn selected.
pub fn toolbar_strip(
    ui: &mut egui::Ui,
    toolbar: &Toolbar,
    index: usize,
    current: Tool,
    vertical: bool,
) -> Option<ToolbarEvent> {
    toolbar_strip_response(ui, toolbar, index, current, vertical).event
}

/// Draw one toolbar strip and report its rectangle and whether its grip is being dragged.
pub fn toolbar_strip_response(
    ui: &mut egui::Ui,
    toolbar: &Toolbar,
    index: usize,
    current: Tool,
    vertical: bool,
) -> ToolbarStripResponse {
    let mut event = None;
    let mut dragging = false;
    let layout = if vertical {
        egui::Layout::top_down(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    let response = ui.with_layout(layout, |ui| {
        let grip_size = if vertical {
            egui::vec2(icons::BUTTON_SIZE, GRIP_LENGTH)
        } else {
            egui::vec2(GRIP_LENGTH, icons::BUTTON_SIZE)
        };
        let (grip_rect, grip) = ui.allocate_exact_size(grip_size, egui::Sense::click_and_drag());
        let grip_label = format!("Move {} toolbar", toolbar.name);
        let enabled = ui.is_enabled();
        grip.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &grip_label)
        });
        paint_grip(ui, grip_rect, vertical);

        for &tool in &toolbar.tools {
            let label = TOOL_BUTTONS
                .iter()
                .find(|(button, _)| *button == tool)
                .map(|(_, label)| *label)
                .unwrap_or("");
            let button = icons::icon_button(
                ui,
                Icon::Tool(tool),
                label,
                &tool_tooltip(tool),
                tool == current,
            );
            if button.clicked() {
                event = Some(ToolbarEvent::Select(tool));
            }
        }

        dragging = grip.dragged();
        if grip.drag_stopped()
            && let Some(pos) = grip.interact_pointer_pos()
        {
            event = Some(ToolbarEvent::Dropped { index, pos });
        }
    });
    ToolbarStripResponse {
        event,
        rect: response.response.rect,
        dragging,
    }
}

/// A few dots in the theme's text color, arranged along the grip.
fn paint_grip(ui: &egui::Ui, rect: egui::Rect, vertical: bool) {
    let color = ui.visuals().text_color().gamma_multiply(0.7);
    let (columns, rows) = if vertical { (3, 2) } else { (2, 3) };
    let painter = ui.painter();
    for column in 0..columns {
        for row in 0..rows {
            let x = rect.left() + rect.width() * (column as f32 + 1.0) / (columns as f32 + 1.0);
            let y = rect.top() + rect.height() * (row as f32 + 1.0) / (rows as f32 + 1.0);
            painter.circle_filled(egui::pos2(x, y), 1.2, color);
        }
    }
}

/// What the View > Toolbars menu asks of the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolbarsMenuEvent {
    /// The layout changed (a toolbar was shown or hidden, or the layout was reset).
    Changed,
    /// Open the Customize window.
    Customize,
    /// "Reset Toolbars..." was chosen: the app asks before resetting
    /// (`docs/specs/m6-confirm.md`).
    Reset,
}

/// What the Customize window remembers between frames.
#[derive(Clone, Debug, Default)]
pub struct CustomizeState {
    /// The toolbar being edited (an index into the layout; clamped when out of range).
    pub selected: usize,
    /// The dialog open over the window, if any (`docs/specs/m6-confirm.md`).
    pub prompt: Option<CustomizePrompt>,
}

/// A dialog the Customize window shows over itself (`docs/specs/m6-confirm.md`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CustomizePrompt {
    /// "New Toolbar...": ask for the new toolbar's name.
    New { name: String },
    /// "Rename Toolbar...": ask for toolbar `index`'s new name.
    Rename { index: usize, name: String },
    /// "Delete Toolbar...": confirm deleting toolbar `index`.
    Delete { index: usize },
    /// "Reset Toolbars...": confirm the reset.
    Reset,
}

/// The contents of the View > Toolbars submenu.
/// Behaviour: `docs/specs/m6-toolbar-customize.md`.
pub fn toolbars_menu(ui: &mut egui::Ui, layout: &mut ToolbarLayout) -> Option<ToolbarsMenuEvent> {
    let mut event = None;
    let count = layout.toolbars().len();
    for index in 0..count {
        let name = layout.toolbars()[index].name.clone();
        let visible = layout.toolbars()[index].visible;
        if menu_check_item(ui, &name, visible).clicked() {
            layout.set_visible(index, !visible);
            event = Some(ToolbarsMenuEvent::Changed);
        }
    }
    ui.separator();
    if menu_item(ui, None, "Customize...", "", false).clicked() {
        event = Some(ToolbarsMenuEvent::Customize);
    }
    if menu_item(ui, None, "Reset Toolbars...", "", false).clicked() {
        event = Some(ToolbarsMenuEvent::Reset);
    }
    event
}

/// Where a tool dragged in the Customize window was dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomizeDrop {
    /// Into the "On the toolbar" list, before the entry at this index (the end when past it).
    OnToolbar(usize),
    /// Into the "Available" list.
    Available,
}

/// The tools of `TOOL_BUTTONS` (in that order) that toolbar `toolbar` does not hold.
/// Behaviour: `docs/specs/m6-toolbar-customize-2.md`.
pub fn available_tools(layout: &ToolbarLayout, toolbar: usize) -> Vec<Tool> {
    let held = layout
        .toolbars()
        .get(toolbar)
        .map(|toolbar| toolbar.tools.as_slice())
        .unwrap_or(&[]);
    TOOL_BUTTONS
        .iter()
        .map(|(tool, _)| *tool)
        .filter(|tool| !held.contains(tool))
        .collect()
}

/// Apply a drop of `tool` onto toolbar `toolbar`'s lists; true when the layout changed.
/// Behaviour: `docs/specs/m6-toolbar-customize-2.md`.
pub fn apply_customize_drop(
    layout: &mut ToolbarLayout,
    toolbar: usize,
    tool: Tool,
    drop: CustomizeDrop,
) -> bool {
    let Some(entry) = layout.toolbars().get(toolbar) else {
        return false;
    };
    let at = entry.tools.iter().position(|&existing| existing == tool);
    match drop {
        CustomizeDrop::OnToolbar(index) => match at {
            Some(at) => {
                let target = if index <= at { index } else { index - 1 };
                if target == at {
                    return false;
                }
                layout.move_tool(toolbar, at, target);
                true
            }
            None => {
                layout.add_tool(toolbar, tool, index);
                true
            }
        },
        CustomizeDrop::Available => match at {
            Some(at) => {
                layout.remove_tool(toolbar, at);
                true
            }
            None => false,
        },
    }
}

fn tool_label(tool: Tool) -> &'static str {
    TOOL_BUTTONS
        .iter()
        .find(|(button, _)| *button == tool)
        .map(|(_, label)| *label)
        .unwrap_or("")
}

fn customize_drag_handle(ui: &mut egui::Ui, tool: Tool, label: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::Vec2::splat(icons::BUTTON_SIZE),
        egui::Sense::click_and_drag(),
    );
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, format!("Drag {label}"))
    });
    icons::paint(
        ui.painter(),
        rect.shrink(4.0),
        Icon::Tool(tool),
        ui.visuals().text_color(),
    );
    response.on_hover_cursor(egui::CursorIcon::Grab)
}

/// The Customize Toolbars window (`docs/specs/m11-dialog-print-toolbars.md`):
/// [`customize_panel`] in a window titled "Customize Toolbars" while `*open`; its close
/// button clears `open`. Returns true when the layout changed.
pub fn customize_window(
    ctx: &egui::Context,
    open: &mut bool,
    layout: &mut ToolbarLayout,
    state: &mut CustomizeState,
) -> bool {
    let mut changed = false;
    let prompt_open = state.prompt.is_some();
    let mut dialog_open = *open;
    dialog_window(
        ctx,
        &DialogWindow {
            title: "Customize Toolbars",
            size: egui::vec2(520.0, 480.0),
        },
        &mut dialog_open,
        |ui| {
            changed = customize_panel(ui, layout, state);
        },
    );
    if !prompt_open || dialog_open || !*open {
        *open = dialog_open;
    }
    changed
}

/// The contents of the Customize window. Returns true when the layout changed.
/// Behaviour: `docs/specs/m6-toolbar-customize-2.md`.
pub fn customize_panel(
    ui: &mut egui::Ui,
    layout: &mut ToolbarLayout,
    state: &mut CustomizeState,
) -> bool {
    let count = layout.toolbars().len();
    state.selected = if count == 0 {
        0
    } else {
        state.selected.min(count - 1)
    };
    let selected = state.selected;
    let mut changed = false;

    ui.horizontal_wrapped(|ui| {
        for (index, toolbar) in layout.toolbars().iter().enumerate() {
            if ui
                .selectable_label(selected == index, toolbar.name.as_str())
                .clicked()
            {
                state.selected = index;
            }
        }
    });

    let tools = layout
        .toolbars()
        .get(selected)
        .map(|toolbar| toolbar.tools.clone())
        .unwrap_or_default();
    let available = available_tools(layout, selected);
    let mut dropped: Option<(Tool, egui::Pos2)> = None;
    let mut dragging: Option<egui::Pos2> = None;
    let mut row_rects = Vec::new();

    let (on_rect, available_rect) = ui.columns(2, |columns| {
        let on_rect = {
            let ui = &mut columns[0];
            ui.heading("On the toolbar");
            for &tool in &tools {
                let label = tool_label(tool);
                let row = ui.horizontal(|ui| {
                    let handle = customize_drag_handle(ui, tool, label);
                    if handle.dragged() {
                        dragging = handle.interact_pointer_pos();
                    }
                    if handle.drag_stopped() {
                        dropped = handle.interact_pointer_pos().map(|pos| (tool, pos));
                    }
                    ui.label(label);
                });
                row_rects.push(row.response.rect);
            }
            ui.min_rect()
        };
        let available_rect = {
            let ui = &mut columns[1];
            ui.heading("Available");
            for &tool in &available {
                let label = tool_label(tool);
                ui.horizontal(|ui| {
                    let handle = customize_drag_handle(ui, tool, label);
                    if handle.dragged() {
                        dragging = handle.interact_pointer_pos();
                    }
                    if handle.drag_stopped() {
                        dropped = handle.interact_pointer_pos().map(|pos| (tool, pos));
                    }
                    ui.label(label);
                });
            }
            ui.min_rect()
        };
        (on_rect, available_rect)
    });

    if let Some(pos) = dragging
        && on_rect.contains(pos)
    {
        let index = insertion_index(Edge::Left, &row_rects, pos);
        let y = row_rects.get(index).map_or_else(
            || row_rects.last().map_or(on_rect.top(), |rect| rect.bottom()),
            |rect| rect.top(),
        );
        ui.painter().line_segment(
            [
                egui::pos2(on_rect.left(), y),
                egui::pos2(on_rect.right(), y),
            ],
            egui::Stroke::new(2.0, ui.visuals().selection.stroke.color),
        );
    }

    if let Some((tool, pos)) = dropped {
        let target = if on_rect.contains(pos) {
            Some(CustomizeDrop::OnToolbar(insertion_index(
                Edge::Left,
                &row_rects,
                pos,
            )))
        } else if available_rect.contains(pos) {
            Some(CustomizeDrop::Available)
        } else {
            None
        };
        if let Some(target) = target
            && apply_customize_drop(layout, selected, tool, target)
        {
            changed = true;
        }
    }

    ui.horizontal(|ui| {
        if ui.button("New Toolbar...").clicked() {
            state.prompt = Some(CustomizePrompt::New {
                name: String::new(),
            });
        }
        let toolbars = count > 0;
        if ui
            .add_enabled(toolbars, egui::Button::new("Rename Toolbar..."))
            .clicked()
            && let Some(toolbar) = layout.toolbars().get(selected)
        {
            state.prompt = Some(CustomizePrompt::Rename {
                index: selected,
                name: toolbar.name.clone(),
            });
        }
        if ui
            .add_enabled(toolbars, egui::Button::new("Delete Toolbar..."))
            .clicked()
        {
            state.prompt = Some(CustomizePrompt::Delete { index: selected });
        }
        if ui.button("Reset Toolbars...").clicked() {
            state.prompt = Some(CustomizePrompt::Reset);
        }
    });

    if let Some(mut prompt) = state.prompt.take() {
        let id = egui::Id::new("customize-prompt");
        let mut done = false;
        match &mut prompt {
            CustomizePrompt::New { name } => {
                if let Some(ok) = name_dialog(ui.ctx(), id, "New Toolbar", name) {
                    done = true;
                    if ok {
                        state.selected = layout.add_toolbar(name);
                        changed = true;
                    }
                }
            }
            CustomizePrompt::Rename { index, name } => {
                if let Some(ok) = name_dialog(ui.ctx(), id, "Rename Toolbar", name) {
                    done = true;
                    if ok && layout.rename_toolbar(*index, name) {
                        changed = true;
                    }
                }
            }
            CustomizePrompt::Delete { index } => {
                let name = layout
                    .toolbars()
                    .get(*index)
                    .map(|toolbar| toolbar.name.clone())
                    .unwrap_or_default();
                let message = format!("Delete the toolbar \"{name}\"?");
                if let Some(ok) = confirm_dialog(ui.ctx(), id, "Delete Toolbar", &message, "Delete")
                {
                    done = true;
                    if ok {
                        layout.remove_toolbar(*index);
                        let remaining = layout.toolbars().len();
                        state.selected = if remaining == 0 {
                            0
                        } else {
                            state.selected.min(remaining - 1)
                        };
                        changed = true;
                    }
                }
            }
            CustomizePrompt::Reset => {
                if let Some(ok) = confirm_dialog(
                    ui.ctx(),
                    id,
                    "Reset Toolbars",
                    "The toolbars go back to how they were installed and your own toolbars are deleted.",
                    "Reset",
                ) {
                    done = true;
                    if ok {
                        layout.reset();
                        state.selected = state
                            .selected
                            .min(layout.toolbars().len().saturating_sub(1));
                        changed = true;
                    }
                }
            }
        }
        if !done {
            state.prompt = Some(prompt);
        }
    }
    changed
}
