// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-picker-after-prompt: a picker asked for by a modal prompt starts only after the
//! prompt's window is gone, and the main window is released whenever the picker answers with
//! none of our prompts open. The Windows behaviour itself cannot run in the gate; these tests
//! pin the queue, the quiet-frame gate and the release hook.
//! See docs/specs/m11-picker-after-prompt.md.

use std::cell::Cell;
use std::path::PathBuf;

use drydock::file_dialogs::SaveMode;
use drydock::mesh_export::{ExportFormat, ExportOptions, StlLayout};
use drydock::picker::{
    Backend, ModalGate, PickPurpose, PickRequest, Picker, QUIET_FRAMES, Started, StubPicker,
};
use eframe::egui;

fn stub() -> (StubPicker, Picker) {
    let stub = StubPicker::new();
    (stub.clone(), Picker::new(Backend::Stub(stub)))
}

fn export_request() -> PickRequest {
    let mut options = ExportOptions::new(ExportFormat::Stl);
    options.layout = StlLayout::SingleFile;
    PickRequest::save(
        SaveMode::Export(options),
        PathBuf::from("/w"),
        "chair.stl".to_owned(),
    )
}

#[test]
fn the_gate_opens_after_quiet_frames_in_a_row() {
    let mut gate = ModalGate::new();
    const { assert!(QUIET_FRAMES >= 2) };
    for _ in 0..3 {
        assert!(!gate.frame(true));
    }
    for _ in 0..QUIET_FRAMES - 1 {
        assert!(!gate.frame(false));
    }
    assert!(gate.frame(false));
    assert!(gate.frame(false));
}

#[test]
fn a_prompt_frame_closes_the_gate_again() {
    let mut gate = ModalGate::new();
    for _ in 0..QUIET_FRAMES {
        gate.frame(false);
    }
    assert!(gate.frame(false));
    assert!(!gate.frame(true));
    assert!(!gate.frame(false));
}

#[test]
fn a_queued_request_does_not_reach_the_backend_until_allowed() {
    let ctx = egui::Context::default();
    let (stub, mut picker) = stub();
    picker.queue(export_request());
    assert!(picker.has_queued());
    assert!(picker.is_busy());
    assert!(picker.start_queued(&ctx, None, false).is_none());
    assert!(stub.requests().is_empty());
    assert!(picker.has_queued());
    let (started, request) = picker.start_queued(&ctx, None, true).expect("started");
    assert_eq!(started, Started::Native);
    assert_eq!(request, export_request());
    assert_eq!(stub.requests(), vec![export_request()]);
    assert!(!picker.has_queued());
}

#[test]
fn a_prompt_then_ok_starts_the_picker_exactly_once_on_a_later_frame() {
    // The frames of an Export > OK: the prompt is drawn, OK queues the picker, the prompt is
    // gone from the next frame on.
    let ctx = egui::Context::default();
    let (stub, mut picker) = stub();
    let mut gate = ModalGate::new();
    let mut started_on = Vec::new();
    for frame in 0..6 {
        let prompt_drawn = frame == 0;
        let may_start = gate.frame(prompt_drawn);
        if prompt_drawn {
            picker.queue(export_request());
        }
        if picker.start_queued(&ctx, None, may_start).is_some() {
            started_on.push(frame);
        }
        if frame == 0 {
            assert!(stub.requests().is_empty(), "nothing in the prompt's frame");
        }
    }
    assert_eq!(started_on.len(), 1);
    assert!(started_on[0] > 0);
    assert_eq!(stub.requests(), vec![export_request()]);
}

#[test]
fn a_later_queued_request_replaces_an_earlier_one() {
    let ctx = egui::Context::default();
    let (stub, mut picker) = stub();
    picker.queue(PickRequest::open(
        drydock::open_buttons::OpenMode::Open,
        PathBuf::from("/w"),
    ));
    picker.queue(export_request());
    picker.start_queued(&ctx, None, true).expect("started");
    assert_eq!(stub.requests(), vec![export_request()]);
}

#[test]
fn a_queued_request_waits_while_another_is_out() {
    let ctx = egui::Context::default();
    let (stub, mut picker) = stub();
    picker.start(&ctx, None, export_request());
    picker.queue(export_request());
    assert!(picker.start_queued(&ctx, None, true).is_none());
    assert!(picker.has_queued());
    assert_eq!(stub.requests().len(), 1);
    assert!(picker.poll().is_some());
    assert!(picker.start_queued(&ctx, None, true).is_some());
    assert_eq!(stub.requests().len(), 2);
}

#[test]
fn every_answer_releases_the_main_window_when_no_prompt_is_open() {
    let ctx = egui::Context::default();
    for answer in [Some(PathBuf::from("/w/a.stl")), None] {
        let (stub, mut picker) = stub();
        stub.answer(answer.clone());
        picker.start(&ctx, None, export_request());
        let released = Cell::new(0);
        let picked = picker
            .poll_releasing(false, || released.set(released.get() + 1))
            .expect("answered");
        assert_eq!(picked.path.is_some(), answer.is_some());
        assert!(matches!(picked.purpose, PickPurpose::Save(_)));
        assert_eq!(released.get(), 1);
        // No second answer, no second release.
        assert!(
            picker
                .poll_releasing(false, || released.set(released.get() + 1))
                .is_none()
        );
        assert_eq!(released.get(), 1);
    }
}

#[test]
fn no_release_while_a_prompt_is_open_or_nothing_has_answered() {
    let ctx = egui::Context::default();
    let (_stub, mut picker) = stub();
    let released = Cell::new(0);
    assert!(
        picker
            .poll_releasing(false, || released.set(released.get() + 1))
            .is_none()
    );
    picker.start(&ctx, None, export_request());
    let picked = picker.poll_releasing(true, || released.set(released.get() + 1));
    assert!(picked.is_some(), "the answer still comes back");
    assert_eq!(released.get(), 0);
}
