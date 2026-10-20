// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Drawing and selection in the viewport: runs inference and the kernel's tools on the
//! model's root body, and keeps the selection, the undo stack and the typed text of the
//! measurement box. No egui, no GPU: the window code feeds it pointer and key events.
//!
//! Behaviour: `docs/specs/m2-app-editor.md`.

use glam::{DVec2, DVec3};
use modeler_kernel::click::{self, ClickMods};
use modeler_kernel::infer::{self, Inference, Probe};
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::select::{self, BoxMode, Entity};
use modeler_kernel::tools::line::LineTool;
use modeler_kernel::tools::{Input, Target, ToolError};
use modeler_kernel::units::{self, Unit};
use modeler_view::{Camera, Projection};

use crate::input::{EditKey, Modifiers, Tool};

/// Snap radius for inference and picking, pixels.
pub const SNAP_RADIUS_PX: f64 = 10.0;

const TOL: f64 = modeler_kernel::geom::LINEAR_TOL;

const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];

fn unit_or(v: DVec3, fallback: DVec3) -> DVec3 {
    let len = v.length();
    if len.is_finite() && len > 0.0 {
        v / len
    } else {
        fallback
    }
}

/// The camera and the viewport size, (width, height) in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub camera: Camera,
    pub size_px: (f64, f64),
}

impl Viewport {
    fn size(&self) -> (f64, f64) {
        let (w, h) = self.size_px;
        if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
            (w, h)
        } else {
            (1.0, 1.0)
        }
    }

    fn aspect(&self) -> f64 {
        let (w, h) = self.size();
        w / h
    }

    /// The ray through the pixel `cursor_px`: (origin, unit direction).
    pub fn ray(&self, cursor_px: DVec2) -> (DVec3, DVec3) {
        let (w, h) = self.size();
        let aspect = self.aspect();
        let n = DVec2::new(2.0 * cursor_px.x / w - 1.0, 1.0 - 2.0 * cursor_px.y / h);
        let q = self.camera.target_plane_point(n, aspect);
        let (forward, _, _) = self.camera.basis();
        match self.camera.projection {
            Projection::Perspective => {
                let origin = self.camera.eye();
                let dir = unit_or(q - origin, forward);
                (origin, dir)
            }
            Projection::Parallel => {
                let origin = q - forward * (1000.0 * self.camera.distance);
                (origin, forward)
            }
        }
    }

    /// The pixel a world point appears at; `None` when it is behind a perspective camera.
    pub fn project(&self, p: DVec3) -> Option<DVec2> {
        let (w, h) = self.size();
        let (n, _) = self.camera.project(p, self.aspect())?;
        let c = DVec2::new((n.x + 1.0) / 2.0 * w, (1.0 - n.y) / 2.0 * h);
        c.is_finite().then_some(c)
    }
}

pub struct Editor {
    /// The unit for typed numbers without a unit, and for the measurement display.
    pub unit: Unit,
    tool: Tool,
    selection: Vec<Entity>,
    undo: UndoStack,
    typed: String,
    hint: Option<Inference>,
    arrow_lock: Option<usize>,
    shift_lock: Option<DVec3>,
    message: Option<String>,
    line: LineTool,
}

fn click_mods(mods: Modifiers) -> ClickMods {
    ClickMods {
        shift: mods.shift,
        ctrl: mods.ctrl,
        alt: mods.alt,
    }
}

fn error_message(error: &ToolError) -> &'static str {
    match error {
        ToolError::Parse(_) => "Not a length",
        ToolError::NotStarted => "Click a start point first",
        ToolError::NoDirection => "Move the cursor to show the direction",
        ToolError::Geometry(_) | ToolError::Model(_) => "Can't draw that here",
    }
}

impl Editor {
    /// Orbit tool, nothing selected, empty undo stack, inches.
    pub fn new() -> Editor {
        Editor {
            unit: Unit::Inch,
            tool: Tool::Orbit,
            selection: Vec::new(),
            undo: UndoStack::new(),
            typed: String::new(),
            hint: None,
            arrow_lock: None,
            shift_lock: None,
            message: None,
            line: LineTool::new(),
        }
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Switch tools; the shape in progress, the typed text and the locks are dropped.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.line = LineTool::new();
        self.typed.clear();
        self.arrow_lock = None;
        self.shift_lock = None;
        self.hint = None;
        self.message = None;
    }

    /// Sorted, no duplicates; only entities of the root body.
    pub fn selection(&self) -> &[Entity] {
        &self.selection
    }

    pub fn undo_stack(&self) -> &UndoStack {
        &self.undo
    }

