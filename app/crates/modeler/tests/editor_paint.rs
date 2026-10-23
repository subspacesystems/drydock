// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Paint Bucket in the editor (`docs/specs/m4-app-paint.md`).

use glam::{DMat4, DVec2, DVec3};
use modeler::chrome::TOOL_BUTTONS;
use modeler::editor::*;
use modeler::input::{self, Action, EditKey, Modifiers, Tool};
use modeler_kernel::model::{
    BodyId, DefinitionInfo, FaceMaterials, Instance, InstanceId, Material, MaterialId, Model,
    ModelEdit, UndoStack,
};
use modeler_kernel::paint::{Side, create_material, paint};
use modeler_kernel::select::Entity;
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::topology::FaceId;
use modeler_kernel::units::Unit;
use modeler_view::{Bounds, Camera, Projection, StandardView};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};
const SHIFT: Modifiers = Modifiers {
    shift: true,
    ctrl: false,
    alt: false,
};
const ALT: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: true,
};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn view(standard: StandardView) -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(standard);
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

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
    red: MaterialId,
    blue: MaterialId,
}

impl World {
    /// Top view; unit squares at x 1..2 and x 3..4 (y 0..1, z 0); materials red and blue;
    /// Paint tool with red current.
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
        let mut editor = Editor::new();
        editor.unit = Unit::Meter;
        editor.set_tool(Tool::Paint);
        editor.set_material(Some(red));
        World {
            model,
            editor,
            view: view(StandardView::Top),
            red,
            blue,
        }
    }

    fn face_at(&self, x: f64) -> FaceId {
        let ctx = self.model.root().geometry();
        ctx.faces()
            .map(|(id, _)| id)
            .find(|&id| {
                let pts = ctx.loop_points(&ctx.face(id).unwrap().outer);
                pts.iter().all(|p| p.x >= x - 1e-9 && p.x <= x + 1.0 + 1e-9)
            })
            .expect("face")
    }

    /// The side of the face at `x` that a camera looking along `dir` sees.
    fn seen(&self, x: f64, dir: DVec3) -> Side {
        let n = self
            .model
            .root()
            .geometry()
            .face_plane(self.face_at(x))
            .expect("plane")
            .normal;
        if dir.dot(n) < 0.0 {
            Side::Front
        } else {
            Side::Back
        }
    }

    fn mats(&self, x: f64) -> FaceMaterials {
        self.model.root().face_materials(self.face_at(x))
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn click_with(&mut self, p: DVec3, mods: Modifiers) -> bool {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, mods);
        self.editor.click(&mut self.model, &self.view, c, mods, 1)
    }

    fn click(&mut self, p: DVec3) -> bool {
        self.click_with(p, NONE)
    }

    fn select(&mut self, p: DVec3, mods: Modifiers) {
        let tool = self.editor.tool();
        let material = self.editor.material();
        self.editor.set_tool(Tool::Select);
        self.click_with(p, mods);
        self.editor.set_tool(tool);
        self.editor.set_material(material);
    }

    fn steps(&self) -> usize {
        self.editor.undo_stack().undo_count()
    }
}

fn on(side: Side, m: Option<MaterialId>) -> FaceMaterials {
    match side {
        Side::Front => FaceMaterials {
            front: m,
            back: None,
            ..Default::default()
        },
        Side::Back => FaceMaterials {
            front: None,
            back: m,
            ..Default::default()
        },
    }
}

const DOWN: DVec3 = DVec3::new(0.0, 0.0, -1.0);
const UP: DVec3 = DVec3::new(0.0, 0.0, 1.0);

#[test]
fn click_paints_the_seen_side_of_the_face() {
    let mut w = World::new();
    let side = w.seen(1.0, DOWN);
    assert!(w.click(v(1.5, 0.5, 0.0)));
    assert_eq!(w.mats(1.0), on(side, Some(w.red)));
    assert_eq!(w.mats(3.0), FaceMaterials::default());
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Paint"));
    assert_eq!(w.steps(), 1);
    assert_eq!(w.editor.message(), None);
}

