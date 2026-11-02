// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The editor working inside an open instance (`docs/specs/m4-app-edit-context.md`).

use drydock::editor::*;
use drydock::input::{EditKey, Modifiers, Tool};
use drydock_kernel::editpath::EditPath;
use drydock_kernel::infer::Snap;
use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use drydock_kernel::units::Unit;
use drydock_view::{Bounds, Camera, Projection, StandardView};
use glam::{DMat4, DVec2, DVec3};

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
            max: DVec3::splat(16.0),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
    def: DefinitionId,
    inst: InstanceId,
}

impl World {
    /// A definition holding a 4 m square (0,0)-(4,4) with its face, one instance of it
    /// moved to x = 6, and a root edge (-4,-4)-(-2,-4) outside it. Top view, tool Select.
    fn new() -> World {
        let mut model = Model::new();
        let mut log = ModelEdit::new();
        let def = model.add_definition(
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
            .edit_geometry(BodyId::Definition(def), &mut log, |ctx, edit| {
                for i in 0..4 {
                    ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
                }
            })
            .unwrap();
        model
            .edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
                ctx.add_edge(v(-4.0, -4.0, 0.0), v(-2.0, -4.0, 0.0), edit)
                    .unwrap();
            })
            .unwrap();
        let inst = model
            .add_instance(
                BodyId::Root,
                Instance {
                    definition: def,
                    transform: DMat4::from_translation(v(6.0, 0.0, 0.0)),
                    name: String::new(),
                    material: None,
                },
                &mut log,
            )
            .unwrap();
        let mut editor = Editor::new();
        set_unit(&mut model, Unit::Meter);
        editor.set_tool(Tool::Select);
        World {
            model,
            editor,
            view: top_view(),
            def,
            inst,
        }
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn mv(&mut self, p: DVec3) {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, NONE);
    }

    fn click(&mut self, p: DVec3, count: u32) -> bool {
        self.mv(p);
        let c = self.px(p);
        self.editor
            .click(&mut self.model, &self.view, c, NONE, count)
    }

    fn open(&mut self) {
        assert!(self.editor.open_instance(&self.model, self.inst));
    }

    fn def_edges(&self) -> usize {
        self.model
            .body(BodyId::Definition(self.def))
            .unwrap()
            .geometry()
            .edge_count()
    }

    fn def_faces(&self) -> usize {
        self.model
            .body(BodyId::Definition(self.def))
            .unwrap()
            .geometry()
            .face_count()
    }
}

#[test]
fn starts_at_the_root() {
    let w = World::new();
    assert!(w.editor.path().is_root());
    assert!(!w.editor.hide_rest());
}

#[test]
fn open_instance_pushes_the_step_and_clears_the_selection() {
    let mut w = World::new();
    w.click(v(8.0, 2.0, 0.0), 1);
    assert_eq!(w.editor.selection(), &[Entity::Instance(w.inst)]);
    w.open();
    assert_eq!(
        w.editor.path(),
        &EditPath {
            steps: vec![w.inst]
        }
    );
    assert!(w.editor.selection().is_empty());
    assert_eq!(
        w.editor.path().body(&w.model),
        Some(BodyId::Definition(w.def))
    );
}

#[test]
fn opening_something_that_is_not_an_instance_of_the_open_body_fails() {
    let mut w = World::new();
    assert!(!w.editor.open_instance(&w.model, InstanceId(9999)));
    assert!(w.editor.path().is_root());
    w.open();
    // The instance lives in the root, not inside the definition.
    assert!(!w.editor.open_instance(&w.model, w.inst));
    assert_eq!(w.editor.path().steps.len(), 1);
}

#[test]
fn close_path_pops_and_clears_the_selection() {
    let mut w = World::new();
    w.open();
    w.click(v(8.0, 2.0, 0.0), 1);
    assert_eq!(w.editor.selection().len(), 1);
    w.editor.close_path();
    assert!(w.editor.path().is_root());
    assert!(w.editor.selection().is_empty());
    // At the root it does nothing, and keeps the selection.
    w.click(v(8.0, 2.0, 0.0), 1);
    assert_eq!(w.editor.selection(), &[Entity::Instance(w.inst)]);
    w.editor.close_path();
    assert_eq!(w.editor.selection(), &[Entity::Instance(w.inst)]);
}

#[test]
fn the_path_survives_tool_changes() {
    let mut w = World::new();
    w.open();
    w.editor.set_tool(Tool::Line);
    w.editor.set_tool(Tool::Erase);
    assert_eq!(w.editor.path().steps, vec![w.inst]);
}

#[test]
fn double_click_on_an_instance_opens_it() {
    let mut w = World::new();
    w.click(v(8.0, 2.0, 0.0), 1);
    let changed = w.click(v(8.0, 2.0, 0.0), 2);
    assert!(!changed);
    assert_eq!(w.editor.path().steps, vec![w.inst]);
    assert!(w.editor.selection().is_empty());
}

