// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Document editing support (`docs/specs/m2-app-ui.md`).

use std::path::{Path, PathBuf};

use glam::DVec3;
use modeler::document::{Document, FileKind};
use modeler_dae::kernel_scene;
use modeler_kernel::model::{BodyId, UndoStack};
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::units::Unit;

fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("modeler-docedit-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn slab() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/slab.modeler")
}

/// Draw a closed 1 m square on the ground into the document's model.
fn draw_square(doc: &mut Document) {
    let model = doc.kernel.as_mut().expect("kernel model");
    let mut undo = UndoStack::new();
    let mut target = Target {
        model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    let p = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(1.0, 1.0, 0.0),
        DVec3::new(0.0, 1.0, 0.0),
    ];
    let segments: Vec<_> = (0..4).map(|i| (p[i], p[(i + 1) % 4])).collect();
    draw_segments(&mut target, &segments, "square").expect("square draws");
}

#[test]
fn new_empty_is_an_untitled_native_model() {
    let doc = Document::new_empty();
    assert_eq!(doc.path, PathBuf::from("untitled.modeler"));
    assert_eq!(doc.kind, FileKind::Native);
    assert!(doc.kernel.is_some());
    assert!(doc.kernel_error.is_none());
    assert!(doc.untitled);
    assert!(!doc.modified);
    assert_eq!(doc.title(), "untitled.modeler - modeler");
    assert_eq!(doc.stats.triangles, 0);
    assert_eq!(doc.stats.lines, 0);
    assert_eq!(doc.stats.faces, 0);
    assert_eq!(doc.stats.edges, 0);
    assert_eq!(doc.stats.size_m, [0.0, 0.0, 0.0]);
    assert_eq!(doc.scene.triangle_count(), 0);
    assert_eq!(doc.scene.line_count(), 0);
}

#[test]
fn refresh_rebuilds_scene_and_stats() {
    let mut doc = Document::new_empty();
    draw_square(&mut doc);
    // Not refreshed yet: the scene is stale.
    assert_eq!(doc.scene.triangle_count(), 0);
    doc.refresh();
    assert!(doc.modified);
    let want = kernel_scene(doc.kernel.as_ref().unwrap());
    assert_eq!(doc.scene.triangle_count(), want.triangle_count());
    assert_eq!(doc.scene.line_count(), want.line_count());
    assert!(doc.scene.triangle_count() >= 2, "the square is a face");
    assert_eq!(doc.stats.triangles, doc.scene.triangle_count());
    assert_eq!(doc.stats.lines, doc.scene.line_count());
    assert_eq!(doc.stats.faces, 1);
    assert_eq!(doc.stats.edges, 4);
    for (got, want) in doc.stats.size_m.iter().zip([1.0, 1.0, 0.0]) {
        assert!((got - want).abs() < 1e-5, "{:?}", doc.stats.size_m);
    }
}

#[test]
fn refresh_without_a_kernel_changes_nothing() {
    let mut doc = Document::new_empty();
    doc.kernel = None;
    let stats = doc.stats.clone();
    doc.refresh();
    assert!(!doc.modified);
    assert_eq!(doc.stats, stats);
}

#[test]
fn open_is_not_untitled_or_modified() {
    let doc = Document::open(&slab()).expect("slab.modeler opens");
    assert!(!doc.untitled);
    assert!(!doc.modified);
}

