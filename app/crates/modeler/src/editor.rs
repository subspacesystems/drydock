// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Drawing and selection in the viewport: runs inference and the kernel's tools on the
//! model's root body, and keeps the selection, the undo stack and the typed text of the
//! measurement box. No egui, no GPU: the window code feeds it pointer and key events.
//!
//! Behaviour: `docs/specs/m2-app-editor.md`, shape tools: `docs/specs/m2-app-shapes.md`,
//! Push/Pull: `docs/specs/m3-app-pushpull.md`, Rotate: `docs/specs/m3-app-rotate.md`,
//! Move: `docs/specs/m3-app-move.md`, Scale and Flip: `docs/specs/m3-app-scale-flip.md`,
//! Tape: `docs/specs/m3-app-tape.md`, Paint Bucket: `docs/specs/m4-app-paint.md`.

use std::collections::BTreeSet;

use glam::{DAffine2, DMat4, DVec2, DVec3};
use modeler_kernel::click::{self, ClickMods};
use modeler_kernel::component::make_component;
use modeler_kernel::copy::{self, Clipboard, copy_selection};
use modeler_kernel::editpath::EditPath;
use modeler_kernel::erase::erase;
use modeler_kernel::explode::explode;
use modeler_kernel::infer::{self, Inference, Probe, Snap};
use modeler_kernel::model::{
    BodyId, DefinitionInfo, InstanceId, Material, MaterialId, Model, UndoStack,
};
use modeler_kernel::paint::{self, Side};
use modeler_kernel::pushpull::push_pull;
use modeler_kernel::select::{self, BoxMode, Entity};
use modeler_kernel::tape::Guide;
use modeler_kernel::texture::{self, TextureMap};
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
use crate::texture_handles::{GRIP_PX, Grip, TextureHandles, plane_point_at, tile_corners};

/// A change to how a texture lies on a face, in texture space (meters).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextureOp {
    /// Slide the image by this much.
    Move(DVec2),
    /// Turn the image by `angle` radians (counter-clockwise on the image) about `about`.
    Rotate { angle: f64, about: DVec2 },
    /// Stretch the image by `factor` (per axis, > 0) about `about`.
    Scale { factor: DVec2, about: DVec2 },
    /// Back to the default placement.
    Reset,
}

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

#[derive(Clone, Copy)]
struct PasteState {
    point: Option<DVec3>,
}