#[test]
fn double_click_outside_closes() {
    let mut w = World::new();
    w.open();
    // Empty space.
    w.click(v(0.0, 10.0, 0.0), 2);
    assert!(w.editor.path().is_root());
    w.open();
    // Rest of the model (the root edge).
    w.click(v(-3.0, -4.0, 0.0), 2);
    assert!(w.editor.path().is_root());
}

#[test]
fn double_click_on_empty_space_at_the_root_does_nothing() {
    let mut w = World::new();
    w.click(v(0.0, 10.0, 0.0), 2);
    assert!(w.editor.path().is_root());
    assert!(w.editor.selection().is_empty());
}

#[test]
fn escape_closes_with_the_select_tool_only() {
    let mut w = World::new();
    w.open();
    w.editor.set_tool(Tool::Line);
    w.editor.key(EditKey::Cancel, &mut w.model);
    assert_eq!(w.editor.path().steps, vec![w.inst]);
    w.editor.set_tool(Tool::Select);
    w.editor.key(EditKey::Cancel, &mut w.model);
    assert!(w.editor.path().is_root());
    // At the root it still does nothing.
    w.editor.key(EditKey::Cancel, &mut w.model);
    assert!(w.editor.path().is_root());
}

#[test]
fn selecting_inside_the_open_instance_picks_its_face() {
    let mut w = World::new();
    w.open();
    w.click(v(8.0, 2.0, 0.0), 1);
    let sel = w.editor.selection().to_vec();
    assert_eq!(sel.len(), 1);
    assert!(matches!(sel[0], Entity::Face(_)), "{sel:?}");
    // The face is the definition's.
    let ctx = w.model.body(BodyId::Definition(w.def)).unwrap().geometry();
    let Entity::Face(f) = sel[0] else {
        unreachable!()
    };
    assert!(ctx.face_plane(f).is_some());
}

#[test]
fn clicking_the_rest_of_the_model_clears_the_selection() {
    let mut w = World::new();
    w.open();
    w.click(v(8.0, 2.0, 0.0), 1);
    assert_eq!(w.editor.selection().len(), 1);
    w.click(v(-3.0, -4.0, 0.0), 1);
    assert!(w.editor.selection().is_empty());
    assert_eq!(
        w.editor.path().steps,
        vec![w.inst],
        "single click keeps it open"
    );
}

#[test]
fn a_box_selects_inside_the_open_instance() {
    let mut w = World::new();
    w.open();
    let a = w.px(v(5.0, 5.0, 0.0));
    let b = w.px(v(11.0, -1.0, 0.0));
    w.editor.box_select(&w.model, &w.view, a, b, NONE);
    let sel = w.editor.selection();
    assert_eq!(sel.len(), 5, "{sel:?}");
    assert!(sel.iter().all(|e| !matches!(e, Entity::Instance(_))));
    assert_eq!(
        sel.iter().filter(|e| matches!(e, Entity::Edge(_))).count(),
        4
    );
}

#[test]
fn the_line_tool_draws_into_the_definition_in_body_coordinates() {
    let mut w = World::new();
    w.open();
    w.editor.set_tool(Tool::Line);
    // Diagonal from the instance's (6,0) corner to its (10,4) corner.
    w.mv(v(6.0, 0.0, 0.0));
    let hint = w.editor.hint().expect("a hint");
    assert!(matches!(hint.snap, Snap::Endpoint(_)), "{:?}", hint.snap);
    assert!(
        hint.point.distance(v(6.0, 0.0, 0.0)) < 1e-6,
        "{:?}",
        hint.point
    );
    w.click(v(6.0, 0.0, 0.0), 1);
    w.click(v(10.0, 4.0, 0.0), 1);
    assert_eq!(w.def_edges(), 5);
    assert_eq!(w.def_faces(), 2, "the diagonal splits the face");
    assert_eq!(w.model.root().geometry().edge_count(), 1, "root untouched");
    let ctx = w.model.body(BodyId::Definition(w.def)).unwrap().geometry();
    let ends: Vec<Vec<DVec3>> = ctx
        .edges()
        .map(|(_, e)| {
            e.ends
                .iter()
                .map(|&id| ctx.vertex(id).unwrap().position)
                .collect()
        })
        .collect();
    let diagonal = ends.iter().any(|p| {
        let d = |a: DVec3, b: DVec3| {
            (p[0].distance(a) < 1e-6 && p[1].distance(b) < 1e-6)
                || (p[1].distance(a) < 1e-6 && p[0].distance(b) < 1e-6)
        };
        d(v(0.0, 0.0, 0.0), v(4.0, 4.0, 0.0))
    });
    assert!(diagonal, "{ends:?}");
}