#[test]
fn save_as_clears_untitled_and_modified() {
    let dir = temp_dir("save-as");
    let mut doc = Document::new_empty();
    draw_square(&mut doc);
    doc.refresh();
    let written = doc.save_as(&dir.join("sq.modeler")).expect("saves");
    assert_eq!(doc.path, written);
    assert!(!doc.untitled);
    assert!(!doc.modified);
    let reopened = Document::open(&written).expect("reopens");
    assert_eq!(reopened.stats.faces, 1);
    assert_eq!(reopened.stats.edges, 4);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_after_edit_clears_modified() {
    let dir = temp_dir("save");
    let mut doc = Document::new_empty();
    doc.save_as(&dir.join("a.modeler")).unwrap();
    draw_square(&mut doc);
    doc.refresh();
    assert!(doc.modified);
    doc.save().expect("saves");
    assert!(!doc.modified);
    assert_eq!(Document::open(&doc.path).unwrap().stats.faces, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_on_untitled_asks_for_save_as() {
    let mut doc = Document::new_empty();
    draw_square(&mut doc);
    doc.refresh();
    let err = doc.save().expect_err("untitled has no file");
    assert!(err.contains("untitled.modeler"), "{err}");
    assert!(err.contains("Save As"), "{err}");
    assert!(doc.untitled);
    assert!(doc.modified);
}

#[test]
fn failed_save_leaves_flags() {
    let mut doc = Document::new_empty();
    draw_square(&mut doc);
    doc.refresh();
    let result = doc.save_as(Path::new("/nonexistent/dir/x.modeler"));
    assert!(result.is_err());
    assert!(doc.untitled);
    assert!(doc.modified);
    assert_eq!(doc.path, PathBuf::from("untitled.modeler"));
}

/// One more edge away from the origin: another undoable edit.
fn push_edit(model: &mut modeler_kernel::model::Model, undo: &mut UndoStack, x: f64) {
    let mut target = Target {
        model,
        body: BodyId::Root,
        undo,
        unit: Unit::Meter,
    };
    draw_segments(
        &mut target,
        &[(DVec3::new(5.0, 5.0, 0.0), DVec3::new(5.0 + x, 5.0, 0.0))],
        "edge",
    )
    .expect("edge draws");
}

#[test]
fn modified_follows_the_saved_undo_state() {
    let dir = temp_dir("sync");
    let mut doc = Document::open(&slab()).expect("slab opens");
    let mut undo = UndoStack::new();
    assert_eq!(doc.saved_state, 0);

    push_edit(doc.kernel.as_mut().unwrap(), &mut undo, 1.0);
    doc.refresh();
    doc.sync_modified(undo.state());
    assert!(doc.modified);

    doc.save_as(&dir.join("saved.modeler")).expect("saves");
    doc.mark_saved_at(undo.state());
    assert!(!doc.modified);
    assert_eq!(doc.saved_state, undo.state());
    assert!(!doc.title().contains('*'));

    // Undo: differs from the saved state.
    undo.undo(doc.kernel.as_mut().unwrap()).unwrap();
    doc.refresh();
    doc.sync_modified(undo.state());
    assert!(doc.modified);
    assert!(doc.title().contains('*'));

    // Redo: back at the saved state, so clean again.
    undo.redo(doc.kernel.as_mut().unwrap()).unwrap();
    doc.refresh();
    doc.sync_modified(undo.state());
    assert!(!doc.modified);
    assert!(!doc.title().contains('*'));
}

#[test]
fn undoing_back_to_the_open_state_is_clean() {
    let mut doc = Document::open(&slab()).expect("slab opens");
    let mut undo = UndoStack::new();
    push_edit(doc.kernel.as_mut().unwrap(), &mut undo, 1.0);
    doc.refresh();
    doc.sync_modified(undo.state());
    assert!(doc.modified);
    undo.undo(doc.kernel.as_mut().unwrap()).unwrap();
    doc.refresh();
    doc.sync_modified(undo.state());
    assert!(!doc.modified, "back at the opened state");
}

#[test]
fn a_different_edit_after_undo_is_not_the_saved_state() {
    let dir = temp_dir("branch");
    let mut doc = Document::open(&slab()).expect("slab opens");
    let mut undo = UndoStack::new();
    push_edit(doc.kernel.as_mut().unwrap(), &mut undo, 1.0);
    push_edit(doc.kernel.as_mut().unwrap(), &mut undo, 2.0);
    doc.save_as(&dir.join("b.modeler")).expect("saves");
    doc.mark_saved_at(undo.state());
    undo.undo(doc.kernel.as_mut().unwrap()).unwrap();
    push_edit(doc.kernel.as_mut().unwrap(), &mut undo, 3.0);
    doc.refresh();
    doc.sync_modified(undo.state());
    assert!(doc.modified, "same undo depth, different content");
}

#[test]
fn mark_saved_at_clears_modified_and_records_state() {
    let mut doc = Document::new_empty();
    doc.modified = true;
    doc.mark_saved_at(7);
    assert!(!doc.modified);
    assert_eq!(doc.saved_state, 7);
    doc.sync_modified(7);
    assert!(!doc.modified);
    doc.sync_modified(8);
    assert!(doc.modified);
}
