// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Placing a component definition with the editor (`docs/specs/m6-components-place.md`).

use glam::{DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler_kernel::model::{BodyId, DefinitionId, DefinitionInfo, Model, ModelEdit, UndoStack};
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

fn view() -> Viewport {
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

/// The definition's unit square, origin at its corner.
const SQUARE: [DVec3; 4] = [
    DVec3::new(0.0, 0.0, 0.0),
    DVec3::new(1.0, 0.0, 0.0),
    DVec3::new(1.0, 1.0, 0.0),
    DVec3::new(0.0, 1.0, 0.0),
];

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
    /// A component with the unit square.
    part: DefinitionId,
    /// A group (not placeable).
    group: DefinitionId,
}

impl World {
    /// An empty root, one component "Part" holding the square, one group "Box".
    fn new() -> World {
        let mut model = Model::new();
        let mut log = ModelEdit::default();
        let part = model.add_definition(
            DefinitionInfo {
                name: "Part".to_string(),
                group: false,
            },
            &mut log,
        );
        let group = model.add_definition(
            DefinitionInfo {
                name: "Box".to_string(),
                group: true,
            },
            &mut log,
        );
        let segs: Vec<_> = (0..4).map(|i| (SQUARE[i], SQUARE[(i + 1) % 4])).collect();
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut model,
            body: BodyId::Definition(part),
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segs, "Draw").expect("draw");
        let mut editor = Editor::new();
        set_unit(&mut model, Unit::Meter);
        editor.set_tool(Tool::Select);
        World {
            model,
            editor,
            view: view(),
            part,
            group,
        }
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn move_to(&mut self, p: DVec3) {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, NONE);
    }

    fn click(&mut self, p: DVec3) -> bool {
        self.move_to(p);
        let c = self.px(p);
        self.editor.click(&mut self.model, &self.view, c, NONE, 1)
    }

    fn instances(&self) -> Vec<(modeler_kernel::model::InstanceId, DVec3, DefinitionId)> {
        self.model
            .root()
            .instances()
            .map(|(id, i)| (id, i.transform.transform_point3(DVec3::ZERO), i.definition))
            .collect()
    }

    fn undo_count(&self) -> usize {
        self.editor.undo_stack().undo_count()
    }
}

#[test]
fn not_placing_at_start() {
    let w = World::new();
    assert_eq!(w.editor.placing(), None);
    assert!(w.editor.place_preview(&w.model).is_empty());
}

#[test]
fn an_unknown_definition_is_refused() {
    let mut w = World::new();
    assert!(!w.editor.place_definition(&w.model, DefinitionId(9999)));
    assert_eq!(w.editor.placing(), None);
    assert!(w.editor.message().is_some());
}

#[test]
fn a_group_is_refused() {
    let mut w = World::new();
    assert!(!w.editor.place_definition(&w.model, w.group));
    assert_eq!(w.editor.placing(), None);
    assert!(w.editor.message().is_some());
}

#[test]
fn placing_starts_with_the_select_tool_and_changes_nothing() {
    let mut w = World::new();
    w.editor.set_tool(Tool::Line);
    let base = w.undo_count();
    assert!(w.editor.place_definition(&w.model, w.part));
    assert_eq!(w.editor.placing(), Some(w.part));
    assert_eq!(w.editor.tool(), Tool::Select);
    assert_eq!(w.undo_count(), base);
    assert!(w.instances().is_empty());
    // No pointer over the viewport yet.
    assert!(w.editor.place_preview(&w.model).is_empty());
}

#[test]
fn the_preview_holds_the_origin_at_the_cursor() {
    let mut w = World::new();
    assert!(w.editor.place_definition(&w.model, w.part));
    w.move_to(v(3.0, 2.0, 0.0));
    let preview = w.editor.place_preview(&w.model);
    assert_eq!(preview.len(), 4);
    let d = v(3.0, 2.0, 0.0);
    for p in SQUARE {
        assert!(
            preview
                .iter()
                .any(|(a, b)| a.distance(p + d) < 1e-6 || b.distance(p + d) < 1e-6),
            "corner {p:?} missing in the preview"
        );
    }
    w.move_to(v(-4.0, -3.0, 0.0));
    let preview = w.editor.place_preview(&w.model);
    let d = v(-4.0, -3.0, 0.0);
    assert!(
        preview
            .iter()
            .any(|(a, b)| a.distance(d) < 1e-6 || b.distance(d) < 1e-6)
    );
}

#[test]
fn a_click_places_one_instance_at_the_cursor() {
    let mut w = World::new();
    assert!(w.editor.place_definition(&w.model, w.part));
    let base = w.undo_count();
    assert!(w.click(v(3.0, 2.0, 0.0)));
    let instances = w.instances();
    assert_eq!(instances.len(), 1);
    let (id, origin, definition) = instances[0];
    assert_eq!(definition, w.part);
    assert!(origin.distance(v(3.0, 2.0, 0.0)) < 1e-6, "{origin:?}");
    assert_eq!(w.editor.selection(), &[Entity::Instance(id)]);
    assert_eq!(w.editor.placing(), None);
    assert!(w.editor.place_preview(&w.model).is_empty());
    assert_eq!(w.editor.tool(), Tool::Select);
    assert_eq!(w.editor.message(), None);
    assert_eq!(w.undo_count(), base + 1);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Place Component"));
}

#[test]
fn placing_is_undone_in_one_step() {
    let mut w = World::new();
    assert!(w.editor.place_definition(&w.model, w.part));
    assert!(w.click(v(3.0, 2.0, 0.0)));
    assert!(w.editor.key(EditKey::Undo, &mut w.model));
    assert!(w.instances().is_empty());
}

#[test]
fn the_same_definition_places_again() {
    let mut w = World::new();
    assert!(w.editor.place_definition(&w.model, w.part));
    assert!(w.click(v(3.0, 2.0, 0.0)));
    assert!(w.editor.place_definition(&w.model, w.part));
    assert!(w.click(v(-4.0, -3.0, 0.0)));
    assert_eq!(w.instances().len(), 2);
}

#[test]
fn escape_cancels_the_placing() {
    let mut w = World::new();
    assert!(w.editor.place_definition(&w.model, w.part));
    w.move_to(v(3.0, 2.0, 0.0));
    let base = w.undo_count();
    w.editor.key(EditKey::Cancel, &mut w.model);
    assert_eq!(w.editor.placing(), None);
    assert!(w.editor.place_preview(&w.model).is_empty());
    assert_eq!(w.undo_count(), base);
    assert!(!w.click(v(3.0, 2.0, 0.0)));
    assert!(w.instances().is_empty());
}

#[test]
fn switching_tools_ends_the_placing() {
    let mut w = World::new();
    assert!(w.editor.place_definition(&w.model, w.part));
    w.editor.set_tool(Tool::Move);
    assert_eq!(w.editor.placing(), None);
    assert!(w.editor.place_preview(&w.model).is_empty());
    w.editor.set_tool(Tool::Select);
    assert!(!w.click(v(3.0, 2.0, 0.0)));
    assert!(w.instances().is_empty());
}

#[test]
fn a_definition_cannot_be_placed_inside_itself() {
    let mut w = World::new();
    assert!(w.editor.place_definition(&w.model, w.part));
    assert!(w.click(v(3.0, 2.0, 0.0)));
    let (id, _, _) = w.instances()[0];
    assert!(w.editor.open_instance(&w.model, id));
    assert!(!w.editor.place_definition(&w.model, w.part));
    assert_eq!(w.editor.placing(), None);
    assert!(w.editor.message().is_some());
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
