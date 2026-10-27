// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-eyedropper: the Materials panel's eyedropper button and the editor's armed sampling
//! click. See docs/specs/m6-eyedropper.md.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::{DVec2, DVec3};
use modeler::editor::*;
use modeler::icons::BUTTON_SIZE;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler::materials_panel::{MaterialAction, MaterialsPanel, materials_panel};
use modeler_kernel::model::{BodyId, Material, MaterialId, Model, UndoStack};
use modeler_kernel::paint::{Side, create_material, paint};
use modeler_kernel::select::Entity;
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::units::Unit;
use modeler_view::{Bounds, Camera, Projection, StandardView};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn top_view() -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Top);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(-8.0),
            max: DVec3::splat(8.0),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

fn square(x0: f64) -> Vec<(DVec3, DVec3)> {
    let p = [
        v(x0, 0.0, 0.0),
        v(x0 + 1.0, 0.0, 0.0),
        v(x0 + 1.0, 1.0, 0.0),
        v(x0, 1.0, 0.0),
    ];
    (0..4).map(|i| (p[i], p[(i + 1) % 4])).collect()
}

/// Top view; unit squares at x 1..2 and x 3..4 (the second is painted blue on the side the
/// top camera sees); red is current; the Select tool is active.
struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
    red: MaterialId,
    blue: MaterialId,
}

impl World {
    fn new() -> World {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        for x0 in [1.0, 3.0] {
            let mut t = Target {
                model: &mut model,
                body: BodyId::Root,
                undo: &mut undo,
                unit: Unit::Meter,
            };
            draw_segments(&mut t, &square(x0), "Draw").expect("draw");
        }
        let red = create_material(
            &mut model,
            Material::new("red", [1.0, 0.0, 0.0, 1.0]),
            &mut undo,
            "New",
        )
        .expect("red");
        let blue = create_material(
            &mut model,
            Material::new("blue", [0.0, 0.0, 1.0, 1.0]),
            &mut undo,
            "New",
        )
        .expect("blue");
        let face = {
            let ctx = model.root().geometry();
            ctx.faces()
                .map(|(id, _)| id)
                .find(|&id| {
                    let pts = ctx.loop_points(&ctx.face(id).unwrap().outer);
                    pts.iter().all(|p| p.x >= 3.0 - 1e-9)
                })
                .expect("face")
        };
        let normal = model
            .root()
            .geometry()
            .face_plane(face)
            .expect("plane")
            .normal;
        let seen = if DVec3::NEG_Z.dot(normal) < 0.0 {
            Side::Front
        } else {
            Side::Back
        };
        paint(
            &mut model,
            BodyId::Root,
            &[Entity::Face(face)],
            seen,
            Some(blue),
            &mut undo,
            "Paint",
        )
        .expect("paint");
        let mut editor = Editor::new();
        editor.unit = Unit::Meter;
        editor.set_tool(Tool::Select);
        editor.set_material(Some(red));
        World {
            model,
            editor,
            view: top_view(),
            red,
            blue,
        }
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn click(&mut self, p: DVec3) -> bool {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, NONE);
        self.editor.click(&mut self.model, &self.view, c, NONE, 1)
    }

    fn steps(&self) -> usize {
        self.editor.undo_stack().undo_count()
    }
}

const PAINTED: DVec3 = DVec3::new(3.5, 0.5, 0.0);
const UNPAINTED: DVec3 = DVec3::new(1.5, 0.5, 0.0);
const EMPTY: DVec3 = DVec3::new(6.0, 5.0, 0.0);

#[test]
fn a_new_editor_is_not_armed() {
    let w = World::new();
    assert!(!w.editor.eyedropper_armed());
}

#[test]
fn the_next_click_samples_in_any_tool_and_switches_to_the_paint_bucket() {
    let mut w = World::new();
    w.editor.arm_eyedropper();
    assert!(w.editor.eyedropper_armed());
    assert_eq!(w.editor.tool(), Tool::Select, "arming keeps the tool");
    let before = w.model.clone();
    assert!(!w.click(PAINTED), "sampling changes nothing in the model");
    assert_eq!(w.editor.material(), Some(w.blue));
    assert_eq!(w.editor.tool(), Tool::Paint);
    assert!(!w.editor.eyedropper_armed(), "one shot");
    assert_eq!(w.model, before);
    assert_eq!(w.steps(), 0);
    assert!(w.editor.selection().is_empty(), "does not select");
}

