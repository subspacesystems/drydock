// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-fix-export-fit: when File > Export is enabled and what name the save dialog suggests.
//! See docs/specs/m7-fix-export-fit.md.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::{DVec2, DVec3};
use modeler::chrome::{MenuState, menu_bar};
use modeler::editor::*;
use modeler::input::{Action, EditKey, Modifiers, Tool};
use modeler::mesh_export::*;
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

/// A model with the unit square at x 1..2, y 0..1 drawn loose into the root, and an editor
/// in the Select tool.
fn square_model() -> (Model, Editor, Viewport) {
    let corners = [
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(2.0, 0.0, 0.0),
        DVec3::new(2.0, 1.0, 0.0),
        DVec3::new(1.0, 1.0, 0.0),
    ];
    let segs: Vec<_> = (0..4).map(|i| (corners[i], corners[(i + 1) % 4])).collect();
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let mut t = Target {
        model: &mut model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    draw_segments(&mut t, &segs, "Draw").expect("draw");
    let mut editor = Editor::new();
    editor.set_tool(Tool::Select);
    (model, editor, top_view())
}

fn click_middle(model: &mut Model, editor: &mut Editor, view: &Viewport) {
    let c: DVec2 = view.project(DVec3::new(1.5, 0.5, 0.0)).expect("visible");
    editor.pointer_move(model, view, c, NONE);
    editor.click(model, view, c, NONE, 1);
}

#[test]
fn an_empty_model_cannot_export() {
    assert!(!can_export(&Model::new()));
}

#[test]
fn loose_geometry_alone_cannot_export() {
    let (model, _, _) = square_model();
    assert_eq!(model.root().geometry().face_count(), 1);
    assert!(!can_export(&model));
}

#[test]
fn a_group_can_export() {
    let (mut model, mut editor, view) = square_model();
    click_middle(&mut model, &mut editor, &view);
    assert!(matches!(editor.selection(), [Entity::Face(_)]));
    assert!(editor.make_component(&mut model, "G", true));
    assert!(can_export(&model));
}

#[test]
fn a_component_inside_a_component_can_export() {
    let (mut model, mut editor, view) = square_model();
    click_middle(&mut model, &mut editor, &view);
    assert!(editor.make_component(&mut model, "Inner", false));
    assert!(matches!(editor.selection(), [Entity::Instance(_)]));
    assert!(editor.make_component(&mut model, "Outer", false));
    assert_eq!(model.root().instances().count(), 1);
    assert!(can_export(&model));
}

#[test]
fn undoing_the_group_makes_export_unavailable_again() {
    let (mut model, mut editor, view) = square_model();
    click_middle(&mut model, &mut editor, &view);
    assert!(editor.make_component(&mut model, "G", true));
    assert!(can_export(&model));
    assert!(editor.key(EditKey::Undo, &mut model));
    assert!(!can_export(&model));
}

fn name(path: &str, format: ExportFormat, layout: StlLayout) -> String {
    default_export_name(Path::new(path), format, layout)
}

#[test]
fn the_name_drops_the_documents_own_extension() {
    use ExportFormat::*;
    use StlLayout::*;
    assert_eq!(name("/m/two.stl", Stl, SingleFile), "two.stl");
    assert_eq!(name("/m/two.stl", ThreeMf, SingleFile), "two.3mf");
    assert_eq!(name("/m/a.modeler", Stl, SingleFile), "a.stl");
    assert_eq!(name("/m/a.modeler", ThreeMf, SingleFile), "a.3mf");
    assert_eq!(name("/m/x.dae", ThreeMf, SingleFile), "x.3mf");
    assert_eq!(name("x.3mf", Stl, SingleFile), "x.stl");
}

#[test]
fn only_the_last_extension_is_removed() {
    use ExportFormat::*;
    use StlLayout::*;
    assert_eq!(name("/m/part.v2.stl", ThreeMf, SingleFile), "part.v2.3mf");
    assert_eq!(name("/m/noext", Stl, SingleFile), "noext.stl");
}

#[test]
fn a_folder_export_gets_the_bare_stem() {
    use ExportFormat::*;
    use StlLayout::*;
    assert_eq!(name("/m/two.stl", Stl, OneFilePerObject), "two");
    assert_eq!(name("/m/a.modeler", Stl, OneFilePerObject), "a");
}

#[test]
fn the_layout_is_ignored_for_3mf() {
    use ExportFormat::*;
    use StlLayout::*;
    assert_eq!(name("/m/a.modeler", ThreeMf, OneFilePerObject), "a.3mf");
}

#[test]
fn no_file_name_gives_untitled() {
    use ExportFormat::*;
    use StlLayout::*;
    assert_eq!(name("", Stl, SingleFile), "untitled.stl");
    assert_eq!(name("", ThreeMf, SingleFile), "untitled.3mf");
    assert_eq!(name("", Stl, OneFilePerObject), "untitled");
}

fn clicked_export_item(can_export: bool, label: &str) -> Vec<Action> {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState {
        can_export,
        ..MenuState::default()
    };
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("File").click();
    h.run();
    h.get_by_label(label).click();
    h.run();
    seen.borrow().clone()
}

#[test]
fn the_export_items_work_when_can_export_is_true() {
    assert_eq!(
        clicked_export_item(true, "Export 3MF..."),
        vec![Action::ExportMesh(ExportFormat::ThreeMf)]
    );
    assert_eq!(
        clicked_export_item(true, "Export STL..."),
        vec![Action::ExportMesh(ExportFormat::Stl)]
    );
}

#[test]
fn the_export_items_do_nothing_when_can_export_is_false() {
    assert!(clicked_export_item(false, "Export 3MF...").is_empty());
    assert!(clicked_export_item(false, "Export STL...").is_empty());
}
