// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-native-file-dialogs: what each Open/Save/Export mode asks the system picker for
//! (title, kind, filters, start folder, suggested name) and how a picked path is finished.
//! See docs/specs/m11-native-file-dialogs.md.

use std::path::PathBuf;

use drydock::document::OPEN_EXTENSIONS;
use drydock::file_dialogs::SaveMode;
use drydock::mesh_export::{ExportFormat, ExportOptions, StlLayout};
use drydock::mesh_import::MeshUnit;
use drydock::open_buttons::OpenMode;
use drydock::picker::{PickKind, PickPurpose, PickRequest, finish_path};

fn options(format: ExportFormat, layout: StlLayout) -> ExportOptions {
    ExportOptions {
        format,
        selection_only: false,
        unit: MeshUnit::default(),
        tolerance_mm: 0.1,
        layout,
    }
}

fn dir() -> PathBuf {
    PathBuf::from("/work/models")
}

#[test]
fn open_modes_ask_for_one_existing_model_file() {
    for (mode, title) in [
        (OpenMode::Open, "Open model"),
        (OpenMode::Component, "Import component"),
        (OpenMode::Models, "Import into model"),
    ] {
        let request = PickRequest::open(mode, dir());
        assert_eq!(request.purpose, PickPurpose::Open(mode));
        assert_eq!(request.kind, PickKind::OpenFile);
        assert_eq!(request.title, title);
        assert_eq!(request.dir, dir());
        assert_eq!(request.name, "");
        assert_eq!(request.filters.len(), 1, "{mode:?}");
        assert_eq!(request.filters[0].label, "Models");
        assert_eq!(request.filters[0].extensions, OPEN_EXTENSIONS);
    }
}

#[test]
fn save_asks_for_a_drydock_file_with_the_suggested_name() {
    let request = PickRequest::save(SaveMode::Model, dir(), "chair.drydock".to_owned());
    assert_eq!(request.purpose, PickPurpose::Save(SaveMode::Model));
    assert_eq!(request.kind, PickKind::SaveFile);
    assert_eq!(request.title, "Save model");
    assert_eq!(request.dir, dir());
    assert_eq!(request.name, "chair.drydock");
    assert_eq!(request.filters.len(), 1);
    assert_eq!(request.filters[0].label, "Drydock model");
    assert_eq!(request.filters[0].extensions, ["drydock"]);
}

#[test]
fn export_to_a_file_filters_by_the_format() {
    for (format, label, extension) in [
        (ExportFormat::ThreeMf, "3MF", "3mf"),
        (ExportFormat::Stl, "STL", "stl"),
    ] {
        let options = options(format, StlLayout::SingleFile);
        let mode = SaveMode::Export(options);
        let request = PickRequest::save(mode, dir(), format!("chair.{extension}"));
        assert_eq!(request.purpose, PickPurpose::Save(mode));
        assert_eq!(request.kind, PickKind::SaveFile);
        assert_eq!(request.title, format!("Export {label}"));
        assert_eq!(request.name, format!("chair.{extension}"));
        assert_eq!(request.filters.len(), 1);
        assert_eq!(request.filters[0].label, label);
        assert_eq!(request.filters[0].extensions, [extension]);
    }
}

#[test]
fn export_to_a_folder_is_a_folder_pick_without_filters_or_name() {
    let mode = SaveMode::ExportFolder(options(ExportFormat::Stl, StlLayout::OneFilePerObject));
    let request = PickRequest::save(mode, dir(), "chair".to_owned());
    assert_eq!(request.purpose, PickPurpose::Save(mode));
    assert_eq!(request.kind, PickKind::Folder);
    assert_eq!(request.title, "Export STL to folder");
    assert_eq!(request.dir, dir());
    assert!(request.filters.is_empty());
    assert_eq!(request.name, "");
}

#[test]
fn a_picked_open_path_or_folder_is_left_alone() {
    let path = PathBuf::from("/work/a.dae");
    for mode in [OpenMode::Open, OpenMode::Component, OpenMode::Models] {
        assert_eq!(finish_path(PickPurpose::Open(mode), path.clone()), path);
    }
    let folder = SaveMode::ExportFolder(options(ExportFormat::Stl, StlLayout::OneFilePerObject));
    let path = PathBuf::from("/work/parts");
    assert_eq!(finish_path(PickPurpose::Save(folder), path.clone()), path);
}

#[test]
fn a_picked_save_path_gets_the_native_extension() {
    let save = |name: &str| finish_path(PickPurpose::Save(SaveMode::Model), PathBuf::from(name));
    assert_eq!(save("/w/chair"), PathBuf::from("/w/chair.drydock"));
    assert_eq!(save("/w/chair.drydock"), PathBuf::from("/w/chair.drydock"));
    assert_eq!(save("/w/chair.modeler"), PathBuf::from("/w/chair.modeler"));
    assert_eq!(save("/w/chair.dae"), PathBuf::from("/w/chair.drydock"));
}

#[test]
fn a_picked_export_path_gets_the_formats_extension_only_when_it_has_none() {
    let stl = SaveMode::Export(options(ExportFormat::Stl, StlLayout::SingleFile));
    let mf = SaveMode::Export(options(ExportFormat::ThreeMf, StlLayout::SingleFile));
    let done = |mode, name: &str| finish_path(PickPurpose::Save(mode), PathBuf::from(name));
    assert_eq!(done(stl, "/w/part"), PathBuf::from("/w/part.stl"));
    assert_eq!(done(stl, "/w/part.stl"), PathBuf::from("/w/part.stl"));
    assert_eq!(done(mf, "/w/part"), PathBuf::from("/w/part.3mf"));
    assert_eq!(done(mf, "/w/part.v2"), PathBuf::from("/w/part.v2"));
}