#[test]
fn from_below_the_other_side_is_painted() {
    let mut w = World::new();
    w.view = view(StandardView::Bottom);
    let side = w.seen(1.0, UP);
    assert_ne!(side, w.seen(1.0, DOWN));
    assert!(w.click(v(1.5, 0.5, 0.0)));
    assert_eq!(w.mats(1.0), on(side, Some(w.red)));
}

#[test]
fn both_sides_can_hold_different_materials() {
    let mut w = World::new();
    let top = w.seen(1.0, DOWN);
    assert!(w.click(v(1.5, 0.5, 0.0)));
    w.view = view(StandardView::Bottom);
    w.editor.set_material(Some(w.blue));
    assert!(w.click(v(1.5, 0.5, 0.0)));
    let m = w.mats(1.0);
    let (seen_top, seen_bottom) = match top {
        Side::Front => (m.front, m.back),
        Side::Back => (m.back, m.front),
    };
    assert_eq!(seen_top, Some(w.red));
    assert_eq!(seen_bottom, Some(w.blue));
    assert_eq!(w.steps(), 2);
}

#[test]
fn click_on_nothing_does_nothing() {
    let mut w = World::new();
    let before = w.model.clone();
    assert!(!w.click(v(-5.0, -5.0, 0.0)));
    assert_eq!(w.model, before);
    assert_eq!(w.steps(), 0);
    assert_eq!(w.editor.message(), None);
}

#[test]
fn unselected_edge_is_not_painted() {
    let mut w = World::new();
    let before = w.model.clone();
    assert!(!w.click(v(1.5, 0.0, 0.0)));
    assert_eq!(w.model, before);
    assert_eq!(w.steps(), 0);
    assert_eq!(w.editor.message(), None);
}

#[test]
fn clicking_a_selected_face_paints_the_whole_selection() {
    let mut w = World::new();
    w.select(v(1.5, 0.5, 0.0), NONE);
    w.select(v(3.5, 0.5, 0.0), SHIFT);
    assert_eq!(w.editor.selection().len(), 2);
    let side = w.seen(3.0, DOWN);
    assert!(w.click(v(3.5, 0.5, 0.0)));
    assert_eq!(w.mats(1.0), on(side, Some(w.red)));
    assert_eq!(w.mats(3.0), on(side, Some(w.red)));
    assert_eq!(w.steps(), 1, "one undo step for the whole selection");
    assert_eq!(w.editor.selection().len(), 2, "selection kept");
}

#[test]
fn clicking_outside_the_selection_paints_only_the_clicked_face() {
    let mut w = World::new();
    w.select(v(1.5, 0.5, 0.0), NONE);
    let selection = w.editor.selection().to_vec();
    let side = w.seen(3.0, DOWN);
    assert!(w.click(v(3.5, 0.5, 0.0)));
    assert_eq!(w.mats(1.0), FaceMaterials::default());
    assert_eq!(w.mats(3.0), on(side, Some(w.red)));
    assert_eq!(w.editor.selection(), &selection[..]);
}

#[test]
fn clicking_a_selected_edge_paints_the_selected_faces_front() {
    let mut w = World::new();
    w.select(v(1.5, 0.5, 0.0), NONE);
    w.select(v(1.5, 0.0, 0.0), SHIFT);
    assert_eq!(w.editor.selection().len(), 2);
    assert!(w.click(v(1.5, 0.0, 0.0)));
    assert_eq!(w.mats(1.0), on(Side::Front, Some(w.red)));
}

#[test]
fn repainting_the_same_material_changes_nothing() {
    let mut w = World::new();
    assert!(w.click(v(1.5, 0.5, 0.0)));
    let before = w.model.clone();
    assert!(!w.click(v(1.5, 0.5, 0.0)));
    assert_eq!(w.model, before);
    assert_eq!(w.steps(), 1);
    assert_eq!(w.editor.message(), None);
}