#[test]
fn sampling_an_unpainted_face_makes_the_default_material_current() {
    let mut w = World::new();
    w.editor.arm_eyedropper();
    w.click(UNPAINTED);
    assert_eq!(w.editor.material(), None);
    assert_eq!(w.editor.tool(), Tool::Paint);
    assert!(!w.editor.eyedropper_armed());
}

#[test]
fn the_click_after_a_sample_paints_normally() {
    let mut w = World::new();
    w.editor.arm_eyedropper();
    w.click(PAINTED);
    assert!(
        w.click(UNPAINTED),
        "the Paint Bucket paints with the sample"
    );
    let ctx = w.model.root().geometry();
    let painted = ctx
        .faces()
        .filter(|(id, _)| {
            let m = w.model.root().face_materials(*id);
            m.front == Some(w.blue) || m.back == Some(w.blue)
        })
        .count();
    assert_eq!(painted, 2);
}

#[test]
fn a_click_on_nothing_keeps_waiting() {
    let mut w = World::new();
    w.editor.arm_eyedropper();
    assert!(!w.click(EMPTY));
    assert!(w.editor.eyedropper_armed());
    assert_eq!(w.editor.material(), Some(w.red));
    assert_eq!(w.editor.tool(), Tool::Select);
    w.click(PAINTED);
    assert_eq!(w.editor.material(), Some(w.blue));
}

#[test]
fn escape_disarms_and_the_next_click_acts_as_usual() {
    let mut w = World::new();
    w.editor.arm_eyedropper();
    w.editor.key(EditKey::Cancel, &mut w.model);
    assert!(!w.editor.eyedropper_armed());
    w.click(PAINTED);
    assert_eq!(w.editor.material(), Some(w.red));
    assert_eq!(w.editor.tool(), Tool::Select);
    assert_eq!(w.editor.selection().len(), 1, "a plain Select click");
}

#[test]
fn choosing_a_tool_disarms() {
    let mut w = World::new();
    w.editor.arm_eyedropper();
    w.editor.set_tool(Tool::Line);
    assert!(!w.editor.eyedropper_armed());
}

// ---- the panel button ----

struct State {
    panel: MaterialsPanel,
    model: Model,
    actions: Vec<MaterialAction>,
}

fn harness() -> Harness<'static, State> {
    let mut model = Model::new();
    let mut log = modeler_kernel::model::ModelEdit::new();
    let id = model.add_material(Material::new("Brick", [0.7, 0.3, 0.2, 1.0]), &mut log);
    let mut h = Harness::builder()
        .with_size(egui::vec2(600.0, 800.0))
        .build_ui_state(
            move |ui, state: &mut State| {
                ui.allocate_ui(egui::vec2(160.0, 400.0), |ui| {
                    ui.set_max_width(160.0);
                    if let Some(action) =
                        materials_panel(ui, &mut state.panel, &state.model, Some(id))
                    {
                        state.actions.push(action);
                    }
                });
            },
            State {
                panel: MaterialsPanel::default(),
                model,
                actions: Vec::new(),
            },
        );
    h.run();
    h
}

#[test]
fn the_eyedropper_button_is_a_square_icon_button_after_purge_on_the_first_row() {
    let h = harness();
    let eye = h.get_by_label("Eyedropper").rect();
    assert!((eye.width() - BUTTON_SIZE).abs() < 0.5 && (eye.height() - BUTTON_SIZE).abs() < 0.5);
    let new = h.get_by_label("New...").rect();
    let purge = h.get_by_label("Purge Unused").rect();
    assert!((eye.center().y - new.center().y).abs() < 0.5, "one row");
    assert!(purge.right() <= eye.left(), "after Purge");
    assert!(eye.right() <= 160.0 + 0.5, "inside the column");
}

#[test]
fn the_eyedropper_button_has_a_tooltip_and_asks_to_arm() {
    let mut h = harness();
    h.get_by_label("Eyedropper").hover();
    h.run();
    h.get_by_label("Pick a material from the model");
    h.get_by_label("Eyedropper").click();
    h.run();
    assert_eq!(h.state().actions, vec![MaterialAction::Eyedropper]);
}
