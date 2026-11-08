// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The app's own Open and Save dialogs, drawn in egui. They are the fallback when the system
//! has no file picker and the picker in UI tests (`crate::picker`). Moved out of `app.rs`
//! unchanged (`docs/specs/m11-native-file-dialogs.md`).

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Vec2};

use crate::dialog::{DialogWindow, modal_dialog_window};
use crate::document::{NATIVE_EXTENSION, is_openable_file, native_path};
use crate::look;
use crate::mesh_export::ExportOptions;
use crate::open_buttons::{self, OpenMode};

const ERROR_COLOR: Color32 = Color32::from_rgb(200, 60, 50);

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

pub enum DialogOutcome {
    Pending,
    Open(PathBuf),
    Cancel,
}

/// A minimal open-file dialog in plain egui: browse folders, pick a model file, or type
/// a path.
pub struct OpenDialog {
    dir: PathBuf,
    /// Subfolders and model files of `dir`, folders first, each sorted by name.
    entries: Vec<(String, PathBuf, bool)>,
    path_text: String,
    error: Option<String>,
    pub mode: OpenMode,
}

impl OpenDialog {
    pub fn new(dir: PathBuf) -> OpenDialog {
        OpenDialog::with_mode(dir, OpenMode::Open)
    }

    pub fn import(dir: PathBuf) -> OpenDialog {
        OpenDialog::with_mode(dir, OpenMode::Component)
    }

    pub fn models(dir: PathBuf) -> OpenDialog {
        OpenDialog::with_mode(dir, OpenMode::Models)
    }

    pub fn with_mode(dir: PathBuf, mode: OpenMode) -> OpenDialog {
        let mut dialog = OpenDialog {
            dir: PathBuf::new(),
            entries: Vec::new(),
            path_text: String::new(),
            error: None,
            mode,
        };
        dialog.enter(dir);
        dialog
    }

    fn enter(&mut self, dir: PathBuf) {
        self.error = None;
        self.entries.clear();
        match std::fs::read_dir(&dir) {
            Ok(read_dir) => {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let is_dir = path.is_dir();
                    let is_model = is_openable_file(&path);
                    if (is_dir && !name.starts_with('.')) || (!is_dir && is_model) {
                        self.entries.push((name, path, is_dir));
                    }
                }
            }
            Err(err) => self.error = Some(format!("{}: {err}", dir.display())),
        }
        self.entries.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
        });
        self.path_text = dir.display().to_string();
        self.dir = dir;
    }

    pub fn show(&mut self, ctx: &egui::Context) -> DialogOutcome {
        let mut outcome = DialogOutcome::Pending;
        let mut enter = None;
        let mut open = true;
        let title = match self.mode {
            OpenMode::Open => "Open model",
            OpenMode::Component => "Import component",
            OpenMode::Models => "Import into model",
        };
        let window = DialogWindow {
            title,
            size: Vec2::new(560.0, 480.0),
        };
        modal_dialog_window(
            ctx,
            egui::Id::new("open-dialog"),
            &window,
            &mut open,
            |ui| {
                ui.label(self.dir.display().to_string());
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if let Some(parent) = self.dir.parent()
                            && ui.selectable_label(false, "../").clicked()
                        {
                            enter = Some(parent.to_path_buf());
                        }
                        for (name, path, is_dir) in &self.entries {
                            let text = if *is_dir {
                                format!("{name}/")
                            } else {
                                name.clone()
                            };
                            if ui.selectable_label(false, text).clicked() {
                                if *is_dir {
                                    enter = Some(path.clone());
                                } else {
                                    outcome = DialogOutcome::Open(path.clone());
                                }
                            }
                        }
                    });
                ui.separator();
                let typed = ui.add(
                    egui::TextEdit::singleline(&mut self.path_text)
                        .desired_width(f32::INFINITY)
                        .hint_text("Path to a .dae, .modeler, .stl or .3mf file, or a folder"),
                );
                let submitted = typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if let Some(error) = &self.error {
                    ui.colored_label(ERROR_COLOR, error);
                }
                let buttons = open_buttons::open_buttons(ui, self.mode);
                if buttons.chosen || submitted {
                    let path = PathBuf::from(self.path_text.trim());
                    if path.is_dir() {
                        enter = Some(path);
                    } else if path.is_file() {
                        outcome = DialogOutcome::Open(path);
                    } else {
                        self.error = Some(format!("{}: no such file", path.display()));
                    }
                }
                if buttons.cancelled {
                    outcome = DialogOutcome::Cancel;
                }
            },
        );
        if let Some(dir) = enter {
            self.enter(dir);
        }
        if !open && matches!(outcome, DialogOutcome::Pending) {
            outcome = DialogOutcome::Cancel;
        }
        outcome
    }
}

pub enum SaveOutcome {
    Pending,
    Save(PathBuf),
    Cancel,
}

/// What a save dialog is choosing a name for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SaveMode {
    /// The open document (a `.modeler` file).
    Model,
    /// An export to one file.
    Export(ExportOptions),
    /// An export to a folder, one file per object.
    ExportFolder(ExportOptions),
}