    /// The cursor moved over the viewport. Never changes the model (the tools need it
    /// mutably).
    pub fn pointer_move(
        &mut self,
        model: &mut Model,
        view: &Viewport,
        cursor: DVec2,
        mods: Modifiers,
    ) {
        if self.tool != Tool::Line {
            self.hint = None;
            return;
        }
        self.update_shift_lock(mods);
        let inf = self.infer_at(model, view, cursor);
        match inf {
            Some(inf) => {
                self.hint = Some(inf);
                let mut target = Target {
                    model,
                    body: BodyId::Root,
                    undo: &mut self.undo,
                    unit: self.unit,
                };
                let _ = self.line.handle(Input::Move(inf.point), &mut target);
            }
            None => self.hint = None,
        }
    }

    /// A click; `count` is 1, 2 (double click) or 3 (triple click). True when the model
    /// changed.
    pub fn click(
        &mut self,
        model: &mut Model,
        view: &Viewport,
        cursor: DVec2,
        mods: Modifiers,
        count: u32,
    ) -> bool {
        match self.tool {
            Tool::Line => {
                let before = self.undo.undo_count();
                let Some(inf) = self.infer_at(model, view, cursor) else {
                    return false;
                };
                self.hint = Some(inf);
                let result = {
                    let mut target = Target {
                        model,
                        body: BodyId::Root,
                        undo: &mut self.undo,
                        unit: self.unit,
                    };
                    self.line.handle(Input::Click(inf.point), &mut target)
                };
                match result {
                    Ok(()) => {
                        self.message = None;
                        self.arrow_lock = None;
                        self.shift_lock = None;
                    }
                    Err(error) => self.message = Some(error_message(&error).to_string()),
                }
                let changed = self.undo.undo_count() != before;
                if changed {
                    self.prune(model);
                }
                changed
            }
            Tool::Select => {
                let hit = self.pick_at(model, view, cursor);
                let mods = click_mods(mods);
                self.selection = match hit {
                    Some(entity) if count >= 2 => {
                        let ctx = model.root().geometry();
                        let picked = if count == 2 {
                            select::bounded(ctx, entity)
                        } else {
                            select::connected(ctx, entity)
                        };
                        click::apply(&self.selection, &picked, mods)
                    }
                    _ => click::apply_click(&self.selection, hit, mods),
                };
                false
            }
            Tool::Orbit | Tool::Pan | Tool::Zoom => false,
        }
    }

    /// A selection box dragged from `start` to `end` (pixels) and released.
    pub fn box_select(
        &mut self,
        model: &Model,
        view: &Viewport,
        start: DVec2,
        end: DVec2,
        mods: Modifiers,
    ) {
        if self.tool != Tool::Select {
            return;
        }
        let project = |p: DVec3| view.project(p);
        let picked = select::box_select(
            model,
            BodyId::Root,
            &project,
            start,
            end,
            BoxMode::from_drag(start, end),
        );
        self.selection = click::apply(&self.selection, &picked, click_mods(mods));
    }

    /// Typed characters for the measurement box. True when they were taken.
    pub fn text(&mut self, text: &str) -> bool {
        if self.tool != Tool::Line {
            return false;
        }
        let filtered: String = text.chars().filter(|c| !c.is_control()).collect();
        if self.typed.is_empty() {
            let Some(first) = filtered.chars().next() else {
                return false;
            };
            if !matches!(first, '0'..='9' | '.' | '-' | '+' | ',') {
                return false;
            }
        }
        if filtered.is_empty() {
            return false;
        }
        self.typed.push_str(&filtered);
        true
    }

