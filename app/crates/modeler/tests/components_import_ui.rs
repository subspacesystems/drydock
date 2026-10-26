// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-components-import-ui: Import button, file reading and the editor import.
//! See docs/specs/m6-components-import-ui.md.

use std::path::Path;

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::DVec3;
use modeler::components_panel::*;
use modeler::editor::Editor;
use modeler_kernel::model::*;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

struct State {
    panel: ComponentsPanel,
    model: Model,
    actions: Vec<ComponentAction>,
}

fn harness(model: Model) -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(400.0, 600.0))
        .build_ui_state(
            |ui, state: &mut State| {
                if let Some(action) = components_panel(ui, &mut state.panel, &state.model) {
                    state.actions.push(action);
                }
            },
            State {
                panel: ComponentsPanel::default(),
                model,
                actions: Vec::new(),
            },
        );
    h.run();
    h
}

/// A model whose root holds a unit square.
fn square_model() -> Model {
    let mut model = Model::new();
    let pts = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(1.0, 1.0, 0.0),
        DVec3::new(0.0, 1.0, 0.0),
    ];
    let mut log = ModelEdit::new();
    model
        .edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
            for i in 0..4 {
                ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
            }
        })
        .unwrap();
    model
}

fn components(model: &Model) -> Vec<(DefinitionId, String)> {
    model
        .definitions()
        .filter(|(_, info)| !info.group)
        .map(|(id, info)| (id, info.name.clone()))
        .collect()
}

#[test]
fn import_button_asks_to_import() {
    let mut h = harness(Model::new());
    h.get_by_label("Import").click();
    h.run();
    assert_eq!(h.state().actions, vec![ComponentAction::Import]);
}

#[test]
fn import_button_is_there_with_components_too() {
    let mut h = harness(Model::new());
    h.state_mut().model = {
        let mut m = square_model();
        let mut log = ModelEdit::new();
        m.add_definition(
            DefinitionInfo {
                name: "Part".to_string(),
                group: false,
            },
            &mut log,
        );
        m
    };
    h.run();
    assert!(h.query_by_label("Import").is_some());
    assert!(h.query_by_label("List").is_some());
    assert!(h.query_by_label("Tiles").is_some());
    assert!(h.state().actions.is_empty());
}

#[test]
fn reads_a_dae_and_names_it_after_the_file() {
    let (model, name) = read_component_source(&fixture("slab.dae")).unwrap();
    assert_eq!(name, "slab");
    assert!(!model.root().is_empty());
}

#[test]
fn reads_a_native_file() {
    let (model, name) = read_component_source(&fixture("slab.modeler")).unwrap();
    assert_eq!(name, "slab");
    assert!(!model.root().is_empty());
}

#[test]
fn a_missing_file_is_a_one_line_error_naming_it() {
    let err = read_component_source(&fixture("nothing-here.dae")).unwrap_err();
    assert!(err.contains("nothing-here.dae"), "{err}");
    assert!(!err.contains('\n'), "{err}");
}

#[test]
fn editor_import_adds_a_component_and_starts_placing_it() {
    let mut editor = Editor::new();
    let mut model = Model::new();
    let source = square_model();
    let id = editor
        .import_component(&mut model, &source, "Slab")
        .unwrap();
    assert_eq!(components(&model), vec![(id, "Slab".to_string())]);
    assert_eq!(editor.placing(), Some(id));
    assert_eq!(editor.undo_stack().undo_count(), 1);
    assert_eq!(editor.undo_stack().undo_label(), Some("Import Component"));
    assert!(model.root().instances().next().is_none());
}

#[test]
fn editor_import_names_are_made_unique() {
    let mut editor = Editor::new();
    let mut model = Model::new();
    let source = square_model();
    editor
        .import_component(&mut model, &source, "Slab")
        .unwrap();
    editor
        .import_component(&mut model, &source, "Slab")
        .unwrap();
    let mut names: Vec<String> = components(&model).into_iter().map(|c| c.1).collect();
    names.sort();
    assert_eq!(names, vec!["Slab".to_string(), "Slab 2".to_string()]);
    assert_eq!(editor.undo_stack().undo_count(), 2);
}

#[test]
fn editor_import_is_undone_in_one_step() {
    let mut editor = Editor::new();
    let mut model = Model::new();
    editor
        .import_component(&mut model, &square_model(), "Slab")
        .unwrap();
    assert_eq!(components(&model).len(), 1);
    // The editor's undo key reverts the whole import.
    editor.key(modeler::input::EditKey::Undo, &mut model);
    assert!(components(&model).is_empty());
}

#[test]
fn editor_import_of_an_empty_model_is_refused_and_changes_nothing() {
    let mut editor = Editor::new();
    let mut model = Model::new();
    let err = editor
        .import_component(&mut model, &Model::new(), "Nothing")
        .unwrap_err();
    assert!(err.contains("Nothing to import"), "{err}");
    assert!(!err.contains('\n'));
    assert!(components(&model).is_empty());
    assert_eq!(editor.undo_stack().undo_count(), 0);
    assert_eq!(editor.placing(), None);
}
