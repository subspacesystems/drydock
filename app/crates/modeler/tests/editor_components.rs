// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Make component, make group and explode in the editor (`docs/specs/m4-app-components.md`).

use glam::{DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler_kernel::model::*;
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

const SQUARE: [DVec3; 4] = [
    DVec3::new(1.0, 0.0, 0.0),
    DVec3::new(2.0, 0.0, 0.0),
    DVec3::new(2.0, 1.0, 0.0),
    DVec3::new(1.0, 1.0, 0.0),
];

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
}

impl World {
    /// Top view, the unit square at x 1..2, y 0..1 drawn into the root, Select tool.
    fn new() -> World {
        let mut w = World {
            model: Model::new(),
            editor: Editor::new(),
            view: top_view(),
        };
        set_unit(&mut w.model, Unit::Meter);
        let segs: Vec<_> = (0..4).map(|i| (SQUARE[i], SQUARE[(i + 1) % 4])).collect();
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut w.model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segs, "Draw").expect("draw");
        w.editor.set_tool(Tool::Select);
        w
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn click(&mut self, p: DVec3) {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, NONE);
        self.editor.click(&mut self.model, &self.view, c, NONE, 1);
    }

    fn select_face(&mut self) {
        self.editor.set_tool(Tool::Select);
        self.click(v(1.5, 0.5, 0.0));
        assert!(matches!(self.editor.selection(), [Entity::Face(_)]));
    }

    fn undo_label(&self) -> Option<String> {
        self.editor.undo_stack().undo_label().map(str::to_owned)
    }

    fn faces(&self) -> usize {
        self.model.root().geometry().face_count()
    }

    fn root_instances(&self) -> Vec<(InstanceId, Instance)> {
        self.model
            .root()
            .instances()
            .map(|(id, i)| (id, i.clone()))
            .collect()
    }
}

#[test]
fn make_component_replaces_the_selection_with_an_instance() {
    let mut w = World::new();
    w.select_face();
    assert!(w.editor.make_component(&mut w.model, "Slab", false));
    assert_eq!(w.faces(), 0);
    let inst = w.root_instances();
    assert_eq!(inst.len(), 1);
    let def = w.model.definition(inst[0].1.definition).unwrap();
    assert_eq!(def.name, "Slab");
    assert!(!def.group);
    assert_eq!(w.editor.selection(), &[Entity::Instance(inst[0].0)]);
    assert_eq!(w.editor.message(), None);
    assert_eq!(w.undo_label().as_deref(), Some("Make Component"));
}

#[test]
fn make_group_is_a_group() {
    let mut w = World::new();
    w.select_face();
    assert!(w.editor.make_component(&mut w.model, "", true));
    let inst = w.root_instances();
    let def = w.model.definition(inst[0].1.definition).unwrap();
    assert!(def.group);
    assert_eq!(def.name, "Group");
    assert_eq!(w.undo_label().as_deref(), Some("Make Group"));
}

