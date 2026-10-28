// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The editor on geometry inside instances (`docs/specs/m2-pick-imported.md`).

use glam::{DMat4, DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{Modifiers, Tool};
use modeler_kernel::infer::Snap;
use modeler_kernel::model::*;
use modeler_kernel::select::Entity;
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

/// Top view, parallel, framing -5..15 on every axis, 800x600 px.
fn top_view() -> Viewport {
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

/// A model whose only content is an instance of a 4 m square (0,0)-(4,4), moved to x = 6.
fn world() -> (Model, InstanceId, Editor, Viewport) {
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
    let editor = Editor::new();
    set_unit(&mut model, Unit::Meter);
    (model, i, editor, top_view())
}

fn px(view: &Viewport, p: DVec3) -> DVec2 {
    view.project(p).expect("visible")
}

#[test]
fn clicking_an_instance_face_selects_the_instance() {
    let (mut model, i, mut editor, view) = world();
    editor.set_tool(Tool::Select);
    let c = px(&view, v(8.0, 2.0, 0.0));
    editor.pointer_move(&mut model, &view, c, NONE);
    editor.click(&mut model, &view, c, NONE, 1);
    assert_eq!(editor.selection(), &[Entity::Instance(i)]);
}

#[test]
fn clicking_an_instance_edge_selects_the_instance() {
    let (mut model, i, mut editor, view) = world();
    editor.set_tool(Tool::Select);
    let c = px(&view, v(6.0, 2.0, 0.0));
    editor.pointer_move(&mut model, &view, c, NONE);
    editor.click(&mut model, &view, c, NONE, 1);
    assert_eq!(editor.selection(), &[Entity::Instance(i)]);
}

#[test]
fn clicking_empty_space_clears_the_selection() {
    let (mut model, i, mut editor, view) = world();
    editor.set_tool(Tool::Select);
    let c = px(&view, v(8.0, 2.0, 0.0));
    editor.click(&mut model, &view, c, NONE, 1);
    assert_eq!(editor.selection(), &[Entity::Instance(i)]);
    let c = px(&view, v(-3.0, -3.0, 0.0));
    editor.click(&mut model, &view, c, NONE, 1);
    assert!(editor.selection().is_empty());
}

#[test]
fn a_triple_click_keeps_the_instance_selected() {
    let (mut model, i, mut editor, view) = world();
    editor.set_tool(Tool::Select);
    let c = px(&view, v(8.0, 2.0, 0.0));
    for count in [1, 3] {
        editor.click(&mut model, &view, c, NONE, count);
        assert_eq!(editor.selection(), &[Entity::Instance(i)], "click {count}");
    }
}

#[test]
fn a_box_over_the_instance_selects_it() {
    let (mut model, i, mut editor, view) = world();
    editor.set_tool(Tool::Select);
    let a = px(&view, v(5.0, 5.0, 0.0));
    let b = px(&view, v(11.0, -1.0, 0.0));
    editor.box_select(&model, &view, a, b, NONE);
    assert_eq!(editor.selection(), &[Entity::Instance(i)]);
    let _ = &mut model;
}

#[test]
fn the_line_tool_snaps_to_an_instance_corner() {
    let (mut model, _, mut editor, view) = world();
    set_unit(&mut model, Unit::Meter);
    editor.set_tool(Tool::Line);
    // A little off the corner (6, 0).
    let c = px(&view, v(6.0, 0.0, 0.0)) + DVec2::new(3.0, -2.0);
    editor.pointer_move(&mut model, &view, c, NONE);
    let hint = editor.hint().expect("a hint");
    assert!(matches!(hint.snap, Snap::Endpoint(_)), "{:?}", hint.snap);
    assert!(
        hint.point.distance(v(6.0, 0.0, 0.0)) < 1e-6,
        "{:?}",
        hint.point
    );
}

#[test]
fn a_line_started_on_an_instance_corner_starts_exactly_there() {
    let (mut model, _, mut editor, view) = world();
    editor.set_tool(Tool::Line);
    let c = px(&view, v(6.0, 4.0, 0.0)) + DVec2::new(-2.0, 2.0);
    editor.pointer_move(&mut model, &view, c, NONE);
    editor.click(&mut model, &view, c, NONE, 1);
    let end = px(&view, v(6.0, 8.0, 0.0));
    editor.pointer_move(&mut model, &view, end, NONE);
    editor.click(&mut model, &view, end, NONE, 1);
    let ctx = model.root().geometry();
    assert_eq!(ctx.edge_count(), 1);
    let (_, e) = ctx.edges().next().unwrap();
    let ends = [
        ctx.vertex(e.ends[0]).unwrap().position,
        ctx.vertex(e.ends[1]).unwrap().position,
    ];
    assert!(
        ends.iter().any(|p| p.distance(v(6.0, 4.0, 0.0)) < 1e-6),
        "{ends:?}"
    );
}

#[test]
fn the_rectangle_tool_snaps_to_an_instance_corner() {
    let (mut model, _, mut editor, view) = world();
    editor.set_tool(Tool::Rectangle);
    let c = px(&view, v(10.0, 4.0, 0.0)) + DVec2::new(2.0, 2.0);
    editor.pointer_move(&mut model, &view, c, NONE);
    let hint = editor.hint().expect("a hint");
    assert!(matches!(hint.snap, Snap::Endpoint(_)), "{:?}", hint.snap);
    assert!(
        hint.point.distance(v(10.0, 4.0, 0.0)) < 1e-6,
        "{:?}",
        hint.point
    );
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