#[test]
fn the_instance_moves_nothing_when_its_definition_is_edited() {
    let mut w = World::new();
    let before = w.model.root().instance(w.inst).unwrap().transform;
    w.open();
    w.editor.set_tool(Tool::Line);
    w.click(v(6.0, 0.0, 0.0), 1);
    w.click(v(10.0, 4.0, 0.0), 1);
    assert_eq!(w.model.root().instance(w.inst).unwrap().transform, before);
}

#[test]
fn erase_removes_geometry_of_the_open_body_only() {
    let mut w = World::new();
    w.open();
    w.editor.set_tool(Tool::Erase);
    assert!(w.click(v(6.0, 2.0, 0.0), 1), "the instance's left edge");
    assert_eq!(w.def_edges(), 3);
    assert_eq!(w.def_faces(), 0, "the face goes with its edge");
    // The root edge is outside the open instance: not erased.
    assert!(!w.click(v(-3.0, -4.0, 0.0), 1));
    assert_eq!(w.model.root().geometry().edge_count(), 1);
}

#[test]
fn paint_works_on_the_open_body() {
    let mut w = World::new();
    let mut log = ModelEdit::new();
    let red = w
        .model
        .add_material(Material::new("red", [1.0, 0.0, 0.0, 1.0]), &mut log);
    w.open();
    w.editor.set_tool(Tool::Paint);
    w.editor.set_material(Some(red));
    assert!(w.click(v(8.0, 2.0, 0.0), 1));
    let ctx = w.model.body(BodyId::Definition(w.def)).unwrap().geometry();
    let (f, _) = ctx.faces().next().unwrap();
    let m = drydock_kernel::paint::sample(
        &w.model,
        BodyId::Definition(w.def),
        Entity::Face(f),
        drydock_kernel::paint::Side::Front,
    );
    let back = drydock_kernel::paint::sample(
        &w.model,
        BodyId::Definition(w.def),
        Entity::Face(f),
        drydock_kernel::paint::Side::Back,
    );
    // Top view looks down at the face: one of the sides got red, the other did not.
    assert!(
        m == Some(Some(red)) || back == Some(Some(red)),
        "{m:?} {back:?}"
    );
}

#[test]
fn the_rest_of_the_model_is_snapped_to_unless_hidden() {
    let mut w = World::new();
    w.open();
    w.editor.set_tool(Tool::Line);
    let corner = v(-4.0, -4.0, 0.0);
    let c = w.px(corner) + DVec2::new(2.0, -2.0);
    w.editor.pointer_move(&mut w.model, &w.view, c, NONE);
    let hint = w.editor.hint().expect("snaps to the root edge's end");
    assert!(matches!(hint.snap, Snap::Endpoint(_)), "{:?}", hint.snap);
    assert!(hint.point.distance(corner) < 1e-6, "{:?}", hint.point);

    w.editor.set_hide_rest(true);
    assert!(w.editor.hide_rest());
    w.editor.pointer_move(&mut w.model, &w.view, c, NONE);
    let near = w
        .editor
        .hint()
        .is_some_and(|h| matches!(h.snap, Snap::Endpoint(_)) && h.point.distance(corner) < 1e-6);
    assert!(!near, "hidden rest must not be snapped to");
}

#[test]
fn hidden_rest_is_not_erased_either() {
    let mut w = World::new();
    w.open();
    w.editor.set_hide_rest(true);
    w.editor.set_tool(Tool::Erase);
    assert!(!w.click(v(-3.0, -4.0, 0.0), 1));
    assert_eq!(w.model.root().geometry().edge_count(), 1);
}

#[test]
fn a_removed_instance_closes_the_path() {
    let mut w = World::new();
    w.open();
    w.click(v(8.0, 2.0, 0.0), 1);
    assert_eq!(w.editor.selection().len(), 1);
    let mut log = ModelEdit::new();
    w.model
        .remove_instance(BodyId::Root, w.inst, &mut log)
        .unwrap();
    w.mv(v(0.0, 10.0, 0.0));
    assert!(w.editor.path().is_root());
    assert!(w.editor.selection().is_empty());
}

#[test]
fn the_root_behaves_as_before_when_nothing_is_open() {
    let mut w = World::new();
    w.editor.set_tool(Tool::Erase);
    assert!(w.click(v(-3.0, -4.0, 0.0), 1));
    assert_eq!(w.model.root().geometry().edge_count(), 0);
}

/// Typed numbers and measurements in `unit`, 2 places.
fn set_unit(model: &mut Model, unit: Unit) {
    model
        .set_length_format(
            drydock_kernel::units::LengthFormat::Decimal { unit, decimals: 2 },
            &mut drydock_kernel::model::ModelEdit::new(),
        )
        .unwrap();
}