#[test]
fn names_are_made_unique() {
    let mut w = World::new();
    w.select_face();
    assert!(w.editor.make_component(&mut w.model, "Part", false));
    // Draw a second square elsewhere and make another "Part".
    let segs = [
        (v(-3.0, 0.0, 0.0), v(-2.0, 0.0, 0.0)),
        (v(-2.0, 0.0, 0.0), v(-2.0, 1.0, 0.0)),
        (v(-2.0, 1.0, 0.0), v(-3.0, 1.0, 0.0)),
        (v(-3.0, 1.0, 0.0), v(-3.0, 0.0, 0.0)),
    ];
    let mut undo = UndoStack::new();
    let mut t = Target {
        model: &mut w.model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    draw_segments(&mut t, &segs, "Draw").unwrap();
    w.editor.set_tool(Tool::Select);
    w.click(v(-2.5, 0.5, 0.0));
    assert!(w.editor.make_component(&mut w.model, "Part", false));
    let mut names: Vec<_> = w.model.definitions().map(|(_, d)| d.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["Part", "Part 2"]);
}

#[test]
fn empty_selection_is_refused() {
    let mut w = World::new();
    let steps = w.editor.undo_stack().undo_count();
    assert!(!w.editor.make_component(&mut w.model, "X", false));
    assert_eq!(
        w.editor.message(),
        Some("Select something to make a component of")
    );
    assert!(!w.editor.make_component(&mut w.model, "X", true));
    assert_eq!(w.editor.message(), Some("Select something to group"));
    assert_eq!(w.faces(), 1);
    assert!(w.root_instances().is_empty());
    assert_eq!(w.editor.undo_stack().undo_count(), steps);
}

#[test]
fn undo_restores_the_face() {
    let mut w = World::new();
    w.select_face();
    assert!(w.editor.make_component(&mut w.model, "Slab", false));
    assert!(w.editor.key(EditKey::Undo, &mut w.model));
    assert_eq!(w.faces(), 1);
    assert!(w.root_instances().is_empty());
    assert_eq!(w.model.definitions().count(), 0);
}

#[test]
fn explode_puts_the_geometry_back() {
    let mut w = World::new();
    w.select_face();
    assert!(w.editor.make_component(&mut w.model, "Slab", false));
    assert!(w.editor.explode_selection(&mut w.model));
    assert_eq!(w.faces(), 1);
    assert!(w.root_instances().is_empty());
    assert_eq!(w.editor.message(), None);
    assert_eq!(w.undo_label().as_deref(), Some("Explode"));
    let sel = w.editor.selection();
    assert!(sel.iter().any(|e| matches!(e, Entity::Face(_))));
    assert!(sel.iter().any(|e| matches!(e, Entity::Edge(_))));
    assert!(w.editor.key(EditKey::Undo, &mut w.model));
    assert_eq!(w.faces(), 0);
    assert_eq!(w.root_instances().len(), 1);
}

#[test]
fn explode_needs_exactly_one_instance() {
    let mut w = World::new();
    w.select_face();
    let steps = w.editor.undo_stack().undo_count();
    assert!(!w.editor.explode_selection(&mut w.model));
    assert_eq!(
        w.editor.message(),
        Some("Select one group or component to explode")
    );
    assert_eq!(w.editor.undo_stack().undo_count(), steps);
    assert_eq!(w.faces(), 1);
    w.editor.set_tool(Tool::Select);
    w.editor.key(EditKey::Cancel, &mut w.model);
    assert!(!w.editor.explode_selection(&mut w.model));
    assert_eq!(
        w.editor.message(),
        Some("Select one group or component to explode")
    );
}

#[test]
fn works_inside_an_open_instance() {
    let mut w = World::new();
    w.select_face();
    assert!(w.editor.make_component(&mut w.model, "Slab", false));
    let Entity::Instance(outer) = w.editor.selection()[0] else {
        panic!("instance selected");
    };
    assert!(w.editor.open_instance(&w.model, outer));
    // Inside, the face is in the definition; select it and nest it.
    w.editor.set_tool(Tool::Select);
    w.click(v(1.5, 0.5, 0.0));
    assert!(matches!(w.editor.selection(), [Entity::Face(_)]));
    assert!(w.editor.make_component(&mut w.model, "Inner", true));
    assert_eq!(w.faces(), 0);
    assert_eq!(w.model.definitions().count(), 2);
    let def = w.root_instances()[0].1.definition;
    assert_eq!(
        w.model
            .body(BodyId::Definition(def))
            .unwrap()
            .instances()
            .count(),
        1
    );
    // Explode the inner group again, still inside.
    assert!(matches!(w.editor.selection(), [Entity::Instance(_)]));
    assert!(w.editor.explode_selection(&mut w.model));
    let body = w.model.body(BodyId::Definition(def)).unwrap();
    assert_eq!(body.instances().count(), 0);
    assert_eq!(body.geometry().face_count(), 1);
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
