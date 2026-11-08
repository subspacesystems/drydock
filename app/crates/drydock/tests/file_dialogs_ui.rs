// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-native-file-dialogs: our own Open and Save dialogs, now in `file_dialogs`, behave as
//! before (they stay as the fallback picker). Driven with egui_kittest.
//! See docs/specs/m11-native-file-dialogs.md.

use std::path::PathBuf;

use drydock::file_dialogs::{DialogOutcome, OpenDialog, SaveDialog, SaveMode, SaveOutcome};
use drydock::open_buttons::OpenMode;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "drydock-file-dialogs-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("chair.drydock"), b"x").unwrap();
    std::fs::write(dir.join("notes.txt"), b"x").unwrap();
    dir
}

#[test]
fn the_open_dialog_lists_models_and_picks_one() {
    let dir = scratch("open");
    let mut harness = Harness::builder()
        .with_size(egui::vec2(700.0, 600.0))
        .build_ui_state(
            |ui, state: &mut (OpenDialog, Option<PathBuf>)| {
                if let DialogOutcome::Open(path) = state.0.show(ui.ctx()) {
                    state.1 = Some(path);
                }
            },
            (OpenDialog::with_mode(dir.clone(), OpenMode::Open), None),
        );
    harness.run();
    assert!(harness.query_by_label("notes.txt").is_none());
    harness.get_by_label("chair.drydock").click();
    harness.run();
    assert_eq!(harness.state().1, Some(dir.join("chair.drydock")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_save_dialog_saves_under_the_typed_name() {
    let dir = scratch("save");
    let mut harness = Harness::builder()
        .with_size(egui::vec2(700.0, 600.0))
        .build_ui_state(
            |ui, state: &mut (SaveDialog, Option<PathBuf>)| {
                if let SaveOutcome::Save(path) = state.0.show(ui.ctx()) {
                    state.1 = Some(path);
                }
            },
            (
                SaveDialog::with_mode(dir.clone(), "fresh".to_owned(), SaveMode::Model),
                None,
            ),
        );
    harness.run();
    harness.get_by_label("Save").click();
    harness.run();
    assert_eq!(harness.state().1, Some(dir.join("fresh.drydock")));
    let _ = std::fs::remove_dir_all(&dir);
}