#[test]
fn default_material_unpaints() {
    let mut w = World::new();
    assert!(w.click(v(1.5, 0.5, 0.0)));
    w.editor.set_material(None);
    assert!(w.click(v(1.5, 0.5, 0.0)));
    assert_eq!(w.mats(1.0), FaceMaterials::default());
}

#[test]
fn undo_takes_the_paint_off() {
    let mut w = World::new();
    let before = w.model.clone();
    assert!(w.click(v(1.5, 0.5, 0.0)));
    assert!(w.editor.key(EditKey::Undo, &mut w.model));
    assert_eq!(w.model, before);
    assert!(w.editor.key(EditKey::Redo, &mut w.model));
    assert_ne!(w.model, before);
}

#[test]
fn a_missing_material_is_refused() {
    let mut w = World::new();
    w.editor.set_material(Some(MaterialId(9999)));
    let before = w.model.clone();
    assert!(!w.click(v(1.5, 0.5, 0.0)));
    assert_eq!(w.model, before);
    assert_eq!(w.steps(), 0);
    assert_eq!(w.editor.message(), Some("Can't paint that"));
    w.editor.set_material(Some(w.red));
    assert!(w.click(v(1.5, 0.5, 0.0)));
    assert_eq!(w.editor.message(), None, "a good paint clears the message");
}

#[test]
fn alt_click_samples_the_seen_side() {
    let mut w = World::new();
    let face = w.face_at(1.0);
    let side = w.seen(1.0, DOWN);
    let mut undo = UndoStack::new();
    paint(
        &mut w.model,
        BodyId::Root,
        &[Entity::Face(face)],
        side,
        Some(w.blue),
        &mut undo,
        "Paint",
    )
    .expect("paint");
    let before = w.model.clone();
    assert!(!w.click_with(v(1.5, 0.5, 0.0), ALT));
    assert_eq!(w.editor.material(), Some(w.blue));
    assert_eq!(w.model, before);
    assert_eq!(w.steps(), 0);
    assert_eq!(w.editor.tool(), Tool::Paint);
}

#[test]
fn alt_click_on_the_other_side_samples_that_side() {
    let mut w = World::new();
    let face = w.face_at(1.0);
    let top = w.seen(1.0, DOWN);
    let mut undo = UndoStack::new();
    paint(
        &mut w.model,
        BodyId::Root,
        &[Entity::Face(face)],
        top,
        Some(w.blue),
        &mut undo,
        "Paint",
    )
    .expect("paint");
    w.view = view(StandardView::Bottom);
    assert!(!w.click_with(v(1.5, 0.5, 0.0), ALT));
    assert_eq!(w.editor.material(), None, "the bottom side is unpainted");
}

#[test]
fn alt_click_on_an_unpainted_face_samples_the_default() {
    let mut w = World::new();
    assert_eq!(w.editor.material(), Some(w.red));
    assert!(!w.click_with(v(3.5, 0.5, 0.0), ALT));
    assert_eq!(w.editor.material(), None);
}

#[test]
fn alt_click_on_an_edge_or_nothing_keeps_the_material() {
    let mut w = World::new();
    assert!(!w.click_with(v(1.5, 0.0, 0.0), ALT));
    assert_eq!(w.editor.material(), Some(w.red));
    assert!(!w.click_with(v(-5.0, -5.0, 0.0), ALT));
    assert_eq!(w.editor.material(), Some(w.red));
}

#[test]
fn sample_then_paint_copies_a_material() {
    let mut w = World::new();
    w.editor.set_material(Some(w.blue));
    assert!(w.click(v(1.5, 0.5, 0.0)));
    w.editor.set_material(None);
    assert!(!w.click_with(v(1.5, 0.5, 0.0), ALT));
    assert!(w.click(v(3.5, 0.5, 0.0)));
    assert_eq!(w.mats(3.0), on(w.seen(3.0, DOWN), Some(w.blue)));
}