    /// An editor key. True when the model changed.
    pub fn key(&mut self, key: EditKey, model: &mut Model) -> bool {
        match key {
            EditKey::Cancel => {
                self.line = LineTool::new();
                self.typed.clear();
                self.arrow_lock = None;
                self.shift_lock = None;
                self.message = None;
                false
            }
            EditKey::Enter => {
                if self.typed.is_empty() {
                    return false;
                }
                let text = std::mem::take(&mut self.typed);
                if self.tool != Tool::Line {
                    return false;
                }
                let before = self.undo.undo_count();
                let result = {
                    let mut target = Target {
                        model,
                        body: BodyId::Root,
                        undo: &mut self.undo,
                        unit: self.unit,
                    };
                    self.line.handle(Input::Typed(text), &mut target)
                };
                match result {
                    Ok(()) => {
                        self.message = None;
                        self.arrow_lock = None;
                        self.shift_lock = None;
                    }
                    Err(error) => self.message = Some(error_message(&error).to_string()),
                }
                let changed = self.undo.undo_count() != before;
                if changed {
                    self.prune(model);
                }
                changed
            }
            EditKey::Backspace => {
                self.typed.pop();
                false
            }
            EditKey::Undo => {
                self.line = LineTool::new();
                self.typed.clear();
                match self.undo.undo(model) {
                    Ok(Some(_)) => {
                        self.prune(model);
                        true
                    }
                    Ok(None) => false,
                    Err(_) => {
                        self.message = Some("Can't undo".to_string());
                        false
                    }
                }
            }
            EditKey::Redo => {
                self.line = LineTool::new();
                self.typed.clear();
                match self.undo.redo(model) {
                    Ok(Some(_)) => {
                        self.prune(model);
                        true
                    }
                    Ok(None) => false,
                    Err(_) => {
                        self.message = Some("Can't redo".to_string());
                        false
                    }
                }
            }
            EditKey::Lock(i) => {
                if self.tool == Tool::Line && i < AXES.len() {
                    if self.arrow_lock == Some(i) {
                        self.arrow_lock = None;
                    } else {
                        self.arrow_lock = Some(i);
                    }
                }
                false
            }
        }
    }

    /// The measurement box's typed text.
    pub fn typed(&self) -> &str {
        &self.typed
    }

    /// The measurement box's label and value for the shape in progress.
    pub fn measurement(&self) -> Option<(&'static str, String)> {
        if self.tool != Tool::Line {
            return None;
        }
        let value = match self.line.length() {
            Some(length) => units::format_length(length, self.unit, 2),
            None => String::new(),
        };
        Some(("Length", value))
    }

    /// The last inferred point, for the hint.
    pub fn hint(&self) -> Option<Inference> {
        self.hint
    }

    /// The direction the drawing is locked to, if any.
    pub fn lock(&self) -> Option<DVec3> {
        if let Some(i) = self.arrow_lock {
            return AXES.get(i).copied();
        }
        self.shift_lock
    }

    /// Edges of the shape in progress, as the next click would draw them.
    pub fn preview(&self) -> Vec<(DVec3, DVec3)> {
        if self.tool != Tool::Line {
            return Vec::new();
        }
        self.line.preview().into_iter().collect()
    }

    /// Why the last input was refused, for the status bar.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    fn update_shift_lock(&mut self, mods: Modifiers) {
        if !mods.shift {
            self.shift_lock = None;
            return;
        }
        if self.shift_lock.is_some() {
            return;
        }
        let Some(anchor) = self.line.anchor() else {
            return;
        };
        let Some(hint) = self.hint else {
            return;
        };
        if hint.point.distance(anchor) <= TOL {
            return;
        }
        let lock = match hint.axis {
            Some(i) => {
                let Some(axis) = AXES.get(i).copied() else {
                    return;
                };
                axis
            }
            None => {
                let direction = hint.point - anchor;
                let len = direction.length();
                if !len.is_finite() || len <= 0.0 {
                    return;
                }
                direction / len
            }
        };
        self.shift_lock = Some(lock);
    }

    fn infer_at(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Inference> {
        let (origin, dir) = view.ray(cursor);
        let project = |p: DVec3| view.project(p);
        let centers: [DVec3; 0] = [];
        let probe = Probe {
            cursor,
            origin,
            dir,
            parallel: view.camera.projection == Projection::Parallel,
            project: &project,
            radius: SNAP_RADIUS_PX,
            anchor: self.line.anchor(),
            axes: AXES,
            centers: &centers,
            lock: self.lock(),
        };
        infer::infer(model.root().geometry(), &probe)
    }

    fn pick_at(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Entity> {
        let (origin, dir) = view.ray(cursor);
        let project = |p: DVec3| view.project(p);
        let centers: [DVec3; 0] = [];
        let probe = Probe {
            cursor,
            origin,
            dir,
            parallel: view.camera.projection == Projection::Parallel,
            project: &project,
            radius: SNAP_RADIUS_PX,
            anchor: self.line.anchor(),
            axes: AXES,
            centers: &centers,
            lock: self.lock(),
        };
        infer::pick(model.root().geometry(), &probe)
    }

    fn prune(&mut self, model: &Model) {
        let body = model.root();
        let ctx = body.geometry();
        self.selection.retain(|entity| match entity {
            Entity::Edge(id) => ctx.edge(*id).is_some(),
            Entity::Face(id) => ctx.face(*id).is_some(),
            Entity::Instance(id) => body.instance(*id).is_some(),
        });
        self.selection.sort();
        self.selection.dedup();
    }
}

impl Default for Editor {
    fn default() -> Self {
        Editor::new()
    }
}
