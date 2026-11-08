// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-modal-dialogs: the unsaved-changes, mesh-units, 3D Text and export prompts, and the
//! Purge Unused question, as modal windows (egui_kittest, embedded). See
//! docs/specs/m11-modal-dialogs.md.

use drydock::materials_panel::{MaterialAction, MaterialsPanel, materials_panel};
use drydock::mesh_export::{ExportDialog, ExportFormat, ExportOptions, ExportOutcome};
use drydock::mesh_import::{MeshUnit, UnitsOutcome};
use drydock::prompts::{UnsavedChoice, export_prompt, text_prompt, units_prompt, unsaved_prompt};
use drydock::text_tool::{TextDialog, TextOutcome};
use drydock_kernel::model::{Material, Model, ModelEdit};
use eframe::egui::{self, Key, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A harness with a "Main button" under the prompt; `ask` runs the prompt every frame while
/// `answer` is `None` and stores what it returns.
struct Ask<T, D> {
    data: D,
    answer: Option<T>,
    main_clicks: u32,
}

fn harness<T: 'static, D: 'static>(
    data: D,
    mut ask: impl FnMut(&egui::Context, &mut D) -> Option<T> + 'static,
) -> Harness<'static, Ask<T, D>> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(700.0, 500.0))
        .build_ui_state(
            move |ui, state: &mut Ask<T, D>| {
                if ui.button("Main button").clicked() {
                    state.main_clicks += 1;
                }
                if state.answer.is_none() {
                    state.answer = ask(ui.ctx(), &mut state.data);
                }
            },
            Ask {
                data,
                answer: None,
                main_clicks: 0,
            },
        );
    h.run();
    assert!(h.ctx.embed_viewports(), "kittest embeds viewports");
    h
}

// ---- unsaved changes ----

fn unsaved() -> Harness<'static, Ask<UnsavedChoice, ()>> {
    harness((), |ctx, _| unsaved_prompt(ctx, "house.drydock"))
}

#[test]
fn unsaved_shows_title_question_and_three_buttons() {
    let h = unsaved();
    h.get_by_label("Unsaved changes");
    h.get_by_label("Save changes to house.drydock?");
    h.get_by_label("Save");
    h.get_by_label("Discard");
    h.get_by_label("Cancel");
}

#[test]
fn unsaved_buttons_answer() {
    for (label, want) in [
        ("Save", UnsavedChoice::Save),
        ("Discard", UnsavedChoice::Discard),
        ("Cancel", UnsavedChoice::Cancel),
    ] {
        let mut h = unsaved();
        h.get_by_label(label).click();
        h.run();
        assert_eq!(h.state().answer, Some(want), "{label}");
    }
}

#[test]
fn unsaved_escape_is_cancel_and_the_main_window_is_blocked() {
    let mut h = unsaved();
    h.get_by_label("Main button").click();
    h.run();
    assert_eq!(h.state().main_clicks, 0, "the main window ignores clicks");
    assert_eq!(h.state().answer, None);
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state().answer, Some(UnsavedChoice::Cancel));
}

// ---- mesh units ----

fn units() -> Harness<'static, Ask<UnitsOutcome, MeshUnit>> {
    harness(MeshUnit::Millimeters, |ctx, unit| {
        units_prompt(ctx, "part.stl", unit)
    })
}

#[test]
fn units_shows_title_file_and_units() {
    let h = units();
    h.get_by_label("Mesh units");
    h.get_by_label("Choose the unit of part.stl");
    for unit in MeshUnit::ALL {
        h.get_by_label(unit.label());
    }
}

#[test]
fn units_ok_keeps_the_chosen_unit() {
    let mut h = units();
    h.get_by_label(MeshUnit::Inches.label()).click();
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().answer, Some(UnitsOutcome::Ok));
    assert_eq!(h.state().data, MeshUnit::Inches);
}

#[test]
fn units_cancel_and_escape_cancel() {
    let mut h = units();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().answer, Some(UnitsOutcome::Cancel));
    let mut h = units();
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state().answer, Some(UnitsOutcome::Cancel));
}