/// A minimal save-file dialog in plain egui: browse folders, list matching files,
/// and choose a file or folder name.
pub struct SaveDialog {
    dir: PathBuf,
    /// Subfolders and `.modeler` files of `dir`, folders first, each sorted by name.
    entries: Vec<(String, PathBuf, bool)>,
    name_text: String,
    error: Option<String>,
    /// A target that already exists and awaits confirmation.
    replacing: Option<PathBuf>,
    /// What the name is for.
    pub mode: SaveMode,
}

impl SaveDialog {
    pub fn new(dir: PathBuf, name: String) -> SaveDialog {
        SaveDialog::with_mode(dir, name, SaveMode::Model)
    }

    pub fn with_mode(dir: PathBuf, name: String, mode: SaveMode) -> SaveDialog {
        let mut dialog = SaveDialog {
            dir: PathBuf::new(),
            entries: Vec::new(),
            name_text: name,
            error: None,
            replacing: None,
            mode,
        };
        dialog.enter(dir);
        dialog
    }

    fn enter(&mut self, dir: PathBuf) {
        self.error = None;
        self.replacing = None;
        self.entries.clear();
        match std::fs::read_dir(&dir) {
            Ok(read_dir) => {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let is_dir = path.is_dir();
                    let extension = match self.mode {
                        SaveMode::Model => Some(NATIVE_EXTENSION),
                        SaveMode::Export(options) => Some(options.format.extension()),
                        SaveMode::ExportFolder(_) => None,
                    };
                    let is_model = extension.is_some_and(|extension| {
                        path.extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
                    });
                    if (is_dir && !name.starts_with('.')) || (!is_dir && is_model) {
                        self.entries.push((name, path, is_dir));
                    }
                }
            }
            Err(err) => self.error = Some(format!("{}: {err}", dir.display())),
        }
        self.entries.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
        });
        self.dir = dir;
    }

    fn target(&self) -> Option<PathBuf> {
        let name = self.name_text.trim();
        if name.is_empty() {
            return None;
        }
        let path = self.dir.join(name);
        Some(match self.mode {
            SaveMode::Model => native_path(&path),
            SaveMode::Export(_) => path,
            SaveMode::ExportFolder(_) => path.with_extension(""),
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> SaveOutcome {
        let mut outcome = SaveOutcome::Pending;
        let mut enter = None;
        // Cancel on the replace question goes back to the file name.
        let mut back = false;
        let mut open = true;
        let title = match self.mode {
            SaveMode::Model => "Save model".to_string(),
            SaveMode::Export(options) => format!("Export {}", options.format.label()),
            SaveMode::ExportFolder(options) => {
                format!("Export {} to folder", options.format.label())
            }
        };
        let window = DialogWindow {
            title: &title,
            size: Vec2::new(560.0, 480.0),
        };
        modal_dialog_window(
            ctx,
            egui::Id::new("save-dialog"),
            &window,
            &mut open,
            |ui| {
                ui.label(self.dir.display().to_string());
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if let Some(parent) = self.dir.parent()
                            && ui.selectable_label(false, "../").clicked()
                        {
                            enter = Some(parent.to_path_buf());
                        }
                        for (name, path, is_dir) in &self.entries {
                            let text = if *is_dir {
                                format!("{name}/")
                            } else {
                                name.clone()
                            };
                            if ui.selectable_label(false, text).clicked() {
                                if *is_dir {
                                    enter = Some(path.clone());
                                } else {
                                    self.name_text = name.clone();
                                    self.replacing = None;
                                }
                            }
                        }
                    });
                ui.separator();
                if let Some(target) = &self.replacing {
                    ui.label(format!("Replace {}?", file_name(target)));
                    ui.horizontal(|ui| {
                        if look::primary_button(ui, "Replace").clicked() {
                            outcome = SaveOutcome::Save(target.clone());
                        }
                        if ui.button("Cancel").clicked() {
                            back = true;
                        }
                    });
                } else {
                    let hint = match self.mode {
                        SaveMode::ExportFolder(_) => "Folder name",
                        SaveMode::Model | SaveMode::Export(_) => "File name",
                    };
                    let name = ui.add(
                        egui::TextEdit::singleline(&mut self.name_text)
                            .desired_width(f32::INFINITY)
                            .hint_text(hint),
                    );
                    let submitted =
                        name.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if let Some(error) = &self.error {
                        ui.colored_label(ERROR_COLOR, error);
                    }
                    let choose = match self.mode {
                        SaveMode::Model => "Save",
                        SaveMode::Export(_) | SaveMode::ExportFolder(_) => "Export",
                    };
                    ui.horizontal(|ui| {
                        if (look::primary_button(ui, choose).clicked() || submitted)
                            && let Some(target) = self.target()
                        {
                            if matches!(self.mode, SaveMode::ExportFolder(_)) && target.is_dir() {
                                outcome = SaveOutcome::Save(target);
                            } else if target.exists() {
                                self.replacing = Some(target);
                            } else {
                                outcome = SaveOutcome::Save(target);
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            outcome = SaveOutcome::Cancel;
                        }
                    });
                }
            },
        );
        if back {
            self.replacing = None;
        }
        if let Some(dir) = enter {
            self.enter(dir);
        }
        if !open && matches!(outcome, SaveOutcome::Pending) {
            outcome = SaveOutcome::Cancel;
        }
        outcome
    }
}