/// The texture handles mode: the face and side they act on, the map at grab time, the
/// grabbed image copy's texture-space pin `base`, and the last valid preview map.
#[derive(Clone, Debug)]
struct TextureHandleState {
    face: FaceId,
    side: Side,
    map: TextureMap,
    base: DVec2,
    size: DVec2,
    plane: modeler_kernel::geom::Plane,
    hover: Option<Grip>,
    held: Option<Grip>,
    /// The plane point under the click that grabbed a Slide grip.
    grab: Option<DVec3>,
    preview: TextureMap,
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
    move_copy: bool,
    move_segments: Vec<(DVec3, DVec3)>,
    scale: ScaleTool,
    scale_hover: Option<Handle>,
    tape: TapeTool,
    tape_point_guides: bool,
    scale_segments: Vec<(DVec3, DVec3)>,
    material: Option<MaterialId>,
    clipboard: Option<Clipboard>,
    paste: Option<PasteState>,
    texture_handles: Option<TextureHandleState>,
    path: EditPath,
    /// Cumulative world transforms of the path's steps; the last one maps the open body to
    /// the world. Kept next to the path so the getters can map points without the model.
    transforms: Vec<DMat4>,
    hide_rest: bool,
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

/// The copied entities that keep the kinds of the selection they came from. A face copy
/// also brings its boundary edges out of the copy, but selecting those too would grow the
/// selection beyond what the user had chosen.
fn copied_selection(original: &[Entity], copied: Vec<Entity>) -> Vec<Entity> {
    copied
        .into_iter()
        .filter(|entity| {
            original
                .iter()
                .any(|old| std::mem::discriminant(old) == std::mem::discriminant(entity))
        })
        .collect()
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
            move_copy: false,
            move_segments: Vec::new(),
            scale: ScaleTool::new(),
            scale_hover: None,
            tape: TapeTool::new(),
            tape_point_guides: false,
            scale_segments: Vec::new(),
            material: None,
            clipboard: None,
            paste: None,
            texture_handles: None,
            path: EditPath::root(),
            transforms: Vec::new(),
            hide_rest: false,
        }
    }

    /// The open body: `path`'s body, or the root when the path doesn't resolve.
    fn body(&self, model: &Model) -> BodyId {
        self.path.body(model).unwrap_or(BodyId::Root)
    }

    /// The open body's world transform (identity at the root).
    fn world(&self) -> DMat4 {
        self.transforms.last().copied().unwrap_or(DMat4::IDENTITY)
    }

    fn to_world(&self, point: DVec3) -> DVec3 {
        self.world().transform_point3(point)
    }

    fn to_body(&self, point: DVec3) -> DVec3 {
        self.world().inverse().transform_point3(point)
    }

    fn dir_to_world(&self, dir: DVec3) -> DVec3 {
        self.world().transform_vector3(dir)
    }

    fn dir_to_body(&self, dir: DVec3) -> DVec3 {
        self.world().inverse().transform_vector3(dir)
    }

    fn body_axes(&self) -> [DVec3; 3] {
        AXES.map(|axis| unit_or(self.dir_to_body(axis), axis))
    }

    /// The cursor ray in the open body's coordinates.
    fn body_ray(&self, view: &Viewport, cursor: DVec2) -> (DVec3, DVec3) {
        let (origin, dir) = view.ray(cursor);
        (self.to_body(origin), unit_or(self.dir_to_body(dir), dir))
    }

    /// Rebuild the cached world transforms by walking the path from the root.
    fn recompute_transforms(&mut self, model: &Model) {
        self.transforms.clear();
        let mut body = BodyId::Root;
        let mut transform = DMat4::IDENTITY;
        for &step in &self.path.steps {
            let Some(instance) = model.body(body).and_then(|b| b.instance(step)) else {
                break;
            };
            transform *= instance.transform;
            self.transforms.push(transform);
            body = BodyId::Definition(instance.definition);
        }
    }

    /// Drop steps that no longer resolve; clear the selection when the path got shorter.
    fn refresh_path(&mut self, model: &Model) {
        let prefix = self.path.valid_prefix(model);
        let shortened = prefix.steps.len() < self.path.steps.len();
        self.path = prefix;
        self.recompute_transforms(model);
        if shortened {
            self.selection.clear();
        }
    }

    fn infer_with(
        &self,
        model: &Model,
        view: &Viewport,
        cursor: DVec2,
        anchor: Option<DVec3>,
        lock: Option<DVec3>,
    ) -> Option<Inference> {
        let (origin, dir) = self.body_ray(view, cursor);
        let project = |p: DVec3| view.project(self.to_world(p));
        let centers: [DVec3; 0] = [];
        let axes = self.body_axes();
        let probe = Probe {
            cursor,
            origin,
            dir,
            parallel: view.camera.projection == Projection::Parallel,
            project: &project,
            radius: SNAP_RADIUS_PX,
            anchor,
            axes,
            centers: &centers,
            lock,
        };
        infer::infer_in(model, &self.path, !self.hide_rest, &probe)
    }

    fn pick_with(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Entity> {
        let (origin, dir) = self.body_ray(view, cursor);
        let project = |p: DVec3| view.project(self.to_world(p));
        let centers: [DVec3; 0] = [];
        let axes = self.body_axes();
        let probe = Probe {
            cursor,
            origin,
            dir,
            parallel: view.camera.projection == Projection::Parallel,
            project: &project,
            radius: SNAP_RADIUS_PX,
            anchor: self.line.anchor(),
            axes,
            centers: &centers,
            lock: self.lock_body(),
        };
        infer::pick_in(model, &self.path, !self.hide_rest, &probe)
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Switch tools; the shape in progress, the typed text and the locks are dropped.
    pub fn set_tool(&mut self, tool: Tool) {
        self.end_texture_handles();
        self.tool = tool;
        self.paste = None;
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
        self.move_copy = false;
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
        self.refresh_path(model);
        if self.texture_handles.is_some() {
            if self
                .texture_handles
                .as_ref()
                .is_some_and(|state| state.held.is_some())
            {
                if let Some(op) = self.held_texture_op(view, cursor, mods) {
                    self.preview_texture_op(op);
                }
            } else {
                let hover = self.texture_grip_at(model, view, cursor);
                if let Some(state) = self.texture_handles.as_mut() {
                    state.hover = hover;
                }
            }
            self.hint = None;
            return;
        }
        if self.paste.is_some() {
            if let Some(inf) = self.infer_move_at(model, view, cursor) {
                self.hint = Some(inf);
                if let Some(paste) = self.paste.as_mut() {
                    paste.point = Some(inf.point);
                }
            }
            return;
        }
        if is_shape(self.tool) {
            let (origin, dir) = self.body_ray(view, cursor);
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
                let body = self.body(model);
                let mut target = Target {
                    model,
                    body,
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
        self.refresh_path(model);
        if self.texture_handles.is_some() {
            return self.click_texture_handles(model, view, cursor, mods);
        }
        if self.paste.is_some() {
            return self.click_paste(model, view, cursor);
        }
        match self.tool {
            Tool::Line => {
                let before = self.undo.undo_count();
                let Some(inf) = self.infer_at(model, view, cursor) else {
                    return false;
                };
                self.hint = Some(inf);
                let result = {
                    let body = self.body(model);
                    let mut target = Target {
                        model,
                        body,
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
                if count == 2
                    && let Some(Entity::Instance(instance)) = hit
                {
                    self.open_instance(model, instance);
                    return false;
                }
                if count == 2 && hit.is_none() && !self.path.is_root() {
                    self.close_path();
                    return false;
                }
                let mods = click_mods(mods);
                self.selection = match hit {
                    Some(entity) if count >= 2 => {
                        let body = self.body(model);
                        let ctx = model
                            .body(body)
                            .map(|body| body.geometry())
                            .unwrap_or(model.root().geometry());
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
            Tool::Paint => self.click_paint(model, view, cursor, mods),
            Tool::Rectangle | Tool::Circle | Tool::Arc | Tool::Arc3 => {
                let before = self.undo.undo_count();
                let (origin, dir) = self.body_ray(view, cursor);
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
        self.refresh_path(model);
        if self.tool != Tool::Select {
            return;
        }
        let project = |p: DVec3| view.project(self.to_world(p));
        let picked = select::box_select(
            model,
            self.body(model),
            &project,
            start,
            end,
            BoxMode::from_drag(start, end),
        );
        self.selection = click::apply(&self.selection, &picked, click_mods(mods));
    }

    /// Typed characters for the measurement box. True when they were taken.
    pub fn text(&mut self, text: &str) -> bool {
        if self.texture_handles.is_some() {
            return false;
        }
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
        self.refresh_path(model);
        if self.texture_handles.is_some() {
            match key {
                EditKey::Cancel => {
                    let held = self
                        .texture_handles
                        .as_ref()
                        .is_some_and(|state| state.held.is_some());
                    if held {
                        if let Some(state) = self.texture_handles.as_mut() {
                            state.held = None;
                            state.grab = None;
                            state.hover = None;
                        }
                    } else {
                        self.end_texture_handles();
                    }
                    return false;
                }
                EditKey::Enter => {
                    self.end_texture_handles();
                    return false;
                }
                EditKey::Undo | EditKey::Redo => {
                    if let Some(state) = self.texture_handles.as_mut() {
                        state.held = None;
                        state.grab = None;
                        state.hover = None;
                    }
                }
                _ => {}
            }
        }
        match key {
            EditKey::Cancel => {
                if self.paste.take().is_some() {
                    self.hint = None;
                    return false;
                }
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
                if matches!(
                    self.tool,
                    Tool::Select | Tool::Orbit | Tool::Pan | Tool::Zoom
                ) && !self.path.is_root()
                {
                    self.close_path();
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
                    let body = self.body(model);
                    let mut target = Target {
                        model,
                        body,
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
        let scale = self.world().to_scale_rotation_translation().0;
        let factor = scale.length() / 3f64.sqrt();
        let length = |value: f64| units::format_length(value * factor, self.unit, 2);
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
            Tool::Orbit | Tool::Pan | Tool::Zoom | Tool::Select | Tool::Erase | Tool::Paint => None,
        }
    }

    /// The last inferred point, for the hint.
    pub fn hint(&self) -> Option<Inference> {
        self.hint.map(|hint| Inference {
            point: self.to_world(hint.point),
            ..hint
        })
    }

    /// The direction the drawing is locked to, if any.
    pub fn lock(&self) -> Option<DVec3> {
        self.lock_body().map(|dir| self.dir_to_world(dir))
    }

    fn lock_body(&self) -> Option<DVec3> {
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
        self.preview_body()
            .into_iter()
            .map(|(a, b)| (self.to_world(a), self.to_world(b)))
            .collect()
    }

    fn preview_body(&self) -> Vec<(DVec3, DVec3)> {
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
            Tool::Orbit | Tool::Pan | Tool::Zoom | Tool::Select | Tool::Erase | Tool::Paint => {
                Vec::new()
            }
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
            | Tool::Erase
            | Tool::Paint => None,
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

    /// Whether the Move tool copies instead of moving (`docs/specs/m4-app-move-copy.md`).
    pub fn move_copy(&self) -> bool {
        self.move_copy
    }

    /// Turn Move's copy mode on or off. Switching tools turns it off.
    pub fn set_move_copy(&mut self, on: bool) {
        self.move_copy = on;
    }

    /// Drop the tool in progress and the typed text, as the editing commands do.
    fn idle_edit(&mut self) {
        self.line = LineTool::new();
        self.idle_shapes();
        self.idle_rotate();
        self.idle_move();
        self.idle_scale();
        self.idle_tape();
        self.typed.clear();
        self.push = None;
        self.hint = None;
    }

    /// The clipboard, if something was copied or cut. Behaviour:
    /// `docs/specs/m4-app-copy-paste.md`.
    pub fn clipboard(&self) -> Option<&Clipboard> {
        self.clipboard.as_ref()
    }

    /// Copy the selection of the open body to the clipboard. The model is untouched.
    /// True when the clipboard was set.
    pub fn copy(&mut self, model: &Model) -> bool {
        self.idle_edit();
        if self.selection.is_empty() {
            self.message = Some("Select something to copy".to_string());
            return false;
        }
        match copy::extract(model, self.body(model), &self.selection) {
            Ok(clipboard) => {
                self.clipboard = Some(clipboard);
                self.message = None;
                true
            }
            Err(_) => {
                self.message = Some("Can't copy that".to_string());
                false
            }
        }
    }

    /// Copy the selection to the clipboard, then erase it (one undo step "Cut"). True when
    /// the model changed.
    pub fn cut(&mut self, model: &mut Model) -> bool {
        self.idle_edit();
        if self.selection.is_empty() {
            self.message = Some("Select something to cut".to_string());
            return false;
        }
        let body = self.body(model);
        let clipboard = match copy::extract(model, body, &self.selection) {
            Ok(clipboard) => clipboard,
            Err(_) => {
                self.message = Some("Can't cut that".to_string());
                return false;
            }
        };
        match erase(model, body, &self.selection, &mut self.undo, "Cut") {
            Ok(()) => {
                self.clipboard = Some(clipboard);
                self.message = None;
                self.prune(model);
                true
            }
            Err(_) => {
                self.message = Some("Can't cut that".to_string());
                false
            }
        }
    }

    /// Start pasting the clipboard: the Select tool is made current and the copy follows
    /// the cursor until a click places it. True when pasting started.
    pub fn paste(&mut self, model: &Model) -> bool {
        let _ = model;
        self.idle_edit();
        let available = self
            .clipboard
            .as_ref()
            .is_some_and(|clipboard| !clipboard.is_empty());
        if !available {
            self.message = Some("Nothing to paste".to_string());
            return false;
        }
        self.set_tool(Tool::Select);
        self.paste = Some(PasteState { point: None });
        self.message = None;
        true
    }

    /// Whether a paste is waiting for its click.
    pub fn pasting(&self) -> bool {
        self.paste.is_some()
    }

    /// The clipboard's x/y middle at its lowest z: where a paste is held by the cursor.
    fn paste_anchor(&self) -> Option<DVec3> {
        let (min, max) = self.clipboard.as_ref()?.bounds()?;
        Some(DVec3::new(
            (min.x + max.x) * 0.5,
            (min.y + max.y) * 0.5,
            min.z,
        ))
    }

    fn paste_offset(&self, point: DVec3) -> Option<DVec3> {
        Some(point - self.paste_anchor()?)
    }

    /// Edges of the paste as a click would place it now (world coordinates); empty when
    /// not pasting or the cursor has not been over the viewport yet.
    pub fn paste_preview(&self) -> Vec<(DVec3, DVec3)> {
        let Some(paste) = self.paste else {
            return Vec::new();
        };
        let Some(point) = paste.point else {
            return Vec::new();
        };
        let Some(offset) = self.paste_offset(point) else {
            return Vec::new();
        };
        let Some(clipboard) = &self.clipboard else {
            return Vec::new();
        };
        let root = clipboard.model().root();
        let edges: Vec<(DVec3, DVec3)> = if root.instances().next().is_some() {
            let Some((min, max)) = clipboard.bounds() else {
                return Vec::new();
            };
            box_edges(Bounds { min, max })
        } else {
            root.geometry()
                .edges()
                .filter_map(|(_, edge)| {
                    let a = root.geometry().vertex(edge.ends[0])?.position;
                    let b = root.geometry().vertex(edge.ends[1])?.position;
                    Some((a, b))
                })
                .collect()
        };
        edges
            .into_iter()
            .map(|(a, b)| (self.to_world(a + offset), self.to_world(b + offset)))
            .collect()
    }

    fn click_paste(&mut self, model: &mut Model, view: &Viewport, cursor: DVec2) -> bool {
        let Some(inf) = self.infer_move_at(model, view, cursor) else {
            return false;
        };
        self.hint = Some(inf);
        if let Some(paste) = self.paste.as_mut() {
            paste.point = Some(inf.point);
        }
        let Some(offset) = self.paste_offset(inf.point) else {
            return false;
        };
        let body = self.body(model);
        let Some(clipboard) = self.clipboard.as_ref() else {
            return false;
        };
        let matrix = DMat4::from_translation(offset);
        match copy::insert(model, body, clipboard, matrix, &mut self.undo, "Paste") {
            Ok(entities) => {
                self.selection = entities;
                self.paste = None;
                self.message = None;
                true
            }
            Err(_) => {
                self.message = Some("Can't paste that".to_string());
                false
            }
        }
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
        let Some(bounds) = bounds(model, self.body(model), &self.selection) else {
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
            self.body(model),
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
        self.end_texture_handles();
        self.paste = None;
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
            self.body(model),
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
        match erase(model, self.body(model), &[entity], &mut self.undo, "Erase") {
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

    /// The instances opened for editing (empty: the root is open). Behaviour:
    /// `docs/specs/m4-app-edit-context.md`.
    pub fn path(&self) -> &EditPath {
        &self.path
    }

    /// Open `instance`, an instance of the open body, for editing. False (nothing changes)
    /// when the open body has no such instance.
    pub fn open_instance(&mut self, model: &Model, instance: InstanceId) -> bool {
        let Some(path) = self.path.open(model, instance) else {
            return false;
        };
        self.end_texture_handles();
        self.paste = None;
        self.path = path;
        self.recompute_transforms(model);
        self.selection.clear();
        true
    }

    /// Close the innermost open instance; the root stays open.
    pub fn close_path(&mut self) {
        self.end_texture_handles();
        if self.path.is_root() {
            return;
        }
        self.paste = None;
        self.path = self.path.close();
        self.transforms.pop();
        self.selection.clear();
    }

    /// Whether the rest of the model is ignored (not snapped to, not picked) while an
    /// instance is open.
    pub fn hide_rest(&self) -> bool {
        self.hide_rest
    }

    pub fn set_hide_rest(&mut self, hide: bool) {
        self.hide_rest = hide;
    }

    /// Make the selection (in the open body) into a new component, or a group when `group`,
    /// named `name` (made unique; blank gets a default). One undo step "Make Component" /
    /// "Make Group". The new instance becomes the selection. True when the model changed.
    /// Behaviour: `docs/specs/m4-app-components.md`.
    pub fn make_component(&mut self, model: &mut Model, name: &str, group: bool) -> bool {
        self.end_texture_handles();
        self.paste = None;
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
            self.message = Some(
                if group {
                    "Select something to group"
                } else {
                    "Select something to make a component of"
                }
                .to_string(),
            );
            return false;
        }
        let info = DefinitionInfo {
            name: name.to_string(),
            group,
        };
        let label = if group {
            "Make Group"
        } else {
            "Make Component"
        };
        match make_component(
            model,
            self.body(model),
            &self.selection,
            info,
            &mut self.undo,
            label,
        ) {
            Ok(instance) => {
                self.selection = vec![Entity::Instance(instance)];
                self.message = None;
                true
            }
            Err(_) => {
                self.message = Some(
                    if group {
                        "Can't group that"
                    } else {
                        "Can't make that a component"
                    }
                    .to_string(),
                );
                false
            }
        }
    }

    /// Explode the selected instance into the open body. One undo step "Explode"; the
    /// entities that came out become the selection. True when the model changed.
    /// Behaviour: `docs/specs/m4-app-components.md`.
    pub fn explode_selection(&mut self, model: &mut Model) -> bool {
        self.end_texture_handles();
        self.paste = None;
        self.line = LineTool::new();
        self.idle_shapes();
        self.idle_rotate();
        self.idle_move();
        self.idle_scale();
        self.idle_tape();
        self.typed.clear();
        self.push = None;
        self.hint = None;
        let instance = match self.selection.as_slice() {
            [Entity::Instance(instance)] => *instance,
            _ => {
                self.message = Some("Select one group or component to explode".to_string());
                return false;
            }
        };
        match explode(model, self.body(model), instance, &mut self.undo, "Explode") {
            Ok(entities) => {
                self.selection = entities;
                self.message = None;
                true
            }
            Err(_) => {
                self.message = Some("Can't explode that".to_string());
                false
            }
        }
    }

    /// Add `material` (name made unique) as one undo step "New Material" and make it the
    /// Paint Bucket's current material. `None` with a message when the kernel refuses.
    /// Behaviour: `docs/specs/m4-app-materials.md`.
    pub fn create_material(&mut self, model: &mut Model, material: Material) -> Option<MaterialId> {
        match paint::create_material(model, material, &mut self.undo, "New Material") {
            Ok(id) => {
                self.material = Some(id);
                self.message = None;
                Some(id)
            }
            Err(_) => {
                self.message = Some("Can't make that material".to_string());
                None
            }
        }
    }

    /// Replace material `id` as one undo step "Edit Material". True when accepted (also
    /// when nothing changed); false with a message when refused.
    /// Behaviour: `docs/specs/m4-app-materials.md`.
    pub fn edit_material(&mut self, model: &mut Model, id: MaterialId, material: Material) -> bool {
        match paint::edit_material(model, id, material, &mut self.undo, "Edit Material") {
            Ok(()) => {
                self.message = None;
                true
            }
            Err(_) => {
                self.message = Some("Can't change that material".to_string());
                false
            }
        }
    }

    /// Remove materials nothing uses as one undo step "Purge Materials"; resets the current
    /// material if it went. Returns how many were removed.
    /// Behaviour: `docs/specs/m4-app-materials.md`.
    pub fn purge_materials(&mut self, model: &mut Model) -> usize {
        match paint::purge_materials(model, &mut self.undo, "Purge Materials") {
            Ok(removed) => {
                if removed.is_empty() {
                    self.message = Some("No unused materials".to_string());
                    return 0;
                }
                if let Some(current) = self.material
                    && removed.contains(&current)
                {
                    self.material = None;
                }
                self.message = None;
                removed.len()
            }
            Err(_) => {
                self.message = Some("Can't purge materials".to_string());
                0
            }
        }
    }

    /// Position the texture on `side` of `face` in the open body: apply `op` to the map in
    /// use there. One undo step "Position Texture" ("Reset Texture" for `Reset`). True when
    /// the model changed. Behaviour: `docs/specs/m4-app-texture-position.md`.
    pub fn position_texture(
        &mut self,
        model: &mut Model,
        face: FaceId,
        side: Side,
        op: TextureOp,
    ) -> bool {
        let body = self.body(model);
        let materials = model
            .body(body)
            .filter(|body| body.geometry().face(face).is_some())
            .map(|body| body.face_materials(face));
        let stored = materials.and_then(|materials| match side {
            Side::Front => materials.front_map,
            Side::Back => materials.back_map,
        });
        let material = materials.and_then(|materials| match side {
            Side::Front => materials.front,
            Side::Back => materials.back,
        });
        let textured = material
            .and_then(|id| model.material(id))
            .is_some_and(|material| material.texture.is_some());
        if !textured {
            self.message = Some("That side has no texture".to_string());
            return false;
        }

        let finite = match op {
            TextureOp::Move(d) => d.is_finite(),
            TextureOp::Rotate { angle, about } => angle.is_finite() && about.is_finite(),
            TextureOp::Scale { factor, about } => {
                factor.is_finite() && factor.x > 0.0 && factor.y > 0.0 && about.is_finite()
            }
            TextureOp::Reset => true,
        };
        if !finite {
            self.message = Some("Can't position that texture".to_string());
            return false;
        }

        let Some(current) = texture::side_map(model, body, face, side) else {
            self.message = Some("Can't position that texture".to_string());
            return false;
        };
        let (map, label) = match op {
            TextureOp::Move(d) => (
                Some(current.then(DAffine2::from_translation(d))),
                "Position Texture",
            ),
            TextureOp::Rotate { angle, about } => {
                let affine = DAffine2::from_translation(about)
                    * DAffine2::from_angle(angle)
                    * DAffine2::from_translation(-about);
                (Some(current.then(affine)), "Position Texture")
            }
            TextureOp::Scale { factor, about } => {
                let affine = DAffine2::from_translation(about)
                    * DAffine2::from_scale(factor)
                    * DAffine2::from_translation(-about);
                (Some(current.then(affine)), "Position Texture")
            }
            TextureOp::Reset => (None, "Reset Texture"),
        };
        if stored == map || (stored.is_none() && map == Some(current)) {
            self.message = None;
            return false;
        }
        match texture::set_texture_maps(model, body, &[(face, side, map)], &mut self.undo, label) {
            Ok(changed) => {
                self.message = None;
                changed > 0
            }
            Err(_) => {
                self.message = Some("Can't position that texture".to_string());
                false
            }
        }
    }

    /// The texture size on `side` of `face` when that side has a material with a texture.
    fn textured_size(
        &self,
        model: &Model,
        body: BodyId,
        face: FaceId,
        side: Side,
    ) -> Option<DVec2> {
        let body = model.body(body)?;
        body.geometry().face(face)?;
        let materials = body.face_materials(face);
        let material = match side {
            Side::Front => materials.front,
            Side::Back => materials.back,
        }?;
        model.material(material)?.texture.as_ref().map(|t| t.size)
    }

    /// The current map, size, plane and image copy corners of the handles, or `None` when
    /// the face or its texture is gone.
    fn texture_tile(
        &self,
        model: &Model,
    ) -> Option<(TextureMap, DVec2, modeler_kernel::geom::Plane, [DVec3; 4])> {
        let state = self.texture_handles.as_ref()?;
        let body = self.body(model);
        let size = self.textured_size(model, body, state.face, state.side)?;
        let map = texture::side_map(model, body, state.face, state.side)?;
        let geometry = model.body(body)?.geometry();
        let face = geometry.face(state.face)?;
        let plane = geometry.face_plane(state.face)?;
        let points = geometry.loop_points(&face.outer);
        if points.is_empty() {
            return None;
        }
        let center = points.iter().copied().sum::<DVec3>() / points.len() as f64;
        let corners = tile_corners(&map, size, plane.point, plane.normal, center)?;
        Some((map, size, plane, corners))
    }

    /// The grip under `cursor` while nothing is held.
    fn texture_grip_at(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Grip> {
        let (map, size, plane, corners) = self.texture_tile(model)?;
        let scale = view
            .project(self.to_world(corners[2]))
            .map(|p| p.distance(cursor));
        let turn = view
            .project(self.to_world(corners[1]))
            .map(|p| p.distance(cursor));
        match (scale, turn) {
            (Some(scale), Some(turn)) if scale <= GRIP_PX && turn <= GRIP_PX => {
                return Some(if turn < scale {
                    Grip::Turn
                } else {
                    Grip::Scale
                });
            }
            _ => {}
        }
        if scale.is_some_and(|distance| distance <= GRIP_PX) {
            return Some(Grip::Scale);
        }
        if turn.is_some_and(|distance| distance <= GRIP_PX) {
            return Some(Grip::Turn);
        }

        let (origin, dir) = self.body_ray(view, cursor);
        let denom = dir.dot(plane.normal);
        if !denom.is_finite() || denom.abs() < 1e-12 {
            return None;
        }
        let p = origin + (plane.point - origin).dot(plane.normal) / denom * dir;
        if !p.is_finite() {
            return None;
        }
        let q = map.apply(p);
        let base = map.apply(corners[0]);
        let tol = 1e-9;
        (q.x >= base.x - tol
            && q.x <= base.x + size.x + tol
            && q.y >= base.y - tol
            && q.y <= base.y + size.y + tol)
            .then_some(Grip::Slide)
    }

    /// The op for the held grip at `cursor`, or `None` when it is invalid there.
    fn held_texture_op(
        &self,
        view: &Viewport,
        cursor: DVec2,
        mods: Modifiers,
    ) -> Option<TextureOp> {
        let state = self.texture_handles.as_ref()?;
        let grip = state.held?;
        let (origin, dir) = self.body_ray(view, cursor);
        let denom = dir.dot(state.plane.normal);
        if !denom.is_finite() || denom.abs() < 1e-12 {
            return None;
        }
        let p = origin + (state.plane.point - origin).dot(state.plane.normal) / denom * dir;
        if !p.is_finite() {
            return None;
        }
        let q = state.map.apply(p);
        if !q.is_finite() {
            return None;
        }
        let about = state.base;
        match grip {
            Grip::Slide => {
                let g = state.map.apply(state.grab?);
                let op = TextureOp::Move(g - q);
                (g - q).is_finite().then_some(op)
            }
            Grip::Turn => {
                let from = q - about;
                if from.length() < 1e-9 {
                    return None;
                }
                let to = DVec2::new(state.size.x, 0.0);
                let angle = (from.x * to.y - from.y * to.x).atan2(from.dot(to));
                angle
                    .is_finite()
                    .then_some(TextureOp::Rotate { angle, about })
            }
            Grip::Scale => {
                let from = q - about;
                let to = state.size;
                let factor = if mods.shift {
                    DVec2::splat(to.length_squared() / from.dot(to))
                } else {
                    DVec2::new(to.x / from.x, to.y / from.y)
                };
                (factor.is_finite() && factor.x > 0.0 && factor.y > 0.0)
                    .then_some(TextureOp::Scale { factor, about })
            }
        }
    }

    /// Update the held grip's preview map from `op`.
    fn preview_texture_op(&mut self, op: TextureOp) {
        let affine = match op {
            TextureOp::Move(d) => DAffine2::from_translation(d),
            TextureOp::Rotate { angle, about } => {
                DAffine2::from_translation(about)
                    * DAffine2::from_angle(angle)
                    * DAffine2::from_translation(-about)
            }
            TextureOp::Scale { factor, about } => {
                DAffine2::from_translation(about)
                    * DAffine2::from_scale(factor)
                    * DAffine2::from_translation(-about)
            }
            TextureOp::Reset => return,
        };
        let Some(state) = self.texture_handles.as_mut() else {
            return;
        };
        let preview = state.map.then(affine);
        if preview.is_finite() {
            state.preview = preview;
        }
    }

    /// A click while the texture handles show. Behaviour:
    /// `docs/specs/m4-app-texture-handles.md`.
    fn click_texture_handles(
        &mut self,
        model: &mut Model,
        view: &Viewport,
        cursor: DVec2,
        mods: Modifiers,
    ) -> bool {
        let Some(state) = self.texture_handles.as_ref() else {
            return false;
        };
        let (face, side) = (state.face, state.side);
        let body = self.body(model);
        if self.textured_size(model, body, face, side).is_none() {
            self.end_texture_handles();
            return false;
        }
        if state.held.is_none() {
            let Some(grip) = self.texture_grip_at(model, view, cursor) else {
                self.end_texture_handles();
                return false;
            };
            let Some((map, size, plane, corners)) = self.texture_tile(model) else {
                self.end_texture_handles();
                return false;
            };
            let grab = if grip == Grip::Slide {
                let (origin, dir) = self.body_ray(view, cursor);
                let denom = dir.dot(plane.normal);
                if !denom.is_finite() || denom.abs() < 1e-12 {
                    self.end_texture_handles();
                    return false;
                }
                let p = origin + (plane.point - origin).dot(plane.normal) / denom * dir;
                p.is_finite().then_some(p)
            } else {
                None
            };
            if let Some(state) = self.texture_handles.as_mut() {
                state.map = map;
                state.base = map.apply(corners[0]);
                state.size = size;
                state.plane = plane;
                state.held = Some(grip);
                state.hover = None;
                state.grab = grab;
                state.preview = map;
            }
            return false;
        }

        let Some(op) = self.held_texture_op(view, cursor, mods) else {
            self.message = Some("Can't position that texture".to_string());
            return false;
        };
        let changed = self.position_texture(model, face, side, op);
        if let Some(state) = self.texture_handles.as_mut() {
            state.held = None;
            state.grab = None;
            state.hover = None;
        }
        changed
    }

    /// Show texture handles on `side` of `face` in the open body; while they show, clicks
    /// and pointer moves in the viewport go to them. True when they started. Behaviour:
    /// `docs/specs/m4-app-texture-handles.md`.
    pub fn start_texture_handles(&mut self, model: &Model, face: FaceId, side: Side) -> bool {
        let body = self.body(model);
        let Some(size) = self.textured_size(model, body, face, side) else {
            self.message = Some("That side has no texture".to_string());
            return false;
        };
        let computed = (|| {
            let map = texture::side_map(model, body, face, side)?;
            let geometry = model.body(body)?.geometry();
            let face_data = geometry.face(face)?;
            let plane = geometry.face_plane(face)?;
            let points = geometry.loop_points(&face_data.outer);
            if points.is_empty() {
                return None;
            }
            let center = points.iter().copied().sum::<DVec3>() / points.len() as f64;
            let corners = tile_corners(&map, size, plane.point, plane.normal, center)?;
            Some((map, plane, map.apply(corners[0])))
        })();
        let Some((map, plane, base)) = computed else {
            self.message = Some("Can't position that texture".to_string());
            return false;
        };

        let tool = self.tool;
        self.set_tool(tool);
        self.texture_handles = Some(TextureHandleState {
            face,
            side,
            map,
            base,
            size,
            plane,
            hover: None,
            held: None,
            grab: None,
            preview: map,
        });
        self.message = None;
        true
    }

    /// Hide the texture handles (dropping a held grip); nothing changes in the model.
    pub fn end_texture_handles(&mut self) {
        self.texture_handles = None;
    }

    /// The texture handles to draw, following the cursor while a grip is held; `None` when
    /// they aren't showing or their side has no texture any more.
    pub fn texture_handles(&self, model: &Model) -> Option<TextureHandles> {
        let state = self.texture_handles.as_ref()?;
        let body = self.body(model);
        self.textured_size(model, body, state.face, state.side)?;
        let corners = match state.held {
            Some(_) => {
                let offsets = [
                    DVec2::ZERO,
                    DVec2::new(state.size.x, 0.0),
                    state.size,
                    DVec2::new(0.0, state.size.y),
                ];
                let mut corners = [DVec3::ZERO; 4];
                for (corner, offset) in corners.iter_mut().zip(offsets) {
                    *corner = plane_point_at(
                        &state.preview,
                        state.plane.point,
                        state.plane.normal,
                        state.base + offset,
                    )?;
                }
                corners
            }
            None => self.texture_tile(model)?.3,
        };
        Some(TextureHandles {
            face: state.face,
            side: state.side,
            corners,
            hover: state.hover,
            held: state.held,
        })
    }

    /// The Paint Bucket's current material (`None` = the default material).
    pub fn material(&self) -> Option<MaterialId> {
        self.material
    }

    /// Choose the Paint Bucket's material; kept across tool changes.
    pub fn set_material(&mut self, material: Option<MaterialId>) {
        self.material = material;
    }

    /// Paint Bucket click: paint the clicked side, or the selection when the clicked
    /// entity is selected; Alt samples instead. Behaviour: `docs/specs/m4-app-paint.md`.
    fn click_paint(
        &mut self,
        model: &mut Model,
        view: &Viewport,
        cursor: DVec2,
        mods: Modifiers,
    ) -> bool {
        let Some(entity) = self.pick_at(model, view, cursor) else {
            return false;
        };
        let seen = match entity {
            Entity::Face(face) => {
                let (_, dir) = self.body_ray(view, cursor);
                let plane = model
                    .body(self.body(model))
                    .and_then(|body| body.geometry().face_plane(face));
                match plane {
                    Some(plane) if dir.dot(plane.normal) > 0.0 => Side::Back,
                    _ => Side::Front,
                }
            }
            Entity::Edge(_) | Entity::Instance(_) => Side::Front,
        };
        if mods.alt {
            match entity {
                Entity::Face(_) | Entity::Instance(_) => {
                    self.material = paint::sample(model, self.body(model), entity, seen).flatten();
                    self.message = None;
                }
                Entity::Edge(_) => {}
            }
            return false;
        }
        if matches!(entity, Entity::Edge(_)) && !self.selection.contains(&entity) {
            return false;
        }
        let targets = if self.selection.contains(&entity) {
            self.selection.clone()
        } else {
            vec![entity]
        };
        match paint::paint(
            model,
            self.body(model),
            &targets,
            seen,
            self.material,
            &mut self.undo,
            "Paint",
        ) {
            Ok(n) => {
                self.message = None;
                n > 0
            }
            Err(_) => {
                self.message = Some("Can't paint that".to_string());
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
        let (origin, dir) = self.body_ray(view, cursor);
        let ctx = model.body(self.body(model))?.geometry();
        let (face, t) = infer::ray_face(ctx, origin, dir)?;
        let normal = ctx.face_plane(face)?.normal;
        let start = origin + unit_or(dir, DVec3::Z) * t;
        Some((face, start, normal))
    }

    fn move_push(&mut self, model: &mut Model, view: &Viewport, cursor: DVec2) {
        let (origin, dir) = self.body_ray(view, cursor);
        self.push_hover = model
            .body(self.body(model))
            .and_then(|body| infer::ray_face(body.geometry(), origin, dir))
            .map(|(face, _)| face);
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
                    let (origin, dir) = self.body_ray(view, cursor);
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
        let Some(body) = model.body(self.body(model)) else {
            return false;
        };
        let ctx = body.geometry();
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
            self.body(model),
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
            self.body(model),
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
        let body = self.body(model);
        let mut target = Target {
            model,
            body,
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
        let (origin, dir) = self.body_ray(view, cursor);
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
        let body = self.body(model);
        let mut target = Target {
            model,
            body,
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
        let (origin, dir) = self.body_ray(view, cursor);
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
            let body = self.body(model);
            let mut target = Target {
                model,
                body,
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
            let body = self.body(model);
            let mut target = Target {
                model,
                body,
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
        let body = self.body(model);
        let Some(ctx) = model.body(body).map(|body| body.geometry()) else {
            return Vec::new();
        };
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
                    instances.extend(select::instance_segments(model, body, id));
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
        let body = self.body(model);
        let mut target = Target {
            model,
            body,
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
                .grab(handle, model, self.body(model), &self.selection)
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
            let body = self.body(model);
            let mut target = Target {
                model,
                body,
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
            let body = self.body(model);
            let mut target = Target {
                model,
                body,
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
        let bounds = bounds(model, self.body(model), &self.selection)?;
        let mut best: Option<(f64, Handle)> = None;
        for handle in handles(&bounds) {
            let Some(point) = view.project(self.to_world(bounds.point(handle))) else {
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
        self.infer_with(model, view, cursor, self.scale.fixed_point(), None)
    }

    fn move_move(&mut self, model: &mut Model, view: &Viewport, cursor: DVec2) {
        let Some(inf) = self.infer_move_at(model, view, cursor) else {
            self.hint = None;
            return;
        };
        self.hint = Some(inf);
        let body = self.body(model);
        let mut target = Target {
            model,
            body,
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
        if !was_from && self.move_copy {
            {
                let body = self.body(model);
                let mut target = Target {
                    model,
                    body,
                    undo: &mut self.undo,
                    unit: self.unit,
                };
                let _ = self
                    .translate
                    .handle(Input::Move(inf.point), &self.selection, &mut target);
            }
            if let Some(offset) = self.translate.offset()
                && offset.length() > TOL
            {
                let matrix = DMat4::from_translation(offset);
                let body = self.body(model);
                match copy_selection(model, body, &self.selection, matrix, &mut self.undo, "Copy") {
                    Ok(entities) => {
                        let selection = copied_selection(&self.selection, entities);
                        self.selection = selection;
                        self.translate = MoveTool::new();
                        self.move_segments.clear();
                        self.message = None;
                        return self.undo.undo_count() != before;
                    }
                    Err(_) => {
                        self.message = Some("Can't copy that".to_string());
                        return false;
                    }
                }
            }
        }
        let result = {
            let body = self.body(model);
            let mut target = Target {
                model,
                body,
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
        if self.move_copy && self.translate.stage() == MoveStage::To {
            let value = match units::parse_length(&text, self.unit) {
                Ok(value) => value,
                Err(error) => {
                    self.message = Some(move_error_message(&MoveError::Parse(error)).to_string());
                    return false;
                }
            };
            let Some(offset) = self.translate.offset() else {
                self.message = Some(move_error_message(&MoveError::NoDirection).to_string());
                return false;
            };
            if offset.length() <= TOL {
                self.message = Some(move_error_message(&MoveError::NoDirection).to_string());
                return false;
            }
            if value.abs() <= TOL {
                self.message = None;
                return false;
            }
            let matrix = DMat4::from_translation(offset.normalize() * value);
            let body = self.body(model);
            match copy_selection(model, body, &self.selection, matrix, &mut self.undo, "Copy") {
                Ok(entities) => {
                    let selection = copied_selection(&self.selection, entities);
                    self.selection = selection;
                    self.translate = MoveTool::new();
                    self.move_segments.clear();
                    self.message = None;
                    return self.undo.undo_count() != before;
                }
                Err(_) => {
                    self.message = Some("Can't copy that".to_string());
                    return false;
                }
            }
        }
        let result = {
            let body = self.body(model);
            let mut target = Target {
                model,
                body,
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
        let body = self.body(model);
        let mut target = Target {
            model,
            body,
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
            let body = self.body(model);
            let mut target = Target {
                model,
                body,
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
            let body = self.body(model);
            let mut target = Target {
                model,
                body,
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
                let Some(body) = model.body(self.body(model)) else {
                    self.tape.set_hover_edge(None);
                    return;
                };
                let ctx = body.geometry();
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
        self.infer_with(model, view, cursor, self.tape.start(), None)
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
                let Some(body) = model.body(self.body(model)) else {
                    return plane_for_normal(DVec3::Z);
                };
                let ctx = body.geometry();
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
        self.infer_with(model, view, cursor, self.rotate.center(), None)
    }

    fn infer_move_at(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Inference> {
        self.infer_with(model, view, cursor, self.translate.from(), None)
    }

    fn infer_shape_at(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Inference> {
        self.infer_with(model, view, cursor, self.shape_anchor(), None)
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
        self.infer_with(model, view, cursor, self.line.anchor(), self.lock_body())
    }

    fn pick_at(&self, model: &Model, view: &Viewport, cursor: DVec2) -> Option<Entity> {
        self.pick_with(model, view, cursor)
    }

    fn prune(&mut self, model: &Model) {
        let Some(body) = model.body(self.body(model)) else {
            self.selection.clear();
            return;
        };
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
