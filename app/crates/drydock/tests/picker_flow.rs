// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-native-file-dialogs: the picker's backends. The stub answers through the same channel as
//! the system picker; the system probe and the backend choice are plain functions.
//! See docs/specs/m11-native-file-dialogs.md.

use std::path::PathBuf;

use drydock::file_dialogs::SaveMode;
use drydock::open_buttons::OpenMode;
use drydock::picker::{
    Backend, PickPurpose, PickRequest, Picked, Picker, Started, StubPicker, choose_backend,
    lists_file_chooser,
};
use eframe::egui;

fn open_request(mode: OpenMode) -> PickRequest {
    PickRequest::open(mode, PathBuf::from("/w"))
}

fn stub() -> (StubPicker, Picker) {
    let stub = StubPicker::new();
    (stub.clone(), Picker::new(Backend::Stub(stub)))
}

#[test]
fn a_stub_answer_comes_back_once_with_the_purpose() {
    let ctx = egui::Context::default();
    let (stub, mut picker) = stub();
    stub.answer(Some(PathBuf::from("/w/a.dae")));
    assert_eq!(
        picker.start(&ctx, None, open_request(OpenMode::Models)),
        Started::Native
    );
    assert!(picker.is_busy());
    assert_eq!(
        picker.poll(),
        Some(Picked {
            purpose: PickPurpose::Open(OpenMode::Models),
            path: Some(PathBuf::from("/w/a.dae")),
        })
    );
    assert!(!picker.is_busy());
    assert_eq!(picker.poll(), None);
}

#[test]
fn cancel_is_an_answer_without_a_path() {
    let ctx = egui::Context::default();
    let (stub, mut picker) = stub();
    stub.answer(None);
    picker.start(&ctx, None, open_request(OpenMode::Open));
    let picked = picker.poll().expect("answered");
    assert_eq!(picked.purpose, PickPurpose::Open(OpenMode::Open));
    assert_eq!(picked.path, None);
    // With nothing queued the stub cancels too.
    picker.start(&ctx, None, open_request(OpenMode::Component));
    assert_eq!(picker.poll().expect("answered").path, None);
}

#[test]
fn the_stub_records_every_request_in_order() {
    let ctx = egui::Context::default();
    let (stub, mut picker) = stub();
    let first = open_request(OpenMode::Component);
    let second = PickRequest::save(SaveMode::Model, PathBuf::from("/w"), "a.drydock".to_owned());
    picker.start(&ctx, None, first.clone());
    picker.poll();
    picker.start(&ctx, None, second.clone());
    picker.poll();
    assert_eq!(stub.requests(), [first, second]);
}

#[test]
fn answers_are_used_in_the_order_queued() {
    let ctx = egui::Context::default();
    let (stub, mut picker) = stub();
    stub.answer(Some(PathBuf::from("/w/one.stl")));
    stub.answer(Some(PathBuf::from("/w/two.stl")));
    picker.start(&ctx, None, open_request(OpenMode::Open));
    assert_eq!(
        picker.poll().unwrap().path,
        Some(PathBuf::from("/w/one.stl"))
    );
    picker.start(&ctx, None, open_request(OpenMode::Open));
    assert_eq!(
        picker.poll().unwrap().path,
        Some(PathBuf::from("/w/two.stl"))
    );
}

#[test]
fn a_second_request_while_one_is_out_is_ignored() {
    let ctx = egui::Context::default();
    let (stub, mut picker) = stub();
    stub.answer(Some(PathBuf::from("/w/one.stl")));
    stub.answer(Some(PathBuf::from("/w/two.stl")));
    picker.start(&ctx, None, open_request(OpenMode::Open));
    assert_eq!(
        picker.start(&ctx, None, open_request(OpenMode::Models)),
        Started::Native
    );
    assert_eq!(stub.requests().len(), 1);
    let picked = picker.poll().unwrap();
    assert_eq!(picked.purpose, PickPurpose::Open(OpenMode::Open));
    assert_eq!(picked.path, Some(PathBuf::from("/w/one.stl")));
    assert_eq!(picker.poll(), None);
}

#[test]
fn the_stub_finishes_a_save_path_like_the_system_picker() {
    let ctx = egui::Context::default();
    let (stub, mut picker) = stub();
    stub.answer(Some(PathBuf::from("/w/chair")));
    picker.start(
        &ctx,
        None,
        PickRequest::save(
            SaveMode::Model,
            PathBuf::from("/w"),
            "chair.drydock".to_owned(),
        ),
    );
    assert_eq!(
        picker.poll().unwrap().path,
        Some(PathBuf::from("/w/chair.drydock"))
    );
}

#[test]
fn the_builtin_backend_hands_the_request_back_to_the_app() {
    let ctx = egui::Context::default();
    let mut picker = Picker::new(Backend::Builtin);
    assert_eq!(
        picker.start(&ctx, None, open_request(OpenMode::Open)),
        Started::Builtin
    );
    assert!(!picker.is_busy());
    assert_eq!(picker.poll(), None);
}

#[test]
fn the_backend_follows_the_setting_then_availability() {
    let is = |backend: Backend, system: bool| match backend {
        Backend::System => system,
        Backend::Builtin => !system,
        Backend::Stub(_) => false,
    };
    assert!(is(choose_backend(None, true), true));
    assert!(is(choose_backend(None, false), false));
    assert!(is(choose_backend(Some(""), true), true));
    assert!(is(choose_backend(Some("other"), false), false));
    assert!(is(choose_backend(Some("builtin"), true), false));
    assert!(is(choose_backend(Some("system"), false), true));
    assert!(is(choose_backend(Some("system"), true), true));
}

#[test]
fn the_portal_probe_looks_for_the_file_chooser_interface() {
    let with = "node /org/freedesktop/portal/desktop {\n  interface org.freedesktop.portal.Settings {\n  };\n  interface org.freedesktop.portal.FileChooser {\n    methods:\n      OpenFile(...);\n  };\n};\n";
    let without = "node /org/freedesktop/portal/desktop {\n  interface org.freedesktop.portal.Settings {\n  };\n};\n";
    assert!(lists_file_chooser(with));
    assert!(!lists_file_chooser(without));
    assert!(!lists_file_chooser(""));
    assert!(!lists_file_chooser(
        "Error: GDBus.Error:org.freedesktop.DBus.Error.ServiceUnknown"
    ));
    // A longer interface name is not the file chooser.
    assert!(!lists_file_chooser(
        "interface org.freedesktop.portal.FileChooserExtra {"
    ));
}