#[test]
fn pointer_move_never_changes_the_model() {
    let mut w = World::new();
    let before = w.model.clone();
    for p in [v(1.5, 0.5, 0.0), v(1.5, 0.0, 0.0), v(-5.0, 0.0, 0.0)] {
        let c = w.px(p);
        w.editor.pointer_move(&mut w.model, &w.view, c, NONE);
        assert!(w.editor.hint().is_none());
    }
    assert_eq!(w.model, before);
}

#[test]
fn material_survives_tool_changes() {
    let mut w = World::new();
    w.editor.set_tool(Tool::Select);
    w.editor.set_tool(Tool::Paint);
    assert_eq!(w.editor.material(), Some(w.red));
    assert_eq!(Editor::new().material(), None);
}

#[test]
fn b_key_and_toolbar_button_choose_the_paint_bucket() {
    assert_eq!(input::key("B", NONE), Some(Action::SelectTool(Tool::Paint)));
    assert!(TOOL_BUTTONS.contains(&(Tool::Paint, "Paint Bucket")));
}

/// A model with an instance of a 4 m square (0,0)-(4,4) placed at x = 6.
fn instance_world() -> (Model, InstanceId, MaterialId, Editor) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let d = model.add_definition(
        DefinitionInfo {
            name: "part".into(),
            group: false,
        },
        &mut log,
    );
    let pts = [
        v(0.0, 0.0, 0.0),
        v(4.0, 0.0, 0.0),
        v(4.0, 4.0, 0.0),
        v(0.0, 4.0, 0.0),
    ];
    model
        .edit_geometry(BodyId::Definition(d), &mut log, |ctx, edit| {
            for i in 0..4 {
                ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
            }
        })
        .unwrap();
    let i = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: d,
                transform: DMat4::from_translation(v(6.0, 0.0, 0.0)),
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    let mut undo = UndoStack::new();
    let red = create_material(
        &mut model,
        Material::new("red", [1.0, 0.0, 0.0, 1.0]),
        &mut undo,
        "New",
    )
    .unwrap();
    let mut editor = Editor::new();
    editor.unit = Unit::Meter;
    editor.set_tool(Tool::Paint);
    (model, i, red, editor)
}

fn wide_top() -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Top);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(-5.0),
            max: DVec3::splat(15.0),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

#[test]
fn clicking_an_instance_paints_the_instance() {
    let (mut model, i, red, mut editor) = instance_world();
    let view = wide_top();
    editor.set_material(Some(red));
    let inside_before = model.definitions().map(|(d, _)| d).collect::<Vec<_>>();
    let d = inside_before[0];
    let def_body_before = model.body(BodyId::Definition(d)).cloned();
    let c = view.project(v(8.0, 2.0, 0.0)).unwrap();
    assert!(editor.click(&mut model, &view, c, NONE, 1));
    assert_eq!(model.root().instance(i).unwrap().material, Some(red));
    assert_eq!(
        model.body(BodyId::Definition(d)).cloned(),
        def_body_before,
        "the definition's faces are untouched"
    );
    assert_eq!(editor.undo_stack().undo_label(), Some("Paint"));
}

#[test]
fn alt_clicking_an_instance_samples_its_material() {
    let (mut model, i, red, mut editor) = instance_world();
    let view = wide_top();
    let mut inst = model.root().instance(i).unwrap().clone();
    inst.material = Some(red);
    let mut log = ModelEdit::new();
    model.set_instance(BodyId::Root, i, inst, &mut log).unwrap();
    let c = view.project(v(8.0, 2.0, 0.0)).unwrap();
    assert!(!editor.click(&mut model, &view, c, ALT, 1));
    assert_eq!(editor.material(), Some(red));
}
