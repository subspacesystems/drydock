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
use modeler_kernel::tools::arc3::Arc3Tool;
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
    if n.z.abs() >= 1.0 - 1e-9 {
        let u = DVec3::X;
        let v = n.cross(u);
        DrawPlane { u, v }
    } else {
        let u = unit_or(DVec3::Z.cross(n), DVec3::X);
        let v = n.cross(u);
        DrawPlane { u, v }
    }
}

fn is_shape(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Rectangle | Tool::Circle | Tool::Arc | Tool::Arc3
    )
}

fn is_drawing(tool: Tool) -> bool {
    tool == Tool::Line || is_shape(tool)
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
    plane_lock: Option<usize>,
    message: Option<String>,
    line: LineTool,
    rect: RectTool,
    circle: CircleTool,
    arc: ArcTool,
    arc3: Arc3Tool,
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
            plane_lock: None,
            message: None,
            line: LineTool::new(),
            rect: RectTool::new(plane_for_normal(DVec3::Z)),
            circle: CircleTool::new(plane_for_normal(DVec3::Z)),
            arc: ArcTool::new(plane_for_normal(DVec3::Z)),
            arc3: Arc3Tool::new(plane_for_normal(DVec3::Z)),
        }
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Switch tools; the shape in progress, the typed text and the locks are dropped.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.line = LineTool::new();
        self.idle_shapes();
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
        if is_shape(self.tool) {
            let (origin, dir) = view.ray(cursor);
            match self.infer_shape_at(model, view, cursor) {
                Some(inf) => {
                    if self.shape_idle() {
                        self.pick_shape_plane(model, origin, dir);
                    }
                    let point = self.shape_input_point(inf, origin, dir);
                    self.hint = Some(Inference { point, ..inf });
                    let _ = self.handle_shape(Input::Move(point), model);
                }
                None => self.hint = None,
            }
            return;
        }
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
            Tool::Rectangle | Tool::Circle | Tool::Arc | Tool::Arc3 => {
                let before = self.undo.undo_count();
                let (origin, dir) = view.ray(cursor);
                let Some(inf) = self.infer_shape_at(model, view, cursor) else {
                    self.hint = None;
                    return false;
                };
                if self.shape_idle() {
                    self.pick_shape_plane(model, origin, dir);
                }
                let point = self.shape_input_point(inf, origin, dir);
                self.hint = Some(Inference { point, ..inf });
                let result = self.handle_shape(Input::Click(point), model);
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
        if !is_drawing(self.tool) {
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
                self.idle_shapes();
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
                if !is_drawing(self.tool) {
                    return false;
                }
                let before = self.undo.undo_count();
                let result = if self.tool == Tool::Line {
                    let mut target = Target {
                        model,
                        body: BodyId::Root,
                        undo: &mut self.undo,
                        unit: self.unit,
                    };
                    self.line.handle(Input::Typed(text), &mut target)
                } else {
                    self.handle_shape(Input::Typed(text), model)
                };
                match result {
                    Ok(()) => {
                        self.message = None;
                        if self.tool == Tool::Line {
                            self.arrow_lock = None;
                            self.shift_lock = None;
                        }
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
                self.idle_shapes();
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
                self.idle_shapes();
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
                if is_shape(self.tool) {
                    if self.shape_idle() && i < AXES.len() {
                        if self.plane_lock == Some(i) {
                            self.plane_lock = None;
                        } else {
                            self.plane_lock = Some(i);
                        }
                    }
                } else if self.tool == Tool::Line && i < AXES.len() {
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
        let length = |value: f64| units::format_length(value, self.unit, 2);
        match self.tool {
            Tool::Line => {
                let value = match self.line.length() {
                    Some(value) => length(value),
                    None => String::new(),
                };
                Some(("Length", value))
            }
            Tool::Rectangle => {
                let value = match self.rect.size() {
                    Some((w, h)) => format!("{}, {}", length(w), length(h)),
                    None => String::new(),
                };
                Some(("Dimensions", value))
            }
            Tool::Circle => {
                if self.circle.anchor().is_none() {
                    Some(("Segments", self.circle.segments().to_string()))
                } else {
                    let value = match self.circle.radius() {
                        Some(value) => length(value),
                        None => String::new(),
                    };
                    Some(("Radius", value))
                }
            }
            Tool::Arc => match self.arc.placed() {
                0 => Some(("Segments", self.arc.segments().to_string())),
                1 => {
                    let value = match self.arc.radius() {
                        Some(value) => length(value),
                        None => String::new(),
                    };
                    Some(("Radius", value))
                }
                _ => {
                    let value = match self.arc.sweep() {
                        Some(value) => format!("{:.1}°", value.to_degrees()),
                        None => String::new(),
                    };
                    Some(("Angle", value))
                }
            },
            Tool::Arc3 => match self.arc3.placed() {
                0 => Some(("Segments", self.arc3.segments().to_string())),
                1 => {
                    let value = match self.arc3.chord() {
                        Some(value) => length(value),
                        None => String::new(),
                    };
                    Some(("Length", value))
                }
                _ => {
                    let value = match (self.arc3.radius(), self.arc3.chord(), self.arc3.preview()) {
                        (Some(r), Some(c), Some(points)) if points.len() >= 2 => {
                            let a = points[0];
                            let b = points[points.len() - 1];
                            let mid = points[points.len() / 2];
                            let d = b - a;
                            let distance = if d.length() > TOL {
                                (mid - a).cross(d).length() / d.length()
                            } else {
                                f64::INFINITY
                            };
                            let h0 = (r * r - c * c / 4.0).max(0.0).sqrt();
                            let bulge = if distance > r { r + h0 } else { r - h0 };
                            length(bulge)
                        }
                        _ => String::new(),
                    };
                    Some(("Bulge", value))
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
        if is_shape(self.tool) {
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
            Tool::Rectangle => self
                .rect
                .preview()
                .map(|c| (0..4).map(|i| (c[i], c[(i + 1) % 4])).collect())
                .unwrap_or_default(),
            Tool::Circle => self
                .circle
                .preview()
                .map(|p| {
                    let n = p.len();
                    (0..n).map(|i| (p[i], p[(i + 1) % n])).collect()
                })
                .unwrap_or_default(),
            Tool::Arc => self
                .arc
                .preview()
                .map(|p| p.windows(2).map(|w| (w[0], w[1])).collect())
                .unwrap_or_default(),
            Tool::Arc3 => self
                .arc3
                .preview()
                .map(|p| p.windows(2).map(|w| (w[0], w[1])).collect())
                .unwrap_or_default(),
            Tool::Orbit | Tool::Pan | Tool::Zoom | Tool::Select => Vec::new(),
        }
    }

    /// The plane the shape tool draws in (the next shape's while idle); `None` for other tools.
    pub fn draw_plane(&self) -> Option<DrawPlane> {
        match self.tool {
            Tool::Rectangle => Some(self.rect.plane()),
            Tool::Circle => Some(self.circle.plane()),
            Tool::Arc => Some(self.arc.plane()),
            Tool::Arc3 => Some(self.arc3.plane()),
            Tool::Orbit | Tool::Pan | Tool::Zoom | Tool::Select | Tool::Line => None,
        }
    }

    /// The axis the shape tool's plane normal is locked to (arrow keys), if any.
    pub fn plane_lock(&self) -> Option<usize> {
        if is_shape(self.tool) {
            self.plane_lock
        } else {
            None
        }
    }

    /// Why the last input was refused, for the status bar.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    fn shape_anchor(&self) -> Option<DVec3> {
        match self.tool {
            Tool::Rectangle => self.rect.anchor(),
            Tool::Circle => self.circle.anchor(),
            Tool::Arc => self.arc.anchor(),
            Tool::Arc3 => self.arc3.anchor(),
            _ => None,
        }
    }

    fn shape_plane(&self) -> Option<DrawPlane> {
        match self.tool {
            Tool::Rectangle => Some(self.rect.plane()),
            Tool::Circle => Some(self.circle.plane()),
            Tool::Arc => Some(self.arc.plane()),
            Tool::Arc3 => Some(self.arc3.plane()),
            _ => None,
        }
    }

    fn shape_idle(&self) -> bool {
        self.shape_anchor().is_none()
    }

    fn set_shape_plane(&mut self, plane: DrawPlane) {
        match self.tool {
            Tool::Rectangle => self.rect.set_plane(plane),
            Tool::Circle => self.circle.set_plane(plane),
            Tool::Arc => self.arc.set_plane(plane),
            Tool::Arc3 => self.arc3.set_plane(plane),
            _ => {}
        }
    }

    fn handle_shape(&mut self, input: Input, model: &mut Model) -> Result<(), ToolError> {
        let mut target = Target {
            model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: self.unit,
        };
        match self.tool {
            Tool::Rectangle => self.rect.handle(input, &mut target),
            Tool::Circle => self.circle.handle(input, &mut target),
            Tool::Arc => self.arc.handle(input, &mut target),
            Tool::Arc3 => self.arc3.handle(input, &mut target),
            _ => Ok(()),
        }
    }

    fn idle_shapes(&mut self) {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let mut target = Target {
            model: &mut model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: self.unit,
        };
        let _ = self.rect.handle(Input::Cancel, &mut target);
        let _ = self.circle.handle(Input::Cancel, &mut target);
        let _ = self.arc.handle(Input::Cancel, &mut target);
        let _ = self.arc3.handle(Input::Cancel, &mut target);
    }

    fn pick_shape_plane(&mut self, model: &Model, origin: DVec3, dir: DVec3) {
        let n = match self.plane_lock.and_then(|i| AXES.get(i).copied()) {
            Some(axis) => axis,
            None => {
                let ctx = model.root().geometry();
                match infer::ray_face(ctx, origin, dir).and_then(|(face, _)| ctx.face_plane(face)) {
                    Some(plane) => plane.normal,
                    None => DVec3::Z,
                }
            }
        };
        let n = if n.dot(dir) > 0.0 { -n } else { n };
        self.set_shape_plane(plane_for_normal(unit_or(n, DVec3::Z)));
    }

    fn shape_input_point(&self, inf: Inference, origin: DVec3, dir: DVec3) -> DVec3 {
        if !self.shape_idle()
            && matches!(inf.snap, Snap::OnFace(_) | Snap::Ground | Snap::Along)
            && let (Some(anchor), Some(plane)) = (self.shape_anchor(), self.shape_plane())
        {
            let n = plane.normal();
            let denom = dir.dot(n);
            if denom.abs() > 1e-9 {
                let t = (anchor - origin).dot(n) / denom;
                let hit = origin + dir * t;
                if t > 0.0 && hit.is_finite() {
                    return hit;
                }
            }
        }
        inf.point
    }

    fn infer_shape_at(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Inference> {
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
        infer::infer_model(model, &probe)
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
        infer::infer_model(model, &probe)
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
        infer::pick_model(model, &probe)
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
