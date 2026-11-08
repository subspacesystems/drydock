// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-modal-file-dialogs: our own Open and Save dialogs as modal windows (egui_kittest,
//! embedded). See docs/specs/m11-modal-file-dialogs.md.

use std::path::PathBuf;

use drydock::file_dialogs::{DialogOutcome, OpenDialog, SaveDialog, SaveMode, SaveOutcome};
use drydock::mesh_export::{ExportFormat, ExportOptions};
use drydock::open_buttons::OpenMode;
use eframe::egui::{self, Key, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A fresh folder with `house.drydock`, `part.stl` and a subfolder `sub`.
fn folder(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("modeler-mfd-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("house.drydock"), b"x").unwrap();
    std::fs::write(dir.join("part.stl"), b"x").unwrap();
    dir
}

struct Run<D, O> {
    dialog: D,
    outcome: Option<O>,
    main_clicks: u32,
}

fn harness<D: 'static, O: 'static>(
    dialog: D,
    mut show: impl FnMut(&mut D, &egui::Context) -> O + 'static,
    pending: impl Fn(&O) -> bool + 'static,
) -> Harness<'static, Run<D, O>> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(900.0, 700.0))
        .build_ui_state(
            move |ui, state: &mut Run<D, O>| {
                if ui.button("Main button").clicked() {
                    state.main_clicks += 1;
                }
                if state.outcome.is_none() {
                    let out = show(&mut state.dialog, ui.ctx());
                    if !pending(&out) {
                        state.outcome = Some(out);
                    }
                }
            },
            Run {
                dialog,
                outcome: None,
                main_clicks: 0,
            },
        );
    h.run();
    assert!(h.ctx.embed_viewports(), "kittest embeds viewports");
    h
}

fn open(dir: PathBuf, mode: OpenMode) -> Harness<'static, Run<OpenDialog, DialogOutcome>> {
    harness(
        OpenDialog::with_mode(dir, mode),
        |d, ctx| d.show(ctx),
        |o| matches!(o, DialogOutcome::Pending),
    )
}

fn save(
    dir: PathBuf,
    name: &str,
    mode: SaveMode,
) -> Harness<'static, Run<SaveDialog, SaveOutcome>> {
    harness(
        SaveDialog::with_mode(dir, name.to_string(), mode),
        |d, ctx| d.show(ctx),
        |o| matches!(o, SaveOutcome::Pending),
    )
}

// ---- Open ----

#[test]
fn open_titles_are_the_window_title_per_mode() {
    for (mode, title) in [
        (OpenMode::Open, "Open model"),
        (OpenMode::Component, "Import component"),
        (OpenMode::Models, "Import into model"),
    ] {
        let h = open(folder("titles"), mode);
        h.get_by_label(title);
        // The title is drawn once (by the window), not again as a heading.
        assert_eq!(h.get_all_by_label(title).count(), 1, "{title}");
    }
}

#[test]
fn open_lists_folders_and_model_files() {
    let h = open(folder("list"), OpenMode::Open);
    h.get_by_label("sub/");
    h.get_by_label("house.drydock");
    h.get_by_label("part.stl");
}

#[test]
fn open_picking_a_file_gives_its_path() {
    let dir = folder("pick");
    let mut h = open(dir.clone(), OpenMode::Open);
    h.get_by_label("house.drydock").click();
    h.run();
    match &h.state().outcome {
        Some(DialogOutcome::Open(path)) => assert_eq!(path, &dir.join("house.drydock")),
        _ => panic!("expected Open(path)"),
    }
}

#[test]
fn open_blocks_the_main_window_and_escape_cancels() {
    let mut h = open(folder("esc"), OpenMode::Open);
    h.get_by_label("Main button").click();
    h.run();
    assert_eq!(h.state().main_clicks, 0, "the main window ignores clicks");
    assert!(h.state().outcome.is_none());
    h.key_press(Key::Escape);
    h.run();
    assert!(matches!(h.state().outcome, Some(DialogOutcome::Cancel)));
}

#[test]
fn open_cancel_button_cancels() {
    let mut h = open(folder("cancel"), OpenMode::Component);
    h.get_by_label("Cancel").click();
    h.run();
    assert!(matches!(h.state().outcome, Some(DialogOutcome::Cancel)));
}

#[test]
fn open_entering_a_folder_keeps_the_dialog_open() {
    let mut h = open(folder("enter"), OpenMode::Open);
    h.get_by_label("sub/").click();
    h.run();
    assert!(h.state().outcome.is_none());
    h.get_by_label("Open model");
}

// ---- Save ----

#[test]
fn save_titles_are_the_window_title_per_mode() {
    let stl = ExportOptions::new(ExportFormat::Stl);
    let mf = ExportOptions::new(ExportFormat::ThreeMf);
    for (mode, title) in [
        (SaveMode::Model, "Save model"),
        (SaveMode::Export(stl), "Export STL"),
        (SaveMode::Export(mf), "Export 3MF"),
        (SaveMode::ExportFolder(stl), "Export STL to folder"),
    ] {
        let h = save(folder("stitles"), "out", mode);
        h.get_by_label(title);
        assert_eq!(h.get_all_by_label(title).count(), 1, "{title}");
    }
}

#[test]
fn save_new_name_gives_the_native_path() {
    let dir = folder("new");
    let mut h = save(dir.clone(), "fresh", SaveMode::Model);
    h.get_by_label("Save").click();
    h.run();
    match &h.state().outcome {
        Some(SaveOutcome::Save(path)) => assert_eq!(path, &dir.join("fresh.drydock")),
        _ => panic!("expected Save(path)"),
    }
}

#[test]
fn save_over_an_existing_file_asks_to_replace() {
    let dir = folder("replace");
    let mut h = save(dir.clone(), "house", SaveMode::Model);
    h.get_by_label("Save").click();
    h.run();
    assert!(h.state().outcome.is_none());
    h.get_by_label("Replace house.drydock?");
    h.get_by_label("Replace").click();
    h.run();
    match &h.state().outcome {
        Some(SaveOutcome::Save(path)) => assert_eq!(path, &dir.join("house.drydock")),
        _ => panic!("expected Save(path)"),
    }
}

#[test]
fn save_cancel_on_the_replace_question_goes_back_to_the_name() {
    let mut h = save(folder("back"), "house", SaveMode::Model);
    h.get_by_label("Save").click();
    h.run();
    h.get_by_label("Replace house.drydock?");
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().outcome.is_none(), "the dialog stays open");
    assert!(h.query_by_label("Replace house.drydock?").is_none());
    h.get_by_label("Save");
}

#[test]
fn save_blocks_the_main_window_and_escape_cancels() {
    let mut h = save(folder("sesc"), "out", SaveMode::Model);
    h.get_by_label("Main button").click();
    h.run();
    assert_eq!(h.state().main_clicks, 0);
    assert!(h.state().outcome.is_none());
    h.key_press(Key::Escape);
    h.run();
    assert!(matches!(h.state().outcome, Some(SaveOutcome::Cancel)));
}

#[test]
fn save_cancel_button_cancels() {
    let mut h = save(
        folder("scancel"),
        "out",
        SaveMode::Export(ExportOptions::new(ExportFormat::Stl)),
    );
    h.get_by_label("Cancel").click();
    h.run();
    assert!(matches!(h.state().outcome, Some(SaveOutcome::Cancel)));
}
