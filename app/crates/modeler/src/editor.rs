// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Drawing and selection in the viewport: runs inference and the kernel's tools on the
//! model's root body, and keeps the selection, the undo stack and the typed text of the
//! measurement box. No egui, no GPU: the window code feeds it pointer and key events.
//!
//! Behaviour: `docs/specs/m2-app-editor.md`, shape tools: `docs/specs/m2-app-shapes.md`.

use glam::{DVec2, DVec3};
use modeler_kernel::click::{self, ClickMods};
use modeler_kernel::infer::{self, Inference, Probe, Snap};
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::select::{self, BoxMode, Entity};
use modeler_kernel::tools::arc::ArcTool;
use modeler_kernel::tools::circle::CircleTool;
use modeler_kernel::tools::line::LineTool;
use modeler_kernel::tools::plane::DrawPlane;
use modeler_kernel::tools::rectangle::RectTool;
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

/// The draw plane for a shape on a plane with unit normal `n`: `u` horizontal unless the
/// plane is, `u × v = n`; the ground gives `(X, Y)`.
pub fn plane_for_normal(n: DVec3) -> DrawPlane {
    let n = unit_or(n, DVec3::Z);
    if n.z.abs() >= 1.0 - 1e-9 {
        let u = DVec3::X;
        DrawPlane { u, v: n.cross(u) }
    } else {
        let u = unit_or(DVec3::Z.cross(n), DVec3::X);
        DrawPlane { u, v: n.cross(u) }
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
    rect: RectTool,
    circle: CircleTool,
    arc: ArcTool,
    plane_lock: Option<usize>,
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
            rect: RectTool::new(plane_for_normal(DVec3::Z)),
            circle: CircleTool::new(plane_for_normal(DVec3::Z)),
            arc: ArcTool::new(plane_for_normal(DVec3::Z)),
            plane_lock: None,
        }
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Switch tools; the shape in progress, the typed text and the locks are dropped.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.line = LineTool::new();
        let mut idle_model = Model::new();
        self.idle_shape_tools(&mut idle_model);
        self.typed.clear();
        self.arrow_lock = None;
        self.shift_lock = None;
        self.plane_lock = None;
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
        match self.tool {
            Tool::Line => {
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
            Tool::Rectangle | Tool::Circle | Tool::Arc => {
                let (origin, dir) = view.ray(cursor);
                if self.shape_anchor().is_none() {
                    self.pick_shape_plane(model, origin, dir);
                }
                let Some(inf) = self.shape_infer_at(model, view, cursor) else {
                    self.hint = None;
                    return;
                };
                let point = self.shape_point(inf, origin, dir);
                self.hint = Some(Inference { point, ..inf });
                let tool = self.tool;
                let mut target = Target {
                    model,
                    body: BodyId::Root,
                    undo: &mut self.undo,
                    unit: self.unit,
                };
                let _ = match tool {
                    Tool::Rectangle => self.rect.handle(Input::Move(point), &mut target),
                    Tool::Circle => self.circle.handle(Input::Move(point), &mut target),
                    Tool::Arc => self.arc.handle(Input::Move(point), &mut target),
                    _ => Ok(()),
                };
            }
            Tool::Orbit | Tool::Pan | Tool::Zoom | Tool::Select => self.hint = None,
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
            Tool::Rectangle | Tool::Circle | Tool::Arc => {
                let (origin, dir) = view.ray(cursor);
                if self.shape_anchor().is_none() {
                    self.pick_shape_plane(model, origin, dir);
                }
                let Some(inf) = self.shape_infer_at(model, view, cursor) else {
                    return false;
                };
                let point = self.shape_point(inf, origin, dir);
                self.hint = Some(Inference { point, ..inf });
                let tool = self.tool;
                let before = self.undo.undo_count();
                let result = {
                    let mut target = Target {
                        model,
                        body: BodyId::Root,
                        undo: &mut self.undo,
                        unit: self.unit,
                    };
                    match tool {
                        Tool::Rectangle => self.rect.handle(Input::Click(point), &mut target),
                        Tool::Circle => self.circle.handle(Input::Click(point), &mut target),
                        Tool::Arc => self.arc.handle(Input::Click(point), &mut target),
                        _ => Ok(()),
                    }
                };
                match result {
                    Ok(()) => self.message = None,
                    Err(error) => self.message = Some(error_message(&error).to_string()),
                }
                let changed = self.undo.undo_count() != before;
                if changed {
                    self.prune(model);
                }
                changed
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
        if !self.drawing_tool() {
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
                self.idle_shape_tools(model);
                self.typed.clear();
                self.arrow_lock = None;
                self.shift_lock = None;
                self.plane_lock = None;
                self.message = None;
                false
            }
            EditKey::Enter => {
                if self.typed.is_empty() {
                    return false;
                }
                let text = std::mem::take(&mut self.typed);
                let tool = self.tool;
                if !matches!(
                    tool,
                    Tool::Line | Tool::Rectangle | Tool::Circle | Tool::Arc
                ) {
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
                    match tool {
                        Tool::Line => self.line.handle(Input::Typed(text), &mut target),
                        Tool::Rectangle => self.rect.handle(Input::Typed(text), &mut target),
                        Tool::Circle => self.circle.handle(Input::Typed(text), &mut target),
                        Tool::Arc => self.arc.handle(Input::Typed(text), &mut target),
                        _ => Ok(()),
                    }
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
                self.idle_shape_tools(model);
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
                self.idle_shape_tools(model);
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
                } else if self.shape_tool() && i < AXES.len() && self.shape_anchor().is_none() {
                    if self.plane_lock == Some(i) {
                        self.plane_lock = None;
                    } else {
                        self.plane_lock = Some(i);
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
        let length = |value: Option<f64>| match value {
            Some(length) => units::format_length(length, self.unit, 2),
            None => String::new(),
        };
        match self.tool {
            Tool::Line => Some(("Length", length(self.line.length()))),
            Tool::Rectangle => {
                let value = match self.rect.size() {
                    Some((w, h)) => {
                        format!("{}, {}", length(Some(w)), length(Some(h)))
                    }
                    None => String::new(),
                };
                Some(("Dimensions", value))
            }
            Tool::Circle => {
                if self.circle.anchor().is_none() {
                    Some(("Segments", self.circle.segments().to_string()))
                } else {
                    Some(("Radius", length(self.circle.radius())))
                }
            }
            Tool::Arc => match self.arc.placed() {
                0 => Some(("Segments", self.arc.segments().to_string())),
                1 => Some(("Radius", length(self.arc.radius()))),
                _ => {
                    let value = match self.arc.sweep() {
                        Some(sweep) => format!("{:.1}°", sweep.to_degrees()),
                        None => String::new(),
                    };
                    Some(("Angle", value))
                }
            },
            Tool::Orbit | Tool::Pan | Tool::Zoom | Tool::Select => None,
        }
    }

    /// The last inferred point, for the hint.
    pub fn hint(&self) -> Option<Inference> {
        self.hint
    }

    /// The direction the drawing is locked to, if any.
    pub fn lock(&self) -> Option<DVec3> {
        if self.shape_tool() {
            return None;
        }
        if let Some(i) = self.arrow_lock {
            return AXES.get(i).copied();
        }
        self.shift_lock
    }

    /// Edges of the shape in progress, as the next click would draw them.
    pub fn preview(&self) -> Vec<(DVec3, DVec3)> {
        match self.tool {
            Tool::Line => self.line.preview().into_iter().collect(),
            Tool::Rectangle => match self.rect.preview() {
                Some(c) => (0..4).map(|i| (c[i], c[(i + 1) % 4])).collect(),
                None => Vec::new(),
            },
            Tool::Circle => match self.circle.preview() {
                Some(points) => loops(&points, true),
                None => Vec::new(),
            },
            Tool::Arc => match self.arc.preview() {
                Some(points) => loops(&points, false),
                None => Vec::new(),
            },
            Tool::Orbit | Tool::Pan | Tool::Zoom | Tool::Select => Vec::new(),
        }
    }

    /// The plane the shape tool draws in (the next shape's while idle); `None` for other tools.
    pub fn draw_plane(&self) -> Option<DrawPlane> {
        match self.tool {
            Tool::Rectangle => Some(self.rect.plane()),
            Tool::Circle => Some(self.circle.plane()),
            Tool::Arc => Some(self.arc.plane()),
            Tool::Orbit | Tool::Pan | Tool::Zoom | Tool::Select | Tool::Line => None,
        }
    }

    /// The axis the shape tool's plane normal is locked to (arrow keys), if any.
    pub fn plane_lock(&self) -> Option<usize> {
        self.shape_tool().then_some(self.plane_lock).flatten()
    }

    /// Why the last input was refused, for the status bar.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    fn drawing_tool(&self) -> bool {
        matches!(
            self.tool,
            Tool::Line | Tool::Rectangle | Tool::Circle | Tool::Arc
        )
    }

    fn shape_tool(&self) -> bool {
        matches!(self.tool, Tool::Rectangle | Tool::Circle | Tool::Arc)
    }

    fn shape_anchor(&self) -> Option<DVec3> {
        match self.tool {
            Tool::Rectangle => self.rect.anchor(),
            Tool::Circle => self.circle.anchor(),
            Tool::Arc => self.arc.anchor(),
            _ => None,
        }
    }

    fn shape_plane(&self) -> Option<DrawPlane> {
        match self.tool {
            Tool::Rectangle => Some(self.rect.plane()),
            Tool::Circle => Some(self.circle.plane()),
            Tool::Arc => Some(self.arc.plane()),
            _ => None,
        }
    }

    fn idle_shape_tools(&mut self, model: &mut Model) {
        let Editor {
            rect,
            circle,
            arc,
            undo,
            unit,
            ..
        } = self;
        let mut target = Target {
            model,
            body: BodyId::Root,
            undo,
            unit: *unit,
        };
        let _ = rect.handle(Input::Cancel, &mut target);
        let _ = circle.handle(Input::Cancel, &mut target);
        let _ = arc.handle(Input::Cancel, &mut target);
    }

    fn pick_shape_plane(&mut self, model: &Model, origin: DVec3, dir: DVec3) {
        let normal = match self.plane_lock.and_then(|i| AXES.get(i).copied()) {
            Some(axis) => axis,
            None => {
                let geometry = model.root().geometry();
                match infer::ray_face(geometry, origin, dir)
                    .and_then(|(face, _)| geometry.face_plane(face))
                {
                    Some(plane) => plane.normal,
                    None => DVec3::Z,
                }
            }
        };
        let normal = if normal.dot(dir) > 0.0 {
            -normal
        } else {
            normal
        };
        let plane = plane_for_normal(normal);
        match self.tool {
            Tool::Rectangle => self.rect.set_plane(plane),
            Tool::Circle => self.circle.set_plane(plane),
            Tool::Arc => self.arc.set_plane(plane),
            _ => {}
        }
    }

    fn shape_point(&self, inf: Inference, origin: DVec3, dir: DVec3) -> DVec3 {
        let Some(anchor) = self.shape_anchor() else {
            return inf.point;
        };
        if !matches!(inf.snap, Snap::OnFace(_) | Snap::Ground | Snap::Along) {
            return inf.point;
        }
        let Some(plane) = self.shape_plane() else {
            return inf.point;
        };
        let normal = plane.normal();
        let denom = dir.dot(normal);
        if denom.abs() <= 1e-9 {
            return inf.point;
        }
        let t = (anchor - origin).dot(normal) / denom;
        if !t.is_finite() || t <= 0.0 {
            return inf.point;
        }
        let hit = origin + dir * t;
        if hit.is_finite() { hit } else { inf.point }
    }

    fn shape_infer_at(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Inference> {
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
            anchor: self.shape_anchor(),
            axes: AXES,
            centers: &centers,
            lock: None,
        };
        infer::infer(model.root().geometry(), &probe)
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

fn loops(points: &[DVec3], closed: bool) -> Vec<(DVec3, DVec3)> {
    if points.len() < 2 {
        return Vec::new();
    }
    let mut edges: Vec<_> = points.windows(2).map(|pair| (pair[0], pair[1])).collect();
    if closed {
        edges.push((points[points.len() - 1], points[0]));
    }
    edges
}

impl Default for Editor {
    fn default() -> Self {
        Editor::new()
    }
}
