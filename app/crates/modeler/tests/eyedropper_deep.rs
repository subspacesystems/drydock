// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-eyedropper-deep: the eyedropper and Alt+click in the Paint Bucket sample the material
//! drawn under the cursor, also on faces inside components (`docs/specs/m6-eyedropper-deep.md`).

use glam::{DMat4, DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{Modifiers, Tool};
use modeler_kernel::model::*;
use modeler_kernel::paint::{Side, create_material, paint};
use modeler_kernel::select::Entity;
use modeler_kernel::units::Unit;
use modeler_view::{Bounds, Camera, Projection, StandardView};

const NONE: Modifiers = Modifiers {
    shift: false,
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

fn top_view() -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Top);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(-2.0),
            max: DVec3::splat(14.0),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

/// A component "part" (a 4 m square at the origin, its top side painted green) placed twice:
/// instance `a` at x 0 painted blue, instance `b` at x 6 unpainted. A second component
/// "plain" (an unpainted 4 m square) placed at y 6 painted red. Current material: none.
struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
    green: MaterialId,
    red: MaterialId,
    a: InstanceId,
}

fn square_def(model: &mut Model, name: &str) -> DefinitionId {
    let mut log = ModelEdit::new();
    let d = model.add_definition(
        DefinitionInfo {
            name: name.into(),
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
    d
}

fn place(
    model: &mut Model,
    d: DefinitionId,
    at: DVec3,
    material: Option<MaterialId>,
) -> InstanceId {
    let mut log = ModelEdit::new();
    model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: d,
                transform: DMat4::from_translation(at),
                name: String::new(),
                material,
            },
            &mut log,
        )
        .unwrap()
}

impl World {
    fn new() -> World {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let mut make = |model: &mut Model, name: &str, c: [f32; 4]| {
            create_material(model, Material::new(name, c), &mut undo, "New").unwrap()
        };
        let green = make(&mut model, "green", [0.0, 1.0, 0.0, 1.0]);
        let blue = make(&mut model, "blue", [0.0, 0.0, 1.0, 1.0]);
        let red = make(&mut model, "red", [1.0, 0.0, 0.0, 1.0]);
        let part = square_def(&mut model, "part");
        let body = BodyId::Definition(part);
        let face = model
            .body(body)
            .unwrap()
            .geometry()
            .faces()
            .map(|(id, _)| id)
            .next()
            .unwrap();
        let normal = model
            .body(body)
            .unwrap()
            .geometry()
            .face_plane(face)
            .unwrap()
            .normal;
        let top = if normal.z > 0.0 {
            Side::Front
        } else {
            Side::Back
        };
        let mut undo = UndoStack::new();
        paint(
            &mut model,
            body,
            &[Entity::Face(face)],
            top,
            Some(green),
            &mut undo,
            "Paint",
        )
        .unwrap();
        let a = place(&mut model, part, v(0.0, 0.0, 0.0), Some(blue));
        place(&mut model, part, v(6.0, 0.0, 0.0), None);
        let plain = square_def(&mut model, "plain");
        place(&mut model, plain, v(0.0, 6.0, 0.0), Some(red));
        let mut editor = Editor::new();
        set_unit(&mut model, Unit::Meter);
        editor.set_tool(Tool::Select);
        World {
            model,
            editor,
            view: top_view(),
            green,
            red,
            a,
        }
    }

    fn click_with(&mut self, p: DVec3, mods: Modifiers) -> bool {
        let c: DVec2 = self.view.project(p).expect("visible");
        self.editor
            .pointer_move(&mut self.model, &self.view, c, mods);
        self.editor.click(&mut self.model, &self.view, c, mods, 1)
    }
}

#[test]
fn the_eyedropper_samples_a_painted_face_inside_a_painted_component() {
    let mut w = World::new();
    w.editor.arm_eyedropper();
    let before = w.model.clone();
    w.click_with(v(2.0, 2.0, 0.0), NONE);
    assert_eq!(
        w.editor.material(),
        Some(w.green),
        "the face, not the instance"
    );
    assert!(!w.editor.eyedropper_armed());
    assert_eq!(w.editor.tool(), Tool::Select);
    assert_eq!(w.model, before);
    assert!(w.editor.selection().is_empty());
}

#[test]
fn the_eyedropper_samples_a_painted_face_inside_an_unpainted_component() {
    let mut w = World::new();
    w.editor.arm_eyedropper();
    w.click_with(v(8.0, 2.0, 0.0), NONE);
    assert_eq!(w.editor.material(), Some(w.green));
}

#[test]
fn an_unpainted_face_gives_the_component_material() {
    let mut w = World::new();
    w.editor.arm_eyedropper();
    w.click_with(v(2.0, 8.0, 0.0), NONE);
    assert_eq!(w.editor.material(), Some(w.red));
}

#[test]
fn alt_click_in_the_paint_bucket_samples_the_same_way() {
    let mut w = World::new();
    w.editor.set_tool(Tool::Paint);
    let before = w.model.clone();
    assert!(!w.click_with(v(2.0, 2.0, 0.0), ALT));
    assert_eq!(w.editor.material(), Some(w.green));
    assert_eq!(w.model, before, "sampling paints nothing");
    assert_eq!(w.editor.tool(), Tool::Paint);
}

#[test]
fn inside_an_open_component_the_eyedropper_samples_too() {
    let mut w = World::new();
    assert!(w.editor.open_instance(&w.model, w.a));
    w.editor.arm_eyedropper();
    w.click_with(v(2.0, 2.0, 0.0), NONE);
    assert_eq!(w.editor.material(), Some(w.green));
    // A component outside the open one, shown as the rest of the model.
    w.editor.arm_eyedropper();
    w.click_with(v(2.0, 8.0, 0.0), NONE);
    assert_eq!(w.editor.material(), Some(w.red));
}

#[test]
fn empty_space_keeps_waiting() {
    let mut w = World::new();
    w.editor.set_material(Some(w.red));
    w.editor.arm_eyedropper();
    w.click_with(v(12.0, 12.0, 0.0), NONE);
    assert_eq!(w.editor.material(), Some(w.red));
    assert!(w.editor.eyedropper_armed(), "still armed");
}

/// Typed numbers and measurements in `unit`, 2 places.
fn set_unit(model: &mut Model, unit: Unit) {
    model
        .set_length_format(
            modeler_kernel::units::LengthFormat::Decimal { unit, decimals: 2 },
            &mut modeler_kernel::model::ModelEdit::new(),
        )
        .unwrap();
}
