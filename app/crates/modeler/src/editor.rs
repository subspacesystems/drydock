// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Drawing and selection in the viewport: runs inference and the kernel's tools on the
//! model's root body, and keeps the selection, the undo stack and the typed text of the
//! measurement box. No egui, no GPU: the window code feeds it pointer and key events.
//!
//! Behaviour: `docs/specs/m2-app-editor.md`, shape tools: `docs/specs/m2-app-shapes.md`,
//! Push/Pull: `docs/specs/m3-app-pushpull.md`, Rotate: `docs/specs/m3-app-rotate.md`,
//! Move: `docs/specs/m3-app-move.md`, Scale and Flip: `docs/specs/m3-app-scale-flip.md`,
//! Tape: `docs/specs/m3-app-tape.md`.

use std::collections::BTreeSet;

use glam::{DMat4, DVec2, DVec3};
use modeler_kernel::click::{self, ClickMods};
use modeler_kernel::erase::erase;
use modeler_kernel::infer::{self, Inference, Probe, Snap};
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::pushpull::push_pull;
use modeler_kernel::select::{self, BoxMode, Entity};
use modeler_kernel::tape::Guide;
use modeler_kernel::tools::arc::ArcTool;
use modeler_kernel::tools::arc3::Arc3Tool;
use modeler_kernel::tools::circle::CircleTool;
use modeler_kernel::tools::line::LineTool;
use modeler_kernel::tools::plane::DrawPlane;
use modeler_kernel::tools::rectangle::RectTool;
use modeler_kernel::tools::rotate::{RotateError, RotateStage, RotateTool};
use modeler_kernel::tools::scale::{
    Bounds, Handle, ScaleError, ScaleStage, ScaleTool, bounds, handles,
};
use modeler_kernel::tools::tape::{TapeError, TapeStage, TapeTool};
use modeler_kernel::tools::translate::{MoveError, MoveStage, MoveTool};
use modeler_kernel::tools::{Input, Target, ToolError};
use modeler_kernel::topology::FaceId;
use modeler_kernel::transform;
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

fn takes_text(tool: Tool) -> bool {
    is_drawing(tool)
        || tool == Tool::PushPull
        || tool == Tool::Rotate
        || tool == Tool::Move
        || tool == Tool::Scale
        || tool == Tool::Tape
}

/// The parameter along `start + t·n` of the point closest to the ray `origin + u·dir`.
/// `None` when the line and the ray are parallel.
fn closest_on_line(start: DVec3, n: DVec3, origin: DVec3, dir: DVec3) -> Option<f64> {
    let w = start - origin;
    let b = n.dot(dir);
    let denom = 1.0 - b * b;
    if denom.abs() < 1e-12 {
        return None;
    }
    Some((b * dir.dot(w) - n.dot(w)) / denom)
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

struct PushState {
    face: FaceId,
    start: DVec3,
    normal: DVec3,
    distance: f64,
    loops: Vec<Vec<DVec3>>,
}

#[derive(Clone, Copy)]
struct LastPush {
    face: FaceId,
    distance: f64,
    undo_count: usize,
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
    push: Option<PushState>,
    push_hover: Option<FaceId>,
    last_push: Option<LastPush>,
    rotate: RotateTool,
    rotate_segments: Vec<(DVec3, DVec3)>,
    translate: MoveTool,
    move_segments: Vec<(DVec3, DVec3)>,
    scale: ScaleTool,
    scale_hover: Option<Handle>,
    tape: TapeTool,
    tape_point_guides: bool,
    scale_segments: Vec<(DVec3, DVec3)>,
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

/// True when the inferred point is not snapped to a vertex, edge or axis line, so tools may
/// round it (Rotate's 15 degree steps).
fn free_point(inf: &Inference) -> bool {
    matches!(inf.snap, Snap::OnFace(_) | Snap::Ground | Snap::Along)
}

fn rotate_error_message(error: &RotateError) -> String {
    match error {
        RotateError::Parse(_) => "Not an angle".to_string(),
        RotateError::NotStarted => "Click the center first".to_string(),
        RotateError::Empty => "Select something to rotate".to_string(),
        RotateError::NotFinite => "Can't rotate that".to_string(),
        // The reason is shown until the refusals are understood well enough to word them.
        RotateError::Transform(why) => format!("Can't rotate that ({why:?})"),
    }
}

fn move_error_message(error: &MoveError) -> &'static str {
    match error {
        MoveError::Parse(_) => "Not a length",
        MoveError::NotStarted => "Click a start point first",
        MoveError::NoDirection => "Move the cursor to show the direction",
        MoveError::Empty => "Select something to move",
        MoveError::NotFinite | MoveError::Transform(_) => "Can't move that",
    }
}

fn tape_error_message(error: &TapeError) -> &'static str {
    match error {
        TapeError::Parse(_) => "Not a length",
        TapeError::NotStarted => "Click a start point first",
        TapeError::NotParallel => "Start on an edge to place a guide",
        TapeError::NotFinite => "Can't measure that",
        TapeError::NoDirection => "Move the cursor to show the side",
        TapeError::BadLength => "Length must be more than zero",
        TapeError::Model(_) => "Can't add that guide",
    }
}