// ---- 3D Text ----

fn text() -> Harness<'static, Ask<TextOutcome, TextDialog>> {
    let mut dialog = TextDialog::new();
    dialog.text = "Hi".to_string();
    harness(dialog, text_prompt)
}

#[test]
fn text_shows_title_and_fields() {
    let h = text();
    h.get_by_label("3D Text");
    h.get_by_label("Font size (mm)");
    h.get_by_label("Emboss");
    h.get_by_label("Engrave");
}

#[test]
fn text_ok_and_escape() {
    let mut h = text();
    h.get_by_label("Engrave").click();
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().answer, Some(TextOutcome::Ok));
    assert_eq!(
        h.state().data.mode,
        drydock_kernel::text3d::TextMode::Engrave
    );
    let mut h = text();
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state().answer, Some(TextOutcome::Cancel));
}

// ---- export options ----

fn export(format: ExportFormat) -> Harness<'static, Ask<ExportOutcome, ExportDialog>> {
    harness(
        ExportDialog::new(ExportOptions::new(format)),
        |ctx, dialog| export_prompt(ctx, dialog, false),
    )
}

#[test]
fn export_title_names_the_format() {
    let h = export(ExportFormat::Stl);
    h.get_by_label(&format!("Export {}", ExportFormat::Stl.label()));
    h.get_by_label("Whole model");
    h.get_by_label("Curve tolerance (mm)");
    let h = export(ExportFormat::ThreeMf);
    h.get_by_label(&format!("Export {}", ExportFormat::ThreeMf.label()));
}

#[test]
fn export_ok_cancel_escape() {
    let mut h = export(ExportFormat::Stl);
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().answer, Some(ExportOutcome::Ok));
    let mut h = export(ExportFormat::Stl);
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().answer, Some(ExportOutcome::Cancel));
    let mut h = export(ExportFormat::Stl);
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state().answer, Some(ExportOutcome::Cancel));
}

#[test]
fn export_main_window_is_blocked() {
    let mut h = export(ExportFormat::Stl);
    h.get_by_label("Main button").click();
    h.run();
    assert_eq!(h.state().main_clicks, 0);
    assert_eq!(h.state().answer, None);
}

// ---- purge unused ----

struct Purge {
    panel: MaterialsPanel,
    model: Model,
    actions: Vec<MaterialAction>,
    main_clicks: u32,
}

fn purge() -> Harness<'static, Purge> {
    let mut model = Model::new();
    model.add_material(
        Material::new("Brick", [0.5, 0.4, 0.3, 1.0]),
        &mut ModelEdit::new(),
    );
    let mut h = Harness::builder()
        .with_size(Vec2::new(800.0, 600.0))
        .build_ui_state(
            |ui, state: &mut Purge| {
                if ui.button("Main button").clicked() {
                    state.main_clicks += 1;
                }
                if let Some(action) = materials_panel(ui, &mut state.panel, &state.model, None) {
                    state.actions.push(action);
                }
            },
            Purge {
                panel: MaterialsPanel::default(),
                model,
                actions: Vec::new(),
                main_clicks: 0,
            },
        );
    h.run();
    h.get_by_label("Purge Unused").click();
    h.run();
    h
}

#[test]
fn purge_has_its_own_title_and_blocks_the_main_window() {
    let mut h = purge();
    assert!(h.state().panel.confirming());
    h.get_by_label("Purge Unused Materials");
    h.get_by_label("Brick");
    h.get_by_label("Main button").click();
    h.run();
    assert_eq!(h.state().main_clicks, 0);
    assert!(h.state().panel.confirming());
}

#[test]
fn purge_cancel_closes_and_purge_sends_the_action() {
    let mut h = purge();
    h.get_by_label("Cancel").click();
    h.run();
    assert!(!h.state().panel.confirming());
    assert!(h.state().actions.is_empty());
    let mut h = purge();
    h.get_by_label("Purge").click();
    h.run();
    assert_eq!(h.state().actions.len(), 1);
    assert!(!h.state().panel.confirming());
}