fn scale_error_message(error: &ScaleError) -> &'static str {
    match error {
        ScaleError::Parse(_) => "Not a scale factor",
        ScaleError::WrongCount => "Wrong number of factors",
        ScaleError::ZeroFactor => "Can't scale to nothing",
        ScaleError::NotStarted => "Click a handle first",
        ScaleError::Empty => "Select something to scale",
        ScaleError::NoHandle => "Click a handle to scale",
        ScaleError::NotFinite | ScaleError::Transform(_) => "Can't scale that",
    }
}

fn format_factors(factors: DVec3) -> String {
    if (factors.x - factors.y).abs() <= 1e-9 && (factors.y - factors.z).abs() <= 1e-9 {
        format!("{:.3}", factors.x)
    } else {
        format!("{:.3}, {:.3}, {:.3}", factors.x, factors.y, factors.z)
    }
}

fn box_edges(bounds: Bounds) -> Vec<(DVec3, DVec3)> {
    let corner = |i: usize| {
        DVec3::new(
            if i & 1 != 0 {
                bounds.max.x
            } else {
                bounds.min.x
            },
            if i & 2 != 0 {
                bounds.max.y
            } else {
                bounds.min.y
            },
            if i & 4 != 0 {
                bounds.max.z
            } else {
                bounds.min.z
            },
        )
    };
    let mut edges = Vec::with_capacity(12);
    for i in 0..8 {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                edges.push((corner(i), corner(i | bit)));
            }
        }
    }
    edges
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
            push: None,
            push_hover: None,
            last_push: None,
            rotate: RotateTool::new(plane_for_normal(DVec3::Z)),
            rotate_segments: Vec::new(),
            translate: MoveTool::new(),
            move_segments: Vec::new(),
            scale: ScaleTool::new(),
            scale_hover: None,
            tape: TapeTool::new(),
            tape_point_guides: false,
            scale_segments: Vec::new(),
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
        self.push = None;
        self.push_hover = None;
        self.rotate = RotateTool::new(plane_for_normal(DVec3::Z));
        self.rotate_segments.clear();
        self.translate = MoveTool::new();
        self.move_segments.clear();
        self.scale = ScaleTool::new();
        self.scale_hover = None;
        self.scale_segments.clear();
        self.tape = TapeTool::new();
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
        if self.tool == Tool::PushPull {
            self.move_push(model, view, cursor);
            return;
        }
        if self.tool == Tool::Rotate {
            self.move_rotate(model, view, cursor);
            return;
        }
        if self.tool == Tool::Move {
            self.move_move(model, view, cursor);
            return;
        }
        if self.tool == Tool::Scale {
            self.move_scale(model, view, cursor, mods);
            return;
        }
        if self.tool == Tool::Tape {
            self.move_tape(model, view, cursor);
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
            Tool::PushPull => self.click_push(model, view, cursor, count),
            Tool::Rotate => self.click_rotate(model, view, cursor),
            Tool::Move => self.click_move(model, view, cursor),
            Tool::Scale => self.click_scale(model, view, cursor, mods),
            Tool::Tape => self.click_tape(model, view, cursor),
            Tool::Erase => self.click_erase(model, view, cursor),
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
        if !takes_text(self.tool) {
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
                self.idle_rotate();
                self.idle_move();
                self.idle_scale();
                self.idle_tape();
                self.typed.clear();
                self.arrow_lock = None;
                self.shift_lock = None;
                self.plane_lock = None;
                self.message = None;
                self.push = None;
                if self.tool == Tool::PushPull {
                    self.hint = None;
                }
                false
            }
            EditKey::Enter => {
                if self.typed.is_empty() {
                    return false;
                }
                let text = std::mem::take(&mut self.typed);
                if self.tool == Tool::PushPull {
                    return self.enter_push(text, model);
                }
                if self.tool == Tool::Rotate {
                    return self.enter_rotate(text, model);
                }
                if self.tool == Tool::Move {
                    return self.enter_move(text, model);
                }
                if self.tool == Tool::Scale {
                    return self.enter_scale(text, model);
                }
                if self.tool == Tool::Tape {
                    return self.enter_tape(text, model);
                }
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
                if !self.typed.is_empty() {
                    self.typed.pop();
                    return false;
                }
                if matches!(self.tool, Tool::Select | Tool::Erase) {
                    self.erase_selection(model)
                } else {
                    false
                }
            }
            EditKey::Undo => {
                self.line = LineTool::new();
                self.idle_shapes();
                self.idle_rotate();
                self.idle_move();
                self.idle_scale();
                self.idle_tape();
                self.typed.clear();
                self.push = None;
                if self.tool == Tool::PushPull {
                    self.hint = None;
                }
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
                self.idle_rotate();
                self.idle_move();
                self.idle_scale();
                self.idle_tape();
                self.typed.clear();
                self.push = None;
                if self.tool == Tool::PushPull {
                    self.hint = None;
                }
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
                let plane_idle = if is_shape(self.tool) {
                    self.shape_idle()
                } else if self.tool == Tool::Rotate {
                    self.rotate.stage() == RotateStage::Center
                } else {
                    false
                };
                if plane_idle && i < AXES.len() {
                    if self.plane_lock == Some(i) {
                        self.plane_lock = None;
                    } else {
                        self.plane_lock = Some(i);
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
            Tool::PushPull => {
                let value = match self.push.as_ref() {
                    Some(push) => Some(length(push.distance.abs())),
                    None => self
                        .last_push
                        .as_ref()
                        .map(|last| length(last.distance.abs())),
                };
                Some(("Distance", value.unwrap_or_default()))
            }
            Tool::Rotate => {
                let value = match self.rotate.angle() {
                    Some(angle) => format!("{:.1}°", angle),
                    None => match self.rotate.last_angle(&self.undo) {
                        Some(angle) => format!("{:.1}°", angle),
                        None => String::new(),
                    },
                };
                Some(("Angle", value))
            }
            Tool::Move => {
                let value = match self.translate.offset() {
                    Some(offset) => length(offset.length()),
                    None => match self.translate.last_offset(&self.undo) {
                        Some(offset) => length(offset.length()),
                        None => String::new(),
                    },
                };
                Some(("Distance", value))
            }
            Tool::Scale => {
                let factors = if self.scale.stage() == ScaleStage::Drag {
                    self.scale.factors()
                } else {
                    self.scale.last_factors(&self.undo)
                };
                let value = match factors {
                    Some(factors) => format_factors(factors),
                    None => String::new(),
                };
                Some(("Scale", value))
            }
            Tool::Tape => {
                let value = match self.tape.stage() {
                    TapeStage::End => self.tape.distance(),
                    TapeStage::Start => self.tape.last_distance(),
                };
                Some(("Length", value.map(length).unwrap_or_default()))
            }
            Tool::Orbit | Tool::Pan | Tool::Zoom | Tool::Select | Tool::Erase => None,
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
            Tool::PushPull => {
                let Some(push) = self.push.as_ref() else {
                    return Vec::new();
                };
                let offset = push.normal * push.distance;
                let mut segments = Vec::new();
                for points in &push.loops {
                    let n = points.len();
                    if n < 2 {
                        continue;
                    }
                    for i in 0..n {
                        let a = points[i];
                        let b = points[(i + 1) % n];
                        segments.push((a + offset, b + offset));
                        segments.push((a, a + offset));
                    }
                }
                segments
            }
            Tool::Rotate => {
                let mut segments = self.rotate.preview();
                if self.rotate.stage() == RotateStage::Angle
                    && let Some(matrix) = self.rotate.matrix()
                {
                    for &(a, b) in &self.rotate_segments {
                        segments.push((matrix.transform_point3(a), matrix.transform_point3(b)));
                    }
                }
                segments
            }
            Tool::Move => {
                let mut segments = self.translate.preview();
                if let Some(matrix) = self.translate.matrix() {
                    for &(a, b) in &self.move_segments {
                        segments.push((matrix.transform_point3(a), matrix.transform_point3(b)));
                    }
                }
                segments
            }
            Tool::Scale => {
                if self.scale.stage() != ScaleStage::Drag {
                    return Vec::new();
                }
                let Some(bounds) = self.scale.preview() else {
                    return Vec::new();
                };
                let mut segments = box_edges(bounds);
                if let Some(matrix) = self.scale.matrix() {
                    for &(a, b) in &self.scale_segments {
                        segments.push((matrix.transform_point3(a), matrix.transform_point3(b)));
                    }
                }
                segments
            }
            Tool::Tape => {
                if self.tape.stage() != TapeStage::End {
                    return Vec::new();
                }
                if self.tape.edge().is_none() {
                    return match (self.tape.start(), self.hint) {
                        (Some(start), Some(hint)) => vec![(start, hint.point)],
                        _ => Vec::new(),
                    };
                }
                let Some((a, b)) = self.tape.edge() else {
                    return Vec::new();
                };
                match self.tape.preview() {
                    Some(Guide::Line { point, dir }) => {
                        let onto = |p: DVec3| point + dir * dir.dot(p - point);
                        vec![(onto(a), onto(b))]
                    }
                    _ => Vec::new(),
                }
            }
            Tool::Orbit | Tool::Pan | Tool::Zoom | Tool::Select | Tool::Erase => Vec::new(),
        }
    }

    /// The plane the shape tool draws in (the next shape's while idle); `None` for other tools.
    pub fn draw_plane(&self) -> Option<DrawPlane> {
        match self.tool {
            Tool::Rectangle => Some(self.rect.plane()),
            Tool::Circle => Some(self.circle.plane()),
            Tool::Arc => Some(self.arc.plane()),
            Tool::Arc3 => Some(self.arc3.plane()),
            Tool::Rotate => Some(self.rotate.plane()),
            Tool::Orbit
            | Tool::Pan
            | Tool::Zoom
            | Tool::Select
            | Tool::Line
            | Tool::PushPull
            | Tool::Move
            | Tool::Scale
            | Tool::Tape
            | Tool::Erase => None,
        }
    }

    /// The axis the shape tool's plane normal is locked to (arrow keys), if any.
    pub fn plane_lock(&self) -> Option<usize> {
        if is_shape(self.tool) || self.tool == Tool::Rotate {
            self.plane_lock
        } else {
            None
        }
    }

    /// Push/Pull: the face under the cursor while idle, the face being pushed while a push
    /// is in progress (`docs/specs/m3-app-pushpull.md`). `None` for other tools.
    pub fn pushpull_face(&self) -> Option<FaceId> {
        if self.tool != Tool::PushPull {
            return None;
        }
        match self.push.as_ref() {
            Some(push) => Some(push.face),
            None => self.push_hover,
        }
    }

    /// Push/Pull: the signed distance along the face's front normal that the next click
    /// would use; `None` while no push is in progress or for other tools.
    pub fn pushpull_distance(&self) -> Option<f64> {
        if self.tool != Tool::PushPull {
            return None;
        }
        self.push.as_ref().map(|push| push.distance)
    }

    /// Rotate: the tool, for drawing the protractor (`docs/specs/m3-app-rotate.md`). `None`
    /// for other tools.
    pub fn rotate_tool(&self) -> Option<&RotateTool> {
        (self.tool == Tool::Rotate).then_some(&self.rotate)
    }

    /// Move: the tool, for drawing the from point (`docs/specs/m3-app-move.md`). `None` for
    /// other tools.
    pub fn move_tool(&self) -> Option<&MoveTool> {
        (self.tool == Tool::Move).then_some(&self.translate)
    }

    /// Tape: the tool, for drawing the start point (`docs/specs/m3-app-tape.md`). `None` for
    /// other tools.
    pub fn tape_tool(&self) -> Option<&TapeTool> {
        (self.tool == Tool::Tape).then_some(&self.tape)
    }

    /// Tape: the point-guide modifier (Ctrl in the window): while on, a click at the start
    /// stage leaves a guide point. Kept across `set_tool`.
    pub fn set_point_guides(&mut self, on: bool) {
        self.tape_point_guides = on;
    }

    /// Scale: the tool, for drawing the box and its handles (`docs/specs/m3-app-scale-flip.md`).
    /// `None` for other tools.
    pub fn scale_tool(&self) -> Option<&ScaleTool> {
        (self.tool == Tool::Scale).then_some(&self.scale)
    }

    /// Scale: the handle under the cursor while no scale is in progress.
    pub fn scale_hover(&self) -> Option<Handle> {
        if self.tool == Tool::Scale {
            self.scale_hover
        } else {
            None
        }
    }

    /// Flip the selection about the plane through the centre of its bounding box that is
    /// perpendicular to axis `axis` (0 red, 1 green, 2 blue). True when the model changed.
    pub fn flip(&mut self, axis: usize, model: &mut Model) -> bool {
        if axis >= 3 || self.scale.stage() == ScaleStage::Drag {
            return false;
        }
        if self.selection.is_empty() {
            self.message = Some("Select something to flip".to_string());
            return false;
        }
        let Some(bounds) = bounds(model, BodyId::Root, &self.selection) else {
            self.message = Some("Select something to flip".to_string());
            return false;
        };
        let center = bounds.center();
        let factors = match axis {
            0 => DVec3::new(-1.0, 1.0, 1.0),
            1 => DVec3::new(1.0, -1.0, 1.0),
            _ => DVec3::new(1.0, 1.0, -1.0),
        };
        let matrix = DMat4::from_translation(center)
            * DMat4::from_scale(factors)
            * DMat4::from_translation(-center);
        match transform::transform(
            model,
            BodyId::Root,
            &self.selection,
            matrix,
            &mut self.undo,
            "Flip",
        ) {
            Ok(()) => {
                self.message = None;
                self.prune(model);
                true
            }
            Err(_) => {
                self.message = Some("Can't flip that".to_string());
                false
            }
        }
    }

    /// Erase the selection (one undo step "Erase"). True when the model changed.
    /// Behaviour: `docs/specs/m3-app-erase.md`.
    pub fn erase_selection(&mut self, model: &mut Model) -> bool {
        self.line = LineTool::new();
        self.idle_shapes();
        self.idle_rotate();
        self.idle_move();
        self.idle_scale();
        self.idle_tape();
        self.typed.clear();
        self.push = None;
        self.hint = None;
        if self.selection.is_empty() {
            self.message = Some("Select something to erase".to_string());
            return false;
        }
        match erase(
            model,
            BodyId::Root,
            &self.selection,
            &mut self.undo,
            "Erase",
        ) {
            Ok(()) => {
                self.message = None;
                self.prune(model);
                true
            }
            Err(_) => {
                self.message = Some("Can't erase that".to_string());
                false
            }
        }
    }

    /// Erase tool: erase the entity under the cursor.
    fn click_erase(&mut self, model: &mut Model, view: &Viewport, cursor: DVec2) -> bool {
        let Some(entity) = self.pick_at(model, view, cursor) else {
            return false;
        };
        match erase(model, BodyId::Root, &[entity], &mut self.undo, "Erase") {
            Ok(()) => {
                self.message = None;
                self.prune(model);
                true
            }
            Err(_) => {
                self.message = Some("Can't erase that".to_string());
                false
            }
        }
    }

    /// Why the last input was refused, for the status bar.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// The face under the cursor at `cursor`, with where the ray met it and its front normal.
    fn face_under(
        &self,
        model: &Model,
        view: &Viewport,
        cursor: DVec2,
    ) -> Option<(FaceId, DVec3, DVec3)> {
        let (origin, dir) = view.ray(cursor);
        let ctx = model.root().geometry();
        let (face, t) = infer::ray_face(ctx, origin, dir)?;
        let normal = ctx.face_plane(face)?.normal;
        let start = origin + unit_or(dir, DVec3::Z) * t;
        Some((face, start, normal))
    }

    fn move_push(&mut self, model: &mut Model, view: &Viewport, cursor: DVec2) {
        let (origin, dir) = view.ray(cursor);
        self.push_hover =
            infer::ray_face(model.root().geometry(), origin, dir).map(|(face, _)| face);
        if self.push.is_none() {
            self.hint = None;
            return;
        }

        let inf = self.infer_at(model, view, cursor);
        let (snap, axis) = match inf {
            Some(inf) => (inf.snap, inf.axis),
            None => (Snap::Along, None),
        };
        let distance = {
            let push = self.push.as_ref().expect("pushing");
            match inf {
                Some(inf) if matches!(inf.snap, Snap::Endpoint(_) | Snap::Midpoint(_)) => {
                    Some((inf.point - push.start).dot(push.normal))
                }
                _ => {
                    let (origin, dir) = view.ray(cursor);
                    closest_on_line(push.start, push.normal, origin, dir)
                }
            }
        };
        if let Some(distance) = distance
            && let Some(push) = self.push.as_mut()
        {
            push.distance = distance;
        }

        let push = self.push.as_ref().expect("pushing");
        self.hint = Some(Inference {
            point: push.start + push.normal * push.distance,
            snap,
            axis,
        });
    }

    fn click_push(
        &mut self,
        model: &mut Model,
        view: &Viewport,
        cursor: DVec2,
        count: u32,
    ) -> bool {
        if count >= 2 {
            self.push = None;
            self.hint = None;
            let Some(last) = self.last_push else {
                return false;
            };
            let Some((face, _, _)) = self.face_under(model, view, cursor) else {
                return false;
            };
            return self.commit_push(model, face, last.distance);
        }

        if let Some(push) = self.push.as_ref() {
            let (face, distance) = (push.face, push.distance);
            if distance.abs() <= TOL {
                return false;
            }
            return self.commit_push(model, face, distance);
        }

        let Some((face, start, normal)) = self.face_under(model, view, cursor) else {
            return false;
        };
        let ctx = model.root().geometry();
        let Some(face_data) = ctx.face(face) else {
            return false;
        };
        let mut loops = vec![ctx.loop_points(&face_data.outer)];
        for inner in &face_data.inner {
            loops.push(ctx.loop_points(inner));
        }
        self.push = Some(PushState {
            face,
            start,
            normal,
            distance: 0.0,
            loops,
        });
        self.hint = Some(Inference {
            point: start,
            snap: Snap::OnFace(face),
            axis: None,
        });
        false
    }

    fn enter_push(&mut self, text: String, model: &mut Model) -> bool {
        let length = match units::parse_length(&text, self.unit) {
            Ok(length) => length,
            Err(_) => {
                self.message = Some("Not a length".to_string());
                return false;
            }
        };

        if let Some(push) = self.push.as_ref() {
            let direction = if push.distance < 0.0 { -1.0 } else { 1.0 };
            let flip = if length < 0.0 { -1.0 } else { 1.0 };
            return self.commit_push(model, push.face, length.abs() * direction * flip);
        }

        let Some(last) = self.last_push else {
            return false;
        };
        if self.undo.undo_count() != last.undo_count || self.undo.redo_count() != 0 {
            return false;
        }
        if self.undo.undo(model).ok().flatten().is_none() {
            return false;
        }

        let direction = if last.distance < 0.0 { -1.0 } else { 1.0 };
        let flip = if length < 0.0 { -1.0 } else { 1.0 };
        let distance = length.abs() * direction * flip;
        match push_pull(
            model,
            BodyId::Root,
            last.face,
            distance,
            &mut self.undo,
            "Push/Pull",
        ) {
            Ok(_) => {
                self.message = None;
                self.push = None;
                self.hint = None;
                self.last_push = Some(LastPush {
                    face: last.face,
                    distance,
                    undo_count: self.undo.undo_count(),
                });
                self.prune(model);
                true
            }
            Err(_) => {
                let _ = self.undo.redo(model);
                self.message = Some("Can't push/pull that".to_string());
                false
            }
        }
    }

    fn commit_push(&mut self, model: &mut Model, face: FaceId, distance: f64) -> bool {
        match push_pull(
            model,
            BodyId::Root,
            face,
            distance,
            &mut self.undo,
            "Push/Pull",
        ) {
            Ok(_) => {
                self.push = None;
                self.push_hover = None;
                self.hint = None;
                self.message = None;
                self.typed.clear();
                self.last_push = Some(LastPush {
                    face,
                    distance,
                    undo_count: self.undo.undo_count(),
                });
                self.prune(model);
                true
            }
            Err(_) => {
                self.message = Some("Can't push/pull that".to_string());
                false
            }
        }
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

    fn move_rotate(&mut self, model: &mut Model, view: &Viewport, cursor: DVec2) {
        let (origin, dir) = view.ray(cursor);
        if self.rotate.stage() == RotateStage::Center {
            self.pick_rotate_plane(model, origin, dir);
        }
        let Some(inf) = self.infer_rotate_at(model, view, cursor) else {
            self.hint = None;
            return;
        };
        let point = self.rotate_input_point(inf, origin, dir);
        self.hint = Some(Inference { point, ..inf });
        self.rotate.set_angle_snap(free_point(&inf));
        let mut target = Target {
            model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: self.unit,
        };
        let _ = self
            .rotate
            .handle(Input::Move(point), &self.selection, &mut target);
    }

    fn click_rotate(&mut self, model: &mut Model, view: &Viewport, cursor: DVec2) -> bool {
        let before = self.undo.undo_count();
        let was_center = self.rotate.stage() == RotateStage::Center;
        let (origin, dir) = view.ray(cursor);
        if was_center {
            self.pick_rotate_plane(model, origin, dir);
            if self.selection.is_empty() {
                let Some(entity) = self.pick_at(model, view, cursor) else {
                    self.message = Some("Select something to rotate".to_string());
                    return false;
                };
                self.selection = click::apply_click(
                    &self.selection,
                    Some(entity),
                    click_mods(Modifiers::default()),
                );
            }
        }
        let Some(inf) = self.infer_rotate_at(model, view, cursor) else {
            self.hint = None;
            return false;
        };
        let point = self.rotate_input_point(inf, origin, dir);
        self.hint = Some(Inference { point, ..inf });
        self.rotate.set_angle_snap(free_point(&inf));
        let result = {
            let mut target = Target {
                model,
                body: BodyId::Root,
                undo: &mut self.undo,
                unit: self.unit,
            };
            self.rotate
                .handle(Input::Click(point), &self.selection, &mut target)
        };
        match result {
            Ok(()) => {
                self.message = None;
                if was_center {
                    self.rotate_segments = self.collect_rotate_segments(model);
                }
            }
            Err(error) => self.message = Some(rotate_error_message(&error)),
        }
        let changed = self.undo.undo_count() != before;
        if changed {
            self.prune(model);
        }
        changed
    }

    fn enter_rotate(&mut self, text: String, model: &mut Model) -> bool {
        let before = self.undo.undo_count();
        let before_angle = self.rotate.last_angle(&self.undo);
        let result = {
            let mut target = Target {
                model,
                body: BodyId::Root,
                undo: &mut self.undo,
                unit: self.unit,
            };
            self.rotate
                .handle(Input::Typed(text), &self.selection, &mut target)
        };
        match result {
            Ok(()) => self.message = None,
            Err(error) => self.message = Some(rotate_error_message(&error)),
        }
        let after_angle = self.rotate.last_angle(&self.undo);
        let retyped = before_angle.zip(after_angle).is_some_and(|(a, b)| a != b);
        let changed = self.undo.undo_count() != before || retyped;
        if changed {
            self.prune(model);
        }
        changed
    }

    fn idle_rotate(&mut self) {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let mut target = Target {
            model: &mut model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: self.unit,
        };
        let _ = self.rotate.handle(Input::Cancel, &[], &mut target);
        self.rotate_segments.clear();
    }

    fn collect_rotate_segments(&self, model: &Model) -> Vec<(DVec3, DVec3)> {
        let ctx = model.root().geometry();
        let mut edges = BTreeSet::new();
        let mut instances = Vec::new();
        for entity in &self.selection {
            match *entity {
                Entity::Edge(id) => {
                    edges.insert(id);
                }
                Entity::Face(id) => {
                    let Some(face) = ctx.face(id) else {
                        continue;
                    };
                    for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
                        for u in &l.uses {
                            edges.insert(u.edge);
                        }
                    }
                }
                Entity::Instance(id) => {
                    instances.extend(select::instance_segments(model, BodyId::Root, id));
                }
            }
        }
        let mut segments = Vec::with_capacity(edges.len() + instances.len());
        for id in edges {
            let Some(edge) = ctx.edge(id) else {
                continue;
            };
            let (Some(a), Some(b)) = (ctx.vertex(edge.ends[0]), ctx.vertex(edge.ends[1])) else {
                continue;
            };
            segments.push((a.position, b.position));
        }
        segments.extend(instances);
        segments
    }

    fn move_scale(&mut self, model: &mut Model, view: &Viewport, cursor: DVec2, mods: Modifiers) {
        self.scale.set_about_center(mods.ctrl);
        self.scale.set_invert_uniform(mods.shift);
        if self.scale.stage() == ScaleStage::Pick {
            self.scale_hover = self.hovered_handle(model, view, cursor);
            self.hint = None;
            return;
        }
        self.scale_hover = None;
        let Some(inf) = self.infer_scale_at(model, view, cursor) else {
            self.hint = None;
            return;
        };
        self.hint = Some(inf);
        let mut target = Target {
            model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: self.unit,
        };
        let _ = self
            .scale
            .handle(Input::Move(inf.point), &self.selection, &mut target);
    }

    fn click_scale(
        &mut self,
        model: &mut Model,
        view: &Viewport,
        cursor: DVec2,
        mods: Modifiers,
    ) -> bool {
        self.scale.set_about_center(mods.ctrl);
        self.scale.set_invert_uniform(mods.shift);
        if self.scale.stage() == ScaleStage::Pick {
            if self.selection.is_empty() {
                self.message = Some("Select something to scale".to_string());
                return false;
            }
            let Some(handle) = self.hovered_handle(model, view, cursor) else {
                self.message = Some("Click a handle to scale".to_string());
                return false;
            };
            match self
                .scale
                .grab(handle, model, BodyId::Root, &self.selection)
            {
                Ok(()) => {
                    self.message = None;
                    self.scale_segments = self.collect_rotate_segments(model);
                    self.scale_hover = None;
                }
                Err(error) => self.message = Some(scale_error_message(&error).to_string()),
            }
            return false;
        }
        let Some(inf) = self.infer_scale_at(model, view, cursor) else {
            self.hint = None;
            return false;
        };
        self.hint = Some(inf);
        let before = self.undo.undo_count();
        let result = {
            let mut target = Target {
                model,
                body: BodyId::Root,
                undo: &mut self.undo,
                unit: self.unit,
            };
            self.scale
                .handle(Input::Click(inf.point), &self.selection, &mut target)
        };
        match result {
            Ok(()) => self.message = None,
            Err(error) => self.message = Some(scale_error_message(&error).to_string()),
        }
        let changed = self.undo.undo_count() != before;
        if changed {
            self.prune(model);
        }
        changed
    }

    fn enter_scale(&mut self, text: String, model: &mut Model) -> bool {
        let before = self.undo.undo_count();
        let before_factors = self.scale.last_factors(&self.undo);
        let result = {
            let mut target = Target {
                model,
                body: BodyId::Root,
                undo: &mut self.undo,
                unit: self.unit,
            };
            self.scale
                .handle(Input::Typed(text), &self.selection, &mut target)
        };
        match result {
            Ok(()) => self.message = None,
            Err(error) => self.message = Some(scale_error_message(&error).to_string()),
        }
        let after_factors = self.scale.last_factors(&self.undo);
        let retyped = before_factors
            .zip(after_factors)
            .is_some_and(|(a, b)| a != b);
        let changed = self.undo.undo_count() != before || retyped;
        if changed {
            self.prune(model);
        }
        changed
    }

    fn idle_scale(&mut self) {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let mut target = Target {
            model: &mut model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: self.unit,
        };
        let _ = self.scale.handle(Input::Cancel, &[], &mut target);
        self.scale_segments.clear();
    }

    fn hovered_handle(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Handle> {
        if self.selection.is_empty() {
            return None;
        }
        let bounds = bounds(model, BodyId::Root, &self.selection)?;
        let mut best: Option<(f64, Handle)> = None;
        for handle in handles(&bounds) {
            let Some(point) = view.project(bounds.point(handle)) else {
                continue;
            };
            let distance = point.distance(cursor);
            if distance <= SNAP_RADIUS_PX && best.is_none_or(|(nearest, _)| distance < nearest) {
                best = Some((distance, handle));
            }
        }
        best.map(|(_, handle)| handle)
    }

    fn infer_scale_at(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Inference> {
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
            anchor: self.scale.fixed_point(),
            axes: AXES,
            centers: &centers,
            lock: None,
        };
        infer::infer_model(model, &probe)
    }

    fn move_move(&mut self, model: &mut Model, view: &Viewport, cursor: DVec2) {
        let Some(inf) = self.infer_move_at(model, view, cursor) else {
            self.hint = None;
            return;
        };
        self.hint = Some(inf);
        let mut target = Target {
            model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: self.unit,
        };
        let _ = self
            .translate
            .handle(Input::Move(inf.point), &self.selection, &mut target);
    }

    fn click_move(&mut self, model: &mut Model, view: &Viewport, cursor: DVec2) -> bool {
        let before = self.undo.undo_count();
        let was_from = self.translate.stage() == MoveStage::From;
        if was_from && self.selection.is_empty() {
            let Some(entity) = self.pick_at(model, view, cursor) else {
                self.message = Some("Select something to move".to_string());
                return false;
            };
            self.selection = click::apply_click(
                &self.selection,
                Some(entity),
                click_mods(Modifiers::default()),
            );
        }
        let Some(inf) = self.infer_move_at(model, view, cursor) else {
            self.hint = None;
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
            self.translate
                .handle(Input::Click(inf.point), &self.selection, &mut target)
        };
        match result {
            Ok(()) => {
                self.message = None;
                if was_from {
                    self.move_segments = self.collect_rotate_segments(model);
                }
            }
            Err(error) => self.message = Some(move_error_message(&error).to_string()),
        }
        let changed = self.undo.undo_count() != before;
        if changed {
            self.prune(model);
        }
        changed
    }

    fn enter_move(&mut self, text: String, model: &mut Model) -> bool {
        let before = self.undo.undo_count();
        let before_offset = self.translate.last_offset(&self.undo);
        let result = {
            let mut target = Target {
                model,
                body: BodyId::Root,
                undo: &mut self.undo,
                unit: self.unit,
            };
            self.translate
                .handle(Input::Typed(text), &self.selection, &mut target)
        };
        match result {
            Ok(()) => self.message = None,
            Err(error) => self.message = Some(move_error_message(&error).to_string()),
        }
        let after_offset = self.translate.last_offset(&self.undo);
        let retyped = before_offset.zip(after_offset).is_some_and(|(a, b)| a != b);
        let changed = self.undo.undo_count() != before || retyped;
        if changed {
            self.prune(model);
        }
        changed
    }

    fn idle_move(&mut self) {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let mut target = Target {
            model: &mut model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: self.unit,
        };
        let _ = self.translate.handle(Input::Cancel, &[], &mut target);
        self.move_segments.clear();
    }

    fn move_tape(&mut self, model: &mut Model, view: &Viewport, cursor: DVec2) {
        self.refresh_hover_edge(model, view, cursor);
        let Some(inf) = self.infer_tape_at(model, view, cursor) else {
            self.hint = None;
            return;
        };
        self.hint = Some(inf);
        let mut target = Target {
            model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: self.unit,
        };
        let _ = self.tape.handle(Input::Move(inf.point), &mut target);
    }

    fn click_tape(&mut self, model: &mut Model, view: &Viewport, cursor: DVec2) -> bool {
        let before = self.undo.undo_count();
        self.refresh_hover_edge(model, view, cursor);
        self.tape.set_point_guides(self.tape_point_guides);
        let Some(inf) = self.infer_tape_at(model, view, cursor) else {
            self.hint = None;
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
            self.tape.handle(Input::Click(inf.point), &mut target)
        };
        match result {
            Ok(()) => self.message = None,
            Err(error) => self.message = Some(tape_error_message(&error).to_string()),
        }
        self.undo.undo_count() != before
    }

    fn enter_tape(&mut self, text: String, model: &mut Model) -> bool {
        let before = self.undo.undo_count();
        let result = {
            let mut target = Target {
                model,
                body: BodyId::Root,
                undo: &mut self.undo,
                unit: self.unit,
            };
            self.tape.handle(Input::Typed(text), &mut target)
        };
        match result {
            Ok(()) => self.message = None,
            Err(error) => self.message = Some(tape_error_message(&error).to_string()),
        }
        self.undo.undo_count() != before
    }

    fn idle_tape(&mut self) {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let mut target = Target {
            model: &mut model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: self.unit,
        };
        let _ = self.tape.handle(Input::Cancel, &mut target);
    }

    fn refresh_hover_edge(&mut self, model: &Model, view: &Viewport, cursor: DVec2) {
        if self.tape.stage() != TapeStage::Start {
            return;
        }
        let edge = match self.pick_at(model, view, cursor) {
            Some(Entity::Edge(id)) => {
                let ctx = model.root().geometry();
                ctx.edge(id).and_then(|edge| {
                    let a = ctx.vertex(edge.ends[0])?;
                    let b = ctx.vertex(edge.ends[1])?;
                    Some((a.position, b.position))
                })
            }
            _ => None,
        };
        self.tape.set_hover_edge(edge);
    }

    fn infer_tape_at(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Inference> {
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
            anchor: self.tape.start(),
            axes: AXES,
            centers: &centers,
            lock: None,
        };
        infer::infer_model(model, &probe)
    }

    fn pick_shape_plane(&mut self, model: &Model, origin: DVec3, dir: DVec3) {
        let plane = self.picked_plane(model, origin, dir);
        self.set_shape_plane(plane);
    }

    fn pick_rotate_plane(&mut self, model: &Model, origin: DVec3, dir: DVec3) {
        let plane = self.picked_plane(model, origin, dir);
        self.rotate.set_plane(plane);
    }

    fn picked_plane(&self, model: &Model, origin: DVec3, dir: DVec3) -> DrawPlane {
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
        plane_for_normal(unit_or(n, DVec3::Z))
    }

    fn plane_input_point(
        inf: Inference,
        origin: DVec3,
        dir: DVec3,
        anchor: DVec3,
        plane: DrawPlane,
    ) -> DVec3 {
        if matches!(inf.snap, Snap::OnFace(_) | Snap::Ground | Snap::Along) {
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

    fn shape_input_point(&self, inf: Inference, origin: DVec3, dir: DVec3) -> DVec3 {
        if !self.shape_idle()
            && let (Some(anchor), Some(plane)) = (self.shape_anchor(), self.shape_plane())
        {
            return Self::plane_input_point(inf, origin, dir, anchor, plane);
        }
        inf.point
    }

    fn rotate_input_point(&self, inf: Inference, origin: DVec3, dir: DVec3) -> DVec3 {
        if let Some(center) = self.rotate.center() {
            return Self::plane_input_point(inf, origin, dir, center, self.rotate.plane());
        }
        inf.point
    }

    fn infer_rotate_at(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Inference> {
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
            anchor: self.rotate.center(),
            axes: AXES,
            centers: &centers,
            lock: None,
        };
        infer::infer_model(model, &probe)
    }

    fn infer_move_at(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Inference> {
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
            anchor: self.translate.from(),
            axes: AXES,
            centers: &centers,
            lock: None,
        };
        infer::infer_model(model, &probe)
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
